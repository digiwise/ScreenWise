"""Offline, aggregate-only evidence helpers for the interactive controller.

This module deliberately has no recorder, GUI, device, or subprocess control.
It accepts only a fresh controller-owned SQLite directory and returns counts.
"""

from __future__ import annotations

import json
import re
import sqlite3
from contextlib import closing
from collections import Counter
from pathlib import Path
from typing import Any, Iterable
from urllib.error import HTTPError
from urllib.parse import parse_qs, urlencode, urlsplit, urlunsplit
from urllib.request import Request, urlopen, build_opener, ProxyHandler, HTTPRedirectHandler

_IDENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
_LEGACY_NOTICE_REASONS = {
    "locked", "secure_desktop", "unknown_privacy_state", "privacy_gate",
    "drm", "excluded_window", "capture_failure", "persistence_degraded",
    "event_delivery_pressure", "event_delivery_loss",
} | {x.upper() for x in ("locked", "secure_desktop", "unknown_privacy_state", "privacy_gate", "drm", "excluded_window", "capture_failure", "persistence_degraded", "event_delivery_pressure", "event_delivery_loss")}
_WINDOWS_LOCK_REASONS = {
    "wts_session_locked", "wts_session_unlocked", "wts_session_disconnected",
    "input_desktop_unavailable", "process_session_query_failed", "wts_query_failed",
    "wts_short_buffer", "wts_unsupported_level", "wts_session_mismatch",
    "wts_session_state_unknown", "wts_session_flags_unknown", "wts_session_flags_invalid",
}
_AUDIO_SHUTDOWN_ISSUES = {
    "producer_stop_timeout", "producer_stop_failed", "consumer_drain_timeout",
    "consumer_drain_failed", "queued_work_discarded", "worker_completion_unconfirmed",
}
_CAPTURE_TABLES = {"frames", "audio_chunks", "audio_transcriptions", "ui_events", "ocr"}
_MARKER_COLUMNS = {
    "audio_transcriptions": {"transcription", "text", "content"},
    "frames": {"ocr_text", "accessibility_text", "accessibility_tree_json", "text"},
    "ui_events": {"text_content", "element_name", "value", "description", "window_title", "browser_url"},
}
_METRIC_FIELDS = {
    "chunks_sent", "chunks_received", "vad_passed", "vad_rejected",
    "db_inserted", "transcription_errors", "audio_level_rms",
}
_MARKER_LIMIT = 128


def _quote_ident(name: str) -> str:
    if not _IDENT.fullmatch(name):
        raise ValueError("unsafe SQL identifier")
    return '"' + name + '"'


def _columns(conn: sqlite3.Connection, table: str) -> set[str]:
    if not _IDENT.fullmatch(table):
        return set()
    return {row[1] for row in conn.execute(f"PRAGMA table_info({_quote_ident(table)})")}


def _count(conn: sqlite3.Connection, table: str) -> int:
    if not _IDENT.fullmatch(table):
        return 0
    try:
        return int(conn.execute(f"SELECT COUNT(*) FROM {_quote_ident(table)}").fetchone()[0])
    except sqlite3.DatabaseError:
        return 0


def _marker_hits(conn: sqlite3.Connection, table: str, markers: Iterable[str]) -> dict[str, int]:
    cols = _columns(conn, table)
    usable = [c for c in cols if c in _MARKER_COLUMNS.get(table, set())]
    if not usable:
        return {m: 0 for m in markers}
    out: dict[str, int] = {}
    for marker in markers:
        # Marker values are bound parameters; identifiers above are allowlisted.
        predicates = " OR ".join(f"instr(lower(CAST({_quote_ident(c)} AS TEXT)), lower(?)) > 0" for c in usable)
        out[marker] = int(conn.execute(f"SELECT COUNT(*) FROM {_quote_ident(table)} WHERE {predicates}", (marker,) * len(usable)).fetchone()[0])
    return out


def _classify_notice(reason_code: Any, text_content: Any) -> tuple[str, str]:
    """Classify a notice using fixed keys and enum values only."""
    if isinstance(reason_code, str) and reason_code in _LEGACY_NOTICE_REASONS:
        return "legacy", reason_code
    if not isinstance(text_content, str):
        return "unknown", "unknown"
    try:
        payload = json.loads(text_content)
    except (TypeError, ValueError):
        return "unknown", "unknown"
    if not isinstance(payload, dict) or len(payload) != 1:
        return "unknown", "unknown"
    reason = payload.get("reason")
    if isinstance(reason, str) and reason in _WINDOWS_LOCK_REASONS:
        return "windows_lock", reason
    issue = payload.get("issue")
    if isinstance(issue, str) and issue in _AUDIO_SHUTDOWN_ISSUES:
        return "audio_shutdown", issue
    return "unknown", "unknown"


