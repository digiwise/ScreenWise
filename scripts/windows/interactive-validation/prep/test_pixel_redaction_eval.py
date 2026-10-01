"""Offline tests for the fixed synthetic pixel evaluator and evidence gate."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from PIL import Image, ImageDraw

from pixel_redaction_eval import (
    Box,
    EvaluationError,
    PixelPlan,
    evaluate_fail_closed_pair,
    evaluate_pixel_pair,
    evaluate_run,
)


class PixelRedactionEvaluationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.source_path = self.root / "fixture.jpg"
        self.redacted_path = self.root / "redacted.jpg"
        self.failure_path = self.root / "failure.jpg"
        self.pii = Box(30, 30, 190, 90)
        self.ordinary = Box(30, 115, 155, 155)
        self.sentinel = Box(220, 110, 305, 165)
        self.plan = PixelPlan(self.pii, self.ordinary, self.sentinel)

        image = Image.new("RGB", (320, 180), "white")
        draw = ImageDraw.Draw(image)
        draw.rectangle((self.pii.left, self.pii.top, self.pii.right - 1,
                        self.pii.bottom - 1), fill=(20, 175, 230))
        draw.text((42, 50), "SYNTHETIC PII REGION", fill=(10, 25, 80))
        draw.rectangle((self.ordinary.left, self.ordinary.top,
                        self.ordinary.right - 1, self.ordinary.bottom - 1),
                       fill=(225, 240, 190))
        draw.text((38, 126), "ORDINARY CONTROL", fill=(25, 50, 20))
        for y in range(self.sentinel.top, self.sentinel.bottom):
            for x in range(self.sentinel.left, self.sentinel.right):
                image.putpixel((x, y), (((x // 12) * 47) % 255,
                                        ((y // 12) * 61) % 255,
                                        (((x // 12) + (y // 12)) * 37) % 255))
        image.save(self.source_path, "JPEG", quality=96)

        with Image.open(self.source_path) as original:
            redacted = original.convert("RGB")
        ImageDraw.Draw(redacted).rectangle(
            (self.pii.left, self.pii.top, self.pii.right - 1, self.pii.bottom - 1),
            fill="black")
        redacted.save(self.redacted_path, "JPEG", quality=91)

        failure = Image.new("RGB", (320, 180), (18, 18, 20))
        ImageDraw.Draw(failure).text((80, 80), "PII REDACTION FAILED", fill="white")
        failure.save(self.failure_path, "JPEG", quality=90)

    def tearDown(self):
        self.temp.cleanup()

    def test_pixel_pair_requires_changed_pii_and_preserved_controls(self):
        result = evaluate_pixel_pair(self.source_path, self.redacted_path, self.plan)
        self.assertEqual(result["status"], "passed_pixel_pair")
        self.assertTrue(result["checks"]["pii_region_obscured"])
        self.assertTrue(result["checks"]["ordinary_region_preserved"])
        self.assertTrue(result["checks"]["sentinel_region_preserved"])
        self.assertNotIn("SYNTHETIC PII REGION", str(result))

    def test_missing_redaction_fails_without_exposing_image_data(self):
        result = evaluate_pixel_pair(self.source_path, self.source_path, self.plan)
        self.assertEqual(result["status"], "failed_pixel_pair")
        self.assertFalse(result["checks"]["pii_region_changed"])
        self.assertNotIn("SYNTHETIC PII REGION", str(result))

    def test_regions_must_be_disjoint_and_in_bounds(self):
        with self.assertRaisesRegex(EvaluationError, "roi_overlap"):
            evaluate_pixel_pair(self.source_path, self.redacted_path,
                                PixelPlan(self.pii, self.pii, self.sentinel))
        with self.assertRaisesRegex(EvaluationError, "roi_out_of_bounds"):
            evaluate_pixel_pair(self.source_path, self.redacted_path,
                                PixelPlan(Box(-1, 0, 4, 4), self.ordinary, self.sentinel))

    def test_unreadable_image_fails_closed(self):
        missing = self.root / "missing.jpg"
        with self.assertRaisesRegex(EvaluationError, "image_unreadable"):
            evaluate_pixel_pair(missing, self.redacted_path, self.plan)

    def test_fail_closed_pair_requires_source_free_replacement(self):
        result = evaluate_fail_closed_pair(self.source_path, self.failure_path, self.pii)
        self.assertEqual(result["status"], "passed_fail_closed_pair")
        self.assertTrue(result["checks"]["whole_frame_replaced"])
        self.assertTrue(result["checks"]["pii_region_not_source_like"])

    def test_run_gate_requires_positive_control_auth_fail_closed_and_cleanup(self):
        pixel = evaluate_pixel_pair(self.source_path, self.redacted_path, self.plan)
        failure = evaluate_fail_closed_pair(self.source_path, self.failure_path, self.pii)
        result = evaluate_run(
            pixel, failure,
            ordinary_positive_control_count=1,
            bearer_statuses={"missing": 403, "wrong": 403, "valid": 200},
            cleanup={"recorder_stopped": True, "fixture_closed": True,
                     "listener_closed": True, "owned_processes_quiescent": True},
            failure_injection_verified=True,
        )
        self.assertEqual(result["status"], "passed_synthetic_pixel_check")
        self.assertTrue(all(result["checks"].values()))
        self.assertEqual(result["metrics"]["cleanup_items_passed"], 4)

    def test_run_gate_rejects_missing_controls_or_unknown_failure_path(self):
        pixel = evaluate_pixel_pair(self.source_path, self.redacted_path, self.plan)
        failure = evaluate_fail_closed_pair(self.source_path, self.failure_path, self.pii)
        with self.assertRaisesRegex(EvaluationError, "bearer_auth_matrix_incomplete"):
            evaluate_run(pixel, failure, ordinary_positive_control_count=0,
                         bearer_statuses={"missing": 403, "wrong": 401, "valid": 200},
                         cleanup={"recorder_stopped": True, "fixture_closed": True,
                                  "listener_closed": True, "owned_processes_quiescent": True},
                         failure_injection_verified=False)

        result = evaluate_run(
            pixel, failure, ordinary_positive_control_count=0,
            bearer_statuses={"missing": 403, "wrong": 403, "valid": 200},
            cleanup={"recorder_stopped": True, "fixture_closed": True,
                     "listener_closed": True, "owned_processes_quiescent": True},
            failure_injection_verified=False,
        )
        self.assertEqual(result["status"], "failed_synthetic_pixel_check")
        self.assertFalse(result["checks"]["ordinary_positive_control"])
        self.assertFalse(result["checks"]["failure_injection_verified"])

    def test_run_gate_rejects_incomplete_cleanup(self):
        pixel = evaluate_pixel_pair(self.source_path, self.redacted_path, self.plan)
        failure = evaluate_fail_closed_pair(self.source_path, self.failure_path, self.pii)
        with self.assertRaisesRegex(EvaluationError, "cleanup_incomplete"):
            evaluate_run(pixel, failure, ordinary_positive_control_count=1,
                         bearer_statuses={"missing": 403, "wrong": 403, "valid": 200},
                         cleanup={"recorder_stopped": True, "fixture_closed": True,
                                  "listener_closed": True, "owned_processes_quiescent": False},
                         failure_injection_verified=True)


if __name__ == "__main__":
    unittest.main()
