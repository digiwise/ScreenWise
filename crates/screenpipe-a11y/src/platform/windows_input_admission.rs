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
//!
//! With sensitive diagnostics enabled, a separate observation-only UIA focus
//! subscription (the invalidating one is unchanged) classifies every focus
//! event by fixed codes `relation|element|native` plus the sender's UIA
//! control type:
//! - relation: `foreground` (the foreground process), `webview2_child` (a
//!   `msedgewebview2.exe` process started by the foreground process: the
//!   process test keyboard admission uses; admission's additional check that
//!   the focused window lies inside the foreground window is not applied,
//!   since a sender has no window), `other_process`, `unknown`;
//! - element / native: the sender against the UIA element, and native
//!   keyboard focus (sampled when the diagnostic callback ran, after the
//!   event) against the native focus, of the text buffered when the event's
//!   own invalidation ran. Each event is matched with its invalidation by
//!   sequence (both subscriptions see the same events in order); text
//!   admitted in an earlier focus epoch than that invalidation produced
//!   existed before it. `no_text` only when certainly nothing was buffered
//!   then; `ambiguous` whenever that is uncertain (an unmatched event, an
//!   older text that may have spanned it), never same or different.
//! The epoch record counts all focus events (`focus_events`, with `dropped`
//! and `uncorrelated` counts) and attributes each recheck discard's
//! keystrokes to the last focus event classified while its text was
//! buffered (`discard_focus`, in total and per process keyed like `by_pid`):
//! `none` only when every focus event so far is classified and none
//! concerned the text, `unclassified` when a focus invalidation after the
//! text began has no classified event yet. Element identities are compared
//! in memory only and never logged. Native focus may have changed again
//! between the event and its sample, so `native_*` is an approximation.

use super::{FocusEventFacts, NativeKeyboardFocus, SenderRelation, SENDER_RELATIONS};
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

/// A focus event's sender against the UIA element of the text buffered at
/// the event's invalidation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ElementMatch {
    Same,
    Other,
    Unknown,
    /// Certainly no text was buffered at the event's invalidation.
    NoText,
    /// Whether, or which, text was buffered then is not certain.
    Ambiguous,
}

/// Native keyboard focus when a focus event was handled against the
/// buffered text's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeMatch {
    Same,
    Differs,
    Unknown,
    /// Certainly no text was buffered at the event's invalidation.
    NoText,
    /// Whether, or which, text was buffered then is not certain.
    Ambiguous,
}

const MATCH_STATES: usize = 5;
const ELEMENT_CODES: [&str; MATCH_STATES] = [
    "same_element",
    "other_element",
    "element_unknown",
    "no_text",
    "ambiguous",
];
const NATIVE_CODES: [&str; MATCH_STATES] = [
    "native_same",
    "native_differs",
    "native_unknown",
    "no_text",
    "ambiguous",
];
/// Every relation x element x native combination, then `none` (a discard
/// certainly without a focus event while its text was buffered) and
/// `unclassified` (a discard whose last focus event was not yet classified).
const EVENT_CLASSES: usize = SENDER_RELATIONS * MATCH_STATES * MATCH_STATES;
const NO_FOCUS_EVENT: usize = EVENT_CLASSES;
const UNCLASSIFIED: usize = EVENT_CLASSES + 1;
const FOCUS_CLASSES: usize = EVENT_CLASSES + 2;
/// UIA control type ids 50000..=50040 by their fixed labels, then
/// `unavailable` (no cached control type) and `other` (any other id).
const FIRST_CONTROL_TYPE: i32 = 50000;
const KNOWN_CONTROL_TYPES: usize = 41;
const CONTROL_TYPE_SLOTS: usize = KNOWN_CONTROL_TYPES + 2;

/// Content-free class of one focus event (fixed codes; no identities).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FocusClass {
    pub(crate) relation: SenderRelation,
    pub(crate) element: ElementMatch,
    pub(crate) native: NativeMatch,
    pub(crate) control_type: Option<i32>,
}

impl FocusClass {
    fn index(self) -> usize {
        (self.relation.index() * MATCH_STATES + self.element as usize) * MATCH_STATES
            + self.native as usize
    }

    fn label(index: usize) -> String {
        match index {
            NO_FOCUS_EVENT => "none".to_string(),
            UNCLASSIFIED => "unclassified".to_string(),
            index => format!(
                "{}|{}|{}",
                SenderRelation::CODES[index / (MATCH_STATES * MATCH_STATES)],
                ELEMENT_CODES[index / MATCH_STATES % MATCH_STATES],
                NATIVE_CODES[index % MATCH_STATES]
            ),
        }
    }

