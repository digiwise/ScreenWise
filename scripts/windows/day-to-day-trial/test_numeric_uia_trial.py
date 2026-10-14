# screenpipe — AI that knows everything you've seen, said, or heard
# https://screenpi.pe
import json
import sqlite3
import unittest
from unittest.mock import patch
import numeric_uia_trial as trial


class NumericTrialTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.execute("CREATE TABLE frames(timestamp TEXT, capture_privacy TEXT, captured_text TEXT)")

    def tearDown(self):
        self.db.close()

    def add(self, decision, timestamp="2026-10-13T10:00:00.000Z"):
        self.db.execute("INSERT INTO frames VALUES(?,?,?)", (timestamp, decision, "PRIVATE_CANARY"))

    def test_aggregate_never_returns_payload_or_unknown_reason(self):
        self.add(json.dumps({"outcome": "capture_failed", "app_name": "PRIVATE_CANARY",
                             "accessibility": {"cause": "uia_walk_timeout", "processing": "withheld"}}))
        self.add(json.dumps({"outcome": "capture_redacted", "window_name": "PRIVATE_CANARY"}))
        self.add(json.dumps({"outcome": "PRIVATE_CANARY"}))
        result = trial.aggregate(self.db, "2026-10-13T09:00:00.000Z")
        self.assertEqual(result["frames"], 3)
        self.assertEqual(result["failed"], 1)
        self.assertEqual(result["redacted"], 1)
        self.assertEqual(result["uia_walk_timeout"], 1)
        self.assertNotIn("PRIVATE_CANARY", json.dumps(result))

    def test_bad_json_and_missing_metadata_are_not_false_success(self):
        self.add("broken_PRIVATE_CANARY")
        self.add(None)
        result = trial.aggregate(self.db, "2026-10-13T09:00:00.000Z")
        self.assertEqual(result["frames"], 2)
        self.assertEqual(result["decision_rows"], 0)
        self.assertEqual(result["other_outcomes"], 0)

    def test_interval_is_bounded_and_empty_counts_zero(self):
        self.add("{}", "2026-10-12T10:00:00.000Z")
        result = trial.aggregate(self.db, "2026-10-13T09:00:00.000Z")
        self.assertEqual(result["frames"], 0)

    def test_api_projection_is_numeric_allowlist_and_missing_is_null(self):
        def fetch(_base, endpoint, _token):
            if endpoint == "/health":
                return {"drm_content_paused": False, "window_name": "PRIVATE_CANARY"}, 200, 1
            return {"frames_captured": 5, "capture_attempts": "PRIVATE_CANARY", "unknown": 9}, 200, 1
        with patch.object(trial, "fetch", fetch):
            result = trial.sample({"port": 31579}, "PRIVATE_CANARY", self.db, "2026-10-13T09:00:00.000Z")
        self.assertEqual(result["vision"]["frames_captured"], 5)
        self.assertIsNone(result["vision"]["capture_attempts"])
        self.assertIsNone(result["admission_latency_ms"])
        self.assertNotIn("PRIVATE_CANARY", json.dumps(result))
        self.assertNotIn("unknown", result["vision"])

    def test_uia_vectors_discard_content_and_unexpected_keys(self):
        result = trial.uia_projection({"histogram_counts": [1, "PRIVATE_CANARY", 2],
                                       "reason_codes": ["PRIVATE_CANARY"], "tree": "PRIVATE_CANARY"})
        self.assertEqual(result["histogram_counts"], [1, None, 2])
        self.assertNotIn("PRIVATE_CANARY", json.dumps(result))

    def test_timing_projection_keeps_only_fixed_numeric_vectors(self):
        source = {"stage_names": ["PRIVATE_CANARY"],
                  "stages": {"calls": [1] * trial.TIMING_STAGE_COUNT,
                             "total_us": [2] * trial.TIMING_STAGE_COUNT,
                             "max_us": [3] * trial.TIMING_STAGE_COUNT,
                             "content": "PRIVATE_CANARY"},
                  "timeout_stage_counts": [4] * trial.TIMING_STAGE_COUNT,
                  "rule_type_names": ["PRIVATE_CANARY"],
                  "rule_types": {"calls": [5] * trial.TIMING_RULE_COUNT,
                                 "total_us": [6] * trial.TIMING_RULE_COUNT,
                                 "max_us": [7] * trial.TIMING_RULE_COUNT},
                  "rule_match_counts": [8] * trial.TIMING_RULE_COUNT}
        result = trial.timing_projection(source)
        self.assertEqual(result["stages"]["calls"][0], 1)
        self.assertEqual(result["rule_match_counts"][-1], 8)
        self.assertNotIn("PRIVATE_CANARY", json.dumps(result))
        self.assertIsNone(trial.timing_projection({"timeout_stage_counts": [1]})
                          ["timeout_stage_counts"])

    def test_recovery_only_counts_confirmed_outcomes_on_same_device(self):
        self.db.execute("ALTER TABLE frames ADD COLUMN device_name TEXT")
        rows = [("2026-10-13T10:00:00.000Z", "capture_failed", "a"),
                ("2026-10-13T10:00:01.000Z", "full_monitor", "b"),
                ("2026-10-13T10:00:02.000Z", "full_monitor", "a")]
        for timestamp, outcome, device in rows:
            self.db.execute("INSERT INTO frames(timestamp,capture_privacy,device_name) VALUES(?,?,?)",
                            (timestamp, json.dumps({"outcome": outcome}), device))
        result = trial.aggregate(self.db, "2026-10-13T09:00:00.000Z")
        self.assertEqual(result["failure_to_admitted_transitions"], 1)
        self.assertAlmostEqual(result["last_failure_to_admitted_avg_ms"], 2000, delta=1)

    def test_process_projection_bounds_rows_and_discards_identity_and_content(self):
        row = {"basename": "code.exe", "attempts": 3, "failed": 1,
               "window_name": "PRIVATE_CANARY", "stage_calls": [2] * 21,
               "failed_stage_total_us": [9] * 21, "mode_attempt_counts": [0, 3],
               "retry_cause_counts": [1, 0, 0, 0], "failure_phase_counts": [1, 0, 0],
               "native_timeout_hresult_counts": [1], "failed_native_timeout_hresult_counts": [1],
               "failed_retry_cause_counts": [1, 0, 0, 0], "failures_with_budget": 1,
               "failed_other_hresult_count": 0, "reason_counts": [0] * 10,
               "content": "PRIVATE_CANARY"}
        result = trial.process_projection({"processes": [row] * 40, "total": row,
                                           "unknown": {"basename": "PRIVATE_CANARY"}})
        self.assertEqual(len(result["processes"]), 32)
        self.assertEqual(result["processes"][0]["selector"], 1)
        self.assertEqual(result["processes"][0]["mode_attempt_counts"], [0, 3])
        self.assertEqual(result["processes"][0]["failed_stage_total_us"], [9] * 21)
        self.assertEqual(result["processes"][0]["failed_native_timeout_hresult_counts"], [1])
        self.assertEqual(result["processes"][0]["failures_with_budget"], 1)
        self.assertEqual(result["processes"][0]["reason_counts"], [0] * 10)
        self.assertEqual(result["unknown"]["selector"], 0)
        serialized = json.dumps(result)
        for content in ("PRIVATE_CANARY", "code.exe", "basename", "window_name"):
            self.assertNotIn(content, serialized)
        self.assertIsNone(trial.process_projection({"total": {"retry_cause_counts": [1]}})
                          ["total"]["retry_cause_counts"])

    def test_legacy_process_reasons_remain_distinguishable_from_new_schema(self):
        old = trial.process_projection({"schema_version": 1, "total": {"reason_counts": [2] * 9}})
        new = trial.process_projection({"schema_version": 2, "total": {"reason_counts": [3] * 10}})
        self.assertEqual(old["total"]["reason_counts"], [2] * 9)
        self.assertEqual(new["total"]["reason_counts"], [3] * 10)
        self.assertIsNone(trial.process_projection({"schema_version": 2,
                          "total": {"reason_counts": [2] * 9}})["total"]["reason_counts"])

    def test_admission_latency_uses_published_privacy_not_optional_text_wall_time(self):
        def fetch(_base, endpoint, _token):
            if endpoint == "/vision/uia-metrics":
                return {"attempts": 4, "total_duration_us": 2400000,
                        "timing": {"privacy_phase": {"published_admissions": 2,
                                  "published_mandatory_complete_total_us": 200000,
                                  "published_mandatory_complete_bucket_counts": [0, 2, 0, 0, 0, 0, 0]}}}, 200, 1
            return {}, 200, 1
        with patch.object(trial, "fetch", fetch):
            result = trial.sample({"port": 31579}, "PRIVATE_CANARY", self.db, "2026-10-13T09:00:00.000Z")
        self.assertEqual(result["admission_latency_ms"], 100)
        self.assertEqual(result["inspection_wall_latency_ms"], 600)
        self.assertEqual(result["timing"]["privacy_phase"]
                         ["published_mandatory_complete_bucket_counts"], [0, 2, 0, 0, 0, 0, 0])

    def test_current_redacted_outcome_is_not_counted_as_capture(self):
        self.add(json.dumps({"outcome": "redacted"}))
        result = trial.aggregate(self.db, "2026-10-13T09:00:00.000Z")
        self.assertEqual(result["redacted"], 1)
        self.assertEqual(result["other_outcomes"], 0)


if __name__ == "__main__":
    unittest.main()
