"""Pure, aggregate-only acceptance evaluator for one Windows lock transition.

The live adapter owns all Windows, process, API, and SQLite interaction.  This
module accepts its reduced evidence dictionary and performs no I/O.  A passing
result means the supplied control, transition, row-count, notice, API, process,
and cleanup facts satisfy this contract.  Aggregate row counts cannot establish
that every pixel was protected.
"""

from __future__ import annotations

from collections import Counter
from typing import Any


EVIDENCE_SCHEMA = "screenwise.lock-transition-evidence.v1"
RESULT_SCHEMA = "screenwise.lock-transition-acceptance.v1"
MARKER_ID = "public_cedar_garden"
WTS_SOURCE = "windows.session_state"
NOTICE_MAPPING = "fixed_run_start_utc_monotonic_offset"
LOCKED_MESSAGE = "Screen locked; screen/UI capture paused."
UNLOCKED_MESSAGE = (
    "Screen unlocked; other privacy and recording controls still apply."
)
DISCONNECTED_MESSAGE = "Windows session disconnected; screen/UI capture paused."
DESKTOP_MESSAGE = (
    "Secure desktop active or input desktop unavailable; screen/UI capture paused."
)
DETECTION_FAILED_MESSAGE = (
    "Screen lock detection unavailable; screen/UI capture paused for privacy."
)
_NOTICE_MESSAGES = {
    "wts_session_locked": LOCKED_MESSAGE,
    "wts_session_unlocked": UNLOCKED_MESSAGE,
    "wts_session_disconnected": DISCONNECTED_MESSAGE,
    "input_desktop_unavailable": DESKTOP_MESSAGE,
    "process_session_query_failed": DETECTION_FAILED_MESSAGE,
    "wts_query_failed": DETECTION_FAILED_MESSAGE,
    "wts_short_buffer": DETECTION_FAILED_MESSAGE,
    "wts_unsupported_level": DETECTION_FAILED_MESSAGE,
    "wts_session_mismatch": DETECTION_FAILED_MESSAGE,
    "wts_session_state_unknown": DETECTION_FAILED_MESSAGE,
    "wts_session_flags_unknown": DETECTION_FAILED_MESSAGE,
    "wts_session_flags_invalid": DETECTION_FAILED_MESSAGE,
}

_PAYLOAD_KEYS = {
    "accessibility_text",
    "accessibility_tree_json",
    "audio_bytes",
    "clipboard",
    "frame_bytes",
    "image",
    "ocr_text",
    "payload",
    "text_content",
    "token",
    "transcription",
    "url",
    "window_title",
}
_FAIL_REASONS = {
    "captured_payload_present",
    "locked_frame_writes",
    "locked_uia_writes",
}
_TRANSITION_UNKNOWN_CODE = "secure_desktop_unavailable"
_TRANSITION_UNKNOWN_MAX_MS = 2_000
_TRANSITION_UNKNOWN_MAX_SAMPLES = 16


