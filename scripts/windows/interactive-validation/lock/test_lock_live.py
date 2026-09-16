from __future__ import annotations

import hashlib
import io
import json
import sqlite3
import tempfile
import threading
import time
import unittest
from contextlib import closing
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock
from urllib.parse import parse_qs, urlsplit

import lock_live as subject


class FakeOwned:
    def __init__(self):
        self.pid = 4123
        self.creation_time_ticks = 998877
        self.code = None

    def poll(self):
        return self.code


class FakeClock:
    def __init__(self, value=10.0):
        self.value = value

    def __call__(self):
        return self.value

    def sleep(self, seconds):
        self.value += seconds


def make_backend(root: Path, clock=None):
    path = root / "session"
    path.mkdir()
    return subject.LockWindowsBackend(
        {"run_id": "lock-unit", "mode": "lock"}, path,
        clock=clock or time.monotonic,
        sleeper=(clock.sleep if isinstance(clock, FakeClock) else time.sleep),
    )


def create_current_db(backend, generation=3):
    backend.data.mkdir()
    backend._data_marker().write_text(backend.record["run_id"], encoding="utf-8")
    marker = f"{subject.MARKER_PREFIX}{generation}"
    db = sqlite3.connect(backend.data / "db.sqlite")
    db.executescript("""
        CREATE TABLE frames (
          id INTEGER PRIMARY KEY,
          accessibility_text TEXT,
          accessibility_tree_json TEXT,
          full_text TEXT
        );
        CREATE TABLE ui_events (event_type TEXT, text_content TEXT);
        CREATE TABLE audio_chunks (id INTEGER);
        CREATE TABLE audio_transcriptions (id INTEGER);
    """)
    db.execute("INSERT INTO frames VALUES (1, ?, ?, ?)",
               (marker, json.dumps({"synthetic": marker}), marker))
    db.execute("INSERT INTO ui_events VALUES ('privacy_notice', ?)",
               (json.dumps({"reason": "wts_session_unlocked"}),))
    db.commit()
    db.close()


