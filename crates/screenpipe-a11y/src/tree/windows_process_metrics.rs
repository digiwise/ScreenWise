//! Bounded, aggregate UIA attempt metrics by freshly verified executable basename.
//! The caller must verify the native process identity for this attempt. This
//! module validates the shape of that basename but cannot establish provenance.

use super::timing::{self, AttemptTiming};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const MAX_PROCESSES: usize = 32;
const MAX_BASENAME_BYTES: usize = 128;
const MODE_NAMES: [&str; 2] = ["privacy_and_text", "privacy_only"];
// `unknown` must stay last: the lookup below searches every entry except the
// final one and falls back to it. A new code therefore goes immediately before
// it, which shifts only `unknown`'s index. Consumers read `reason_codes`
// alongside `reason_counts`, so the vector stays self-describing.
const REASON_CODES: [&str; 11] = [
    "uia_walk_timeout",
    "uia_node_limit",
    "uia_depth_limit",
    "uia_property_text_limit",
    "uia_aggregate_text_limit",
    "uia_provider_failure",
    "uia_backoff",
    "uia_initialization_failed",
    "uia_monitor_unverified",
    "uia_walk_disabled",
    "unknown",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttemptOutcome {
    Completed,
    Failed,
    Denied,
}

#[derive(Clone, Debug)]
struct Counts {
    attempts: u64,
    completed: u64,
    failed: u64,
    denied: u64,
    total_duration_us: u64,
    max_duration_us: u64,
    reason_counts: [u64; REASON_CODES.len()],
    stage_calls: [u64; timing::STAGE_COUNT],
    stage_total_us: [u64; timing::STAGE_COUNT],
    stage_max_us: [u64; timing::STAGE_COUNT],
    attempts_with_timing: u64,
    mode_attempt_counts: [u64; MODE_NAMES.len()],
    failed_stage_calls: [u64; timing::STAGE_COUNT],
    failed_stage_total_us: [u64; timing::STAGE_COUNT],
    failed_stage_max_us: [u64; timing::STAGE_COUNT],
    completed_stage_calls: [u64; timing::STAGE_COUNT],
    completed_stage_total_us: [u64; timing::STAGE_COUNT],
    completed_stage_max_us: [u64; timing::STAGE_COUNT],
    retry_cause_counts: [u64; timing::RETRY_CAUSE_COUNT],
    failed_retry_cause_counts: [u64; timing::RETRY_CAUSE_COUNT],
    native_timeout_hresult_counts: [u64; timing::NATIVE_TIMEOUT_HRESULT_CODES.len()],
    failed_native_timeout_hresult_counts: [u64; timing::NATIVE_TIMEOUT_HRESULT_CODES.len()],
    other_hresult_count: u64,
    failed_other_hresult_count: u64,
    failures_with_budget: u64,
    failure_phase_counts: [u64; timing::PHASE_COUNT],
    failure_phase_remaining_total_us: [u64; timing::PHASE_COUNT],
    failure_phase_remaining_min_us: [u64; timing::PHASE_COUNT],
    failure_phase_remaining_max_us: [u64; timing::PHASE_COUNT],
    failure_phase_expired_counts: [u64; timing::PHASE_COUNT],
    failure_overall_remaining_total_us: u64,
    failure_overall_remaining_min_us: u64,
    failure_overall_remaining_max_us: u64,
    failure_overall_expired_count: u64,
}

impl Default for Counts {
    fn default() -> Self {
        Self {
            attempts: 0,
            completed: 0,
            failed: 0,
            denied: 0,
            total_duration_us: 0,
            max_duration_us: 0,
            reason_counts: [0; REASON_CODES.len()],
            stage_calls: [0; timing::STAGE_COUNT],
            stage_total_us: [0; timing::STAGE_COUNT],
            stage_max_us: [0; timing::STAGE_COUNT],
            attempts_with_timing: 0,
            mode_attempt_counts: [0; MODE_NAMES.len()],
            failed_stage_calls: [0; timing::STAGE_COUNT],
            failed_stage_total_us: [0; timing::STAGE_COUNT],
            failed_stage_max_us: [0; timing::STAGE_COUNT],
            completed_stage_calls: [0; timing::STAGE_COUNT],
            completed_stage_total_us: [0; timing::STAGE_COUNT],
            completed_stage_max_us: [0; timing::STAGE_COUNT],
            retry_cause_counts: [0; timing::RETRY_CAUSE_COUNT],
            failed_retry_cause_counts: [0; timing::RETRY_CAUSE_COUNT],
            native_timeout_hresult_counts: [0; timing::NATIVE_TIMEOUT_HRESULT_CODES.len()],
            failed_native_timeout_hresult_counts: [0; timing::NATIVE_TIMEOUT_HRESULT_CODES.len()],
            other_hresult_count: 0,
            failed_other_hresult_count: 0,
            failures_with_budget: 0,
            failure_phase_counts: [0; timing::PHASE_COUNT],
            failure_phase_remaining_total_us: [0; timing::PHASE_COUNT],
            failure_phase_remaining_min_us: [u64::MAX; timing::PHASE_COUNT],
            failure_phase_remaining_max_us: [0; timing::PHASE_COUNT],
            failure_phase_expired_counts: [0; timing::PHASE_COUNT],
            failure_overall_remaining_total_us: 0,
            failure_overall_remaining_min_us: u64::MAX,
            failure_overall_remaining_max_us: 0,
            failure_overall_expired_count: 0,
        }
    }
}

impl Counts {
    fn record(
        &mut self,
        duration: Duration,
        privacy_only: bool,
        outcome: AttemptOutcome,
        reason_code: Option<&str>,
        timing: Option<AttemptTiming>,
    ) {
        self.attempts = self.attempts.saturating_add(1);
        self.mode_attempt_counts[usize::from(privacy_only)] =
            self.mode_attempt_counts[usize::from(privacy_only)].saturating_add(1);
        match outcome {
            AttemptOutcome::Completed => self.completed = self.completed.saturating_add(1),
            AttemptOutcome::Failed => {
                self.failed = self.failed.saturating_add(1);
                let index = reason_code
                    .and_then(|code| {
                        REASON_CODES[..REASON_CODES.len() - 1]
                            .iter()
                            .position(|known| *known == code)
                    })
                    .unwrap_or(REASON_CODES.len() - 1);
                self.reason_counts[index] = self.reason_counts[index].saturating_add(1);
            }
            AttemptOutcome::Denied => self.denied = self.denied.saturating_add(1),
        }
        let duration_us = duration.as_micros().min(u64::MAX as u128) as u64;
        self.total_duration_us = self.total_duration_us.saturating_add(duration_us);
        self.max_duration_us = self.max_duration_us.max(duration_us);
        if let Some(timing) = timing {
            self.attempts_with_timing = self.attempts_with_timing.saturating_add(1);
            for index in 0..timing::STAGE_COUNT {
                self.stage_calls[index] =
                    self.stage_calls[index].saturating_add(timing.stage_calls[index]);
                self.stage_total_us[index] =
                    self.stage_total_us[index].saturating_add(timing.stage_us[index]);
                self.stage_max_us[index] = self.stage_max_us[index].max(timing.stage_us[index]);
                let (calls, total, max) = match outcome {
                    AttemptOutcome::Failed => (
                        &mut self.failed_stage_calls,
                        &mut self.failed_stage_total_us,
                        &mut self.failed_stage_max_us,
                    ),
                    AttemptOutcome::Completed => (
                        &mut self.completed_stage_calls,
                        &mut self.completed_stage_total_us,
                        &mut self.completed_stage_max_us,
                    ),
                    AttemptOutcome::Denied => continue,
                };
                calls[index] = calls[index].saturating_add(timing.stage_calls[index]);
                total[index] = total[index].saturating_add(timing.stage_us[index]);
                max[index] = max[index].max(timing.stage_us[index]);
            }
            for index in 0..timing::RETRY_CAUSE_COUNT {
                self.retry_cause_counts[index] =
                    self.retry_cause_counts[index].saturating_add(timing.retry_causes[index]);
                if outcome == AttemptOutcome::Failed {
                    self.failed_retry_cause_counts[index] = self.failed_retry_cause_counts[index]
                        .saturating_add(timing.retry_causes[index]);
                }
            }
            for index in 0..timing::NATIVE_TIMEOUT_HRESULT_CODES.len() {
                self.native_timeout_hresult_counts[index] = self.native_timeout_hresult_counts
                    [index]
                    .saturating_add(timing.native_timeout_hresults[index]);
                if outcome == AttemptOutcome::Failed {
                    self.failed_native_timeout_hresult_counts[index] = self
                        .failed_native_timeout_hresult_counts[index]
                        .saturating_add(timing.native_timeout_hresults[index]);
                }
            }
            self.other_hresult_count = self
                .other_hresult_count
                .saturating_add(timing.other_hresults);
            if outcome == AttemptOutcome::Failed {
                self.failed_other_hresult_count = self
                    .failed_other_hresult_count
                    .saturating_add(timing.other_hresults);
                if let Some(budget) = timing.failure_budget {
                    let index = budget.phase as usize;
                    self.failures_with_budget = self.failures_with_budget.saturating_add(1);
                    self.failure_phase_counts[index] =
                        self.failure_phase_counts[index].saturating_add(1);
                    self.failure_phase_remaining_total_us[index] = self
                        .failure_phase_remaining_total_us[index]
                        .saturating_add(budget.phase_remaining_us);
                    self.failure_phase_remaining_min_us[index] =
                        self.failure_phase_remaining_min_us[index].min(budget.phase_remaining_us);
                    self.failure_phase_remaining_max_us[index] =
                        self.failure_phase_remaining_max_us[index].max(budget.phase_remaining_us);
                    self.failure_phase_expired_counts[index] = self.failure_phase_expired_counts
                        [index]
                        .saturating_add(u64::from(budget.phase_expired));
                    self.failure_overall_remaining_total_us = self
                        .failure_overall_remaining_total_us
                        .saturating_add(budget.overall_remaining_us);
                    self.failure_overall_remaining_min_us = self
                        .failure_overall_remaining_min_us
                        .min(budget.overall_remaining_us);
                    self.failure_overall_remaining_max_us = self
                        .failure_overall_remaining_max_us
                        .max(budget.overall_remaining_us);
                    self.failure_overall_expired_count = self
                        .failure_overall_expired_count
                        .saturating_add(u64::from(budget.overall_expired));
                }
            }
        }
    }

    fn snapshot(&self) -> Value {
        json!({
            "attempts": self.attempts,
            "completed": self.completed,
            "failed": self.failed,
            "denied": self.denied,
            "total_duration_us": self.total_duration_us,
            "max_duration_us": self.max_duration_us,
            "reason_counts": self.reason_counts,
            "attempts_with_timing": self.attempts_with_timing,
            "stage_calls": self.stage_calls,
            "stage_total_us": self.stage_total_us,
            "stage_max_us": self.stage_max_us,
            "mode_attempt_counts": self.mode_attempt_counts,
            "failed_stage_calls": self.failed_stage_calls,
            "failed_stage_total_us": self.failed_stage_total_us,
            "failed_stage_max_us": self.failed_stage_max_us,
            "completed_stage_calls": self.completed_stage_calls,
            "completed_stage_total_us": self.completed_stage_total_us,
            "completed_stage_max_us": self.completed_stage_max_us,
            "retry_cause_counts": self.retry_cause_counts,
            "failed_retry_cause_counts": self.failed_retry_cause_counts,
            "native_timeout_hresult_counts": self.native_timeout_hresult_counts,
            "failed_native_timeout_hresult_counts": self.failed_native_timeout_hresult_counts,
            "other_hresult_count": self.other_hresult_count,
            "failed_other_hresult_count": self.failed_other_hresult_count,
            "failures_with_budget": self.failures_with_budget,
            "failure_phase_counts": self.failure_phase_counts,
            "failure_phase_remaining_total_us": self.failure_phase_remaining_total_us,
            "failure_phase_remaining_min_us": self.failure_phase_remaining_min_us.map(|value| if value == u64::MAX { 0 } else { value }),
            "failure_phase_remaining_max_us": self.failure_phase_remaining_max_us,
            "failure_phase_expired_counts": self.failure_phase_expired_counts,
            "failure_overall_remaining_total_us": self.failure_overall_remaining_total_us,
            "failure_overall_remaining_min_us": if self.failure_overall_remaining_min_us == u64::MAX { 0 } else { self.failure_overall_remaining_min_us },
            "failure_overall_remaining_max_us": self.failure_overall_remaining_max_us,
            "failure_overall_expired_count": self.failure_overall_expired_count,
        })
    }
}

#[derive(Default)]
struct Registry {
    total: Counts,
    processes: BTreeMap<String, Counts>,
    unknown: Counts,
    overflow: Counts,
}

impl Registry {
    fn record(
        &mut self,
        verified_basename: Option<&str>,
        duration: Duration,
        privacy_only: bool,
        outcome: AttemptOutcome,
        reason_code: Option<&str>,
        timing: Option<AttemptTiming>,
    ) {
        self.total
            .record(duration, privacy_only, outcome, reason_code, timing);
        let target = match verified_basename.and_then(normalize_basename) {
            Some(name) if self.processes.contains_key(&name) => {
                self.processes.get_mut(&name).unwrap()
            }
            Some(name) if self.processes.len() < MAX_PROCESSES => {
                self.processes.entry(name).or_default()
            }
            Some(_) => &mut self.overflow,
            None => &mut self.unknown,
        };
        target.record(duration, privacy_only, outcome, reason_code, timing);
    }

    fn snapshot(&self, include_process_names: bool) -> Value {
        let processes: Vec<Value> = self
            .processes
            .iter()
            .enumerate()
            .map(|(index, (name, counts))| {
                let mut row = counts.snapshot();
                row["bucket"] = json!(index);
                if include_process_names {
                    row["basename"] = json!(name);
                }
                row
            })
            .collect();
        json!({
            "schema_version": 2,
            "process_limit": MAX_PROCESSES,
            "process_count": self.processes.len(),
            "reason_codes": REASON_CODES,
            "stage_names": timing::STAGE_NAMES,
            "mode_names": MODE_NAMES,
            "retry_cause_names": timing::RETRY_CAUSE_NAMES,
            "failure_phase_names": timing::PHASE_NAMES,
            "native_timeout_hresult_codes": timing::NATIVE_TIMEOUT_HRESULT_CODES,
            "total": self.total.snapshot(),
            "processes": processes,
            "unknown": self.unknown.snapshot(),
            "overflow": self.overflow.snapshot(),
        })
    }
}

/// Accept only a single conservative Windows executable basename. Rejecting a
/// valid but unusual name loses attribution, never an attempt or its metrics.
fn normalize_basename(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    if bytes.len() < 5
        || bytes.len() > MAX_BASENAME_BYTES
        || !raw.to_ascii_lowercase().ends_with(".exe")
    {
        return None;
    }
    let stem = &bytes[..bytes.len() - 4];
    if !stem.first()?.is_ascii_alphanumeric() || !stem.last()?.is_ascii_alphanumeric() {
        return None;
    }
    if !stem
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b' ' | b'_' | b'-' | b'.'))
    {
        return None;
    }
    Some(raw.to_ascii_lowercase())
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(Registry::default()))
}