    /// Whether the event was compared with certainly buffered text.
    pub(crate) fn compared(self) -> bool {
        !matches!(self.element, ElementMatch::NoText | ElementMatch::Ambiguous)
    }
}

/// What a recheck discard is attributed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiscardFocus {
    /// The last focus event classified while the text was buffered.
    Last(FocusClass),
    /// Certainly no focus event while the text was buffered.
    NoFocusEvent,
    /// A focus event after the text began is not classified yet.
    Unclassified,
}

fn control_type_slot(control_type: Option<i32>) -> usize {
    match control_type {
        None => KNOWN_CONTROL_TYPES,
        Some(id) => id
            .checked_sub(FIRST_CONTROL_TYPE)
            .and_then(|offset| usize::try_from(offset).ok())
            .filter(|offset| *offset < KNOWN_CONTROL_TYPES)
            .unwrap_or(KNOWN_CONTROL_TYPES + 1),
    }
}

fn control_type_label(slot: usize) -> &'static str {
    match slot {
        KNOWN_CONTROL_TYPES => "unavailable",
        slot if slot > KNOWN_CONTROL_TYPES => "other",
        slot => super::control_type_id_to_name(FIRST_CONTROL_TYPE + slot as i32),
    }
}

/// What buffered text was admitted for: its UIA element identity and native
/// keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextBinding {
    pub(crate) element: Option<u64>,
    pub(crate) focus: Option<NativeKeyboardFocus>,
}

/// The text buffered now: its binding and the focus epoch (generation >> 32)
/// it was admitted in (`None` if unknown).
#[derive(Clone, Copy, Debug)]
pub(crate) struct CurrentText {
    pub(crate) epoch: Option<u64>,
    pub(crate) binding: TextBinding,
}

/// The most recently ended buffered text: as `CurrentText`, plus the
/// privacy generation when it was emitted or discarded.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EndedText {
    pub(crate) epoch: Option<u64>,
    pub(crate) ended_generation: u64,
    pub(crate) binding: TextBinding,
}

/// Which text was buffered when a focus event's own invalidation ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextAtEvent {
    /// The current buffer (admitted before it, still buffered after it).
    Current(TextBinding),
    /// The most recently ended buffer (admitted before it, ended after it).
    Ended(TextBinding),
    /// Certainly none.
    None,
    /// Not certain; `current_possible` when that could be the current
    /// buffer.
    Ambiguous { current_possible: bool },
}

/// Decide which text was buffered at a focus event's invalidation, from the
/// generation that invalidation produced (`None`: the event could not be
/// correlated with it). Text admitted in an earlier focus epoch existed
/// before that invalidation; text admitted in its epoch or later did not.
/// Only the latest ended buffer is known, so when it began after the
/// invalidation an older one may have spanned it: ambiguous.
pub(crate) fn text_at_invalidation(
    invalidation: Option<u64>,
    current: Option<CurrentText>,
    previous: Option<EndedText>,
) -> TextAtEvent {
    let Some(invalidation) = invalidation else {
        return TextAtEvent::Ambiguous {
            current_possible: current.is_some(),
        };
    };
    let epoch = invalidation >> 32;
    if let Some(current) = current {
        match current.epoch {
            Some(admitted) if admitted < epoch => return TextAtEvent::Current(current.binding),
            Some(_) => {}
            None => {
                return TextAtEvent::Ambiguous {
                    current_possible: true,
                }
            }
        }
    }
    match previous {
        None => TextAtEvent::None,
        Some(previous) => match previous.epoch {
            Some(admitted) if admitted < epoch => {
                if previous.ended_generation >= invalidation {
                    TextAtEvent::Ended(previous.binding)
                } else {
                    TextAtEvent::None
                }
            }
            _ => TextAtEvent::Ambiguous {
                current_possible: false,
            },
        },
    }
}

