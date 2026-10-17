import json
import sqlite3
import unittest
import screening_metrics as metrics


class ScreeningMetricsTests(unittest.TestCase):
    def test_decision_and_native_metadata_codes_are_reported(self):
        self.assertEqual(metrics.safe("decision", "off_monitor"), "off_monitor")
        self.assertEqual(metrics.safe("native_metadata", "native_title"), "native_title")
        self.assertEqual(metrics.safe("decision", "Not A Code"), "other")

    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        self.db.execute("CREATE TABLE frames(timestamp TEXT, app_name TEXT, text_source TEXT, "
                        "capture_privacy TEXT, window_name TEXT, ocr_text TEXT, full_text TEXT)")

    def tearDown(self):
        self.db.close()

    def add(self, app, source, decision, timestamp="2026-10-15T11:10:00.000Z"):
        self.db.execute("INSERT INTO frames VALUES(?,?,?,?,?,?,?)",
                        (timestamp, app, source, json.dumps(decision), "PRIVATE_CANARY", "PRIVATE_CANARY",
                         "PRIVATE_CANARY"))

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
        self.db.execute("INSERT INTO frames VALUES('2026-10-15T11:10:00Z','a.exe','ocr','broken',NULL,NULL,NULL)")
        totals = next(rows for t, _, rows in self.output() if t == "Frames since start")[0]
        self.assertEqual(totals[0], 0)

    def test_privacy_blockers_report_only_codes_apps_and_text_presence(self):
        self.add("Code.exe", "privacy_placeholder", {"outcome": "capture_redacted", "reason": "inconsistent_focus",
            "blockers": [{"app": "code.exe", "reasons": ["foreground_not_enumerated"], "foreground": True},
                         {"app": "PRIVATE/CANARY", "reasons": ["PRIVATE CANARY"], "foreground": False}]})
        result = self.output()
        self.assertNotIn("CANARY", json.dumps(result))
        rows = next(rows for t, _, rows in result if t == "Capture reason x foreground blocker reason")
        self.assertEqual(rows, [["inconsistent_focus", "foreground_not_enumerated", 1]])
        presence = next(rows for t, _, rows in result if t == "Stored text presence x text_source")
        self.assertEqual(presence, [["privacy_placeholder", 1, 1, 0]])

    def test_no_safe_window_detail_reports_codes_and_counts_only(self):
        self.add("", "privacy_placeholder", {"reason": "no_safe_active_window", "no_safe_window": {
            "phase": "initial", "path": "scoped", "cause": "foreground_on_other_monitor",
            "foreground_skip": None, "window_app": "PRIVATE/CANARY", "foreground_on_monitor": False,
            "visible_windows": 3, "excluded_visible_windows": 1}})
        result = self.output()
        self.assertNotIn("CANARY", json.dumps(result))
        rows = next(rows for t, _, rows in result if t == "No-safe-window detail")
        self.assertEqual(rows, [["initial", "scoped", "foreground_on_other_monitor", None, "other", 0, 1, 3.0, 1.0]])


if __name__ == "__main__":
    unittest.main()