/// Record exactly once for every finished attempt, after native verification.
/// An absent or invalid basename goes to `unknown`. All names stay in memory.
pub(super) fn record_attempt(
    verified_basename: Option<&str>,
    duration: Duration,
    privacy_only: bool,
    outcome: AttemptOutcome,
    reason_code: Option<&str>,
    timing: Option<AttemptTiming>,
) {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .record(
            verified_basename,
            duration,
            privacy_only,
            outcome,
            reason_code,
            timing,
        );
}

/// The authenticated endpoint alone may pass `true`; default callers must use
/// `false`, which yields numeric anonymous process buckets without basenames.
pub(super) fn metrics_snapshot(include_process_names: bool) -> Value {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .snapshot(include_process_names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn invalid_or_uncertain_identity_stays_unknown() {
        let mut registry = Registry::default();
        for name in [
            None,
            Some("C:\\Windows\\x.exe"),
            Some("https://x.exe"),
            Some("x.txt"),
            Some("weird💡.exe"),
            Some(" x.exe"),
            Some("x .exe"),
        ] {
            registry.record(
                name,
                Duration::from_micros(2),
                false,
                AttemptOutcome::Failed,
                Some("other"),
                None,
            );
        }
        assert_eq!(registry.unknown.attempts, 7);
        assert!(registry.processes.is_empty());
        // `unknown` is the fallback bucket and is always last, so derive its
        // index: hard-coding it silently broke when a reason code was inserted.
        let unknown = REASON_CODES.len() - 1;
        assert_eq!(registry.total.reason_counts[unknown], 7);
    }

    #[test]
    fn bounded_processes_preserve_all_totals_and_existing_buckets() {
        let mut registry = Registry::default();
        for index in 0..MAX_PROCESSES + 3 {
            let name = format!("app{index}.exe");
            registry.record(
                Some(&name),
                Duration::from_micros(10),
                false,
                AttemptOutcome::Completed,
                None,
                None,
            );
        }
        registry.record(
            Some("APP0.EXE"),
            Duration::from_micros(7),
            false,
            AttemptOutcome::Failed,
            Some("uia_walk_timeout"),
            None,
        );
        registry.record(
            None,
            Duration::from_micros(4),
            true,
            AttemptOutcome::Denied,
            None,
            None,
        );
        assert_eq!(registry.processes.len(), MAX_PROCESSES);
        assert_eq!(registry.overflow.attempts, 3);
        assert_eq!(registry.processes["app0.exe"].attempts, 2);
        assert_eq!(registry.total.attempts, (MAX_PROCESSES + 5) as u64);
        assert_eq!(
            registry.total.attempts,
            registry.total.completed + registry.total.failed + registry.total.denied
        );
        assert_eq!(
            registry.total.attempts,
            registry
                .processes
                .values()
                .map(|row| row.attempts)
                .sum::<u64>()
                + registry.unknown.attempts
                + registry.overflow.attempts
        );
        assert_eq!(
            registry.total.total_duration_us,
            (MAX_PROCESSES as u64 + 3) * 10 + 7 + 4
        );
        assert_eq!(registry.total.max_duration_us, 10);
    }

    #[test]
    fn stage_totals_maxima_and_named_projection() {
        let mut registry = Registry::default();
        let mut first = AttemptTiming::default();
        first.stage_calls[3] = 2;
        first.stage_us[3] = 90;
        let mut second = AttemptTiming::default();
        second.stage_calls[3] = 1;
        second.stage_us[3] = 20;
        registry.record(
            Some("Calculator.EXE"),
            Duration::from_micros(100),
            false,
            AttemptOutcome::Completed,
            None,
            Some(first),
        );
        registry.record(
            Some("calculator.exe"),
            Duration::from_micros(30),
            true,
            AttemptOutcome::Failed,
            Some("uia_node_limit"),
            Some(second),
        );
        assert_eq!(registry.total.stage_calls[3], 3);
        assert_eq!(registry.total.stage_total_us[3], 110);
        assert_eq!(registry.total.stage_max_us[3], 90);
        assert_eq!(registry.total.completed_stage_calls[3], 2);
        assert_eq!(registry.total.completed_stage_total_us[3], 90);
        assert_eq!(registry.total.failed_stage_calls[3], 1);
        assert_eq!(registry.total.failed_stage_total_us[3], 20);
        assert_eq!(registry.total.mode_attempt_counts, [1, 1]);
        let anonymous = registry.snapshot(false).to_string();
        assert!(!anonymous.contains("calculator.exe"));
        assert!(registry
            .snapshot(true)
            .to_string()
            .contains("calculator.exe"));
    }

    #[test]
    fn failed_attempt_diagnostics_exclude_success_and_denial() {
        let mut registry = Registry::default();
        let mut failure = AttemptTiming::default();
        failure.stage_calls[3] = 2;
        failure.stage_us[3] = 90;
        failure.retry_causes = [1, 2, 1, 0];
        failure.native_timeout_hresults = [1];
        failure.other_hresults = 2;
        failure.failure_budget = Some(timing::FailureBudget {
            phase: timing::Phase::Finalization,
            phase_remaining_us: 12,
            phase_expired: false,
            overall_remaining_us: 21,
            overall_expired: false,
        });
        let mut completed = AttemptTiming::default();
        completed.stage_calls[3] = 1;
        completed.stage_us[3] = 20;
        completed.retry_causes[0] = 3;
        completed.native_timeout_hresults[0] = 1;
        registry.record(
            Some("code.exe"),
            Duration::from_micros(100),
            true,
            AttemptOutcome::Failed,
            Some("uia_walk_timeout"),
            Some(failure),
        );
        registry.record(
            Some("code.exe"),
            Duration::from_micros(30),
            false,
            AttemptOutcome::Completed,
            None,
            Some(completed),
        );
        registry.record(
            Some("code.exe"),
            Duration::from_micros(5),
            false,
            AttemptOutcome::Denied,
            None,
            None,
        );
        let counts = &registry.processes["code.exe"];
        assert_eq!(counts.mode_attempt_counts, [2, 1]);
        assert_eq!(counts.stage_total_us[3], 110);
        assert_eq!(counts.failed_stage_total_us[3], 90);
        assert_eq!(counts.failed_stage_max_us[3], 90);
        assert_eq!(counts.completed_stage_total_us[3], 20);
        assert_eq!(counts.retry_cause_counts, [4, 2, 1, 0]);
        assert_eq!(counts.failed_retry_cause_counts, [1, 2, 1, 0]);
        assert_eq!(counts.native_timeout_hresult_counts, [2]);
        assert_eq!(counts.failed_native_timeout_hresult_counts, [1]);
        assert_eq!(counts.failed_other_hresult_count, 2);
        assert_eq!(counts.failure_phase_counts, [0, 0, 1]);
        assert_eq!(counts.failure_phase_remaining_total_us, [0, 0, 12]);
        assert_eq!(counts.failure_overall_remaining_total_us, 21);
        assert_eq!(counts.failures_with_budget, 1);
    }

    #[test]
    fn concurrent_records_reconcile() {
        let registry = Arc::new(Mutex::new(Registry::default()));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let registry = Arc::clone(&registry);
                thread::spawn(move || {
                    for _ in 0..100 {
                        registry.lock().unwrap().record(
                            Some("safe.exe"),
                            Duration::from_micros(1),
                            false,
                            AttemptOutcome::Failed,
                            None,
                            None,
                        );
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let registry = registry.lock().unwrap();
        assert_eq!(registry.total.attempts, 800);
        assert_eq!(registry.processes["safe.exe"].failed, 800);
        // `unknown` is the fallback bucket and is always last, so derive its
        // index: hard-coding it silently broke when a reason code was inserted.
        let unknown = REASON_CODES.len() - 1;
        assert_eq!(registry.total.reason_counts[unknown], 800);
    }
}