/// Compare one focus event with the text buffered at its invalidation.
/// Missing facts classify as unknown, uncertain text as ambiguous (never
/// same/different); nothing here affects admission. Native focus is the
/// sample taken when the diagnostic callback ran, after the event.
pub(crate) fn classify_focus_event(facts: &FocusEventFacts, text: TextAtEvent) -> FocusClass {
    let (element, native) = match text {
        TextAtEvent::None => (ElementMatch::NoText, NativeMatch::NoText),
        TextAtEvent::Ambiguous { .. } => (ElementMatch::Ambiguous, NativeMatch::Ambiguous),
        TextAtEvent::Current(text) | TextAtEvent::Ended(text) => (
            match super::same_element(facts.element, text.element) {
                Some(true) => ElementMatch::Same,
                Some(false) => ElementMatch::Other,
                None => ElementMatch::Unknown,
            },
            match (facts.native, text.focus) {
                (Some(event), Some(text)) if event == text => NativeMatch::Same,
                (Some(_), Some(_)) => NativeMatch::Differs,
                _ => NativeMatch::Unknown,
            },
        ),
    };
    FocusClass {
        relation: facts.relation,
        element,
        native,
        control_type: facts.control_type,
    }
}

/// Bounded focus-event counts: per class and per sender control type.
/// For all focus events each event weighs 1; for discards each discard
/// weighs its keystrokes (`events` then counts discards).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FocusEventCounts {
    pub(crate) events: u64,
    /// Focus events dropped by the bounded hand-offs before classification.
    pub(crate) dropped: u64,
    /// Focus events that could not be correlated with their invalidation.
    pub(crate) uncorrelated: u64,
    classes: [u64; FOCUS_CLASSES],
    control_types: [u64; CONTROL_TYPE_SLOTS],
}

impl Default for FocusEventCounts {
    fn default() -> Self {
        Self {
            events: 0,
            dropped: 0,
            uncorrelated: 0,
            classes: [0; FOCUS_CLASSES],
            control_types: [0; CONTROL_TYPE_SLOTS],
        }
    }
}

impl FocusEventCounts {
    fn record_event(&mut self, class: FocusClass, correlated: bool) {
        self.events += 1;
        self.uncorrelated += u64::from(!correlated);
        self.classes[class.index()] += 1;
        self.control_types[control_type_slot(class.control_type)] += 1;
    }

    fn record_discard(&mut self, focus: DiscardFocus, keys: u64) {
        self.events += 1;
        match focus {
            DiscardFocus::Last(class) => {
                self.classes[class.index()] += keys;
                self.control_types[control_type_slot(class.control_type)] += keys;
            }
            DiscardFocus::NoFocusEvent => self.classes[NO_FOCUS_EVENT] += keys,
            DiscardFocus::Unclassified => self.classes[UNCLASSIFIED] += keys,
        }
    }

    fn add(&mut self, other: &Self) {
        self.events += other.events;
        self.dropped += other.dropped;
        self.uncorrelated += other.uncorrelated;
        for (total, value) in self.classes.iter_mut().zip(other.classes) {
            *total += value;
        }
        for (total, value) in self.control_types.iter_mut().zip(other.control_types) {
            *total += value;
        }
    }

    fn classes_json(&self) -> Value {
        Value::Object(
            (0..FOCUS_CLASSES)
                .filter(|index| self.classes[*index] > 0)
                .map(|index| (FocusClass::label(index), json!(self.classes[index])))
                .collect(),
        )
    }

    fn control_types_json(&self) -> Value {
        Value::Object(
            (0..CONTROL_TYPE_SLOTS)
                .filter(|slot| self.control_types[*slot] > 0)
                .map(|slot| {
                    (
                        control_type_label(slot).to_string(),
                        json!(self.control_types[slot]),
                    )
                })
                .collect(),
        )
    }

    /// All focus events: counts of events.
    pub(crate) fn events_json(&self) -> Value {
        json!({
            "events": self.events,
            "dropped": self.dropped,
            "uncorrelated": self.uncorrelated,
            "classes": self.classes_json(),
            "control_types": self.control_types_json(),
        })
    }

    /// Recheck discards by their last focus event: keystroke counts.
    pub(crate) fn discards_json(&self) -> Value {
        json!({
            "discards": self.events,
            "keys": self.classes.iter().sum::<u64>(),
            "classes": self.classes_json(),
            "control_types": self.control_types_json(),
        })
    }
}

/// One epoch's hook-side counts.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EpochKeys {
    pub(crate) totals: KeyCounts,
    /// Active typing time attributed to this epoch, in milliseconds.
    pub(crate) active_ms: u64,
    pub(crate) by_pid: HashMap<u32, KeyCounts>,
    /// Every classified UIA focus event handled in this epoch.
    pub(crate) focus_events: FocusEventCounts,
    /// Recheck discards by the last focus event while their text was
    /// buffered, in total and by the text's foreground process (keyed
    /// exactly as `by_pid`, including its shared overflow pid 0).
    pub(crate) discard_focus: FocusEventCounts,
    pub(crate) discard_focus_by_pid: HashMap<u32, FocusEventCounts>,
}

