import copy
import unittest

from tail_acceptance import evaluate


def passing_observation():
    return {
        "evidence_version": 1,
        "data_store_id": "fresh-store-run-01",
        "readiness": {
            "gate_created_at": 1.0,
            "accepted_at": 10001.0,
            "expires_at": None,
            "deadline_enforced": False,
            "explicit_owner_ready": True,
            "nonce_match": True,
            "recording_stopped_while_waiting": True,
            "playback_stopped_while_waiting": True,
            "owned_windows_hidden_while_waiting": True,
        },
        "markers": {
            "before": "synthetic-before-7d3f",
            "tail": "synthetic-final-tail-8e4a",
            "restart": "synthetic-restart-9f5b",
        },
        "before_control": {"db_count": 1, "search_count": 1, "authenticated": True, "observed_at": 10010.0},
        "pre_tail_snapshot": {
            "observed_at": 10020.0,
            "max_audio_row_id": 41,
            "tail_db_count": 0,
            "tail_search_count": 0,
            "authenticated": True,
            "data_store_id": "fresh-store-run-01",
        },
        "tail_stimulus": {"started_at": 10021.0, "completed_at": 10023.884, "stop_requested_at": 10024.0},
        "partial_boundary": {
            "segment_window_seconds": 7.0,
            "overlap_seconds": 2.0,
            "emission_stride_seconds": 5.0,
            "tail_audio_seconds": 2.884,
            "capture_callback_after_tail_start": True,
            "normal_chunk_emitted_after_tail_start": False,
            "last_callback_at": 10023.9,
            "last_regular_emission_at": 10020.9,
        },
        "api_auth": {
            "base_url": "http://127.0.0.1:31479",
            "missing_status": 403,
            "wrong_status": 403,
            "valid_status": 200,
            "redirect_followed": False,
        },
        "first_shutdown": {
            "graceful_requested": True,
            "process_identity": "recorder-pid-100-created-1",
            "process_exited": True,
            "exit_code": 0,
            "forced": False,
            "unresolved_workers": [],
            "shutdown_issues": [],
            "observed_at": 10040.0,
        },
        "post_stop_tail": {
            "marker": "synthetic-final-tail-8e4a",
            "data_store_id": "fresh-store-run-01",
            "db_count": 1,
            "min_matching_row_id": 42,
            "observed_at": 10041.0,
        },
        "restart": {
            "started": True,
            "ready": True,
            "same_data_store": True,
            "data_store_id": "fresh-store-run-01",
            "kind": "recorder_process",
            "new_process_identity": True,
            "process_identity": "recorder-pid-200-created-2",
            "started_at": 10050.0,
            "ready_at": 10060.0,
            "tail_db_count": 1,
            "tail_query_marker": "synthetic-final-tail-8e4a",
            "tail_search_count": 1,
            "tail_search_authenticated": True,
            "control_db_count": 1,
            "control_query_marker": "synthetic-restart-9f5b",
            "control_search_count": 1,
            "control_search_authenticated": True,
            "control_observed_at": 10070.0,
            "api_auth": {
                "missing_status": 403,
                "wrong_status": 403,
                "valid_status": 200,
                "redirect_followed": False,
            },
        },
        "final_shutdown": {
            "graceful_requested": True,
            "process_identity": "recorder-pid-200-created-2",
            "process_exited": True,
            "exit_code": 0,
            "forced": False,
            "unresolved_workers": [],
            "shutdown_issues": [],
            "observed_at": 10080.0,
        },
    }


