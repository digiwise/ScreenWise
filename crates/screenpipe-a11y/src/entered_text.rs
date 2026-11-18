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
/// A read window keeps at most this many lines; a line window reads this
/// many lines above and below the caret's ([`cut_window`]).
pub const MAX_READ_LINES: usize = 50;

/// What a read splits lines at (`\r\n` is one break).
const LINE_BREAKS: [char; 5] = ['\r', '\n', '\u{2028}', '\u{2029}', '\u{000B}'];
/// Those of [`LINE_BREAKS`] that end a paragraph (the others break a line
/// within one).
const PARAGRAPH_BREAKS: [char; 3] = ['\r', '\n', '\u{2029}'];

/// Read this long after the last edit key.
const PAUSE_DELAY: Duration = Duration::from_millis(300);
/// While typing goes on without a pause, read at most this often, so an edit
/// ended soon after its last key (Enter in a chat box or a terminal) still
/// has an "after".
const TYPING_READ_EVERY: Duration = Duration::from_secs(1);
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

/// A character an admitted edit key typed ([`EditSignals::note_char`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypedChar {
    /// The edit count with this key: its place among the edit keys.
    edit: u64,
    ch: char,
    /// The native focus it was typed into ([`CaretText::new`]'s `focus`).
    focus: u64,
}

/// Typed characters held for the tracker; more are dropped (the edits they
/// belong to then get none added).
const MAX_TYPED_CHARS: usize = 256;

/// Written by the input hooks: counters, and the characters keystroke
/// capture admitted for edit keys. Atomics and a preallocated lock-free
/// channel only: no locks or allocation, so noting never delays the input
/// queue.
#[derive(Debug)]
pub struct EditSignals {
    edits: AtomicU64,
    carets: AtomicU64,
    enters: AtomicU64,
    enter_edits: AtomicU64,
    typed: (
        crossbeam_channel::Sender<TypedChar>,
        crossbeam_channel::Receiver<TypedChar>,
    ),
}

impl Default for EditSignals {
    fn default() -> Self {
        Self {
            edits: AtomicU64::default(),
            carets: AtomicU64::default(),
            enters: AtomicU64::default(),
            enter_edits: AtomicU64::default(),
            typed: crossbeam_channel::bounded(MAX_TYPED_CHARS),
        }
    }
}

/// One reading of [`EditSignals`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignalCounts {
    pub edits: u64,
    pub carets: u64,
    pub enters: u64,
    /// The edit count when the latest Enter was pressed.
    pub enter_edits: u64,
}

impl EditSignals {
    pub fn note(&self, signal: Signal) {
        match signal {
            Signal::Edit => self.edits.fetch_add(1, Ordering::Relaxed),
            Signal::Caret => self.carets.fetch_add(1, Ordering::Relaxed),
            Signal::Enter => {
                // At the key: edit keys after it are the next edit's.
                self.enter_edits
                    .store(self.edits.load(Ordering::Relaxed), Ordering::Relaxed);
                self.enters.fetch_add(1, Ordering::Release)
            }
        };
    }

    pub fn counts(&self) -> SignalCounts {
        // An Enter seen comes with its own edit count.
        let enters = self.enters.load(Ordering::Acquire);
        SignalCounts {
            edits: self.edits.load(Ordering::Relaxed),
            carets: self.carets.load(Ordering::Relaxed),
            enters,
            enter_edits: self.enter_edits.load(Ordering::Relaxed),
        }
    }

    /// The character the edit key just noted typed into the native focus
    /// `focus`, once keystroke capture admitted it there.
    pub fn note_char(&self, ch: char, focus: u64) {
        let edit = self.edits.load(Ordering::Relaxed);
        let _ = self.typed.0.try_send(TypedChar { edit, ch, focus });
    }

    /// The characters noted since the last call.
    pub fn take_typed(&self) -> Vec<TypedChar> {
        self.typed.1.try_iter().collect()
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
    /// A line window through the text pattern, cut to the input at its
    /// prompt ([`cut_window`]).
    TextPatternLines,
    Value,
}

impl TextSource {
    pub fn code(self) -> &'static str {
        match self {
            Self::TextPattern => "text_pattern",
            Self::TextPatternLines => "text_pattern_lines",
            Self::Value => "value",
        }
    }
}

impl Serialize for TextSource {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

/// Where the caret is in a read's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaretAt {
    /// Byte offset of the caret (a selection's start) in the text.
    pub offset: usize,
    /// A selection, not a bare caret: typed characters cannot be placed.
    pub selection: bool,
}

/// The focused field's text at the caret, as one read returned it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaretText {
    /// Opaque identity of the field and native focus the text belongs to.
    pub target: u64,
    /// Opaque identity of the native focus alone, as typed characters carry
    /// it ([`EditSignals::note_char`]).
    focus: u64,
    lines: Vec<String>,
    truncated: bool,
    pub source: TextSource,
    /// The bare caret's line and byte column in `lines`, when known.
    caret: Option<(usize, usize)>,
    /// A line window's prompt ([`Prompt`]).
    prompt: Option<Prompt>,
    /// A line window with its caret right after its prompt and no input: the
    /// only kind that may be a "before".
    at_empty_prompt: bool,
    /// Edit counts when the read started and ended (set by the tracker).
    edits: (u64, u64),
    /// The app and window the text was read from (verified native focus).
    pub app_name: String,
    pub window_title: Option<String>,
}

