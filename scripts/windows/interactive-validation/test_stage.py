from __future__ import annotations

import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import stage


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest().upper()


class StageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        base = Path(self.temp.name)
        self.package = base / "package"
        self.repo = base / "repo"
        self.package.mkdir()
        self.repo.mkdir()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def write_archive(
        self,
        *,
        archive_path: str = "prep/controller_v1/example.py",
        stage_path: str = "target/interactive-prep-20260915-01a09e45/controller_v1/example.py",
        data: bytes = b"print('safe')\n",
    ) -> tuple[Path, Path]:
        source = self.package.joinpath(*archive_path.split("/"))
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(data)
        manifest = {
            "schema": stage.SCHEMA,
            "provenance": {"kind": "local_validation_source", "upstream_fetch": False},
            "assets": [
                {
                    "archive_path": archive_path,
                    "stage_path": stage_path,
                    "sha256": digest(data),
                    "original_sha256": digest(data),
                }
            ],
        }
        (self.package / stage.MANIFEST_NAME).write_text(json.dumps(manifest), encoding="utf-8")
        return source, self.repo.joinpath(*stage_path.split("/"))

    def test_check_is_read_only(self) -> None:
        _, destination = self.write_archive()
        result = stage.stage_archive(self.package, self.repo)
        self.assertEqual(result["mode"], "check")
        self.assertEqual(result["checked"], 1)
        self.assertFalse(destination.exists())

    def test_common_support_files_have_a_portable_stage_mapping(self) -> None:
        _, destination = self.write_archive(
            archive_path="common/config.example.json",
            stage_path="target/interactive-validation/common/config.example.json",
            data=b"{}\n",
        )
        result = stage.stage_archive(self.package, self.repo, stage=True)
        self.assertEqual(result["copied"], 1)
        self.assertEqual(destination.read_bytes(), b"{}\n")

    def test_stage_uses_exclusive_create_and_same_file_is_unchanged(self) -> None:
        _, destination = self.write_archive()
        result = stage.stage_archive(self.package, self.repo, stage=True)
        self.assertEqual(result["copied"], 1)
        self.assertEqual(destination.read_bytes(), b"print('safe')\n")
        again = stage.stage_archive(self.package, self.repo, stage=True)
        self.assertEqual(again["unchanged"], 1)
        self.assertEqual(again["copied"], 0)

    def test_differing_destination_is_never_overwritten(self) -> None:
        _, destination = self.write_archive()
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(b"local change\n")
        with self.assertRaisesRegex(stage.ArchiveError, "stage_destination_differs"):
            stage.stage_archive(self.package, self.repo, stage=True)
        self.assertEqual(destination.read_bytes(), b"local change\n")
        with self.assertRaises(TypeError):
            stage.stage_archive(self.package, self.repo, stage=True, overwrite=True)  # type: ignore[call-arg]

    def test_removed_overwrite_cli_is_rejected(self) -> None:
        self.write_archive()
        with self.assertRaises(SystemExit):
            stage.main(["--overwrite", "--repo-root", str(self.repo)])

    def test_path_traversal_wrong_mapping_generated_paths_and_ads_are_refused(self) -> None:
        for archive_path in ("prep/../escape.py", "prep/example.py:stream", "prep/C:/example.py"):
            with self.subTest(archive_path=archive_path):
                with self.assertRaises(stage.ArchiveError):
                    stage._relative(archive_path, "archive_path")

        cases = [
            ("prep/example.py", "target/lock-transition-20260916-01a09e45/example.py", "stage_prefix_invalid"),
            ("prep/example.exe", "target/interactive-prep-20260915-01a09e45/example.exe", "archive_extension_invalid"),
            ("prep/sessions/example.py", "target/interactive-prep-20260915-01a09e45/sessions/example.py", "forbidden_path"),
        ]
        for archive_path, stage_path, reason in cases:
            with self.subTest(archive_path=archive_path, stage_path=stage_path):
                for child in list(self.package.iterdir()):
                    if child.is_file():
                        child.unlink()
                    else:
                        import shutil

                        shutil.rmtree(child)
                self.write_archive(archive_path=archive_path, stage_path=stage_path)
                with self.assertRaisesRegex(stage.ArchiveError, reason):
                    stage.verify_archive(self.package)

    def test_non_utf8_and_nul_sources_are_refused(self) -> None:
        for data, reason in ((b"\xff\xfe", "archive_source_not_utf8"), (b"safe\0text", "archive_source_contains_nul")):
            with self.subTest(reason=reason):
                for child in list(self.package.iterdir()):
                    if child.is_file():
                        child.unlink()
                    else:
                        import shutil

                        shutil.rmtree(child)
                self.write_archive(data=data)
                with self.assertRaisesRegex(stage.ArchiveError, reason):
                    stage.verify_archive(self.package)

    def test_reparse_attribute_and_source_symlink_are_refused(self) -> None:
        self.assertTrue(stage._is_reparse(SimpleNamespace(st_file_attributes=0x400)))
        self.assertFalse(stage._is_reparse(SimpleNamespace(st_file_attributes=0)))
        source, _ = self.write_archive()
        real = self.package / "real.py"
        real.write_bytes(source.read_bytes())
        source.unlink()
        try:
            source.symlink_to(real)
        except OSError as exc:
            self.skipTest(f"symlink unavailable: {exc}")
        with self.assertRaisesRegex(stage.ArchiveError, "archive_source_reparse"):
            stage.verify_archive(self.package)

    def test_mocked_root_reparse_is_refused(self) -> None:
        self.write_archive()
        with mock.patch.object(stage, "_is_reparse", return_value=True):
            with self.assertRaisesRegex(stage.ArchiveError, "package_root_reparse"):
                stage.verify_archive(self.package)

    def test_destination_creation_race_does_not_overwrite(self) -> None:
        _, destination = self.write_archive()
        original_mkdir = Path.mkdir

        def racing_mkdir(path: Path, *args, **kwargs):
            result = original_mkdir(path, *args, **kwargs)
            if path == destination.parent and not destination.exists():
                destination.write_bytes(b"racing writer\n")
            return result

        with mock.patch.object(Path, "mkdir", autospec=True, side_effect=racing_mkdir):
            with self.assertRaisesRegex(stage.ArchiveError, "stage_destination_exists"):
                stage.stage_archive(self.package, self.repo, stage=True)
        self.assertEqual(destination.read_bytes(), b"racing writer\n")

    def test_source_mutation_is_rechecked_immediately_before_copy(self) -> None:
        source, destination = self.write_archive()
        original_safe_root = stage._safe_root
        calls = 0

        def mutate_on_second_package(path: Path, label: str):
            nonlocal calls
            result = original_safe_root(path, label)
            if label == "package_root":
                calls += 1
                if calls == 2:
                    source.write_bytes(b"changed\n")
            return result

        with mock.patch.object(stage, "_safe_root", side_effect=mutate_on_second_package):
            with self.assertRaisesRegex(stage.ArchiveError, "archive_hash_mismatch"):
                stage.stage_archive(self.package, self.repo, stage=True)
        self.assertFalse(destination.exists())

    def test_unchanged_destination_is_rechecked_at_execution(self) -> None:
        _, destination = self.write_archive()
        destination.parent.mkdir(parents=True)
        destination.write_bytes(b"print('safe')\n")
        original_sha = stage.sha256_file
        destination_calls = 0

        def mutate_after_planning(path: Path) -> str:
            nonlocal destination_calls
            result = original_sha(path)
            if path == destination:
                destination_calls += 1
                if destination_calls == 1:
                    destination.write_bytes(b"changed after plan\n")
            return result

        with mock.patch.object(stage, "sha256_file", side_effect=mutate_after_planning):
            with self.assertRaisesRegex(stage.ArchiveError, "stage_destination_differs"):
                stage.stage_archive(self.package, self.repo, stage=True)


class CueSourceTests(unittest.TestCase):
    def test_cue_is_fixed_phrase_preview_by_default(self) -> None:
        text = (Path(__file__).parent / "say-status.ps1").read_text(encoding="utf-8")
        self.assertIn("ValidateSet('lock-now', 'unlock-now', 'finished', 'aborted')", text)
        self.assertIn("[switch]$Speak", text)
        self.assertIn("if ($Speak)", text)
        self.assertIn("System.Speech", text)
        self.assertIn("Press Windows L to lock the computer now.", text)
        self.assertNotIn("Invoke-WebRequest", text)
        self.assertNotIn("Start-BitsTransfer", text)
        self.assertNotIn("[string]$Phrase", text)


if __name__ == "__main__":
    unittest.main()
