"""Pure acceptance evaluator for a future audio partial-tail stop/restart run.

This module performs no I/O.  A live harness must collect the small, synthetic,
content-bounded evidence mapping described in README.md and pass it to evaluate().
"""

from __future__ import annotations

import os

import math
from typing import Any

API_URL = f"http://127.0.0.1:{int(os.environ.get('SCREENWISE_VALIDATION_API_PORT', '31479'))}"


def _mapping(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _positive_int(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def _number(value: Any) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
    )


def _exact_int(value: Any, expected: int) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value == expected


def _true(value: Any) -> bool:
    return value is True


def _false(value: Any) -> bool:
    return value is False


def evaluate(observation: Any) -> dict[str, Any]:
    """Return pass, fail, or incomplete without reading files or calling APIs.

    ``fail`` means supplied evidence contradicts an acceptance invariant.
    ``incomplete`` means required evidence is absent or malformed.  A confirmed
    hard failure wins over incomplete fields so cleanup/auth failures stay visible.
    """

    if not isinstance(observation, dict):
        return {"status": "incomplete", "reasons": ["observation_mapping_required"]}

    missing: list[str] = []
    failures: list[str] = []

    def require(condition: bool, reason: str) -> None:
        if not condition:
            missing.append(reason)

    def reject(condition: bool, reason: str) -> None:
        if condition:
            failures.append(reason)

    require(observation.get("evidence_version") == 1, "evidence_version_1_required")
    data_store_id = observation.get("data_store_id")
    require(isinstance(data_store_id, str) and bool(data_store_id.strip()), "opaque_data_store_identity_required")

    readiness = _mapping(observation.get("readiness"))
    require(_number(readiness.get("gate_created_at")), "readiness_gate_time_required")
    require(_number(readiness.get("accepted_at")), "readiness_accept_time_required")
    require(_true(readiness.get("explicit_owner_ready")), "explicit_owner_ready_required")
    require(_true(readiness.get("nonce_match")), "fresh_readiness_nonce_required")
    require(_true(readiness.get("recording_stopped_while_waiting")), "recorder_stopped_during_readiness_required")
    require(_true(readiness.get("playback_stopped_while_waiting")), "playback_stopped_during_readiness_required")
    require(_true(readiness.get("owned_windows_hidden_while_waiting")), "owned_windows_hidden_during_readiness_required")
    require("expires_at" in readiness, "readiness_expiry_field_required")
    require(_false(readiness.get("deadline_enforced")), "readiness_no_deadline_evidence_required")
    reject(readiness.get("expires_at") is not None, "readiness_expiry_forbidden")
    reject(readiness.get("deadline_enforced") is True, "readiness_deadline_forbidden")
    if _number(readiness.get("gate_created_at")) and _number(readiness.get("accepted_at")):
        reject(readiness["accepted_at"] < readiness["gate_created_at"], "readiness_precedes_gate")

    markers = _mapping(observation.get("markers"))
    marker_values = [markers.get(name) for name in ("before", "tail", "restart")]
    require(all(isinstance(value, str) and value.strip() for value in marker_values), "three_nonempty_markers_required")
    if all(isinstance(value, str) and value.strip() for value in marker_values):
        normalized_markers = [" ".join(value.split()).casefold() for value in marker_values]
        reject(len(set(normalized_markers)) != 3, "markers_must_be_distinct")
        reject(
            any(left in right for index, left in enumerate(normalized_markers) for other, right in enumerate(normalized_markers) if index != other),
            "marker_substring_overlap_forbidden",
        )

    pre_tail = _mapping(observation.get("pre_tail_snapshot"))
    require(_number(pre_tail.get("observed_at")), "pre_tail_snapshot_time_required")
    require(isinstance(pre_tail.get("max_audio_row_id"), int) and not isinstance(pre_tail.get("max_audio_row_id"), bool) and pre_tail.get("max_audio_row_id", -1) >= 0, "pre_tail_max_row_id_required")
    require(_exact_int(pre_tail.get("tail_db_count"), 0), "tail_must_be_absent_from_pre_tail_database")
    require(_exact_int(pre_tail.get("tail_search_count"), 0), "tail_must_be_absent_from_pre_tail_search")
    require(_true(pre_tail.get("authenticated")), "pre_tail_search_must_be_authenticated")
    require(pre_tail.get("data_store_id") == data_store_id, "pre_tail_data_store_identity_mismatch")
    reject(_positive_int(pre_tail.get("tail_db_count")), "tail_seen_before_tail_stimulus")
    reject(_positive_int(pre_tail.get("tail_search_count")), "tail_searchable_before_tail_stimulus")

    before = _mapping(observation.get("before_control"))
    require(_positive_int(before.get("db_count")), "before_control_database_positive_required")
    require(_positive_int(before.get("search_count")), "before_control_search_positive_required")
    require(_true(before.get("authenticated")), "before_control_authenticated_search_required")
    require(_number(before.get("observed_at")), "before_control_time_required")

    stimulus = _mapping(observation.get("tail_stimulus"))
    for field in ("started_at", "completed_at", "stop_requested_at"):
        require(_number(stimulus.get(field)), f"tail_{field}_required")
    if all(_number(stimulus.get(field)) for field in ("started_at", "completed_at", "stop_requested_at")):
        reject(not (stimulus["started_at"] <= stimulus["completed_at"] <= stimulus["stop_requested_at"]), "tail_stop_order_invalid")
    if _number(pre_tail.get("observed_at")) and _number(stimulus.get("started_at")):
        reject(pre_tail["observed_at"] >= stimulus["started_at"], "pre_tail_snapshot_not_before_tail")
    if _number(before.get("observed_at")) and _number(stimulus.get("started_at")):
        reject(before["observed_at"] >= stimulus["started_at"], "before_control_not_before_tail")

    boundary = _mapping(observation.get("partial_boundary"))
    require(_number(boundary.get("segment_window_seconds")) and boundary.get("segment_window_seconds", 0) > 0, "positive_segment_window_required")
    require(_number(boundary.get("overlap_seconds")) and boundary.get("overlap_seconds", -1) >= 0, "nonnegative_overlap_required")
    require(_number(boundary.get("emission_stride_seconds")) and boundary.get("emission_stride_seconds", 0) > 0, "positive_emission_stride_required")
    require(_number(boundary.get("tail_audio_seconds")) and boundary.get("tail_audio_seconds", 0) > 0, "positive_tail_duration_required")
    require(_true(boundary.get("capture_callback_after_tail_start")), "capture_callback_after_tail_start_required")
    require(_false(boundary.get("normal_chunk_emitted_after_tail_start")), "normal_chunk_after_tail_start_forbidden")
    require(_number(boundary.get("last_callback_at")), "tail_callback_time_required")
    require(_number(boundary.get("last_regular_emission_at")), "last_regular_emission_time_required")
    if all(_number(boundary.get(field)) for field in ("segment_window_seconds", "overlap_seconds", "emission_stride_seconds")):
        reject(
            not math.isclose(boundary["segment_window_seconds"] - boundary["overlap_seconds"], boundary["emission_stride_seconds"], rel_tol=0.0, abs_tol=1e-6),
            "segment_overlap_stride_inconsistent",
        )
    if _number(boundary.get("emission_stride_seconds")) and _number(boundary.get("tail_audio_seconds")):
        reject(boundary["tail_audio_seconds"] >= boundary["emission_stride_seconds"], "tail_not_shorter_than_emission_stride")
    if _number(stimulus.get("started_at")) and _number(stimulus.get("completed_at")) and _number(boundary.get("tail_audio_seconds")):
        reject(
            not math.isclose(stimulus["completed_at"] - stimulus["started_at"], boundary["tail_audio_seconds"], rel_tol=0.0, abs_tol=0.25),
            "tail_playback_duration_mismatch",
        )
    if _number(boundary.get("last_regular_emission_at")) and _number(stimulus.get("started_at")):
        reject(boundary["last_regular_emission_at"] >= stimulus["started_at"], "tail_not_after_regular_emission")
    if _number(boundary.get("last_regular_emission_at")) and _number(stimulus.get("stop_requested_at")) and _number(boundary.get("emission_stride_seconds")):
        reject(stimulus["stop_requested_at"] - boundary["last_regular_emission_at"] >= boundary["emission_stride_seconds"], "stop_not_before_next_regular_emission")
    if _number(stimulus.get("started_at")) and _number(stimulus.get("stop_requested_at")) and _number(boundary.get("last_callback_at")):
        reject(not (stimulus["started_at"] <= boundary["last_callback_at"] <= stimulus["stop_requested_at"]), "tail_callback_outside_tail_stop_window")
    reject(boundary.get("normal_chunk_emitted_after_tail_start") is True, "normal_chunk_emitted_after_tail_start")

    api = _mapping(observation.get("api_auth"))
    require(api.get("base_url") == API_URL, "exact_loopback_api_required")
    require(_exact_int(api.get("missing_status"), 403), "missing_bearer_must_be_rejected")
    require(_exact_int(api.get("wrong_status"), 403), "wrong_bearer_must_be_rejected")
    require(_exact_int(api.get("valid_status"), 200), "valid_bearer_must_succeed")
    require(_false(api.get("redirect_followed")), "api_redirects_must_be_disabled")
    reject(_exact_int(api.get("missing_status"), 200) or _exact_int(api.get("wrong_status"), 200), "protected_api_auth_bypass")

    shutdown = _mapping(observation.get("first_shutdown"))
    require(_true(shutdown.get("graceful_requested")), "first_graceful_stop_required")
    require(isinstance(shutdown.get("process_identity"), str) and bool(shutdown.get("process_identity", "").strip()), "first_process_identity_required")
    require(_true(shutdown.get("process_exited")), "first_process_exit_required")
    require(_exact_int(shutdown.get("exit_code"), 0), "first_zero_exit_required")
    require(_false(shutdown.get("forced")), "first_forced_stop_forbidden")
    require(shutdown.get("unresolved_workers") == [], "first_no_unresolved_workers_required")
    require(shutdown.get("shutdown_issues") == [], "first_no_shutdown_issues_required")
    require(_number(shutdown.get("observed_at")), "first_shutdown_time_required")
    reject(shutdown.get("forced") is True, "first_shutdown_was_forced")
    reject(shutdown.get("process_exited") is False, "first_process_did_not_exit")
    reject(isinstance(shutdown.get("exit_code"), int) and not isinstance(shutdown.get("exit_code"), bool) and shutdown.get("exit_code") != 0, "first_process_exit_nonzero")
    reject(isinstance(shutdown.get("unresolved_workers"), list) and len(shutdown["unresolved_workers"]) > 0, "first_shutdown_has_unresolved_workers")
    reject(isinstance(shutdown.get("shutdown_issues"), list) and len(shutdown["shutdown_issues"]) > 0, "first_shutdown_reported_issue")
    if _number(stimulus.get("stop_requested_at")) and _number(shutdown.get("observed_at")):
        reject(shutdown["observed_at"] < stimulus["stop_requested_at"], "first_exit_precedes_stop_request")

    post_stop = _mapping(observation.get("post_stop_tail"))
    require(post_stop.get("marker") == markers.get("tail"), "post_stop_database_match_must_use_exact_tail_marker")
    require(post_stop.get("data_store_id") == data_store_id, "post_stop_data_store_identity_mismatch")
    require(_positive_int(post_stop.get("db_count")), "tail_database_positive_after_stop_required")
    require(_number(post_stop.get("observed_at")), "post_stop_tail_time_required")
    require(isinstance(post_stop.get("min_matching_row_id"), int) and not isinstance(post_stop.get("min_matching_row_id"), bool) and post_stop.get("min_matching_row_id", -1) >= 0, "tail_matching_row_id_required")
    if isinstance(pre_tail.get("max_audio_row_id"), int) and isinstance(post_stop.get("min_matching_row_id"), int):
        reject(post_stop["min_matching_row_id"] <= pre_tail["max_audio_row_id"], "tail_match_not_newer_than_pre_tail_checkpoint")
    if _number(shutdown.get("observed_at")) and _number(post_stop.get("observed_at")):
        reject(post_stop["observed_at"] <= shutdown["observed_at"], "tail_checked_before_process_exit")

    restart = _mapping(observation.get("restart"))
    require(_true(restart.get("started")), "restart_required")
    require(_true(restart.get("ready")), "restart_readiness_required")
    require(_true(restart.get("same_data_store")), "restart_same_data_store_required")
    require(restart.get("data_store_id") == data_store_id, "restart_data_store_identity_mismatch")
    require(restart.get("kind") == "recorder_process", "recorder_process_restart_required")
    require(_true(restart.get("new_process_identity")), "new_recorder_process_identity_required")
    require(isinstance(restart.get("process_identity"), str) and bool(restart.get("process_identity", "").strip()), "restart_process_identity_required")
    if isinstance(shutdown.get("process_identity"), str) and isinstance(restart.get("process_identity"), str):
        reject(shutdown["process_identity"] == restart["process_identity"], "restart_reused_process_identity")
    require(_number(restart.get("started_at")), "restart_start_time_required")
    require(_number(restart.get("ready_at")), "restart_ready_time_required")
    require(_positive_int(restart.get("tail_db_count")), "tail_database_durability_after_restart_required")
    require(restart.get("tail_query_marker") == markers.get("tail"), "restart_tail_query_must_use_exact_tail_marker")
    require(_positive_int(restart.get("tail_search_count")), "tail_search_durability_after_restart_required")
    require(_true(restart.get("tail_search_authenticated")), "restart_tail_search_must_be_authenticated")
    require(_positive_int(restart.get("control_db_count")), "restart_control_database_positive_required")
    require(restart.get("control_query_marker") == markers.get("restart"), "restart_control_query_must_use_exact_restart_marker")
    require(_positive_int(restart.get("control_search_count")), "restart_control_search_positive_required")
    require(_true(restart.get("control_search_authenticated")), "restart_control_search_must_be_authenticated")
    require(_number(restart.get("control_observed_at")), "restart_control_time_required")
    restart_api = _mapping(restart.get("api_auth"))
    require(_exact_int(restart_api.get("missing_status"), 403), "restart_missing_bearer_must_be_rejected")
    require(_exact_int(restart_api.get("wrong_status"), 403), "restart_wrong_bearer_must_be_rejected")
    require(_exact_int(restart_api.get("valid_status"), 200), "restart_valid_bearer_must_succeed")
    require(_false(restart_api.get("redirect_followed")), "restart_api_redirects_must_be_disabled")
    reject(_exact_int(restart_api.get("missing_status"), 200) or _exact_int(restart_api.get("wrong_status"), 200), "restart_protected_api_auth_bypass")
    if _number(post_stop.get("observed_at")) and _number(restart.get("started_at")):
        reject(restart["started_at"] <= post_stop["observed_at"], "restart_started_before_post_stop_tail_check")
    if _number(restart.get("started_at")) and _number(restart.get("ready_at")):
        reject(restart["ready_at"] < restart["started_at"], "restart_ready_precedes_start")
    if _number(restart.get("ready_at")) and _number(restart.get("control_observed_at")):
        reject(restart["control_observed_at"] <= restart["ready_at"], "restart_control_not_after_readiness")

    final_shutdown = _mapping(observation.get("final_shutdown"))
    require(_true(final_shutdown.get("graceful_requested")), "final_graceful_stop_required")
    require(_true(final_shutdown.get("process_exited")), "final_process_exit_required")
    require(_exact_int(final_shutdown.get("exit_code"), 0), "final_zero_exit_required")
    require(_false(final_shutdown.get("forced")), "final_forced_stop_forbidden")
    require(final_shutdown.get("unresolved_workers") == [], "final_no_unresolved_workers_required")
    require(final_shutdown.get("shutdown_issues") == [], "final_no_shutdown_issues_required")
    require(final_shutdown.get("process_identity") == restart.get("process_identity"), "final_shutdown_process_identity_mismatch")
    require(_number(final_shutdown.get("observed_at")), "final_shutdown_time_required")
    reject(final_shutdown.get("forced") is True, "final_shutdown_was_forced")
    reject(final_shutdown.get("process_exited") is False, "final_process_did_not_exit")
    reject(isinstance(final_shutdown.get("exit_code"), int) and not isinstance(final_shutdown.get("exit_code"), bool) and final_shutdown.get("exit_code") != 0, "final_process_exit_nonzero")
    reject(isinstance(final_shutdown.get("unresolved_workers"), list) and len(final_shutdown["unresolved_workers"]) > 0, "final_shutdown_has_unresolved_workers")
    reject(isinstance(final_shutdown.get("shutdown_issues"), list) and len(final_shutdown["shutdown_issues"]) > 0, "final_shutdown_reported_issue")
    if _number(restart.get("control_observed_at")) and _number(final_shutdown.get("observed_at")):
        reject(final_shutdown["observed_at"] <= restart["control_observed_at"], "final_shutdown_not_after_restart_control")

    if failures:
        return {"status": "fail", "reasons": list(dict.fromkeys(failures))}
    if missing:
        return {"status": "incomplete", "reasons": list(dict.fromkeys(missing))}
    return {"status": "pass", "reasons": []}
