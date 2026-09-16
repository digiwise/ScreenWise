import subprocess
import tempfile
import unittest
import os
from pathlib import Path
from unittest.mock import Mock, patch

import browser_runtime as runtime


class FakeServer:
    def __init__(self, address=("127.0.0.1", 31480), handler=None):
        self.server_address = address
        self.handler = handler
        self.daemon_threads = False
        self.block_on_close = True
        self.shutdown_calls = 0
        self.close_calls = 0
        self.serve_calls = 0

    def serve_forever(self):
        self.serve_calls += 1

    def shutdown(self):
        self.shutdown_calls += 1

    def server_close(self):
        self.close_calls += 1


class FakeThread:
    def __init__(self, *, target, name, daemon):
        self.target = target
        self.name = name
        self.daemon = daemon
        self.start_calls = 0
        self.join_calls = []
        self.alive = False

    def start(self):
        self.start_calls += 1
        self.target()

    def join(self, timeout):
        self.join_calls.append(timeout)

    def is_alive(self):
        return self.alive


class FakeOwned:
    executable_path = r"C:\Program Files\Google\Chrome\Application\chrome.exe"
    pid = 4321
    creation_time_ticks = 987654

    def __init__(self):
        self.terminate_calls = 0
        self.graceful_calls = 0
        self.graceful_result = {"stopped": True, "forced": False, "exit_code": 0, "error_code": None}

    def terminate_owned(self, timeout=5):
        self.terminate_calls += 1
        return {"stopped": True, "forced": True, "exit_code": 1, "error_code": "forced_terminate"}

    def graceful_stop(self, timeout=10):
        self.graceful_calls += 1
        return self.graceful_result

    def poll(self):
        return None

    def window_visibility(self):
        return {"pid": self.pid, "has_visible_window": True, "error_code": None}


def inventory_for(processes, error_code=None):
    def check(_paths):
        return {"processes": list(processes), "error_code": error_code}
    return check


class ImportAndServerTests(unittest.TestCase):
    def test_import_has_no_runtime_side_effects(self):
        with patch.object(runtime, "ThreadingHTTPServer") as server, patch.object(
            runtime.windows_controls.OwnedProcess, "start"
        ) as start:
            self.assertFalse(runtime.BrowserFixtureServer().started)
        server.assert_not_called()
        start.assert_not_called()

    def test_server_lifecycle_is_explicit_and_loopback_only(self):
        made = []

        def factory(address, handler):
            actual = (address[0], 31480 if address[1] == 0 else address[1])
            server = FakeServer(address=actual, handler=handler)
            made.append(server)
            return server

        server = runtime.BrowserFixtureServer(
            browser_html=b"<html>synthetic</html>",
            server_factory=factory,
            thread_factory=FakeThread,
        )
        self.assertFalse(server.started)
        self.assertEqual("http://127.0.0.1:31480/browser-fixture.html", server.start())
        self.assertTrue(server.started)
        self.assertEqual(1, made[0].serve_calls)
        self.assertEqual(0, made[0].shutdown_calls)
        result = server.stop()
        self.assertEqual({"stopped": True, "error_code": None}, result)
        self.assertEqual(1, made[0].shutdown_calls)
        self.assertEqual(1, made[0].close_calls)

    def test_server_rejects_non_loopback_binding(self):
        def factory(_address, _handler):
            return FakeServer(address=("0.0.0.0", 31480))

        server = runtime.BrowserFixtureServer(
            browser_html=b"x", server_factory=factory, thread_factory=FakeThread
        )
        with self.assertRaisesRegex(runtime.BrowserRuntimeError, "loopback"):
            server.start()


