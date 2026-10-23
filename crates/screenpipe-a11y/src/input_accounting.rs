//! Content-free end-to-end accounting of recordable keystrokes.
//!
//! A keystroke is one press of a non-modifier key (a chord counts once, an
//! auto-repeat counts once). The input hook counts each recordable keystroke
//! with its gate outcome and active time, per 10-second epoch since the hook
//! started. Every admitted keystroke travels as a [`KeystrokeTag`] with the
//! event that carries its effect (text, key or edit) and ends here exactly
//! once: persisted when the database confirms the insert, or lost with a
//! fixed cause. No key, text, title or URL enters this module.
//!
//! Noting a disposition never blocks (the input hook notes losses): when the
//! ledger is busy the note is queued and applied by the next ledger user.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock, TryLockError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Length of one accounting epoch.
pub const EPOCH: Duration = Duration::from_secs(10);
/// Bounded per-process cardinality per epoch; further processes share pid 0.
const MAX_PROCESSES_PER_EPOCH: usize = 32;
/// Epochs kept while waiting for persistence before they are dropped unreported.
const MAX_OPEN_EPOCHS: usize = 4096;

/// What an admitted keystroke's effect was recorded as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeystrokeKind {
    /// Characters (and edits applied to them) in a text event.
    Text,
    /// A key or clipboard-operation event.
    Key,
}

/// Admitted keystrokes carried by one event, attributed to the epochs and the
/// foreground process in which they were typed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeystrokeTag {
    pub pid: u32,
    /// `(epoch, keystrokes)` in typing order.
    pub epochs: Vec<(u32, u32)>,
    /// Of those, Backspace/Delete edits applied to the text.
    pub edits: u32,
    pub key: bool,
}

impl KeystrokeTag {
    pub fn key(pid: u32, epoch: u32) -> Self {
        Self {
            pid,
            epochs: vec![(epoch, 1)],
            edits: 0,
            key: true,
        }
    }

    /// Add one keystroke typed in `epoch`.
    pub fn push(&mut self, pid: u32, epoch: u32, edit: bool) {
        self.pid = pid;
        match self.epochs.last_mut() {
            Some((last, count)) if *last == epoch => *count += 1,
            _ => self.epochs.push((epoch, 1)),
        }
        if edit {
            self.edits += 1;
        }
    }

    pub fn keystrokes(&self) -> u64 {
        self.epochs.iter().map(|(_, n)| u64::from(*n)).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.epochs.is_empty()
    }

    pub fn kind(&self) -> KeystrokeKind {
        if self.key {
            KeystrokeKind::Key
        } else {
            KeystrokeKind::Text
        }
    }
}

/// Why admitted keystrokes never reached the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LostCause {
    /// Buffered text dropped by the hook's emission recheck (see the hook's
    /// fixed discard cause, reported separately).
    BufferDiscarded,
    /// A local queue between the hook and the recorder was full.
    QueueFull,
    /// The content worker refused it (privacy pause or shutdown).
    ContentWorker,
    /// The recorder's own exclusion rules matched (an intended block).
    RecorderExcluded,
    /// The recorder's configuration does not record this event type.
    RecorderFiltered,
    /// Discarded before database admission because privacy state changed.
    RecorderPrivacyPause,
    /// The database insert failed or the batch was dropped after failures.
    DatabaseFailure,
    /// The admitting keystroke produced no recordable effect.
    NoEffect,
    /// Text typed and fully deleted again before the buffer was emitted.
    NetZeroEdit,
}

pub const LOST_CAUSES: usize = 9;

impl LostCause {
    pub const ALL: [Self; LOST_CAUSES] = [
        Self::BufferDiscarded,
        Self::QueueFull,
        Self::ContentWorker,
        Self::RecorderExcluded,
        Self::RecorderFiltered,
        Self::RecorderPrivacyPause,
        Self::DatabaseFailure,
        Self::NoEffect,
        Self::NetZeroEdit,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::BufferDiscarded => "buffer_discarded",
            Self::QueueFull => "queue_full",
            Self::ContentWorker => "content_worker",
            Self::RecorderExcluded => "recorder_excluded",
            Self::RecorderFiltered => "recorder_filtered",
            Self::RecorderPrivacyPause => "recorder_privacy_pause",
            Self::DatabaseFailure => "database_failure",
            Self::NoEffect => "no_effect",
            Self::NetZeroEdit => "net_zero_edit",
        }
    }

    /// Blocked on purpose by the privacy configuration: not recordable.
    pub fn intended(self) -> bool {
        matches!(self, Self::RecorderExcluded)
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|cause| *cause == self)
            .unwrap_or(0)
    }
}

