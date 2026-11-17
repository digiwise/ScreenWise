//! Entered-text capture: the platform-independent edit tracker.
//!
//! The input hooks count edit keys, caret moves and Enter presses
//! ([`EditSignals`]; atomics only). The platform's privacy worker polls an
//! [`EditTracker`] with those counts every few milliseconds; the tracker says
//! when to read the focused field's text at the caret ([`ReadKind`]), and
//! turns the reads into finished edits ([`FinishedEdit`]): the changed lines
//! before and after one edit, as displayed. See `docs/ENTERED_TEXT_CAPTURE.md`.
//!
//! Nothing here touches UI Automation, the hooks or the database, so the
//! whole lifecycle is unit-tested.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Serialize, Serializer};

/// A read window keeps at most this many bytes of text.
pub const MAX_READ_BYTES: usize = 4096;
/// A read window keeps at most this many lines.
pub const MAX_READ_LINES: usize = 16;

/// Read this long after the last edit key.
const PAUSE_DELAY: Duration = Duration::from_millis(300);
/// Read this long after Enter (time for a chat box to clear).
const ENTER_DELAY: Duration = Duration::from_millis(150);
/// Read this long after a caret key or click (time for it to settle).
const SETTLE_DELAY: Duration = Duration::from_millis(250);
/// An open edit with no edit key for this long is stored.
const IDLE_END: Duration = Duration::from_secs(10);
/// A read the platform could not attempt yet is given up after this long.
const READ_GIVE_UP: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// Hook signals
// ---------------------------------------------------------------------------

/// What a key or click means for entered text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Changes the text at the caret (characters, Backspace, Delete, paste,
    /// cut, undo, redo, IME input).
    Edit,
    /// Moves the caret, the selection or the focus (navigation keys, Tab,
    /// Ctrl+A, clicks).
    Caret,
    /// Enter, with any modifiers: submits or starts a new line.
    Enter,
}

/// Modifier bits as packed by the hooks (`Modifiers`).
const CTRL: u8 = 0x02;
const ALT_OR_WIN: u8 = 0x04 | 0x08;

/// Classify a key press (Windows virtual-key code and packed modifiers) for
/// entered text. Modifier keys alone and keys that neither edit nor move the
/// caret (function keys, Escape, shortcuts) give `None`.
pub fn classify_key(vk: u16, modifiers: u8) -> Option<Signal> {
    const BACKSPACE: u16 = 0x08;
    const TAB: u16 = 0x09;
    const ENTER: u16 = 0x0D;
    const DELETE: u16 = 0x2E;
    const IME_PROCESS: u16 = 0xE5;
    // Unicode characters sent as input (IMEs, text expanders, on-screen
    // keyboards).
    const PACKET: u16 = 0xE7;
    if vk == ENTER {
        return Some(Signal::Enter);
    }
    if modifiers & ALT_OR_WIN != 0 {
        return None;
    }
    let navigation = matches!(vk, 0x21..=0x28) || vk == TAB;
    if modifiers & CTRL != 0 {
        return match vk {
            // Ctrl+V/X/Z/Y paste, cut, undo, redo; Ctrl+Backspace/Delete
            // delete a word.
            0x56 | 0x58 | 0x5A | 0x59 | BACKSPACE | DELETE => Some(Signal::Edit),
            // Ctrl+A selects all; Ctrl+arrows/Home/End move by words.
            0x41 => Some(Signal::Caret),
            _ if navigation => Some(Signal::Caret),
            _ => None,
        };
    }
    // Shift only changes which character, or extends the selection.
    match vk {
        _ if navigation => Some(Signal::Caret),
        BACKSPACE | DELETE | IME_PROCESS | PACKET => Some(Signal::Edit),
        // Space, digits, letters, numpad and OEM punctuation.
        0x20 | 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2 => {
            Some(Signal::Edit)
        }
        _ => None,
    }
}

/// Counters written by the input hooks. Atomics only: no locks, allocation
/// or key identity, so noting a signal never delays the input queue.
#[derive(Debug, Default)]
pub struct EditSignals {
    edits: AtomicU64,
    carets: AtomicU64,
    enters: AtomicU64,
}

/// One reading of [`EditSignals`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignalCounts {
    pub edits: u64,
    pub carets: u64,
    pub enters: u64,
}

