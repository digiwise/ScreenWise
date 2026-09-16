import copy
import importlib.util
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    "lock_acceptance", HERE / "lock_acceptance.py"
)
lock_acceptance = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(lock_acceptance)


def passing_evidence():
    pid = 4242
    creation = 133_999_000_000_000_000
    return {
        "schema": lock_acceptance.EVIDENCE_SCHEMA,
        "owner_readiness": {
            "fresh": True,
            "waited_indefinitely": True,
            "phase_id": "lock-transition-01",
        },
        "clock": {
            "run_start_utc": "2026-09-16T01:00:00Z",
            "run_start_monotonic_ms": 1_000,
            "notice_mapping": lock_acceptance.NOTICE_MAPPING,
            "verified": True,
        },
        "process": {
            "pid": pid,
            "creation_time": creation,
            "scope_count": 1,
            "observations": [
                {
                    "checkpoint": "pre_lock",
                    "pid": pid,
                    "creation_time": creation,
                    "alive": True,
                    "observed_ms": 3_500,
                },
                {
                    "checkpoint": "locked_stable_end",
                    "pid": pid,
                    "creation_time": creation,
                    "alive": True,
                    "observed_ms": 17_500,
                },
                {
                    "checkpoint": "post_unlock",
                    "pid": pid,
                    "creation_time": creation,
                    "alive": True,
                    "observed_ms": 21_000,
                },
            ],
            "shutdown": {
                "observed": True,
                "exit_code": 0,
                "forced": False,
                "issues": [],
            },
        },
        "controls": {
            "before": {
                "marker_id": lock_acceptance.MARKER_ID,
                "generation": 1,
                "frame_hits": 1,
                "uia_hits": 1,
                "authenticated_search_hits": 1,
                "observed_ms": 3_000,
            },
            "after": {
                "marker_id": lock_acceptance.MARKER_ID,
                "generation": 2,
                "frame_hits": 1,
                "uia_hits": 1,
                "authenticated_search_hits": 1,
                "observed_ms": 20_000,
            },
        },
        "wts_samples": [
            {
                "sample_id": "w1",
                "observed_ms": 3_600,
                "state": "unlocked",
                "source": lock_acceptance.WTS_SOURCE,
                "real": True,
                "independent": True,
                "error_code": None,
            },
            {
                "sample_id": "w2",
                "observed_ms": 4_000,
                "state": "locked",
                "source": lock_acceptance.WTS_SOURCE,
                "real": True,
                "independent": True,
                "error_code": None,
            },
            {
                "sample_id": "w3",
                "observed_ms": 6_000,
                "state": "locked",
                "source": lock_acceptance.WTS_SOURCE,
                "real": True,
                "independent": True,
                "error_code": None,
            },
            {
                "sample_id": "w4",
                "observed_ms": 16_000,
                "state": "locked",
                "source": lock_acceptance.WTS_SOURCE,
                "real": True,
                "independent": True,
                "error_code": None,
            },
            {
                "sample_id": "w5",
                "observed_ms": 18_000,
                "state": "unlocked",
                "source": lock_acceptance.WTS_SOURCE,
                "real": True,
                "independent": True,
                "error_code": None,
            },
        ],
        "locked_plateau": {
            "grace_complete": True,
            "start_ms": 6_000,
            "end_ms": 16_000,
            "start_frame_rows": 12,
            "end_frame_rows": 12,
            "start_uia_capture_rows": 8,
            "end_uia_capture_rows": 8,
            "start_privacy_notice_rows": 1,
            "end_privacy_notice_rows": 2,
        },
        "notices": [
            {
                "typed": True,
                "reason_code": "wts_session_unlocked",
                "message": lock_acceptance.UNLOCKED_MESSAGE,
                "timestamp_ms": 2_000,
            },
            {
                "typed": True,
                "reason_code": "wts_session_locked",
                "message": lock_acceptance.LOCKED_MESSAGE,
                "timestamp_ms": 4_500,
            },
            {
                "typed": True,
                "reason_code": "wts_session_unlocked",
                "message": lock_acceptance.UNLOCKED_MESSAGE,
                "timestamp_ms": 18_500,
            },
        ],
        "logs": {
            "safe_notice_lines_verified": True,
            "reason_code_counts": {
                "wts_session_locked": 1,
                "wts_session_unlocked": 2,
            },
        },
        "api_checks": [
            {
                "checkpoint": "before",
                "observed_ms": 2_500,
                "bind_host": "127.0.0.1",
                "port": 31479,
                "endpoint": "/search",
                "content_type": "all",
                "missing": 403,
                "wrong": 403,
                "valid": 200,
            },
            {
                "checkpoint": "after",
                "observed_ms": 19_000,
                "bind_host": "127.0.0.1",
                "port": 31479,
                "endpoint": "/search",
                "content_type": "all",
                "missing": 403,
                "wrong": 403,
                "valid": 200,
            },
        ],
        "audio": {
            "enabled": False,
            "tested": False,
            "running_device_count": 0,
            "audio_chunk_rows": 0,
            "audio_transcription_rows": 0,
        },
        "cleanup": {"processes_stopped": True, "fixture_closed": True},
    }


