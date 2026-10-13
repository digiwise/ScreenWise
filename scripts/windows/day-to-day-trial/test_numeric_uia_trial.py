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


if __name__ == "__main__":
    unittest.main()
