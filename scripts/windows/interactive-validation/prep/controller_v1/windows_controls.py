"""Narrow Windows controls for a future interactive validation orchestrator.

Importing this module does not load Windows DLLs, inspect the desktop, or start
processes.  The public process controls operate only on a process started by
``OwnedProcess.start`` and revalidate its executable path and creation time
before signalling or terminating it.
"""

from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Callable, Iterable, Mapping, Sequence


CREATE_NEW_CONSOLE = 0x00000010
CREATE_NO_WINDOW = 0x08000000
STARTF_USESHOWWINDOW = 0x00000001
SW_HIDE = 0

PROCESS_TERMINATE = 0x0001
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
SYNCHRONIZE = 0x00100000
TH32CS_SNAPPROCESS = 0x00000002
INVALID_HANDLE_VALUE = ctypes.c_void_p(-1).value


@dataclass(frozen=True)
class ProcessIdentity:
    pid: int
    executable_path: str
    creation_time_ticks: int


def _error_result(code: str | None, *, stopped: bool = False, forced: bool = False,
                  exit_code: int | None = None) -> dict[str, object]:
    return {
        "stopped": stopped,
        "forced": forced,
        "exit_code": exit_code,
        "error_code": code,
    }


def _normalize_path(path: os.PathLike[str] | str) -> str:
    return os.path.normcase(os.path.realpath(os.path.abspath(os.fspath(path))))


def session_state() -> dict[str, object]:
    """Return validated WTS lock state without exposing desktop metadata."""
    result: dict[str, object] = {
        "state": "unknown",
        "session": None,
        "error_code": None,
    }
    try:
        probe_path = Path(__file__).resolve().parent.parent / "lock_probe.py"
        spec = importlib.util.spec_from_file_location(
            "screenwise_interactive_lock_probe", probe_path
        )
        if spec is None or spec.loader is None:
            result["error_code"] = "lock_probe_unavailable"
            return result
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        raw = module.probe()
    except Exception:
        result["error_code"] = "lock_probe_failed"
        return result

    session = raw.get("session")
    if type(session) is int and session >= 0:
        result["session"] = session
    else:
        result["error_code"] = "invalid_session"
        return result

    required = ("wts_level", "wts_session", "wts_flags", "wts_state")
    if any(type(raw.get(key)) is not int for key in required):
        result["error_code"] = "incomplete_wts_state"
        return result
    if raw["wts_level"] != 1 or raw["wts_session"] != session:
        result["error_code"] = "mismatched_wts_state"
        return result
    if raw["wts_state"] != 0:  # WTSActive
        result["error_code"] = "inactive_wts_session"
        return result
    if raw["wts_flags"] == 0:  # WTS_SESSIONSTATE_LOCK
        result["state"] = "locked"
    elif raw["wts_flags"] == 1:  # WTS_SESSIONSTATE_UNLOCK
        desktop = raw.get("desktop_access_256")
        if not isinstance(desktop, dict) or desktop.get("available") is not True:
            result["error_code"] = "secure_desktop_unavailable"
            return result
        result["state"] = "unlocked"
    else:
        result["error_code"] = "unknown_wts_flag"
    return result


