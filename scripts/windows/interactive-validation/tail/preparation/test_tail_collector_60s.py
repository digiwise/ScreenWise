import contextlib
import hashlib
import io
import json
import os
import sqlite3
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

os.environ.setdefault("SCREENWISE_VALIDATION_OUTPUT_DEVICE", "selected-output-device (output)")

import tail_collector_60s as collector

READINESS = {"explicit_owner_ready": True, "nonce_match": True,
             "expires_at": None, "deadline_enforced": False}


class QueueClock:
    def __init__(self):
        self.values = iter([5.0, 19.5, 19.7, 26.0, 45.0, 45.1, 45.2, 60.0, 60.5, 66.0])

    def __call__(self):
        return next(self.values)


class FakeBackend:
    def __init__(self, issues=None):
        self.record = {"run_id": "tail-test-01"}
        self.calls = []
        self.shutdown = None
        self.sleeper = lambda _: None
        self.issues = list(issues or [])
        self.snapshot_calls = 0

    def start_fixture(self): self.calls.append("start_fixture")
    def journal(self, name, payload): self.calls.append(("journal", name, payload))
    def start_phase(self, phase, chunk):
        self.calls.append(("start_phase", phase, chunk))
        return {"process_identity": "pid1:create1" if phase == "partial" else "pid2:create2",
                "created_at": 0.0 if phase == "partial" else 40.0}
    def await_api(self): self.calls.append("await_api")
    def api_checks(self):
        return {"passed": True, "capture_handle_ready": True, "exact_audio_selection": True,
                "observed_device_identity": collector.live.OUTPUT}
    def verify_owned_loopback_listener(self): self.calls.append("listener"); return True
    def capture_status(self): return {"issues": [], "privacy_transition_count": 0}
    def play_once(self, name):
        self.calls.append(("play", name))
        return {"baseline": (5.5, 16.283), "tail": (16.5, 19.384), "restart": (45.5, 56.273)}[name]
    def db_snapshot(self):
        self.snapshot_calls += 1
        if self.snapshot_calls == 1:
            return {"audio_chunk_count": 0, "max_audio_chunk_id": 0,
                    "marker_counts": {"baseline": 0, "tail": 0, "restart": 0}}
        return {"audio_chunk_count": 2, "max_audio_chunk_id": 2,
                "marker_counts": {"baseline": 1, "tail": 1, "restart": 1}}
    def auth_matrix(self):
        return {"base_url": "http://127.0.0.1:31479", "missing_status": 403,
                "wrong_status": 403, "valid_status": 200, "redirect_followed": False}
    def stop_recorder(self):
        self.calls.append("stop_recorder")
        self.shutdown = {"exit_code": 0, "forced": False}
        return True
    def safe_phase_log_counts(self, phase):
        return {"device_recovery_count": 0, "privacy_transition_count": 0}
    def safe_shutdown_issues(self): return list(self.issues)
    def clean_shutdown(self): return self.shutdown == {"exit_code": 0, "forced": False}
    def one_shutdown_chunk(self, prior):
        return {"new_chunk_count": 1, "total_audio_chunk_count": 1, "chunk_id": 1,
                "audio_file_exists": True, "duration_source": "ffprobe", "duration_seconds": 14.0,
                "baseline_marker": collector.MARKERS["baseline"], "tail_marker": collector.MARKERS["tail"],
                "baseline_count_same_chunk": 1, "tail_count_same_chunk": 1,
                "baseline_audio_chunk_id": 1, "tail_audio_chunk_id": 1,
                "device_identity": collector.live.OUTPUT}
    def search_chunk_ids(self, marker):
        return 200, [2] if marker == collector.MARKERS["restart"] else [1]