impl EditSignals {
    pub fn note(&self, signal: Signal) {
        let counter = match signal {
            Signal::Edit => &self.edits,
            Signal::Caret => &self.carets,
            Signal::Enter => &self.enters,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts(&self) -> SignalCounts {
        SignalCounts {
            edits: self.edits.load(Ordering::Relaxed),
            carets: self.carets.load(Ordering::Relaxed),
            enters: self.enters.load(Ordering::Relaxed),
        }
    }
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

/// Why the tracker wants a read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadKind {
    /// After a caret move or click settled: the next edit's "before".
    Settle,
    /// After a typing pause: the open edit's latest "after".
    Pause,
    /// After Enter: ends the open edit as submitted (field empty) or not,
    /// and becomes the next edit's "before". Never an edit's "after": in a
    /// terminal it already shows the command's output.
    Enter,
}

/// Where a read's text came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSource {
    TextPattern,
    Value,
}

impl TextSource {
    pub fn code(self) -> &'static str {
        match self {
            Self::TextPattern => "text_pattern",
            Self::Value => "value",
        }
    }
}

impl Serialize for TextSource {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

/// The focused field's text at the caret, as one read returned it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaretText {
    /// Opaque identity of the field and native focus the text belongs to.
    pub target: u64,
    lines: Vec<String>,
    truncated: bool,
    pub source: TextSource,
    /// The app and window the text was read from (verified native focus).
    pub app_name: String,
    pub window_title: Option<String>,
}

impl CaretText {
    /// Split `text` into lines and apply the read bounds
    /// ([`MAX_READ_BYTES`], [`MAX_READ_LINES`]). `truncated` reports that the
    /// reader already cut the text short.
    pub fn new(
        target: u64,
        text: &str,
        truncated: bool,
        source: TextSource,
        app_name: String,
        window_title: Option<String>,
    ) -> Self {
        let mut end = text.len().min(MAX_READ_BYTES);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let mut truncated = truncated || end < text.len();
        let mut lines: Vec<String> = text[..end]
            .replace("\r\n", "\n")
            .split(['\r', '\n', '\u{2028}', '\u{2029}', '\u{000B}'])
            .map(str::to_string)
            .collect();
        if lines.len() > MAX_READ_LINES {
            lines.truncate(MAX_READ_LINES);
            truncated = true;
        }
        Self {
            target,
            lines,
            truncated,
            source,
            app_name,
            window_title,
        }
    }

    /// The field holds no visible text.
    fn is_blank(&self) -> bool {
        self.lines.iter().all(|line| line.trim().is_empty())
    }
}

/// Why a read produced no text (fixed codes for content-free diagnostics).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadFailure {
    /// The focused element exposes no text at the caret.
    NoTextSource,
    /// The keyboard privacy decision did not allow a read.
    Denied,
    /// The decision relied on a relaxation not listed for reads.
    Relaxed,
    /// Focus, element or privacy generation changed during the read.
    Changed,
    /// The text provider did not answer within the read budget.
    Timeout,
    /// Any other provider error.
    Failed,
}

impl ReadFailure {
    pub fn code(self) -> &'static str {
        match self {
            Self::NoTextSource => "no_text_source",
            Self::Denied => "denied",
            Self::Relaxed => "relaxed",
            Self::Changed => "changed",
            Self::Timeout => "timeout",
            Self::Failed => "failed",
        }
    }
}

/// The platform's answer to a [`ReadKind`] request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadOutcome {
    Text(CaretText),
    Failed(ReadFailure),
    /// The read could not be attempted now (provider paced or busy); the
    /// tracker asks again later.
    NotNow,
}

// ---------------------------------------------------------------------------
// Finished edits
// ---------------------------------------------------------------------------

/// How an edit ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// The caret, selection or focus moved (key or click).
    Moved,
    /// Enter, and the field was empty afterwards (a sent message).
    Submitted,
    /// Enter, and the field still held text (a new line, or a sent message
    /// whose box had not cleared yet).
    Enter,
    /// A later read found another field focused.
    FocusChanged,
    /// No edit key for a while.
    Idle,
    /// Capture stopped.
    Stopped,
}

