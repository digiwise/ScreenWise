"""Verify or stage the curated Windows interactive-validation source snapshot.

The default operation is read-only. Staging copies only manifest-allowlisted text
sources to their original target paths. It never executes a staged file.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
from pathlib import Path, PurePosixPath, PureWindowsPath
from typing import Any


SCHEMA = "screenwise.interactive-validation-source-archive.v2"
MANIFEST_NAME = "source-manifest.json"
ALLOWED_EXTENSIONS = {".cs", ".html", ".json", ".md", ".ps1", ".py"}
STAGE_PREFIXES = {
    "prep": PurePosixPath("target/interactive-prep-20260915-01a09e45"),
    "tail": PurePosixPath("target/background-meeting-20260916-01a09e45"),
    "lock": PurePosixPath("target/lock-transition-20260916-01a09e45"),
    "common": PurePosixPath("target/interactive-validation/common"),
}
FORBIDDEN_PARTS = {
    "__pycache__",
    "data",
    "evidence",
    "logs",
    "media",
    "models",
    "runs",
    "sessions",
    "speech-retry",
}
INVALID_WINDOWS_CHARS = set('<>:"|?*')
RESERVED_WINDOWS_NAMES = {
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5",
    "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4",
    "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
}


class ArchiveError(RuntimeError):
    """The archive or requested staging destination is unsafe or inconsistent."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def _relative(value: Any, label: str) -> PurePosixPath:
    if not isinstance(value, str) or not value or "\\" in value or any(
        character in INVALID_WINDOWS_CHARS for character in value
    ):
        raise ArchiveError(f"{label}_invalid")
    path = PurePosixPath(value)
    windows_path = PureWindowsPath(value)
    if (
        path.is_absolute()
        or windows_path.is_absolute()
        or windows_path.drive
        or windows_path.root
        or any(part in {"", ".", ".."} or part.endswith((" ", ".")) for part in path.parts)
        or any(part.split(".", 1)[0].upper() in RESERVED_WINDOWS_NAMES for part in path.parts)
    ):
        raise ArchiveError(f"{label}_invalid")
    return path


def _inside(path: Path, root: Path, label: str) -> None:
    try:
        path.relative_to(root)
    except ValueError as exc:
        raise ArchiveError(f"{label}_outside_root") from exc


def _absolute(path: Path) -> Path:
    return Path(os.path.abspath(os.fspath(path)))


def _lstat(path: Path):
    try:
        return path.lstat()
    except FileNotFoundError:
        return None


def _is_reparse(info: os.stat_result) -> bool:
    attributes = getattr(info, "st_file_attributes", 0)
    return bool(attributes & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400))


def _safe_root(path: Path, label: str) -> tuple[Path, Path]:
    raw = _absolute(path)
    chain = [raw]
    chain.extend(raw.parents)
    for node in reversed(chain):
        info = _lstat(node)
        if info is not None and (stat.S_ISLNK(info.st_mode) or _is_reparse(info)):
            raise ArchiveError(f"{label}_reparse")
    try:
        resolved = raw.resolve(strict=True)
    except OSError as exc:
        raise ArchiveError(f"{label}_missing") from exc
    if not resolved.is_dir():
        raise ArchiveError(f"{label}_not_directory")
    return raw, resolved


def _reject_reparse_chain(
    path: Path, raw_root: Path, resolved_root: Path, label: str
) -> None:
    path = _absolute(path)
    _inside(path, raw_root, label)
    current = raw_root
    for part in path.relative_to(raw_root).parts:
        current = current / part
        info = _lstat(current)
        if info is not None and (stat.S_ISLNK(info.st_mode) or _is_reparse(info)):
            raise ArchiveError(f"{label}_reparse")
        try:
            resolved = current.resolve(strict=False)
        except OSError as exc:
            raise ArchiveError(f"{label}_unresolvable") from exc
        _inside(resolved, resolved_root, label)


def _load_manifest(package_root: Path, resolved_root: Path) -> dict[str, Any]:
    manifest_path = package_root / MANIFEST_NAME
    _reject_reparse_chain(manifest_path, package_root, resolved_root, "manifest")
    try:
        payload = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise ArchiveError("manifest_unreadable") from exc
    if payload.get("schema") != SCHEMA or not isinstance(payload.get("assets"), list):
        raise ArchiveError("manifest_schema_invalid")
    return payload


