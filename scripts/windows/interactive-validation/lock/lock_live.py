"""Bounded, silent Windows lock-transition controller.

Importing this module is inert.  The CLI is preview-only unless both the
explicit execution switch and a freshly consumed owner-readiness nonce are
provided.  Evidence is aggregate-only; captured rows and media are never read
back into the result.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import sqlite3
import sys
import threading
import time
from contextlib import closing
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable
from urllib.parse import urlencode

HERE = Path(__file__).resolve().parent
TARGET = HERE.parent
PREP = TARGET / "interactive-prep-20260915-01a09e45"
CONTROLLER = PREP / "controller_v1"
TAIL_PREP = TARGET / "background-meeting-20260916-01a09e45" / "preparation"
if str(CONTROLLER) not in sys.path:
    sys.path.insert(0, str(CONTROLLER))
if str(TAIL_PREP) not in sys.path:
    sys.path.insert(0, str(TAIL_PREP))

import coordinator  # noqa: E402
import live  # noqa: E402
import tail_collector_60s as tail  # noqa: E402

LOCK_ACCEPTANCE = HERE / "lock_acceptance.py"
NEW_PINS = HERE / "pins.json"
OLD_PINS = CONTROLLER / "pins.json"
TAIL_PINS = TAIL_PREP.parent / "collector-pins.json"
PORT = live.API_PORT
WATCHDOG_SECONDS = 180.0
POLL_SECONDS = 0.25
SETTLE_SECONDS = 2.0
PLATEAU_SECONDS = 12.0
CONTROL_WAIT_SECONDS = 30.0
TRANSITION_UNKNOWN_CODE = "secure_desktop_unavailable"
TRANSITION_UNKNOWN_MAX_SECONDS = 2.0
TRANSITION_UNKNOWN_MAX_SAMPLES = 16
EVIDENCE_CHECKPOINT_SCHEMA = "screenwise.lock-transition-evidence-checkpoint.v1"
EVIDENCE_CHECKPOINT_STAGES = frozenset({
    "ready_for_lock", "locked_observation_complete",
    "unlocked_waiting_fixture_focus", "cleanup_complete", "evaluation_failed",
})
CONTROL_ERROR_CODES = frozenset({
    "active_recording_deadline", "audio_status_schema_invalid",
    "auth_token_unavailable", "capture_delivery_degraded",
    "capture_notice_timestamp_invalid", "capture_shutdown_status_degraded",
    "capture_status_degraded", "capture_status_schema_invalid",
    "capture_status_unavailable", "control_focus_or_session_changed",
    "fixture_focus_timeout", "fixture_generation_unverified", "fixture_hide_failed",
    "initial_session_not_unlocked", "lock_not_stable_after_settle",
    "lock_not_stable_at_plateau_end", "lock_not_stable_during_settle",
    "lock_plateau_interrupted", "owned_database_missing",
    "owned_database_schema_invalid", "positive_control_timeout",
    "post_unlock_plain_failed", "pre_lock_state_changed",
    "prepared_plan_unavailable", "recorder_exited",
    "recorder_exited_during_focus_wait", "recorder_port_plan_invalid",
    "recording_watchdog_fired", "secure_desktop_transition_reverted",
    "secure_desktop_transition_timeout", "session_changed_during_focus_wait",
    "silent_recorder_plan_invalid", "silent_scope_violated",
    "windows_session_state_unknown", "windows_session_transition_invalid",
})
WTS_ERROR_CODES = {
    "lock_probe_unavailable", "lock_probe_failed", "invalid_session",
    "incomplete_wts_state", "mismatched_wts_state", "inactive_wts_session",
    "secure_desktop_unavailable", "unknown_wts_flag",
    "process_session_query_failed", "wts_query_failed", "wts_short_buffer",
    "wts_unsupported_level", "wts_session_mismatch", "wts_session_state_unknown",
    "wts_session_flags_unknown", "wts_session_flags_invalid",
    "windows_session_state_unknown", "unrecognized_wts_error",
}
MARKER_ID = "public_cedar_garden"
MARKER_PREFIX = "public cedar garden control "
LOCKED_MESSAGE = "Screen locked; screen/UI capture paused."
UNLOCKED_MESSAGE = "Screen unlocked; other privacy and recording controls still apply."
NOTICE_MESSAGES = {
    "wts_session_locked": LOCKED_MESSAGE,
    "wts_session_unlocked": UNLOCKED_MESSAGE,
    "wts_session_disconnected": "Windows session disconnected; screen/UI capture paused.",
    "input_desktop_unavailable": "Secure desktop active or input desktop unavailable; screen/UI capture paused.",
    **{code: "Screen lock detection unavailable; screen/UI capture paused for privacy."
       for code in (
           "process_session_query_failed", "wts_query_failed", "wts_short_buffer",
           "wts_unsupported_level", "wts_session_mismatch", "wts_session_state_unknown",
           "wts_session_flags_unknown", "wts_session_flags_invalid",
       )},
}
PIN_REQUIRED = {
    "lock_live.py", "test_lock_live.py", "lock_acceptance.py",
    "test_lock_acceptance.py", "launch.ps1",
}


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _json_object(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8-sig"))
    if not isinstance(value, dict):
        raise coordinator.ControlError("pin_manifest_invalid")
    return value


def verify_pin_manifest(path: Path, root: Path, schema: str, *,
                        required: set[str] | None = None,
                        exact_count: int | None = None) -> dict[str, Any]:
    """Verify a reviewed manifest without permitting path escape or empties."""
    try:
        manifest = _json_object(path)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        raise coordinator.ControlError("pin_manifest_missing_or_invalid") from error
    assets = manifest.get("assets")
    if manifest.get("schema") != schema or not isinstance(assets, dict) or not assets:
        raise coordinator.ControlError("pin_manifest_schema_invalid")
    if exact_count is not None and len(assets) != exact_count:
        raise coordinator.ControlError("pin_manifest_asset_count_invalid")
    if required is not None and set(assets) != required:
        raise coordinator.ControlError("pin_manifest_asset_set_invalid")
    resolved_root = root.resolve()
    for relative, expected in assets.items():
        if (not isinstance(relative, str) or not relative
                or not isinstance(expected, str) or len(expected) != 64):
            raise coordinator.ControlError("pin_manifest_entry_invalid")
        target = (resolved_root / relative).resolve()
        if not target.is_relative_to(resolved_root) or not target.is_file() or target.stat().st_size <= 0:
            raise coordinator.ControlError("pinned_asset_missing")
        actual = hashlib.sha256(target.read_bytes()).hexdigest().upper()
        if actual != expected.upper():
            raise coordinator.ControlError("pinned_asset_mismatch")
    return {"schema": schema, "asset_count": len(assets)}


def verify_all_pins() -> list[dict[str, Any]]:
    """Verify old 34, tail 16, and this controller before gate consumption."""
    return [
        verify_pin_manifest(OLD_PINS, PREP, "screenwise.prepared-controller-pins.v1", exact_count=34),
        verify_pin_manifest(TAIL_PINS, TAIL_PREP.parent, "screenwise.tail-collector-pins.v1", exact_count=16),
        verify_pin_manifest(NEW_PINS, HERE, "screenwise.lock-transition-pins.v1", required=PIN_REQUIRED),
    ]


def load_evaluator(path: Path = LOCK_ACCEPTANCE):
    spec = importlib.util.spec_from_file_location("screenwise_lock_acceptance", path)
    if spec is None or spec.loader is None:
        raise coordinator.ControlError("lock_evaluator_unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    evaluator = getattr(module, "evaluate_lock_acceptance", None)
    if not callable(evaluator):
        raise coordinator.ControlError("lock_evaluator_unavailable")
    return evaluator


def write_json_atomic(path: Path, value: dict[str, Any]) -> None:
    temp = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    payload = json.dumps(value, indent=2, sort_keys=True) + "\n"
    with temp.open("x", encoding="utf-8") as stream:
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temp, path)


def safe_evaluator_error_class(error: Exception) -> str:
    """Map evaluator exceptions to fixed codes without exposing their text."""
    return {
        TypeError: "type_error",
        ValueError: "value_error",
        KeyError: "key_error",
        IndexError: "index_error",
        AttributeError: "attribute_error",
        RuntimeError: "runtime_error",
        OverflowError: "overflow_error",
    }.get(type(error), "other_error")


def safe_controller_error(error: Exception) -> dict[str, str]:
    """Return fixed controller diagnostics without copying exception text."""
    if isinstance(error, coordinator.ControlError):
        candidate = str(error)
        code = candidate if candidate in CONTROL_ERROR_CODES else "controller_control_error"
        error_class = "control_error"
    else:
        code = "controller_failure"
        error_class = safe_evaluator_error_class(error)
    return {"code": code, "class": error_class}


def phase(path: Path, state: str, observed_ms: int) -> None:
    value = {"schema": "screenwise.lock-transition-phase.v1", "phase": state,
             "observed_ms": observed_ms}
    write_json_atomic(path / "phase.json", value)
    print(json.dumps(value, sort_keys=True), flush=True)


class LockWindowsBackend(tail.TailWindowsBackend):
    """Silent vision/UIA adapter built on the reviewed Windows ownership layer."""

    def __init__(self, record: dict[str, Any], path: Path, *,
                 clock: Callable[[], float] = time.monotonic,
                 sleeper: Callable[[float], None] = time.sleep):
        super().__init__(record, path, clock=clock, sleeper=sleeper)
        self.run_start_utc = utc_now()
        self.run_start_monotonic_ms = int(clock() * 1000)
        self.active_deadline: float | None = None
        self.watchdog_done = threading.Event()
        self.watchdog_fired = threading.Event()
        self.stop_lock = threading.RLock()
        self.last_control_diagnostic: dict[str, Any] | None = None

    def start_recorder(self) -> None:
        self.data.mkdir(exist_ok=False)
        self._data_marker().write_text(self.record["run_id"], encoding="utf-8")
        spec = importlib.util.spec_from_file_location("lock_prepared_plan", live.PREP / "plan.py")
        if spec is None or spec.loader is None:
            raise coordinator.ControlError("prepared_plan_unavailable")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        # The coordinator session already exists; use a separate never-created
        # plan root, then replace its data path with this owned session path.
        args = list(module.make_plan(
            "privacy", self.record["run_id"], self.path / "plan-preview-root"
        )["argv"])
        args[args.index("--data-dir") + 1] = str(self.data)
        args += [
            "--disable-keyboard-capture", "--disable-clipboard-capture",
            "--included-windows", "ScreenWise Synthetic Privacy Fixture",
        ]
        for flag in ("--disable-audio", "--disable-keyboard-capture",
                     "--disable-clipboard-capture", "--disable-meeting-detector",
                     "--disable-snapshot-compaction"):
            if args.count(flag) != 1:
                raise coordinator.ControlError("silent_recorder_plan_invalid")
        # make_plan already supplies these; reject accidental conflicting duplicates.
        if args.count("--port") != 1 or args[args.index("--port") + 1] != str(PORT):
            raise coordinator.ControlError("recorder_port_plan_invalid")
        self.journal("command", {"argv": args})
        self.recorder = live.windows.OwnedProcess.start(
            args, live.RELEASE, self.data / "stdout.log", self.data / "stderr.log"
        )
        self.shutdown = None
        self.shutdown_attempts = []
        self.active_deadline = self.clock() + WATCHDOG_SECONDS
        identity = (self.recorder.pid, self.recorder.creation_time_ticks)
        self.journal("recorder_identity", {"pid": identity[0], "creation_ticks": identity[1]})
        threading.Thread(target=self._lock_watchdog,
                         args=(self.recorder, identity, self.active_deadline), daemon=True).start()

    def _lock_watchdog(self, owned: Any, identity: tuple[int, int], deadline: float) -> None:
        if self.watchdog_done.wait(max(0.0, deadline - self.clock())):
            return
        with self.stop_lock:
            if (self.recorder is not owned or owned.pid != identity[0]
                    or owned.creation_time_ticks != identity[1]):
                self.journal("watchdog", {"reason": "owned_identity_changed"})
                return
            self.watchdog_fired.set()
            try:
                self._stop_current_recorder()
            finally:
                self.watchdog_done.set()
                self.journal("watchdog", {"reason": "recording_safety_deadline",
                                           "seconds": WATCHDOG_SECONDS})

    def stop_recorder(self):
        with self.stop_lock:
            stopped = self._stop_current_recorder()
            if stopped:
                self.watchdog_done.set()
            return stopped

    def _connect_readonly(self) -> sqlite3.Connection:
        self._validate_owned_data()
        database = (self.data / "db.sqlite").resolve()
        if database.parent != self.data.resolve() or not database.is_file():
            raise coordinator.ControlError("owned_database_missing")
        return sqlite3.connect(f"file:{database.as_posix()}?mode=ro", uri=True)

    @staticmethod
    def _columns(db: sqlite3.Connection, table: str) -> set[str]:
        return {str(row[1]) for row in db.execute(f'PRAGMA table_info("{table}")')}

    @classmethod
    def _require_schema(cls, db: sqlite3.Connection, tables: set[str],
                        table: str, columns: set[str]) -> None:
        if table not in tables or not columns.issubset(cls._columns(db, table)):
            raise coordinator.ControlError("owned_database_schema_invalid")

    def capture_counts(self) -> dict[str, int]:
        """Return only row counts used to define the locked plateau."""
        with closing(self._connect_readonly()) as db:
            tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            self._require_schema(db, tables, "frames", {"id", "accessibility_text", "accessibility_tree_json"})
            self._require_schema(db, tables, "ui_events", {"event_type"})
            frames = int(db.execute("SELECT COUNT(*) FROM frames").fetchone()[0])
            uia = 0
            predicates = [f'"{name}" IS NOT NULL AND "{name}" != \'\''
                          for name in ("accessibility_text", "accessibility_tree_json")]
            uia = int(db.execute(
                "SELECT COUNT(*) FROM frames WHERE " + " OR ".join(f"({item})" for item in predicates)
            ).fetchone()[0])
            notices = int(db.execute("SELECT COUNT(*) FROM ui_events WHERE event_type='privacy_notice'").fetchone()[0])
            non_notice_ui = int(db.execute(
                "SELECT COUNT(*) FROM ui_events WHERE event_type IS NULL OR event_type!='privacy_notice'"
            ).fetchone()[0])
            # Input capture is disabled; an unexpected non-notice UI event is
            # still included so it cannot be hidden from the plateau.
            return {"frame_rows": frames, "uia_capture_rows": uia + non_notice_ui,
                    "privacy_notice_rows": notices}

    def control_hits(self, generation: int) -> dict[str, int]:
        marker = f"{MARKER_PREFIX}{generation}"
        with closing(self._connect_readonly()) as db:
            tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            self._require_schema(db, tables, "frames", {"id", "accessibility_text", "accessibility_tree_json"})
            frame_ids: set[int] = set()
            uia_hits = 0
            for column in ("accessibility_text", "accessibility_tree_json"):
                matching = [int(row[0]) for row in db.execute(
                    f'SELECT id FROM frames WHERE instr(lower("{column}"), lower(?)) > 0',
                    (marker,))]
                frame_ids.update(matching)
                uia_hits += len(matching)
        # Search responses are cached for 60 seconds.  A fixed poll route can
        # therefore replay the first pre-persistence empty response throughout
        # this controller's 30-second positive-control window.  Scoping the
        # query to this run and advancing its end time gives it a fresh cache
        # key at least once per second (the server hashes timestamps to seconds)
        # without changing the required content_type=all semantics.
        code, body = self.protected_request(
            "/search?" + urlencode({
                "content_type": "all", "q": marker, "limit": 20,
                "start_time": self.run_start_utc, "end_time": utc_now(),
            }))
        try:
            value = json.loads(body) if code == 200 else {}
            rows = value.get("data") if isinstance(value, dict) else None
            search_hits = len(rows) if isinstance(rows, list) else 0
        except (TypeError, ValueError):
            search_hits = 0
        return {"frame_hits": len(frame_ids), "uia_hits": uia_hits,
                "authenticated_search_hits": search_hits}

    def auth_matrix(self) -> dict[str, Any]:
        if not self.token:
            raise coordinator.ControlError("auth_token_unavailable")
        route = "/search?content_type=all&limit=1"
        missing = self.protected_request(route, authenticated=False)[0]
        saved = self.token
        self.token = "wrong-token"
        try:
            wrong = self.protected_request(route, authenticated=True)[0]
        finally:
            self.token = saved
        valid = self.protected_request(route, authenticated=True)[0]
        return {"missing_status": missing, "wrong_status": wrong,
                "valid_status": valid, "redirect_followed": False}

    def silence_snapshot(self) -> dict[str, int]:
        code, body = self.protected_request("/audio/device/status")
        try:
            rows = json.loads(body) if code == 200 else None
        except (TypeError, ValueError):
            rows = None
        if not isinstance(rows, list):
            raise coordinator.ControlError("audio_status_schema_invalid")
        running = sum(1 for row in rows if isinstance(row, dict) and row.get("is_running") is True)
        with closing(self._connect_readonly()) as db:
            tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            self._require_schema(db, tables, "audio_chunks", {"id"})
            self._require_schema(db, tables, "audio_transcriptions", {"id"})
            chunks = int(db.execute("SELECT COUNT(*) FROM audio_chunks").fetchone()[0])
            transcripts = int(db.execute("SELECT COUNT(*) FROM audio_transcriptions").fetchone()[0])
        if running or chunks or transcripts:
            raise coordinator.ControlError("silent_scope_violated")
        return {"running_device_count": running, "audio_chunk_rows": chunks,
                "audio_transcription_rows": transcripts}

    def accepted_plain_generation(self) -> int:
        """Count matched successful plain acks; focus success is not assumed."""
        command_dir = self.fixture_dir / "commands"
        ack_dir = self.fixture_dir / "acks"
        count = 0
        if not command_dir.is_dir() or not ack_dir.is_dir():
            return 0
        for command_path in sorted(command_dir.glob("*.json")):
            try:
                command = _json_object(command_path)
                ack = _json_object(ack_dir / command_path.name)
            except (OSError, ValueError, json.JSONDecodeError):
                continue
            if (command.get("runId") == self.record["run_id"]
                    and command.get("action") == "plain"
                    and ack.get("runId") == command.get("runId")
                    and ack.get("phaseId") == command.get("phaseId")
                    and ack.get("action") == "plain" and ack.get("success") is True):
                count += 1
        return count

    def post_unlock_plain_once(self) -> int:
        before = self.accepted_plain_generation()
        self.counter += 1
        ack = self.client.send(f"{self.counter:04d}-post-unlock-plain", "plain",
                               expected_pid=self.fixture.pid)
        if ack.get("success") is not True:
            raise coordinator.ControlError("post_unlock_plain_failed")
        after = self.accepted_plain_generation()
        if after != before + 1:
            raise coordinator.ControlError("fixture_generation_unverified")
        if ack.get("verifiedForeground") is True:
            self.expected_pid = self.fixture.pid
            self.expected_hwnd = ack.get("foregroundHwnd")
        return after

    def wait_fixture_focus(self, deadline: float) -> None:
        while self.clock() < deadline:
            if self.session_state() != "unlocked":
                raise coordinator.ControlError("session_changed_during_focus_wait")
            if self.recorder is None or self.recorder.poll() is not None:
                raise coordinator.ControlError("recorder_exited_during_focus_wait")
            identity = live.windows.foreground_identity()
            if (identity.get("pid") == self.fixture.pid and type(identity.get("hwnd")) is int
                    and identity["hwnd"] > 0):
                self.counter += 1
                ack = self.client.send(f"{self.counter:04d}-post-unlock-show", "show",
                                       expected_pid=self.fixture.pid)
                if ack.get("success") is True and ack.get("verifiedForeground") is True:
                    self.expected_pid, self.expected_hwnd = self.fixture.pid, ack["foregroundHwnd"]
                    return
            self.sleeper(POLL_SECONDS)
        raise coordinator.ControlError("fixture_focus_timeout")

    def process_observation(self, checkpoint: str, observed_ms: int) -> dict[str, Any]:
        self._verify_owned_identity()
        return {"checkpoint": checkpoint, "pid": self.recorder.pid,
                "creation_time": self.recorder.creation_time_ticks,
                "alive": self.recorder.poll() is None, "observed_ms": observed_ms}

    def normalized_auth(self, checkpoint: str, observed_ms: int) -> dict[str, Any]:
        matrix = self.auth_matrix()
        return {"checkpoint": checkpoint, "observed_ms": observed_ms,
                "bind_host": "127.0.0.1", "port": PORT, "endpoint": "/search",
                "content_type": "all", "missing": matrix.get("missing_status"), "wrong": matrix.get("wrong_status"),
                "valid": matrix.get("valid_status")}

    def capture_notices(self) -> tuple[list[dict[str, Any]], list[str]]:
        query = urlencode({"start_time": self.run_start_utc, "end_time": utc_now(), "limit": 1000})
        code, body = self.protected_request("/capture-events?" + query)
        if code != 200:
            raise coordinator.ControlError("capture_status_unavailable")
        value = json.loads(body)
        if (not isinstance(value, dict) or value.get("has_more") is not False
                or value.get("persistence_degraded") is not False):
            raise coordinator.ControlError("capture_status_degraded")
        if value.get("audio_shutdown_degraded") is not False or value.get("audio_shutdown_issues") != []:
            raise coordinator.ControlError("capture_shutdown_status_degraded")
        delivery = value.get("event_delivery")
        if (not isinstance(delivery, dict) or delivery.get("near_capacity") is not False
                or delivery.get("dropped_events") != 0):
            raise coordinator.ControlError("capture_delivery_degraded")
        notices = []
        issues = []
        allowed = NOTICE_MESSAGES
        start_utc = datetime.fromisoformat(self.run_start_utc)
        rows = value.get("data")
        if not isinstance(rows, list):
            raise coordinator.ControlError("capture_status_schema_invalid")
        for row in reversed(rows):  # API is newest-first; evaluator requires increasing time.
            if not isinstance(row, dict):
                raise coordinator.ControlError("capture_status_schema_invalid")
            reason = row.get("reason_code")
            if reason not in allowed or row.get("message") != allowed[reason]:
                issues.append("unexpected_safe_notice")
                continue
            try:
                stamp = datetime.fromisoformat(str(row.get("timestamp")))
                timestamp_ms = self.run_start_monotonic_ms + int((stamp - start_utc).total_seconds() * 1000)
            except (TypeError, ValueError):
                raise coordinator.ControlError("capture_notice_timestamp_invalid")
            notices.append({"typed": True, "reason_code": reason,
                            "message": allowed[reason], "timestamp_ms": timestamp_ms})
        return notices, sorted(set(issues))

    def safe_lock_log_evidence(self, notices: list[dict[str, Any]]) -> dict[str, Any]:
        counts = {code: 0 for code in sorted({row["reason_code"] for row in notices})}
        messages = NOTICE_MESSAGES
        paths = (self.data / "stdout.log", self.data / "stderr.log")
        if not all(path.is_file() for path in paths):
            return {"safe_notice_lines_verified": False, "reason_code_counts": counts}
        lines = []
        for path in paths:
            lines.extend(path.read_text(encoding="utf-8", errors="replace").splitlines())
        ansi = re.compile(r"\x1b\[[0-9;]*m")
        verified = True
        normalized = [ansi.sub("", line) for line in lines]
        timestamp = r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z"
        patterns = {
            code: re.compile(
                rf"^{timestamp}\s+(?:INFO|WARN)\s+screenpipe_engine::sleep_monitor:\s+"
                rf"{re.escape(message)}\s+reason_code=\"{re.escape(code)}\"\s*$")
            for code, message in messages.items()
        }
        for line in normalized:
            present = [code for code in messages if code in line]
            if present and (len(present) != 1 or not patterns[present[0]].search(line)):
                verified = False
        for code in counts:
            matching = [line for line in normalized if patterns[code].search(line)]
            counts[code] = len(matching)
            if len(matching) != sum(row["reason_code"] == code for row in notices):
                verified = False
        return {"safe_notice_lines_verified": verified, "reason_code_counts": counts}

    def post_shutdown_issues(self) -> list[str]:
        issues: list[str] = []
        with closing(self._connect_readonly()) as db:
            if str(db.execute("PRAGMA quick_check").fetchone()[0]).lower() != "ok":
                issues.append("database_integrity_unverified")
            tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            if "ui_events" not in tables:
                issues.append("notice_table_missing")
            else:
                rows = db.execute(
                    "SELECT text_content FROM ui_events WHERE event_type='privacy_notice'").fetchall()
                for (raw,) in rows:
                    try:
                        value = json.loads(raw)
                    except (TypeError, ValueError):
                        issues.append("unknown_safe_notice_schema")
                        continue
                    if (not isinstance(value, dict) or set(value) != {"reason"}
                            or value.get("reason") not in NOTICE_MESSAGES):
                        issues.append("unknown_safe_notice_schema")
        fixed_log_failures = {
            "Privacy timeline writer stopped unexpectedly; consult the local diagnostic log.":
                "notice_writer_stopped",
            "Privacy timeline writer shutdown timed out; pending notices remain in the local diagnostic log.":
                "notice_writer_timeout",
            "Recording status notice could not be saved after retries; consult the local diagnostic log.":
                "notice_persistence_failed",
            "Recording status notice was invalid; no event payload was saved.":
                "invalid_notice_rejected",
        }
        for path in (self.data / "stdout.log", self.data / "stderr.log"):
            if not path.is_file():
                issues.append("owned_log_missing")
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            for fixed, code in fixed_log_failures.items():
                if fixed in text:
                    issues.append(code)
        return sorted(set(issues))


class LockTransitionController:
    def __init__(self, backend: Any, readiness: dict[str, Any], *,
                 clock: Callable[[], float] = time.monotonic,
                 sleeper: Callable[[float], None] = time.sleep,
                 evaluator: Callable[[dict[str, Any]], dict[str, Any]] | None = None):
        self.backend, self.readiness = backend, readiness
        self.clock, self.sleeper = clock, sleeper
        self.evaluator = evaluator or load_evaluator()
        self.sample_number = 0
        self.last_sample_ms: int | None = None

    def ms(self) -> int:
        return int(self.clock() * 1000)

    def sample_wts(self, evidence: dict[str, Any]) -> str:
        while self.last_sample_ms is not None and self.ms() <= self.last_sample_ms:
            self.sleeper(0.001)
        raw = live.windows.session_state()
        self.sample_number += 1
        observed = self.ms()
        state, error = raw.get("state"), raw.get("error_code")
        if state not in ("locked", "unlocked") or error is not None:
            state = "unknown"
            error = error if isinstance(error, str) and error else "windows_session_state_unknown"
            if error not in WTS_ERROR_CODES:
                error = "unrecognized_wts_error"
        evidence["wts_samples"].append({
            "sample_id": f"wts-{self.sample_number:04d}", "observed_ms": observed,
            "state": state, "source": "windows.session_state", "real": True,
            "independent": True, "error_code": error,
        })
        self.last_sample_ms = observed
        return state

    def wait_transition(self, evidence: dict[str, Any], *, from_state: str,
                        to_state: str) -> None:
        """Wait for one confirmed transition, retaining a bounded secure-desktop gap."""
        left_ms: int | None = next(
            (
                sample.get("observed_ms")
                for sample in reversed(evidence.get("wts_samples", []))
                if sample.get("state") == from_state and sample.get("error_code") is None
            ),
            None,
        )
        unknown_count = 0
        while True:
            self.guard()
            state = self.sample_wts(evidence)
            sample = evidence["wts_samples"][-1]
            if state == to_state:
                if (unknown_count and left_ms is not None
                        and sample["observed_ms"] - left_ms > int(TRANSITION_UNKNOWN_MAX_SECONDS * 1000)):
                    raise coordinator.ControlError("secure_desktop_transition_timeout")
                return
            if state == from_state:
                if unknown_count:
                    raise coordinator.ControlError("secure_desktop_transition_reverted")
                left_ms = sample["observed_ms"]
            elif state == "unknown":
                if sample["error_code"] != TRANSITION_UNKNOWN_CODE or left_ms is None:
                    raise coordinator.ControlError("windows_session_state_unknown")
                unknown_count += 1
                if (unknown_count > TRANSITION_UNKNOWN_MAX_SAMPLES
                        or sample["observed_ms"] - left_ms > int(TRANSITION_UNKNOWN_MAX_SECONDS * 1000)):
                    raise coordinator.ControlError("secure_desktop_transition_timeout")
            else:
                raise coordinator.ControlError("windows_session_transition_invalid")
            self.sleeper(POLL_SECONDS)

    def guard(self) -> None:
        if self.backend.active_deadline is None or self.clock() >= self.backend.active_deadline:
            raise coordinator.ControlError("active_recording_deadline")
        if self.backend.watchdog_fired.is_set():
            raise coordinator.ControlError("recording_watchdog_fired")
        if self.backend.recorder is None or self.backend.recorder.poll() is not None:
            raise coordinator.ControlError("recorder_exited")

    def wait_control(self, checkpoint: str, generation: int) -> dict[str, Any]:
        until = min(self.backend.active_deadline, self.clock() + CONTROL_WAIT_SECONDS)
        while self.clock() < until:
            self.guard()
            if self.backend.session_state() != "unlocked" or not self.backend.foreground_valid("control"):
                raise coordinator.ControlError("control_focus_or_session_changed")
            hits = self.backend.control_hits(generation)
            self.backend.last_control_diagnostic = {
                "checkpoint": checkpoint, "generation": generation, **hits,
                "observed_ms": self.ms(),
            }
            if (hits["frame_hits"] > 0 and hits["uia_hits"] > 0
                    and hits["authenticated_search_hits"] > 0):
                return {"marker_id": MARKER_ID, "generation": generation,
                        **hits, "observed_ms": self.ms()}
            self.sleeper(POLL_SECONDS)
        self.backend.journal("control_timeout", dict(self.backend.last_control_diagnostic or {
            "checkpoint": checkpoint, "generation": generation,
            "frame_hits": 0, "uia_hits": 0, "authenticated_search_hits": 0,
        }))
        raise coordinator.ControlError("positive_control_timeout")

    def persist_evidence_checkpoint(
        self,
        evidence: dict[str, Any],
        issues: list[str],
        stage: str,
        *,
        evaluator_error: dict[str, str] | None = None,
    ) -> bool:
        if stage not in EVIDENCE_CHECKPOINT_STAGES:
            return False
        value: dict[str, Any] = {
            "schema": EVIDENCE_CHECKPOINT_SCHEMA,
            "stage": stage,
            "evidence": evidence,
            "controller_issues": sorted(set(issues)),
        }
        if evaluator_error is not None:
            value["evaluator_error"] = evaluator_error
        try:
            write_json_atomic(
                self.backend.path / f"lock-evidence-{stage}.json", value
            )
            write_json_atomic(self.backend.path / "lock-evidence.json", value)
            return True
        except Exception:
            return False

    def run(self) -> dict[str, Any]:
        b = self.backend
        evidence: dict[str, Any] = {
            "schema": "screenwise.lock-transition-evidence.v1",
            "owner_readiness": self.readiness,
            "clock": {"run_start_utc": b.run_start_utc,
                      "run_start_monotonic_ms": b.run_start_monotonic_ms,
                      "notice_mapping": "fixed_run_start_utc_monotonic_offset", "verified": True},
            "process": {"pid": 0, "creation_time": 0, "scope_count": 1,
                        "observations": [], "shutdown": {"observed": False, "exit_code": None,
                                                           "forced": False, "issues": []}},
            "controls": {}, "wts_samples": [], "locked_plateau": {}, "notices": [],
            "logs": {"safe_notice_lines_verified": False, "reason_code_counts": {}},
            "api_checks": [], "audio": {"enabled": False, "tested": False,
                                             "running_device_count": None,
                                             "audio_chunk_rows": None,
                                             "audio_transcription_rows": None},
            "cleanup": {"processes_stopped": False, "fixture_closed": False},
        }
        issues: list[str] = []
        controller_error = None
        try:
            b.preflight()
            if b.session_state() != "unlocked":
                raise coordinator.ControlError("initial_session_not_unlocked")
            # May wait indefinitely for owner focus; no recorder exists yet.
            b.start_fixture()
            before_generation = b.accepted_plain_generation()
            if before_generation < 1:
                raise coordinator.ControlError("fixture_generation_unverified")
            b.start_recorder()
            b.await_api()
            b.verify_owned_loopback_listener()
            evidence["process"].update(pid=b.recorder.pid,
                                       creation_time=b.recorder.creation_time_ticks)
            evidence["controls"]["before"] = self.wait_control("before", before_generation)
            evidence["api_checks"].append(b.normalized_auth("before", self.ms()))
            evidence["audio"].update(b.silence_snapshot())
            evidence["process"]["observations"].append(
                b.process_observation("pre_lock", self.ms()))
            if self.sample_wts(evidence) != "unlocked":
                raise coordinator.ControlError("pre_lock_state_changed")
            if b.release_focus() is not True:
                raise coordinator.ControlError("fixture_hide_failed")
            if not self.persist_evidence_checkpoint(evidence, issues, "ready_for_lock"):
                issues.append("aggregate_evidence_persistence_failed")
            phase(b.path, "ready_for_lock", self.ms())

            self.wait_transition(evidence, from_state="unlocked", to_state="locked")
            lock_seen = self.clock()
            while self.clock() < lock_seen + SETTLE_SECONDS:
                self.guard()
                if self.sample_wts(evidence) != "locked":
                    raise coordinator.ControlError("lock_not_stable_during_settle")
                self.sleeper(POLL_SECONDS)
            if self.sample_wts(evidence) != "locked":
                raise coordinator.ControlError("lock_not_stable_after_settle")
            start_counts = b.capture_counts()
            plateau_start = self.ms()
            while self.clock() < lock_seen + SETTLE_SECONDS + PLATEAU_SECONDS:
                self.guard()
                if self.sample_wts(evidence) != "locked":
                    raise coordinator.ControlError("lock_plateau_interrupted")
                self.sleeper(POLL_SECONDS)
            end_counts = b.capture_counts()
            plateau_end = self.ms()
            if self.sample_wts(evidence) != "locked":
                raise coordinator.ControlError("lock_not_stable_at_plateau_end")
            evidence["locked_plateau"] = {
                "grace_complete": True, "start_ms": plateau_start, "end_ms": plateau_end,
                "start_frame_rows": start_counts["frame_rows"],
                "end_frame_rows": end_counts["frame_rows"],
                "start_uia_capture_rows": start_counts["uia_capture_rows"],
                "end_uia_capture_rows": end_counts["uia_capture_rows"],
                "start_privacy_notice_rows": start_counts["privacy_notice_rows"],
                "end_privacy_notice_rows": end_counts["privacy_notice_rows"],
            }
            evidence["process"]["observations"].append(
                b.process_observation("locked_stable_end", self.ms()))
            if not self.persist_evidence_checkpoint(
                evidence, issues, "locked_observation_complete"
            ):
                issues.append("aggregate_evidence_persistence_failed")
            phase(b.path, "locked_observation_complete", self.ms())
            self.wait_transition(evidence, from_state="locked", to_state="unlocked")
            if not self.persist_evidence_checkpoint(
                evidence, issues, "unlocked_waiting_fixture_focus"
            ):
                issues.append("aggregate_evidence_persistence_failed")
            phase(b.path, "unlocked_waiting_fixture_focus", self.ms())
            after_generation = b.post_unlock_plain_once()
            b.wait_fixture_focus(b.active_deadline)
            evidence["controls"]["after"] = self.wait_control("after", after_generation)
            evidence["api_checks"].append(b.normalized_auth("after", self.ms()))
            evidence["audio"].update(b.silence_snapshot())
            evidence["process"]["observations"].append(
                b.process_observation("post_unlock", self.ms()))

            # Allow the async notice writer a bounded interval to persist the
            # final unlock row; retain all completed checkpoints on timeout.
            notice_deadline = min(b.active_deadline, self.clock() + 10.0)
            while self.clock() < notice_deadline:
                evidence["notices"], notice_issues = b.capture_notices()
                codes = [row["reason_code"] for row in evidence["notices"]]
                if (codes.count("wts_session_locked") == 1
                        and codes[-1:] == ["wts_session_unlocked"]
                        and codes.index("wts_session_locked") < len(codes) - 1):
                    issues.extend(notice_issues)
                    break
                self.sleeper(POLL_SECONDS)
            else:
                issues.append("typed_notice_sequence_missing")
        except Exception as error:
            controller_error = safe_controller_error(error)
            reason = controller_error["code"]
            issues.append(reason)
            diagnostic = getattr(b, "last_control_diagnostic", None)
            if reason == "positive_control_timeout" and isinstance(diagnostic, dict):
                evidence["control_timeout"] = dict(diagnostic)
        finally:
            # Recorder always stops before fixture cleanup.  No force fallback
            # is accepted for the recorder.
            if not evidence["notices"] and getattr(b, "recorder", None) is not None:
                try:
                    if b.recorder.poll() is None:
                        evidence["notices"], partial_notice_issues = b.capture_notices()
                        issues.extend(partial_notice_issues)
                except Exception:
                    issues.append("partial_notice_collection_unavailable")
            try:
                stopped = b.stop_recorder() is True
            except Exception:
                stopped = False
            shutdown = b.shutdown if isinstance(b.shutdown, dict) else {}
            forced = any(row.get("forced") is True for row in b.shutdown_attempts)
            shutdown_issues = []
            if stopped:
                try:
                    shutdown_issues.extend(b.post_shutdown_issues())
                except Exception:
                    shutdown_issues.append("post_shutdown_evidence_unavailable")
            if not stopped or shutdown.get("exit_code") != 0 or forced:
                shutdown_issues.append("unclean_shutdown")
            evidence["process"]["shutdown"] = {
                "observed": stopped, "exit_code": shutdown.get("exit_code"),
                "forced": forced, "issues": sorted(set(shutdown_issues)),
            }
            try:
                if not evidence["notices"] and stopped:
                    # API is gone after stop, so only pre-stop collection is valid.
                    issues.append("notices_not_collected_before_shutdown")
                evidence["logs"] = b.safe_lock_log_evidence(evidence["notices"])
            except Exception:
                issues.append("safe_log_evidence_unavailable")
            try:
                fixture_closed = b.close_fixture() is True
            except Exception:
                fixture_closed = False
            try:
                processes_stopped = b.processes_stopped() is True
            except Exception:
                processes_stopped = False
            evidence["cleanup"] = {"processes_stopped": processes_stopped,
                                   "fixture_closed": fixture_closed}
            if not self.persist_evidence_checkpoint(evidence, issues, "cleanup_complete"):
                issues.append("aggregate_evidence_persistence_failed")
            try:
                phase(b.path, "finished", self.ms())
            except Exception:
                issues.append("finished_phase_write_failed")
        evaluator_error = None
        try:
            decision = self.evaluator(evidence)
            if not isinstance(decision, dict):
                raise TypeError("evaluator result must be an object")
        except Exception as error:
            evaluator_error = {
                "code": "evaluator_exception",
                "class": safe_evaluator_error_class(error),
            }
            issues.append("evaluator_exception")
            decision = {
                "schema": "screenwise.lock-transition-acceptance.v1",
                "status": "incomplete",
                "reasons": ["evaluator_exception"],
                "eligible": False,
                "scope_note": (
                    "Aggregate frame/UIA row-count plateau evidence does not prove that "
                    "every pixel was protected."
                ),
                "audio_scope": (
                    "Audio was disabled and lock recovery for audio remains untested."
                ),
                "additional_notice_reason_counts": {},
                "wts_unknown_reason_counts": {},
            }
            if not self.persist_evidence_checkpoint(
                evidence, issues, "evaluation_failed", evaluator_error=evaluator_error
            ):
                issues.append("aggregate_evidence_persistence_failed")
        if issues and decision.get("status") == "pass":
            decision = dict(decision, status="incomplete", eligible=False,
                            reasons=sorted(set(decision.get("reasons", []) + issues)))
        result = {"schema": "screenwise.lock-transition-run.v1", "decision": decision,
                  "evidence": evidence, "controller_issues": sorted(set(issues))}
        if evaluator_error is not None:
            result["evaluator_error"] = evaluator_error
        if controller_error is not None:
            result["controller_error"] = controller_error
        return result


def preview(run_id: str) -> dict[str, Any]:
    return {
        "schema": "screenwise.lock-transition-preview.v1", "run_id": run_id,
        "execution": False, "audio": False, "watchdog_seconds": WATCHDOG_SECONDS,
        "required": ["--execute-interactive", "--owner-ready <fresh nonce>"],
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Preview by default; run one bounded silent lock transition only with a fresh owner gate")
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--owner-ready")
    parser.add_argument("--execute-interactive", action="store_true")
    args = parser.parse_args(argv)
    if not args.execute_interactive:
        print(json.dumps(preview(args.run_id), indent=2))
        return 0
    live.require_runtime_configuration()
    if not args.owner_ready:
        print(json.dumps({"status": "incomplete", "reason": "owner_readiness_required"}))
        return 2
    try:
        # This must precede consume_confirmation so a changed asset cannot burn
        # a fresh owner nonce.
        verify_all_pins()
        run_path = coordinator.session_path(args.run_id, CONTROLLER)
        waiting = _json_object(run_path / "waiting.json")
        if waiting.get("mode") != "lock" or waiting.get("state") != "waiting_for_owner":
            raise coordinator.ControlError("owner_gate_mode_mismatch")
        backend = LockWindowsBackend(waiting, run_path)
        # Establish actual stopped/hidden scope before consuming the one-shot
        # readiness gate; this read-only inventory does not start a fixture.
        backend.quiescent()
        record = coordinator.consume_confirmation(
            args.run_id, args.owner_ready, recorder_stopped=True,
            fixtures_hidden=True, root=CONTROLLER,
        )
        readiness = {"fresh": True,
                     "waited_indefinitely": record.get("readiness_timeout_seconds") is None,
                     "phase_id": args.run_id}
        backend.record = record
        result = LockTransitionController(backend, readiness).run()
        write_json_atomic(run_path / "lock-result.json", result)
        print(json.dumps({"schema": result["schema"], "decision": result["decision"]}, indent=2), flush=True)
        return 0 if result["decision"].get("status") == "pass" else 1
    except Exception as error:
        failure = safe_controller_error(error)
        print(json.dumps({"status": "incomplete", "reason": failure["code"],
                          "controller_error": failure}), flush=True)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
