"""Explicit runtime controls for the prepared synthetic browser fixture.

Importing this module is inert: it does not bind a socket, inspect processes,
create a profile, launch Chrome, or touch the clipboard.  A caller must first
start :class:`BrowserFixtureServer`, then explicitly call
:meth:`OwnedChrome.launch`.  The server binds only to IPv4 loopback, and the
browser launch has one fixed, reviewed flag set with a fresh profile.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import re
import threading
import time
from typing import Any, Callable, Mapping, Sequence
from urllib.parse import parse_qsl, urlencode, urlsplit, urlunsplit

import windows_controls
from fixture_transport import route_browser_fixture


LOOPBACK_HOST = "127.0.0.1"
ALLOWED_BROWSER_HOSTS = frozenset((LOOPBACK_HOST, "localhost"))
BROWSER_FIXTURE_PATH = "/browser-fixture.html"
_SAFE_ID = re.compile(r"^[A-Za-z0-9._-]{1,80}$")
_ALLOWED_QUERY_KEYS = frozenset(("runId", "phaseId", "case", "focus"))

# These are the only Chrome switches this adapter may add.  The dynamic
# user-data-dir and fixture URL are constructed separately and cannot be
# replaced by caller-provided arbitrary flags.
REVIEWED_CHROME_FLAGS = (
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    "--disable-component-update",
    "--disable-extensions",
    "--disable-sync",
    "--disable-translate",
    "--no-proxy-server",
    "--new-window",
    "--window-size=900,700",
    "--window-position=80,80",
)


class BrowserRuntimeError(RuntimeError):
    """A browser runtime precondition or ownership check failed."""


def _absolute_file(path: os.PathLike[str] | str, label: str) -> str:
    raw = os.fspath(path)
    if not os.path.isabs(raw):
        raise BrowserRuntimeError(f"{label}_must_be_absolute")
    value = os.path.abspath(raw)
    if not os.path.isfile(value):
        raise BrowserRuntimeError(f"{label}_not_found")
    return os.path.normcase(os.path.realpath(value))


def _absolute_path(path: os.PathLike[str] | str, label: str) -> Path:
    raw = os.fspath(path)
    if not os.path.isabs(raw):
        raise BrowserRuntimeError(f"{label}_must_be_absolute")
    return Path(os.path.abspath(raw)).resolve()


def _validate_port(port: int) -> int:
    if isinstance(port, bool) or not isinstance(port, int) or not 0 <= port <= 65535:
        raise BrowserRuntimeError("invalid_loopback_port")
    return port


def _validate_fixture_url(url: str) -> None:
    try:
        parsed = urlsplit(url)
    except ValueError as exc:
        raise BrowserRuntimeError("invalid_browser_url") from exc
    if (
        parsed.scheme != "http"
        or parsed.hostname not in ALLOWED_BROWSER_HOSTS
        or parsed.port is None
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path != BROWSER_FIXTURE_PATH
        or parsed.fragment
    ):
        raise BrowserRuntimeError("browser_url_not_loopback_fixture")
    try:
        pairs = parse_qsl(parsed.query, keep_blank_values=True, strict_parsing=True)
    except ValueError as exc:
        raise BrowserRuntimeError("invalid_browser_query") from exc
    if any(key not in _ALLOWED_QUERY_KEYS for key, _value in pairs):
        raise BrowserRuntimeError("invalid_browser_query")
    if len({key for key, _value in pairs}) != len(pairs):
        raise BrowserRuntimeError("duplicate_browser_query")
    for key, value in pairs:
        if key in ("runId", "phaseId") and not _SAFE_ID.fullmatch(value):
            raise BrowserRuntimeError("invalid_browser_query")
        if key == "case" and value != "excluded":
            raise BrowserRuntimeError("invalid_browser_query")
        if key == "focus" and value not in ("ordinary", "secret"):
            raise BrowserRuntimeError("invalid_browser_query")


def _fixture_html() -> bytes:
    path = Path(__file__).resolve().parent.parent / "fixtures" / "browser-fixture.html"
    try:
        body = path.read_bytes()
    except OSError as exc:
        raise BrowserRuntimeError("browser_fixture_not_found") from exc
    if not body:
        raise BrowserRuntimeError("browser_fixture_empty")
    return body


def _handler_factory(browser_html: bytes):
    if not isinstance(browser_html, bytes) or not browser_html:
        raise BrowserRuntimeError("browser_fixture_invalid")

    class BrowserFixtureHandler(BaseHTTPRequestHandler):
        def handle(self):
            try:
                super().handle()
            except (ConnectionResetError, BrokenPipeError):
                # Chrome may reset an idle keep-alive connection while a phase
                # closes. This carries no request data and is not a test fault.
                return

        def do_GET(self):  # noqa: N802
            port = int(self.server.server_address[1])
            status, headers, body = route_browser_fixture(
                self.path,
                self.headers.get("Host", ""),
                port,
                browser_html,
            )
            self.send_response(status)
            for key, value in headers.items():
                self.send_header(key, value)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_args):
            return

    return BrowserFixtureHandler


class BrowserFixtureServer:
    """A loopback-only server for the existing self-contained HTML fixture."""

    def __init__(
        self,
        port: int = 0,
        *,
        browser_html: bytes | None = None,
        server_factory: Callable[..., Any] = ThreadingHTTPServer,
        thread_factory: Callable[..., Any] = threading.Thread,
    ) -> None:
        self._requested_port = _validate_port(port)
        self._browser_html = browser_html
        self._server_factory = server_factory
        self._thread_factory = thread_factory
        self._server: Any | None = None
        self._thread: Any | None = None
        self._port: int | None = None

    @property
    def started(self) -> bool:
        return self._server is not None

    @property
    def port(self) -> int:
        if self._port is None:
            raise BrowserRuntimeError("browser_server_not_started")
        return self._port

    def url(self, *, run_id: str | None = None, phase_id: str | None = None,
            excluded: bool = False) -> str:
        query: list[tuple[str, str]] = []
        if run_id is not None:
            if not _SAFE_ID.fullmatch(run_id):
                raise BrowserRuntimeError("invalid_browser_query")
            query.append(("runId", run_id))
        if phase_id is not None:
            if not _SAFE_ID.fullmatch(phase_id):
                raise BrowserRuntimeError("invalid_browser_query")
            query.append(("phaseId", phase_id))
        if excluded:
            query.append(("case", "excluded"))
        return urlunsplit(("http", f"{LOOPBACK_HOST}:{self.port}", BROWSER_FIXTURE_PATH,
                          urlencode(query), ""))

    def start(self) -> str:
        if self.started:
            return self.url()
        browser_html = self._browser_html if self._browser_html is not None else _fixture_html()
        if not isinstance(browser_html, bytes) or not browser_html:
            raise BrowserRuntimeError("browser_fixture_invalid")
        handler = _handler_factory(browser_html)
        server: Any | None = None
        try:
            server = self._server_factory((LOOPBACK_HOST, self._requested_port), handler)
            address = server.server_address
            if (
                not isinstance(address, tuple)
                or len(address) != 2
                or address[0] != LOOPBACK_HOST
                or isinstance(address[1], bool)
                or not isinstance(address[1], int)
                or address[1] <= 0
                or address[1] > 65535
            ):
                raise BrowserRuntimeError("server_not_bound_to_loopback")
            server.daemon_threads = True
            server.block_on_close = False
            thread = self._thread_factory(
                target=server.serve_forever,
                name="screenwise-browser-fixture",
                daemon=True,
            )
            thread.start()
        except BrowserRuntimeError:
            try:
                if server is not None:
                    server.server_close()
            except (AttributeError, OSError):
                pass
            raise
        except (OSError, TypeError, ValueError) as exc:
            try:
                if server is not None:
                    server.server_close()
            except (AttributeError, OSError):
                pass
            raise BrowserRuntimeError("browser_server_start_failed") from exc
        self._server = server
        self._thread = thread
        self._port = int(address[1])
        return self.url()

    def stop(self, timeout: float = 5.0) -> dict[str, object]:
        server, thread = self._server, self._thread
        if server is None:
            return {"stopped": True, "error_code": "already_stopped"}
        errors: list[str] = []
        try:
            server.shutdown()
        except (OSError, RuntimeError):
            errors.append("server_shutdown_failed")
        try:
            server.server_close()
        except OSError:
            errors.append("server_close_failed")
        try:
            thread.join(timeout=max(0.0, float(timeout)))
        except (AttributeError, TypeError, ValueError):
            errors.append("server_thread_join_failed")
        alive = bool(getattr(thread, "is_alive", lambda: False)())
        if alive:
            errors.append("server_thread_alive")
        else:
            self._server = None
            self._thread = None
            self._port = None
        return {"stopped": not errors, "error_code": errors[0] if errors else None}


@dataclass(frozen=True)
class OwnedChrome:
    """A Chrome root process guarded by ``windows_controls.OwnedProcess``."""

    process: Any
    executable_path: str
    profile_dir: Path
    url: str
    inventory: Callable[[Sequence[os.PathLike[str] | str]], Mapping[str, Any]] = field(
        default=windows_controls.inventory_known_executables, repr=False
    )

    @classmethod
    def launch(
        cls,
        executable: os.PathLike[str] | str,
        profile_dir: os.PathLike[str] | str,
        url: str,
        stdout_path: os.PathLike[str] | str,
        stderr_path: os.PathLike[str] | str,
        *,
        process_start: Callable[..., Any] = windows_controls.OwnedProcess.start,
        inventory: Callable[[Sequence[os.PathLike[str] | str]], Mapping[str, Any]] = windows_controls.inventory_known_executables,
    ) -> "OwnedChrome":
        _validate_fixture_url(url)
        executable_path = _absolute_file(executable, "browser_executable")
        profile = _absolute_path(profile_dir, "browser_profile")
        stdout = _absolute_path(stdout_path, "browser_stdout")
        stderr = _absolute_path(stderr_path, "browser_stderr")
        if profile.exists():
            raise BrowserRuntimeError("browser_profile_must_be_fresh")
        if stdout.exists() or stderr.exists():
            raise BrowserRuntimeError("browser_log_must_be_fresh")
        if not stdout.parent.is_dir() or not stderr.parent.is_dir():
            raise BrowserRuntimeError("browser_log_directory_missing")

        before = inventory([executable_path])
        if not isinstance(before, Mapping):
            raise BrowserRuntimeError("browser_inventory_failed")
        if before.get("error_code") is not None:
            raise BrowserRuntimeError("browser_inventory_failed")
        if before.get("processes"):
            raise BrowserRuntimeError("browser_executable_already_running")

        try:
            profile.mkdir(parents=True, exist_ok=False)
            argv = [
                executable_path,
                *REVIEWED_CHROME_FLAGS,
                f"--user-data-dir={profile}",
                url,
            ]
            owned = process_start(argv, Path(executable_path).parent, stdout, stderr, visible=True)
        except BrowserRuntimeError:
            raise
        except (OSError, RuntimeError, TypeError, ValueError) as exc:
            raise BrowserRuntimeError("browser_launch_failed") from exc

        if (
            _normalize_process_path(getattr(owned, "executable_path", "")) != executable_path
            or int(getattr(owned, "pid", 0)) <= 0
        ):
            _stop_owned(owned)
            raise BrowserRuntimeError("browser_identity_unverified")

        after = inventory([executable_path])
        if not isinstance(after, Mapping):
            _stop_owned(owned)
            raise BrowserRuntimeError("browser_inventory_failed")
        if after.get("error_code") is not None:
            _stop_owned(owned)
            raise BrowserRuntimeError("browser_inventory_failed")
        if not _inventory_contains_owned(after.get("processes"), owned, executable_path):
            _stop_owned(owned)
            raise BrowserRuntimeError("browser_identity_unverified")
        return cls(owned, executable_path, profile, url, inventory)

    @property
    def pid(self) -> int:
        return int(self.process.pid)

    def stop(self, timeout: float = 10.0) -> dict[str, object]:
        """Stop the root and allow its exact-path children a bounded drain."""
        result = self.process.graceful_stop(timeout=timeout)
        if result.get("stopped") is not True:
            return result
        deadline = time.monotonic() + max(0.0, float(timeout))
        error_code = "owned_browser_children_remain"
        while True:
            remaining = self.inventory([self.executable_path])
            if not isinstance(remaining, Mapping) or remaining.get("error_code") is not None:
                error_code = "browser_inventory_failed_after_stop"
            elif not remaining.get("processes"):
                return result
            else:
                error_code = "owned_browser_children_remain"
            if time.monotonic() >= deadline:
                return {
                    "stopped": False,
                    "forced": bool(result.get("forced")),
                    "exit_code": result.get("exit_code"),
                    "error_code": error_code,
                }
            time.sleep(0.05)


def _normalize_process_path(path: os.PathLike[str] | str) -> str:
    if not path:
        return ""
    return os.path.normcase(os.path.realpath(os.path.abspath(os.fspath(path))))


def _inventory_contains_owned(entries: Any, owned: Any, executable_path: str) -> bool:
    if not isinstance(entries, list):
        return False
    for entry in entries:
        if not isinstance(entry, dict):
            continue
        if (
            entry.get("pid") == int(owned.pid)
            and _normalize_process_path(entry.get("executable_path", "")) == executable_path
            and isinstance(entry.get("creation_time_ticks"), int)
            and entry["creation_time_ticks"] == int(owned.creation_time_ticks)
        ):
            return True
    return False


def _stop_owned(owned: Any) -> None:
    try:
        owned.terminate_owned(timeout=5)
    except (AttributeError, OSError, RuntimeError):
        return


class BrowserFixtureRuntime:
    """Own one loopback fixture server and one browser phase at a time.

    This facade is intentionally separate from the live controller.  Creating
    it performs no I/O.  ``start_server`` and ``show_phase`` are the explicit
    operations that bind the listener and launch a dedicated Chrome profile.
    A new profile is used for each phase so every Chrome root remains
    identity-guarded and can be stopped without touching an existing browser.
    """

    _FOCUS_TARGETS = frozenset(("ordinary", "secret"))

    def __init__(
        self,
        browser_exe: os.PathLike[str] | str,
        server_port: int,
        fixture_html: os.PathLike[str] | str | bytes,
        run_root: os.PathLike[str] | str,
        run_id: str,
        *,
        process_start: Callable[..., Any] = windows_controls.OwnedProcess.start,
        inventory: Callable[[Sequence[os.PathLike[str] | str]], Mapping[str, Any]] = windows_controls.inventory_known_executables,
        foreground_query: Callable[[], Mapping[str, Any]] = windows_controls.foreground_identity,
        server_factory: Callable[..., Any] = ThreadingHTTPServer,
        thread_factory: Callable[..., Any] = threading.Thread,
    ) -> None:
        self.browser_exe = browser_exe
        self.server_port = _validate_port(server_port)
        if isinstance(fixture_html, bytes):
            self._fixture_html_path: Path | None = None
            self._fixture_html_bytes: bytes | None = fixture_html
        else:
            self._fixture_html_path = _absolute_path(fixture_html, "browser_fixture")
            self._fixture_html_bytes = None
        self.run_root = _absolute_path(run_root, "browser_run_root")
        if not _SAFE_ID.fullmatch(run_id):
            raise BrowserRuntimeError("invalid_run_id")
        self.run_id = run_id
        self._process_start = process_start
        self._inventory = inventory
        self._foreground_query = foreground_query
        self._server_factory = server_factory
        self._thread_factory = thread_factory
        self._server: BrowserFixtureServer | None = None
        self._browser: OwnedChrome | None = None
        self._owned_pids: set[int] = set()
        self._expected_hwnd: int | None = None

    @property
    def owned_pids(self) -> tuple[int, ...]:
        return tuple(sorted(self._owned_pids))

    def start_server(self) -> str:
        if self._server is None:
            html = self._fixture_html_bytes
            if html is None:
                try:
                    html = self._fixture_html_path.read_bytes()  # type: ignore[union-attr]
                except OSError as exc:
                    raise BrowserRuntimeError("browser_fixture_not_found") from exc
            self._server = BrowserFixtureServer(
                self.server_port,
                browser_html=html,
                server_factory=self._server_factory,
                thread_factory=self._thread_factory,
            )
        return self._server.start()

    def _phase_url(self, host: str, phase_id: str, focus_target: str) -> str:
        if host not in (LOOPBACK_HOST, "localhost"):
            raise BrowserRuntimeError("invalid_browser_host")
        if not _SAFE_ID.fullmatch(phase_id):
            raise BrowserRuntimeError("invalid_phase_id")
        if self._server is None or not self._server.started:
            raise BrowserRuntimeError("browser_server_not_started")
        if focus_target not in self._FOCUS_TARGETS:
            raise BrowserRuntimeError("invalid_focus_target")
        base = self._server.url(run_id=self.run_id, phase_id=phase_id)
        parsed = urlsplit(base)
        query = parse_qsl(parsed.query, keep_blank_values=True)
        query.append(("focus", focus_target))
        return urlunsplit((parsed.scheme, f"{host}:{self._server.port}", parsed.path,
                           urlencode(query), parsed.fragment))

    def _start_browser(self, url: str, phase_id: str) -> OwnedChrome:
        if not self.run_root.is_dir():
            raise BrowserRuntimeError("browser_run_root_missing")
        profile = self.run_root / f"browser-profile-{phase_id}"
        stdout = self.run_root / f"browser-{phase_id}-stdout.log"
        stderr = self.run_root / f"browser-{phase_id}-stderr.log"
        browser = OwnedChrome.launch(
            self.browser_exe,
            profile,
            url,
            stdout,
            stderr,
            process_start=self._process_start,
            inventory=self._inventory,
        )
        self._owned_pids.add(browser.pid)
        return browser

    def show_phase(self, host: str, phase_id: str, focus_target: str) -> dict[str, object]:
        if focus_target not in self._FOCUS_TARGETS:
            raise BrowserRuntimeError("invalid_focus_target")
        url = self._phase_url(host, phase_id, focus_target)
        if self._browser is not None:
            stopped = self.stop_browser()
            if stopped.get("stopped") is not True:
                raise BrowserRuntimeError("previous_browser_not_stopped")
        self._browser = self._start_browser(url, phase_id)
        deadline = time.monotonic() + 10.0
        pid = hwnd = None
        while time.monotonic() < deadline:
            foreground = self._foreground_query()
            if isinstance(foreground, Mapping):
                pid = foreground.get("pid")
                hwnd = foreground.get("hwnd")
                if (not isinstance(pid, bool) and isinstance(pid, int)
                        and not isinstance(hwnd, bool) and isinstance(hwnd, int)
                        and pid == self._browser.pid and hwnd > 0):
                    break
            if self._browser.process.poll() is not None:
                break
            time.sleep(0.05)
        else:
            pid = hwnd = None
        if pid != self._browser.pid or not isinstance(hwnd, int) or hwnd <= 0:
            self.stop_browser()
            raise BrowserRuntimeError("browser_foreground_unverified")
        self._expected_hwnd = hwnd
        return {"pid": pid, "hwnd": hwnd, "phase_id": phase_id, "focus_target": focus_target}

    def foreground_valid(self) -> bool:
        if self._browser is None or self._browser.process.poll() is not None:
            return False
        try:
            foreground = self._foreground_query()
        except (AttributeError, OSError):
            return False
        return (
            isinstance(foreground, Mapping)
            and foreground.get("pid") == self._browser.pid
            and foreground.get("hwnd") == self._expected_hwnd
            and self._browser.process.window_visibility().get("error_code") is None
        )

    def stop_browser(self) -> dict[str, object]:
        if self._browser is None:
            return {"stopped": True, "error_code": "already_stopped"}
        result = self._browser.stop()
        if result.get("stopped") is True:
            self._browser = None
            self._expected_hwnd = None
        return result

    def close(self) -> dict[str, object]:
        browser = self.stop_browser()
        server = self._server.stop() if self._server is not None else {
            "stopped": True,
            "error_code": "already_stopped",
        }
        return {
            "browser_stopped": browser.get("stopped") is True,
            "server_stopped": server.get("stopped") is True,
            "browser": browser,
            "server": server,
        }
