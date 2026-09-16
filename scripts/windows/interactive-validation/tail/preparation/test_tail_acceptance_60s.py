import copy
import unittest

from tail_acceptance_60s import evaluate


def passing():
    return {
        "evidence_version": 1,
        "data_store_id": "fresh-store-60s-01",
        "markers": {"baseline": "cobalt meadow seven", "tail": "violet harbor nine", "restart": "silver orchard five"},
        "readiness": {"explicit_owner_ready": True, "nonce_match": True, "expires_at": None, "deadline_enforced": False},
        "config": {"chunk_seconds": 60.0, "overlap_seconds": 2.0, "normal_emission_seconds": 62.0, "watchdog_seconds": 45.0},
        "source": {"single_selected_device": True, "device_identity": "selected-output-device", "capture_ready": True, "device_recovery_count": 0, "privacy_transition_count": 0},
        "timing": {
            "process_created_at": 0.0, "capture_ready_at": 5.0,
            "baseline_started_at": 5.5, "baseline_completed_at": 16.283,
            "tail_started_at": 16.5, "tail_completed_at": 19.384,
            "pre_stop_snapshot_at": 19.5, "stop_requested_at": 19.7,
            "process_exited_at": 26.0,
        },
        "pre_stop": {"data_store_id": "fresh-store-60s-01", "audio_chunk_count": 0, "baseline_db_count": 0, "tail_db_count": 0, "max_audio_chunk_id": 0},
        "api_auth_before_stop": {"base_url": "http://127.0.0.1:31479", "missing_status": 403, "wrong_status": 403, "valid_status": 200, "redirect_followed": False},
        "first_shutdown": {"graceful_requested": True, "process_exited": True, "exit_code": 0, "forced": False, "unresolved_workers": [], "shutdown_issues": [], "process_identity": "pid100-created1"},
        "post_stop": {
            "data_store_id": "fresh-store-60s-01", "new_chunk_count": 1, "total_audio_chunk_count": 1,
            "chunk_id": 1, "device_identity": "selected-output-device", "audio_file_exists": True,
            "duration_source": "ffprobe", "duration_seconds": 14.0,
            "baseline_marker": "cobalt meadow seven", "tail_marker": "violet harbor nine",
            "baseline_count_same_chunk": 1, "tail_count_same_chunk": 1,
            "baseline_audio_chunk_id": 1, "tail_audio_chunk_id": 1,
        },
        "restart": {
            "kind": "recorder_process", "data_store_id": "fresh-store-60s-01",
            "process_identity": "pid200-created2", "capture_ready": True, "chunk_seconds": 5.0,
            "baseline_query_marker": "cobalt meadow seven", "tail_query_marker": "violet harbor nine",
            "baseline_search_count": 1, "tail_search_count": 1,
            "baseline_search_chunk_id": 1, "tail_search_chunk_id": 1,
            "control_query_marker": "silver orchard five", "control_db_count": 1,
            "control_search_count": 1, "control_audio_chunk_id": 2,
            "device_recovery_count": 0, "privacy_transition_count": 0,
            "process_created_at": 40.0, "capture_ready_at": 45.0,
            "control_started_at": 45.5, "control_completed_at": 56.273,
            "control_observed_at": 60.0,
            "api_auth": {"base_url": "http://127.0.0.1:31479", "missing_status": 403, "wrong_status": 403, "valid_status": 200, "redirect_followed": False},
        },
        "final_shutdown": {"process_identity": "pid200-created2", "graceful_requested": True, "process_exited": True, "exit_code": 0, "forced": False, "unresolved_workers": [], "shutdown_issues": [], "stop_requested_at": 60.5, "process_exited_at": 66.0},
    }


