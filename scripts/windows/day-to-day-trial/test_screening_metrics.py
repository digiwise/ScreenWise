import json
import sqlite3
import unittest
import screening_metrics as metrics


class ScreeningMetricsTests(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.execute("CREATE TABLE frames(timestamp TEXT, app_name TEXT, text_source TEXT, "
                        "capture_privacy TEXT, window_name TEXT, ocr_text TEXT)")

    def tearDown(self):
        self.db.close()

    def add(self, app, source, decision, timestamp="2026-10-15T11:10:00.000Z"):
        self.db.execute("INSERT INTO frames VALUES(?,?,?,?,?,?)",
                        (timestamp, app, source, json.dumps(decision), "PRIVATE_CANARY", "PRIVATE_CANARY"))

    def output(self, since="2026-10-15T07:00:00Z"):
        return list(metrics.run(self.db, since, list(metrics.SECTIONS)))

    def test_aggregates_never_return_unpermitted_or_unexpected_text(self):
        self.add("Code.exe", "ocr", {"content_screening": {"outcome": "passed", "ocr_forced": False, "ocr_ms": 180,
                                                           "screen_ms": 2, "reasons": ["PRIVATE CANARY"]}})
        self.add("", "privacy_placeholder", {"reason": "accessibility_inspection", "accessibility": {
            "processing": "withheld", "cause": "uia_walk_timeout", "phase": "PRIVATE CANARY",
            "sampled_target_app": "PRIVATE/CANARY"}})
        self.add("PRIVATE/CANARY", "ocr", {"reason": "PRIVATE CANARY"})
        result = self.output()
        text = json.dumps(result)
        self.assertNotIn("CANARY", text)
        reasons = dict((r[0], r[2]) for t, _, rows in result if t == "Capture reason x text_source" for r in rows)
        self.assertEqual(reasons["accessibility_inspection"], 1)
        self.assertEqual(reasons["other"], 1)
        totals = next(rows for t, _, rows in result if t == "Frames since start")[0]
        self.assertEqual(totals[:3], [3, 1, 1])

    def test_window_is_bounded_and_empty_database_is_quiet(self):
        self.add("Code.exe", "ocr", {"content_screening": {"outcome": "passed"}}, "2026-10-14T00:00:00.000Z")
        totals = next(rows for t, _, rows in self.output() if t == "Frames since start")[0]
        self.assertEqual(totals[0], 0)

    def test_bad_json_is_not_counted_as_screened(self):
        self.db.execute("INSERT INTO frames VALUES('2026-10-15T11:10:00Z','a.exe','ocr','broken',NULL,NULL)")
        totals = next(rows for t, _, rows in self.output() if t == "Frames since start")[0]
        self.assertEqual(totals[0], 0)


if __name__ == "__main__":
    unittest.main()