impl CaretText {
    /// One read: `text` with the caret at `caret`. A line window
    /// ([`TextSource::TextPatternLines`]) is first cut to the input at its
    /// prompt ([`cut_window`]; `prompt` is the open edit's, if it has one),
    /// and is kept only around a known caret. Any other read with a known
    /// caret starts at the paragraph before the caret's
    /// ([`previous_paragraph`]). Then the read bounds apply
    /// ([`MAX_READ_BYTES`], [`MAX_READ_LINES`]); `truncated` reports that
    /// the reader already cut the text short.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target: u64,
        focus: u64,
        text: &str,
        caret: Option<CaretAt>,
        truncated: bool,
        source: TextSource,
        prompt: Option<&Prompt>,
        app_name: String,
        window_title: Option<String>,
    ) -> Self {
        let caret = caret.filter(|at| text.is_char_boundary(at.offset));
        let (text, caret, prompt, at_empty_prompt) = if source == TextSource::TextPatternLines {
            match caret.map(|at| (at.selection, cut_window(text, at.offset, prompt))) {
                Some((selection, window)) => (
                    window.text,
                    (!selection).then_some(window.caret),
                    window.prompt,
                    window.at_empty_prompt,
                ),
                None => (String::new(), None, None, false),
            }
        } else {
            let start = caret.map_or(0, |at| previous_paragraph(&text[..at.offset]));
            let caret = caret.filter(|at| !at.selection).map(|at| at.offset - start);
            (text[start..].to_string(), caret, None, false)
        };
        let mut end = text.len().min(MAX_READ_BYTES);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let mut truncated = truncated || end < text.len();
        let mut lines = lines_of(&text[..end]);
        if lines.len() > MAX_READ_LINES {
            lines.truncate(MAX_READ_LINES);
            truncated = true;
        }
        let caret = caret
            .filter(|&at| at <= end)
            .map(|at| locate(&text[..at]))
            .filter(|&(row, _)| row < lines.len());
        Self {
            target,
            focus,
            lines,
            truncated,
            source,
            caret,
            prompt,
            at_empty_prompt,
            edits: (0, 0),
            app_name,
            window_title,
        }
    }

    /// The field holds no visible text.
    fn is_blank(&self) -> bool {
        self.lines.iter().all(|line| line.trim().is_empty())
    }

    /// This read as the next edit's "before". A line window qualifies only
    /// with its caret right after its prompt and no input: otherwise (output
    /// still being written after Enter, a click into the output) its lines
    /// may be output, not entered text.
    fn into_baseline(self) -> Option<Self> {
        (self.source != TextSource::TextPatternLines || self.at_empty_prompt).then_some(self)
    }

    /// This read with the characters typed after it inserted at its caret
    /// (`typed`, for edit keys up to the count `edits`), and how many. Only
    /// when no edit key arrived while it was read, it has a bare caret, and
    /// every edit key since typed exactly one character into its native
    /// focus: a Backspace, Delete, paste or IME input among them (no
    /// character), or one typed elsewhere, adds nothing.
    fn with_typed(&self, typed: &[TypedChar], edits: u64) -> Option<(Self, u64)> {
        let (started, ended) = self.edits;
        let (row, column) = self.caret?;
        if started != ended || edits <= ended {
            return None;
        }
        let keys = (ended + 1..=edits)
            .map(|edit| {
                typed
                    .iter()
                    .find(|typed| typed.edit == edit && typed.focus == self.focus)
                    .map(|typed| typed.ch)
            })
            .collect::<Option<String>>()?;
        let mut text = self.clone();
        text.lines[row].insert_str(column, &keys);
        text.caret = Some((row, column + keys.len()));
        Some((text, edits - ended))
    }
}

/// `text` split into lines at every kind of line break, `\r\n` once.
fn lines_of(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .split(LINE_BREAKS)
        .map(str::to_string)
        .collect()
}

/// Where the paragraph before the one `head` (a text up to its caret) ends
/// in begins: the start of a paragraph read. The provider's read reaches one
/// paragraph further back when the caret is at the start of its paragraph
/// (moving back from a paragraph's start passes the previous one), so reads
/// before and after typing on a new line would otherwise cover different
/// paragraphs, and none of their lines would match.
fn previous_paragraph(head: &str) -> usize {
    let after_last_break = |text: &str| {
        text.rfind(PARAGRAPH_BREAKS).map_or(0, |at| {
            at + text[at..].chars().next().map_or(0, char::len_utf8)
        })
    };
    let own = after_last_break(head);
    if own == 0 {
        return 0;
    }
    // The text above the caret's paragraph, without the break ending it.
    let above = &head[..own];
    let above = match above.strip_suffix('\n') {
        Some(rest) => rest.strip_suffix('\r').unwrap_or(rest),
        None => &above[..own - above.chars().next_back().map_or(0, char::len_utf8)],
    };
    after_last_break(above)
}

/// The line and byte column, in [`lines_of`]'s lines, of the end of `head`
/// (a text up to its caret).
fn locate(head: &str) -> (usize, usize) {
    let lines = lines_of(head);
    (lines.len() - 1, lines[lines.len() - 1].len())
}

// ---------------------------------------------------------------------------
// Line windows
// ---------------------------------------------------------------------------

/// Most lines a paragraph read covering its whole field may hold: two short
/// paragraphs with a few soft line breaks. More means the provider's
/// "paragraph" is the whole field (a terminal's buffer).
const MAX_WHOLE_FIELD_PARAGRAPH_LINES: usize = 8;

/// Whether a paragraph read (`paragraphs`; `too_long`: over the read limit;
/// `whole_field`: it spans the element's whole text) is replaced by a line
/// window: it holds more than a read keeps, or it is the whole field and
/// more lines than two paragraphs hold. A field with real paragraphs keeps
/// paragraph reads, so a multi-paragraph selection is still read whole.
pub fn needs_line_window(paragraphs: &str, too_long: bool, whole_field: bool) -> bool {
    too_long || (whole_field && lines_of(paragraphs).len() > MAX_WHOLE_FIELD_PARAGRAPH_LINES)
}

/// What ends a prompt: the first of these before the caret on its line.
const PROMPT_MARKS: [char; 3] = ['>', '\u{276F}', '$'];

/// A terminal input's prompt, learnt from the caret's line where typing
/// starts: that line up to and including its first prompt mark before the
/// caret. Any shell's or chat app's prompt has this shape whatever precedes
/// the mark (`PS C:\Users\me>`, `C:\>`, `>`, `❯`, `me@host:~$`), and a
/// command may contain marks after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    text: String,
    /// The first non-blank row below the input while it was empty (a chat
    /// app's border), trimmed: the input's rows below the caret end above
    /// it. `None` (a shell): no row below the caret is the input's.
    footer: Option<String>,
}

/// A line window cut to the input at its prompt.
#[derive(Debug, PartialEq, Eq)]
struct Window {
    text: String,
    /// The caret's byte offset in `text`.
    caret: usize,
    prompt: Option<Prompt>,
    at_empty_prompt: bool,
}

