//! Content-free keystroke admission and keyboard-probe outcome counters.
//!
//! The low-level hook counts recordable keystrokes (one per press of a
//! non-modifier key; a chord and an auto-repeat count once) per 10-second
//! epoch: admitted, or lost with the gate's denial reason, plus the active
//! typing time (gaps under 10 s between recordable keystrokes). Keystrokes
//! blocked by the lock/schedule/DRM pause or a configured window filter are
//! not counted, nor are gate denials labelled as a password field or excluded
//! application. The label needs the worker's latest verdict for the same
//! native focus (kept until a later verdict replaces it), so keystrokes
//! before that verdict exists (a password typed right after Tab, an
//! application excluded only by the worker) or while its lock is briefly
//! contended are still counted as lost. These counts are
//! diagnostics, not a guarantee that nothing about such input is recorded.
//! The hook thread hands completed epochs to the keyboard privacy worker,
//! which joins them with `input_accounting`'s persisted/lost dispositions and
//! writes totals to the app log and per-process counts (with executable
//! basenames) only to the opt-in sensitive diagnostics. No key, text, title or
//! URL enters these counters.

use super::{InvalidationSource, INVALIDATION_SOURCES};
use super::{KeyboardPrivacyDenial, KEYBOARD_PRIVACY_DENIAL_REASONS};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

/// Bounded per-process cardinality; further processes share pid 0.
const MAX_TRACKED_PROCESSES: usize = 32;
const MAX_FAILURE_KEYS: usize = 32;
const MAX_NAMED_PROCESSES: usize = 64;
/// Epochs held by the hook while the worker's hand-off is contended.
const MAX_HELD_EPOCHS: usize = 360;

/// Why admitted keystrokes were dropped from the text buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiscardCause {
    /// A key denied for a reason that may mean focus moved.
    DeniedKey,
    GenerationChanged,
    NativeFocusChanged,
    /// Same generation and native window, but a different UIA element.
    ElementChanged,
    /// The idle flush could not re-verify the field.
    FlushUnverified,
    /// No re-verification arrived within the hold period.
    HoldExpired,
    /// Tab/Enter/click/Alt/Win with no fresh permit for the source field.
    NavigationUnverified,
    Paused,
    Excluded,
}
pub(crate) const DISCARD_CAUSES: usize = 9;

impl DiscardCause {
    fn code(index: usize) -> &'static str {
        [
            "denied_key",
            "generation_changed",
            "native_focus_changed",
            "element_changed",
            "flush_unverified",
            "hold_expired",
            "navigation_unverified",
            "paused",
            "excluded",
        ][index]
    }
}
const NAME_REFRESH: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyCounts {
    /// Recordable keystrokes the gate admitted.
    pub(crate) admitted: u64,
    /// Of those, admitted under an owner-authorised relaxation.
    pub(crate) admitted_relaxed: u64,
    /// Recordable keystrokes lost at the gate, by denial reason.
    pub(crate) denied: [u64; KEYBOARD_PRIVACY_DENIAL_REASONS],
    /// Admitted keystrokes later dropped from the text buffer, by cause
    /// (counted when dropped; the ledger attributes them to typing epochs).
    pub(crate) discarded: u64,
    pub(crate) discarded_by: [u64; DISCARD_CAUSES],
    /// Of those dropped by a recheck, by the latest invalidation's source.
    pub(crate) discarded_after: [u64; INVALIDATION_SOURCES + 1],
}

impl KeyCounts {
    pub(crate) fn add(&mut self, other: &Self) {
        self.admitted += other.admitted;
        self.admitted_relaxed += other.admitted_relaxed;
        for (total, value) in self.denied.iter_mut().zip(other.denied) {
            *total += value;
        }
        self.discarded += other.discarded;
        for (total, value) in self.discarded_by.iter_mut().zip(other.discarded_by) {
            *total += value;
        }
        for (total, value) in self.discarded_after.iter_mut().zip(other.discarded_after) {
            *total += value;
        }
    }

    /// Recordable keystrokes lost at the gate.
    pub(crate) fn suppressed(&self) -> u64 {
        self.denied.iter().sum()
    }

