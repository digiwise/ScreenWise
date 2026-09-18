import subprocess
import unittest
from unittest.mock import Mock, mock_open, patch

import windows_controls as controls


class FakeProcess:
    def __init__(self, pid=4321, poll_values=None, wait_values=None):
        self.pid = pid
        self._poll_values = iter(poll_values or [None])
        self._wait_values = iter(wait_values or [0])
        self.terminate_calls = 0

    def poll(self):
        return next(self._poll_values, None)

    def wait(self, timeout):
        value = next(self._wait_values)
        if value == "timeout":
            raise subprocess.TimeoutExpired("fake", timeout)
        return value

    def terminate(self):
        self.terminate_calls += 1


def identity(pid=4321, path=r"C:\Tools\fixture.exe", ticks=987654):
    return controls.ProcessIdentity(pid, controls._normalize_path(path), ticks)


class OwnedProcessTests(unittest.TestCase):
    def make_owned(self, process, current_identity=None, helper=None):
        expected = identity(pid=process.pid)
        query = Mock(return_value=current_identity if current_identity is not None else expected)
        helper = helper or Mock(return_value=subprocess.CompletedProcess([], 0))
        return controls.OwnedProcess(process, expected, run_helper=helper, identity_query=query), query, helper

    def test_stale_pid_refuses_signal_and_terminate(self):
        process = FakeProcess()
        stale = identity(ticks=987655)
        owned, _query, helper = self.make_owned(process, stale)

        result = owned.graceful_stop()

        self.assertEqual("identity_mismatch", result["error_code"])
        self.assertFalse(result["stopped"])
        self.assertEqual(0, process.terminate_calls)
        helper.assert_not_called()

    def test_path_mismatch_refuses_window_inspection(self):
        process = FakeProcess()
        other = identity(path=r"C:\Other\fixture.exe")
        window_query = Mock()
        owned = controls.OwnedProcess(
            process,
            identity(),
            identity_query=Mock(return_value=other),
            window_query=window_query,
        )

        result = owned.window_visibility()

        self.assertEqual("identity_mismatch", result["error_code"])
        window_query.assert_not_called()

    def test_graceful_close_returns_child_exit(self):
        process = FakeProcess(wait_values=[0])
        owned, _query, helper = self.make_owned(process)

        result = owned.graceful_stop(timeout=2)

        self.assertEqual(
            {"stopped": True, "forced": False, "exit_code": 0, "error_code": None},
            result,
        )
        helper.assert_called_once()
        self.assertEqual(0, process.terminate_calls)

    def test_graceful_timeout_uses_guarded_fallback(self):
        process = FakeProcess(
            poll_values=[None, None],
            wait_values=["timeout", 1],
        )
        owned, query, _helper = self.make_owned(process)

        result = owned.graceful_stop(timeout=2)

        self.assertTrue(result["stopped"])
        self.assertTrue(result["forced"])
        self.assertEqual("graceful_timeout_forced", result["error_code"])
        self.assertEqual(1, process.terminate_calls)
        self.assertEqual(2, query.call_count)

    def test_fallback_rechecks_identity_after_timeout(self):
        process = FakeProcess(
            poll_values=[None, None],
            wait_values=["timeout"],
        )
        expected = identity()
        stale = identity(ticks=expected.creation_time_ticks + 1)
        query = Mock(side_effect=[expected, stale])
        helper = Mock(return_value=subprocess.CompletedProcess([], 0))
        owned = controls.OwnedProcess(
            process, expected, run_helper=helper, identity_query=query
        )

        result = owned.graceful_stop(timeout=2)

        self.assertEqual("identity_mismatch", result["error_code"])
        self.assertFalse(result["stopped"])
        self.assertEqual(0, process.terminate_calls)

    def test_exit_during_failed_helper_is_not_reported_as_forced(self):
        process = FakeProcess(poll_values=[None, 7])
        helper = Mock(return_value=subprocess.CompletedProcess([], 20))
        owned, _query, _helper = self.make_owned(process, helper=helper)

        result = owned.graceful_stop(timeout=2)

        self.assertEqual(
            {"stopped": True, "forced": False, "exit_code": 7, "error_code": None},
            result,
        )
        self.assertEqual(0, process.terminate_calls)