class _WindowsApi:
    """Lazy ctypes bindings. Construct only inside an explicit operation."""

    class PROCESSENTRY32W(ctypes.Structure):
        _fields_ = [
            ("dwSize", wintypes.DWORD),
            ("cntUsage", wintypes.DWORD),
            ("th32ProcessID", wintypes.DWORD),
            ("th32DefaultHeapID", ctypes.c_size_t),
            ("th32ModuleID", wintypes.DWORD),
            ("cntThreads", wintypes.DWORD),
            ("th32ParentProcessID", wintypes.DWORD),
            ("pcPriClassBase", wintypes.LONG),
            ("dwFlags", wintypes.DWORD),
            ("szExeFile", wintypes.WCHAR * 260),
        ]

    def __init__(self) -> None:
        self.kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        self.user32 = ctypes.WinDLL("user32", use_last_error=True)

        k = self.kernel32
        k.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        k.OpenProcess.restype = wintypes.HANDLE
        k.CloseHandle.argtypes = [wintypes.HANDLE]
        k.CloseHandle.restype = wintypes.BOOL
        k.QueryFullProcessImageNameW.argtypes = [
            wintypes.HANDLE,
            wintypes.DWORD,
            wintypes.LPWSTR,
            ctypes.POINTER(wintypes.DWORD),
        ]
        k.QueryFullProcessImageNameW.restype = wintypes.BOOL
        k.GetProcessTimes.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(wintypes.FILETIME),
            ctypes.POINTER(wintypes.FILETIME),
            ctypes.POINTER(wintypes.FILETIME),
            ctypes.POINTER(wintypes.FILETIME),
        ]
        k.GetProcessTimes.restype = wintypes.BOOL
        k.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
        k.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
        k.Process32FirstW.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(self.PROCESSENTRY32W),
        ]
        k.Process32FirstW.restype = wintypes.BOOL
        k.Process32NextW.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(self.PROCESSENTRY32W),
        ]
        k.Process32NextW.restype = wintypes.BOOL
        k.FreeConsole.argtypes = []
        k.FreeConsole.restype = wintypes.BOOL
        k.AttachConsole.argtypes = [wintypes.DWORD]
        k.AttachConsole.restype = wintypes.BOOL
        k.SetConsoleCtrlHandler.argtypes = [ctypes.c_void_p, wintypes.BOOL]
        k.SetConsoleCtrlHandler.restype = wintypes.BOOL
        k.GenerateConsoleCtrlEvent.argtypes = [wintypes.DWORD, wintypes.DWORD]
        k.GenerateConsoleCtrlEvent.restype = wintypes.BOOL

        u = self.user32
        u.GetForegroundWindow.argtypes = []
        u.GetForegroundWindow.restype = wintypes.HWND
        u.GetWindowThreadProcessId.argtypes = [
            wintypes.HWND,
            ctypes.POINTER(wintypes.DWORD),
        ]
        u.GetWindowThreadProcessId.restype = wintypes.DWORD
        u.IsWindowVisible.argtypes = [wintypes.HWND]
        u.IsWindowVisible.restype = wintypes.BOOL
        self.WNDENUMPROC = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
        u.EnumWindows.argtypes = [self.WNDENUMPROC, wintypes.LPARAM]
        u.EnumWindows.restype = wintypes.BOOL

    def process_identity(self, pid: int) -> ProcessIdentity | None:
        handle = self.open_identity_handle(pid)
        if not handle:
            return None
        try:
            return self.process_identity_from_handle(handle, pid)
        finally:
            self.kernel32.CloseHandle(handle)

    def open_identity_handle(self, pid: int) -> int | None:
        return self.kernel32.OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, False, pid
        )

    def process_identity_from_handle(
        self, handle: int, pid: int
    ) -> ProcessIdentity | None:
        try:
            capacity = wintypes.DWORD(32768)
            path = ctypes.create_unicode_buffer(capacity.value)
            if not self.kernel32.QueryFullProcessImageNameW(
                handle, 0, path, ctypes.byref(capacity)
            ):
                return None
            created = wintypes.FILETIME()
            exited = wintypes.FILETIME()
            kernel = wintypes.FILETIME()
            user = wintypes.FILETIME()
            if not self.kernel32.GetProcessTimes(
                handle,
                ctypes.byref(created),
                ctypes.byref(exited),
                ctypes.byref(kernel),
                ctypes.byref(user),
            ):
                return None
            ticks = (created.dwHighDateTime << 32) | created.dwLowDateTime
            return ProcessIdentity(pid, _normalize_path(path.value), ticks)
        except (OSError, ValueError):
            return None

    def foreground_identity(self) -> tuple[int, int]:
        hwnd = self.user32.GetForegroundWindow()
        if not hwnd:
            return 0, 0
        pid = wintypes.DWORD()
        self.user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        return int(pid.value), int(hwnd)

    def visible_window_count(self, pid: int) -> int | None:
        count = 0

        @self.WNDENUMPROC
        def callback(hwnd: int, _lparam: int) -> bool:
            nonlocal count
            window_pid = wintypes.DWORD()
            self.user32.GetWindowThreadProcessId(hwnd, ctypes.byref(window_pid))
            if window_pid.value == pid and self.user32.IsWindowVisible(hwnd):
                count += 1
            return True

        ctypes.set_last_error(0)
        if not self.user32.EnumWindows(callback, 0):
            return None
        return count

    def process_entries(self) -> list[tuple[int, str]]:
        """Return Toolhelp PID/basename metadata without opening processes."""
        snapshot = self.kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
        if not snapshot or snapshot == INVALID_HANDLE_VALUE:
            raise OSError("process_snapshot_failed")
        try:
            entry = self.PROCESSENTRY32W()
            entry.dwSize = ctypes.sizeof(entry)
            entries: list[tuple[int, str]] = []
            ok = self.kernel32.Process32FirstW(snapshot, ctypes.byref(entry))
            while ok:
                entries.append((int(entry.th32ProcessID), str(entry.szExeFile)))
                ok = self.kernel32.Process32NextW(snapshot, ctypes.byref(entry))
            return entries
        finally:
            self.kernel32.CloseHandle(snapshot)


