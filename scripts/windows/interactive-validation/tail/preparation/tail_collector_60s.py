"""Prepared collector for the preferred 60-second partial-tail acceptance run.

Importing this module performs no OS actions.  CLI execution defaults to preview.
The live path requires both ``--execute-interactive`` and the nonce from a fresh,
indefinite owner-readiness gate.  This collector is prepared but not live verified.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import sqlite3
import subprocess
import sys
import threading
import time
from contextlib import closing
from pathlib import Path
from typing import Any, Callable
from urllib.parse import urlencode

HERE = Path(__file__).resolve().parent
TARGET = HERE.parents[1]
CONTROLLER = TARGET / "interactive-prep-20260915-01a09e45" / "controller_v1"
if str(CONTROLLER) not in sys.path:
    sys.path.insert(0, str(CONTROLLER))

import coordinator  # noqa: E402
import live  # noqa: E402
from tail_acceptance_60s import evaluate  # noqa: E402

SPEECH = HERE.parent / "speech-retry"
SPEECH_VALIDATION = HERE.parent / "speech-validation.json"
MARKERS = {
    "baseline": "cobalt meadow seven",
    "tail": "violet harbor nine",
    "restart": "silver orchard five",
}
MARKER_QUERY_FORMS = {
    "baseline": (MARKERS["baseline"], "cobalt meadow 7"),
    "tail": (MARKERS["tail"], "violet harbor 9"),
    "restart": (MARKERS["restart"], "silver orchard 5"),
}
DURATIONS = {"baseline": 10.783, "tail": 2.884, "restart": 10.773}
WATCHDOG_SECONDS = 45.0
TARGET_STOP_SECONDS = 30.0
PIN_MANIFEST = HERE.parent / "collector-pins.json"
PLAYBACK_CHECK_SECONDS = 0.05
PLAYBACK_GUARD_SECONDS = 0.5


def _finite_number(value: Any) -> bool:
    import math

    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def _load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8-sig"))
    if not isinstance(value, dict):
        raise coordinator.ControlError("json_object_required")
    return value


def _marker_name_and_forms(marker: str) -> tuple[str, tuple[str, ...]]:
    """Resolve only a canonical synthetic marker to its fixed lexical forms."""
    for name, canonical in MARKERS.items():
        if marker == canonical:
            return name, MARKER_QUERY_FORMS[name]
    raise coordinator.ControlError("unknown_synthetic_marker")


def _marker_sql(column: str, forms: tuple[str, ...]) -> tuple[str, tuple[str, ...]]:
    clauses = " OR ".join(f"instr(lower({column}), lower(?)) > 0" for _ in forms)
    return f"({clauses})", forms


def verified_speech_assets() -> dict[str, Path]:
    """Validate only the three newly prepared synthetic WAVs and their metadata."""
    manifest = _load_json(SPEECH / "manifest.json")
    validation = _load_json(SPEECH_VALIDATION)
    if manifest.get("schema") != "screenwise.synthetic-tail-speech.v1":
        raise coordinator.ControlError("tail_speech_manifest_invalid")
    if manifest.get("playbackPerformed") is not False or validation.get("playback_performed") is not False:
        raise coordinator.ControlError("tail_speech_preparation_state_invalid")
    manifest_rows = {row.get("name"): row for row in manifest.get("files", []) if isinstance(row, dict)}
    validation_rows = {row.get("name"): row for row in validation.get("files", []) if isinstance(row, dict)}
    result: dict[str, Path] = {}
    for name in ("baseline", "tail", "restart"):
        row = manifest_rows.get(name, {})
        measured = validation_rows.get(name, {})
        path = (SPEECH / str(row.get("file", ""))).resolve()
        if path.parent != SPEECH.resolve() or not path.is_file():
            raise coordinator.ControlError("tail_speech_file_missing")
        digest = hashlib.sha256(path.read_bytes()).hexdigest().upper()
        if digest != row.get("sha256") or digest != measured.get("sha256"):
            raise coordinator.ControlError("tail_speech_hash_mismatch")
        if row.get("marker") != MARKERS[name] or measured.get("duration_seconds") != DURATIONS[name]:
            raise coordinator.ControlError("tail_speech_metadata_mismatch")
        if measured.get("sample_rate_hz") != 22050:
            raise coordinator.ControlError("tail_speech_format_mismatch")
        result[name] = path
    return result


def verify_collector_pins() -> dict[str, Any]:
    """Fail closed on the separately reviewed manifest for every new collector asset."""
    try:
        manifest = _load_json(PIN_MANIFEST)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        raise coordinator.ControlError("collector_pins_missing_or_invalid") from error
    if manifest.get("schema") != "screenwise.tail-collector-pins.v1" or not isinstance(manifest.get("assets"), dict):
        raise coordinator.ControlError("collector_pins_schema_invalid")
    root = HERE.parent.resolve()
    assets = manifest["assets"]
    if not assets:
        raise coordinator.ControlError("collector_pins_empty")
    for relative, expected in assets.items():
        if not isinstance(relative, str) or not isinstance(expected, str) or len(expected) != 64:
            raise coordinator.ControlError("collector_pin_entry_invalid")
        path = (root / relative).resolve()
        if not path.is_relative_to(root) or not path.is_file():
            raise coordinator.ControlError("collector_pinned_asset_missing")
        if hashlib.sha256(path.read_bytes()).hexdigest().upper() != expected.upper():
            raise coordinator.ControlError("collector_pinned_asset_mismatch")
    return {"schema": manifest["schema"], "asset_count": len(assets)}


class WindowsOneShotPlayer:
    """Lazy Windows playback; construction and import are side-effect free."""

    def play(self, path: Path) -> None:
        import winsound

        winsound.PlaySound(str(path), winsound.SND_FILENAME | winsound.SND_NODEFAULT)

    def stop(self) -> None:
        import winsound

        winsound.PlaySound(None, 0)


class TailWindowsBackend(live.WindowsBackend):
    def __init__(self, record: dict, path: Path, *, clock: Callable[[], float] = time.monotonic,
                 sleeper: Callable[[float], None] = time.sleep, player: Any | None = None):
        super().__init__(record, path)
        self.clock = clock
        self.sleeper = sleeper
        self.player = player or WindowsOneShotPlayer()
        self.phase_name: str | None = None
        self.process_created_at: float | None = None
        self.phase_watchdog_done = threading.Event()
        self.assets: dict[str, Path] | None = None
        # The inherited stop_recorder also acquires this lock.
        self.stop_lock = threading.RLock()
        self.phase_deadline: float | None = None

    def fixture_action(self, action: str, phase: str):
        try:
            return super().fixture_action(action, phase)
        except coordinator.ControlError as error:
            if str(error) != "fixture_focus_not_verified" or (action, phase) != ("plain", "initial-control"):
                raise
        # Windows may refuse programmatic focus. Wait passively for the owner;
        # no recorder or playback exists yet and no interaction deadline applies.
        self.journal("waiting", {"reason": "owner_fixture_focus_required", "timeout_seconds": None,
                                 "recording_started": False})
        while True:
            if self.recorder is not None:
                raise coordinator.ControlError("fixture_wait_requires_stopped_recorder")
            if self.fixture is None or self.fixture.poll() is not None or self.session_state() != "unlocked":
                raise coordinator.ControlError("fixture_focus_wait_interrupted")
            identity = live.windows.foreground_identity()
            if identity.get("pid") == self.fixture.pid and type(identity.get("hwnd")) is int and identity["hwnd"] > 0:
                try:
                    return super().fixture_action(action, phase)
                except coordinator.ControlError as error:
                    if str(error) != "fixture_focus_not_verified":
                        raise
            self.sleeper(0.25)

    def _data_marker(self) -> Path:
        return self.data / ".controller-owned"

    def _validate_owned_data(self) -> None:
        try:
            resolved = self.data.resolve()
            expected_parent = self.path.resolve()
        except OSError as error:
            raise coordinator.ControlError("data_path_resolution_failed") from error
        if resolved.parent != expected_parent or not self._data_marker().is_file():
            raise coordinator.ControlError("unowned_data_directory")
        if self._data_marker().read_text(encoding="utf-8") != self.record["run_id"]:
            raise coordinator.ControlError("data_owner_mismatch")

    def _argv(self, chunk_seconds: int) -> list[str]:
        spec = importlib.util.spec_from_file_location("tail_prepared_plan", live.PREP / "plan.py")
        if spec is None or spec.loader is None:
            raise coordinator.ControlError("prepared_plan_unavailable")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        args = list(module.make_plan("audio-output", self.record["run_id"])["argv"])
        args[args.index("--data-dir") + 1] = str(self.data)
        args[args.index("--audio-chunk-duration") + 1] = str(chunk_seconds)
        args += [
            "--disable-vision", "--disable-keyboard-capture", "--disable-clipboard-capture",
            "--included-windows", "ScreenWise Synthetic Privacy Fixture",
        ]
        return args

    def start_phase(self, phase: str, chunk_seconds: int) -> dict[str, Any]:
        if phase not in ("partial", "restart") or chunk_seconds not in (60, 5):
            raise coordinator.ControlError("invalid_tail_phase")
        if self.recorder is not None and self.recorder.poll() is None:
            raise coordinator.ControlError("recorder_already_running")
        if phase == "partial":
            self.data.mkdir(exist_ok=False)
            self._data_marker().write_text(self.record["run_id"], encoding="utf-8")
        else:
            self._validate_owned_data()
            if not (self.data / "db.sqlite").is_file():
                raise coordinator.ControlError("restart_database_missing")
        args = self._argv(chunk_seconds)
        self.journal("command", {"phase": phase, "argv": args})
        launch_started = self.clock()  # Conservative: never later than actual process creation.
        self.recorder = live.windows.OwnedProcess.start(
            args, live.RELEASE, self.data / f"{phase}-stdout.log", self.data / f"{phase}-stderr.log"
        )
        self.phase_name = phase
        self.process_created_at = launch_started
        self.shutdown = None
        self.shutdown_attempts = []
        self.watchdog_done = threading.Event()
        self.phase_watchdog_done = threading.Event()
        self.phase_deadline = launch_started + WATCHDOG_SECONDS
        identity = {"pid": self.recorder.pid, "creation_ticks": self.recorder.creation_time_ticks}
        self.journal("recorder_identity", {"phase": phase, **identity})
        thread = threading.Thread(
            target=self._phase_watchdog,
            args=(self.phase_deadline, identity, self.phase_watchdog_done, self.recorder),
            daemon=True,
        )
        thread.start()
        return {"process_identity": f"{identity['pid']}:{identity['creation_ticks']}", "created_at": launch_started}

    def _phase_watchdog(self, deadline: float, identity: dict[str, int], done: threading.Event,
                        owned_process: Any) -> None:
        remaining = max(0.0, deadline - self.clock())
        if done.wait(remaining):
            return
        with self.stop_lock:
            current = self.recorder
            if current is not owned_process or current.pid != identity["pid"] or current.creation_time_ticks != identity["creation_ticks"]:
                self.journal("watchdog", {"reason": "owned_identity_changed"})
                return
            try:
                self.stop_playback()
                self._stop_current_recorder()
            finally:
                done.set()
                self.journal("watchdog", {"reason": "recording_safety_deadline", "seconds": WATCHDOG_SECONDS})

    def play_once(self, name: str) -> tuple[float, float]:
        if name not in DURATIONS:
            raise coordinator.ControlError("unknown_tail_stimulus")
        if self.assets is None:
            self.assets = verified_speech_assets()
        if self.phase_deadline is None or self.phase_watchdog_done.is_set() or self.recorder is None or self.recorder.poll() is not None:
            raise coordinator.ControlError("tail_phase_not_running")
        if self.phase_deadline - self.clock() <= DURATIONS[name] + PLAYBACK_GUARD_SECONDS:
            raise coordinator.ControlError("tail_playback_budget_insufficient")
        if self.session_state() != "unlocked" or not self.foreground_valid(f"tail-{name}"):
            raise coordinator.ControlError("tail_fixture_foreground_unverified")
        started = self.clock()
        done = threading.Event()
        failed: list[bool] = []

        def worker() -> None:
            try:
                self.player.play(self.assets[name])
            except Exception:
                failed.append(True)
            finally:
                done.set()

        threading.Thread(target=worker, daemon=True).start()
        try:
            while not done.wait(PLAYBACK_CHECK_SECONDS):
                if (self.clock() >= self.phase_deadline or self.phase_watchdog_done.is_set()
                        or self.recorder is None or self.recorder.poll() is not None
                        or self.session_state() != "unlocked"
                        or not self.foreground_valid(f"tail-{name}")):
                    raise coordinator.ControlError("tail_playback_guard_changed")
        finally:
            self.player.stop()
        if failed:
            raise coordinator.ControlError("tail_playback_failed")
        completed = self.clock()
        self.journal("stimulus", {"name": name, "seconds": DURATIONS[name], "repeat": 1})
        return started, completed

    def stop_playback(self):
        self.player.stop()
        return True

    def _stop_current_recorder(self):
        stopped = super().stop_recorder()
        if stopped:
            self.phase_watchdog_done.set()
        return stopped

    def stop_recorder(self):
        with self.stop_lock:
            return self._stop_current_recorder()

    def _verify_owned_identity(self) -> None:
        if self.recorder is None or self.recorder.poll() is not None:
            raise coordinator.ControlError("owned_recorder_not_running")
        inventory = live.windows.inventory_known_executables([live.SCOPED[0]])
        if inventory.get("error_code"):
            raise coordinator.ControlError("owned_identity_inventory_failed")
        if not any(row.get("pid") == self.recorder.pid
                   and row.get("creation_time_ticks") == self.recorder.creation_time_ticks
                   and Path(row.get("executable_path", "")).resolve() == live.SCOPED[0].resolve()
                   for row in inventory.get("processes", [])):
            raise coordinator.ControlError("owned_recorder_identity_unverified")

    def verify_owned_loopback_listener(self) -> bool:
        self._verify_owned_identity()
        result = live.windows.inspect_network_endpoints([self.recorder.pid])
        if result.get("error_code"):
            raise coordinator.ControlError("owned_listener_inspection_failed")
        listeners = [row for row in result.get("endpoints", [])
                     if row.get("protocol") == "tcp" and row.get("state") == "listening"
                     and row.get("local_port") == live.API_PORT]
        if len(listeners) != 1 or listeners[0].get("local_address") != "127.0.0.1":
            raise coordinator.ControlError("owned_loopback_listener_unverified")
        return True

    def protected_request(self, path: str, authenticated: bool = True):
        self.verify_owned_loopback_listener()
        return super().request(path, authenticated=authenticated)

    def request(self, path: str, authenticated: bool = True):
        # Startup health polling is intentionally unauthenticated. Once a bearer
        # is involved, bind every adapter request to the exact owned listener.
        if authenticated:
            self.verify_owned_loopback_listener()
        return super().request(path, authenticated=authenticated)

    def auth_matrix(self) -> dict[str, Any]:
        if not self.token:
            raise coordinator.ControlError("auth_token_unavailable")
        statuses = {
            "missing_status": self.protected_request("/search?content_type=audio&limit=1", authenticated=False)[0],
        }
        saved = self.token
        self.token = "wrong-token"
        try:
            statuses["wrong_status"] = self.protected_request("/search?content_type=audio&limit=1", authenticated=True)[0]
        finally:
            self.token = saved
        statuses["valid_status"] = self.protected_request("/search?content_type=audio&limit=1", authenticated=True)[0]
        return {"base_url": live.API_URL, **statuses, "redirect_followed": False}

    def api_checks(self) -> dict[str, Any]:
        # evidence.verify_api performs its own HTTP calls, so bind its whole
        # sample to an exact listener check immediately before entering it.
        self.verify_owned_loopback_listener()
        result = super().api_checks()
        code, body = self.protected_request("/audio/device/status")
        rows = json.loads(body) if code == 200 else None
        running = [row.get("name") for row in rows if isinstance(row, dict) and row.get("is_running") is True] \
            if isinstance(rows, list) else []
        if running != [live.OUTPUT]:
            raise coordinator.ControlError("exact_audio_device_identity_unverified")
        result["observed_device_identity"] = running[0]
        return result

    def _connect_readonly(self) -> sqlite3.Connection:
        self._validate_owned_data()
        db_path = (self.data / "db.sqlite").resolve()
        if db_path.parent != self.data.resolve() or not db_path.is_file():
            raise coordinator.ControlError("owned_database_missing")
        return sqlite3.connect(f"file:{db_path.as_posix()}?mode=ro", uri=True)

    def db_snapshot(self) -> dict[str, Any]:
        with closing(self._connect_readonly()) as db:
            chunk_count = db.execute("SELECT COUNT(*) FROM audio_chunks").fetchone()[0]
            max_id = db.execute("SELECT COALESCE(MAX(id), 0) FROM audio_chunks").fetchone()[0]
            counts = {}
            for name, forms in MARKER_QUERY_FORMS.items():
                predicate, parameters = _marker_sql("transcription", forms)
                self.journal("synthetic_marker_query", {
                    "surface": "sqlite_snapshot", "marker": name, "query_forms": list(forms),
                })
                counts[name] = db.execute(
                    f"SELECT COUNT(*) FROM audio_transcriptions WHERE {predicate}",
                    parameters,
                ).fetchone()[0]
        return {"audio_chunk_count": int(chunk_count), "max_audio_chunk_id": int(max_id), "marker_counts": counts}

    def safe_phase_log_counts(self, phase: str) -> dict[str, int]:
        if phase not in ("partial", "restart"):
            raise coordinator.ControlError("invalid_log_phase")
        paths = [self.data / f"{phase}-stdout.log", self.data / f"{phase}-stderr.log"]
        if not all(path.is_file() for path in paths):
            raise coordinator.ControlError("owned_phase_log_missing")
        text = "\n".join(path.read_text(encoding="utf-8", errors="replace") for path in paths)
        recovery_phrases = (
            "[DEVICE_RECOVERY]", "restarting recording process",
            "stream rebuild required after screen unlock",
        )
        privacy_phrases = (
            "audio privacy generation changed", "audio acquisition suppressed by privacy policy",
            "audio acquisition resumed after privacy pause", "audio segment discarded after privacy transition",
        )
        return {
            "device_recovery_count": sum(text.count(value) for value in recovery_phrases),
            "privacy_transition_count": sum(text.count(value) for value in privacy_phrases),
        }

    def safe_shutdown_issues(self) -> list[str]:
        aggregate = live.evidence.collect_db(self.data, [])
        if aggregate.get("quick_check") != "ok":
            return ["database_integrity_unverified"]
        categories = aggregate.get("safe_notices", {}).get("category_counts", {})
        reasons = aggregate.get("safe_notices", {}).get("reason_counts", {})
        allowlist = {
            "producer_stop_timeout", "producer_stop_failed", "consumer_drain_timeout",
            "consumer_drain_failed", "queued_work_discarded", "worker_completion_unconfirmed",
        }
        issues = [reason for reason in allowlist if isinstance(reasons.get(reason), int) and not isinstance(reasons.get(reason), bool)
                  and reasons[reason] > 0]
        if any(name not in {"audio_shutdown", "windows_lock", "legacy"} or not isinstance(count, int)
               or isinstance(count, bool) for name, count in categories.items()):
            issues.append("unknown_safe_notice_schema")
        known_non_audio = live.evidence._WINDOWS_LOCK_REASONS | live.evidence._LEGACY_NOTICE_REASONS
        if any(name not in allowlist | known_non_audio
               or not isinstance(count, int) or isinstance(count, bool) for name, count in reasons.items()):
            issues.append("unknown_safe_notice_schema")
        if any(name != "wts_session_unlocked" and isinstance(count, int) and count > 0
               for name, count in reasons.items() if name in known_non_audio):
            issues.append("privacy_or_delivery_notice_observed")
        return sorted(set(issues))

    def capture_status(self) -> dict[str, Any]:
        query = urlencode({"start_time": self.started, "end_time": live.utc(), "limit": 1000})
        code, body = self.protected_request("/capture-events?" + query)
        if code != 200:
            raise coordinator.ControlError("capture_status_unavailable")
        value = json.loads(body)
        expected = {"data", "has_more", "persistence_degraded", "event_delivery",
                    "audio_shutdown_degraded", "audio_shutdown_issues"}
        if not isinstance(value, dict) or not expected.issubset(value):
            raise coordinator.ControlError("capture_status_schema_invalid")
        delivery = value["event_delivery"]
        shutdown = value["audio_shutdown_issues"]
        allowed = {
            "producer_stop_timeout", "producer_stop_failed", "consumer_drain_timeout",
            "consumer_drain_failed", "queued_work_discarded", "worker_completion_unconfirmed",
        }
        if (not isinstance(value["persistence_degraded"], bool)
                or not isinstance(value["audio_shutdown_degraded"], bool)
                or not isinstance(delivery, dict) or set(delivery) != {"near_capacity", "dropped_events"}
                or not isinstance(delivery["near_capacity"], bool)
                or not isinstance(delivery["dropped_events"], int) or isinstance(delivery["dropped_events"], bool)
                or delivery["dropped_events"] < 0
                or not isinstance(shutdown, list) or any(item not in allowed for item in shutdown)):
            raise coordinator.ControlError("capture_status_schema_invalid")
        issues = list(shutdown)
        if value["persistence_degraded"]:
            issues.append("persistence_degraded")
        if delivery["near_capacity"]:
            issues.append("event_delivery_near_capacity")
        if delivery["dropped_events"]:
            issues.append("event_delivery_loss")
        if value["audio_shutdown_degraded"] != bool(shutdown):
            issues.append("audio_shutdown_status_inconsistent")
        api_audio_reasons = {"audio_" + reason for reason in {
            "producer_stop_timeout", "producer_stop_failed", "consumer_drain_timeout",
            "consumer_drain_failed", "queued_work_discarded", "worker_completion_unconfirmed",
        }}
        known_api_reasons = live.evidence._WINDOWS_LOCK_REASONS | api_audio_reasons
        if value["has_more"] is not False:
            raise coordinator.ControlError("capture_status_truncated")
        if (not isinstance(value["data"], list)
                or any(not isinstance(item, dict) or item.get("reason_code") not in known_api_reasons
                       for item in value["data"])):
            raise coordinator.ControlError("capture_status_schema_invalid")
        transitions = sum(item.get("reason_code") != "wts_session_unlocked" for item in value["data"])
        return {"issues": sorted(set(issues)), "privacy_transition_count": transitions}

    def one_shutdown_chunk(self, prior_max_id: int) -> dict[str, Any]:
        if self.recorder is None or self.recorder.poll() is None:
            raise coordinator.ControlError("post_exit_evidence_requires_stopped_recorder")
        with closing(self._connect_readonly()) as db:
            rows = db.execute("SELECT id, file_path FROM audio_chunks WHERE id > ? ORDER BY id", (prior_max_id,)).fetchall()
            total = db.execute("SELECT COUNT(*) FROM audio_chunks").fetchone()[0]
            if len(rows) != 1:
                return {"new_chunk_count": len(rows), "total_audio_chunk_count": int(total)}
            chunk_id, raw_path = rows[0]
            marker_counts: dict[str, int] = {}
            for name in ("baseline", "tail"):
                forms = MARKER_QUERY_FORMS[name]
                predicate, parameters = _marker_sql("transcription", forms)
                self.journal("synthetic_marker_query", {
                    "surface": "sqlite_shutdown_chunk", "marker": name,
                    "query_forms": list(forms), "audio_chunk_id": int(chunk_id),
                })
                marker_counts[name] = db.execute(
                    f"SELECT COUNT(*) FROM audio_transcriptions WHERE audio_chunk_id = ? AND {predicate}",
                    (chunk_id, *parameters),
                ).fetchone()[0]
            columns = {row[1] for row in db.execute("PRAGMA table_info(audio_transcriptions)")}
            if not {"device", "is_input_device"}.issubset(columns):
                raise coordinator.ControlError("transcription_device_schema_missing")
            expected_suffix = " (output)"
            if not live.OUTPUT.endswith(expected_suffix):
                raise coordinator.ControlError("selected_output_identity_invalid")
            expected_name = live.OUTPUT.removesuffix(expected_suffix)
            devices = db.execute(
                "SELECT DISTINCT device, is_input_device FROM audio_transcriptions WHERE audio_chunk_id = ?",
                (chunk_id,),
            ).fetchall()
            if devices != [(expected_name, 0)]:
                raise coordinator.ControlError("shutdown_chunk_device_mismatch")
            observed_device_identity = f"{devices[0][0]} (output)"
            if observed_device_identity != live.OUTPUT:
                raise coordinator.ControlError("shutdown_chunk_device_mismatch")
        media = Path(raw_path).resolve()
        if not media.is_relative_to(self.data.resolve()) or not media.is_file():
            raise coordinator.ControlError("shutdown_media_outside_owned_data")
        duration = self._ffprobe_duration(media)
        return {
            "new_chunk_count": 1, "total_audio_chunk_count": int(total), "chunk_id": int(chunk_id),
            "audio_file_exists": True, "duration_source": "ffprobe", "duration_seconds": duration,
            "baseline_marker": MARKERS["baseline"], "tail_marker": MARKERS["tail"],
            "baseline_count_same_chunk": int(marker_counts["baseline"]),
            "tail_count_same_chunk": int(marker_counts["tail"]),
            "baseline_audio_chunk_id": int(chunk_id), "tail_audio_chunk_id": int(chunk_id),
            "device_identity": observed_device_identity,
        }

    def _ffprobe_duration(self, media: Path) -> float:
        probe = live.SCOPED[2].resolve()
        completed = subprocess.run(
            [str(probe), "-v", "error", "-show_entries", "format=duration", "-of", "json", str(media)],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            timeout=10, check=False, creationflags=subprocess.CREATE_NO_WINDOW,
        )
        if completed.returncode:
            raise coordinator.ControlError("ffprobe_duration_failed")
        value = json.loads(completed.stdout.decode("utf-8"))["format"]["duration"]
        duration = float(value)
        if not _finite_number(duration):
            raise coordinator.ControlError("ffprobe_duration_invalid")
        return duration

    def search_chunk_ids(self, marker: str) -> tuple[int, list[int]]:
        name, forms = _marker_name_and_forms(marker)
        self.journal("synthetic_marker_query", {
            "surface": "authenticated_search", "marker": name, "query_forms": list(forms),
        })
        ids: list[int] = []
        seen: set[int] = set()
        for form in forms:
            code, body = self.protected_request(
                "/search?" + urlencode({"content_type": "audio", "q": form, "limit": 20})
            )
            if code != 200:
                return code, []
            value = json.loads(body)
            rows = value.get("data") if isinstance(value, dict) else None
            if not isinstance(rows, list):
                raise coordinator.ControlError("search_schema_invalid")
            for row in rows:
                if not isinstance(row, dict):
                    continue
                content = row.get("content")
                if row.get("type") != "Audio" or not isinstance(content, dict):
                    continue
                candidate = content.get("chunk_id")
                if isinstance(candidate, int) and not isinstance(candidate, bool) and candidate not in seen:
                    seen.add(candidate)
                    ids.append(candidate)
        return 200, ids


class TailCollector60s:
    def __init__(self, backend: Any, readiness: dict[str, Any], *, clock: Callable[[], float] = time.monotonic):
        self.backend = backend
        self.clock = clock
        required = {"explicit_owner_ready": True, "nonce_match": True,
                    "expires_at": None, "deadline_enforced": False}
        if readiness != required:
            raise coordinator.ControlError("consumed_readiness_evidence_required")
        self.readiness = dict(readiness)

    def run(self) -> dict[str, Any]:
        evidence: dict[str, Any] = {
            "evidence_version": 1,
            "data_store_id": f"run:{self.backend.record['run_id']}",
            "markers": dict(MARKERS),
            "readiness": self.readiness,
            "config": {"chunk_seconds": 60.0, "overlap_seconds": 2.0, "normal_emission_seconds": 62.0, "watchdog_seconds": WATCHDOG_SECONDS},
        }
        store_id = evidence["data_store_id"]
        self.backend.start_fixture()
        first = self.backend.start_phase("partial", 60)
        self.backend.await_api()
        api = self.backend.api_checks()
        if api.get("passed") is not True or api.get("capture_handle_ready") is not True:
            raise coordinator.ControlError("partial_capture_not_ready")
        self.backend.verify_owned_loopback_listener()
        ready = self.clock()
        evidence["source"] = {
            "single_selected_device": api.get("exact_audio_selection") is True,
            "device_identity": api.get("observed_device_identity"), "capture_ready": True,
        }
        baseline_start, baseline_end = self.backend.play_once("baseline")
        tail_start, tail_end = self.backend.play_once("tail")
        pre_time = self.clock()
        pre = self.backend.db_snapshot()
        evidence["pre_stop"] = {
            "data_store_id": store_id, "audio_chunk_count": pre["audio_chunk_count"],
            "baseline_db_count": pre["marker_counts"]["baseline"],
            "tail_db_count": pre["marker_counts"]["tail"],
            "max_audio_chunk_id": pre["max_audio_chunk_id"],
        }
        self.backend.journal("evidence_checkpoint", {"name": "pre_stop", **evidence["pre_stop"]})
        evidence["api_auth_before_stop"] = self.backend.auth_matrix()
        self.backend.journal("evidence_checkpoint", {
            "name": "api_auth_before_stop", **evidence["api_auth_before_stop"],
        })
        first_status = self.backend.capture_status()
        stop_time = self.clock()
        stopped = self.backend.stop_recorder()
        exit_time = self.clock()
        first_log_counts = self.backend.safe_phase_log_counts("partial")
        evidence["source"].update(first_log_counts)
        first_issues = sorted(set(first_status["issues"] + self.backend.safe_shutdown_issues()))
        evidence["source"]["privacy_transition_count"] += first_status["privacy_transition_count"]
        evidence["timing"] = {
            "process_created_at": first["created_at"], "capture_ready_at": ready,
            "baseline_started_at": baseline_start, "baseline_completed_at": baseline_end,
            "tail_started_at": tail_start, "tail_completed_at": tail_end,
            "pre_stop_snapshot_at": pre_time, "stop_requested_at": stop_time,
            "process_exited_at": exit_time,
        }
        evidence["first_shutdown"] = {
            "graceful_requested": True, "process_exited": stopped,
            "exit_code": self.backend.shutdown.get("exit_code") if self.backend.shutdown else None,
            "forced": bool(self.backend.shutdown and self.backend.shutdown.get("forced")),
            "unresolved_workers": ["worker_completion_unconfirmed"] if "worker_completion_unconfirmed" in first_issues else [],
            "shutdown_issues": first_issues,
            "process_identity": first["process_identity"],
        }
        self.backend.journal("evidence_checkpoint", {
            "name": "first_shutdown", **evidence["first_shutdown"],
        })
        if not stopped or not self.backend.clean_shutdown() or first_issues:
            result = evaluate(evidence)
            return {"status": result["status"], "reasons": result["reasons"], "evidence": evidence,
                    "collector_live_verified": False, "target_stop_seconds": TARGET_STOP_SECONDS}
        post = self.backend.one_shutdown_chunk(pre["max_audio_chunk_id"])
        evidence["post_stop"] = {
            "data_store_id": store_id, **post,
        }

        second = self.backend.start_phase("restart", 5)
        self.backend.await_api()
        restart_api = self.backend.api_checks()
        if (restart_api.get("passed") is not True or restart_api.get("capture_handle_ready") is not True
                or restart_api.get("exact_audio_selection") is not True):
            raise coordinator.ControlError("restart_capture_not_ready")
        self.backend.verify_owned_loopback_listener()
        restart_ready = self.clock()
        baseline_code, baseline_ids = self.backend.search_chunk_ids(MARKERS["baseline"])
        tail_code, tail_ids = self.backend.search_chunk_ids(MARKERS["tail"])
        control_start, control_end = self.backend.play_once("restart")
        deadline = self.clock() + 15.0
        control_ids: list[int] = []
        while self.clock() < deadline:
            _, control_ids = self.backend.search_chunk_ids(MARKERS["restart"])
            if control_ids:
                break
            self.backend.sleeper(0.25)
        observed = self.clock()
        second_snapshot = self.backend.db_snapshot()
        restart_auth = self.backend.auth_matrix()
        self.backend.journal("evidence_checkpoint", {"name": "restart_api_auth", **restart_auth})
        evidence["restart"] = {
            "kind": "recorder_process", "data_store_id": store_id,
            "process_identity": second["process_identity"], "capture_ready": True,
            "chunk_seconds": 5.0,
            "baseline_query_marker": MARKERS["baseline"], "tail_query_marker": MARKERS["tail"],
            "baseline_search_count": len(baseline_ids) if baseline_code == 200 else 0,
            "tail_search_count": len(tail_ids) if tail_code == 200 else 0,
            "baseline_search_chunk_id": post.get("chunk_id") if post.get("chunk_id") in baseline_ids else None,
            "tail_search_chunk_id": post.get("chunk_id") if post.get("chunk_id") in tail_ids else None,
            "control_query_marker": MARKERS["restart"],
            "control_db_count": second_snapshot["marker_counts"]["restart"],
            "control_search_count": len(control_ids),
            "control_audio_chunk_id": next((item for item in control_ids if item != post.get("chunk_id")), None),
            "device_recovery_count": 0, "privacy_transition_count": 0,
            "process_created_at": second["created_at"], "capture_ready_at": restart_ready,
            "control_started_at": control_start, "control_completed_at": control_end,
            "control_observed_at": observed, "api_auth": restart_auth,
        }
        restart_status = self.backend.capture_status()
        final_stop = self.clock()
        final_stopped = self.backend.stop_recorder()
        final_exit = self.clock()
        restart_log_counts = self.backend.safe_phase_log_counts("restart")
        evidence["restart"]["device_recovery_count"] = restart_log_counts["device_recovery_count"]
        evidence["restart"]["privacy_transition_count"] = (restart_log_counts["privacy_transition_count"]
                                                               + restart_status["privacy_transition_count"])
        all_issues = sorted(set(restart_status["issues"] + self.backend.safe_shutdown_issues()))
        # The first attempt was already classified. A recurrence cannot be attributed
        # by row timestamp safely here, so any latched issue also blocks final shutdown.
        final_issues = all_issues
        evidence["final_shutdown"] = {
            "process_identity": second["process_identity"], "graceful_requested": True,
            "process_exited": final_stopped,
            "exit_code": self.backend.shutdown.get("exit_code") if self.backend.shutdown else None,
            "forced": bool(self.backend.shutdown and self.backend.shutdown.get("forced")),
            "unresolved_workers": ["worker_completion_unconfirmed"] if "worker_completion_unconfirmed" in final_issues else [],
            "shutdown_issues": final_issues,
            "stop_requested_at": final_stop, "process_exited_at": final_exit,
        }
        self.backend.journal("evidence_checkpoint", {
            "name": "final_shutdown", **evidence["final_shutdown"],
        })
        result = evaluate(evidence)
        return {"status": result["status"], "reasons": result["reasons"], "evidence": evidence,
                "collector_live_verified": False, "target_stop_seconds": TARGET_STOP_SECONDS}


def preview(run_id: str) -> dict[str, Any]:
    path = coordinator.session_path(run_id, CONTROLLER)
    record = _load_json(path / "waiting.json")
    return {
        "run_id": run_id, "mode": record.get("mode"), "state": record.get("state"),
        "execution": "preview_only", "collector_live_verified": False,
        "requires": ["--execute-interactive", "fresh owner nonce", "reviewed refreshed binary pin",
                     "reviewed collector-pins.json"],
        "watchdog_seconds": WATCHDOG_SECONDS, "readiness_timeout_seconds": None,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--execute-interactive", action="store_true")
    parser.add_argument("--owner-ready")
    args = parser.parse_args(argv)
    if not args.execute_interactive:
        print(json.dumps(preview(args.run_id), indent=2))
        return 0
    live.require_runtime_configuration()
    path = coordinator.session_path(args.run_id, CONTROLLER)
    record = _load_json(path / "waiting.json")
    if record.get("mode") != "audio-output":
        raise coordinator.ControlError("tail_collector_requires_audio_output_gate")
    live.verify_prepared_pins()
    verify_collector_pins()
    backend = TailWindowsBackend(record, path)
    backend.quiescent()
    consumed = coordinator.consume_confirmation(
        args.run_id, args.owner_ready or "", recorder_stopped=True,
        fixtures_hidden=True, root=CONTROLLER,
    )
    if consumed.get("run_id") != record.get("run_id"):
        raise coordinator.ControlError("consumed_gate_identity_mismatch")
    readiness = {"explicit_owner_ready": True, "nonce_match": True,
                 "expires_at": None, "deadline_enforced": False}
    result: dict[str, Any]
    try:
        backend.preflight()
        result = TailCollector60s(backend, readiness).run()
    except Exception:
        result = {"status": "incomplete", "reasons": ["collector_execution_failed"],
                  "collector_live_verified": False}
    finally:
        cleanup_issues: list[str] = []
        try:
            if backend.stop_playback() is not True:
                cleanup_issues.append("playback_stop_unconfirmed")
        except Exception:
            cleanup_issues.append("playback_stop_unconfirmed")
        try:
            if backend.recorder is not None and backend.recorder.poll() is None:
                if backend.stop_recorder() is not True:
                    cleanup_issues.append("recorder_stop_unconfirmed")
        except Exception:
            cleanup_issues.append("recorder_stop_unconfirmed")
        try:
            if backend.close_fixture() is not True:
                cleanup_issues.append("fixture_close_unconfirmed")
        except Exception:
            cleanup_issues.append("fixture_close_unconfirmed")
        try:
            if backend.processes_stopped() is not True:
                cleanup_issues.append("owned_processes_remain")
        except Exception:
            cleanup_issues.append("owned_processes_remain")
        if cleanup_issues:
            try:
                previous = result.get("reasons", []) if isinstance(result.get("reasons"), list) else []
            except Exception:
                previous = []
            result = {"status": "incomplete", "reasons": list(dict.fromkeys(previous + cleanup_issues)),
                      "collector_live_verified": False}
    # TailCollector60s.run() is also exercised with mocked backends, so it must
    # never self-claim live verification. Only this explicit Windows execution
    # path can add live provenance, and only after cleanup has completed.
    result["execution"] = "live"
    result["collector_live_verified"] = result.get("status") == "pass"
    coordinator.exclusive_json(path / "tail-result.json", result)
    print(json.dumps({"status": result["status"], "reasons": result["reasons"]}, indent=2))
    return 0 if result["status"] == "pass" else 2


if __name__ == "__main__":
    raise SystemExit(main())
