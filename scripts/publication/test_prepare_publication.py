from __future__ import annotations

import importlib.util
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("prepare_publication.py")
SPEC = importlib.util.spec_from_file_location("prepare_publication", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


SOURCE_EMAIL = "private-author@example.invalid"
NOREPLY_EMAIL = "12345+publication-test@users.noreply.github.com"


def git(repo: Path, *args: str, env: dict[str, str] | None = None, check: bool = True) -> subprocess.CompletedProcess[str]:
    command_env = os.environ.copy()
    if env:
        command_env.update(env)
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=command_env,
        check=check,
    )


class SyntheticRepository:
    def __init__(self, root: Path, *, all_postbaseline_noreply: bool = False) -> None:
        self.root = root
        root.mkdir()
        git(root, "init", "--initial-branch=screenwise")
        git(root, "config", "user.name", "Publication Test")
        git(root, "config", "user.email", SOURCE_EMAIL)
        (root / ".gitignore").write_text("/.local/\n", encoding="utf-8")
        git(root, "add", "--", ".gitignore")
        self.commit("baseline-one", "one.txt", "one\n")
        self.baseline = self.commit("baseline-two", "two.txt", "two\n")
        if all_postbaseline_noreply:
            git(root, "config", "user.email", NOREPLY_EMAIL)
        self.local_one = self.commit("local-one", "one.txt", "changed\n")
        git(root, "config", "user.email", NOREPLY_EMAIL)
        self.local_two = self.commit("local-two", "three.txt", "three\n")

    def commit(self, message: str, relative: str, content: str, env: dict[str, str] | None = None) -> str:
        (self.root / relative).write_text(content, encoding="utf-8")
        git(self.root, "add", "--", relative)
        git(self.root, "commit", "-m", message, env=env)
        return git(self.root, "rev-parse", "HEAD").stdout.strip()


class PublicationPreparationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = SyntheticRepository(self.root / "source")

    def run_script(self, *extra: str, check: bool = True) -> subprocess.CompletedProcess[str]:
        result = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                "--source",
                str(self.source.root),
                "--baseline",
                self.source.baseline,
                "--noreply-email",
                NOREPLY_EMAIL,
                *extra,
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if check and result.returncode != 0:
            self.fail(f"script failed: {result.stderr}")
        return result

    def test_dry_run_writes_nothing_and_never_prints_source_email(self) -> None:
        (self.source.root / "uncommitted.txt").write_text("excluded\n", encoding="utf-8")
        result = self.run_script()
        self.assertEqual(result.returncode, 0)
        self.assertIn("mode=dry-run", result.stdout)
        self.assertIn("uncommitted changes excluded", result.stdout)
        self.assertNotIn(SOURCE_EMAIL, result.stdout + result.stderr)
        self.assertFalse((self.source.root / ".local" / "publication").exists())

    def test_execute_preserves_baseline_and_rewrites_only_email(self) -> None:
        original_tip = git(self.source.root, "rev-parse", "HEAD").stdout.strip()
        original_status = git(self.source.root, "status", "--porcelain=v1").stdout
        result = self.run_script("--execute")
        self.assertNotIn(SOURCE_EMAIL, result.stdout + result.stderr)
        self.assertIn("commits_already_noreply=1", result.stdout)
        self.assertIn("commits_requiring_email_redaction=1", result.stdout)
        candidate = self.source.root / ".local" / "publication" / "screenwise-public.git"
        self.assertTrue(candidate.is_dir())
        self.assertEqual(git(self.source.root, "rev-parse", "HEAD").stdout.strip(), original_tip)
        self.assertEqual(git(self.source.root, "status", "--porcelain=v1").stdout, original_status)
        self.assertEqual(git(self.source.root, "config", "--get", "user.email").stdout.strip(), NOREPLY_EMAIL)
        self.assertEqual(git(candidate, "rev-list", "--count", f"{self.source.baseline}..screenwise").stdout.strip(), "2")
        self.assertEqual(git(candidate, "cat-file", "-t", self.source.baseline).stdout.strip(), "commit")
        self.assertNotEqual(git(candidate, "rev-parse", "screenwise").stdout.strip(), original_tip)
        self.assertNotEqual(git(candidate, "cat-file", "-e", f"{self.source.local_one}^{{commit}}", check=False).returncode, 0)
        log = git(candidate, "log", "--format=%an%x00%ae%x00%cn%x00%ce", f"{self.source.baseline}..screenwise").stdout
        self.assertNotIn(SOURCE_EMAIL, log)
        self.assertEqual(log.count(NOREPLY_EMAIL), 4)
        refs = git(candidate, "for-each-ref", "--format=%(refname)").stdout.splitlines()
        self.assertEqual(refs, ["refs/heads/screenwise"])
        self.assertEqual(git(candidate, "remote").stdout, "")
        self.assertTrue((candidate / "PUBLICATION_SHA_MAP.tsv").is_file())

    def test_already_noreply_history_is_supported(self) -> None:
        self.source = SyntheticRepository(
            self.root / "noreply-source", all_postbaseline_noreply=True
        )
        result = self.run_script("--execute")
        self.assertIn("commits_already_noreply=2", result.stdout)
        self.assertIn("commits_requiring_email_redaction=0", result.stdout)

    def test_message_containing_source_email_fails_without_disclosure(self) -> None:
        self.source.commit(f"do not publish {SOURCE_EMAIL}", "four.txt", "four\n")
        result = self.run_script(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("message contains the sensitive source email", result.stderr)
        self.assertNotIn(SOURCE_EMAIL, result.stdout + result.stderr)

    def test_unexpected_identity_fails_without_disclosure(self) -> None:
        env = {
            "GIT_AUTHOR_NAME": "Unexpected Author",
            "GIT_AUTHOR_EMAIL": "unexpected@example.invalid",
        }
        self.source.commit("unexpected-author", "four.txt", "four\n", env=env)
        result = self.run_script(check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unexpected author identity", result.stderr)
        self.assertNotIn("unexpected@example.invalid", result.stdout + result.stderr)

    def test_existing_or_outside_destination_is_rejected(self) -> None:
        existing = self.source.root / ".local" / "publication" / "existing.git"
        existing.mkdir(parents=True)
        result = self.run_script("--destination", str(existing), check=False)
        self.assertNotEqual(result.returncode, 0)
        outside = self.root / "outside.git"
        result = self.run_script("--destination", str(outside), check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(outside.exists())

    def test_cleanup_guard_rejects_outside_directory(self) -> None:
        outside = self.root / "outside-owned-looking.git"
        outside.mkdir()
        with self.assertRaises(MODULE.PublicationError):
            MODULE._remove_new_candidate(self.source.root, outside)
        self.assertTrue(outside.is_dir())

    def test_source_ref_change_is_detected(self) -> None:
        original_tip = git(self.source.root, "rev-parse", "screenwise").stdout.strip()
        self.source.commit("later", "later.txt", "later\n")
        with self.assertRaises(MODULE.PublicationError):
            MODULE._assert_source_ref_unchanged(
                self.source.root, "refs/heads/screenwise", original_tip
            )

    def test_signed_or_extra_headers_are_rejected(self) -> None:
        raw = (
            b"tree " + b"0" * 40 + b"\n"
            b"parent " + b"1" * 40 + b"\n"
            b"author Publication Test <private-author@example.invalid> 1 +0000\n"
            b"committer Publication Test <private-author@example.invalid> 1 +0000\n"
            b"gpgsig synthetic\n continuation\n\nmessage\n"
        )
        with self.assertRaises(MODULE.PublicationError):
            MODULE.parse_commit(raw, "2" * 40)


if __name__ == "__main__":
    unittest.main()