def _api() -> _WindowsApi:
    return _WindowsApi()


def _query_process_identity(pid: int, api: _WindowsApi | None = None) -> ProcessIdentity | None:
    if pid <= 0:
        return None
    try:
        return (api or _api()).process_identity(pid)
    except (OSError, AttributeError):
        return None


def foreground_pid() -> int:
    """Return the foreground process id, or zero when unavailable."""
    return int(foreground_identity()["pid"])


def foreground_identity() -> dict[str, int]:
    """Return foreground PID/HWND metadata without reading any window text."""
    try:
        pid, hwnd = _api().foreground_identity()
        return {"pid": pid, "hwnd": hwnd}
    except (OSError, AttributeError):
        return {"pid": 0, "hwnd": 0}


def _window_visibility(pid: int, api: _WindowsApi | None = None) -> dict[str, object]:
    try:
        count = (api or _api()).visible_window_count(pid)
    except (OSError, AttributeError):
        count = None
    return {
        "pid": pid,
        "visible_window_count": count,
        "has_visible_window": bool(count) if count is not None else None,
        "error_code": None if count is not None else "window_enumeration_failed",
    }


def owned_window_visibility_info(pid: int) -> dict[str, object]:
    """Inspect visible top-level windows only for this controller process."""
    if pid != os.getpid():
        return {
            "pid": pid,
            "visible_window_count": None,
            "has_visible_window": None,
            "error_code": "pid_not_self",
        }
    return _window_visibility(pid)


def _identity_matches(actual: ProcessIdentity | None, expected: ProcessIdentity) -> bool:
    return (
        actual is not None
        and actual.pid == expected.pid
        and actual.creation_time_ticks == expected.creation_time_ticks
        and _normalize_path(actual.executable_path) == _normalize_path(expected.executable_path)
    )


