import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import fixture_transport as ft


class Clock:
    def __init__(self): self.t = 0.0
    def __call__(self): return self.t
    def sleep(self, seconds): self.t += seconds


class FixtureTransportTests(unittest.TestCase):
    def ready(self, root, run="run-1"):
        (root / "ready.json").write_text(json.dumps({"schemaVersion": 1, "fixture": "privacy", "runId": run,
                                                       "processId": 7, "utc": "2026-01-01T00:00:00Z"}), encoding="utf-8")

    def ack_on_sleep(self, root, phase, action, clock, kind="privacy"):
        def sleep(seconds):
            clock.t += seconds
            ack = root / ("acks" if kind == "privacy" else "drm-acks") / f"{phase}.json"
            ack.parent.mkdir(exist_ok=True)
            ack.write_text(json.dumps({"schemaVersion": 1, "fixture": "privacy" if kind == "privacy" else "synthetic-drm",
                                       "runId": "run-1", "phaseId": phase, "action": action,
                                       "success": True, "visible": False,
                                       "verifiedForeground": True,
                                       "foregroundHwnd": 1,
                                       "excludedVisible": False}), encoding="utf-8")
        return sleep

    def test_atomic_command_and_release_focus(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, clock = Path(tmp), Clock(); self.ready(root)
            client = ft.FixtureClient(root, "run-1", clock=clock, sleep=lambda _: None)
            client.sleep = self.ack_on_sleep(root, "p1", "release-focus", clock)
            ack = client.send("p1", "release-focus")
            self.assertTrue((root / "commands/p1.json").is_file())
            self.assertFalse(ack["visible"])

    def test_stale_ack_and_duplicate_phase_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); self.ready(root); (root / "acks").mkdir()
            (root / "acks/p1.json").write_text(json.dumps({"runId": "old"}), encoding="utf-8")
            c = ft.FixtureClient(root, "run-1")
            with self.assertRaises(ft.FixtureTransportError): c.send("p1", "hide")
            c._phases.add("p2")
            with self.assertRaises(ft.FixtureTransportError): c.send("p2", "hide")

    def test_restore_requires_stopped_state(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); self.ready(root); c = ft.FixtureClient(root, "run-1")
            with self.assertRaises(ft.FixtureTransportError): c.send("p1", "restore-clipboard-after-recorder-stop")
            with self.assertRaises(ft.FixtureTransportError): c.send("p2", "hide", recorder_stopped=True)

    def test_ready_identity_and_strict_ack(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, clock = Path(tmp), Clock(); self.ready(root)
            c = ft.FixtureClient(root, "run-1", clock=clock, sleep=lambda _: None)
            c.sleep = self.ack_on_sleep(root, "p1", "hide", clock)
            with self.assertRaises(ft.FixtureTransportError): c.send("p1", "hide", expected_pid=8)
            (root / "acks/p1.json").unlink(missing_ok=True)
            c = ft.FixtureClient(root, "run-1", clock=clock, sleep=lambda _: None)
            c.sleep = lambda seconds: (setattr(clock, "t", clock.t + seconds), (root / "acks").mkdir(exist_ok=True), (root / "acks/p1.json").write_text(json.dumps({"runId":"run-1","phaseId":"p1","action":"hide","success":1,"verifiedForeground":True,"foregroundHwnd":1,"visible":False}), encoding="utf-8"))
            with self.assertRaises(ft.FixtureTransportError): c.send("p1", "hide")

    def test_browser_route_has_one_path_and_security_headers(self):
        status, headers, body = ft.route_browser_fixture("/browser-fixture.html?runId=x", "127.0.0.1:31480", 31480)
        self.assertEqual(status, 200); self.assertIn(b"synthetic browser fixture", body)
        self.assertEqual(headers["Cache-Control"], "no-store")
        self.assertEqual(ft.route_browser_fixture("/x", "127.0.0.1:31480", 31480)[0], 404)
        self.assertEqual(ft.route_browser_fixture("/browser-fixture.html", "evil:31480", 31480)[0], 403)

    def test_drm_reuses_fixed_startup_ready_for_each_command(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); clock = Clock()
            (root / "drm-ready-startup").with_suffix(".json").write_text(json.dumps({
                "schemaVersion": 1, "fixture": "synthetic-drm", "runId": "run-1", "phaseId": "startup",
                "processId": 7, "utc": "now", "success": True, "verifiedForeground": True, "foregroundHwnd": 1, "visible": True}), encoding="utf-8")
            c = ft.FixtureClient(root, "run-1", "drm", clock=clock, ready_phase_id="startup")
            c.sleep = self.ack_on_sleep(root, "p1", "release-focus", clock, "drm")
            ack = c.send("p1", "release-focus")
            self.assertFalse(ack["visible"])

    def test_failed_ack_is_rejected_except_terminal_clipboard_preservation(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, clock = Path(tmp), Clock(); self.ready(root)
            c = ft.FixtureClient(root, "run-1", clock=clock)
            c.sleep = lambda seconds: (clock.sleep(seconds), (root / "acks").mkdir(exist_ok=True),
                (root / "acks/p1.json").write_text(json.dumps({"schemaVersion":1,"fixture":"privacy","runId":"run-1",
                    "phaseId":"p1","action":"hide","success":False,"verifiedForeground":True,"foregroundHwnd":1,"visible":False}), encoding="utf-8"))
            with self.assertRaises(ft.FixtureTransportError): c.send("p1", "hide")

    def test_timeout_boundary_is_finite_and_does_not_spin_at_deadline(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, clock = Path(tmp), Clock(); self.ready(root)
            c = ft.FixtureClient(root, "run-1", clock=clock, sleep=clock.sleep)
            with self.assertRaises(ft.FixtureTransportError): c.send("p1", "hide", timeout=0.01)
            self.assertGreater(clock.t, 0)


if __name__ == "__main__": unittest.main()