class ChromeLaunchTests(unittest.TestCase):
    def make_paths(self, root):
        exe = root / "chrome.exe"
        exe.write_bytes(b"synthetic executable")
        return exe, root / "profile", root / "stdout.log", root / "stderr.log"

    def test_launch_requires_fresh_profile_and_uses_only_reviewed_flags(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            exe, profile, stdout, stderr = self.make_paths(root)
            owned = FakeOwned()
            owned.executable_path = str(exe.resolve())
            start = Mock(return_value=owned)
            entries = [{
                "pid": owned.pid,
                "executable_path": str(exe),
                "creation_time_ticks": owned.creation_time_ticks,
            }]
            inventory = Mock(side_effect=[
                {"processes": [], "error_code": None},
                {"processes": entries, "error_code": None},
            ])
            launched = runtime.OwnedChrome.launch(
                exe,
                profile,
                "http://127.0.0.1:31480/browser-fixture.html?runId=run-1&phaseId=phase-1",
                stdout,
                stderr,
                process_start=start,
                inventory=inventory,
            )
            argv = start.call_args.args[0]
            self.assertEqual(os.path.normcase(str(exe.resolve())), argv[0])
            self.assertIn("--user-data-dir=" + str(profile.resolve()), argv)
            self.assertEqual(
                set(runtime.REVIEWED_CHROME_FLAGS),
                set(argv[1:-2]),
            )
            self.assertTrue(profile.is_dir())
            self.assertEqual(owned.pid, launched.pid)
            self.assertTrue(start.call_args.kwargs["visible"])

    def test_launch_rejects_existing_exact_executable(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            exe, profile, stdout, stderr = self.make_paths(root)
            with self.assertRaisesRegex(runtime.BrowserRuntimeError, "already_running"):
                runtime.OwnedChrome.launch(
                    exe,
                    profile,
                    "http://127.0.0.1:31480/browser-fixture.html",
                    stdout,
                    stderr,
                    process_start=Mock(),
                    inventory=inventory_for([{"pid": 12}]),
                )

    def test_launch_stops_unverified_process(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            exe, profile, stdout, stderr = self.make_paths(root)
            owned = FakeOwned()
            with self.assertRaisesRegex(runtime.BrowserRuntimeError, "identity"):
                runtime.OwnedChrome.launch(
                    exe,
                    profile,
                    "http://127.0.0.1:31480/browser-fixture.html",
                    stdout,
                    stderr,
                    process_start=Mock(return_value=owned),
                    inventory=inventory_for([]),
                )
            self.assertEqual(1, owned.terminate_calls)

    def test_invalid_url_and_arbitrary_query_are_rejected(self):
        with self.assertRaisesRegex(runtime.BrowserRuntimeError, "loopback"):
            runtime._validate_fixture_url("https://127.0.0.1:31480/browser-fixture.html")
        with self.assertRaisesRegex(runtime.BrowserRuntimeError, "query"):
            runtime._validate_fixture_url("http://127.0.0.1:31480/browser-fixture.html?url=https")

    def test_stop_reports_remaining_exact_path_processes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            exe, profile, _stdout, _stderr = self.make_paths(root)
            owned = FakeOwned()
            browser = runtime.OwnedChrome(
                owned,
                str(exe.resolve()),
                profile,
                "http://127.0.0.1:31480/browser-fixture.html",
                inventory=inventory_for([{"pid": 999}]),
            )
            result = browser.stop()
            self.assertFalse(result["stopped"])
            self.assertEqual("owned_browser_children_remain", result["error_code"])
            self.assertEqual(1, owned.graceful_calls)


class BrowserFixtureRuntimeTests(unittest.TestCase):
    def test_runtime_facade_requires_explicit_server_and_phase_calls(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            exe, _profile, _stdout, _stderr = ChromeLaunchTests().make_paths(root)
            runtime_instance = runtime.BrowserFixtureRuntime(
                exe,
                31480,
                b"<html>synthetic</html>",
                root,
                "run-1",
                server_factory=lambda address, handler: FakeServer(
                    address=(address[0], 31480), handler=handler
                ),
                thread_factory=FakeThread,
            )
            self.assertFalse(runtime_instance.owned_pids)
            with self.assertRaisesRegex(runtime.BrowserRuntimeError, "server"):
                runtime_instance.show_phase("127.0.0.1", "phase-1", "ordinary")
            self.assertEqual(
                "http://127.0.0.1:31480/browser-fixture.html",
                runtime_instance.start_server(),
            )

    def test_runtime_supports_loopback_hosts_and_exact_focus_targets(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            exe, _profile, _stdout, _stderr = ChromeLaunchTests().make_paths(root)
            owned = FakeOwned()
            owned.executable_path = str(exe.resolve())
            start = Mock(return_value=owned)
            inventory = Mock(side_effect=[
                {"processes": [], "error_code": None},
                {"processes": [{
                    "pid": owned.pid,
                    "executable_path": str(exe),
                    "creation_time_ticks": owned.creation_time_ticks,
                }], "error_code": None},
                {"processes": [], "error_code": None},
            ])
            runtime_instance = runtime.BrowserFixtureRuntime(
                exe,
                31480,
                b"<html>synthetic</html>",
                root,
                "run-1",
                process_start=start,
                inventory=inventory,
                foreground_query=lambda: {"pid": owned.pid, "hwnd": 1234},
                server_factory=lambda address, handler: FakeServer(
                    address=(address[0], 31480), handler=handler
                ),
                thread_factory=FakeThread,
            )
            runtime_instance.start_server()
            shown = runtime_instance.show_phase("localhost", "phase-1", "secret")
            self.assertEqual({"pid": 4321, "hwnd": 1234},
                             {key: shown[key] for key in ("pid", "hwnd")})
            self.assertTrue(runtime_instance.foreground_valid())
            self.assertEqual((4321,), runtime_instance.owned_pids)
            self.assertIn("localhost:31480", start.call_args.args[0][-1])
            self.assertIn("focus=secret", start.call_args.args[0][-1])
            result = runtime_instance.close()
            self.assertTrue(result["browser_stopped"])
            self.assertTrue(result["server_stopped"])

    def test_runtime_rejects_non_loopback_or_unknown_focus(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            exe, _profile, _stdout, _stderr = ChromeLaunchTests().make_paths(root)
            runtime_instance = runtime.BrowserFixtureRuntime(
                exe, 31480, b"x", root, "run-1"
            )
            with self.assertRaisesRegex(runtime.BrowserRuntimeError, "host"):
                runtime_instance._phase_url("example.test", "phase-1", "ordinary")
            with self.assertRaisesRegex(runtime.BrowserRuntimeError, "focus"):
                runtime_instance.show_phase("127.0.0.1", "phase-1", "password")


if __name__ == "__main__":
    unittest.main()