def _dict(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _list(value: Any) -> list[Any]:
    return value if isinstance(value, list) else []


def _int(value: Any, *, minimum: int = 0) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= minimum


def _strictly_increasing(values: list[Any]) -> bool:
    return bool(values) and all(
        _int(value) for value in values
    ) and all(left < right for left, right in zip(values, values[1:]))


def _contains_payload_key(value: Any) -> bool:
    if isinstance(value, dict):
        if any(str(key).lower() in _PAYLOAD_KEYS for key in value):
            return True
        return any(_contains_payload_key(item) for item in value.values())
    if isinstance(value, list):
        return any(_contains_payload_key(item) for item in value)
    return False


def _collapsed(values: list[str]) -> list[str]:
    result: list[str] = []
    for value in values:
        if not result or result[-1] != value:
            result.append(value)
    return result


def evaluate_lock_acceptance(evidence: dict[str, Any]) -> dict[str, Any]:
    """Return a conservative decision without echoing supplied evidence."""
    reasons: list[str] = []

    def reject(reason: str) -> None:
        if reason not in reasons:
            reasons.append(reason)

    if not isinstance(evidence, dict) or evidence.get("schema") != EVIDENCE_SCHEMA:
        reject("invalid_schema")
        evidence = _dict(evidence)
    if _contains_payload_key(evidence):
        reject("captured_payload_present")

    readiness = _dict(evidence.get("owner_readiness"))
    if not (
        readiness.get("fresh") is True
        and readiness.get("waited_indefinitely") is True
        and isinstance(readiness.get("phase_id"), str)
        and bool(readiness.get("phase_id"))
    ):
        reject("owner_readiness_unconfirmed")

    clock = _dict(evidence.get("clock"))
    run_start_ms = clock.get("run_start_monotonic_ms")
    if not (
        isinstance(clock.get("run_start_utc"), str)
        and bool(clock.get("run_start_utc"))
        and _int(run_start_ms)
        and clock.get("notice_mapping") == NOTICE_MAPPING
        and clock.get("verified") is True
    ):
        reject("clock_mapping_unconfirmed")
        run_start_ms = None

    process = _dict(evidence.get("process"))
    pid = process.get("pid")
    creation_time = process.get("creation_time")
    if not (
        _int(pid, minimum=1)
        and _int(creation_time, minimum=1)
        and _int(process.get("scope_count"), minimum=1)
        and process.get("scope_count") == 1
    ):
        reject("process_scope_invalid")

    process_observations = _list(process.get("observations"))
    expected_checkpoints = ["pre_lock", "locked_stable_end", "post_unlock"]
    process_times = [
        _dict(item).get("observed_ms") for item in process_observations
    ]
    if (
        [_dict(item).get("checkpoint") for item in process_observations]
        != expected_checkpoints
        or not _strictly_increasing(process_times)
        or any(_dict(item).get("alive") is not True for item in process_observations)
    ):
        reject("process_liveness_unconfirmed")
    if any(
        _dict(item).get("pid") != pid
        or _dict(item).get("creation_time") != creation_time
        for item in process_observations
    ):
        reject("process_identity_reused_or_changed")

    shutdown = _dict(process.get("shutdown"))
    if not (
        shutdown.get("observed") is True
        and _int(shutdown.get("exit_code"))
        and shutdown.get("exit_code") == 0
        and shutdown.get("forced") is False
        and shutdown.get("issues") == []
    ):
        reject("shutdown_unconfirmed")

    controls = _dict(evidence.get("controls"))
    before = _dict(controls.get("before"))
    after = _dict(controls.get("after"))

    def valid_control(control: dict[str, Any]) -> bool:
        return (
            control.get("marker_id") == MARKER_ID
            and _int(control.get("generation"), minimum=1)
            and _int(control.get("frame_hits"), minimum=1)
            and _int(control.get("uia_hits"), minimum=1)
            and _int(control.get("authenticated_search_hits"), minimum=1)
            and _int(control.get("observed_ms"))
        )

    if not (valid_control(before) and valid_control(after)):
        reject("missing_positive_controls")
    elif not (
        after["generation"] > before["generation"]
        and after["observed_ms"] > before["observed_ms"]
    ):
        reject("control_generation_not_distinct")

    wts_samples = [_dict(item) for item in _list(evidence.get("wts_samples"))]
    wts_times = [item.get("observed_ms") for item in wts_samples]
    sample_ids = [item.get("sample_id") for item in wts_samples]
    wts_shape_valid = bool(wts_samples) and _strictly_increasing(wts_times)
    wts_shape_valid = wts_shape_valid and all(
        isinstance(sample_id, str) and bool(sample_id) for sample_id in sample_ids
    ) and len(sample_ids) == len(set(sample_ids))
    wts_shape_valid = wts_shape_valid and all(
        sample.get("state") in {"unlocked", "locked", "unknown"}
        and sample.get("source") == WTS_SOURCE
        and sample.get("real") is True
        and sample.get("independent") is True
        and (
            (sample.get("state") in {"unlocked", "locked"}
             and sample.get("error_code") is None)
            or (sample.get("state") == "unknown"
                and isinstance(sample.get("error_code"), str)
                and bool(sample.get("error_code")))
        )
        for sample in wts_samples
    )
    if not wts_shape_valid:
        reject("invalid_wts_evidence")

    confirmed_samples = [
        sample for sample in wts_samples if sample.get("state") in {"unlocked", "locked"}
    ]
    states = [str(sample.get("state")) for sample in confirmed_samples]
    if _collapsed(states) != ["unlocked", "locked", "unlocked"]:
        reject("wts_transition_order_invalid")

    lock_ms: int | None = None
    unlock_ms: int | None = None
    if wts_shape_valid and _collapsed(states) == ["unlocked", "locked", "unlocked"]:
        confirmed_times = [sample["observed_ms"] for sample in confirmed_samples]
        lock_index = states.index("locked")
        lock_ms = confirmed_times[lock_index]
        unlock_index = next(
            index
            for index in range(lock_index + 1, len(states))
            if states[index] == "unlocked"
        )
        unlock_ms = confirmed_times[unlock_index]
        last_pre_lock_ms = confirmed_times[lock_index - 1]
        last_locked_ms = confirmed_times[unlock_index - 1]
        stable_times = [
            sample["observed_ms"]
            for sample in confirmed_samples
            if sample.get("state") == "locked"
            and sample["observed_ms"] >= lock_ms + 2_000
        ]
        if len(stable_times) < 2 or stable_times[-1] - stable_times[0] < 10_000:
            reject("locked_stability_unconfirmed")
    else:
        stable_times = []
        last_pre_lock_ms = None
        last_locked_ms = None
        reject("locked_stability_unconfirmed")

    unknown_samples = [sample for sample in wts_samples if sample.get("state") == "unknown"]
    unknown_counts = Counter(str(sample.get("error_code")) for sample in unknown_samples)
    if unknown_samples:
        lock_unknowns = []
        unlock_unknowns = []
        for sample in unknown_samples:
            timestamp = sample.get("observed_ms")
            if sample.get("error_code") != _TRANSITION_UNKNOWN_CODE:
                reject("wts_unknown_reason_invalid")
            if (
                last_pre_lock_ms is not None
                and lock_ms is not None
                and last_pre_lock_ms < timestamp < lock_ms
            ):
                lock_unknowns.append(sample)
            elif (
                last_locked_ms is not None
                and unlock_ms is not None
                and last_locked_ms < timestamp < unlock_ms
            ):
                unlock_unknowns.append(sample)
            else:
                reject("wts_unknown_outside_transition")
        for samples, left, right in (
            (lock_unknowns, last_pre_lock_ms, lock_ms),
            (unlock_unknowns, last_locked_ms, unlock_ms),
        ):
            if samples and (
                left is None
                or right is None
                or right - left > _TRANSITION_UNKNOWN_MAX_MS
                or len(samples) > _TRANSITION_UNKNOWN_MAX_SAMPLES
            ):
                reject("wts_unknown_transition_unbounded")

    plateau = _dict(evidence.get("locked_plateau"))
    plateau_start = plateau.get("start_ms")
    plateau_end = plateau.get("end_ms")
    plateau_counts = [
        plateau.get("start_frame_rows"),
        plateau.get("end_frame_rows"),
        plateau.get("start_uia_capture_rows"),
        plateau.get("end_uia_capture_rows"),
        plateau.get("start_privacy_notice_rows"),
        plateau.get("end_privacy_notice_rows"),
    ]
    plateau_shape_valid = (
        plateau.get("grace_complete") is True
        and _int(plateau_start)
        and _int(plateau_end)
        and plateau_end > plateau_start
        and all(_int(value) for value in plateau_counts)
        and plateau["end_privacy_notice_rows"] >= plateau["start_privacy_notice_rows"]
    )
    if not plateau_shape_valid:
        reject("locked_plateau_unconfirmed")
    else:
        if plateau["end_frame_rows"] != plateau["start_frame_rows"]:
            reject("locked_frame_writes")
        if plateau["end_uia_capture_rows"] != plateau["start_uia_capture_rows"]:
            reject("locked_uia_writes")
        if not (
            lock_ms is not None
            and unlock_ms is not None
            and plateau_start >= lock_ms + 2_000
            and plateau_end - plateau_start >= 10_000
            and plateau_end <= unlock_ms
            and stable_times
            and stable_times[0] <= plateau_start
            and stable_times[-1] >= plateau_end
        ):
            reject("locked_plateau_window_invalid")

    notices = [_dict(item) for item in _list(evidence.get("notices"))]
    notice_times = [item.get("timestamp_ms") for item in notices]
    notices_valid = (
        _strictly_increasing(notice_times)
        and all(
            item.get("typed") is True
            and item.get("reason_code") in _NOTICE_MESSAGES
            and item.get("message") == _NOTICE_MESSAGES[item["reason_code"]]
            and set(item) == {"typed", "reason_code", "message", "timestamp_ms"}
            for item in notices
        )
    )
    notice_codes = [str(item.get("reason_code")) for item in notices]
    if not notices_valid:
        reject("typed_notices_invalid")
    locked_notice_indexes = [
        index for index, code in enumerate(notice_codes) if code == "wts_session_locked"
    ]
    if (
        len(locked_notice_indexes) != 1
        or not notice_codes
        or notice_codes[-1] != "wts_session_unlocked"
        or locked_notice_indexes[0] >= len(notice_codes) - 1
    ):
        reject("typed_notice_sequence_missing")
    elif (
        notices_valid
        and last_pre_lock_ms is not None
        and lock_ms is not None
        and last_locked_ms is not None
        and unlock_ms is not None
    ):
        locked_notice_index = locked_notice_indexes[0]
        if not (
            last_pre_lock_ms <= notice_times[locked_notice_index] <= lock_ms + 1_000
            and last_locked_ms <= notice_times[-1] <= unlock_ms + 1_000
        ):
            reject("typed_notice_timing_invalid")

    logs = _dict(evidence.get("logs"))
    log_counts = _dict(logs.get("reason_code_counts"))
    expected_log_counts = dict(Counter(notice_codes))
    if not (
        logs.get("safe_notice_lines_verified") is True
        and log_counts == expected_log_counts
        and all(_int(value, minimum=1) for value in log_counts.values())
        and expected_log_counts.get("wts_session_locked") == 1
        and expected_log_counts.get("wts_session_unlocked", 0) >= 1
    ):
        reject("safe_log_codes_unconfirmed")

    api_checks = [_dict(item) for item in _list(evidence.get("api_checks"))]
    api_times = [item.get("observed_ms") for item in api_checks]
    api_valid = (
        [item.get("checkpoint") for item in api_checks] == ["before", "after"]
        and _strictly_increasing(api_times)
        and all(
            item.get("bind_host") == "127.0.0.1"
            and _int(item.get("port"), minimum=1)
            and item["port"] <= 65_535
            and item.get("endpoint") == "/search"
            and item.get("content_type") == "all"
            and _int(item.get("missing"))
            and item.get("missing") == 403
            and _int(item.get("wrong"))
            and item.get("wrong") == 403
            and _int(item.get("valid"))
            and item.get("valid") == 200
            for item in api_checks
        )
        and len({item.get("port") for item in api_checks}) == 1
    )
    if not api_valid:
        reject("api_auth_or_bind_invalid")

    audio = _dict(evidence.get("audio"))
    silent_counts = [
        audio.get("running_device_count"),
        audio.get("audio_chunk_rows"),
        audio.get("audio_transcription_rows"),
    ]
    if not (
        audio.get("enabled") is False
        and audio.get("tested") is False
        and all(_int(value) and value == 0 for value in silent_counts)
    ):
        reject("audio_scope_invalid")

    cleanup = _dict(evidence.get("cleanup"))
    if not (
        cleanup.get("processes_stopped") is True
        and cleanup.get("fixture_closed") is True
    ):
        reject("cleanup_unconfirmed")

    if lock_ms is not None and unlock_ms is not None:
        if len(process_times) == 3 and _strictly_increasing(process_times):
            if not (
                process_times[0] < lock_ms
                and lock_ms < process_times[1] <= unlock_ms
                and process_times[2] > unlock_ms
            ):
                reject("process_observation_order_invalid")
        if valid_control(before) and valid_control(after) and not (
            before["observed_ms"] < lock_ms
            and after["observed_ms"] > unlock_ms
        ):
            reject("control_order_invalid")
        if len(api_times) == 2 and _strictly_increasing(api_times) and not (
            api_times[0] < lock_ms and api_times[1] > unlock_ms
        ):
            reject("api_checkpoint_order_invalid")

    timestamp_values = (
        process_times
        + wts_times
        + [before.get("observed_ms"), after.get("observed_ms")]
        + api_times
        + notice_times
        + [plateau_start, plateau_end]
    )
    if run_start_ms is not None and any(
        not _int(value) or value < run_start_ms for value in timestamp_values
    ):
        reject("timestamp_outside_run")

    status = "fail" if any(reason in _FAIL_REASONS for reason in reasons) else (
        "incomplete" if reasons else "pass"
    )
    return {
        "schema": RESULT_SCHEMA,
        "status": status,
        "reasons": reasons,
        "eligible": status == "pass",
        "scope_note": (
            "Aggregate frame/UIA row-count plateau evidence does not prove that "
            "every pixel was protected."
        ),
        "audio_scope": "Audio was disabled and lock recovery for audio remains untested.",
        "additional_notice_reason_counts": dict(
            sorted(
                (code, count)
                for code, count in Counter(notice_codes).items()
                if code not in {"wts_session_locked", "wts_session_unlocked"}
            )
        ),
        "wts_unknown_reason_counts": dict(sorted(unknown_counts.items())),
    }


__all__ = [
    "EVIDENCE_SCHEMA",
    "DETECTION_FAILED_MESSAGE",
    "DESKTOP_MESSAGE",
    "DISCONNECTED_MESSAGE",
    "LOCKED_MESSAGE",
    "MARKER_ID",
    "NOTICE_MAPPING",
    "RESULT_SCHEMA",
    "UNLOCKED_MESSAGE",
    "WTS_SOURCE",
    "evaluate_lock_acceptance",
]