    pub(crate) fn denied_json(&self) -> Value {
        Value::Object(
            KeyboardPrivacyDenial::ALL
                .into_iter()
                .filter(|denial| self.denied[denial.index()] > 0)
                .map(|denial| {
                    (
                        denial.code().to_string(),
                        json!(self.denied[denial.index()]),
                    )
                })
                .collect(),
        )
    }

    pub(crate) fn discarded_json(&self) -> Value {
        Value::Object(
            (0..DISCARD_CAUSES)
                .filter(|index| self.discarded_by[*index] > 0)
                .map(|index| {
                    (
                        DiscardCause::code(index).to_string(),
                        json!(self.discarded_by[index]),
                    )
                })
                .collect(),
        )
    }

    pub(crate) fn discarded_after_json(&self) -> Value {
        Value::Object(
            InvalidationSource::CODES
                .iter()
                .zip(self.discarded_after)
                .filter(|(_, count)| *count > 0)
                .map(|(code, count)| (code.to_string(), json!(count)))
                .collect(),
        )
    }
}

/// One epoch's hook-side counts.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EpochKeys {
    pub(crate) totals: KeyCounts,
    /// Active typing time attributed to this epoch, in milliseconds.
    pub(crate) active_ms: u64,
    pub(crate) by_pid: HashMap<u32, KeyCounts>,
}

impl EpochKeys {
    pub(crate) fn add(&mut self, other: &Self) {
        self.totals.add(&other.totals);
        self.active_ms += other.active_ms;
        for (pid, counts) in &other.by_pid {
            bounded_entry(&mut self.by_pid, *pid).add(counts);
        }
    }
}

/// Completed epochs handed from the hook thread to the worker.
#[derive(Debug, Default)]
pub(crate) struct KeyAdmissionReport {
    pub(crate) epochs: BTreeMap<u32, EpochKeys>,
}

impl KeyAdmissionReport {
    /// Fold another unreported report into this one (bounded hand-off).
    pub(crate) fn merge_earlier(&mut self, earlier: Self) {
        for (epoch, keys) in earlier.epochs {
            self.epochs.entry(epoch).or_default().add(&keys);
        }
    }
}

fn bounded_entry<T: Default>(map: &mut HashMap<u32, T>, pid: u32) -> &mut T {
    let key = if map.contains_key(&pid) || map.len() < MAX_TRACKED_PROCESSES {
        pid
    } else {
        0
    };
    map.entry(key).or_default()
}

/// Owned by the hook thread; never locked.
#[derive(Default)]
pub(crate) struct KeyAdmissionTally {
    report: KeyAdmissionReport,
}

impl KeyAdmissionTally {
    fn epoch(&mut self, epoch: u32) -> &mut EpochKeys {
        if !self.report.epochs.contains_key(&epoch) && self.report.epochs.len() >= MAX_HELD_EPOCHS {
            // Never grow without bound if the worker stops taking reports.
            let oldest = *self.report.epochs.keys().next().unwrap_or(&epoch);
            let dropped = self.report.epochs.remove(&oldest).unwrap_or_default();
            self.report.epochs.entry(epoch).or_default().add(&dropped);
        }
        self.report.epochs.entry(epoch).or_default()
    }

    pub(crate) fn admitted(&mut self, pid: u32, epoch: u32, relaxed: bool) {
        let entry = self.epoch(epoch);
        let relaxed = u64::from(relaxed);
        entry.totals.admitted += 1;
        entry.totals.admitted_relaxed += relaxed;
        let process = bounded_entry(&mut entry.by_pid, pid);
        process.admitted += 1;
        process.admitted_relaxed += relaxed;
    }

    /// Counts a lost keystroke; an intended block is ignored entirely.
    pub(crate) fn denied(&mut self, pid: u32, epoch: u32, denial: KeyboardPrivacyDenial) {
        if denial.intended() {
            return;
        }
        let entry = self.epoch(epoch);
        entry.totals.denied[denial.index()] += 1;
        bounded_entry(&mut entry.by_pid, pid).denied[denial.index()] += 1;
    }

