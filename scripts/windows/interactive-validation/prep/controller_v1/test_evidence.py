import json
import sqlite3
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import evidence


class _Response:
    def __init__(self, status=200, body=b""):
        self.status = status
        self.body = body
    def __enter__(self): return self
    def __exit__(self, *args): return False
    def read(self, size=-1): return self.body[:size]


class EvidenceTests(unittest.TestCase):
    def test_collect_db_is_aggregate_only_and_marker_bound(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".controller-owned").touch()
            db = sqlite3.connect(root / "db.sqlite")
            db.executescript("""
                    CREATE TABLE frames (id INTEGER, ocr_text TEXT, window_title TEXT, url TEXT);
                    CREATE TABLE audio_transcriptions (id INTEGER, transcription TEXT, device_name TEXT);
                    CREATE TABLE ui_events (id INTEGER, event_type TEXT, text_content TEXT,
                        app_name TEXT, window_title TEXT, url TEXT, metadata TEXT, reason_code TEXT);
                    INSERT INTO frames VALUES (1, 'marker-alpha', 'secret title', 'https://private.test');
                    INSERT INTO audio_transcriptions VALUES (1, 'marker-alpha', 'Synthetic Output');
                    INSERT INTO ui_events VALUES (2, 'app_switch', 'marker-alpha', 'app', 'title', 'url', '{}', NULL);
                """)
            db.execute("INSERT INTO ui_events VALUES (1, 'privacy_notice', NULL, NULL, NULL, NULL, NULL, 'locked')")
            db.commit()
            db.close()
            result = evidence.collect_db(root, ["marker-alpha"])
            self.assertEqual(result["quick_check"], "ok")
            self.assertEqual(result["marker_hit_counts"]["frames"]["marker-alpha"], 1)
            self.assertEqual(result["marker_hit_counts"]["audio_transcriptions"]["marker-alpha"], 1)
            self.assertEqual(result["safe_notices"]["category_counts"], {"legacy": 1})
            self.assertEqual(result["safe_notices"]["reason_counts"], {"locked": 1})
            self.assertNotIn("secret title", repr(result))
            injected = evidence.collect_db(root, ["x' OR 1=1 --"])
            self.assertEqual(injected["marker_hit_counts"]["frames"]["x' OR 1=1 --"], 0)

    def test_collect_db_counts_allowlisted_notice_payloads_without_echoing_values(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".controller-owned").touch()
            db = sqlite3.connect(root / "db.sqlite")
            db.execute("CREATE TABLE ui_events (id INTEGER, event_type TEXT, text_content TEXT)")
            audio_issues = [
                "producer_stop_timeout",
                "producer_stop_failed",
                "consumer_drain_timeout",
                "consumer_drain_failed",
                "queued_work_discarded",
                "worker_completion_unconfirmed",
            ]
            payloads = [
                {"reason": "wts_session_unlocked"},
                {"reason": "wts_query_failed"},
                *({"issue": issue} for issue in audio_issues),
                {"issue": "private-arbitrary-value"},
                {"reason": "wts_session_locked", "message": "private message"},
            ]
            db.executemany(
                "INSERT INTO ui_events VALUES (?, 'privacy_notice', ?)",
                [(index, json.dumps(payload)) for index, payload in enumerate(payloads)],
            )
            db.commit()
            db.close()

            result = evidence.collect_db(root, [])
            self.assertEqual(result["safe_notices"], {
                "category_counts": {"audio_shutdown": 6, "unknown": 2, "windows_lock": 2},
                "reason_counts": {
                    "consumer_drain_failed": 1,
                    "consumer_drain_timeout": 1,
                    "producer_stop_failed": 1,
                    "producer_stop_timeout": 1,
                    "queued_work_discarded": 1,
                    "unknown": 2,
                    "worker_completion_unconfirmed": 1,
                    "wts_query_failed": 1,
                    "wts_session_unlocked": 1,
                },
            })
            rendered = repr(result)
            self.assertNotIn("private-arbitrary-value", rendered)
            self.assertNotIn("private message", rendered)

    def test_collect_db_requires_controller_marker(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            sqlite3.connect(root / "db.sqlite").close()
            with self.assertRaises(ValueError): evidence.collect_db(root, [])

    def test_verify_api_checks_auth_shape_without_body(self):
        seen = []
        def fake_open(request, timeout=0):
            seen.append((request.full_url, dict(request.header_items())))
            return _Response(200 if request.headers.get("Authorization", "").endswith("secret") or request.full_url.endswith("/health") else 403)
        with patch.object(evidence, "_open", fake_open):
            result = evidence.verify_api("http://127.0.0.1:31479", "secret", "2026-01-01T00:00:00Z", "2026-01-01T00:01:00Z")
        self.assertTrue(result["checks"]["health_ok"])
        self.assertTrue(result["checks"]["protected_auth_shape_ok"])
        self.assertTrue(result["passed"])
        self.assertEqual(set(result["cases"]["search"]), {"missing", "wrong", "valid"})
        self.assertEqual(result["safe_delivery"], {})
        self.assertTrue(any("start_time=" in u and "end_time=" in u for u, _ in seen))
        self.assertNotIn("secret", repr(result))
        self.assertNotIn("Authorization", repr(result))

    def test_verify_api_rejects_non_loopback(self):
        with self.assertRaises(ValueError): evidence.verify_api("http://localhost:31479", "secret", "a", "b")

    def test_api_snapshot_filters_to_numeric_allowlist(self):
        body = json.dumps({"chunks_sent": 5, "audio_level_rms": 0.25,
                           "transcription_errors": 0, "device_name": "private"}).encode()
        with patch.object(evidence, "_open", lambda request, timeout=0: _Response(200, body)):
            self.assertEqual(evidence.api_snapshot("http://127.0.0.1:31479", "secret"),
                             {"audio_level_rms": 0.25, "chunks_sent": 5, "transcription_errors": 0})

    def test_evaluate_result_requires_controls_for_negative_modes(self):
        self.assertEqual(evidence.evaluate_result("privacy", {})["status"], "incomplete")
        complete = {"mode_known": True, "evidence_complete": True, "api_verified": True, "os_verified": True,
                    "db_verified": True, "before_positive": True, "after_positive": True,
                    "protected_controls": True, "verified_phases": True}
        self.assertEqual(evidence.evaluate_result("privacy", complete)["status"], "pass")
        complete["unexpected_forbidden_capture"] = True
        self.assertEqual(evidence.evaluate_result("privacy", complete)["status"], "fail")
        self.assertEqual(evidence.evaluate_result("audio", {"required": ["audio_rows"]})["status"], "incomplete")
        self.assertEqual(evidence.evaluate_result("audio-output", {})["status"], "incomplete")


if __name__ == "__main__":
    unittest.main()