/// Cut a line window to the input at its prompt. `text` holds up to
/// [`MAX_READ_LINES`] lines above the caret's, the caret's and up to as many
/// below, the caret at byte `caret`. With the caret right after a prompt and
/// no input, that prompt is learnt, with its footer; otherwise the open
/// edit's `prompt` is used, or failing that the caret line's own. The input
/// runs from the nearest line above the caret that starts with the prompt
/// (the caret's line when there is none) through the caret's line, and on
/// down to the row above the prompt's footer, when it has one and it is
/// there. Every line loses its trailing whitespace (a terminal pads its rows
/// to its width), but never text before the caret.
fn cut_window(text: &str, caret: usize, prompt: Option<&Prompt>) -> Window {
    let rows = lines_of(text);
    let (row, column) = locate(&text[..caret]);
    let line = &rows[row];
    let head = &line[..column];
    let own = head.find(PROMPT_MARKS).map(|mark| {
        let end = mark + head[mark..].chars().next().map_or(0, char::len_utf8);
        let empty = head[end..].trim().is_empty() && line[column..].trim().is_empty();
        (head[..end].to_string(), empty)
    });
    let at_empty_prompt = own.as_ref().is_some_and(|&(_, empty)| empty);
    let prompt = match own {
        Some((text, true)) => Some(Prompt {
            text,
            footer: rows[row + 1..]
                .iter()
                .map(|row| row.trim())
                .find(|row| !row.is_empty())
                .map(str::to_string),
        }),
        own => prompt
            .cloned()
            .or_else(|| own.map(|(text, _)| Prompt { text, footer: None })),
    };
    let start = prompt
        .as_ref()
        .and_then(|prompt| {
            (row.saturating_sub(MAX_READ_LINES - 1)..=row)
                .rev()
                .find(|&at| rows[at].starts_with(&prompt.text))
        })
        .unwrap_or(row);
    let below = &rows[row + 1..];
    let below = prompt
        .as_ref()
        .and_then(|prompt| prompt.footer.as_deref())
        .and_then(|footer| below.iter().position(|row| row.trim() == footer))
        .map_or(&below[..0], |end| &below[..end]);
    let mut kept: Vec<&str> = rows[start..row].iter().map(|row| row.trim_end()).collect();
    let caret = kept.iter().map(|row| row.len() + 1).sum::<usize>() + column;
    kept.push(&line[..line.trim_end().len().max(column)]);
    kept.extend(below.iter().map(|row| row.trim_end()));
    while kept.len() > row - start + 1 && kept.last().is_some_and(|row| row.is_empty()) {
        kept.pop();
    }
    Window {
        text: kept.join("\n"),
        caret,
        prompt,
        at_empty_prompt,
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
    /// Even the line window around the caret held more than a read takes.
    TooLarge,
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
            Self::TooLarge => "too_large",
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
    /// Of those, the keys whose characters were added at the caret from
    /// keystroke capture (all of them or none).
    pub keys_added: u64,
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
    /// When its first edit key was seen.
    pub started: Instant,
    /// When its last edit key (or the Enter that ended it) was seen.
    pub finished: Instant,
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

/// The edit being typed: its "before", its latest "after", the prompt its
/// line windows are cut at, and when its first and last keys were seen.
#[derive(Debug)]
struct OpenEdit {
    before: Option<CaretText>,
    after: Option<CaretText>,
    prompt: Option<Prompt>,
    started_at: Instant,
    last_edit_at: Instant,
}

#[derive(Clone, Copy, Debug)]
struct PendingRead {
    kind: ReadKind,
    /// The latest request (key) for this read.
    since: Instant,
    /// The first request since the last read of this kind.
    first: Instant,
}

/// The entered-text state machine (see the module documentation). It holds
/// at most one baseline read and one open edit.
#[derive(Debug, Default)]
pub struct EditTracker {
    seen: Option<SignalCounts>,
    pending: Option<PendingRead>,
    baseline: Option<CaretText>,
    open: Option<OpenEdit>,
    /// The edit count when the latest Enter was seen: edit keys after it
    /// belong to the next edit.
    enter_edits: Option<u64>,
    /// When the first and last edit keys after that Enter were seen, while
    /// its read was pending: the next edit, opened by that read.
    after_enter: Option<(Instant, Instant)>,
    /// Characters typed since the latest read started (bounded).
    typed: Vec<TypedChar>,
    /// How edits ended that were dropped for having no "after" (they ended
    /// before any read, and their keys could not be added to the "before"),
    /// since the last [`Self::take_lost`].
    lost: Vec<Ended>,
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
            // The settle read replaces a pending Enter read: keys typed
            // after that Enter are never read.
            if self.after_enter.take().is_some() {
                self.lost.push(Ended::Moved);
            }
            self.enter_edits = None;
            self.baseline = None;
            self.request(ReadKind::Settle, now);
        }
        if counts.edits != seen.edits && self.enter_pending() {
            let (_, last) = self.after_enter.get_or_insert((now, now));
            *last = now;
        } else if counts.edits != seen.edits {
            let open = self.open.get_or_insert_with(|| OpenEdit {
                before: None,
                after: None,
                prompt: None,
                started_at: now,
                last_edit_at: now,
            });
            if open.before.is_none() && open.after.is_none() {
                open.before = self.baseline.take();
                open.prompt = open
                    .before
                    .as_ref()
                    .and_then(|before| before.prompt.clone());
            }
            open.last_edit_at = now;
            self.request(ReadKind::Pause, now);
        }
        if counts.enters != seen.enters {
            // Keys after an earlier Enter whose read is still pending end
            // with this one, in the open edit (or as one, never read).
            if let Some((started_at, last_edit_at)) = self.after_enter.take() {
                self.open.get_or_insert(OpenEdit {
                    before: None,
                    after: None,
                    prompt: None,
                    started_at,
                    last_edit_at,
                });
            }
            self.enter_edits = Some(counts.enter_edits);
            if let Some(open) = &mut self.open {
                open.last_edit_at = now;
            }
            // Keys typed after the Enter key but seen in this poll.
            if counts.edits > counts.enter_edits {
                self.after_enter = Some((now, now));
            }
            self.request(ReadKind::Enter, now);
        }
        if self
            .open
            .as_ref()
            .is_some_and(|open| self.pending.is_none() && now - open.last_edit_at >= IDLE_END)
        {
            finished.extend(self.finish(Ended::Idle, counts.edits));
        }
        let due = self.pending.filter(|pending| {
            now - pending.since >= delay(pending.kind)
                || (pending.kind == ReadKind::Pause && now - pending.first >= TYPING_READ_EVERY)
        });
        (finished, due.map(|pending| pending.kind))
    }

    /// Take in the characters typed since the last call
    /// ([`EditSignals::take_typed`]).
    pub fn note_typed(&mut self, typed: Vec<TypedChar>) {
        self.typed.extend(typed);
        let excess = self.typed.len().saturating_sub(MAX_TYPED_CHARS);
        self.typed.drain(..excess);
    }

    /// The prompt the open edit's line windows are cut at, with the target it
    /// was learnt in: for its next read there ([`CaretText::new`]), never a
    /// read of another field.
    pub fn prompt(&self) -> Option<(u64, &Prompt)> {
        let open = self.open.as_ref()?;
        let known = open.after.as_ref().or(open.before.as_ref())?;
        Some((known.target, open.prompt.as_ref()?))
    }

    /// How the edits dropped since the last call ended (content-free
    /// diagnostics).
    pub fn take_lost(&mut self) -> Vec<Ended> {
        std::mem::take(&mut self.lost)
    }

    /// Report the outcome of the read [`Self::poll`] asked for, with the
    /// signal counts once it ended.
    pub fn read_done(
        &mut self,
        kind: ReadKind,
        outcome: ReadOutcome,
        counts: SignalCounts,
        now: Instant,
    ) -> Vec<FinishedEdit> {
        let edits = self.seen.unwrap_or_default().edits;
        let mut text = match outcome {
            ReadOutcome::NotNow => {
                if let Some(pending) = &mut self.pending {
                    if now - pending.since >= READ_GIVE_UP {
                        self.discard();
                    } else if pending.kind == ReadKind::Pause && now - pending.since < PAUSE_DELAY {
                        // A typing read that cannot start waits for the
                        // next one (or the pause read), not the next poll.
                        pending.first = now;
                    }
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
        text.edits = (edits, counts.edits);
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
                    prompt: text.prompt.clone(),
                    after: Some(text),
                    started_at: now,
                    last_edit_at: now,
                });
                self.typed.retain(|typed| typed.edit > edits);
                return finished;
            }
        }
        match kind {
            ReadKind::Settle => self.baseline = text.into_baseline(),
            ReadKind::Pause => match &mut self.open {
                Some(open) => {
                    if open.prompt.is_none() {
                        open.prompt = text.prompt.clone();
                    }
                    open.after = Some(text);
                }
                None => self.baseline = text.into_baseline(),
            },
            ReadKind::Enter => {
                let ended = if text.is_blank() {
                    Ended::Submitted
                } else {
                    Ended::Enter
                };
                let enter_edits = self.enter_edits.take().unwrap_or(edits);
                finished.extend(self.finish(ended, enter_edits));
                match self.after_enter.take() {
                    // Typing went on during the Enter read: the next edit,
                    // from its first key. The read may show some of it, so
                    // it has no "before"; a pause read gives its "after".
                    Some((started_at, last_edit_at)) => {
                        self.baseline = None;
                        self.open = Some(OpenEdit {
                            before: None,
                            after: None,
                            prompt: None,
                            started_at,
                            last_edit_at,
                        });
                        self.request(ReadKind::Pause, now);
                    }
                    None => self.baseline = text.into_baseline(),
                }
            }
        }
        // What this read shows needs no typed characters any more.
        self.typed.retain(|typed| typed.edit > edits);
        finished
    }

    /// Forget everything read and typed, without storing it: visual capture
    /// is paused (lock, schedule, DRM), or a read failed.
    pub fn discard(&mut self) {
        self.pending = None;
        self.baseline = None;
        self.open = None;
        self.enter_edits = None;
        self.after_enter = None;
        self.typed.clear();
    }

    /// Capture is stopping: store the open edit as last read.
    pub fn stop(&mut self) -> Vec<FinishedEdit> {
        let edits = self.seen.unwrap_or_default().edits;
        let finished = self.finish(Ended::Stopped, edits).into_iter().collect();
        self.discard();
        finished
    }

    /// An Enter's read is pending: edit keys now belong to the edit after
    /// it.
    fn enter_pending(&self) -> bool {
        self.pending
            .is_some_and(|pending| pending.kind == ReadKind::Enter)
    }

    fn request(&mut self, kind: ReadKind, now: Instant) {
        // A pending Enter read is not delayed by typing right after Enter:
        // it still runs 150 ms after the Enter.
        if kind == ReadKind::Pause && self.enter_pending() {
            return;
        }
        let first = self
            .pending
            .filter(|pending| pending.kind == kind)
            .map_or(now, |pending| pending.first);
        self.pending = Some(PendingRead {
            kind,
            since: now,
            first,
        });
    }

    /// End the open edit, if its "after" differs from its "before". Edit
    /// keys up to the count `edits` after its last read have their
    /// characters added at that read's caret when keystroke capture saw them
    /// all ([`CaretText::with_typed`]); with no read since the edit began,
    /// that read is its "before".
    fn finish(&mut self, ended: Ended, edits: u64) -> Option<FinishedEdit> {
        let open = self.open.take()?;
        if ended == Ended::Idle {
            self.baseline = open.after.clone().and_then(CaretText::into_baseline);
        }
        let Some(read) = open.after.as_ref().or(open.before.as_ref()) else {
            self.lost.push(ended);
            return None;
        };
        let keys_after_read = edits.saturating_sub(read.edits.0);
        // Keys after another field's read was seen are not this edit's.
        let added = (ended != Ended::FocusChanged)
            .then(|| read.with_typed(&self.typed, edits))
            .flatten();
        let (after, keys_added) = match (added, open.after) {
            (Some(added), _) => added,
            (None, Some(after)) => (after, 0),
            (None, None) => {
                self.lost.push(ended);
                return None;
            }
        };
        // A "before" from another field, or read another way (a paragraph
        // read of a terminal's whole buffer before it grew into line
        // windows), is not this edit's.
        let before = open
            .before
            .filter(|before| before.target == after.target && before.source == after.source);
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
                keys_after_read,
                keys_added,
            },
            app_name: after.app_name,
            window_title: after.window_title,
            started: open.started_at,
            finished: open.last_edit_at,
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

    /// Marks the caret in a test text.
    const CARET: char = '\u{2038}';

    /// `text` without its caret mark, and the caret there.
    fn with_caret(text: &str) -> (String, Option<CaretAt>) {
        let caret = text.find(CARET).map(|offset| CaretAt {
            offset,
            selection: false,
        });
        (text.replacen(CARET, "", 1), caret)
    }

    fn read(target: u64, text: &str) -> ReadOutcome {
        let (text, caret) = with_caret(text);
        ReadOutcome::Text(CaretText::new(
            target,
            0,
            &text,
            caret,
            false,
            TextSource::TextPattern,
            None,
            "editor.exe".to_string(),
            Some("Doc".to_string()),
        ))
    }

    /// A line window cut as the platform's read does, with `prompt`; the
    /// caret at its mark, or else at the end of its last line.
    fn window_with(target: u64, text: &str, prompt: Option<&Prompt>) -> ReadOutcome {
        let (text, caret) = with_caret(text);
        let caret = caret.or(Some(CaretAt {
            offset: text.trim_end_matches(['\r', '\n']).len(),
            selection: false,
        }));
        ReadOutcome::Text(CaretText::new(
            target,
            0,
            &text,
            caret,
            false,
            TextSource::TextPatternLines,
            prompt,
            "terminal.exe".to_string(),
            None,
        ))
    }

    /// A line window cut without an open edit's prompt.
    fn window(target: u64, text: &str) -> ReadOutcome {
        window_with(target, text, None)
    }

    /// [`cut_window`] of a text with its caret marked.
    fn cut(text: &str, prompt: Option<&Prompt>) -> Window {
        let (text, caret) = with_caret(text);
        cut_window(&text, caret.expect("a caret mark").offset, prompt)
    }

    fn prompt(text: &str, footer: Option<&str>) -> Prompt {
        Prompt {
            text: text.to_string(),
            footer: footer.map(str::to_string),
        }
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
                Signal::Enter => {
                    self.counts.enter_edits = self.counts.edits;
                    self.counts.enters += 1;
                }
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
            let finished = self
                .tracker
                .read_done(expected, outcome, self.counts, self.now);
            self.finished.extend(finished);
        }

        /// Type `text`: a key and its character each poll.
        fn type_chars(&mut self, text: &str) {
            for ch in text.chars() {
                self.type_char_unpolled(ch);
                self.tick(Duration::from_millis(25));
            }
        }

        /// One key typing `ch`, not polled yet.
        fn type_char_unpolled(&mut self, ch: char) {
            self.signal(Signal::Edit);
            let edit = self.counts.edits;
            self.tracker
                .note_typed(vec![TypedChar { edit, ch, focus: 0 }]);
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
    fn continuous_typing_is_read_each_second_so_a_quick_enter_keeps_it() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        let mut keys = 0;
        loop {
            sim.signal(Signal::Edit);
            keys += 1;
            if sim.tick(Duration::from_millis(25)) == Some(ReadKind::Pause) {
                break;
            }
            assert!(keys <= 41, "no read within a second of typing");
        }
        assert!(keys >= 40, "read before a second of typing");
        let finished = sim.tracker.read_done(
            ReadKind::Pause,
            read(FIELD, "hello wor"),
            sim.counts,
            sim.now,
        );
        assert!(finished.is_empty());
        sim.type_keys(2);
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, ""));
        let edit = &sim.finished[0];
        assert_eq!(edit.after, "hello wor");
        assert_eq!(edit.detail.ended, Ended::Submitted);
        assert_eq!(edit.detail.keys_after_read, 2);
        assert!(sim.tracker.take_lost().is_empty());
    }

    #[test]
    fn an_edit_ended_before_any_read_is_counted_as_lost() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.type_keys(3);
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, ""));
        assert!(sim.finished.is_empty());
        assert_eq!(sim.tracker.take_lost(), [Ended::Submitted]);
        assert!(sim.tracker.take_lost().is_empty());
    }

    #[test]
    fn a_line_window_keeps_the_input_from_its_prompt() {
        // PowerShell: the output above is dropped; a redirection's `>`
        // belongs to the command.
        let cut1 = cut(
            "PS C:\\> ls\r\nfile.txt\r\nPS C:\\> echo a > b.txt\u{2038}\r\n",
            None,
        );
        assert_eq!(cut1.text, "PS C:\\> echo a > b.txt");
        assert_eq!(cut1.prompt, Some(prompt("PS C:\\>", None)));
        assert!(!cut1.at_empty_prompt);
        // Any prompt ending in a mark, whatever precedes it.
        for (window, kept) in [
            ("out\nme@host:~$ ls -la\u{2038}", "me@host:~$ ls -la"),
            (
                "out\n~/repo \u{276F} git st\u{2038}",
                "~/repo \u{276F} git st",
            ),
            ("x\nC:\\Users>dir\u{2038}", "C:\\Users>dir"),
            (
                "answer\n│ > summarise this\u{2038} │",
                "│ > summarise this │",
            ),
        ] {
            assert_eq!(cut(window, None).text, kept, "{window}");
        }
        // A command over several lines, cut at the prompt learnt where
        // typing started (continuation lines start `>>`).
        let ps = prompt("PS C:\\x>", None);
        let window = "out\nPS C:\\x> if ($a) {\n>> b\n>> }\u{2038}";
        assert_eq!(
            cut(window, Some(&ps)).text,
            "PS C:\\x> if ($a) {\n>> b\n>> }"
        );
        // Without it, the caret line's own prompt.
        assert_eq!(cut(window, None).text, ">> }");
        // No prompt: only the caret's line, never the lines below it.
        assert_eq!(
            cut("line one\nline two\u{2038}\nline three", None).text,
            "line two"
        );
        assert_eq!(cut("a\rb\u{2028}c\u{2038}", None).text, "c");
        assert_eq!(cut("\u{2038}", None).text, "");
        // A prompt further up than the window's lines is not looked for.
        let far = format!(
            "PS C:\\> cmd\n{}last\u{2038}",
            "out\n".repeat(MAX_READ_LINES)
        );
        assert_eq!(cut(&far, Some(&prompt("PS C:\\>", None))).text, "last");
        // Rows padded to the terminal's width lose the padding, but not a
        // space typed before the caret.
        assert_eq!(
            cut("PS C:\\> ab \u{2038}     \r\n", None).text,
            "PS C:\\> ab "
        );
        // A shell's prompt has no footer: rows below its caret (a prediction
        // list) are never kept.
        let window = "PS C:\\> git p\u{2038}\r\n  git pull   [History]\r\n";
        assert_eq!(
            cut(window, Some(&prompt("PS C:\\>", None))).text,
            "PS C:\\> git p"
        );
    }

    #[test]
    fn an_empty_prompt_is_learnt_with_the_row_that_ends_its_input() {
        let empty = cut(
            "answer\r\n\u{276F}\u{a0}\u{2038}   \r\n──────\r\n  ? for shortcuts",
            None,
        );
        assert_eq!(empty.text, "\u{276F}\u{a0}");
        assert!(empty.at_empty_prompt);
        assert_eq!(empty.prompt, Some(prompt("\u{276F}", Some("──────"))));
        // A shell's empty prompt: nothing below it, so no footer.
        let shell = cut("out\r\nPS C:\\Users\\me> \u{2038}\r\n", None);
        assert_eq!(shell.prompt, Some(prompt("PS C:\\Users\\me>", None)));
        assert!(shell.at_empty_prompt);
        // A wrapped chat prompt with the caret inside it: its rows down to
        // the footer, and the caret's place in them.
        let chat = prompt("\u{276F}", Some("──────"));
        let wrapped = cut(
            "answer\r\n\u{276F}\u{a0}first row   \r\n  sec\u{2038}ond row  \r\n  third row \r\n──────\r\n  ? for shortcuts",
            Some(&chat),
        );
        assert_eq!(
            wrapped.text,
            "\u{276F}\u{a0}first row\n  second row\n  third row"
        );
        assert_eq!(
            &wrapped.text[..wrapped.caret],
            "\u{276F}\u{a0}first row\n  sec"
        );
        // A footer that is not there (the window was resized): nothing below.
        let gone = cut("\u{276F}\u{a0}a\u{2038}\r\n  b\r\n────────", Some(&chat));
        assert_eq!(gone.text, "\u{276F}\u{a0}a");
    }

    #[test]
    fn the_prompt_learnt_where_typing_starts_cuts_its_edit() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        let empty = "out\r\n\u{276F}\u{a0}\u{2038}\r\n──────\r\nstatus";
        sim.answer(ReadKind::Settle, window_with(FIELD, empty, None));
        sim.type_keys(1);
        let learnt = prompt("\u{276F}", Some("──────"));
        assert_eq!(sim.tracker.prompt(), Some((FIELD, &learnt)));
        // The caret back on the first of two rows: both are kept.
        let typed = "out\r\n\u{276F}\u{a0}a\u{2038}b\r\n  c\r\n──────\r\nstatus";
        let outcome = window_with(FIELD, typed, sim.tracker.prompt().map(|(_, p)| p));
        sim.answer(ReadKind::Pause, outcome);
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished[0].before.as_deref(), Some("\u{276F}\u{a0}"));
        assert_eq!(sim.finished[0].after, "\u{276F}\u{a0}ab\n  c");
    }

    #[test]
    fn keys_typed_after_the_last_read_are_added_at_its_caret() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.type_chars("hel");
        sim.answer(ReadKind::Pause, read(FIELD, "hel\u{2038}"));
        sim.type_chars("lo");
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, ""));
        let edit = &sim.finished[0];
        assert_eq!(edit.after, "hello");
        assert_eq!(edit.detail.ended, Ended::Submitted);
        assert_eq!(
            (edit.detail.keys_after_read, edit.detail.keys_added),
            (2, 2)
        );
    }

    #[test]
    fn keys_are_added_in_the_middle_and_to_an_edit_never_read() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "say \u{2038}there"));
        // Typed and sent before any read: its "before" with the keys.
        sim.type_chars("hi ");
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, ""));
        let edit = &sim.finished[0];
        assert_eq!(edit.before.as_deref(), Some("say there"));
        assert_eq!(edit.after, "say hi there");
        assert_eq!(edit.detail.keys_added, 3);
        assert!(sim.tracker.take_lost().is_empty());
    }

    #[test]
    fn keys_are_added_only_when_each_typed_one_character_there() {
        // A key without a character (Backspace) among them.
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.type_chars("hel");
        sim.answer(ReadKind::Pause, read(FIELD, "hel\u{2038}"));
        sim.type_keys(1);
        sim.type_chars("p");
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, ""));
        let edit = &sim.finished[0];
        assert_eq!(edit.after, "hel");
        assert_eq!(
            (edit.detail.keys_after_read, edit.detail.keys_added),
            (2, 0)
        );

        // A character typed into another native focus.
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.type_chars("ab");
        sim.answer(ReadKind::Pause, read(FIELD, "ab\u{2038}"));
        sim.signal(Signal::Edit);
        let edit = sim.counts.edits;
        sim.tracker.note_typed(vec![TypedChar {
            edit,
            ch: 'c',
            focus: 9,
        }]);
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, ""));
        assert_eq!(sim.finished[0].after, "ab");

        // A key that arrived while the read ran: it may already show.
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.type_chars("ab");
        while sim.tick(Duration::from_millis(25)) != Some(ReadKind::Pause) {}
        sim.type_char_unpolled('c');
        let finished = sim.tracker.read_done(
            ReadKind::Pause,
            read(FIELD, "abc\u{2038}"),
            sim.counts,
            sim.now,
        );
        assert!(finished.is_empty());
        sim.type_chars("d");
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, ""));
        assert_eq!(sim.finished[0].after, "abc");
        assert_eq!(sim.finished[0].detail.keys_added, 0);
    }

    #[test]
    fn keys_after_enter_belong_to_the_next_edit() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.type_chars("hi");
        sim.answer(ReadKind::Pause, read(FIELD, "hi\u{2038}"));
        sim.signal(Signal::Enter);
        sim.tick(Duration::from_millis(25));
        sim.type_chars("x");
        sim.answer(ReadKind::Enter, read(FIELD, "x\u{2038}"));
        assert_eq!(sim.finished[0].after, "hi");
        assert_eq!(sim.finished[0].detail.keys_after_read, 0);

        // A key typed after Enter but before the next poll: still the next
        // edit's, by the count taken at the Enter key.
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.type_chars("hi");
        sim.answer(ReadKind::Pause, read(FIELD, "hi\u{2038}"));
        sim.type_char_unpolled('!');
        sim.signal(Signal::Enter);
        sim.type_char_unpolled('x');
        sim.answer(ReadKind::Enter, read(FIELD, "x\u{2038}"));
        assert_eq!(sim.finished[0].after, "hi!");
        assert_eq!(sim.finished[0].detail.keys_added, 1);
    }

    #[test]
    fn keys_are_added_when_capture_stops_but_never_at_a_selection() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.type_chars("ab");
        sim.answer(ReadKind::Pause, read(FIELD, "ab\u{2038}"));
        sim.type_chars("c");
        let stopped = sim.tracker.stop();
        assert_eq!(stopped[0].after, "abc");

        // Typing replaces a selection: where the characters go is unknown.
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        let selected = CaretText::new(
            FIELD,
            0,
            "old text",
            Some(CaretAt {
                offset: 0,
                selection: true,
            }),
            false,
            TextSource::TextPattern,
            None,
            "editor.exe".to_string(),
            None,
        );
        sim.answer(ReadKind::Settle, ReadOutcome::Text(selected));
        sim.type_chars("new");
        let stopped = sim.tracker.stop();
        assert!(stopped.is_empty());
        assert_eq!(sim.tracker.take_lost(), [Ended::Stopped]);
    }

    #[test]
    fn the_open_edits_prompt_is_given_with_its_field() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, window(FIELD, "PS C:\\> \u{2038}"));
        sim.type_keys(1);
        let (target, learnt) = sim.tracker.prompt().expect("a prompt");
        assert_eq!((target, learnt), (FIELD, &prompt("PS C:\\>", None)));
    }

    #[test]
    fn only_a_whole_field_read_with_many_lines_needs_a_line_window() {
        let many = "row\n".repeat(MAX_WHOLE_FIELD_PARAGRAPH_LINES + 1);
        assert!(needs_line_window(&many, false, true));
        // A selection over many paragraphs of a longer document.
        assert!(!needs_line_window(&many, false, false));
        assert!(!needs_line_window("one\rtwo\r", false, true));
        assert!(needs_line_window("short", true, false));
    }

    #[test]
    fn a_before_read_another_way_is_not_kept() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        // A small terminal buffer, read whole as paragraphs.
        sim.answer(ReadKind::Settle, read(FIELD, "output\nPS C:\\> "));
        sim.type_keys(2);
        sim.answer(ReadKind::Pause, window(FIELD, "output\r\nPS C:\\> ab\r\n"));
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished[0].before, None);
        assert_eq!(sim.finished[0].after, "PS C:\\> ab");
    }

    #[test]
    fn an_idle_line_window_becomes_a_before_only_at_its_prompt() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, window(FIELD, "PS C:\\> \r\n"));
        sim.type_keys(1);
        // Type-ahead: output below the prompt.
        sim.answer(
            ReadKind::Pause,
            window(FIELD, "PS C:\\> build\r\nb\r\ncompiling\r\n"),
        );
        sim.tick(IDLE_END);
        assert_eq!(sim.finished.len(), 1);
        sim.type_keys(1);
        sim.answer(ReadKind::Pause, window(FIELD, "PS C:\\> x\r\n"));
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished[1].before, None);
    }

    #[test]
    fn typing_reads_come_a_second_apart_and_a_busy_provider_waits_for_the_next() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        // Polls (25 ms apart, a key each) until the next typing read is due.
        let next_read = |sim: &mut Sim| {
            (1..=80)
                .find(|_| {
                    sim.signal(Signal::Edit);
                    sim.tick(Duration::from_millis(25)) == Some(ReadKind::Pause)
                })
                .expect("a typing read")
        };
        assert!(next_read(&mut sim) >= 40);
        sim.tracker
            .read_done(ReadKind::Pause, read(FIELD, "a"), sim.counts, sim.now);
        assert!(next_read(&mut sim) >= 40);
        // The provider is busy: retried with the next typing read, not the
        // next poll.
        sim.tracker
            .read_done(ReadKind::Pause, ReadOutcome::NotNow, sim.counts, sim.now);
        assert!(next_read(&mut sim) >= 40);
        // After a pause the pause read is due at once.
        sim.tracker
            .read_done(ReadKind::Pause, ReadOutcome::NotNow, sim.counts, sim.now);
        assert_eq!(sim.tick(PAUSE_DELAY), Some(ReadKind::Pause));
    }

    #[test]
    fn a_line_window_is_a_before_only_with_the_caret_on_its_prompt() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        // Read while a command still writes output: the caret is in it.
        sim.answer(
            ReadKind::Settle,
            window(FIELD, "PS C:\\> build\r\ncompiling a\r\ncompiling b\r\n"),
        );
        sim.type_keys(3);
        sim.answer(
            ReadKind::Pause,
            window(FIELD, "PS C:\\> build\r\ndone\r\nPS C:\\> abc\r\n"),
        );
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished[0].before, None);
        assert_eq!(sim.finished[0].after, "PS C:\\> abc");
        assert_eq!(sim.finished[0].detail.source, TextSource::TextPatternLines);

        // Back on the command line after typed text: not an empty prompt.
        sim.answer(ReadKind::Settle, window(FIELD, "done\r\nPS C:\\> abc\r\n"));
        sim.type_keys(1);
        sim.answer(ReadKind::Pause, window(FIELD, "done\r\nPS C:\\> abcd\r\n"));
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished[1].before, None);
        assert_eq!(sim.finished[1].after, "PS C:\\> abcd");

        // At the empty prompt again.
        sim.answer(ReadKind::Settle, window(FIELD, "done\r\nPS C:\\> \r\n"));
        sim.type_keys(1);
        sim.answer(ReadKind::Pause, window(FIELD, "done\r\nPS C:\\> x\r\n"));
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        assert_eq!(sim.finished[2].before.as_deref(), Some("PS C:\\> "));
        assert_eq!(sim.finished[2].after, "PS C:\\> x");
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
        let finished =
            sim.tracker
                .read_done(ReadKind::Pause, ReadOutcome::NotNow, sim.counts, sim.now);
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
        let text = CaretText::new(
            FIELD,
            0,
            &long,
            None,
            false,
            TextSource::Value,
            None,
            String::new(),
            None,
        );
        assert!(text.truncated);
        assert!(text.lines.concat().len() <= MAX_READ_BYTES);
        let many = "x\n".repeat(MAX_READ_LINES * 2);
        let text = CaretText::new(
            FIELD,
            0,
            &many,
            None,
            false,
            TextSource::Value,
            None,
            String::new(),
            None,
        );
        assert!(text.truncated);
        assert_eq!(text.lines.len(), MAX_READ_LINES);
        let multibyte = "é".repeat(MAX_READ_BYTES);
        let text = CaretText::new(
            FIELD,
            0,
            &multibyte,
            None,
            false,
            TextSource::Value,
            None,
            String::new(),
            None,
        );
        assert!(text.truncated);
    }

    #[test]
    fn line_breaks_of_every_kind_split_lines_once() {
        let text = CaretText::new(
            FIELD,
            0,
            "a\r\nb\rc\nd\u{2029}e",
            None,
            false,
            TextSource::TextPattern,
            None,
            String::new(),
            None,
        );
        assert_eq!(text.lines, ["a", "b", "c", "d", "e"]);
        let text = CaretText::new(
            FIELD,
            0,
            "a\r\n\r\nb",
            None,
            false,
            TextSource::TextPattern,
            None,
            String::new(),
            None,
        );
        assert_eq!(text.lines, ["a", "", "b"]);
    }

    #[test]
    fn a_paragraph_read_starts_at_the_paragraph_before_the_carets() {
        let start = |text: &str| {
            let (text, caret) = with_caret(text);
            previous_paragraph(&text[..caret.expect("a caret mark").offset])
        };
        assert_eq!(start("one\ntwo\nthr\u{2038}ee"), 4);
        assert_eq!(start("one\ntwo\n\u{2038}"), 4);
        assert_eq!(start("one\r\ntwo\r\n\u{2038}three"), 5);
        assert_eq!(start("one\rtwo\r\u{2038}"), 4);
        assert_eq!(start("one\u{2029}two\u{2029}\u{2038}"), 6);
        // A soft line break is within its paragraph.
        assert_eq!(start("one\ntwo\u{2028}two b\n\u{2038}x"), 4);
        assert_eq!(start("one\n\u{2038}"), 0);
        assert_eq!(start("on\u{2038}e"), 0);
    }

    #[test]
    fn a_new_line_after_enter_is_stored_without_the_line_above() {
        // The provider's read at the start of a new line reaches two lines
        // back; the read while typing on it, one.
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "First\n\n\u{2038}"));
        sim.type_keys(3);
        sim.answer(ReadKind::Pause, read(FIELD, "\nNew\u{2038}"));
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, "\nNew\n\u{2038}"));
        let edit = &sim.finished[0];
        assert_eq!(edit.before.as_deref(), Some(""));
        assert_eq!(edit.after, "New");
        // The next line: the Enter read's "before" is the lines above it.
        sim.type_keys(4);
        sim.answer(ReadKind::Pause, read(FIELD, "New\nNext\u{2038}"));
        sim.signal(Signal::Enter);
        sim.answer(ReadKind::Enter, read(FIELD, "New\nNext\n\u{2038}"));
        let edit = &sim.finished[1];
        assert_eq!(edit.before.as_deref(), Some(""));
        assert_eq!(edit.after, "Next");
    }

    #[test]
    fn an_edit_spans_its_first_key_to_its_last_or_its_enter() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.signal(Signal::Edit);
        sim.tick(Duration::from_millis(25));
        let first = sim.now;
        sim.type_keys(2);
        let last = sim.now;
        sim.answer(ReadKind::Pause, read(FIELD, "abc"));
        sim.tick(IDLE_END);
        assert_eq!(sim.finished[0].detail.ended, Ended::Idle);
        assert_eq!(sim.finished[0].started, first);
        assert_eq!(sim.finished[0].finished, last);
        // Ended by Enter: keys typed after it are the next edit's.
        sim.type_keys(1);
        let first = sim.now;
        sim.answer(ReadKind::Pause, read(FIELD, "abcd"));
        sim.tick(Duration::from_secs(1));
        sim.signal(Signal::Enter);
        sim.tick(Duration::from_millis(25));
        let enter = sim.now;
        sim.type_keys(1);
        sim.answer(ReadKind::Enter, read(FIELD, "abcd\n"));
        assert_eq!(sim.finished[1].detail.ended, Ended::Enter);
        assert_eq!(sim.finished[1].started, first);
        assert_eq!(sim.finished[1].finished, enter);
    }

    #[test]
    fn keys_typed_while_enter_is_read_open_the_next_edit_from_its_first_key() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.type_keys(2);
        sim.answer(ReadKind::Pause, read(FIELD, "hi\u{2038}"));
        sim.signal(Signal::Enter);
        sim.tick(Duration::from_millis(25));
        sim.type_keys(1);
        let first = sim.now;
        sim.answer(ReadKind::Enter, read(FIELD, "x\u{2038}"));
        assert_eq!(sim.finished[0].after, "hi");
        sim.type_keys(1);
        let last = sim.now;
        sim.answer(ReadKind::Pause, read(FIELD, "xy\u{2038}"));
        sim.tick(IDLE_END);
        let edit = &sim.finished[1];
        assert_eq!(edit.after, "xy");
        assert_eq!(edit.before, None);
        assert_eq!((edit.started, edit.finished), (first, last));

        // With no edit open at the Enter, they are not dropped with it.
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.signal(Signal::Enter);
        sim.tick(Duration::from_millis(25));
        sim.type_keys(1);
        sim.answer(ReadKind::Enter, read(FIELD, "\nz\u{2038}"));
        sim.answer(ReadKind::Pause, read(FIELD, "\nz\u{2038}"));
        sim.tick(IDLE_END);
        assert_eq!(sim.finished.len(), 1);
        assert_eq!(sim.finished[0].after, "\nz");
        assert!(sim.tracker.take_lost().is_empty());
    }

    #[test]
    fn keys_between_two_quick_enters_are_stored_once() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, "\u{2038}"));
        sim.type_keys(2);
        sim.answer(ReadKind::Pause, read(FIELD, "hi\u{2038}"));
        sim.signal(Signal::Enter);
        sim.tick(Duration::from_millis(25));
        sim.type_keys(2);
        sim.signal(Signal::Enter);
        sim.tick(Duration::from_millis(25));
        sim.answer(ReadKind::Enter, read(FIELD, "hi\nwo\n\u{2038}"));
        assert_eq!(sim.tick(Duration::from_secs(1)), None);
        sim.tick(IDLE_END);
        assert_eq!(sim.finished.len(), 1);
    }

    #[test]
    fn a_move_that_replaces_an_enter_read_does_not_freeze_later_edits() {
        let mut sim = Sim::new();
        sim.signal(Signal::Caret);
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        sim.type_keys(2);
        sim.answer(ReadKind::Pause, read(FIELD, "hi"));
        sim.signal(Signal::Enter);
        sim.tick(Duration::from_millis(25));
        sim.signal(Signal::Caret); // before the Enter read
        sim.answer(ReadKind::Settle, read(FIELD, ""));
        // Continuous typing for longer than the idle end.
        sim.signal(Signal::Edit);
        sim.tick(Duration::from_millis(25));
        let first = sim.now;
        for _ in 0..60 {
            sim.signal(Signal::Edit);
            if let Some(kind) = sim.tick(Duration::from_millis(200)) {
                let finished =
                    sim.tracker
                        .read_done(kind, read(FIELD, "typing"), sim.counts, sim.now);
                sim.finished.extend(finished);
            }
        }
        let last = sim.now;
        assert_eq!(sim.finished.len(), 1, "no idle end while typing");
        sim.signal(Signal::Caret);
        sim.tick(Duration::from_millis(25));
        let edit = &sim.finished[1];
        assert_eq!((edit.started, edit.finished), (first, last));
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