    pub(crate) fn active(&mut self, epoch: u32, gap: Duration) {
        self.epoch(epoch).active_ms += gap.as_millis() as u64;
    }

    /// `source` is the latest invalidation (`InvalidationSource::CODES`
    /// index), counted for recheck discards only (not pauses or exclusions).
    pub(crate) fn discarded(
        &mut self,
        pid: u32,
        epoch: u32,
        keys: u64,
        cause: DiscardCause,
        source: usize,
    ) {
        if keys == 0 {
            return;
        }
        let recheck = !matches!(cause, DiscardCause::Paused | DiscardCause::Excluded);
        let entry = self.epoch(epoch);
        for counts in [&mut entry.totals, bounded_entry(&mut entry.by_pid, pid)] {
            counts.discarded += keys;
            counts.discarded_by[cause as usize] += keys;
            if recheck {
                counts.discarded_after[source.min(INVALIDATION_SOURCES)] += keys;
            }
        }
    }

    /// Epochs before `current` (all of them when `force`), then forget them.
    pub(crate) fn take_due(&mut self, current: u32, force: bool) -> Option<KeyAdmissionReport> {
        let keep = if force {
            BTreeMap::new()
        } else {
            self.report.epochs.split_off(&current)
        };
        let done = std::mem::replace(&mut self.report.epochs, keep);
        (!done.is_empty()).then_some(KeyAdmissionReport { epochs: done })
    }

    /// Restore a report whose hand-off was contended.
    pub(crate) fn restore(&mut self, report: KeyAdmissionReport) {
        for (epoch, keys) in report.epochs {
            self.epoch(epoch).add(&keys);
        }
    }
}

/// When a recordable keystroke at `now` adds active time: the gap since the
/// previous recordable keystroke, if shorter than ten seconds.
pub(crate) fn active_gap(previous: Option<Instant>, now: Instant) -> Option<Duration> {
    let gap = now.saturating_duration_since(previous?);
    (gap < Duration::from_secs(10)).then_some(gap)
}

/// Modifier keys pressed on their own are not keystrokes.
pub(crate) fn is_modifier(vk: u16) -> bool {
    matches!(vk, 0x10..=0x12 | 0xA0..=0xA5 | 0x5B | 0x5C)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeSkip {
    Excluded,
    Breaker,
    LongWalk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeOutcome {
    Allowed,
    /// Allowed under an owner-authorised relaxation.
    AllowedRelaxed(super::ProbeRelaxation),
    Password,
    Skipped(ProbeSkip),
    VisualGateClosed,
    Failed {
        reason: &'static str,
        stage: &'static str,
        /// Fixed HRESULT label, or a fixed native-sample detail.
        hresult: Option<&'static str>,
    },
}

impl ProbeOutcome {
    /// Outcomes inside a password field, an excluded application or a
    /// privacy pause are counted without a process.
    fn unattributed(self) -> bool {
        matches!(
            self,
            Self::Password | Self::VisualGateClosed | Self::Skipped(ProbeSkip::Excluded)
        )
    }
}

/// One keyboard-privacy probe, for aggregate diagnostics only.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProbeRecord {
    pub(crate) pid: u32,
    pub(crate) control_pid: Option<u32>,
    pub(crate) duration: Duration,
    pub(crate) outcome: ProbeOutcome,
}

/// Fixed HRESULT labels seen in probe failures; anything else is "other".
pub(crate) fn hresult_label(error: &str) -> Option<&'static str> {
    const KNOWN: [&str; 8] = [
        "0x80131505", // UIA_E_TIMEOUT
        "0x80070057", // E_INVALIDARG (non-Boolean IsPassword)
        "0x80040201", // UIA_E_ELEMENTNOTAVAILABLE
        "0x80004005", // E_FAIL
        "0x80004002", // E_NOINTERFACE
        "0x8001010D", // RPC_E_CANTCALLOUT_ININPUTSYNCCALL
        "0x80010108", // RPC_E_DISCONNECTED
        "0x800706BA", // RPC_S_SERVER_UNAVAILABLE
    ];
    let start = error.find("HRESULT(")? + "HRESULT(".len();
    let code = error[start..].split(')').next()?;
    Some(
        KNOWN
            .into_iter()
            .find(|known| known.eq_ignore_ascii_case(code))
            .unwrap_or("other"),
    )
}