class OwnedProcess:
    """A subprocess plus immutable Windows identity used for guarded controls."""

    def __init__(
        self,
        process: subprocess.Popen[bytes],
        identity: ProcessIdentity,
        *,
        run_helper: Callable[..., subprocess.CompletedProcess[bytes]] = subprocess.run,
        identity_query: Callable[[int], ProcessIdentity | None] = _query_process_identity,
        window_query: Callable[[int], dict[str, object]] = _window_visibility,
    ) -> None:
        self._process = process
        self._identity = identity
        self._run_helper = run_helper
        self._identity_query = identity_query
        self._window_query = window_query

    @classmethod
    def start(
        cls,
        argv: Sequence[os.PathLike[str] | str],
        cwd: os.PathLike[str] | str,
        stdout_path: os.PathLike[str] | str,
        stderr_path: os.PathLike[str] | str,
        *,
        visible: bool = False,
    ) -> "OwnedProcess":
        if not argv:
            raise ValueError("argv must contain an executable")
        raw_executable = os.fspath(argv[0])
        if not os.path.isabs(raw_executable):
            raise ValueError("owned executable path must be absolute")
        executable = _normalize_path(raw_executable)
        if not os.path.isfile(executable):
            raise FileNotFoundError("owned executable was not found")
        startup = None
        if not visible:
            startup = subprocess.STARTUPINFO()
            startup.dwFlags |= STARTF_USESHOWWINDOW
            startup.wShowWindow = SW_HIDE
        with open(stdout_path, "xb", buffering=0) as stdout_file, open(
            stderr_path, "xb", buffering=0
        ) as stderr_file:
            process = subprocess.Popen(
                [os.fspath(value) for value in argv],
                executable=executable,
                cwd=os.fspath(cwd),
                stdin=subprocess.DEVNULL,
                stdout=stdout_file,
                stderr=stderr_file,
                close_fds=True,
                creationflags=CREATE_NEW_CONSOLE,
                startupinfo=startup,
            )

        identity = _query_process_identity(process.pid)
        expected = ProcessIdentity(process.pid, executable, identity.creation_time_ticks if identity else 0)
        if not identity or not _identity_matches(identity, expected):
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
            raise RuntimeError("owned process identity could not be established")
        return cls(process, identity)

    @property
    def pid(self) -> int:
        return int(self._process.pid)

    @property
    def executable_path(self) -> str:
        return self._identity.executable_path

    @property
    def creation_time_ticks(self) -> int:
        return self._identity.creation_time_ticks

    def poll(self) -> int | None:
        return self._process.poll()

    def _still_owned(self) -> bool:
        return _identity_matches(self._identity_query(self.pid), self._identity)

    def window_visibility(self) -> dict[str, object]:
        if not self._still_owned():
            return {
                "pid": self.pid,
                "visible_window_count": None,
                "has_visible_window": None,
                "error_code": "identity_mismatch",
            }
        result = self._window_query(self.pid)
        if not self._still_owned():
            return {
                "pid": self.pid,
                "visible_window_count": None,
                "has_visible_window": None,
                "error_code": "identity_changed_during_enumeration",
            }
        return result

    def terminate_owned(self, timeout: float = 5.0) -> dict[str, object]:
        existing = self.poll()
        if existing is not None:
            return _error_result("already_exited", stopped=True, exit_code=existing)
        if not self._still_owned():
            return _error_result("identity_mismatch")
        self._process.terminate()
        try:
            code = self._process.wait(timeout=max(0.0, timeout))
        except subprocess.TimeoutExpired:
            return _error_result("terminate_timeout", forced=True)
        return _error_result("forced_terminate", stopped=True, forced=True, exit_code=code)

    def graceful_stop(self, timeout: float = 10.0) -> dict[str, object]:
        existing = self.poll()
        if existing is not None:
            return _error_result("already_exited", stopped=True, exit_code=existing)
        if not self._still_owned():
            return _error_result("identity_mismatch")

        timeout = max(0.0, float(timeout))
        started = time.monotonic()
        command = [
            sys.executable,
            str(Path(__file__).resolve()),
            "--signal-owned",
            str(self.pid),
            self.executable_path,
            str(self.creation_time_ticks),
        ]
        try:
            helper = self._run_helper(
                command,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=min(timeout, 5.0),
                check=False,
                creationflags=CREATE_NO_WINDOW,
            )
        except subprocess.TimeoutExpired:
            helper = None
        except OSError:
            helper = None

        remaining = max(0.0, timeout - (time.monotonic() - started))
        if helper is not None and helper.returncode == 0:
            try:
                code = self._process.wait(timeout=remaining)
                return _error_result(None, stopped=True, exit_code=code)
            except subprocess.TimeoutExpired:
                pass

        fallback = self.terminate_owned(timeout=min(5.0, remaining))
        if fallback["stopped"] and fallback["forced"]:
            if helper is None:
                fallback["error_code"] = "signal_helper_failed_forced"
            elif helper.returncode != 0:
                fallback["error_code"] = "signal_rejected_forced"
            else:
                fallback["error_code"] = "graceful_timeout_forced"
        elif fallback["stopped"] and fallback["error_code"] == "already_exited":
            fallback["error_code"] = None
        return fallback


def parse_netstat_output(text: str, scoped_pids: Iterable[int]) -> list[dict[str, object]]:
    """Parse netstat -ano output and retain only exact requested process ids."""
    allowed = {int(pid) for pid in scoped_pids if int(pid) > 0}
    endpoints: list[dict[str, object]] = []
    for raw_line in text.splitlines():
        fields = raw_line.split()
        if not fields or fields[0].upper() not in {"TCP", "UDP"}:
            continue
        protocol = fields[0].upper()
        minimum = 5 if protocol == "TCP" else 4
        if len(fields) < minimum:
            continue
        try:
            pid = int(fields[-1])
        except ValueError:
            continue
        if pid not in allowed:
            continue
        local_host, local_port = _split_endpoint(fields[1])
        remote_host, remote_port = _split_endpoint(fields[2])
        endpoints.append(
            {
                "protocol": protocol.lower(),
                "local_address": local_host,
                "local_port": local_port,
                "remote_address": remote_host,
                "remote_port": remote_port,
                "state": fields[3].lower() if protocol == "TCP" else None,
                "pid": pid,
            }
        )
    return endpoints