impl Ended {
    pub fn code(self) -> &'static str {
        match self {
            Self::Moved => "moved",
            Self::Submitted => "submitted",
            Self::Enter => "enter",
            Self::FocusChanged => "focus_changed",
            Self::Idle => "idle",
            Self::Stopped => "stopped",
        }
    }
}

impl Serialize for Ended {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

/// Fixed-code details stored with an edit (`ui_events.edit_detail`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct EditDetail {
    pub ended: Ended,
    pub source: TextSource,
    /// Whether "before" came from a read (`false`: the edit began before one).
    pub baseline: bool,
    /// A read window was cut to the bounds.
    pub truncated: bool,
    /// Edit keys pressed after the read that supplied "after" (their effect
    /// may be missing from it).
    pub keys_after_read: u64,
}

/// One edit, ready to be stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinishedEdit {
    /// The changed lines before the edit (`None`: no baseline read).
    pub before: Option<String>,
    /// The changed lines after the edit.
    pub after: String,
    pub detail: EditDetail,
    pub app_name: String,
    pub window_title: Option<String>,
}

impl FinishedEdit {
    pub fn detail_json(&self) -> String {
        serde_json::to_string(&self.detail).unwrap_or_default()
    }
}

/// Remove the lines `before` and `after` share at the start and at the end,
/// leaving only the changed lines.
fn changed_lines<'a>(before: &'a [String], after: &'a [String]) -> (&'a [String], &'a [String]) {
    let prefix = before.iter().zip(after).take_while(|(b, a)| b == a).count();
    let (before, after) = (&before[prefix..], &after[prefix..]);
    let suffix = before
        .iter()
        .rev()
        .zip(after.iter().rev())
        .take_while(|(b, a)| b == a)
        .count();
    (
        &before[..before.len() - suffix],
        &after[..after.len() - suffix],
    )
}

// ---------------------------------------------------------------------------
// Tracker
// ---------------------------------------------------------------------------

/// The edit being typed: its "before" and its latest "after".
#[derive(Debug)]
struct OpenEdit {
    before: Option<CaretText>,
    after: Option<CaretText>,
    /// Edit count when the latest "after" was read.
    edits_at_after: u64,
    last_edit_at: Instant,
}

#[derive(Clone, Copy, Debug)]
struct PendingRead {
    kind: ReadKind,
    since: Instant,
}

/// The entered-text state machine (see the module documentation). It holds
/// at most one baseline read and one open edit.
#[derive(Debug, Default)]
pub struct EditTracker {
    seen: Option<SignalCounts>,
    pending: Option<PendingRead>,
    baseline: Option<CaretText>,
    open: Option<OpenEdit>,
}

impl EditTracker {
    /// Take in the latest signal counts. Returns edits finished by them and
    /// the read due now, if any. The caller performs the read and reports it
    /// with [`Self::read_done`].
    pub fn poll(
        &mut self,
        counts: SignalCounts,
        now: Instant,
    ) -> (Vec<FinishedEdit>, Option<ReadKind>) {
        let mut finished = Vec::new();
        let seen = *self.seen.get_or_insert(counts);
        self.seen = Some(counts);
        // Signals seen in the same poll are taken as caret first, then
        // editing: typing right after a click or arrow key belongs to the
        // caret's new place.
        if counts.carets != seen.carets {
            // The edit stays as last read; the caret's new place is read
            // once it settles, as the next edit's "before".
            finished.extend(self.finish(Ended::Moved, seen.edits));
            self.baseline = None;
            self.request(ReadKind::Settle, now);
        }
        if counts.edits != seen.edits {
            let open = self.open.get_or_insert_with(|| OpenEdit {
                before: None,
                after: None,
                edits_at_after: seen.edits,
                last_edit_at: now,
            });
            if open.before.is_none() && open.after.is_none() {
                open.before = self.baseline.take();
            }
            open.last_edit_at = now;
            self.request(ReadKind::Pause, now);
        }
        if counts.enters != seen.enters {
            self.request(ReadKind::Enter, now);
        }
        if self
            .open
            .as_ref()
            .is_some_and(|open| self.pending.is_none() && now - open.last_edit_at >= IDLE_END)
        {
            finished.extend(self.finish(Ended::Idle, counts.edits));
        }
        let due = self
            .pending
            .filter(|pending| now - pending.since >= delay(pending.kind));
        (finished, due.map(|pending| pending.kind))
    }