class Acceptance60sTests(unittest.TestCase):
    def test_complete_single_flush_run_passes(self): self.assertEqual(evaluate(passing()), {"status": "pass", "reasons": []})
    def test_adjacent_handoffs_may_share_a_clock_tick(self):
        item = passing()
        timing = item["timing"]
        timing["baseline_started_at"] = timing["capture_ready_at"]
        timing["tail_started_at"] = timing["baseline_completed_at"]
        timing["pre_stop_snapshot_at"] = timing["tail_completed_at"]
        item["restart"]["control_started_at"] = item["restart"]["capture_ready_at"]
        item["restart"]["control_observed_at"] = item["restart"]["control_completed_at"]
        self.assertEqual(evaluate(item), {"status": "pass", "reasons": []})
    def test_reversed_handoff_still_fails(self):
        item = passing()
        item["timing"]["tail_started_at"] = item["timing"]["baseline_completed_at"] - 0.001
        self.assertIn("live_phase_order_invalid", evaluate(item)["reasons"])
    def test_zero_duration_work_still_fails(self):
        for start, end in (("baseline_started_at", "baseline_completed_at"), ("tail_started_at", "tail_completed_at"), ("stop_requested_at", "process_exited_at")):
            with self.subTest(end=end):
                item = passing(); item["timing"][end] = item["timing"][start]
                self.assertIn("live_phase_order_invalid", evaluate(item)["reasons"])
        item = passing(); item["restart"]["control_completed_at"] = item["restart"]["control_started_at"]
        self.assertIn("restart_phase_order_invalid", evaluate(item)["reasons"])
    def test_input_is_not_mutated(self):
        item = passing(); original = copy.deepcopy(item); evaluate(item); self.assertEqual(item, original)
    def test_one_shutdown_chunk_is_required(self):
        item = passing(); item["post_stop"]["new_chunk_count"] = 2; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_pre_stop_chunk_is_hard_failure(self):
        item = passing(); item["pre_stop"]["audio_chunk_count"] = 1
        result = evaluate(item); self.assertEqual(result["status"], "fail"); self.assertIn("audio_chunk_emitted_before_stop", result["reasons"])
    def test_both_markers_must_join_same_shutdown_chunk(self):
        item = passing(); item["post_stop"]["tail_audio_chunk_id"] = 2; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_duration_must_be_ffprobe_measured_partial(self):
        for key, value in (("duration_source", "estimate"), ("duration_seconds", 60.0)):
            with self.subTest(key=key):
                item = passing(); item["post_stop"][key] = value; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_readiness_too_slow_requires_retry(self):
        item = passing(); item["timing"]["capture_ready_at"] = 45.0
        item["timing"]["baseline_started_at"] = 45.1; item["timing"]["baseline_completed_at"] = 55.883
        item["timing"]["tail_started_at"] = 56.0; item["timing"]["tail_completed_at"] = 58.884
        item["timing"]["pre_stop_snapshot_at"] = 59.0; item["timing"]["stop_requested_at"] = 59.1; item["timing"]["process_exited_at"] = 61.0
        result = evaluate(item); self.assertEqual(result["status"], "incomplete"); self.assertIn("capture_readiness_exhausted_watchdog_retry_required", result["reasons"])
    def test_stop_at_watchdog_requires_retry(self):
        item = passing(); item["timing"]["stop_requested_at"] = 45.0; item["timing"]["process_exited_at"] = 50.0
        result = evaluate(item); self.assertEqual(result["status"], "incomplete"); self.assertIn("sequence_exhausted_watchdog_retry_required", result["reasons"])
    def test_nonfinite_time_is_incomplete(self):
        item = passing(); item["timing"]["tail_started_at"] = float("nan"); self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_device_recovery_invalidates_run(self):
        item = passing(); item["source"]["device_recovery_count"] = 1; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_privacy_transition_invalidates_run(self):
        item = passing(); item["source"]["privacy_transition_count"] = 1; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_shutdown_issue_is_hard_failure(self):
        item = passing(); item["first_shutdown"]["shutdown_issues"] = ["queued_work_discarded"]
        result = evaluate(item); self.assertEqual(result["status"], "fail"); self.assertIn("first_shutdown_reported_issue", result["reasons"])
    def test_forced_shutdown_is_hard_failure(self):
        item = passing(); item["first_shutdown"]["forced"] = True; self.assertEqual(evaluate(item)["status"], "fail")
    def test_restart_auth_bypass_is_hard_failure(self):
        item = passing(); item["restart"]["api_auth"]["missing_status"] = 200; self.assertEqual(evaluate(item)["status"], "fail")
    def test_api_must_be_exact_loopback(self):
        item = passing(); item["restart"]["api_auth"]["base_url"] = "http://localhost:31479"; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_restart_search_must_link_original_chunk(self):
        item = passing(); item["restart"]["tail_search_chunk_id"] = 99; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_restart_control_must_use_new_chunk(self):
        item = passing(); item["restart"]["control_audio_chunk_id"] = 1; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_new_process_identity_required(self):
        item = passing(); item["restart"]["process_identity"] = item["first_shutdown"]["process_identity"]
        item["final_shutdown"]["process_identity"] = item["restart"]["process_identity"]
        self.assertEqual(evaluate(item)["status"], "fail")
    def test_mixed_store_is_incomplete(self):
        item = passing(); item["post_stop"]["data_store_id"] = "other"; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_boolean_cannot_be_zero_count(self):
        item = passing(); item["pre_stop"]["audio_chunk_count"] = False; self.assertEqual(evaluate(item)["status"], "incomplete")
    def test_marker_overlap_fails(self):
        item = passing(); item["markers"]["tail"] = "Cobalt   Meadow Seven tail"
        result = evaluate(item); self.assertEqual(result["status"], "fail"); self.assertIn("marker_substring_overlap_forbidden", result["reasons"])
    def test_final_shutdown_must_follow_control(self):
        item = passing(); item["final_shutdown"]["stop_requested_at"] = item["restart"]["control_observed_at"]
        result = evaluate(item); self.assertEqual(result["status"], "fail"); self.assertIn("final_shutdown_order_invalid", result["reasons"])


if __name__ == "__main__": unittest.main()