def verify_archive(package_root: Path) -> list[dict[str, Any]]:
    package_root, resolved_root = _safe_root(package_root, "package_root")
    payload = _load_manifest(package_root, resolved_root)
    checked: list[dict[str, Any]] = []
    seen_archive: set[str] = set()
    seen_stage: set[str] = set()

    for item in payload["assets"]:
        if not isinstance(item, dict):
            raise ArchiveError("manifest_asset_invalid")
        archive_rel = _relative(item.get("archive_path"), "archive_path")
        stage_rel = _relative(item.get("stage_path"), "stage_path")
        archive_text = archive_rel.as_posix()
        stage_text = stage_rel.as_posix()
        if archive_text in seen_archive or stage_text in seen_stage:
            raise ArchiveError("manifest_duplicate_path")
        seen_archive.add(archive_text)
        seen_stage.add(stage_text)

        group = archive_rel.parts[0] if archive_rel.parts else ""
        expected_prefix = STAGE_PREFIXES.get(group)
        if expected_prefix is None:
            raise ArchiveError("archive_group_invalid")
        if PurePosixPath(*stage_rel.parts[: len(expected_prefix.parts)]) != expected_prefix:
            raise ArchiveError("stage_prefix_invalid")
        expected_tail = PurePosixPath(*stage_rel.parts[len(expected_prefix.parts) :])
        archive_tail = PurePosixPath(*archive_rel.parts[1:])
        if expected_tail != archive_tail:
            raise ArchiveError("stage_mapping_invalid")
        if archive_rel.suffix.lower() not in ALLOWED_EXTENSIONS:
            raise ArchiveError("archive_extension_invalid")
        if any(part.casefold() in FORBIDDEN_PARTS for part in archive_rel.parts + stage_rel.parts):
            raise ArchiveError("forbidden_path")

        expected_hash = item.get("sha256")
        original_hash = item.get("original_sha256")
        if not isinstance(expected_hash, str) or len(expected_hash) != 64:
            raise ArchiveError("archive_hash_invalid")
        if not isinstance(original_hash, str) or len(original_hash) != 64:
            raise ArchiveError("original_hash_invalid")

        source = package_root.joinpath(*archive_rel.parts)
        _reject_reparse_chain(source, package_root, resolved_root, "archive_source")
        try:
            info = source.lstat()
        except OSError as exc:
            raise ArchiveError("archive_source_missing") from exc
        if not stat.S_ISREG(info.st_mode) or info.st_size <= 0:
            raise ArchiveError("archive_source_not_regular")
        try:
            data = source.read_bytes()
            data.decode("utf-8-sig", errors="strict")
        except (OSError, UnicodeError) as exc:
            raise ArchiveError("archive_source_not_utf8") from exc
        if b"\0" in data:
            raise ArchiveError("archive_source_contains_nul")
        if hashlib.sha256(data).hexdigest().upper() != expected_hash.upper():
            raise ArchiveError("archive_hash_mismatch")
        checked.append({"archive": source, "stage_rel": stage_rel, "sha256": expected_hash.upper()})

    if not checked:
        raise ArchiveError("manifest_empty")
    return checked


def stage_archive(
    package_root: Path,
    repo_root: Path,
    *,
    stage: bool = False,
) -> dict[str, Any]:
    assets = verify_archive(package_root)
    package_root, package_resolved = _safe_root(package_root, "package_root")
    repo_root, repo_resolved = _safe_root(repo_root, "repo_root")
    plans: list[tuple[Path, Path, str, str]] = []

    for item in assets:
        destination = repo_root.joinpath(*item["stage_rel"].parts)
        _reject_reparse_chain(destination, repo_root, repo_resolved, "stage_destination")
        if destination.exists():
            if not destination.is_file():
                raise ArchiveError("stage_destination_not_regular")
            current_hash = sha256_file(destination)
            if current_hash == item["sha256"]:
                action = "unchanged"
            else:
                raise ArchiveError("stage_destination_differs")
        else:
            action = "copy"
        plans.append((item["archive"], destination, action, item["sha256"]))

    counts = {"checked": len(plans), "copied": 0, "unchanged": 0}
    if stage:
        for source, destination, action, expected_hash in plans:
            _reject_reparse_chain(source, package_root, package_resolved, "archive_source")
            try:
                source_info = source.lstat()
                if not stat.S_ISREG(source_info.st_mode) or source_info.st_size <= 0:
                    raise ArchiveError("archive_source_not_regular")
                source_data = source.read_bytes()
                source_data.decode("utf-8-sig", errors="strict")
            except (OSError, UnicodeError) as exc:
                raise ArchiveError("archive_source_not_utf8") from exc
            if b"\0" in source_data:
                raise ArchiveError("archive_source_contains_nul")
            if hashlib.sha256(source_data).hexdigest().upper() != expected_hash:
                raise ArchiveError("archive_hash_mismatch")
            if action == "unchanged":
                _reject_reparse_chain(destination, repo_root, repo_resolved, "stage_destination")
                if not destination.is_file() or sha256_file(destination) != expected_hash:
                    raise ArchiveError("stage_destination_differs")
                counts["unchanged"] += 1
                continue
            destination.parent.mkdir(parents=True, exist_ok=True)
            _reject_reparse_chain(destination, repo_root, repo_resolved, "stage_destination")
            try:
                with destination.open("xb") as output:
                    output.write(source_data)
            except FileExistsError as exc:
                raise ArchiveError("stage_destination_exists") from exc
            if sha256_file(destination) != expected_hash:
                raise ArchiveError("stage_copy_hash_mismatch")
            counts["copied"] += 1

    return {
        "schema": "screenwise.interactive-validation-stage-result.v1",
        "mode": "stage" if stage else "check",
        **counts,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="verify the archive (default)")
    mode.add_argument("--stage", action="store_true", help="copy allowlisted sources")
    parser.add_argument("--repo-root", type=Path, help="screenpipe repository root")
    args = parser.parse_args(argv)

    package_root = Path(__file__).resolve().parent
    repo_root = args.repo_root or package_root.parents[2]
    try:
        if args.stage:
            result = stage_archive(package_root, repo_root, stage=True)
        else:
            assets = verify_archive(package_root)
            result = {
                "schema": "screenwise.interactive-validation-stage-result.v1",
                "mode": "check", "checked": len(assets), "copied": 0,
                "unchanged": 0,
            }
    except ArchiveError as exc:
        print(json.dumps({"schema": "screenwise.interactive-validation-stage-result.v1", "status": "error", "reason": str(exc)}, sort_keys=True))
        return 2
    result["status"] = "ok"
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