#[derive(Clone, Debug, Default)]
struct ProbeCounts {
    probes: u64,
    allowed: u64,
    /// Allowed under: unsupported IsPassword assumed, focus assumed.
    allowed_relaxed: [u64; 2],
    password: u64,
    visual_gate_closed: u64,
    skipped: [u64; 3],
    failures: BTreeMap<(&'static str, &'static str, &'static str), u64>,
    /// <10, <25, <50, <75, >=75 ms.
    duration_buckets: [u64; 5],
    max_duration_us: u64,
    other_control_process: u64,
}

impl ProbeCounts {
    fn record(&mut self, record: &ProbeRecord) {
        self.probes += 1;
        match record.outcome {
            ProbeOutcome::Allowed => self.allowed += 1,
            ProbeOutcome::AllowedRelaxed(relaxation) => {
                self.allowed += 1;
                self.allowed_relaxed[match relaxation {
                    super::ProbeRelaxation::PasswordStateAssumed => 0,
                    super::ProbeRelaxation::FocusAssumed => 1,
                }] += 1;
            }
            ProbeOutcome::Password => self.password += 1,
            ProbeOutcome::VisualGateClosed => self.visual_gate_closed += 1,
            ProbeOutcome::Skipped(skip) => {
                self.skipped[match skip {
                    ProbeSkip::Excluded => 0,
                    ProbeSkip::Breaker => 1,
                    ProbeSkip::LongWalk => 2,
                }] += 1
            }
            ProbeOutcome::Failed {
                reason,
                stage,
                hresult,
            } => {
                let key = (reason, stage, hresult.unwrap_or("-"));
                let key =
                    if self.failures.contains_key(&key) || self.failures.len() < MAX_FAILURE_KEYS {
                        key
                    } else {
                        ("overflow", "overflow", "-")
                    };
                *self.failures.entry(key).or_default() += 1;
            }
        }
        let ms = record.duration.as_millis();
        self.duration_buckets[match ms {
            0..10 => 0,
            10..25 => 1,
            25..50 => 2,
            50..75 => 3,
            _ => 4,
        }] += 1;
        self.max_duration_us = self
            .max_duration_us
            .max(record.duration.as_micros().min(u64::MAX as u128) as u64);
        if record.control_pid.is_some_and(|pid| pid != record.pid) {
            self.other_control_process += 1;
        }
    }

    fn failures_total(&self) -> u64 {
        self.failures.values().sum()
    }

    fn to_json(&self) -> Value {
        json!({
            "probes": self.probes,
            "allowed": self.allowed,
            "allowed_password_state_assumed": self.allowed_relaxed[0],
            "allowed_focus_assumed": self.allowed_relaxed[1],
            "password": self.password,
            "visual_gate_closed": self.visual_gate_closed,
            "skipped_excluded": self.skipped[0],
            "skipped_breaker": self.skipped[1],
            "skipped_long_walk": self.skipped[2],
            "failed": self.failures_total(),
            "failures": self.failures.iter().map(|((reason, stage, hresult), count)| {
                json!({"reason": reason, "stage": stage, "hresult": hresult, "count": count})
            }).collect::<Vec<_>>(),
            "duration_ms_buckets_lt10_lt25_lt50_lt75_ge75": self.duration_buckets,
            "max_duration_us": self.max_duration_us,
            "other_control_process": self.other_control_process,
        })
    }
}

/// Worker-owned aggregate of probe outcomes for one interval, plus the
/// executable basenames of attributed processes, resolved while each was the
/// foreground process (one cheap query per process per 30 s).
pub(crate) struct ProbeTally {
    started: Option<Instant>,
    totals: ProbeCounts,
    by_pid: HashMap<u32, ProbeCounts>,
    names: HashMap<u32, (Option<String>, Instant)>,
    resolve: fn(u32) -> Option<String>,
}

impl Default for ProbeTally {
    fn default() -> Self {
        Self::with_resolver(super::process_image_basename)
    }
}

pub(crate) struct ProbeReport {
    pub(crate) interval: Duration,
    pub(crate) totals: Value,
    pub(crate) failures_by_reason: String,
    pub(crate) probes: u64,
    pub(crate) allowed: u64,
    pub(crate) failed: u64,
    pub(crate) skipped: u64,
    pub(crate) by_pid: Vec<(u32, Value)>,
}

impl ProbeTally {
    pub(crate) fn with_resolver(resolve: fn(u32) -> Option<String>) -> Self {
        Self {
            started: None,
            totals: ProbeCounts::default(),
            by_pid: HashMap::new(),
            names: HashMap::new(),
            resolve,
        }
    }