impl EpochKeys {
    pub(crate) fn add(&mut self, other: &Self) {
        self.totals.add(&other.totals);
        self.active_ms += other.active_ms;
        for (pid, counts) in &other.by_pid {
            bounded_entry(&mut self.by_pid, *pid).add(counts);
        }
        self.focus_events.add(&other.focus_events);
        self.discard_focus.add(&other.discard_focus);
        for (pid, counts) in &other.discard_focus_by_pid {
            let key = by_pid_key(&self.by_pid, *pid);
            self.discard_focus_by_pid
                .entry(key)
                .or_default()
                .add(counts);
        }
    }

    /// Keystroke counts or active time (focus-event diagnostics alone also
    /// create an epoch).
    pub(crate) fn has_keystrokes(&self) -> bool {
        self.totals != KeyCounts::default() || self.active_ms > 0
    }
}

/// The key `bounded_entry` gave `pid` in `by_pid` (its own pid or the shared
/// overflow pid 0), so per-process diagnostics never name another process.
fn by_pid_key(by_pid: &HashMap<u32, KeyCounts>, pid: u32) -> u32 {
    if by_pid.contains_key(&pid) {
        pid
    } else {
        0
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
    /// index) and `focus` what the discard is attributed to, both counted
    /// for recheck discards only (not pauses or exclusions).
    pub(crate) fn discarded(
        &mut self,
        pid: u32,
        epoch: u32,
        keys: u64,
        cause: DiscardCause,
        source: usize,
        focus: DiscardFocus,
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
        if recheck {
            entry.discard_focus.record_discard(focus, keys);
            let key = by_pid_key(&entry.by_pid, pid);
            entry
                .discard_focus_by_pid
                .entry(key)
                .or_default()
                .record_discard(focus, keys);
        }
    }

    /// One classified focus event, in the epoch it was handled.
    /// `correlated`: matched with its own invalidation.
    pub(crate) fn focus_event(&mut self, epoch: u32, class: FocusClass, correlated: bool) {
        self.epoch(epoch)
            .focus_events
            .record_event(class, correlated);
    }

    /// Focus events the bounded hand-off dropped before classification.
    pub(crate) fn focus_events_dropped(&mut self, epoch: u32, dropped: u64) {
        if dropped > 0 {
            self.epoch(epoch).focus_events.dropped += dropped;
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
        tally.discarded(
            1,
            4,
            2,
            DiscardCause::GenerationChanged,
            4,
            DiscardFocus::NoFocusEvent,
        );
        tally.discarded(1, 4, 1, DiscardCause::Paused, 0, DiscardFocus::NoFocusEvent);
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

    fn focus_facts(element: Option<u64>, native: Option<NativeKeyboardFocus>) -> FocusEventFacts {
        FocusEventFacts {
            at: Instant::now(),
            sequence: Some(1),
            invalidation: Some(5 << 32),
            relation: SenderRelation::WebView2Child,
            control_type: Some(50004),
            element,
            native,
        }
    }

    fn binding(element: Option<u64>, focus: Option<NativeKeyboardFocus>) -> TextBinding {
        TextBinding { element, focus }
    }

    #[test]
    fn focus_events_compare_only_with_text_certainly_buffered_at_their_invalidation() {
        let focus = NativeKeyboardFocus::for_test(1, 2, 3);
        let text = binding(Some(4), Some(focus));
        let older = binding(Some(8), Some(focus));
        let current = |epoch| {
            Some(CurrentText {
                epoch,
                binding: text,
            })
        };
        let ended = |epoch, ended_generation| {
            Some(EndedText {
                epoch,
                ended_generation,
                binding: older,
            })
        };
        let at = |current, previous| text_at_invalidation(Some(5 << 32), current, previous);
        use TextAtEvent as T;
        // Admitted in an earlier focus epoch and still buffered.
        assert_eq!(at(current(Some(4)), None), T::Current(text));
        // Admitted in the event's own epoch or later: began after it.
        assert_eq!(at(current(Some(5)), None), T::None);
        assert_eq!(at(current(Some(6)), ended(Some(3), 4 << 32)), T::None);
        // An ended buffer spanning the invalidation; one ended before it.
        assert_eq!(at(None, ended(Some(4), 5 << 32)), T::Ended(older));
        assert_eq!(at(current(Some(5)), ended(Some(4), (5 << 32) - 1)), T::None);
        // The latest ended buffer began after it: an older one may have
        // spanned it.
        assert_eq!(
            at(current(Some(6)), ended(Some(5), 6 << 32)),
            T::Ambiguous {
                current_possible: false
            }
        );
        // Unknown epochs and uncorrelated events are never compared.
        assert_eq!(
            at(current(None), None),
            T::Ambiguous {
                current_possible: true
            }
        );
        assert_eq!(
            at(None, ended(None, 9 << 32)),
            T::Ambiguous {
                current_possible: false
            }
        );
        assert_eq!(
            text_at_invalidation(None, current(Some(1)), None),
            T::Ambiguous {
                current_possible: true
            }
        );
        assert_eq!(
            text_at_invalidation(None, None, None),
            T::Ambiguous {
                current_possible: false
            }
        );
        assert_eq!(at(None, None), T::None);
    }

    #[test]
    fn focus_events_classify_missing_facts_as_unknown_and_uncertain_text_as_ambiguous() {
        let focus = NativeKeyboardFocus::for_test(1, 2, 3);
        let other = NativeKeyboardFocus::for_test(1, 9, 3);
        let class = |facts: FocusEventFacts, text| {
            let class = classify_focus_event(&facts, text);
            assert_eq!(class.relation, SenderRelation::WebView2Child);
            assert_eq!(class.control_type, Some(50004));
            (class.element, class.native)
        };
        use ElementMatch as E;
        use NativeMatch as N;
        use TextAtEvent as T;
        let facts = focus_facts(Some(4), Some(focus));
        assert_eq!(class(facts, T::None), (E::NoText, N::NoText));
        for current_possible in [false, true] {
            assert_eq!(
                class(facts, T::Ambiguous { current_possible }),
                (E::Ambiguous, N::Ambiguous),
                "uncertain text is never same or different"
            );
        }
        for found in [T::Current, T::Ended] {
            assert_eq!(
                class(facts, found(binding(Some(4), Some(focus)))),
                (E::Same, N::Same)
            );
            assert_eq!(
                class(facts, found(binding(Some(8), Some(other)))),
                (E::Other, N::Differs)
            );
            assert_eq!(
                class(facts, found(binding(None, Some(focus)))),
                (E::Unknown, N::Same)
            );
            assert_eq!(
                class(facts, found(binding(Some(4), None))),
                (E::Same, N::Unknown)
            );
            assert_eq!(
                class(
                    focus_facts(None, None),
                    found(binding(Some(4), Some(focus)))
                ),
                (E::Unknown, N::Unknown)
            );
        }

        // Every class has its own slot and fixed label.
        let mut labels = std::collections::HashSet::new();
        for relation in [
            SenderRelation::Foreground,
            SenderRelation::WebView2Child,
            SenderRelation::OtherProcess,
            SenderRelation::Unknown,
        ] {
            for element in [E::Same, E::Other, E::Unknown, E::NoText, E::Ambiguous] {
                for native in [N::Same, N::Differs, N::Unknown, N::NoText, N::Ambiguous] {
                    let class = FocusClass {
                        relation,
                        element,
                        native,
                        control_type: None,
                    };
                    assert!(class.index() < EVENT_CLASSES);
                    assert!(labels.insert(FocusClass::label(class.index())));
                    assert_eq!(
                        class.compared(),
                        !matches!(element, E::NoText | E::Ambiguous)
                    );
                }
            }
        }
        assert_eq!(labels.len(), EVENT_CLASSES);
        assert_eq!(FocusClass::label(NO_FOCUS_EVENT), "none");
        assert_eq!(FocusClass::label(UNCLASSIFIED), "unclassified");
        assert_eq!(control_type_label(control_type_slot(Some(50004))), "Edit");
        assert_eq!(control_type_label(control_type_slot(Some(50040))), "AppBar");
        assert_eq!(control_type_label(control_type_slot(None)), "unavailable");
        for id in [50041, 49999, 0, i32::MIN, i32::MAX] {
            assert_eq!(control_type_label(control_type_slot(Some(id))), "other");
        }
    }

    #[test]
    fn focus_counters_attribute_recheck_discards_per_process_with_bounded_fixed_labels() {
        let focus = NativeKeyboardFocus::for_test(1, 2, 3);
        let same = classify_focus_event(
            &focus_facts(Some(4), Some(focus)),
            TextAtEvent::Current(binding(Some(4), Some(focus))),
        );
        let no_text = classify_focus_event(&focus_facts(None, None), TextAtEvent::None);
        let mut tally = KeyAdmissionTally::default();
        tally.focus_event(2, same, true);
        tally.focus_event(2, no_text, false);
        tally.focus_events_dropped(2, 3);
        tally.focus_events_dropped(2, 0);
        let last = DiscardFocus::Last(same);
        tally.discarded(7, 2, 3, DiscardCause::GenerationChanged, 0, last);
        tally.discarded(
            7,
            2,
            2,
            DiscardCause::ElementChanged,
            0,
            DiscardFocus::NoFocusEvent,
        );
        tally.discarded(
            7,
            2,
            1,
            DiscardCause::HoldExpired,
            0,
            DiscardFocus::Unclassified,
        );
        tally.discarded(8, 2, 0, DiscardCause::GenerationChanged, 0, last);
        // Pauses and exclusions are not rechecks: no focus attribution.
        tally.discarded(7, 2, 4, DiscardCause::Paused, 0, last);
        tally.discarded(7, 2, 4, DiscardCause::Excluded, 0, last);
        let report = tally.take_due(3, false).unwrap();
        let epoch = &report.epochs[&2];
        assert_eq!(
            epoch.focus_events.events_json(),
            json!({
                "events": 2,
                "dropped": 3,
                "uncorrelated": 1,
                "classes": {
                    "webview2_child|same_element|native_same": 1,
                    "webview2_child|no_text|no_text": 1,
                },
                "control_types": {"Edit": 2},
            })
        );
        let discards = json!({
            "discards": 3,
            "keys": 6,
            "classes": {
                "webview2_child|same_element|native_same": 3,
                "none": 2,
                "unclassified": 1,
            },
            "control_types": {"Edit": 3},
        });
        assert_eq!(epoch.discard_focus.discards_json(), discards);
        assert_eq!(epoch.discard_focus_by_pid[&7].discards_json(), discards);
        assert!(
            !epoch.discard_focus_by_pid.contains_key(&8),
            "no keys, no entry"
        );
        assert_eq!(epoch.totals.discarded, 14, "discard totals are unchanged");
        assert!(epoch.has_keystrokes());
        // Merging keeps every counter.
        let mut merged = epoch.clone();
        merged.add(epoch);
        assert_eq!(merged.focus_events.events, 4);
        assert_eq!(merged.focus_events.dropped, 6);
        assert_eq!(merged.focus_events.uncorrelated, 2);
        assert_eq!(merged.discard_focus_by_pid[&7].discards_json()["keys"], 12);
        // Focus events alone create an epoch without keystrokes (the app log
        // skips it).
        tally.focus_event(4, no_text, true);
        tally.focus_events_dropped(5, 1);
        let only_focus = tally.take_due(9, false).unwrap();
        assert!(only_focus
            .epochs
            .values()
            .all(|epoch| !epoch.has_keystrokes()));
        // Per-process discards follow `by_pid`'s bound and its overflow pid 0:
        // no process appears that `by_pid` does not name.
        for pid in 100..200 {
            tally.discarded(pid, 3, 1, DiscardCause::HoldExpired, 0, last);
        }
        let report = tally.take_due(0, true).unwrap();
        let epoch = &report.epochs[&3];
        assert!(epoch.discard_focus_by_pid.len() <= MAX_TRACKED_PROCESSES + 1);
        assert!(epoch
            .discard_focus_by_pid
            .keys()
            .all(|pid| epoch.by_pid.contains_key(pid)));
        assert_eq!(epoch.discard_focus_by_pid[&0].discards_json()["keys"], 68);
        assert_eq!(epoch.discard_focus.discards_json()["keys"], 100);
        // Merging into an epoch whose `by_pid` is full keeps that mapping.
        let mut full = EpochKeys::default();
        for pid in 1000..1000 + MAX_TRACKED_PROCESSES as u32 {
            bounded_entry(&mut full.by_pid, pid).admitted += 1;
        }
        let mut extra = EpochKeys::default();
        bounded_entry(&mut extra.by_pid, 5).discarded += 1;
        extra
            .discard_focus_by_pid
            .entry(5)
            .or_default()
            .record_discard(last, 1);
        full.add(&extra);
        assert!(!full.discard_focus_by_pid.contains_key(&5));
        assert!(full.by_pid.contains_key(&0) && full.discard_focus_by_pid.contains_key(&0));
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