class ParserTests(unittest.TestCase):
    def test_netstat_parser_is_exactly_pid_scoped(self):
        sample = """
          TCP    127.0.0.1:3030       0.0.0.0:0       LISTENING       4321
          TCP    [::1]:3030           [::]:0          LISTENING       43210
          UDP    0.0.0.0:5353         *:*                             4321
        """
        parsed = controls.parse_netstat_output(sample, {4321})

        self.assertEqual(2, len(parsed))
        self.assertEqual({4321}, {item["pid"] for item in parsed})
        self.assertEqual("tcp", parsed[0]["protocol"])
        self.assertEqual(3030, parsed[0]["local_port"])
        self.assertEqual("udp", parsed[1]["protocol"])
        self.assertIsNone(parsed[1]["state"])

    def test_ipv6_endpoint_parser(self):
        self.assertEqual(("::1", 3030), controls._split_endpoint("[::1]:3030"))


class ForegroundTests(unittest.TestCase):
    def test_foreground_identity_contains_only_numeric_metadata(self):
        api = Mock()
        api.foreground_identity.return_value = (4321, 98765)
        with patch.object(controls, "_api", return_value=api):
            result = controls.foreground_identity()

        self.assertEqual({"pid": 4321, "hwnd": 98765}, result)
        self.assertNotIn("title", result)


class SessionStateTests(unittest.TestCase):
    def run_probe(self, payload):
        fake_module = Mock()
        fake_module.probe.return_value = payload
        fake_spec = Mock(loader=Mock())
        with patch.object(
            controls.importlib.util,
            "spec_from_file_location",
            return_value=fake_spec,
        ), patch.object(
            controls.importlib.util, "module_from_spec", return_value=fake_module
        ):
            return controls.session_state()

    def test_active_wts_flags_map_to_lock_states(self):
        base = {
            "session": 2,
            "wts_level": 1,
            "wts_session": 2,
            "wts_state": 0,
            "desktop_access_256": {"available": True},
        }
        locked = self.run_probe({**base, "wts_flags": 0})
        unlocked = self.run_probe({**base, "wts_flags": 1})

        self.assertEqual("locked", locked["state"])
        self.assertIsNone(locked["error_code"])
        self.assertEqual("unlocked", unlocked["state"])
        self.assertIsNone(unlocked["error_code"])

    def test_unlocked_requires_secure_desktop_access(self):
        result = self.run_probe({
            "session": 2,
            "wts_level": 1,
            "wts_session": 2,
            "wts_flags": 1,
            "wts_state": 0,
            "desktop_access_256": {"available": False},
        })

        self.assertEqual("unknown", result["state"])
        self.assertEqual("secure_desktop_unavailable", result["error_code"])

    def test_boolean_wts_field_is_rejected(self):
        result = self.run_probe({
            "session": 2,
            "wts_level": 1,
            "wts_session": 2,
            "wts_flags": True,
            "wts_state": 0,
            "desktop_access_256": {"available": True},
        })

        self.assertEqual("unknown", result["state"])
        self.assertEqual("incomplete_wts_state", result["error_code"])

    def test_malformed_wts_state_fails_closed(self):
        result = self.run_probe({
            "session": 2,
            "wts_level": 1,
            "wts_session": 99,
            "wts_flags": 1,
            "wts_state": 0,
        })

        self.assertEqual("unknown", result["state"])
        self.assertEqual("mismatched_wts_state", result["error_code"])


class SignalHelperTests(unittest.TestCase):
    def test_helper_path_mismatch_never_attaches_console(self):
        kernel = Mock()
        kernel.OpenProcess.return_value = 777
        api = Mock(kernel32=kernel)
        api.open_identity_handle.return_value = 777
        api.process_identity_from_handle.return_value = identity(
            path=r"C:\Other\fixture.exe"
        )
        with patch.object(controls, "_api", return_value=api):
            code = controls._signal_owned(
                4321, r"C:\Tools\fixture.exe", identity().creation_time_ticks
            )

        self.assertEqual(20, code)
        kernel.AttachConsole.assert_not_called()
        kernel.GenerateConsoleCtrlEvent.assert_not_called()
        kernel.CloseHandle.assert_called_once_with(777)