    pub(crate) fn record(&mut self, record: ProbeRecord) {
        self.started.get_or_insert_with(Instant::now);
        self.totals.record(&record);
        let pid = if record.outcome.unattributed() {
            0
        } else {
            record.pid
        };
        bounded_entry(&mut self.by_pid, pid).record(&record);
        if pid != 0
            && self
                .names
                .get(&pid)
                .is_none_or(|(_, at)| at.elapsed() >= NAME_REFRESH)
        {
            if self.names.len() >= MAX_NAMED_PROCESSES && !self.names.contains_key(&pid) {
                if let Some(oldest) = self
                    .names
                    .iter()
                    .min_by_key(|(_, (_, at))| *at)
                    .map(|(pid, _)| *pid)
                {
                    self.names.remove(&oldest);
                }
            }
            self.names
                .insert(pid, ((self.resolve)(pid), Instant::now()));
        }
    }

    /// Basenames of recently attributed foreground processes.
    pub(crate) fn names(&self) -> HashMap<u32, String> {
        self.names
            .iter()
            .filter_map(|(pid, (name, _))| name.clone().map(|name| (*pid, name)))
            .collect()
    }

    pub(crate) fn take_due(&mut self, interval: Duration) -> Option<ProbeReport> {
        let started = self.started?;
        if started.elapsed() < interval {
            return None;
        }
        self.started = None;
        let totals = std::mem::take(&mut self.totals);
        let by_pid = std::mem::take(&mut self.by_pid);
        let mut by_reason: BTreeMap<&str, u64> = BTreeMap::new();
        for ((reason, _, _), count) in &totals.failures {
            *by_reason.entry(reason).or_default() += count;
        }
        Some(ProbeReport {
            interval: started.elapsed(),
            failures_by_reason: by_reason
                .iter()
                .map(|(reason, count)| format!("{reason}:{count}"))
                .collect::<Vec<_>>()
                .join(","),
            // Excludes password-field, exclusion and pause outcomes (dwell time).
            probes: totals.probes - totals.password - totals.skipped[0] - totals.visual_gate_closed,
            allowed: totals.allowed,
            failed: totals.failures_total(),
            // Only provider-protection skips: password-field, exclusion and
            // pause outcomes would show how long the user spent there.
            skipped: totals.skipped[1] + totals.skipped[2],
            totals: totals.to_json(),
            by_pid: by_pid
                .into_iter()
                .map(|(pid, counts)| (pid, counts.to_json()))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_tally_counts_recordable_keystrokes_per_epoch_but_never_intended_blocks() {
        let mut tally = KeyAdmissionTally::default();
        assert!(tally.take_due(0, true).is_none());
        tally.admitted(1, 3, false);
        tally.admitted(1, 3, true);
        tally.denied(1, 3, KeyboardPrivacyDenial::DecisionStale);
        tally.denied(2, 3, KeyboardPrivacyDenial::PasswordField);
        tally.denied(3, 4, KeyboardPrivacyDenial::Excluded);
        tally.active(3, Duration::from_millis(1500));
        tally.discarded(1, 4, 2, DiscardCause::GenerationChanged, 4);
        tally.discarded(1, 4, 1, DiscardCause::Paused, 0);
        let report = tally.take_due(4, false).unwrap();
        assert_eq!(report.epochs.len(), 1, "the current epoch stays open");
        let three = &report.epochs[&3];
        assert_eq!(three.totals.admitted, 2);
        assert_eq!(three.totals.admitted_relaxed, 1);
        assert_eq!(three.totals.suppressed(), 1);
        assert_eq!(three.active_ms, 1500);
        assert!(
            !three.by_pid.contains_key(&2) && !three.by_pid.contains_key(&3),
            "intended blocks leave no per-process trace"
        );
        let mut rest = tally.take_due(5, false).unwrap();
        assert_eq!(rest.epochs[&4].totals.discarded, 3);
        assert_eq!(
            rest.epochs[&4].totals.discarded_after_json(),
            json!({"probe_timeout": 2}),
            "pauses carry no invalidation source"
        );
        assert_eq!(
            rest.epochs[&4].totals.discarded_json()["generation_changed"],
            2
        );
        assert!(tally.take_due(9, true).is_none());
        // A contended hand-off is restored and reported on the next tick.
        tally.admitted(9, 6, false);
        rest.merge_earlier(KeyAdmissionReport::default());
        tally.restore(rest);
        let again = tally.take_due(9, true).unwrap();
        assert_eq!(again.epochs[&4].totals.discarded, 3);
        assert_eq!(again.epochs[&6].totals.admitted, 1);
        assert!(is_modifier(0x10) && is_modifier(0xA5) && is_modifier(0x5B));
        assert!(!is_modifier(0x41) && !is_modifier(0x0D));
        let t = Instant::now();
        assert_eq!(active_gap(None, t), None);
        assert_eq!(
            active_gap(Some(t), t + Duration::from_secs(3)),
            Some(Duration::from_secs(3))
        );
        assert_eq!(active_gap(Some(t), t + Duration::from_secs(10)), None);
    }

    #[test]
    fn probe_tally_keys_failures_by_fixed_labels_and_hides_intended_blocks() {
        assert_eq!(
            hresult_label("Error { code: HRESULT(0x80131505), message: \"\" }"),
            Some("0x80131505")
        );
        assert_eq!(hresult_label("HRESULT(0x12345678)"), Some("other"));
        assert_eq!(
            hresult_label("has_keyboard_focus=false; element_pid=1"),
            None
        );
        fn resolver(pid: u32) -> Option<String> {
            Some(format!("app{pid}.exe"))
        }
        let mut tally = ProbeTally::with_resolver(resolver);
        let record = |pid, outcome| ProbeRecord {
            pid,
            control_pid: Some(8),
            duration: Duration::from_millis(30),
            outcome,
        };
        tally.record(record(7, ProbeOutcome::Allowed));
        tally.record(record(7, ProbeOutcome::Skipped(ProbeSkip::Breaker)));
        for _ in 0..2 {
            tally.record(record(
                7,
                ProbeOutcome::Failed {
                    reason: "capture_input_focused_element_failed",
                    stage: "GetFocusedElementBuildCache",
                    hresult: Some("0x80131505"),
                },
            ));
        }
        tally.record(record(9, ProbeOutcome::Password));
        tally.record(record(10, ProbeOutcome::Skipped(ProbeSkip::Excluded)));
        let names = tally.names();
        assert_eq!(names.get(&7).map(String::as_str), Some("app7.exe"));
        assert!(
            !names.contains_key(&9) && !names.contains_key(&10),
            "password and excluded foregrounds are not named"
        );
        let report = tally.take_due(Duration::ZERO).unwrap();
        assert_eq!(
            (report.probes, report.allowed, report.failed, report.skipped),
            (4, 1, 2, 1)
        );
        assert_eq!(
            report.failures_by_reason,
            "capture_input_focused_element_failed:2"
        );
        let pids: Vec<u32> = report.by_pid.iter().map(|(pid, _)| *pid).collect();
        assert!(pids.contains(&7) && pids.contains(&0));
        assert!(!pids.contains(&9) && !pids.contains(&10));
        assert!(tally.take_due(Duration::ZERO).is_none());
    }
}