/// Post-admission dispositions of one epoch (or one process within it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dispositions {
    pub persisted_text: u64,
    pub persisted_edit: u64,
    pub persisted_key: u64,
    pub lost: [u64; LOST_CAUSES],
}

impl Dispositions {
    pub fn persisted(&self) -> u64 {
        self.persisted_text + self.persisted_edit + self.persisted_key
    }

    pub fn lost_unintended(&self) -> u64 {
        LostCause::ALL
            .into_iter()
            .filter(|cause| !cause.intended())
            .map(|cause| self.lost[cause.index()])
            .sum()
    }

    pub fn intended(&self) -> u64 {
        LostCause::ALL
            .into_iter()
            .filter(|cause| cause.intended())
            .map(|cause| self.lost[cause.index()])
            .sum()
    }

    pub fn add(&mut self, other: &Self) {
        self.persisted_text += other.persisted_text;
        self.persisted_edit += other.persisted_edit;
        self.persisted_key += other.persisted_key;
        for (total, value) in self.lost.iter_mut().zip(other.lost) {
            *total += value;
        }
    }

    pub fn lost_json(&self) -> serde_json::Value {
        serde_json::Value::Object(
            LostCause::ALL
                .into_iter()
                .filter(|cause| !cause.intended() && self.lost[cause.index()] > 0)
                .map(|cause| {
                    (
                        cause.code().to_string(),
                        serde_json::json!(self.lost[cause.index()]),
                    )
                })
                .collect(),
        )
    }
}

#[derive(Default)]
struct EpochLedger {
    totals: Dispositions,
    by_pid: HashMap<u32, Dispositions>,
}

#[derive(Default)]
struct Ledger {
    epochs: BTreeMap<u32, EpochLedger>,
    /// Dispositions that arrived after their epoch was reported.
    late: Dispositions,
    /// Epochs no longer at or before this one were reported.
    reported_through: Option<u32>,
}

/// A disposition noted while the ledger was busy.
struct Deferred {
    tag: KeystrokeTag,
    outcome: Outcome,
}

#[derive(Clone, Copy)]
enum Outcome {
    Persisted,
    Lost(LostCause),
}

struct Shared {
    ledger: Mutex<(Ledger, Receiver<Deferred>)>,
    deferred: Sender<Deferred>,
}

static SHARED: OnceLock<Shared> = OnceLock::new();
/// The current recording's start and run id (see `set_origin`).
static ORIGIN: Mutex<Option<(Instant, u64)>> = Mutex::new(None);

fn shared() -> &'static Shared {
    SHARED.get_or_init(|| {
        let (deferred, receiver) = channel();
        Shared {
            ledger: Mutex::new((Ledger::default(), receiver)),
            deferred,
        }
    })
}

/// Apply queued notes, then `f`.
fn locked<T>(guard: &mut (Ledger, Receiver<Deferred>), f: impl FnOnce(&mut Ledger) -> T) -> T {
    let (ledger, receiver) = guard;
    while let Ok(note) = receiver.try_recv() {
        apply(ledger, &note.tag, note.outcome);
    }
    f(ledger)
}

fn with_ledger<T>(f: impl FnOnce(&mut Ledger) -> T) -> T {
    let mut guard = shared()
        .ledger
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locked(&mut guard, f)
}

/// Called by the input hook at each recording start: epochs count from this
/// instant, and the run id (wall-clock milliseconds at that instant) separates
/// recordings. Dispositions still open from an earlier recording are dropped.
pub fn set_origin(start: Instant) {
    let wall = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .saturating_sub(start.elapsed());
    let mut origin = ORIGIN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if origin.is_some() {
        with_ledger(|ledger| *ledger = Ledger::default());
    }
    *origin = Some((start, wall.as_millis() as u64));
}

