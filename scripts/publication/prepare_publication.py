#!/usr/bin/env python3
"""Prepare an isolated bare publication candidate from one local branch.

Dry-run is the default.  Execution never changes the source repository and never
uses a remote.  The approved baseline and its complete ancestry keep their exact
object IDs.  Linear commits after the baseline are recreated with the oldest
post-baseline identity's email changed to the approved noreply address. Commits
already using that noreply identity keep it while their parent IDs are rebuilt.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from typing import Iterable, Sequence


DEFAULT_BASELINE = "892199f742e46d0c5d9e8c06687b35ca7c2b6547"
DEFAULT_BRANCH = "screenwise"
SAFE_REF_COMPONENT = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._/-]*$")
IDENTITY_RE = re.compile(rb"^(.*?) <([^<>\r\n]+)> ([0-9]+) ([+-][0-9]{4})$")
OID_RE = re.compile(r"^[0-9a-f]{40}(?:[0-9a-f]{24})?$")


class PublicationError(RuntimeError):
    """A safe, user-facing validation failure."""


@dataclass(frozen=True)
class Identity:
    name: bytes
    email: bytes
    timestamp: bytes
    timezone: bytes


@dataclass(frozen=True)
class SourceCommit:
    oid: str
    tree: str
    parent: str
    author: Identity
    committer: Identity
    message: bytes


@dataclass(frozen=True)
class Rewrite:
    source: SourceCommit
    new_oid: str
    new_parent: str
    content: bytes


def _safe_env(*, include_user_config: bool = False) -> dict[str, str]:
    env = os.environ.copy()
    for key in (
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_COUNT",
    ):
        env.pop(key, None)
    for key in tuple(env):
        if key.startswith("GIT_CONFIG_KEY_") or key.startswith("GIT_CONFIG_VALUE_"):
            env.pop(key, None)
    env["GIT_NO_REPLACE_OBJECTS"] = "1"
    env["GIT_TERMINAL_PROMPT"] = "0"
    if not include_user_config:
        env["GIT_CONFIG_NOSYSTEM"] = "1"
        env["GIT_CONFIG_GLOBAL"] = os.devnull
    return env


def _git(
    repo: Path,
    args: Sequence[str],
    *,
    input_bytes: bytes | None = None,
    check: bool = True,
    include_user_config: bool = False,
) -> subprocess.CompletedProcess[bytes]:
    command = ["git", "-c", "core.hooksPath=", "-C", str(repo), *args]
    result = subprocess.run(
        command,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=_safe_env(include_user_config=include_user_config),
        check=False,
    )
    if check and result.returncode != 0:
        label = " ".join(args[:2])
        raise PublicationError(f"Git command failed safely: {label}")
    return result


def _parse_identity(value: bytes, commit_oid: str, field: str) -> Identity:
    match = IDENTITY_RE.match(value)
    if not match:
        raise PublicationError(f"Commit {commit_oid} has an unsupported {field} header")
    return Identity(match.group(1), match.group(2), match.group(3), match.group(4))


def parse_commit(raw: bytes, oid: str) -> SourceCommit:
    try:
        header, message = raw.split(b"\n\n", 1)
    except ValueError as exc:
        raise PublicationError(f"Commit {oid} has no header/message boundary") from exc

    fields: dict[bytes, list[bytes]] = {}
    for line in header.split(b"\n"):
        if line.startswith(b" "):
            raise PublicationError(f"Commit {oid} contains a signed or continued header")
        key, separator, value = line.partition(b" ")
        if not separator:
            raise PublicationError(f"Commit {oid} has a malformed header")
        if key in {b"gpgsig", b"gpgsig-sha256", b"mergetag"}:
            raise PublicationError(f"Commit {oid} is signed or contains a merge signature")
        if key not in {b"tree", b"parent", b"author", b"committer"}:
            raise PublicationError(f"Commit {oid} contains unsupported metadata: {key.decode('ascii', 'replace')}")
        fields.setdefault(key, []).append(value)

    if any(len(fields.get(key, [])) != 1 for key in (b"tree", b"parent", b"author", b"committer")):
        raise PublicationError(f"Commit {oid} is not a supported single-parent commit")
    tree = fields[b"tree"][0].decode("ascii", "strict")
    parent = fields[b"parent"][0].decode("ascii", "strict")
    if not OID_RE.fullmatch(tree) or not OID_RE.fullmatch(parent):
        raise PublicationError(f"Commit {oid} contains an invalid object ID")
    return SourceCommit(
        oid=oid,
        tree=tree,
        parent=parent,
        author=_parse_identity(fields[b"author"][0], oid, "author"),
        committer=_parse_identity(fields[b"committer"][0], oid, "committer"),
        message=message,
    )


def _format_identity(identity: Identity, email: bytes) -> bytes:
    return (
        identity.name
        + b" <"
        + email
        + b"> "
        + identity.timestamp
        + b" "
        + identity.timezone
    )


def _commit_content(commit: SourceCommit, parent: str, noreply: bytes) -> bytes:
    return b"\n".join(
        (
            b"tree " + commit.tree.encode("ascii"),
            b"parent " + parent.encode("ascii"),
            b"author " + _format_identity(commit.author, noreply),
            b"committer " + _format_identity(commit.committer, noreply),
            b"",
        )
    ) + b"\n" + commit.message


def _object_oid(content: bytes, object_type: str, object_format: str) -> str:
    try:
        digest = hashlib.new(object_format)
    except ValueError as exc:
        raise PublicationError(f"Unsupported Git object format: {object_format}") from exc
    digest.update(f"{object_type} {len(content)}\0".encode("ascii"))
    digest.update(content)
    return digest.hexdigest()


def _strict_ref(branch: str) -> str:
    if not SAFE_REF_COMPONENT.fullmatch(branch) or ".." in branch or "//" in branch:
        raise PublicationError("Branch name is not safe for publication preparation")
    ref = f"refs/heads/{branch}"
    check = subprocess.run(
        ["git", "check-ref-format", "--branch", branch],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=_safe_env(),
        check=False,
    )
    if check.returncode != 0:
        raise PublicationError("Branch name is not a valid local branch")
    return ref


def _is_reparse(path: Path) -> bool:
    if path.is_symlink():
        return True
    try:
        info = path.lstat()
    except FileNotFoundError:
        return False
    attributes = getattr(info, "st_file_attributes", 0)
    return bool(attributes & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0))


def _validate_candidate_boundary(source: Path, destination: Path, *, require_absent: bool) -> None:
    publication_root = Path(os.path.abspath(source / ".local" / "publication"))
    destination = Path(os.path.abspath(destination))
    try:
        inside = os.path.commonpath((str(destination), str(publication_root))) == str(publication_root)
    except ValueError:
        inside = False
    if not inside or destination == publication_root:
        raise PublicationError("Candidate path is outside the guarded publication directory")
    cursor = source
    for part in destination.relative_to(source).parts:
        cursor = cursor / part
        if cursor.exists() and _is_reparse(cursor):
            raise PublicationError("Candidate path crosses a symlink or reparse point")
    if require_absent and (destination.exists() or destination.is_symlink()):
        raise PublicationError("Destination already exists; refusing to overwrite it")


def _remove_new_candidate(source: Path, path: Path) -> None:
    """Remove only the newly created guarded candidate, including read-only Git objects."""

    _validate_candidate_boundary(source, path, require_absent=False)
    if not path.is_dir() or _is_reparse(path):
        raise PublicationError("Refusing cleanup of an unowned or redirected candidate path")

    def make_writable(_function: object, target: str, _error: object) -> None:
        os.chmod(target, stat.S_IWRITE)
        if os.path.isdir(target):
            os.rmdir(target)
        else:
            os.unlink(target)

    shutil.rmtree(path, onerror=make_writable)


def _guard_destination(source: Path, destination_arg: str | None) -> tuple[Path, str]:
    publication_root = source / ".local" / "publication"
    destination = (
        Path(destination_arg).expanduser()
        if destination_arg
        else publication_root / "screenwise-public.git"
    )
    if not destination.is_absolute():
        destination = source / destination
    destination = Path(os.path.abspath(destination))
    root_abs = Path(os.path.abspath(publication_root))
    _validate_candidate_boundary(source, destination, require_absent=True)
    display = destination.relative_to(source).as_posix()
    return destination, display


def _read_source_plan(
    source: Path,
    branch: str,
    baseline_arg: str,
    noreply_text: str,
) -> tuple[str, str, str, list[Rewrite], bool, str, bytes]:
    ref = _strict_ref(branch)
    if _git(source, ["rev-parse", "--is-bare-repository"]).stdout.strip() == b"true":
        dirty = False
    else:
        dirty = bool(_git(source, ["status", "--porcelain=v1", "--untracked-files=normal"]).stdout)
    if _git(source, ["rev-parse", "--is-shallow-repository"]).stdout.strip() == b"true":
        raise PublicationError("A shallow source cannot preserve the complete baseline ancestry")

    baseline = _git(source, ["rev-parse", "--verify", f"{baseline_arg}^{{commit}}"]).stdout.decode().strip()
    tip_result = _git(source, ["show-ref", "--verify", "--hash", ref], check=False)
    if tip_result.returncode != 0:
        raise PublicationError("The requested source branch is not a local branch")
    tip = tip_result.stdout.decode().strip()
    ancestor = _git(source, ["merge-base", "--is-ancestor", baseline, tip], check=False)
    if ancestor.returncode != 0:
        raise PublicationError("The approved baseline is not an ancestor of the source branch")

    object_format = _git(source, ["rev-parse", "--show-object-format"]).stdout.decode().strip()
    noreply = noreply_text.encode("ascii", "strict")
    if b"\n" in noreply or b"\r" in noreply or b"@" not in noreply:
        raise PublicationError("Approved noreply email is invalid")

    oid_lines = _git(source, ["rev-list", "--reverse", f"{baseline}..{ref}"]).stdout.splitlines()
    old_oids = [line.decode("ascii") for line in oid_lines if line]
    if not old_oids:
        raise PublicationError("There are no post-baseline commits to rewrite")
    if int(_git(source, ["rev-list", "--count", f"{baseline}..{ref}"]).stdout) != len(old_oids):
        raise PublicationError("The post-baseline range is not linear")

    commits: list[SourceCommit] = []
    expected_parent = baseline
    for oid in old_oids:
        raw = _git(source, ["cat-file", "commit", oid]).stdout
        commit = parse_commit(raw, oid)
        if commit.parent != expected_parent:
            raise PublicationError(f"Commit {oid} breaks the required linear parent sequence")
        commits.append(commit)
        expected_parent = oid

    original_name = commits[0].author.name
    original_email = commits[0].author.email
    if (
        commits[0].committer.name != original_name
        or commits[0].committer.email.lower() != original_email.lower()
    ):
        raise PublicationError("The oldest post-baseline commit has mismatched author/committer identities")
    allowed_emails = {original_email.lower(), noreply.lower()}

    rewrites: list[Rewrite] = []
    for commit in commits:
        for identity, field in ((commit.author, "author"), (commit.committer, "committer")):
            if identity.name != original_name or identity.email.lower() not in allowed_emails:
                raise PublicationError(f"Commit {commit.oid} has an unexpected {field} identity")
        if original_email.lower() != noreply.lower() and original_email.lower() in commit.message.lower():
            raise PublicationError(f"Commit {commit.oid} message contains the sensitive source email")
        new_parent = baseline if not rewrites else rewrites[-1].new_oid
        content = _commit_content(commit, new_parent, noreply)
        new_oid = _object_oid(content, "commit", object_format)
        rewrites.append(Rewrite(commit, new_oid, new_parent, content))

    return ref, baseline, tip, rewrites, dirty, object_format, original_email


def _object_ids(source: Path, revisions: Sequence[str]) -> list[str]:
    lines = _git(source, ["rev-list", "--objects", *revisions]).stdout.splitlines()
    result: list[str] = []
    seen: set[str] = set()
    for line in lines:
        oid = line.split(b" ", 1)[0].decode("ascii")
        if oid not in seen:
            seen.add(oid)
            result.append(oid)
    return result


def _object_types(source: Path, object_ids: Sequence[str]) -> dict[str, str]:
    payload = ("\n".join(object_ids) + "\n").encode("ascii")
    lines = _git(source, ["cat-file", "--batch-check=%(objectname) %(objecttype)"], input_bytes=payload).stdout.splitlines()
    result: dict[str, str] = {}
    for line in lines:
        oid_bytes, separator, kind_bytes = line.partition(b" ")
        if not separator:
            raise PublicationError("Git returned malformed object metadata")
        result[oid_bytes.decode("ascii")] = kind_bytes.decode("ascii")
    return result


def _copy_exact_objects(source: Path, destination: Path, object_ids: Iterable[str]) -> None:
    ids = list(object_ids)
    if not ids:
        return
    safe_env = _safe_env()
    with tempfile.TemporaryFile() as pack_error, tempfile.TemporaryFile() as index_error:
        pack = subprocess.Popen(
            ["git", "-c", "core.hooksPath=", "-C", str(source), "pack-objects", "--stdout"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=pack_error,
            env=safe_env,
        )
        assert pack.stdin is not None and pack.stdout is not None
        index = subprocess.Popen(
            ["git", "-c", "core.hooksPath=", "-C", str(destination), "index-pack", "--stdin", "--fix-thin"],
            stdin=pack.stdout,
            stdout=subprocess.PIPE,
            stderr=index_error,
            env=safe_env,
        )
        pack.stdout.close()
        try:
            for oid in ids:
                pack.stdin.write(oid.encode("ascii") + b"\n")
            pack.stdin.close()
        except BrokenPipeError as exc:
            pack.kill()
            index.kill()
            raise PublicationError("Git object transfer stopped unexpectedly") from exc
        index_output = index.communicate()[0]
        pack_status = pack.wait()
        if pack_status != 0 or index.returncode != 0 or not index_output.strip():
            raise PublicationError("Local Git object transfer failed")


def _reachable_digest(repo: Path, commit: str) -> tuple[int, str]:
    ids = sorted(_object_ids(repo, [commit]))
    digest = hashlib.sha256(("\n".join(ids) + "\n").encode("ascii")).hexdigest()
    return len(ids), digest


def _initialize_candidate(destination: Path, branch: str, object_format: str) -> None:
    result = subprocess.run(
        [
            "git",
            "-c",
            "core.hooksPath=",
            "init",
            "--bare",
            f"--object-format={object_format}",
            f"--initial-branch={branch}",
            str(destination),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=_safe_env(),
        check=False,
    )
    if result.returncode != 0:
        raise PublicationError("Could not initialize the isolated bare candidate")
    hooks = destination / "hooks"
    if hooks.exists():
        shutil.rmtree(hooks)
    hooks.mkdir()
    _git(destination, ["config", "--local", "core.hooksPath", "hooks"])
    _git(destination, ["symbolic-ref", "HEAD", f"refs/heads/{branch}"])


def _write_rewritten_commits(destination: Path, rewrites: Sequence[Rewrite]) -> None:
    for rewrite in rewrites:
        actual = _git(destination, ["hash-object", "-w", "-t", "commit", "--stdin"], input_bytes=rewrite.content).stdout.decode().strip()
        if actual != rewrite.new_oid:
            raise PublicationError(f"Rewritten commit hash mismatch for {rewrite.source.oid}")


def _verify_candidate(
    source: Path,
    destination: Path,
    ref: str,
    baseline: str,
    rewrites: Sequence[Rewrite],
    noreply: bytes,
    sensitive_email: bytes,
) -> None:
    refs = _git(destination, ["for-each-ref", "--format=%(refname)"]).stdout.splitlines()
    if refs != [ref.encode("ascii")]:
        raise PublicationError("Candidate contains unexpected refs")
    if _git(destination, ["symbolic-ref", "HEAD"]).stdout.strip() != ref.encode("ascii"):
        raise PublicationError("Candidate HEAD does not select the publication branch")
    if _git(destination, ["remote"]).stdout.strip():
        raise PublicationError("Candidate unexpectedly contains a remote")
    if any((destination / "hooks").iterdir()):
        raise PublicationError("Candidate hook directory is not empty")
    for relative in ("objects/info/alternates", "shallow", "info/grafts"):
        path = destination / relative
        if path.exists() and path.stat().st_size:
            raise PublicationError(f"Candidate unexpectedly depends on {relative}")

    source_baseline = _reachable_digest(source, baseline)
    candidate_baseline = _reachable_digest(destination, baseline)
    if source_baseline != candidate_baseline:
        raise PublicationError("Candidate does not preserve the exact baseline object closure")

    expected_parent = baseline
    for rewrite in rewrites:
        raw = _git(destination, ["cat-file", "commit", rewrite.new_oid]).stdout
        if sensitive_email.lower() != noreply.lower() and sensitive_email.lower() in raw.lower():
            raise PublicationError(f"Candidate commit still contains the sensitive source email: {rewrite.new_oid}")
        if raw != rewrite.content or rewrite.new_parent != expected_parent:
            raise PublicationError(f"Candidate commit verification failed for {rewrite.source.oid}")
        parsed = parse_commit(raw, rewrite.new_oid)
        if parsed.tree != rewrite.source.tree or parsed.parent != expected_parent or parsed.message != rewrite.source.message:
            raise PublicationError(f"Candidate changed tree, parent or message for {rewrite.source.oid}")
        if parsed.author.name != rewrite.source.author.name or parsed.committer.name != rewrite.source.committer.name:
            raise PublicationError(f"Candidate changed a name for {rewrite.source.oid}")
        if parsed.author.timestamp != rewrite.source.author.timestamp or parsed.author.timezone != rewrite.source.author.timezone:
            raise PublicationError(f"Candidate changed an author timestamp for {rewrite.source.oid}")
        if parsed.committer.timestamp != rewrite.source.committer.timestamp or parsed.committer.timezone != rewrite.source.committer.timezone:
            raise PublicationError(f"Candidate changed a committer timestamp for {rewrite.source.oid}")
        if parsed.author.email != noreply or parsed.committer.email != noreply:
            raise PublicationError(f"Candidate email rewrite failed for {rewrite.source.oid}")
        expected_parent = rewrite.new_oid

    reachable = _git(destination, ["rev-list", f"{baseline}..{ref}"]).stdout.splitlines()
    if len(reachable) != len(rewrites) or reachable[0].decode() != rewrites[-1].new_oid:
        raise PublicationError("Candidate post-baseline commit count or tip is wrong")
    for rewrite in rewrites:
        source_had_sensitive_email = (
            rewrite.source.author.email.lower() != noreply.lower()
            or rewrite.source.committer.email.lower() != noreply.lower()
        )
        if (
            source_had_sensitive_email
            and _git(destination, ["cat-file", "-e", f"{rewrite.source.oid}^{{commit}}"], check=False).returncode == 0
        ):
            raise PublicationError("Candidate contains an original post-baseline commit object")
    _git(destination, ["fsck", "--full", "--no-reflogs"])


def _assert_source_ref_unchanged(source: Path, ref: str, original_tip: str) -> None:
    result = _git(source, ["show-ref", "--verify", "--hash", ref], check=False)
    if result.returncode != 0 or result.stdout.decode().strip() != original_tip:
        raise PublicationError("Source branch changed during candidate preparation; candidate discarded")


def _write_report(
    destination: Path,
    branch: str,
    baseline: str,
    original_tip: str,
    rewrites: Sequence[Rewrite],
    dirty: bool,
    noreply: str,
) -> None:
    noreply_bytes = noreply.encode("ascii")
    already_noreply = sum(
        item.source.author.email.lower() == noreply_bytes.lower()
        and item.source.committer.email.lower() == noreply_bytes.lower()
        for item in rewrites
    )
    report = "\n".join(
        (
            "ScreenWise isolated publication candidate",
            f"branch: {branch}",
            f"baseline: {baseline}",
            f"original_tip: {original_tip}",
            f"candidate_tip: {rewrites[-1].new_oid}",
            f"rewritten_commits: {len(rewrites)}",
            f"commits_already_noreply: {already_noreply}",
            f"commits_requiring_email_redaction: {len(rewrites) - already_noreply}",
            f"approved_noreply: {noreply}",
            f"source_worktree_dirty: {'yes; uncommitted changes excluded' if dirty else 'no'}",
            "source_repository_changed: no",
            "network_used: no",
            "refs_imported: one local source branch only",
            "",
        )
    )
    mapping = "source_commit\tcandidate_commit\n" + "".join(
        f"{item.source.oid}\t{item.new_oid}\n" for item in rewrites
    )
    (destination / "PUBLICATION_REPORT.txt").write_text(report, encoding="utf-8", newline="\n")
    (destination / "PUBLICATION_SHA_MAP.tsv").write_text(mapping, encoding="utf-8", newline="\n")


def _print_plan(
    mode: str,
    destination_display: str,
    branch: str,
    baseline: str,
    original_tip: str,
    rewrites: Sequence[Rewrite],
    dirty: bool,
    noreply: str,
) -> None:
    noreply_bytes = noreply.encode("ascii")
    already_noreply = sum(
        item.source.author.email.lower() == noreply_bytes.lower()
        and item.source.committer.email.lower() == noreply_bytes.lower()
        for item in rewrites
    )
    print(f"mode={mode}")
    print(f"destination={destination_display}")
    print(f"source_branch={branch}")
    print(f"baseline={baseline}")
    print(f"original_tip={original_tip}")
    print(f"candidate_tip={rewrites[-1].new_oid}")
    print(f"rewritten_commits={len(rewrites)}")
    print(f"commits_already_noreply={already_noreply}")
    print(f"commits_requiring_email_redaction={len(rewrites) - already_noreply}")
    print(f"approved_noreply={noreply}")
    print(f"source_worktree_dirty={'yes (uncommitted changes excluded)' if dirty else 'no'}")
    print("sha_map_begin")
    for rewrite in rewrites:
        print(f"{rewrite.source.oid}\t{rewrite.new_oid}")
    print("sha_map_end")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", default=".", help="source worktree (default: current directory)")
    parser.add_argument("--branch", default=DEFAULT_BRANCH, help="single local source branch")
    parser.add_argument("--baseline", default=DEFAULT_BASELINE, help="approved baseline commit")
    parser.add_argument("--destination", help="new path below SOURCE/.local/publication")
    parser.add_argument("--noreply-email", required=True, help="approved replacement email")
    parser.add_argument("--execute", action="store_true", help="create and verify the bare candidate")
    parser.add_argument("--gc", action="store_true", help="prune unreachable objects in the verified candidate")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.gc and not args.execute:
        print("error: --gc requires --execute", file=sys.stderr)
        return 2
    try:
        source = Path(args.source).expanduser().resolve(strict=True)
        if _git(source, ["rev-parse", "--show-toplevel"], check=False).returncode != 0:
            raise PublicationError("Source is not a Git repository")
        destination, destination_display = _guard_destination(source, args.destination)
        ignored = _git(
            source,
            ["check-ignore", "--quiet", "--no-index", "--", str(destination)],
            check=False,
        )
        if ignored.returncode != 0:
            raise PublicationError("Destination is not covered by the source repository's ignore rules")
        ref, baseline, original_tip, rewrites, dirty, object_format, sensitive_email = _read_source_plan(
            source, args.branch, args.baseline, args.noreply_email
        )
        _print_plan(
            "execute" if args.execute else "dry-run",
            destination_display,
            args.branch,
            baseline,
            original_tip,
            rewrites,
            dirty,
            args.noreply_email,
        )
        if not args.execute:
            print("result=no files or refs changed")
            return 0

        owned_candidate = False
        try:
            destination.parent.mkdir(parents=True, exist_ok=True)
            _validate_candidate_boundary(source, destination, require_absent=True)
            destination.mkdir()
            owned_candidate = True
            _initialize_candidate(destination, args.branch, object_format)
            baseline_ids = _object_ids(source, [baseline])
            _copy_exact_objects(source, destination, baseline_ids)
            _git(destination, ["update-ref", ref, baseline])

            post_ids = _object_ids(source, [original_tip, f"^{baseline}"])
            types = _object_types(source, post_ids)
            original_commits = {rewrite.source.oid for rewrite in rewrites}
            found_commits = {oid for oid, kind in types.items() if kind == "commit"}
            if found_commits != original_commits:
                raise PublicationError("Post-baseline object inventory does not match the validated commits")
            reusable = [oid for oid in post_ids if types.get(oid) in {"tree", "blob"}]
            unexpected = {kind for kind in types.values() if kind not in {"commit", "tree", "blob"}}
            if unexpected:
                raise PublicationError("Post-baseline range contains unsupported Git object types")
            _copy_exact_objects(source, destination, reusable)
            _write_rewritten_commits(destination, rewrites)
            _git(destination, ["update-ref", ref, rewrites[-1].new_oid, baseline])
            _verify_candidate(
                source,
                destination,
                ref,
                baseline,
                rewrites,
                args.noreply_email.encode("ascii"),
                sensitive_email,
            )
            _assert_source_ref_unchanged(source, ref, original_tip)
            if args.gc:
                _git(destination, ["gc", "--prune=now"])
                _verify_candidate(
                    source,
                    destination,
                    ref,
                    baseline,
                    rewrites,
                    args.noreply_email.encode("ascii"),
                    sensitive_email,
                )
                _assert_source_ref_unchanged(source, ref, original_tip)
            _write_report(
                destination,
                args.branch,
                baseline,
                original_tip,
                rewrites,
                dirty,
                args.noreply_email,
            )
        except Exception:
            if owned_candidate and destination.exists():
                _remove_new_candidate(source, destination)
            raise
        print("result=verified isolated candidate created; source unchanged")
        return 0
    except (PublicationError, OSError, UnicodeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