    /// Report the outcome of the read [`Self::poll`] asked for.
    pub fn read_done(
        &mut self,
        kind: ReadKind,
        outcome: ReadOutcome,
        now: Instant,
    ) -> Vec<FinishedEdit> {
        let edits = self.seen.unwrap_or_default().edits;
        let text = match outcome {
            ReadOutcome::NotNow => {
                if self
                    .pending
                    .is_some_and(|pending| now - pending.since >= READ_GIVE_UP)
                {
                    self.discard();
                }
                return Vec::new();
            }
            // The open edit cannot be completed and the baseline may be
            // stale: both are dropped, nothing is stored.
            ReadOutcome::Failed(_) => {
                self.discard();
                return Vec::new();
            }
            ReadOutcome::Text(text) => text,
        };
        if self.pending.is_some_and(|pending| pending.kind == kind) {
            self.pending = None;
        }
        let mut finished = Vec::new();
        // A read of another field ends the open edit as last read there.
        let other_field = self
            .open
            .as_ref()
            .and_then(|open| open.after.as_ref().or(open.before.as_ref()))
            .is_some_and(|known| known.target != text.target);
        if other_field {
            finished.extend(self.finish(Ended::FocusChanged, edits));
            if kind == ReadKind::Pause {
                // Typing in the newly focused field: no "before" for it.
                self.open = Some(OpenEdit {
                    before: None,
                    after: Some(text),
                    edits_at_after: edits,
                    last_edit_at: now,
                });
                return finished;
            }
        }
        match kind {
            ReadKind::Settle => self.baseline = Some(text),
            ReadKind::Pause => match &mut self.open {
                Some(open) => {
                    open.after = Some(text);
                    open.edits_at_after = edits;
                }
                None => self.baseline = Some(text),
            },
            ReadKind::Enter => {
                let ended = if text.is_blank() {
                    Ended::Submitted
                } else {
                    Ended::Enter
                };
                finished.extend(self.finish(ended, edits));
                self.baseline = Some(text);
            }
        }
        finished
    }

    /// Forget everything read, without storing it: visual capture is paused
    /// (lock, schedule, DRM), or a read failed.
    pub fn discard(&mut self) {
        self.pending = None;
        self.baseline = None;
        self.open = None;
    }

    /// Capture is stopping: store the open edit as last read.
    pub fn stop(&mut self) -> Vec<FinishedEdit> {
        let edits = self.seen.unwrap_or_default().edits;
        let finished = self.finish(Ended::Stopped, edits).into_iter().collect();
        self.discard();
        finished
    }

    fn request(&mut self, kind: ReadKind, now: Instant) {
        // A pending Enter read is not delayed by typing right after Enter:
        // it still runs 150 ms after the Enter, and its read becomes the
        // "before" of what is typed next.
        if kind == ReadKind::Pause
            && self
                .pending
                .is_some_and(|pending| pending.kind == ReadKind::Enter)
        {
            return;
        }
        self.pending = Some(PendingRead { kind, since: now });
    }

    /// End the open edit, if it has an "after" that differs from its "before".
    fn finish(&mut self, ended: Ended, edits: u64) -> Option<FinishedEdit> {
        let open = self.open.take()?;
        let after = open.after?;
        if ended == Ended::Idle {
            self.baseline = Some(after.clone());
        }
        let before = open.before.filter(|before| before.target == after.target);
        let (before_lines, after_lines) = match &before {
            Some(before) => {
                let (b, a) = changed_lines(&before.lines, &after.lines);
                if b == a {
                    return None;
                }
                (Some(b), a)
            }
            None => (None, after.lines.as_slice()),
        };
        Some(FinishedEdit {
            before: before_lines.map(|lines| lines.join("\n")),
            after: after_lines.join("\n"),
            detail: EditDetail {
                ended,
                source: after.source,
                baseline: before.is_some(),
                truncated: after.truncated || before.as_ref().is_some_and(|b| b.truncated),
                keys_after_read: edits.saturating_sub(open.edits_at_after),
            },
            app_name: after.app_name,
            window_title: after.window_title,
        })
    }
}