def collect_db(data_dir: str | Path, synthetic_markers: list[str]) -> dict[str, Any]:
    """Return aggregate evidence from a fresh controller-owned database.

    A marker file is required in the supplied directory.  The helper never
    returns row text, titles, URLs, paths, tokens, or other captured payloads.
    """
    root = Path(data_dir).resolve()
    owner = root / ".controller-owned"
    db_path = root / "db.sqlite"
    if not root.is_dir() or not owner.is_file() or not db_path.is_file():
        raise ValueError("data directory is not a fresh controller-owned fixture")
    markers = list(dict.fromkeys(synthetic_markers))
    if len(markers) > _MARKER_LIMIT or any(not isinstance(m, str) or not m or len(m) > 256 for m in markers):
        raise ValueError("invalid synthetic markers")
    with closing(sqlite3.connect(f"file:{db_path.as_posix()}?mode=ro", uri=True)) as conn:
        conn.row_factory = sqlite3.Row
        tables = {r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        table_counts = {t: _count(conn, t) for t in sorted(tables & _CAPTURE_TABLES)}
        ui_counts: Counter[str] = Counter()
        if "ui_events" in tables and "event_type" in _columns(conn, "ui_events"):
            for row in conn.execute('SELECT event_type, COUNT(*) FROM "ui_events" GROUP BY event_type'):
                ui_counts[str(row[0] if row[0] is not None else "<null>")] = int(row[1])
        audio_devices: Counter[str] = Counter()
        for table in ("audio_chunks", "audio_transcriptions"):
            cols = _columns(conn, table)
            field = next((c for c in ("device_name", "device", "input_device", "output_device") if c in cols), None)
            if field:
                for row in conn.execute(f'SELECT {_quote_ident(field)}, COUNT(*) FROM "{table}" GROUP BY {_quote_ident(field)}'):
                    audio_devices[str(row[0] if row[0] is not None else "<null>")] += int(row[1])
        marker_hits = {t: _marker_hits(conn, t, markers) for t in ("audio_transcriptions", "frames", "ui_events") if t in tables}
        notice_categories: Counter[str] = Counter()
        notice_reasons: Counter[str] = Counter()
        if "ui_events" in tables and {"event_type", "text_content"}.issubset(_columns(conn, "ui_events")):
            cols = _columns(conn, "ui_events")
            reason_col = "reason_code" if "reason_code" in cols else None
            selected = f'{_quote_ident(reason_col)}, "text_content"' if reason_col else 'NULL, "text_content"'
            for reason_code, text_content in conn.execute(
                    f'SELECT {selected} FROM "ui_events" WHERE "event_type" = ?',
                    ("privacy_notice",)):
                category, reason = _classify_notice(reason_code, text_content)
                notice_categories[category] += 1
                notice_reasons[reason] += 1
        quick = str(conn.execute("PRAGMA quick_check").fetchone()[0]).lower()
    return {
        "table_counts": table_counts,
        "ui_event_type_counts": dict(sorted(ui_counts.items())),
        "audio_device_counts": dict(sorted(audio_devices.items())),
        "marker_hit_counts": marker_hits,
        "safe_notices": {
            "category_counts": dict(sorted(notice_categories.items())),
            "reason_counts": dict(sorted(notice_reasons.items())),
        },
        "quick_check": quick if quick == "ok" else "failed",
    }


def _local_url(base_url: str, path: str, query: dict[str, str] | None = None) -> str:
    parsed = urlsplit(base_url)
    if parsed.scheme != "http" or parsed.hostname != "127.0.0.1" or parsed.port is None or parsed.username or parsed.password or parsed.path not in ("", "/") or parsed.query or parsed.fragment:
        raise ValueError("base_url must be exact http://127.0.0.1:<port>")
    return urlunsplit(("http", f"127.0.0.1:{parsed.port}", path, urlencode(query or {}), ""))


def _request(url: str, token: str | None) -> int:
    headers = {"Accept": "application/json"}
    if token is not None:
        headers["Authorization"] = f"Bearer {token}"
    try:
        with _open(Request(url, headers=headers), timeout=3) as response:
            return int(response.status)
    except HTTPError as exc:
        return int(exc.code)


def _request_json(url: str, token: str | None) -> tuple[int, dict[str, Any]]:
    headers = {"Accept": "application/json"}
    if token is not None:
        headers["Authorization"] = f"Bearer {token}"
    try:
        with _open(Request(url, headers=headers), timeout=3) as response:
            status = int(response.status)
            raw = response.read(256 * 1024) if hasattr(response, "read") else b""
            value = json.loads(raw.decode("utf-8")) if raw else {}
            return status, value if isinstance(value, dict) else {}
    except (HTTPError, OSError, ValueError, UnicodeError, AttributeError):
        return (int(getattr(locals().get("response", None), "status", 0) or 0), {})


class _NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, new):
        raise HTTPError(req.full_url, code, "redirect rejected", headers, fp)