class StartAndInventoryTests(unittest.TestCase):
    def test_start_requires_absolute_executable(self):
        with self.assertRaisesRegex(ValueError, "absolute"):
            controls.OwnedProcess.start(
                ["fixture.exe"], r"C:\Run", "stdout.log", "stderr.log"
            )

    def test_start_uses_fresh_logs_and_private_console(self):
        process = FakeProcess(pid=4321)
        popen = Mock(return_value=process)
        expected = identity()
        opened = mock_open()
        startup = Mock(dwFlags=0, wShowWindow=None)
        with patch.object(controls.os.path, "isfile", return_value=True), patch.object(
            controls.subprocess, "STARTUPINFO", return_value=startup
        ), patch.object(controls.subprocess, "Popen", popen), patch.object(
            controls, "_query_process_identity", return_value=expected
        ), patch("builtins.open", opened):
            owned = controls.OwnedProcess.start(
                [r"C:\Tools\fixture.exe"],
                r"C:\Run",
                r"C:\Evidence\stdout.log",
                r"C:\Evidence\stderr.log",
            )

        self.assertEqual(4321, owned.pid)
        self.assertEqual(
            [
                unittest.mock.call(r"C:\Evidence\stdout.log", "xb", buffering=0),
                unittest.mock.call(r"C:\Evidence\stderr.log", "xb", buffering=0),
            ],
            opened.call_args_list,
        )
        kwargs = popen.call_args.kwargs
        self.assertEqual(controls.CREATE_NEW_CONSOLE, kwargs["creationflags"])
        self.assertIs(startup, kwargs["startupinfo"])
        self.assertEqual(controls.SW_HIDE, startup.wShowWindow)

    def test_visible_start_omits_hidden_startup_info(self):
        process = FakeProcess(pid=4321)
        popen = Mock(return_value=process)
        with patch.object(controls.os.path, "isfile", return_value=True), patch.object(
            controls.subprocess, "Popen", popen
        ), patch.object(
            controls, "_query_process_identity", return_value=identity()
        ), patch("builtins.open", mock_open()):
            controls.OwnedProcess.start(
                [r"C:\Tools\fixture.exe"],
                r"C:\Run",
                r"C:\Evidence\stdout.log",
                r"C:\Evidence\stderr.log",
                visible=True,
            )

        self.assertIsNone(popen.call_args.kwargs["startupinfo"])

    def test_snapshot_failure_is_not_reported_as_empty_inventory(self):
        api = Mock()
        api.process_entries.side_effect = OSError("fake")
        with patch.object(controls, "_api", return_value=api):
            result = controls.inventory_known_executables([r"C:\Tools\fixture.exe"])

        self.assertEqual([], result["processes"])
        self.assertEqual("process_inventory_failed", result["error_code"])

    def test_matching_basename_with_unresolved_identity_fails_closed(self):
        api = Mock()
        api.process_entries.return_value = [
            (4, "System"),
            (4321, "fixture.exe"),
        ]
        api.process_identity.return_value = None
        with patch.object(controls, "_api", return_value=api), patch.object(
            controls.time, "sleep"
        ):
            result = controls.inventory_known_executables([r"C:\Tools\fixture.exe"])

        self.assertEqual([], result["processes"])
        self.assertEqual("candidate_identity_unavailable", result["error_code"])
        self.assertEqual(5, api.process_identity.call_count)

    def test_matching_process_that_exits_during_identity_query_is_retried(self):
        api = Mock()
        api.process_entries.side_effect = [
            [(4321, "fixture.exe")],
            [],
        ]
        api.process_identity.return_value = None
        with patch.object(controls, "_api", return_value=api), patch.object(
            controls.time, "sleep"
        ):
            result = controls.inventory_known_executables([r"C:\Tools\fixture.exe"])

        self.assertEqual({"processes": [], "error_code": None}, result)
        api.process_identity.assert_called_once_with(4321)

    def test_inaccessible_unrelated_process_does_not_fail_inventory(self):
        api = Mock()
        api.process_entries.return_value = [(4, "System"), (500, "lsass.exe")]
        with patch.object(controls, "_api", return_value=api):
            result = controls.inventory_known_executables([r"C:\Tools\fixture.exe"])

        self.assertEqual({"processes": [], "error_code": None}, result)
        api.process_identity.assert_not_called()


if __name__ == "__main__":
    unittest.main()
