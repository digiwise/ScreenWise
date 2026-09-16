"""Policy for the gated browser/password/clipboard live batch.

This module is deliberately inert. Importing it and running its unit tests do
not start a browser or listener, display a window, press keys, or access the
clipboard. The separate live adapter still requires a fresh owner gate and the
reviewed browser firewall scope.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass(frozen=True)
class BrowserClipboardPhase:
    name: str
    surface: str
    action: str
    seconds: float
    positive_markers: tuple[str, ...] = ()
    forbidden_markers: tuple[str, ...] = ()


# Five short phases replace the earlier open-ended browser/clipboard plan. The
# browser page and native fixture contain synthetic values only.
PHASES = (
    BrowserClipboardPhase(
        "browser_allowed", "browser", "ordinary-field", 5,
        positive_markers=("public browser cedar", "public bronze hill"),
    ),
    BrowserClipboardPhase(
        "browser_password", "browser", "password-field", 5,
        forbidden_markers=("hidden coral orchard",),
    ),
    BrowserClipboardPhase(
        "browser_excluded", "browser", "excluded-localhost", 5,
        forbidden_markers=("forbidden cyan orchard",),
    ),
    BrowserClipboardPhase(
        "clipboard_plain", "native", "plain-copy-paste", 5,
        positive_markers=("public cedar garden",),
    ),
    BrowserClipboardPhase(
        "clipboard_password", "native", "password-paste", 5,
        # A distinct synthetic value is staged on the clipboard without being
        # rendered in a plain control, then pasted into the password control.
        # Any persisted hit is therefore unambiguously a privacy failure.
        forbidden_markers=("hidden tulip waterfall",),
    ),
)

ALL_MARKERS = tuple(dict.fromkeys(
    marker
    for phase in PHASES
    for marker in (*phase.positive_markers, *phase.forbidden_markers)
))


def preparation_manifest() -> dict[str, Any]:
    """Return the reviewed future-live contract without performing I/O."""
    return {
        "schema": "screenwise.browser-clipboard-prep.v1",
        "interactive_execution_enabled": True,
        "estimated_active_seconds": sum(phase.seconds for phase in PHASES),
        "phases": [
            {
                "name": phase.name,
                "surface": phase.surface,
                "action": phase.action,
                "seconds": phase.seconds,
                "positive_markers": list(phase.positive_markers),
                "forbidden_markers": list(phase.forbidden_markers),
            }
            for phase in PHASES
        ],
        "hard_preconditions": [
            "fresh_loopback_only_server",
            "owned_browser_process_tree",
            "reviewed_exact_browser_firewall_scope",
            "verified_foreground_pid_and_hwnd",
            "fresh_recorder_data_directory",
            "recorder_stopped_before_clipboard_restore",
        ],
        "privacy_limits": (
            "Synthetic marker deltas can test admission and exclusion paths; "
            "they do not prove universal password, image-pixel, keyboard, or "
            "clipboard secrecy."
        ),
    }


def assess_browser_clipboard(phases: list[dict[str, Any]], cleanup: dict[str, Any]) -> dict[str, Any]:
    """Assess aggregate synthetic evidence without inspecting captured content."""
    expected = [phase.name for phase in PHASES]
    if [row.get("name") for row in phases] != expected:
        return {"status": "incomplete", "reason": "phase_sequence_incomplete"}
    if any(row.get("verified") is not True for row in phases):
        return {"status": "incomplete", "reason": "os_or_surface_state_unverified"}

    for spec, row in zip(PHASES, phases, strict=True):
        positives = row.get("positive_deltas")
        forbidden = row.get("forbidden_deltas")
        if not isinstance(positives, dict) or not isinstance(forbidden, dict):
            return {"status": "incomplete", "reason": "marker_delta_schema_invalid"}
        if any(type(value) is not int or value < 0 for value in (*positives.values(), *forbidden.values())):
            return {"status": "incomplete", "reason": "marker_delta_schema_invalid"}
        # Every phase reports the same fixed marker set so a leak cannot be
        # hidden by omitting a marker. The phase specification decides which
        # markers are forbidden on that surface.
        if set(forbidden) != set(ALL_MARKERS):
            return {"status": "incomplete", "reason": "marker_delta_schema_invalid"}
        if any(forbidden.get(marker, 0) > 0 for marker in spec.forbidden_markers):
            return {"status": "failed", "reason": "synthetic_forbidden_marker_persisted"}
        if any(positives.get(marker, 0) <= 0 for marker in spec.positive_markers):
            return {"status": "incomplete", "reason": "positive_control_missing"}

    required_cleanup = (
        cleanup.get("recorder_stopped") is True,
        cleanup.get("clipboard_restore_attempted_after_stop") is True,
        cleanup.get("clipboard_safe_terminal_state") is True,
        cleanup.get("browser_process_tree_stopped") is True,
        cleanup.get("loopback_server_stopped") is True,
        cleanup.get("fixture_stopped") is True,
    )
    if not all(required_cleanup):
        return {"status": "incomplete", "reason": "cleanup_unverified"}
    return {
        "status": "passed_scoped_browser_clipboard_checks",
        "reason": None,
        "limits": preparation_manifest()["privacy_limits"],
    }