_OPENER = build_opener(ProxyHandler({}), _NoRedirect())


def _open(request: Request, timeout: int = 3):
    # The opener has no proxy and rejects redirects before headers can escape.
    return _OPENER.open(request, timeout=timeout)


def api_snapshot(base_url: str, token: str) -> dict[str, float | int]:
    """Read only allowlisted numeric audio metrics from authenticated localhost."""
    if not isinstance(token, str) or not token:
        raise ValueError("token required")
    url = _local_url(base_url, "/audio/metrics")
    request = Request(url, headers={"Accept": "application/json", "Authorization": f"Bearer {token}"})
    try:
        with _open(request, timeout=3) as response:
            if int(response.status) != 200 or not hasattr(response, "read"):
                return {}
            raw = response.read(256 * 1024)
    except (HTTPError, OSError, ValueError):
        return {}
    try:
        payload = json.loads(raw.decode("utf-8"))
    except (ValueError, UnicodeError, AttributeError):
        return {}
    if not isinstance(payload, dict):
        return {}
    return {key: value for key, value in payload.items()
            if key in _METRIC_FIELDS and isinstance(value, (int, float)) and not isinstance(value, bool)}


def verify_api(base_url: str, token: str, start_iso: str, end_iso: str) -> dict[str, Any]:
    """Exercise localhost auth/status endpoints and retain only status counts."""
    if not isinstance(token, str) or not token or len(token) > 512:
        raise ValueError("token required")
    paths = {"search": "/search", "audio_device_status": "/audio/device/status", "capture_events": "/capture-events", "audio_metrics": "/audio/metrics"}
    statuses: dict[str, list[int]] = {k: [] for k in paths}
    safe_delivery: dict[str, Any] = {}
    health = _request(_local_url(base_url, "/health"), None)
    for name, path in paths.items():
        query = {"start_time": start_iso, "end_time": end_iso} if name == "capture_events" else None
        url = _local_url(base_url, path, query)
        statuses[name].extend([_request(url, None), _request(url, "wrong-token"), _request(url, token)])
        if name == "capture_events" and statuses[name][2] == 200:
            _, payload = _request_json(url, token)
            allowed = {k: payload[k] for k in ("near_capacity", "dropped_events", "persistence_degraded")
                       if k in payload and isinstance(payload[k], (bool, int, float)) and not isinstance(payload[k], str)}
            if allowed:
                safe_delivery = allowed
    cases = {k: {"missing": v[0], "wrong": v[1], "valid": v[2]} for k, v in statuses.items()}
    auth_ok = all(v["missing"] == 403 and v["wrong"] == 403 and v["valid"] == 200 for v in cases.values())
    return {
        "health_status": health,
        "cases": cases,
        "status_counts": {k: dict(sorted(Counter(v).items())) for k, v in statuses.items()},
        "safe_delivery": safe_delivery,
        "audio_metrics": api_snapshot(base_url, token),
        "passed": bool(health == 200 and auth_ok),
        "checks": {"health_ok": health == 200, "protected_auth_shape_ok": auth_ok},
    }


def evaluate_result(mode: str, observations: dict[str, Any]) -> dict[str, Any]:
    """Conservatively classify evidence as pass, incomplete, or fail."""
    known_modes = {"audio", "audio-output", "audio-microphone", "privacy", "lock", "drm", "browser", "uac"}
    if mode not in known_modes:
        return {"status": "incomplete", "reasons": ["unknown_mode"]}
    # A confirmed leak always wins over missing controls or incomplete metadata.
    if observations.get("unexpected_forbidden_capture") or observations.get("auth_failure"):
        return {"status": "fail", "reasons": ["forbidden_capture" if observations.get("unexpected_forbidden_capture") else "authentication_failure"]}
    required = observations.get("required", [])
    mandatory = ["mode_known", "evidence_complete", "api_verified", "os_verified", "db_verified"]
    missing = [k for k in mandatory + list(required) if not observations.get(k)]
    negative = mode in {"privacy", "lock", "drm", "browser"} or bool(observations.get("negative_privacy"))
    if negative and not (observations.get("before_positive") and observations.get("after_positive")):
        return {"status": "incomplete", "reasons": ["positive_before_after_controls_required"]}
    if negative and not (observations.get("protected_controls") and observations.get("verified_phases")):
        return {"status": "incomplete", "reasons": ["protected_controls_and_verified_phases_required"]}
    if missing or observations.get("incomplete"):
        return {"status": "incomplete", "reasons": missing or ["incomplete_observation"]}
    return {"status": "pass", "reasons": []}
