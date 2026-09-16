"""Pure evaluator for the preferred 60-second single-flush tail test."""

from __future__ import annotations

import os

import math
from typing import Any

API_URL = f"http://127.0.0.1:{int(os.environ.get('SCREENWISE_VALIDATION_API_PORT', '31479'))}"


def _map(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _num(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def _int(value: Any, expected: int | None = None) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and (expected is None or value == expected)


def _positive_int(value: Any) -> bool:
    return _int(value) and value > 0


def _status_matrix(value: Any) -> tuple[bool, bool]:
    item = _map(value)
    complete = (
        item.get("base_url") == API_URL
        and
        _int(item.get("missing_status"), 403)
        and _int(item.get("wrong_status"), 403)
        and _int(item.get("valid_status"), 200)
        and item.get("redirect_followed") is False
    )
    bypass = _int(item.get("missing_status"), 200) or _int(item.get("wrong_status"), 200)
    return complete, bypass


def evaluate(observation: Any) -> dict[str, Any]:
    if not isinstance(observation, dict):
        return {"status": "incomplete", "reasons": ["observation_mapping_required"]}

    incomplete: list[str] = []
    failures: list[str] = []

    def need(condition: bool, reason: str) -> None:
        if not condition:
            incomplete.append(reason)

    def fail(condition: bool, reason: str) -> None:
        if condition:
            failures.append(reason)

    need(observation.get("evidence_version") == 1, "evidence_version_1_required")
    store_id = observation.get("data_store_id")
    need(isinstance(store_id, str) and bool(store_id.strip()), "opaque_data_store_identity_required")

    markers = _map(observation.get("markers"))
    values = [markers.get(name) for name in ("baseline", "tail", "restart")]
    need(all(isinstance(value, str) and value.strip() for value in values), "three_markers_required")
    if all(isinstance(value, str) and value.strip() for value in values):
        normalized = [" ".join(value.split()).casefold() for value in values]
        fail(len(set(normalized)) != 3, "markers_must_be_distinct")
        fail(any(a in b for i, a in enumerate(normalized) for j, b in enumerate(normalized) if i != j), "marker_substring_overlap_forbidden")

    readiness = _map(observation.get("readiness"))
    need(readiness.get("explicit_owner_ready") is True, "explicit_owner_ready_required")
    need(readiness.get("nonce_match") is True, "fresh_readiness_nonce_required")
    need("expires_at" in readiness, "readiness_expiry_field_required")
    need(readiness.get("deadline_enforced") is False, "readiness_must_not_have_deadline")
    fail(readiness.get("expires_at") is not None, "readiness_expiry_forbidden")

    config = _map(observation.get("config"))
    need(_num(config.get("chunk_seconds")) and config.get("chunk_seconds") == 60, "sixty_second_chunk_required")
    need(_num(config.get("overlap_seconds")) and config.get("overlap_seconds") == 2, "two_second_overlap_required")
    need(_num(config.get("normal_emission_seconds")) and config.get("normal_emission_seconds") == 62, "sixty_two_second_normal_emission_required")
    need(_num(config.get("watchdog_seconds")) and config.get("watchdog_seconds") == 45, "forty_five_second_watchdog_required")

    source = _map(observation.get("source"))
    need(source.get("single_selected_device") is True, "single_selected_device_required")
    need(isinstance(source.get("device_identity"), str) and bool(source.get("device_identity", "").strip()), "selected_device_identity_required")
    need(source.get("capture_ready") is True, "actual_capture_readiness_required")
    need(_int(source.get("device_recovery_count"), 0), "no_device_recovery_required")
    need(_int(source.get("privacy_transition_count"), 0), "no_privacy_transition_required")

    timing = _map(observation.get("timing"))
    fields = (
        "process_created_at", "capture_ready_at", "baseline_started_at", "baseline_completed_at",
        "tail_started_at", "tail_completed_at", "pre_stop_snapshot_at", "stop_requested_at",
        "process_exited_at",
    )
    for field in fields:
        need(_num(timing.get(field)), f"finite_{field}_required")
    if all(_num(timing.get(field)) for field in fields):
        ordered = [timing[field] for field in fields]
        # Adjacent synchronous handoffs can share a monotonic clock tick.
        # Require nondecreasing order and positive duration for real work.
        strict_pairs = (("process_created_at", "capture_ready_at"),
                        ("baseline_started_at", "baseline_completed_at"),
                        ("tail_started_at", "tail_completed_at"),
                        ("stop_requested_at", "process_exited_at"))
        fail(ordered != sorted(ordered) or any(timing[a] >= timing[b] for a, b in strict_pairs), "live_phase_order_invalid")
        ready_elapsed = timing["capture_ready_at"] - timing["process_created_at"]
        stop_elapsed = timing["stop_requested_at"] - timing["process_created_at"]
        need(ready_elapsed < config.get("watchdog_seconds", 0), "capture_readiness_exhausted_watchdog_retry_required")
        need(stop_elapsed < config.get("watchdog_seconds", 0), "sequence_exhausted_watchdog_retry_required")
        need(stop_elapsed < config.get("chunk_seconds", 0), "stop_before_chunk_duration_required")
        need(stop_elapsed < config.get("normal_emission_seconds", 0), "stop_before_normal_emission_required")

    pre = _map(observation.get("pre_stop"))
    need(pre.get("data_store_id") == store_id, "pre_stop_data_store_identity_mismatch")
    need(_int(pre.get("audio_chunk_count"), 0), "pre_stop_database_must_have_zero_audio_chunks")
    need(_int(pre.get("baseline_db_count"), 0), "pre_stop_baseline_must_not_be_persisted")
    need(_int(pre.get("tail_db_count"), 0), "pre_stop_tail_must_not_be_persisted")
    need(_int(pre.get("max_audio_chunk_id")) and pre.get("max_audio_chunk_id", -1) >= 0, "pre_stop_max_audio_chunk_id_required")
    fail(_positive_int(pre.get("audio_chunk_count")), "audio_chunk_emitted_before_stop")

    first_api_complete, first_api_bypass = _status_matrix(observation.get("api_auth_before_stop"))
    need(first_api_complete, "pre_stop_api_auth_matrix_required")
    fail(first_api_bypass, "pre_stop_protected_api_auth_bypass")

    shutdown = _map(observation.get("first_shutdown"))
    need(shutdown.get("graceful_requested") is True, "first_graceful_stop_required")
    need(shutdown.get("process_exited") is True, "first_process_exit_required")
    need(_int(shutdown.get("exit_code"), 0), "first_zero_exit_required")
    need(shutdown.get("forced") is False, "first_forced_stop_forbidden")
    need(shutdown.get("unresolved_workers") == [], "first_no_unresolved_workers_required")
    need(shutdown.get("shutdown_issues") == [], "first_no_shutdown_issues_required")
    need(isinstance(shutdown.get("process_identity"), str) and bool(shutdown.get("process_identity", "").strip()), "first_process_identity_required")
    fail(shutdown.get("forced") is True, "first_shutdown_was_forced")
    fail(isinstance(shutdown.get("unresolved_workers"), list) and bool(shutdown["unresolved_workers"]), "first_shutdown_has_unresolved_workers")
    fail(isinstance(shutdown.get("shutdown_issues"), list) and bool(shutdown["shutdown_issues"]), "first_shutdown_reported_issue")

    post = _map(observation.get("post_stop"))
    need(post.get("data_store_id") == store_id, "post_stop_data_store_identity_mismatch")
    need(_int(post.get("new_chunk_count"), 1), "exactly_one_shutdown_chunk_required")
    need(_int(post.get("total_audio_chunk_count"), 1), "fresh_store_must_contain_one_audio_chunk")
    chunk_id = post.get("chunk_id")
    need(_positive_int(chunk_id), "shutdown_chunk_id_required")
    if _int(pre.get("max_audio_chunk_id")) and _int(chunk_id):
        fail(chunk_id <= pre["max_audio_chunk_id"], "shutdown_chunk_not_newer_than_pre_stop_checkpoint")
    need(post.get("device_identity") == source.get("device_identity"), "shutdown_chunk_device_mismatch")
    need(post.get("audio_file_exists") is True, "shutdown_audio_file_required")
    need(post.get("duration_source") == "ffprobe", "ffprobe_duration_metadata_required")
    need(_num(post.get("duration_seconds")) and 0 < post.get("duration_seconds", 0) < 60, "shutdown_chunk_duration_must_be_partial")
    need(post.get("baseline_marker") == markers.get("baseline"), "shutdown_baseline_marker_identity_mismatch")
    need(post.get("tail_marker") == markers.get("tail"), "shutdown_tail_marker_identity_mismatch")
    need(_positive_int(post.get("baseline_count_same_chunk")), "baseline_positive_in_shutdown_chunk_required")
    need(_positive_int(post.get("tail_count_same_chunk")), "tail_positive_in_shutdown_chunk_required")
    need(post.get("baseline_audio_chunk_id") == chunk_id, "baseline_must_join_shutdown_chunk")
    need(post.get("tail_audio_chunk_id") == chunk_id, "tail_must_join_shutdown_chunk")

    restart = _map(observation.get("restart"))
    need(restart.get("kind") == "recorder_process", "recorder_process_restart_required")
    need(restart.get("data_store_id") == store_id, "restart_data_store_identity_mismatch")
    need(isinstance(restart.get("process_identity"), str) and bool(restart.get("process_identity", "").strip()), "restart_process_identity_required")
    if isinstance(shutdown.get("process_identity"), str) and isinstance(restart.get("process_identity"), str):
        fail(shutdown["process_identity"] == restart["process_identity"], "restart_reused_process_identity")
    need(restart.get("capture_ready") is True, "restart_capture_readiness_required")
    need(_num(restart.get("chunk_seconds")) and restart.get("chunk_seconds") == 5, "restart_five_second_chunk_required")
    need(restart.get("baseline_query_marker") == markers.get("baseline"), "restart_baseline_query_marker_mismatch")
    need(restart.get("tail_query_marker") == markers.get("tail"), "restart_tail_query_marker_mismatch")
    need(_positive_int(restart.get("baseline_search_count")), "restart_baseline_search_positive_required")
    need(_positive_int(restart.get("tail_search_count")), "restart_tail_search_positive_required")
    need(restart.get("baseline_search_chunk_id") == chunk_id, "restart_baseline_search_must_link_shutdown_chunk")
    need(restart.get("tail_search_chunk_id") == chunk_id, "restart_tail_search_must_link_shutdown_chunk")
    need(restart.get("control_query_marker") == markers.get("restart"), "restart_control_query_marker_mismatch")
    need(_positive_int(restart.get("control_db_count")), "restart_control_database_positive_required")
    need(_positive_int(restart.get("control_search_count")), "restart_control_search_positive_required")
    need(_positive_int(restart.get("control_audio_chunk_id")) and restart.get("control_audio_chunk_id") != chunk_id, "restart_control_must_use_new_chunk")
    need(_int(restart.get("device_recovery_count"), 0), "restart_no_device_recovery_required")
    need(_int(restart.get("privacy_transition_count"), 0), "restart_no_privacy_transition_required")
    restart_times = (
        "process_created_at", "capture_ready_at", "control_started_at",
        "control_completed_at", "control_observed_at",
    )
    for field in restart_times:
        need(_num(restart.get(field)), f"finite_restart_{field}_required")
    if all(_num(restart.get(field)) for field in restart_times):
        ordered_restart = [restart[field] for field in restart_times]
        fail(ordered_restart != sorted(ordered_restart)
             or restart["process_created_at"] >= restart["capture_ready_at"]
             or restart["control_started_at"] >= restart["control_completed_at"], "restart_phase_order_invalid")
    restart_api_complete, restart_api_bypass = _status_matrix(restart.get("api_auth"))
    need(restart_api_complete, "restart_api_auth_matrix_required")
    fail(restart_api_bypass, "restart_protected_api_auth_bypass")

    final = _map(observation.get("final_shutdown"))
    need(final.get("process_identity") == restart.get("process_identity"), "final_shutdown_process_identity_mismatch")
    need(final.get("graceful_requested") is True, "final_graceful_stop_required")
    need(final.get("process_exited") is True, "final_process_exit_required")
    need(_int(final.get("exit_code"), 0), "final_zero_exit_required")
    need(final.get("forced") is False, "final_forced_stop_forbidden")
    need(final.get("unresolved_workers") == [], "final_no_unresolved_workers_required")
    need(final.get("shutdown_issues") == [], "final_no_shutdown_issues_required")
    need(_num(final.get("stop_requested_at")), "finite_final_stop_requested_at_required")
    need(_num(final.get("process_exited_at")), "finite_final_process_exited_at_required")
    if _num(restart.get("control_observed_at")) and _num(final.get("stop_requested_at")) and _num(final.get("process_exited_at")):
        fail(not (restart["control_observed_at"] < final["stop_requested_at"] < final["process_exited_at"]), "final_shutdown_order_invalid")
    fail(final.get("forced") is True, "final_shutdown_was_forced")
    fail(isinstance(final.get("unresolved_workers"), list) and bool(final["unresolved_workers"]), "final_shutdown_has_unresolved_workers")
    fail(isinstance(final.get("shutdown_issues"), list) and bool(final["shutdown_issues"]), "final_shutdown_reported_issue")

    if failures:
        return {"status": "fail", "reasons": list(dict.fromkeys(failures))}
    if incomplete:
        return {"status": "incomplete", "reasons": list(dict.fromkeys(incomplete))}
    return {"status": "pass", "reasons": []}