fn origin() -> Option<(Instant, u64)> {
    *ORIGIN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The run id and the wall-clock start (unix ms) of `epoch`.
pub fn epoch_start_unix_ms(epoch: u32) -> Option<(u64, u64)> {
    origin().map(|(_, run)| (run, run + u64::from(epoch) * EPOCH.as_millis() as u64))
}

pub fn epoch_at(start: Instant, at: Instant) -> u32 {
    (at.saturating_duration_since(start).as_millis() / EPOCH.as_millis()) as u32
}

pub fn current_epoch() -> Option<u32> {
    origin().map(|(start, _)| epoch_at(start, Instant::now()))
}

/// Note `outcome` without ever waiting for the ledger lock.
fn record(tag: &KeystrokeTag, outcome: Outcome) {
    if tag.is_empty() {
        return;
    }
    let shared = shared();
    // Applies only this note: queued notes are drained by `with_ledger`
    // (worker and tests), never here, so the hook's work stays bounded.
    match shared.ledger.try_lock() {
        Ok(mut guard) => apply(&mut guard.0, tag, outcome),
        Err(TryLockError::Poisoned(poisoned)) => apply(&mut poisoned.into_inner().0, tag, outcome),
        Err(TryLockError::WouldBlock) => {
            // Applied by the next report (`take_through`).
            let _ = shared.deferred.send(Deferred {
                tag: tag.clone(),
                outcome,
            });
        }
    }
}

fn apply(ledger: &mut Ledger, tag: &KeystrokeTag, outcome: Outcome) {
    let kind_key = tag.key;
    let add = |d: &mut Dispositions, count: u64, edits: u64| match outcome {
        Outcome::Persisted if kind_key => d.persisted_key += count,
        Outcome::Persisted => {
            d.persisted_edit += edits;
            d.persisted_text += count - edits;
        }
        Outcome::Lost(cause) => d.lost[cause.index()] += count,
    };
    let total = tag.keystrokes().max(1);
    // Edits are spread over the tag's epochs in proportion to keystrokes,
    // never more than an epoch's own keystrokes (the rest moves on).
    let mut edits_left = u64::from(tag.edits).min(tag.keystrokes());
    for (index, (epoch, count)) in tag.epochs.iter().enumerate() {
        let count = u64::from(*count);
        let share = if index + 1 == tag.epochs.len() {
            edits_left
        } else {
            u64::from(tag.edits) * count / total
        };
        let edits = share.min(edits_left).min(count);
        edits_left -= edits;
        if ledger.reported_through.is_some_and(|done| *epoch <= done) {
            add(&mut ledger.late, count, edits);
            continue;
        }
        if !ledger.epochs.contains_key(epoch) && ledger.epochs.len() >= MAX_OPEN_EPOCHS {
            add(&mut ledger.late, count, edits);
            continue;
        }
        let entry = ledger.epochs.entry(*epoch).or_default();
        add(&mut entry.totals, count, edits);
        let key = if entry.by_pid.contains_key(&tag.pid)
            || entry.by_pid.len() < MAX_PROCESSES_PER_EPOCH
        {
            tag.pid
        } else {
            0
        };
        add(entry.by_pid.entry(key).or_default(), count, edits);
    }
}

/// The database confirmed the insert of the event carrying `tag`.
pub fn note_persisted(tag: &KeystrokeTag) {
    record(tag, Outcome::Persisted);
}

/// The event carrying `tag` will never be persisted.
pub fn note_lost(tag: &KeystrokeTag, cause: LostCause) {
    record(tag, Outcome::Lost(cause));
}

/// One reported epoch's dispositions.
pub struct EpochDispositions {
    pub epoch: u32,
    pub totals: Dispositions,
    pub by_pid: HashMap<u32, Dispositions>,
}

/// Take the dispositions of every epoch at or before `through`; later notes
/// for those epochs go to the late total.
pub fn take_through(through: u32) -> Vec<EpochDispositions> {
    with_ledger(|ledger| {
        let rest = ledger.epochs.split_off(&(through.saturating_add(1)));
        let done = std::mem::replace(&mut ledger.epochs, rest);
        ledger.reported_through = Some(ledger.reported_through.map_or(through, |d| d.max(through)));
        done.into_iter()
            .map(|(epoch, entry)| EpochDispositions {
                epoch,
                totals: entry.totals,
                by_pid: entry.by_pid,
            })
            .collect()
    })
}

/// Dispositions that arrived after their epoch was reported (taken once).
pub fn take_late() -> Dispositions {
    with_ledger(|ledger| std::mem::take(&mut ledger.late))
}

#[cfg(test)]
pub(crate) fn reset_for_test() {
    with_ledger(|ledger| *ledger = Ledger::default());
}

#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_reconcile_per_epoch_process_and_kind_once() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_for_test();
        let mut text = KeystrokeTag::default();
        text.push(7, 3, false);
        text.push(7, 3, false);
        text.push(7, 4, true);
        assert_eq!(text.keystrokes(), 3);
        note_persisted(&text);
        note_persisted(&KeystrokeTag::key(7, 4));
        let mut lost = KeystrokeTag::default();
        lost.push(8, 4, false);
        note_lost(&lost, LostCause::BufferDiscarded);
        note_lost(&KeystrokeTag::key(8, 4), LostCause::RecorderExcluded);
        let done = take_through(4);
        assert_eq!(done.len(), 2);
        assert_eq!(done[0].totals.persisted_text, 2);
        let four = &done[1].totals;
        assert_eq!(
            (four.persisted_text, four.persisted_edit, four.persisted_key),
            (0, 1, 1)
        );
        assert_eq!(four.lost_unintended(), 1);
        assert_eq!(four.intended(), 1);
        assert_eq!(done[1].by_pid[&8].lost_unintended(), 1);
        assert_eq!(four.lost_json()["buffer_discarded"], serde_json::json!(1));
        // A note for an already reported epoch is kept as late, not lost.
        note_persisted(&KeystrokeTag::key(9, 2));
        assert!(take_through(4).is_empty());
        assert_eq!(take_late().persisted_key, 1);
        assert_eq!(take_late(), Dispositions::default());
    }

    #[test]
    fn edits_never_exceed_an_epochs_keystrokes() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_for_test();
        // Floors of 2*2/5 under-allocate the first two epochs.
        let tag = KeystrokeTag {
            pid: 4,
            epochs: vec![(1, 2), (2, 2), (3, 1)],
            edits: 2,
            key: false,
        };
        note_persisted(&tag);
        let done = take_through(3);
        let persisted: u64 = done.iter().map(|d| d.totals.persisted()).sum();
        assert_eq!(persisted, 5);
        let last = &done[2].totals;
        assert_eq!((last.persisted_text, last.persisted_edit), (0, 1));
    }

    #[test]
    fn a_note_while_the_ledger_is_busy_is_queued_not_waited_for() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_for_test();
        let held = shared().ledger.lock().unwrap();
        // Would deadlock if noting waited for the lock.
        note_lost(&KeystrokeTag::key(5, 1), LostCause::QueueFull);
        note_persisted(&KeystrokeTag::key(5, 1));
        drop(held);
        let done = take_through(1);
        assert_eq!(done[0].totals.lost_unintended(), 1);
        assert_eq!(done[0].totals.persisted_key, 1);
    }

    #[test]
    fn each_recording_start_restarts_epochs_and_drops_open_dispositions() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        reset_for_test();
        set_origin(Instant::now());
        note_persisted(&KeystrokeTag::key(5, 3));
        let _ = take_through(2);
        let later = Instant::now();
        set_origin(later);
        let (run, start) = epoch_start_unix_ms(0).unwrap();
        assert_eq!(run, start);
        assert_eq!(current_epoch(), Some(0));
        // Epoch 1 of the new recording is not "already reported".
        note_persisted(&KeystrokeTag::key(5, 1));
        let done = take_through(1);
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].totals.persisted_key, 1);
        assert_eq!(take_late(), Dispositions::default());
    }
}
