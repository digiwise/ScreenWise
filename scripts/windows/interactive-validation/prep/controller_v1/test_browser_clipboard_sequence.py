import unittest

from browser_clipboard_sequence import PHASES, assess_browser_clipboard, preparation_manifest


def passing_phases():
    all_forbidden = {marker for phase in PHASES for marker in phase.forbidden_markers}
    rows = []
    for phase in PHASES:
        rows.append({
            "name": phase.name,
            "verified": True,
            "positive_deltas": {marker: 1 for marker in phase.positive_markers},
            "forbidden_deltas": {marker: 0 for marker in all_forbidden},
        })
    return rows


def passing_cleanup():
    return {
        "recorder_stopped": True,
        "clipboard_restore_attempted_after_stop": True,
        "clipboard_safe_terminal_state": True,
        "browser_process_tree_stopped": True,
        "loopback_server_stopped": True,
        "fixture_stopped": True,
    }


class BrowserClipboardSequenceTests(unittest.TestCase):
    def test_manifest_is_inert_and_short(self):
        manifest = preparation_manifest()
        self.assertFalse(manifest["interactive_execution_enabled"])
        self.assertEqual(manifest["estimated_active_seconds"], 25)
        self.assertIn("reviewed_exact_browser_firewall_scope", manifest["hard_preconditions"])

    def test_scoped_evidence_can_become_eligible(self):
        result = assess_browser_clipboard(passing_phases(), passing_cleanup())
        self.assertEqual(result["status"], "eligible_for_scoped_live_evidence_review")

    def test_forbidden_marker_is_failure(self):
        rows = passing_phases()
        rows[1]["forbidden_deltas"]["hidden coral orchard"] = 1
        self.assertEqual(assess_browser_clipboard(rows, passing_cleanup())["status"], "failed")

    def test_missing_positive_control_is_incomplete(self):
        rows = passing_phases()
        rows[0]["positive_deltas"]["public browser cedar"] = 0
        result = assess_browser_clipboard(rows, passing_cleanup())
        self.assertEqual(result, {"status": "incomplete", "reason": "positive_control_missing"})

    def test_restore_without_verified_stop_is_incomplete(self):
        cleanup = passing_cleanup()
        cleanup["recorder_stopped"] = False
        result = assess_browser_clipboard(passing_phases(), cleanup)
        self.assertEqual(result, {"status": "incomplete", "reason": "cleanup_unverified"})

    def test_negative_counts_and_wrong_sequence_are_rejected(self):
        rows = passing_phases()
        rows[2]["forbidden_deltas"]["forbidden cyan orchard"] = -1
        self.assertEqual(assess_browser_clipboard(rows, passing_cleanup())["reason"], "marker_delta_schema_invalid")
        self.assertEqual(assess_browser_clipboard(rows[:-1], passing_cleanup())["reason"], "phase_sequence_incomplete")


if __name__ == "__main__":
    unittest.main()