class LockAcceptanceTests(unittest.TestCase):
    def evaluate(self, mutate=None):
        evidence = passing_evidence()
        if mutate is not None:
            mutate(evidence)
        return lock_acceptance.evaluate_lock_acceptance(evidence)

    def test_complete_aggregate_contract_passes_with_explicit_scope_limits(self):
        result = self.evaluate()
        self.assertEqual(result["status"], "pass")
        self.assertTrue(result["eligible"])
        self.assertEqual(result["reasons"], [])
        self.assertIn("does not prove", result["scope_note"])
        self.assertIn("remains untested", result["audio_scope"])

    def test_wrong_wts_order_is_incomplete(self):
        def mutate(evidence):
            evidence["wts_samples"][0]["state"] = "locked"

        result = self.evaluate(mutate)
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("wts_transition_order_invalid", result["reasons"])

    def test_synthetic_dependent_or_failed_wts_is_invalid(self):
        for field, value in (
            ("real", False),
            ("independent", False),
            ("source", "app_health"),
            ("error_code", "query_failed"),
        ):
            with self.subTest(field=field):
                result = self.evaluate(
                    lambda evidence, field=field, value=value: evidence[
                        "wts_samples"
                    ][1].__setitem__(field, value)
                )
                self.assertEqual(result["status"], "incomplete")
                self.assertIn("invalid_wts_evidence", result["reasons"])

    def test_bounded_secure_desktop_unknown_is_preserved_but_not_counted_as_locked(self):
        def mutate(evidence):
            evidence["wts_samples"].insert(
                1,
                {
                    "sample_id": "w-transition",
                    "observed_ms": 3_800,
                    "state": "unknown",
                    "source": lock_acceptance.WTS_SOURCE,
                    "real": True,
                    "independent": True,
                    "error_code": "secure_desktop_unavailable",
                },
            )

        result = self.evaluate(mutate)
        self.assertEqual(result["status"], "pass", result)
        self.assertEqual(
            result["wts_unknown_reason_counts"],
            {"secure_desktop_unavailable": 1},
        )

    def test_unknown_reason_or_unbracketed_unknown_is_incomplete(self):
        def insert(evidence, observed_ms=3_800, error="query_failed"):
            evidence["wts_samples"].insert(
                1,
                {
                    "sample_id": "w-transition",
                    "observed_ms": observed_ms,
                    "state": "unknown",
                    "source": lock_acceptance.WTS_SOURCE,
                    "real": True,
                    "independent": True,
                    "error_code": error,
                },
            )

        result = self.evaluate(insert)
        self.assertIn("wts_unknown_reason_invalid", result["reasons"])
        result = self.evaluate(
            lambda evidence: insert(
                evidence, observed_ms=5_000, error="secure_desktop_unavailable"
            )
        )
        self.assertIn("wts_unknown_outside_transition", result["reasons"])

    def test_unknown_transition_must_be_bounded_and_cannot_substitute_for_locked(self):
        def long_transition(evidence):
            evidence["wts_samples"][0]["observed_ms"] = 1_500
            evidence["wts_samples"].insert(
                1,
                {
                    "sample_id": "w-transition",
                    "observed_ms": 2_000,
                    "state": "unknown",
                    "source": lock_acceptance.WTS_SOURCE,
                    "real": True,
                    "independent": True,
                    "error_code": "secure_desktop_unavailable",
                },
            )

        result = self.evaluate(long_transition)
        self.assertIn("wts_unknown_transition_unbounded", result["reasons"])

        def no_confirmed_lock(evidence):
            for sample in evidence["wts_samples"]:
                if sample["state"] == "locked":
                    sample["state"] = "unknown"
                    sample["error_code"] = "secure_desktop_unavailable"

        result = self.evaluate(no_confirmed_lock)
        self.assertIn("wts_transition_order_invalid", result["reasons"])
        self.assertIn("locked_stability_unconfirmed", result["reasons"])

    def test_short_or_unsettled_locked_window_is_incomplete(self):
        def mutate(evidence):
            evidence["wts_samples"][3]["observed_ms"] = 15_000
            evidence["locked_plateau"]["end_ms"] = 15_000

        result = self.evaluate(mutate)
        self.assertIn("locked_stability_unconfirmed", result["reasons"])
        self.assertIn("locked_plateau_window_invalid", result["reasons"])

    def test_missing_before_or_after_controls_is_incomplete(self):
        for name in ("before", "after"):
            with self.subTest(name=name):
                result = self.evaluate(
                    lambda evidence, name=name: evidence["controls"].pop(name)
                )
                self.assertEqual(result["status"], "incomplete")
                self.assertIn("missing_positive_controls", result["reasons"])

    def test_partial_two_observation_evidence_is_incomplete_without_throwing(self):
        def mutate(evidence):
            evidence["process"]["observations"].pop()
            evidence["controls"].pop("after")
            evidence["api_checks"].pop()

        result = self.evaluate(mutate)
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("process_liveness_unconfirmed", result["reasons"])
        self.assertIn("missing_positive_controls", result["reasons"])

    def test_malformed_notice_timestamp_is_incomplete_without_throwing(self):
        result = self.evaluate(
            lambda evidence: evidence["notices"][1].__setitem__(
                "timestamp_ms", None
            )
        )
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("typed_notices_invalid", result["reasons"])

    def test_reused_control_generation_is_incomplete(self):
        result = self.evaluate(
            lambda evidence: evidence["controls"]["after"].__setitem__(
                "generation", 1
            )
        )
        self.assertIn("control_generation_not_distinct", result["reasons"])

    def test_locked_frame_or_uia_writes_fail(self):
        for field in ("end_frame_rows", "end_uia_capture_rows"):
            with self.subTest(field=field):
                result = self.evaluate(
                    lambda evidence, field=field: evidence[
                        "locked_plateau"
                    ].__setitem__(field, evidence["locked_plateau"][field] + 1)
                )
                self.assertEqual(result["status"], "fail")
                expected = (
                    "locked_frame_writes"
                    if field == "end_frame_rows"
                    else "locked_uia_writes"
                )
                self.assertIn(expected, result["reasons"])

    def test_privacy_notice_growth_is_excluded_from_capture_plateau(self):
        result = self.evaluate(
            lambda evidence: evidence["locked_plateau"].__setitem__(
                "end_privacy_notice_rows", 50
            )
        )
        self.assertEqual(result["status"], "pass")

    def test_absent_or_nonfixed_notice_is_incomplete(self):
        absent = self.evaluate(lambda evidence: evidence.__setitem__("notices", []))
        self.assertIn("typed_notice_sequence_missing", absent["reasons"])
        wrong_message = self.evaluate(
            lambda evidence: evidence["notices"][1].__setitem__(
                "message", "private or variable text"
            )
        )
        self.assertIn("typed_notices_invalid", wrong_message["reasons"])

    def test_initial_unlocked_notice_is_optional(self):
        def mutate(evidence):
            evidence["notices"].pop(0)
            evidence["logs"]["reason_code_counts"]["wts_session_unlocked"] = 1

        self.assertEqual(self.evaluate(mutate)["status"], "pass")

    def test_fixed_transient_notice_is_reported_and_transition_brackets_allow_early_notice(self):
        def mutate(evidence):
            evidence["notices"][1]["timestamp_ms"] = 3_900
            evidence["notices"].insert(
                2,
                {
                    "typed": True,
                    "reason_code": "input_desktop_unavailable",
                    "message": lock_acceptance.DESKTOP_MESSAGE,
                    "timestamp_ms": 5_000,
                },
            )
            evidence["logs"]["reason_code_counts"][
                "input_desktop_unavailable"
            ] = 1

        result = self.evaluate(mutate)
        self.assertEqual(result["status"], "pass")
        self.assertEqual(
            result["additional_notice_reason_counts"],
            {"input_desktop_unavailable": 1},
        )

    def test_unknown_notice_message_or_reason_is_rejected(self):
        def mutate(evidence):
            evidence["notices"][1]["reason_code"] = "unknown_reason"
            evidence["logs"]["reason_code_counts"] = {
                "wts_session_unlocked": 2,
                "unknown_reason": 1,
            }

        result = self.evaluate(mutate)
        self.assertIn("typed_notices_invalid", result["reasons"])
        self.assertIn("typed_notice_sequence_missing", result["reasons"])

    def test_unconfirmed_or_forced_shutdown_is_incomplete(self):
        for field, value in (("observed", False), ("exit_code", None), ("forced", True)):
            with self.subTest(field=field):
                result = self.evaluate(
                    lambda evidence, field=field, value=value: evidence["process"][
                        "shutdown"
                    ].__setitem__(field, value)
                )
                self.assertIn("shutdown_unconfirmed", result["reasons"])

    def test_boolean_is_not_accepted_as_count_or_exit_code(self):
        result = self.evaluate(
            lambda evidence: evidence["process"]["shutdown"].__setitem__(
                "exit_code", False
            )
        )
        self.assertIn("shutdown_unconfirmed", result["reasons"])
        result = self.evaluate(
            lambda evidence: evidence["process"].__setitem__("scope_count", True)
        )
        self.assertIn("process_scope_invalid", result["reasons"])

    def test_same_pid_with_reused_creation_identity_is_incomplete(self):
        def mutate(evidence):
            evidence["process"]["observations"][2]["creation_time"] += 10

        result = self.evaluate(mutate)
        self.assertEqual(result["status"], "incomplete")
        self.assertIn("process_identity_reused_or_changed", result["reasons"])

    def test_api_auth_must_pass_before_and_after_on_exact_loopback_bind(self):
        for mutate in (
            lambda evidence: evidence["api_checks"].pop(),
            lambda evidence: evidence["api_checks"][1].__setitem__(
                "bind_host", "localhost"
            ),
            lambda evidence: evidence["api_checks"][1].__setitem__("wrong", 200),
            lambda evidence: evidence["api_checks"][1].__setitem__("port", 31480),
        ):
            with self.subTest(mutate=mutate):
                result = self.evaluate(mutate)
                self.assertIn("api_auth_or_bind_invalid", result["reasons"])

    def test_readiness_cleanup_audio_and_safe_log_evidence_are_mandatory(self):
        cases = (
            lambda evidence: evidence["owner_readiness"].__setitem__(
                "fresh", False
            ),
            lambda evidence: evidence["cleanup"].__setitem__(
                "fixture_closed", False
            ),
            lambda evidence: evidence["audio"].__setitem__("tested", True),
            lambda evidence: evidence["logs"].__setitem__(
                "safe_notice_lines_verified", False
            ),
        )
        expected = (
            "owner_readiness_unconfirmed",
            "cleanup_unconfirmed",
            "audio_scope_invalid",
            "safe_log_codes_unconfirmed",
        )
        for mutate, reason in zip(cases, expected):
            with self.subTest(reason=reason):
                self.assertIn(reason, self.evaluate(mutate)["reasons"])

    def test_silent_scope_requires_zero_live_devices_and_audio_rows(self):
        for field in (
            "running_device_count",
            "audio_chunk_rows",
            "audio_transcription_rows",
        ):
            with self.subTest(field=field):
                result = self.evaluate(
                    lambda evidence, field=field: evidence["audio"].__setitem__(
                        field, 1
                    )
                )
                self.assertEqual(result["status"], "incomplete")
                self.assertIn("audio_scope_invalid", result["reasons"])

    def test_payload_bearing_evidence_fails_without_echoing_value(self):
        secret = "do-not-echo-private-value"

        def mutate(evidence):
            evidence["controls"]["before"]["ocr_text"] = secret

        result = self.evaluate(mutate)
        self.assertEqual(result["status"], "fail")
        self.assertIn("captured_payload_present", result["reasons"])
        self.assertNotIn(secret, repr(result))

    def test_evaluator_does_not_mutate_evidence(self):
        evidence = passing_evidence()
        original = copy.deepcopy(evidence)
        lock_acceptance.evaluate_lock_acceptance(evidence)
        self.assertEqual(evidence, original)


if __name__ == "__main__":
    unittest.main()