class CollectorTests(unittest.TestCase):
    def test_purely_mocked_two_process_sequence_passes(self):
        backend = FakeBackend()
        result = collector.TailCollector60s(backend, READINESS, clock=QueueClock()).run()
        self.assertEqual(result["status"], "pass")
        self.assertFalse(result["collector_live_verified"])
        self.assertEqual([call for call in backend.calls if isinstance(call, tuple) and call[0] == "start_phase"],
                         [("start_phase", "partial", 60), ("start_phase", "restart", 5)])
        self.assertEqual([call for call in backend.calls if isinstance(call, tuple) and call[0] == "play"],
                         [("play", "baseline"), ("play", "tail"), ("play", "restart")])
        checkpoints = [call[2]["name"] for call in backend.calls
                       if isinstance(call, tuple) and call[:2] == ("journal", "evidence_checkpoint")]
        self.assertEqual(checkpoints, ["pre_stop", "api_auth_before_stop", "first_shutdown",
                                       "restart_api_auth", "final_shutdown"])

    def test_shutdown_issue_blocks_restart_without_fabricating_success(self):
        backend = FakeBackend(["queued_work_discarded"])
        result = collector.TailCollector60s(backend, READINESS, clock=QueueClock()).run()
        self.assertEqual(result["status"], "fail")
        self.assertNotIn(("start_phase", "restart", 5), backend.calls)
        self.assertIn("first_shutdown_reported_issue", result["reasons"])

    def test_preview_main_never_constructs_backend(self):
        with patch.object(collector, "preview", return_value={"execution": "preview_only"}), \
             patch.object(collector, "TailWindowsBackend", side_effect=AssertionError("backend constructed")):
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                self.assertEqual(collector.main(["--run-id", "preview-01"]), 0)
            self.assertEqual(json.loads(output.getvalue())["execution"], "preview_only")

    def test_preview_reads_waiting_record_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            (path / "waiting.json").write_text(json.dumps({"mode": "audio-output", "state": "waiting_for_owner"}), encoding="utf-8")
            with patch.object(collector.coordinator, "session_path", return_value=path):
                result = collector.preview("preview-01")
            self.assertEqual(result["readiness_timeout_seconds"], None)
            self.assertFalse(result["collector_live_verified"])

    def test_asset_validation_is_bounded_to_new_synthetic_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            speech = root / "speech-retry"
            speech.mkdir()
            manifest_rows = []
            validation_rows = []
            for name in ("baseline", "tail", "restart"):
                file = speech / f"{name}.wav"
                file.write_bytes(("synthetic-" + name).encode())
                digest = hashlib.sha256(file.read_bytes()).hexdigest().upper()
                manifest_rows.append({"name": name, "file": file.name,
                                      "marker": collector.MARKERS[name], "sha256": digest})
                validation_rows.append({"name": name, "sha256": digest,
                                        "duration_seconds": collector.DURATIONS[name],
                                        "sample_rate_hz": 22050})
            (speech / "manifest.json").write_text(json.dumps({
                "schema": "screenwise.synthetic-tail-speech.v1",
                "playbackPerformed": False, "files": manifest_rows,
            }), encoding="utf-8")
            validation = root / "speech-validation.json"
            validation.write_text(json.dumps({"playback_performed": False,
                                               "files": validation_rows}), encoding="utf-8")
            with patch.object(collector, "SPEECH", speech), \
                 patch.object(collector, "SPEECH_VALIDATION", validation):
                assets = collector.verified_speech_assets()
                self.assertEqual(set(assets), {"baseline", "tail", "restart"})
                self.assertTrue(all(path.parent == speech.resolve() for path in assets.values()))

    def test_one_shot_player_is_lazy_under_mock(self):
        backend = object.__new__(collector.TailWindowsBackend)
        backend.assets = {"baseline": Path("synthetic.wav")}
        backend.player = unittest.mock.Mock()
        backend.player.play.return_value = None
        backend.clock = unittest.mock.Mock(side_effect=[1.0, 1.1, 11.783])
        backend.sleeper = unittest.mock.Mock()
        backend.phase_deadline = 45.0
        backend.phase_watchdog_done = __import__("threading").Event()
        backend.recorder = unittest.mock.Mock(); backend.recorder.poll.return_value = None
        backend.session_state = unittest.mock.Mock(return_value="unlocked")
        backend.foreground_valid = unittest.mock.Mock(return_value=True)
        backend.journal = unittest.mock.Mock()
        self.assertEqual(backend.play_once("baseline"), (1.1, 11.783))
        backend.player.play.assert_called_once_with(Path("synthetic.wav"))
        backend.player.stop.assert_called_once()

    def test_playback_rejected_when_watchdog_budget_cannot_fit_clip(self):
        backend = object.__new__(collector.TailWindowsBackend)
        backend.assets = {"tail": Path("synthetic.wav")}; backend.player = unittest.mock.Mock()
        backend.clock = unittest.mock.Mock(return_value=44.0); backend.phase_deadline = 45.0
        backend.phase_watchdog_done = __import__("threading").Event()
        backend.recorder = unittest.mock.Mock(); backend.recorder.poll.return_value = None
        with self.assertRaisesRegex(collector.coordinator.ControlError, "tail_playback_budget_insufficient"):
            backend.play_once("tail")
        backend.player.play.assert_not_called()

    def test_stale_phase_watchdog_cannot_stop_new_phase(self):
        backend = object.__new__(collector.TailWindowsBackend)
        old = unittest.mock.Mock(pid=1, creation_time_ticks=10)
        backend.recorder = unittest.mock.Mock(pid=2, creation_time_ticks=20)
        backend.clock = unittest.mock.Mock(return_value=2.0)
        backend.stop_lock = __import__("threading").Lock()
        backend.journal = unittest.mock.Mock(); backend.stop_playback = unittest.mock.Mock()
        backend._stop_current_recorder = unittest.mock.Mock()
        backend._phase_watchdog(1.0, {"pid": 1, "creation_ticks": 10},
                                __import__("threading").Event(), old)
        backend._stop_current_recorder.assert_not_called()

    def test_real_stop_delegation_does_not_deadlock_on_nested_lock(self):
        import threading
        with tempfile.TemporaryDirectory() as tmp:
            backend = collector.TailWindowsBackend({"run_id": "nested-stop", "mode": "audio-output"}, Path(tmp))
            backend.recorder = unittest.mock.Mock()
            backend.recorder.poll.return_value = 0
            backend.recorder.graceful_stop.return_value = {"exit_code": 0, "forced": False}
            backend.journal = unittest.mock.Mock()
            result = []
            thread = threading.Thread(target=lambda: result.append(backend.stop_recorder()), daemon=True)
            thread.start()
            thread.join(1)
            self.assertFalse(thread.is_alive(), "nested shutdown lock deadlocked")
            self.assertEqual(result, [True])
            self.assertTrue(backend.phase_watchdog_done.is_set())
            backend.recorder.graceful_stop.assert_called_once_with(timeout=40)

    def test_startup_focus_wait_is_passive_and_precedes_recording(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = collector.TailWindowsBackend({"run_id": "focus-wait", "mode": "audio-output"}, Path(tmp))
            backend.fixture = unittest.mock.Mock(pid=456)
            backend.fixture.poll.return_value = None
            backend.session_state = unittest.mock.Mock(return_value="unlocked")
            backend.journal = unittest.mock.Mock()
            backend.sleeper = unittest.mock.Mock()
            refused = collector.coordinator.ControlError("fixture_focus_not_verified")
            with patch.object(collector.live.WindowsBackend, "fixture_action", side_effect=[refused, None]) as action, \
                 patch.object(collector.live.windows, "foreground_identity", side_effect=[{"pid": 999, "hwnd": 1}, {"pid": 456, "hwnd": 2}]):
                backend.fixture_action("plain", "initial-control")
            self.assertEqual(action.call_count, 2)
            backend.sleeper.assert_called_once_with(0.25)
            self.assertIsNone(backend.recorder)
            self.assertIsNone(backend.journal.call_args.args[1]["timeout_seconds"])

    def test_startup_focus_wait_fails_closed_on_lock(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = collector.TailWindowsBackend({"run_id": "focus-lock", "mode": "audio-output"}, Path(tmp))
            backend.fixture = unittest.mock.Mock(pid=456)
            backend.fixture.poll.return_value = None
            backend.session_state = unittest.mock.Mock(return_value="locked")
            backend.journal = unittest.mock.Mock()
            with patch.object(collector.live.WindowsBackend, "fixture_action", side_effect=collector.coordinator.ControlError("fixture_focus_not_verified")):
                with self.assertRaisesRegex(collector.coordinator.ControlError, "fixture_focus_wait_interrupted"):
                    backend.fixture_action("plain", "initial-control")
            self.assertIsNone(backend.recorder)

    def test_auth_matrix_uses_missing_wrong_and_valid_token(self):
        backend = object.__new__(collector.TailWindowsBackend)
        backend.token = "valid-token"
        seen = []
        def request(path, authenticated=True):
            seen.append((authenticated, backend.token))
            if not authenticated or backend.token == "wrong-token":
                return 403, b""
            return 200, b"{}"
        backend.protected_request = request
        result = backend.auth_matrix()
        self.assertEqual((result["missing_status"], result["wrong_status"], result["valid_status"]), (403, 403, 200))
        self.assertEqual(backend.token, "valid-token")
        self.assertEqual(seen, [(False, "valid-token"), (True, "wrong-token"), (True, "valid-token")])

    def test_owned_listener_requires_exact_pid_scoped_loopback_port(self):
        backend = object.__new__(collector.TailWindowsBackend)
        backend.recorder = unittest.mock.Mock(pid=123)
        backend.recorder.creation_time_ticks = 456
        backend.recorder.poll.return_value = None
        inventory = {"processes": [{"pid": 123, "creation_time_ticks": 456,
                                     "executable_path": str(collector.live.SCOPED[0])}]}
        good = {"endpoints": [{"protocol": "tcp", "state": "listening", "local_port": 31479, "local_address": "127.0.0.1"}]}
        with patch.object(collector.live.windows, "inventory_known_executables", return_value=inventory), \
             patch.object(collector.live.windows, "inspect_network_endpoints", return_value=good):
            self.assertTrue(backend.verify_owned_loopback_listener())
        bad = {"endpoints": [{"protocol": "tcp", "state": "listening", "local_port": 31479, "local_address": "0.0.0.0"}]}
        with patch.object(collector.live.windows, "inventory_known_executables", return_value=inventory), \
             patch.object(collector.live.windows, "inspect_network_endpoints", return_value=bad):
            with self.assertRaises(collector.coordinator.ControlError):
                backend.verify_owned_loopback_listener()

    def test_post_exit_sqlite_read_is_owned_and_content_bounded(self):
        with tempfile.TemporaryDirectory() as tmp:
            session = Path(tmp)
            data = session / "data"
            data.mkdir()
            (data / ".controller-owned").write_text("tail-test-01", encoding="utf-8")
            media = data / "synthetic.mp4"
            media.write_bytes(b"synthetic-placeholder")
            db = sqlite3.connect(data / "db.sqlite")
            db.execute("CREATE TABLE audio_chunks (id INTEGER, file_path TEXT)")
            db.execute("CREATE TABLE audio_transcriptions (audio_chunk_id INTEGER, transcription TEXT, "
                       "device TEXT, is_input_device BOOLEAN NOT NULL)")
            db.execute("INSERT INTO audio_chunks VALUES (1, ?)", (str(media),))
            selected_name = collector.live.OUTPUT.removesuffix(" (output)")
            db.execute("INSERT INTO audio_transcriptions VALUES (1, ?, ?, 0)",
                       ("cobalt meadow 7", selected_name))
            db.execute("INSERT INTO audio_transcriptions VALUES (1, ?, ?, 0)",
                       (collector.MARKERS["tail"], selected_name))
            db.commit(); db.close()
            backend = object.__new__(collector.TailWindowsBackend)
            backend.path = session; backend.data = data; backend.record = {"run_id": "tail-test-01"}
            backend.recorder = unittest.mock.Mock(); backend.recorder.poll.return_value = 0
            backend._ffprobe_duration = unittest.mock.Mock(return_value=14.0)
            backend.journal = unittest.mock.Mock()
            result = backend.one_shutdown_chunk(0)
            self.assertEqual(result["new_chunk_count"], 1)
            self.assertEqual(result["baseline_audio_chunk_id"], result["tail_audio_chunk_id"])
            self.assertEqual(result["baseline_count_same_chunk"], 1)
            self.assertEqual(result["device_identity"], collector.live.OUTPUT)
            self.assertNotIn(str(media), repr(result))

    def test_shutdown_chunk_rejects_input_flag_for_selected_output_name(self):
        with tempfile.TemporaryDirectory() as tmp:
            session = Path(tmp); data = session / "data"; data.mkdir()
            (data / ".controller-owned").write_text("tail-test-02", encoding="utf-8")
            media = data / "synthetic.mp4"; media.write_bytes(b"synthetic-placeholder")
            db = sqlite3.connect(data / "db.sqlite")
            db.execute("CREATE TABLE audio_chunks (id INTEGER, file_path TEXT)")
            db.execute("CREATE TABLE audio_transcriptions (audio_chunk_id INTEGER, transcription TEXT, "
                       "device TEXT, is_input_device BOOLEAN NOT NULL)")
            db.execute("INSERT INTO audio_chunks VALUES (1, ?)", (str(media),))
            db.execute("INSERT INTO audio_transcriptions VALUES (1, ?, ?, 1)",
                       (collector.MARKERS["tail"], collector.live.OUTPUT.removesuffix(" (output)")))
            db.commit(); db.close()
            backend = object.__new__(collector.TailWindowsBackend)
            backend.path = session; backend.data = data; backend.record = {"run_id": "tail-test-02"}
            backend.recorder = unittest.mock.Mock(); backend.recorder.poll.return_value = 0
            backend.journal = unittest.mock.Mock()
            with self.assertRaisesRegex(collector.coordinator.ControlError, "shutdown_chunk_device_mismatch"):
                backend.one_shutdown_chunk(0)

    def test_db_snapshot_accepts_numeric_marker_forms_once_per_row(self):
        with tempfile.TemporaryDirectory() as tmp:
            session = Path(tmp); data = session / "data"; data.mkdir()
            (data / ".controller-owned").write_text("tail-test-03", encoding="utf-8")
            db = sqlite3.connect(data / "db.sqlite")
            db.execute("CREATE TABLE audio_chunks (id INTEGER, file_path TEXT)")
            db.execute("CREATE TABLE audio_transcriptions (audio_chunk_id INTEGER, transcription TEXT)")
            db.execute("INSERT INTO audio_chunks VALUES (1, 'synthetic.mp4')")
            db.execute("INSERT INTO audio_transcriptions VALUES (1, 'cobalt meadow seven and cobalt meadow 7')")
            db.execute("INSERT INTO audio_transcriptions VALUES (1, 'violet harbor 9')")
            db.execute("INSERT INTO audio_transcriptions VALUES (1, 'silver orchard 5')")
            db.commit(); db.close()
            backend = object.__new__(collector.TailWindowsBackend)
            backend.path = session; backend.data = data; backend.record = {"run_id": "tail-test-03"}
            backend.journal = unittest.mock.Mock()
            result = backend.db_snapshot()
            self.assertEqual(result["marker_counts"], {"baseline": 1, "tail": 1, "restart": 1})
            queried = [call.args[1]["query_forms"] for call in backend.journal.call_args_list]
            self.assertEqual(queried, [list(collector.MARKER_QUERY_FORMS[name])
                                       for name in ("baseline", "tail", "restart")])

    def test_search_uses_actual_audio_content_chunk_id_schema(self):
        backend = object.__new__(collector.TailWindowsBackend)
        word_body = json.dumps({"data": [{"type": "Audio", "content": {"chunk_id": 17}},
                                          {"type": "OCR", "content": {"chunk_id": 99}},
                                          {"audio_chunk_id": 88}]}).encode()
        digit_body = json.dumps({"data": [{"type": "Audio", "content": {"chunk_id": 17}},
                                           {"type": "Audio", "content": {"chunk_id": 18}}]}).encode()
        backend.protected_request = unittest.mock.Mock(side_effect=[(200, word_body), (200, digit_body)])
        backend.journal = unittest.mock.Mock()
        self.assertEqual(backend.search_chunk_ids(collector.MARKERS["baseline"]), (200, [17, 18]))
        requested = [call.args[0] for call in backend.protected_request.call_args_list]
        self.assertTrue(any("q=cobalt+meadow+seven" in value for value in requested))
        self.assertTrue(any("q=cobalt+meadow+7" in value for value in requested))
        backend.journal.assert_called_once_with(
            "synthetic_marker_query",
            {"surface": "authenticated_search", "marker": "baseline",
             "query_forms": list(collector.MARKER_QUERY_FORMS["baseline"])},
        )

    def test_collector_requires_consumed_readiness_evidence(self):
        with self.assertRaises(collector.coordinator.ControlError):
            collector.TailCollector60s(FakeBackend(), {})

    def test_capture_status_fails_closed_on_degraded_or_unknown_schema(self):
        backend = object.__new__(collector.TailWindowsBackend)
        backend.started = "2026-09-16T00:00:00+00:00"
        payload = {"data": [{"reason_code": "wts_session_unlocked", "state": "unlocked"}],
                   "has_more": False, "persistence_degraded": True,
                   "event_delivery": {"near_capacity": False, "dropped_events": 0},
                   "audio_shutdown_degraded": False, "audio_shutdown_issues": []}
        backend.protected_request = unittest.mock.Mock(return_value=(200, json.dumps(payload).encode()))
        with patch.object(collector.live, "utc", return_value="2026-09-16T00:01:00+00:00"):
            status = backend.capture_status()
        self.assertEqual(status, {"issues": ["persistence_degraded"], "privacy_transition_count": 0})
        requested = backend.protected_request.call_args.args[0]
        self.assertIn("start_time=2026-09-16T00%3A00%3A00%2B00%3A00", requested)
        self.assertIn("end_time=2026-09-16T00%3A01%3A00%2B00%3A00", requested)
        self.assertIn("limit=1000", requested)
        payload["event_delivery"]["extra"] = 1
        backend.protected_request.return_value = (200, json.dumps(payload).encode())
        with self.assertRaises(collector.coordinator.ControlError):
            backend.capture_status()

    def test_capture_status_rejects_truncated_or_non_api_reason_codes(self):
        backend = object.__new__(collector.TailWindowsBackend)
        backend.started = "2026-09-16T00:00:00+00:00"
        payload = {"data": [], "has_more": True, "persistence_degraded": False,
                   "event_delivery": {"near_capacity": False, "dropped_events": 0},
                   "audio_shutdown_degraded": False, "audio_shutdown_issues": []}
        backend.protected_request = unittest.mock.Mock(return_value=(200, json.dumps(payload).encode()))
        with self.assertRaisesRegex(collector.coordinator.ControlError, "capture_status_truncated"):
            backend.capture_status()
        payload["has_more"] = False
        payload["data"] = [{"reason_code": "queued_work_discarded"}]
        backend.protected_request.return_value = (200, json.dumps(payload).encode())
        with self.assertRaisesRegex(collector.coordinator.ControlError, "capture_status_schema_invalid"):
            backend.capture_status()

    def test_collector_pin_manifest_is_separate_and_bounded(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); asset = root / "preparation" / "a.py"; asset.parent.mkdir(); asset.write_text("x", encoding="utf-8")
            digest = __import__("hashlib").sha256(asset.read_bytes()).hexdigest().upper()
            pins = root / "collector-pins.json"
            pins.write_text(json.dumps({"schema": "screenwise.tail-collector-pins.v1",
                                        "assets": {"preparation/a.py": digest}}), encoding="utf-8")
            with patch.object(collector, "HERE", asset.parent), patch.object(collector, "PIN_MANIFEST", pins):
                self.assertEqual(collector.verify_collector_pins()["asset_count"], 1)

    def test_phase_log_counts_only_fixed_recovery_and_privacy_phrases(self):
        with tempfile.TemporaryDirectory() as tmp:
            backend = object.__new__(collector.TailWindowsBackend)
            backend.data = Path(tmp)
            (backend.data / "partial-stdout.log").write_text("[DEVICE_RECOVERY] x\nprivate payload\n", encoding="utf-8")
            (backend.data / "partial-stderr.log").write_text("audio privacy generation changed; hidden\n", encoding="utf-8")
            result = backend.safe_phase_log_counts("partial")
            self.assertEqual(result, {"device_recovery_count": 1, "privacy_transition_count": 1})
            self.assertNotIn("private payload", repr(result))

    def test_bad_nonce_cannot_reach_preflight_or_collector(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            waiting = {"schema": "screenwise.owner-paced.v1", "run_id": "gate-01", "mode": "audio-output",
                       "nonce": "correct", "state": "waiting_for_owner"}
            (path / "waiting.json").write_text(json.dumps(waiting), encoding="utf-8")
            fake_backend = unittest.mock.Mock()
            fake_backend.quiescent.return_value = True
            with patch.object(collector.coordinator, "session_path", return_value=path), \
                 patch.object(collector, "TailWindowsBackend", return_value=fake_backend), \
                 patch.object(collector.live, "require_runtime_configuration"), \
                 patch.object(collector.live, "verify_prepared_pins", return_value={}), \
                 patch.object(collector, "verify_collector_pins", return_value={}), \
                 patch.object(collector, "TailCollector60s", side_effect=AssertionError("collector reached")):
                with self.assertRaises(collector.coordinator.ControlError):
                    collector.main(["--run-id", "gate-01", "--execute-interactive", "--owner-ready", "wrong"])
            fake_backend.preflight.assert_not_called()

    def test_explicit_execution_failure_still_cleans_owned_surfaces(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            waiting = {"schema": "screenwise.owner-paced.v1", "run_id": "gate-02", "mode": "audio-output",
                       "nonce": "correct", "state": "waiting_for_owner"}
            (path / "waiting.json").write_text(json.dumps(waiting), encoding="utf-8")
            fake_backend = unittest.mock.Mock()
            fake_backend.quiescent.return_value = True
            fake_backend.recorder = None
            fake_backend.stop_playback.return_value = True
            fake_backend.close_fixture.return_value = True
            fake_backend.processes_stopped.return_value = True
            runner = unittest.mock.Mock()
            runner.run.side_effect = RuntimeError("synthetic failure")
            with patch.object(collector.coordinator, "session_path", return_value=path), \
                 patch.object(collector, "TailWindowsBackend", return_value=fake_backend), \
                 patch.object(collector.live, "require_runtime_configuration"), \
                 patch.object(collector.live, "verify_prepared_pins", return_value={}), \
                 patch.object(collector, "verify_collector_pins", return_value={}), \
                 patch.object(collector, "TailCollector60s", return_value=runner):
                self.assertEqual(collector.main(["--run-id", "gate-02", "--execute-interactive", "--owner-ready", "correct"]), 2)
            fake_backend.preflight.assert_called_once()
            fake_backend.stop_playback.assert_called_once()
            fake_backend.close_fixture.assert_called_once()
            fake_backend.processes_stopped.assert_called_once()
            saved = json.loads((path / "tail-result.json").read_text(encoding="utf-8"))
            self.assertEqual(saved["reasons"], ["collector_execution_failed"])
            self.assertEqual(saved["execution"], "live")
            self.assertFalse(saved["collector_live_verified"])

    def test_explicit_success_is_live_verified_only_after_cleanup(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)
            waiting = {"schema": "screenwise.owner-paced.v1", "run_id": "gate-03", "mode": "audio-output",
                       "nonce": "correct", "state": "waiting_for_owner"}
            (path / "waiting.json").write_text(json.dumps(waiting), encoding="utf-8")
            fake_backend = unittest.mock.Mock()
            fake_backend.quiescent.return_value = True
            fake_backend.recorder = None
            fake_backend.stop_playback.return_value = True
            fake_backend.close_fixture.return_value = True
            fake_backend.processes_stopped.return_value = True
            runner = unittest.mock.Mock()
            runner.run.return_value = {"status": "pass", "reasons": [],
                                       "collector_live_verified": False}
            with patch.object(collector.coordinator, "session_path", return_value=path), \
                 patch.object(collector, "TailWindowsBackend", return_value=fake_backend), \
                 patch.object(collector.live, "require_runtime_configuration"), \
                 patch.object(collector.live, "verify_prepared_pins", return_value={}), \
                 patch.object(collector, "verify_collector_pins", return_value={}), \
                 patch.object(collector, "TailCollector60s", return_value=runner):
                self.assertEqual(collector.main(["--run-id", "gate-03", "--execute-interactive",
                                                 "--owner-ready", "correct"]), 0)
            saved = json.loads((path / "tail-result.json").read_text(encoding="utf-8"))
            self.assertEqual(saved["execution"], "live")
            self.assertTrue(saved["collector_live_verified"])


if __name__ == "__main__":
    unittest.main()