def _split_endpoint(value: str) -> tuple[str, int | None]:
    if value.startswith("[") and "]:" in value:
        host, port_text = value.rsplit(":", 1)
        host = host[1:-1]
    elif ":" in value:
        host, port_text = value.rsplit(":", 1)
    else:
        return value, None
    try:
        port = int(port_text)
    except ValueError:
        port = None
    return host, port


def inspect_network_endpoints(scoped_pids: Iterable[int]) -> dict[str, object]:
    pids = {int(pid) for pid in scoped_pids if int(pid) > 0}
    if not pids:
        return {"endpoints": [], "error_code": None}
    try:
        completed = subprocess.run(
            ["netstat.exe", "-ano"],
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=10,
            check=False,
            creationflags=CREATE_NO_WINDOW,
        )
    except (OSError, subprocess.TimeoutExpired):
        return {"endpoints": [], "error_code": "netstat_failed"}
    if completed.returncode != 0:
        return {"endpoints": [], "error_code": "netstat_nonzero"}
    return {"endpoints": parse_netstat_output(completed.stdout, pids), "error_code": None}


def inventory_known_executables(
    known_paths: Iterable[os.PathLike[str] | str],
) -> dict[str, object]:
    """Inventory exact paths and fail closed on unresolved matching basenames."""
    known: Mapping[str, str] = {
        _normalize_path(path): os.path.abspath(os.fspath(path)) for path in known_paths
    }
    candidate_names = {os.path.basename(path).casefold() for path in known}
    matches: list[dict[str, object]] = []
    try:
        api = _api()
        for pid, executable_name in api.process_entries():
            if executable_name.casefold() not in candidate_names:
                continue
            identity = api.process_identity(pid)
            if identity is None:
                return {
                    "processes": matches,
                    "error_code": "candidate_identity_unavailable",
                }
            if identity.executable_path in known:
                matches.append(
                    {
                        "pid": pid,
                        "executable_path": known[identity.executable_path],
                        "creation_time_ticks": identity.creation_time_ticks,
                    }
                )
    except (OSError, AttributeError):
        return {"processes": [], "error_code": "process_inventory_failed"}
    matches.sort(
        key=lambda item: (str(item["executable_path"]).casefold(), int(item["pid"]))
    )
    return {"processes": matches, "error_code": None}


def _signal_owned(pid: int, expected_path: str, creation_time_ticks: int) -> int:
    """Helper entrypoint: revalidate identity, attach, and signal one process group."""
    try:
        api = _api()
        expected = ProcessIdentity(pid, _normalize_path(expected_path), creation_time_ticks)
        handle = api.open_identity_handle(pid)
        if not handle:
            return 20
        try:
            if not _identity_matches(api.process_identity_from_handle(handle, pid), expected):
                return 20
            # Holding the process handle prevents PID reuse between validation
            # and signalling. The target owns a private console created by start().
            api.kernel32.FreeConsole()
            if not api.kernel32.AttachConsole(pid):
                return 21
            try:
                if not api.kernel32.SetConsoleCtrlHandler(None, True):
                    return 22
                # The owned process has its own console. Group zero signals only
                # processes attached to that private console.
                if not api.kernel32.GenerateConsoleCtrlEvent(0, 0):  # CTRL_C_EVENT
                    return 23
                time.sleep(0.1)
                return 0
            finally:
                api.kernel32.FreeConsole()
        finally:
            api.kernel32.CloseHandle(handle)
    except (OSError, AttributeError, ValueError):
        return 24


def _main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--signal-owned", action="store_true")
    parser.add_argument("pid", type=int)
    parser.add_argument("expected_path")
    parser.add_argument("creation_time_ticks", type=int)
    args = parser.parse_args(argv)
    if not args.signal_owned or args.pid <= 0 or args.creation_time_ticks <= 0:
        return 2
    return _signal_owned(args.pid, args.expected_path, args.creation_time_ticks)


if __name__ == "__main__":
    raise SystemExit(_main())