fn delay(kind: ReadKind) -> Duration {
    match kind {
        ReadKind::Settle => SETTLE_DELAY,
        ReadKind::Pause => PAUSE_DELAY,
        ReadKind::Enter => ENTER_DELAY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIELD: u64 = 1;
    const OTHER: u64 = 2;
    const SHIFT: u8 = 0x01;
    const ALT: u8 = 0x04;

    fn read(target: u64, text: &str) -> ReadOutcome {
        ReadOutcome::Text(CaretText::new(
            target,
            text,
            false,
            TextSource::TextPattern,
            "editor.exe".to_string(),
            Some("Doc".to_string()),
        ))
    }

    /// Drives a tracker with simulated time and signals.
    struct Sim {
        tracker: EditTracker,
        counts: SignalCounts,
        now: Instant,
        finished: Vec<FinishedEdit>,
    }

    impl Sim {
        fn new() -> Self {
            let mut sim = Self {
                tracker: EditTracker::default(),
                counts: SignalCounts::default(),
                now: Instant::now(),
                finished: Vec::new(),
            };
            sim.tick(Duration::ZERO);
            sim
        }

        fn signal(&mut self, signal: Signal) {
            match signal {
                Signal::Edit => self.counts.edits += 1,
                Signal::Caret => self.counts.carets += 1,
                Signal::Enter => self.counts.enters += 1,
            }
        }

        /// Advance time and poll; returns the read due, if any.
        fn tick(&mut self, by: Duration) -> Option<ReadKind> {
            self.now += by;
            let (finished, due) = self.tracker.poll(self.counts, self.now);
            self.finished.extend(finished);
            due
        }

        /// Advance until a read is due, answer it with `outcome`.
        fn answer(&mut self, expected: ReadKind, outcome: ReadOutcome) {
            let mut due = None;
            for _ in 0..40 {
                due = self.tick(Duration::from_millis(25));
                if due.is_some() {
                    break;
                }
            }
            assert_eq!(due, Some(expected));
            let finished = self.tracker.read_done(expected, outcome, self.now);
            self.finished.extend(finished);
        }

        fn type_keys(&mut self, keys: usize) {
            for _ in 0..keys {
                self.signal(Signal::Edit);
                self.tick(Duration::from_millis(25));
            }
        }
    }

    #[test]
    fn a_submitted_prompt_is_stored_with_the_empty_box_before_it() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret); // click into the box
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.type_keys(5);
        sim.answer(ReadKind::Pause, read(FIELD, "hello"));
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, ""));
        assert_eq!(sim.finished.len(), 1);
        let edit = &sim.finished[0];
        assert_eq!(edit.before.as_deref(), Some(""));
        assert_eq!(edit.after, "hello");
        assert_eq!(edit.detail.ended, Ended::Submitted);
        assert!(edit.detail.baseline);
        assert_eq!(edit.detail.keys_after_read, 0);
    }

    #[test]
    fn deleting_a_mouse_selection_stores_the_paragraph_before_and_after() {
        let mut sim = Sim::new();
        let before = "Intro line\nThe quick brown fox jumps.";
        sim.signal(Signal::Caret); // mouse selection released
        sim.answer(ReadKind::Settle, read(FIELD, before));
        sim.type_keys(1); // Delete
        sim.answer(ReadKind::Pause, read(FIELD, "Intro line\nThe fox jumps."));
        sim.signal(Signal::Caret); // click elsewhere
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished.len(), 1);
        let edit = &sim.finished[0];
        assert_eq!(edit.before.as_deref(), Some("The quick brown fox jumps."));
        assert_eq!(edit.after, "The fox jumps.");
        assert_eq!(edit.detail.ended, Ended::Moved);
    }

    #[test]
    fn enter_ends_the_edit_as_last_read_while_typing() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "Title\nfirst"));
        sim.type_keys(4);
        sim.answer(ReadKind::Pause, read(FIELD, "Title\nfirst half"));
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, "first half\n"));
        assert_eq!(sim.finished.len(), 1);
        let edit = &sim.finished[0];
        assert_eq!(edit.detail.ended, Ended::Enter);
        assert_eq!(edit.before.as_deref(), Some("first"));
        assert_eq!(edit.after, "first half");
    }

    #[test]
    fn a_terminal_command_is_stored_and_its_output_is_not() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "old output\nPS> "));
        sim.type_keys(3);
        sim.answer(ReadKind::Pause, read(FIELD, "old output\nPS> cmd"));
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, "command output\nPS> "));
        assert_eq!(sim.finished.len(), 1);
        let edit = &sim.finished[0];
        assert_eq!(edit.before.as_deref(), Some("PS> "));
        assert_eq!(edit.after, "PS> cmd");
        assert!(!sim
            .finished
            .iter()
            .any(|edit| edit.after.contains("command output")
                || edit
                    .before
                    .as_deref()
                    .is_some_and(|b| b.contains("command output"))));
    }

    #[test]
    fn typing_right_after_enter_does_not_delay_the_enter_read() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.type_keys(2);
        sim.answer(ReadKind::Pause, read(FIELD, "hi"));
        sim.signal(Signal::Enter);
        sim.tick(Duration::ZERO); // the poll that sees the Enter
        sim.tick(Duration::from_millis(100));
        sim.signal(Signal::Edit);
        assert_eq!(sim.tick(Duration::from_millis(50)), Some(ReadKind::Enter));
    }

    #[test]
    fn enter_without_an_edit_stores_nothing() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, ""));
        assert!(sim.finished.is_empty());
    }

    #[test]
    fn typing_before_the_caret_settles_has_no_before() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "paragraph four"));
        sim.signal(Signal::Caret); // Down arrow
        sim.signal(Signal::Edit); // typed within the settle delay
        sim.tick(Duration::from_millis(25));
        sim.answer(ReadKind::Pause, read(FIELD, "paragraph five!"));
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        let edit = &sim.finished[0];
        assert_eq!(edit.before, None);
        assert!(!edit.detail.baseline);
        assert_eq!(edit.after, "paragraph five!");
    }

    #[test]
    fn a_read_of_another_field_ends_the_edit_as_last_read() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "a"));
        sim.type_keys(2);
        sim.answer(ReadKind::Pause, read(FIELD, "abc"));
        // Focus moved without a signal (another app); typing there.
        sim.type_keys(3);
        sim.answer(ReadKind::Pause, read(OTHER, "xyz"));
        assert_eq!(sim.finished.len(), 1);
        assert_eq!(sim.finished[0].after, "abc");
        assert_eq!(sim.finished[0].detail.ended, Ended::FocusChanged);
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        let second = &sim.finished[1];
        assert_eq!(second.before, None);
        assert_eq!(second.after, "xyz");
    }

    #[test]
    fn idle_editing_is_stored_and_becomes_the_next_baseline() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "one"));
        sim.type_keys(2);
        sim.answer(ReadKind::Pause, read(FIELD, "one two"));
        sim.tick(IDLE_END);
        assert_eq!(sim.finished.len(), 1);
        assert_eq!(sim.finished[0].detail.ended, Ended::Idle);
        sim.type_keys(1);
        sim.answer(ReadKind::Pause, read(FIELD, "one two!"));
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished[1].before.as_deref(), Some("one two"));
        assert_eq!(sim.finished[1].after, "one two!");
    }

    #[test]
    fn keys_after_the_last_read_are_counted() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.type_keys(3);
        sim.answer(ReadKind::Pause, read(FIELD, "abc"));
        sim.signal(Signal::Edit);
        sim.signal(Signal::Edit);
        sim.tick(Duration::from_millis(25));
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished[0].detail.keys_after_read, 2);
    }

    #[test]
    fn unchanged_text_and_failed_reads_store_nothing() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "same"));
        sim.type_keys(1);
        sim.answer(ReadKind::Pause, read(FIELD, "same"));
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "x"));
        sim.type_keys(1);
        sim.answer(ReadKind::Pause, ReadOutcome::Failed(ReadFailure::Denied));
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert!(sim.finished.is_empty());
    }

    #[test]
    fn a_read_that_cannot_start_is_retried_then_given_up() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.type_keys(1);
        sim.answer(ReadKind::Pause, ReadOutcome::NotNow);
        // Still due on the next poll.
        assert_eq!(sim.tick(Duration::from_millis(25)), Some(ReadKind::Pause));
        sim.answer(ReadKind::Pause, read(FIELD, "a"));
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished[0].after, "a");

        sim.type_keys(1);
        sim.tick(READ_GIVE_UP);
        let finished = sim
            .tracker
            .read_done(ReadKind::Pause, ReadOutcome::NotNow, sim.now);
        assert!(finished.is_empty());
        assert_eq!(sim.tick(Duration::from_millis(25)), None);
    }

    #[test]
    fn a_pause_discards_everything_read() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.type_keys(2);
        sim.answer(ReadKind::Pause, read(FIELD, "ab"));
        sim.tracker.discard();
        assert!(sim.tracker.stop().is_empty());
    }

    #[test]
    fn stopping_stores_the_open_edit() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.type_keys(2);
        sim.answer(ReadKind::Pause, read(FIELD, "ab"));
        let stopped = sim.tracker.stop();
        assert_eq!(stopped.len(), 1);
        assert_eq!(stopped[0].detail.ended, Ended::Stopped);
    }

    #[test]
    fn reads_are_bounded_and_flag_truncation() {
        let long = "word ".repeat(2000);
        let text = CaretText::new(FIELD, &long, false, TextSource::Value, String::new(), None);
        assert!(text.truncated);
        assert!(text.lines.concat().len() <= MAX_READ_BYTES);
        let many = "x\n".repeat(40);
        let text = CaretText::new(FIELD, &many, false, TextSource::Value, String::new(), None);
        assert!(text.truncated);
        assert_eq!(text.lines.len(), MAX_READ_LINES);
        let multibyte = "é".repeat(MAX_READ_BYTES);
        let text = CaretText::new(
            FIELD,
            &multibyte,
            false,
            TextSource::Value,
            String::new(),
            None,
        );
        assert!(text.truncated);
    }

    #[test]
    fn line_breaks_of_every_kind_split_lines_once() {
        let text = CaretText::new(
            FIELD,
            "a\r\nb\rc\nd\u{2029}e",
            false,
            TextSource::TextPattern,
            String::new(),
            None,
        );
        assert_eq!(text.lines, ["a", "b", "c", "d", "e"]);
        let text = CaretText::new(
            FIELD,
            "a\r\n\r\nb",
            false,
            TextSource::TextPattern,
            String::new(),
            None,
        );
        assert_eq!(text.lines, ["a", "", "b"]);
    }

    #[test]
    fn only_changed_lines_are_kept() {
        let lines = |s: &str| s.split('\n').map(str::to_string).collect::<Vec<_>>();
        let (b, a) = (lines("a\nb\nc"), lines("a\nB\nc"));
        assert_eq!(changed_lines(&b, &a), (&b[1..2], &a[1..2]));
        let (b, a) = (lines("a\nb"), lines("a\nb\nc"));
        let (cb, ca) = changed_lines(&b, &a);
        assert!(cb.is_empty());
        assert_eq!(ca, &a[2..]);
    }

    #[test]
    fn keys_are_classified_for_entered_text() {
        assert_eq!(classify_key(0x41, 0), Some(Signal::Edit));
        assert_eq!(classify_key(0x41, SHIFT), Some(Signal::Edit));
        assert_eq!(classify_key(0x20, 0), Some(Signal::Edit));
        assert_eq!(classify_key(0x08, 0), Some(Signal::Edit));
        assert_eq!(classify_key(0xE5, 0), Some(Signal::Edit));
        assert_eq!(classify_key(0xE7, 0), Some(Signal::Edit));
        assert_eq!(classify_key(0x56, CTRL), Some(Signal::Edit));
        assert_eq!(classify_key(0x43, CTRL), None); // copy
        assert_eq!(classify_key(0x41, CTRL), Some(Signal::Caret));
        assert_eq!(classify_key(0x25, 0), Some(Signal::Caret));
        assert_eq!(classify_key(0x25, SHIFT), Some(Signal::Caret));
        assert_eq!(classify_key(0x09, 0), Some(Signal::Caret));
        assert_eq!(classify_key(0x0D, SHIFT), Some(Signal::Enter));
        assert_eq!(classify_key(0x0D, CTRL), Some(Signal::Enter));
        assert_eq!(classify_key(0x41, ALT), None);
        assert_eq!(classify_key(0x1B, 0), None); // Escape
        assert_eq!(classify_key(0x70, 0), None); // F1
        assert_eq!(classify_key(0x10, 0), None); // Shift alone
    }
}
