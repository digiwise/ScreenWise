"""Pure pixel and run-evidence evaluator for a fixed synthetic PII fixture.

This module does not capture screens, call the recorder API, launch processes,
or inspect arbitrary session data. A later gated controller may pass two
images from its own fixed synthetic fixture and content-free protocol facts.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from PIL import Image, ImageStat


class EvaluationError(ValueError):
    """The supplied evidence is incomplete or cannot be safely evaluated."""


@dataclass(frozen=True)
class Box:
    left: int
    top: int
    right: int
    bottom: int

    def validate(self, size: tuple[int, int]) -> None:
        width, height = size
        if not (0 <= self.left < self.right <= width and
                0 <= self.top < self.bottom <= height):
            raise EvaluationError("roi_out_of_bounds")


@dataclass(frozen=True)
class PixelPlan:
    pii: Box
    ordinary: Box
    sentinel: Box
    channel_tolerance: int = 18
    max_changed_fraction: float = 0.12
    min_changed_fraction: float = 0.90
    min_source_contrast: float = 60.0
    max_retained_contrast_fraction: float = 0.35

    def validate(self, size: tuple[int, int]) -> None:
        self.pii.validate(size)
        self.ordinary.validate(size)
        self.sentinel.validate(size)
        regions = (self.pii, self.ordinary, self.sentinel)
        for i, first in enumerate(regions):
            for second in regions[i + 1:]:
                if (first.left < second.right and second.left < first.right and
                        first.top < second.bottom and second.top < first.bottom):
                    raise EvaluationError("roi_overlap")
        if not 0 <= self.channel_tolerance <= 255:
            raise EvaluationError("invalid_channel_tolerance")
        if not (0.0 < self.max_changed_fraction < self.min_changed_fraction <= 1.0):
            raise EvaluationError("invalid_change_thresholds")
        if not 0.0 < self.min_source_contrast <= 127.5:
            raise EvaluationError("invalid_source_contrast")
        if not 0.0 < self.max_retained_contrast_fraction < 1.0:
            raise EvaluationError("invalid_contrast_ratio")


def _load_rgb(path: str | Path) -> Image.Image:
    try:
        with Image.open(path) as image:
            image.load()
            return image.convert("RGB")
    except (OSError, ValueError) as error:
        raise EvaluationError("image_unreadable") from error


def _diff_metrics(before: Image.Image, after: Image.Image, box: Box,
                  tolerance: int) -> tuple[float, float]:
    a = before.crop((box.left, box.top, box.right, box.bottom))
    b = after.crop((box.left, box.top, box.right, box.bottom))
    diff = Image.frombytes("RGB", a.size, bytes(
        abs(x - y) for x, y in zip(a.tobytes(), b.tobytes())
    ))
    pixels = a.width * a.height
    raw_a, raw_b = a.tobytes(), b.tobytes()
    changed = 0
    for offset in range(0, len(raw_a), 3):
        if max(abs(raw_a[offset + channel] - raw_b[offset + channel])
               for channel in range(3)) > tolerance:
            changed += 1
    return changed / pixels, sum(ImageStat.Stat(diff).mean) / 3


def _luma_contrast(image: Image.Image, box: Box) -> float:
    region = image.crop((box.left, box.top, box.right, box.bottom)).convert("L")
    return float(ImageStat.Stat(region).stddev[0])


def evaluate_pixel_pair(before_path: str | Path, after_path: str | Path,
                        plan: PixelPlan) -> dict[str, Any]:
    """Compare a fixed-fixture baseline and stored frame; return safe metrics only."""
    before = _load_rgb(before_path)
    after = _load_rgb(after_path)
    if before.size != after.size:
        raise EvaluationError("image_dimensions_changed")
    plan.validate(before.size)

    # The production route uses strong Gaussian blur, not a solid overlay. A
    # fixed high-contrast source makes loss of local detail measurable without
    # retaining or reporting any source pixels.
    source_contrast = _luma_contrast(before, plan.pii)
    if source_contrast < plan.min_source_contrast:
        raise EvaluationError("pii_source_region_not_high_contrast")

    pii_changed, pii_mean_delta = _diff_metrics(
        before, after, plan.pii, plan.channel_tolerance)
    redacted_contrast = _luma_contrast(after, plan.pii)
    retained_contrast = redacted_contrast / max(source_contrast, 0.001)
    ordinary_changed, _ = _diff_metrics(
        before, after, plan.ordinary, plan.channel_tolerance)
    sentinel_changed, _ = _diff_metrics(
        before, after, plan.sentinel, plan.channel_tolerance)

    checks = {
        "pii_region_changed": pii_changed >= plan.min_changed_fraction,
        "pii_region_obscured": retained_contrast <= plan.max_retained_contrast_fraction,
        "ordinary_region_preserved": ordinary_changed <= plan.max_changed_fraction,
        "sentinel_region_preserved": sentinel_changed <= plan.max_changed_fraction,
    }
    return {
        "schema": "screenwise.pixel-redaction-evaluation.v1",
        "status": "passed_pixel_pair" if all(checks.values()) else "failed_pixel_pair",
        "checks": checks,
        "metrics": {
            "pii_changed_fraction": round(pii_changed, 4),
            "pii_source_contrast": round(source_contrast, 2),
            "pii_redacted_contrast": round(redacted_contrast, 2),
            "pii_retained_contrast_fraction": round(retained_contrast, 4),
            "pii_mean_channel_delta": round(pii_mean_delta, 2),
            "ordinary_changed_fraction": round(ordinary_changed, 4),
            "sentinel_changed_fraction": round(sentinel_changed, 4),
        },
        "limits": "Fixed synthetic regions only; no detector or OCR accuracy claim.",
    }


def evaluate_fail_closed_pair(source_path: str | Path, failure_path: str | Path,
                              pii: Box, *, min_changed_fraction: float = 0.98,
                              min_pii_changed_fraction: float = 0.98) -> dict[str, Any]:
    """Check an explicitly induced failure result is a source-free replacement.

    The caller must separately attest that this was the designated failure
    injection case. This image comparison alone cannot prove why the output changed.
    """
    source = _load_rgb(source_path)
    failure = _load_rgb(failure_path)
    if source.size != failure.size:
        raise EvaluationError("image_dimensions_changed")
    pii.validate(source.size)
    whole = Box(0, 0, source.width, source.height)
    changed, _ = _diff_metrics(source, failure, whole, tolerance=18)
    pii_changed, _ = _diff_metrics(source, failure, pii, tolerance=18)
    passed = changed >= min_changed_fraction and pii_changed >= min_pii_changed_fraction
    return {
        "schema": "screenwise.pixel-fail-closed-evaluation.v1",
        "status": "passed_fail_closed_pair" if passed else "failed_fail_closed_pair",
        "checks": {
            "whole_frame_replaced": changed >= min_changed_fraction,
            "pii_region_not_source_like": pii_changed >= min_pii_changed_fraction,
        },
        "metrics": {
            "whole_frame_changed_fraction": round(changed, 4),
            "pii_changed_fraction": round(pii_changed, 4),
        },
        "limits": "Requires separately verified fixed failure injection; no system-wide claim.",
    }


def evaluate_run(pixel_result: dict[str, Any], failure_result: dict[str, Any],
                 *, ordinary_positive_control_count: int,
                 bearer_statuses: dict[str, int], cleanup: dict[str, bool],
                 failure_injection_verified: bool) -> dict[str, Any]:
    """Apply a strict final evidence gate and emit only booleans and fixed metrics."""
    required_auth = {"missing": 403, "wrong": 403, "valid": 200}
    if type(ordinary_positive_control_count) is not int:
        raise EvaluationError("ordinary_positive_control_invalid")
    if not isinstance(bearer_statuses, dict) or any(
            type(bearer_statuses.get(key)) is not int or bearer_statuses[key] != value
            for key, value in required_auth.items()):
        raise EvaluationError("bearer_auth_matrix_incomplete")
    required_cleanup = ("recorder_stopped", "fixture_closed", "listener_closed",
                       "owned_processes_quiescent")
    if not isinstance(cleanup, dict) or any(cleanup.get(key) is not True for key in required_cleanup):
        raise EvaluationError("cleanup_incomplete")

    checks = {
        "ordinary_positive_control": ordinary_positive_control_count > 0,
        "pixel_pair_passed": pixel_result.get("status") == "passed_pixel_pair",
        "failure_injection_verified": failure_injection_verified is True,
        "fail_closed_pair_passed": failure_result.get("status") == "passed_fail_closed_pair",
        "bearer_auth_passed": True,
        "cleanup_passed": True,
    }
    return {
        "schema": "screenwise.pixel-redaction-run-evaluation.v1",
        "status": "passed_synthetic_pixel_check" if all(checks.values()) else "failed_synthetic_pixel_check",
        "checks": checks,
        "metrics": {
            "ordinary_positive_control_count": ordinary_positive_control_count,
            "missing_token_status": bearer_statuses["missing"],
            "wrong_token_status": bearer_statuses["wrong"],
            "valid_token_status": bearer_statuses["valid"],
            "cleanup_items_passed": sum(cleanup[key] is True for key in required_cleanup),
            "cleanup_items_required": len(required_cleanup),
        },
        "limits": "Fixed synthetic fixture only; not OCR, detector, or general privacy accuracy.",
    }