class PinAndPreviewTests(unittest.TestCase):
    def test_exact_new_pin_set_is_verified(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            assets = {}
            for name in subject.PIN_REQUIRED:
                (root / name).write_text(name, encoding="utf-8")
                assets[name] = hashlib.sha256(name.encode()).hexdigest().upper()
            manifest = root / "pins.json"
            manifest.write_text(json.dumps({
                "schema": "screenwise.lock-transition-pins.v1", "assets": assets,
            }), encoding="utf-8")
            result = subject.verify_pin_manifest(
                manifest, root, "screenwise.lock-transition-pins.v1",
                required=subject.PIN_REQUIRED,
            )
            self.assertEqual(result["asset_count"], 5)

    def test_pin_escape_and_changed_asset_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            outside = root.parent / "outside-lock-test.txt"
            outside.write_text("x", encoding="utf-8")
            self.addCleanup(lambda: outside.unlink(missing_ok=True))
            manifest = root / "pins.json"
            manifest.write_text(json.dumps({
                "schema": "x", "assets": {"../outside-lock-test.txt": "0" * 64},
            }), encoding="utf-8")
            with self.assertRaises(subject.coordinator.ControlError):
                subject.verify_pin_manifest(manifest, root, "x")

    def test_preview_never_verifies_pins_or_touches_live_path(self):
        output = io.StringIO()
        with mock.patch.object(subject, "verify_all_pins", side_effect=AssertionError("live")):
            with redirect_stdout(output):
                code = subject.main(["--run-id", "preview-only"])
        self.assertEqual(code, 0)
        self.assertFalse(json.loads(output.getvalue())["execution"])


class BackendSchemaTests(unittest.TestCase):
    def test_current_frame_schema_supplies_aggregate_control_and_plateau_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = make_backend(Path(tmp))
            create_current_db(backend)
            backend.token = "sp-test"
            backend.protected_request = lambda *_args, **_kwargs: (
                200, json.dumps({"data": [{"type": "Vision"}]}).encode())
            hits = backend.control_hits(3)
            self.assertEqual(hits, {
                "frame_hits": 1, "uia_hits": 2, "authenticated_search_hits": 1,
            })
            self.assertEqual(backend.capture_counts(), {
                "frame_rows": 1, "uia_capture_rows": 1, "privacy_notice_rows": 1,
            })

    def test_control_search_changes_time_scope_to_bypass_stale_empty_cache(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = make_backend(Path(tmp))
            create_current_db(backend)
            backend.token = "sp-test"
            routes = []

            def request(route, **_kwargs):
                routes.append(route)
                return 200, json.dumps({"data": [{"type": "Vision"}]}).encode()

            backend.protected_request = request
            with mock.patch.object(subject, "utc_now", side_effect=(
                    "2026-09-16T00:00:01+00:00", "2026-09-16T00:00:02+00:00")):
                backend.control_hits(3)
                backend.control_hits(3)
            first, second = (parse_qs(urlsplit(route).query) for route in routes)
            self.assertEqual(first["content_type"], ["all"])
            self.assertEqual(first["start_time"], [backend.run_start_utc])
            self.assertNotEqual(first["end_time"], second["end_time"])
            self.assertNotEqual(routes[0], routes[1])

    def test_silence_requires_no_running_device_or_audio_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = make_backend(Path(tmp))
            create_current_db(backend)
            backend.protected_request = lambda *_args, **_kwargs: (200, b"[]")
            self.assertEqual(backend.silence_snapshot()["running_device_count"], 0)
            with closing(sqlite3.connect(backend.data / "db.sqlite")) as db:
                db.execute("INSERT INTO audio_chunks VALUES (1)")
                db.commit()
            with self.assertRaises(subject.coordinator.ControlError):
                backend.silence_snapshot()

    def test_generation_counts_only_matched_successful_plain_acknowledgements(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = make_backend(Path(tmp))
            commands = backend.fixture_dir / "commands"
            acks = backend.fixture_dir / "acks"
            commands.mkdir(parents=True)
            acks.mkdir()
            for number, success, action in ((1, True, "plain"), (2, True, "show"), (3, False, "plain")):
                phase = f"p{number}"
                (commands / f"{phase}.json").write_text(json.dumps({
                    "runId": "lock-unit", "phaseId": phase, "action": action,
                }), encoding="utf-8")
                (acks / f"{phase}.json").write_text(json.dumps({
                    "runId": "lock-unit", "phaseId": phase, "action": action,
                    "success": success,
                }), encoding="utf-8")
            self.assertEqual(backend.accepted_plain_generation(), 1)

    def test_log_evidence_rejects_known_code_with_wrong_message(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = make_backend(Path(tmp))
            backend.data.mkdir()
            good = (
                "2026-01-01T00:00:00.000000Z INFO screenpipe_engine::sleep_monitor: "
                f"{subject.LOCKED_MESSAGE} reason_code=\"wts_session_locked\"\n"
            )
            (backend.data / "stdout.log").write_text(good, encoding="utf-8")
            (backend.data / "stderr.log").write_text("", encoding="utf-8")
            notices = [{"reason_code": "wts_session_locked"}]
            self.assertTrue(backend.safe_lock_log_evidence(notices)["safe_notice_lines_verified"])
            (backend.data / "stderr.log").write_text(
                'WARN screenpipe_engine::sleep_monitor: wrong reason_code="wts_session_locked"\n',
                encoding="utf-8")
            self.assertFalse(backend.safe_lock_log_evidence(notices)["safe_notice_lines_verified"])

    def test_post_shutdown_schema_check_rejects_non_allowlisted_notice(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = make_backend(Path(tmp))
            create_current_db(backend)
            (backend.data / "stdout.log").write_text("", encoding="utf-8")
            (backend.data / "stderr.log").write_text("", encoding="utf-8")
            self.assertEqual(backend.post_shutdown_issues(), [])
            with closing(sqlite3.connect(backend.data / "db.sqlite")) as db:
                db.execute("INSERT INTO ui_events VALUES ('privacy_notice', ?)",
                           (json.dumps({"issue": "unexpected"}),))
                db.commit()
            self.assertIn("unknown_safe_notice_schema", backend.post_shutdown_issues())


class WrapperAndArgvTests(unittest.TestCase):
    def test_stop_wrapper_uses_real_reentrant_lock_and_serializes_callers(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = make_backend(Path(tmp))
            self.assertIs(type(backend.stop_lock), type(threading.RLock()))
            state = {"active": 0, "maximum": 0}
            state_lock = threading.Lock()

            def stop_once():
                with state_lock:
                    state["active"] += 1
                    state["maximum"] = max(state["maximum"], state["active"])
                time.sleep(0.01)
                with state_lock:
                    state["active"] -= 1
                return True

            backend._stop_current_recorder = stop_once
            threads = [threading.Thread(target=backend.stop_recorder) for _ in range(2)]
            for thread in threads:
                thread.start()
            for thread in threads:
                thread.join()
            self.assertEqual(state["maximum"], 1)

    def test_recorder_argv_has_one_of_each_silent_flag(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = make_backend(Path(tmp))
            captured = {}
            owned = FakeOwned()

            def start(argv, *_args, **_kwargs):
                captured["argv"] = list(argv)
                return owned

            with mock.patch.object(subject.live.windows.OwnedProcess, "start", side_effect=start):
                backend.start_recorder()
            backend.watchdog_done.set()
            argv = captured["argv"]
            for flag in ("--disable-audio", "--disable-keyboard-capture",
                         "--disable-clipboard-capture", "--disable-meeting-detector",
                         "--disable-snapshot-compaction"):
                self.assertEqual(argv.count(flag), 1)
            index = argv.index("--included-windows")
            self.assertEqual(argv[index + 1], "ScreenWise Synthetic Privacy Fixture")
            self.assertEqual(argv[argv.index("--port") + 1], "31479")


class FakeBackend:
    def __init__(self, root: Path, clock: FakeClock):
        self.record = {"run_id": "lock-fake"}
        self.path = root
        self.clock = clock
        self.active_deadline = None
        self.watchdog_fired = threading.Event()
        self.recorder = FakeOwned()
        self.fixture = FakeOwned()
        self.fixture.pid = 5123
        self.shutdown = None
        self.shutdown_attempts = []
        self.run_start_utc = "2026-09-16T00:00:00+00:00"
        self.run_start_monotonic_ms = int(clock() * 1000)
        self.cleaned = []

    def preflight(self): pass
    def session_state(self): return "unlocked" if self.clock() < 11 or self.clock() >= 28 else "locked"
    def start_fixture(self): pass
    def accepted_plain_generation(self): return 1
    def start_recorder(self): self.active_deadline = self.clock() + 180
    def await_api(self): pass
    def verify_owned_loopback_listener(self): return True
    def control_hits(self, generation):
        return {"frame_hits": 1, "uia_hits": 1, "authenticated_search_hits": 1}
    def foreground_valid(self, _phase): return True
    def normalized_auth(self, checkpoint, observed):
        return {"checkpoint": checkpoint, "observed_ms": observed, "bind_host": "127.0.0.1",
                "port": 31479, "endpoint": "/search", "content_type": "all",
                "missing": 403, "wrong": 403, "valid": 200}
    def silence_snapshot(self):
        return {"running_device_count": 0, "audio_chunk_rows": 0, "audio_transcription_rows": 0}
    def process_observation(self, checkpoint, observed):
        return {"checkpoint": checkpoint, "pid": self.recorder.pid,
                "creation_time": self.recorder.creation_time_ticks, "alive": True,
                "observed_ms": observed}
    def release_focus(self): return True
    def capture_counts(self):
        return {"frame_rows": 4, "uia_capture_rows": 4, "privacy_notice_rows": 2}
    def post_unlock_plain_once(self): self.clock.sleep(0.1); return 2
    def wait_fixture_focus(self, _deadline): pass
    def capture_notices(self):
        return ([
            {"typed": True, "reason_code": "wts_session_locked",
             "message": subject.LOCKED_MESSAGE, "timestamp_ms": 12000},
            {"typed": True, "reason_code": "wts_session_unlocked",
             "message": subject.UNLOCKED_MESSAGE, "timestamp_ms": 28000},
        ], [])
    def stop_recorder(self):
        self.recorder.code = 0
        self.shutdown = {"exit_code": 0, "forced": False}
        self.shutdown_attempts.append(dict(self.shutdown))
        self.cleaned.append("recorder")
        return True
    def post_shutdown_issues(self): return []
    def safe_lock_log_evidence(self, _notices):
        return {"safe_notice_lines_verified": True,
                "reason_code_counts": {"wts_session_locked": 1, "wts_session_unlocked": 1}}
    def close_fixture(self): self.cleaned.append("fixture"); return True
    def processes_stopped(self): self.cleaned.append("inventory"); return True


class ControllerFlowTests(unittest.TestCase):
    def test_unknown_wts_observation_is_appended_before_policy_branch(self):
        with tempfile.TemporaryDirectory() as tmp:
            clock = FakeClock()
            backend = FakeBackend(Path(tmp), clock)
            controller = subject.LockTransitionController(
                backend, {"fresh": True, "waited_indefinitely": True,
                          "phase_id": "lock-fake"},
                clock=clock, sleeper=clock.sleep, evaluator=lambda _value: {})
            evidence = {"wts_samples": []}
            with mock.patch.object(subject.live.windows, "session_state", return_value={
                    "state": "unknown", "error_code": "wts_query_failed"}):
                self.assertEqual(controller.sample_wts(evidence), "unknown")
            self.assertEqual(evidence["wts_samples"][0]["state"], "unknown")
            self.assertEqual(evidence["wts_samples"][0]["error_code"], "wts_query_failed")

    def test_mocked_flow_uses_real_state_machine_and_cleans_up_in_order(self):
        with tempfile.TemporaryDirectory() as tmp:
            clock = FakeClock()
            backend = FakeBackend(Path(tmp), clock)
            readiness = {"fresh": True, "waited_indefinitely": True, "phase_id": "lock-fake"}
            evaluator = lambda evidence: {
                "schema": "screenwise.lock-transition-acceptance.v1", "status": "pass",
                "reasons": [], "eligible": True,
            }

            def state():
                return {"state": backend.session_state(), "error_code": None}

            output = io.StringIO()
            with mock.patch.object(subject.live.windows, "session_state", side_effect=state):
                with redirect_stdout(output):
                    result = subject.LockTransitionController(
                        backend, readiness, clock=clock, sleeper=clock.sleep,
                        evaluator=evaluator,
                    ).run()
            self.assertEqual(result["decision"]["status"], "pass")
            self.assertEqual(backend.cleaned, ["recorder", "fixture", "inventory"])
            states = [row["state"] for row in result["evidence"]["wts_samples"]]
            collapsed = [state for index, state in enumerate(states)
                         if index == 0 or state != states[index - 1]]
            self.assertEqual(collapsed, ["unlocked", "locked", "unlocked"])
            times = [row["observed_ms"] for row in result["evidence"]["wts_samples"]]
            self.assertEqual(times, sorted(set(times)))
            phases = [json.loads(line)["phase"] for line in output.getvalue().splitlines()]
            self.assertEqual(phases, ["ready_for_lock", "locked_observation_complete",
                                      "unlocked_waiting_fixture_focus", "finished"])

    def test_emitted_evidence_passes_real_acceptance_contract(self):
        with tempfile.TemporaryDirectory() as tmp:
            clock = FakeClock()
            backend = FakeBackend(Path(tmp), clock)
            readiness = {"fresh": True, "waited_indefinitely": True, "phase_id": "lock-fake"}

            def state():
                return {"state": backend.session_state(), "error_code": None}

            with mock.patch.object(subject.live.windows, "session_state", side_effect=state):
                with redirect_stdout(io.StringIO()):
                    result = subject.LockTransitionController(
                        backend, readiness, clock=clock, sleeper=clock.sleep,
                        evaluator=subject.load_evaluator(),
                    ).run()
            self.assertEqual(result["decision"]["status"], "pass", result["decision"])
            self.assertTrue(result["decision"]["eligible"])
            self.assertEqual(result["controller_issues"], [])

    def test_evaluator_exception_keeps_aggregate_checkpoint_and_safe_result(self):
        with tempfile.TemporaryDirectory() as tmp:
            clock = FakeClock()
            backend = FakeBackend(Path(tmp), clock)
            readiness = {"fresh": True, "waited_indefinitely": True,
                         "phase_id": "lock-fake"}
            secret = "never-copy-evaluator-private-text"

            def state():
                return {"state": backend.session_state(), "error_code": None}

            def evaluator(_evidence):
                raise IndexError(secret)

            with mock.patch.object(subject.live.windows, "session_state", side_effect=state):
                with redirect_stdout(io.StringIO()):
                    result = subject.LockTransitionController(
                        backend, readiness, clock=clock, sleeper=clock.sleep,
                        evaluator=evaluator,
                    ).run()

            checkpoint_path = Path(tmp) / "lock-evidence.json"
            self.assertTrue(checkpoint_path.is_file())
            checkpoint = json.loads(checkpoint_path.read_text(encoding="utf-8"))
            self.assertEqual(checkpoint["stage"], "evaluation_failed")
            self.assertEqual(checkpoint["evidence"], result["evidence"])
            self.assertEqual(result["decision"]["status"], "incomplete")
            self.assertFalse(result["decision"]["eligible"])
            self.assertEqual(
                result["evaluator_error"],
                {"code": "evaluator_exception", "class": "index_error"},
            )
            self.assertNotIn(secret, json.dumps(result))
            self.assertNotIn(secret, checkpoint_path.read_text(encoding="utf-8"))

    def test_partial_post_unlock_controller_failure_is_persisted_and_total(self):
        with tempfile.TemporaryDirectory() as tmp:
            clock = FakeClock()
            backend = FakeBackend(Path(tmp), clock)
            readiness = {"fresh": True, "waited_indefinitely": True,
                         "phase_id": "lock-fake"}

            def state():
                return {"state": backend.session_state(), "error_code": None}

            def fail_focus(_deadline):
                raise subject.coordinator.ControlError("fixture_focus_timeout")

            backend.wait_fixture_focus = fail_focus
            with mock.patch.object(subject.live.windows, "session_state", side_effect=state):
                with redirect_stdout(io.StringIO()):
                    result = subject.LockTransitionController(
                        backend, readiness, clock=clock, sleeper=clock.sleep,
                        evaluator=subject.load_evaluator(),
                    ).run()

            self.assertEqual(len(result["evidence"]["process"]["observations"]), 2)
            self.assertEqual(result["decision"]["status"], "incomplete")
            self.assertIn("process_liveness_unconfirmed", result["decision"]["reasons"])
            self.assertEqual(
                result["controller_error"],
                {"code": "fixture_focus_timeout", "class": "control_error"},
            )
            checkpoint = json.loads(
                (Path(tmp) / "lock-evidence.json").read_text(encoding="utf-8")
            )
            self.assertEqual(checkpoint["stage"], "cleanup_complete")
            self.assertEqual(len(checkpoint["evidence"]["process"]["observations"]), 2)

    def test_unallowlisted_control_error_text_is_not_copied(self):
        secret = "private-controller-exception-text"
        failure = subject.safe_controller_error(subject.coordinator.ControlError(secret))
        self.assertEqual(
            failure, {"code": "controller_control_error", "class": "control_error"}
        )
        self.assertNotIn(secret, json.dumps(failure))

    def test_stage_checkpoints_exist_before_evaluation(self):
        with tempfile.TemporaryDirectory() as tmp:
            clock = FakeClock()
            backend = FakeBackend(Path(tmp), clock)
            readiness = {"fresh": True, "waited_indefinitely": True,
                         "phase_id": "lock-fake"}
            observed = {}

            def state():
                return {"state": backend.session_state(), "error_code": None}

            def evaluator(_evidence):
                checkpoint = json.loads(
                    (Path(tmp) / "lock-evidence.json").read_text(encoding="utf-8")
                )
                observed.update(checkpoint)
                return {"schema": "screenwise.lock-transition-acceptance.v1",
                        "status": "pass", "reasons": [], "eligible": True}

            with mock.patch.object(subject.live.windows, "session_state", side_effect=state):
                with redirect_stdout(io.StringIO()):
                    result = subject.LockTransitionController(
                        backend, readiness, clock=clock, sleeper=clock.sleep,
                        evaluator=evaluator,
                    ).run()

            self.assertEqual(result["decision"]["status"], "pass")
            self.assertEqual(observed["schema"], subject.EVIDENCE_CHECKPOINT_SCHEMA)
            self.assertEqual(observed["stage"], "cleanup_complete")
            self.assertTrue(observed["evidence"]["cleanup"]["processes_stopped"])
            self.assertEqual(
                {
                    path.name
                    for path in Path(tmp).glob("lock-evidence-*.json")
                },
                {
                    "lock-evidence-ready_for_lock.json",
                    "lock-evidence-locked_observation_complete.json",
                    "lock-evidence-unlocked_waiting_fixture_focus.json",
                    "lock-evidence-cleanup_complete.json",
                },
            )

    def test_bounded_secure_desktop_gaps_pass_real_acceptance_contract(self):
        with tempfile.TemporaryDirectory() as tmp:
            clock = FakeClock()
            backend = FakeBackend(Path(tmp), clock)
            readiness = {"fresh": True, "waited_indefinitely": True, "phase_id": "lock-fake"}

            def state():
                now = clock()
                if now < 10.75:
                    return {"state": "unlocked", "error_code": None}
                if now < 11.25:
                    return {"state": "unknown", "error_code": "secure_desktop_unavailable"}
                if now < 27.75:
                    return {"state": "locked", "error_code": None}
                if now < 28.25:
                    return {"state": "unknown", "error_code": "secure_desktop_unavailable"}
                return {"state": "unlocked", "error_code": None}

            with mock.patch.object(subject.live.windows, "session_state", side_effect=state):
                with redirect_stdout(io.StringIO()):
                    result = subject.LockTransitionController(
                        backend, readiness, clock=clock, sleeper=clock.sleep,
                        evaluator=subject.load_evaluator(),
                    ).run()
            self.assertEqual(result["decision"]["status"], "pass", result["decision"])
            self.assertEqual(
                result["decision"]["wts_unknown_reason_counts"],
                {"secure_desktop_unavailable": 4},
            )

    def test_first_transition_poll_may_be_bounded_unknown_after_confirmed_sample(self):
        with tempfile.TemporaryDirectory() as tmp:
            clock = FakeClock()
            backend = FakeBackend(Path(tmp), clock)
            readiness = {"fresh": True, "waited_indefinitely": True, "phase_id": "lock-fake"}
            calls = {"count": 0}

            backend.capture_notices = lambda: ([
                {"typed": True, "reason_code": "wts_session_locked",
                 "message": subject.LOCKED_MESSAGE, "timestamp_ms": 10_500},
                {"typed": True, "reason_code": "wts_session_unlocked",
                 "message": subject.UNLOCKED_MESSAGE, "timestamp_ms": 28_000},
            ], [])

            def state():
                calls["count"] += 1
                if calls["count"] == 1:
                    return {"state": "unlocked", "error_code": None}
                if calls["count"] == 2:
                    return {"state": "unknown", "error_code": "secure_desktop_unavailable"}
                if clock() < 28:
                    return {"state": "locked", "error_code": None}
                return {"state": "unlocked", "error_code": None}

            with mock.patch.object(subject.live.windows, "session_state", side_effect=state):
                with redirect_stdout(io.StringIO()):
                    result = subject.LockTransitionController(
                        backend, readiness, clock=clock, sleeper=clock.sleep,
                        evaluator=subject.load_evaluator(),
                    ).run()
            self.assertEqual(result["decision"]["status"], "pass", result["decision"])
            self.assertEqual(
                result["decision"]["wts_unknown_reason_counts"],
                {"secure_desktop_unavailable": 1},
            )

    def test_other_unknown_is_retained_and_partial_notices_precede_stop(self):
        with tempfile.TemporaryDirectory() as tmp:
            clock = FakeClock()
            backend = FakeBackend(Path(tmp), clock)
            readiness = {"fresh": True, "waited_indefinitely": True, "phase_id": "lock-fake"}
            order = []
            capture_notices = backend.capture_notices
            stop_recorder = backend.stop_recorder

            def state():
                if clock() < 10.75:
                    return {"state": "unlocked", "error_code": None}
                return {"state": "unknown", "error_code": "wts_query_failed"}

            def capture():
                order.append("notices")
                return capture_notices()

            def stop():
                order.append("stop")
                return stop_recorder()

            backend.capture_notices = capture
            backend.stop_recorder = stop
            with mock.patch.object(subject.live.windows, "session_state", side_effect=state):
                with redirect_stdout(io.StringIO()):
                    result = subject.LockTransitionController(
                        backend, readiness, clock=clock, sleeper=clock.sleep,
                        evaluator=subject.load_evaluator(),
                    ).run()
            self.assertNotEqual(result["decision"]["status"], "pass")
            self.assertEqual(order[:2], ["notices", "stop"])
            self.assertEqual(result["evidence"]["wts_samples"][-1]["state"], "unknown")
            self.assertEqual(
                result["evidence"]["wts_samples"][-1]["error_code"],
                "wts_query_failed",
            )


if __name__ == "__main__":
    unittest.main()