class TailAcceptanceTests(unittest.TestCase):
    def test_complete_fresh_tail_stop_and_restart_passes(self):
        self.assertEqual(evaluate(passing_observation()), {"status": "pass", "reasons": []})

    def test_long_readiness_wait_does_not_expire(self):
        item = passing_observation()
        item["readiness"]["accepted_at"] = 9_999_999.0
        self.assertEqual(evaluate(item)["status"], "pass")

    def test_tail_reused_from_earlier_chunk_fails(self):
        item = passing_observation()
        item["pre_tail_snapshot"]["tail_db_count"] = 1
        item["pre_tail_snapshot"]["tail_search_count"] = 1
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("tail_seen_before_tail_stimulus", result["reasons"])

    def test_old_row_cannot_certify_unique_tail(self):
        item = passing_observation()
        item["post_stop_tail"]["min_matching_row_id"] = item["pre_tail_snapshot"]["max_audio_row_id"]
        self.assertEqual(evaluate(item)["status"], "fail")

    def test_tail_marker_must_differ_from_controls(self):
        item = passing_observation()
        item["markers"]["tail"] = item["markers"]["before"]
        item["post_stop_tail"]["marker"] = item["markers"]["tail"]
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("markers_must_be_distinct", result["reasons"])

    def test_marker_case_whitespace_variants_are_not_distinct(self):
        item = passing_observation()
        item["markers"]["tail"] = "  SYNTHETIC-before-7d3f  "
        item["post_stop_tail"]["marker"] = item["markers"]["tail"]
        self.assertEqual(evaluate(item)["status"], "fail")

    def test_marker_substring_overlap_fails(self):
        item = passing_observation()
        item["markers"]["tail"] = item["markers"]["before"] + " tail"
        item["post_stop_tail"]["marker"] = item["markers"]["tail"]
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("marker_substring_overlap_forbidden", result["reasons"])

    def test_tail_must_be_checked_after_process_exit(self):
        item = passing_observation()
        item["post_stop_tail"]["observed_at"] = item["first_shutdown"]["observed_at"]
        self.assertEqual(evaluate(item)["status"], "fail")

    def test_full_chunk_after_tail_start_cannot_certify_partial_tail(self):
        item = passing_observation()
        item["partial_boundary"]["normal_chunk_emitted_after_tail_start"] = True
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("normal_chunk_emitted_after_tail_start", result["reasons"])

    def test_tail_must_stop_before_normal_chunk_boundary(self):
        item = passing_observation()
        item["tail_stimulus"]["stop_requested_at"] = item["partial_boundary"]["last_regular_emission_at"] + 5.0
        item["partial_boundary"]["last_callback_at"] = item["tail_stimulus"]["stop_requested_at"]
        self.assertEqual(evaluate(item)["status"], "fail")

    def test_prior_regular_emission_anchor_prevents_false_partial_pass(self):
        item = passing_observation()
        item["partial_boundary"]["last_regular_emission_at"] = 10019.0
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("stop_not_before_next_regular_emission", result["reasons"])

    def test_tail_requires_capture_callback_in_tail_window(self):
        item = passing_observation()
        item["partial_boundary"]["capture_callback_after_tail_start"] = False
        result = evaluate(item)
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("capture_callback_after_tail_start_required", result["reasons"])

    def test_forced_stop_fails_even_with_persisted_tail(self):
        item = passing_observation()
        item["first_shutdown"]["forced"] = True
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("first_shutdown_was_forced", result["reasons"])

    def test_unresolved_worker_fails(self):
        item = passing_observation()
        item["first_shutdown"]["unresolved_workers"] = ["meeting_selected_engine"]
        item["first_shutdown"]["shutdown_issues"] = ["worker_completion_unconfirmed"]
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("first_shutdown_has_unresolved_workers", result["reasons"])

    def test_auth_bypass_fails(self):
        item = passing_observation()
        item["api_auth"]["missing_status"] = 200
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("protected_api_auth_bypass", result["reasons"])

    def test_restart_auth_bypass_fails(self):
        item = passing_observation()
        item["restart"]["api_auth"]["missing_status"] = 200
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("restart_protected_api_auth_bypass", result["reasons"])

    def test_expiring_readiness_fails(self):
        item = passing_observation()
        item["readiness"]["expires_at"] = 101.0
        self.assertEqual(evaluate(item)["status"], "fail")

    def test_missing_readiness_expiry_policy_is_incomplete(self):
        item = passing_observation()
        del item["readiness"]["expires_at"]
        result = evaluate(item)
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("readiness_expiry_field_required", result["reasons"])

    def test_missing_restart_positive_control_is_incomplete(self):
        item = passing_observation()
        del item["restart"]["control_search_count"]
        result = evaluate(item)
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("restart_control_search_positive_required", result["reasons"])

    def test_same_process_manager_restart_does_not_certify_process_restart(self):
        item = passing_observation()
        item["restart"]["kind"] = "audio_manager"
        item["restart"]["new_process_identity"] = False
        result = evaluate(item)
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("recorder_process_restart_required", result["reasons"])

    def test_reused_process_identity_fails(self):
        item = passing_observation()
        item["restart"]["process_identity"] = item["first_shutdown"]["process_identity"]
        item["final_shutdown"]["process_identity"] = item["restart"]["process_identity"]
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("restart_reused_process_identity", result["reasons"])

    def test_mixed_data_store_evidence_is_incomplete(self):
        item = passing_observation()
        item["post_stop_tail"]["data_store_id"] = "different-store"
        result = evaluate(item)
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("post_stop_data_store_identity_mismatch", result["reasons"])

    def test_final_shutdown_must_follow_restart_control(self):
        item = passing_observation()
        item["final_shutdown"]["observed_at"] = item["restart"]["control_observed_at"]
        result = evaluate(item)
        self.assertEqual(result["status"], "fail")
        self.assertIn("final_shutdown_not_after_restart_control", result["reasons"])

    def test_nonfinite_times_are_incomplete(self):
        for value in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(value=value):
                item = passing_observation()
                item["tail_stimulus"]["started_at"] = value
                self.assertEqual(evaluate(item)["status"], "incomplete")

    def test_boolean_is_not_a_zero_count_or_exit_code(self):
        item = passing_observation()
        item["pre_tail_snapshot"]["tail_db_count"] = False
        item["first_shutdown"]["exit_code"] = False
        result = evaluate(item)
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("tail_must_be_absent_from_pre_tail_database", result["reasons"])
        self.assertIn("first_zero_exit_required", result["reasons"])

    def test_post_exit_phase_requires_database_only_then_search_after_restart(self):
        item = passing_observation()
        self.assertNotIn("search_count", item["post_stop_tail"])
        self.assertTrue(item["restart"]["tail_search_authenticated"])
        self.assertEqual(evaluate(item)["status"], "pass")

    def test_malformed_observation_is_incomplete(self):
        self.assertEqual(evaluate(None), {"status": "incomplete", "reasons": ["observation_mapping_required"]})

    def test_evaluator_is_pure_and_does_not_mutate_input(self):
        item = passing_observation()
        original = copy.deepcopy(item)
        evaluate(item)
        self.assertEqual(item, original)


if __name__ == "__main__":
    unittest.main()
