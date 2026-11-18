# Entered-text capture

Status: phase 1 implemented (Windows), off by default behind the
`enteredTextCapture` setting. Not yet live-validated. Enabling it on a live
install is an owner privacy decision. The change touches the keyboard privacy
worker, admission timing and settings, so it takes the full review before any
deployment.

## Goal

Record what the user actually wrote, as it appears on screen, attributed to the
app and window: a submitted chat prompt, a search, a form field, a paragraph
written or changed in a document. Autocorrected, autocompleted and pasted text
is recorded as displayed. Edits are descriptive: each record holds the changed
lines at the caret **before** and **after** the edit, so a paragraph whose
selected words were deleted is recorded with those words in "before" and
without them in "after". "Before" therefore stores text the user then removed;
it was displayed, but the keystroke stream never stored such text.

It is a separate stream from the keystroke capture (`text` and `key` events);
whether to keep that stream is a later owner decision. The two meet in one
place: characters keystroke capture admitted are added to an edit when no
read saw them (see the model). Keystroke `text` events record the app of the
native focus their characters were verified under (the same app name entered
text records), resolved off the hook thread by the input content worker.

## Non-goals (phase 1)

- Apps with no UIA text on the focused element (terminals
  without a UIA text provider, canvas-drawn editors). Phase 2 adds an OCR
  source. Windows Terminal and the console host do expose UIA text and are
  read like any other field.
- Text typed and deleted between reads, exact keystroke timing.
- Reading files, shell history or other apps' storage.

## Model

One rule for every app: **on a pause or a boundary, read the focused field's
text at the caret; while the user keeps editing, update one open edit; when
they move on, store it.**

- A **read** returns, from the focused element only, the text from the start
  of the paragraph before the caret's (or the selection's) to the end of the
  caret's (or the selection's last) paragraph, clamped to the element's own
  text. A single-line field is one paragraph, so a read of a chat or search
  box is the whole field. The text is kept as lines. With the caret at the
  start of a paragraph (a new line after Enter), a provider's paragraph
  moves reach one paragraph further back, so the text read is cut to start
  at the paragraph before the caret's: otherwise the reads before and after
  typing on a new line cover different paragraphs, and the line above is
  stored as if it had been replaced.
- An edit's **span** runs from its first edit key to its last, or to the
  Enter that ended it, as the tracker saw them. Keys typed after that Enter
  while it is read open the next edit, from the first of them, with no
  "before" (the Enter read may already show some of them); a caret move
  before the Enter read drops them as lost.
- A **line window** replaces that read when it is no paragraph read: it
  spans the whole field and holds more than 8 lines (a provider without
  paragraphs, such as a terminal, whose "paragraph" is its whole buffer), or
  it holds more than a read keeps (4,096 UTF-16 units: one very long
  paragraph or selection). A multi-paragraph selection in a longer document
  is still a paragraph read.
  It reads up to 50 lines before the caret's line, that line and up to 50
  after it, with the caret's place in them, and keeps the **input at its
  prompt**. The prompt is learnt from the caret's line where typing starts:
  that line up to and including its first `>`, `❯` or `$` before the caret,
  whatever precedes the mark (`PS C:\Users\me>`, `C:\>`, `>`, `❯`,
  `~/repo ❯`, `me@host:~$`), so a command may contain marks after it. It is
  learnt from the read with the caret right after a prompt and no input (the
  edit's "before"), or failing that the latest one learnt that way in the
  same field (so an edit begun with the caret inside a wrapped input, after a
  caret move, still keeps all its rows), or the edit's first read, or the
  read's own caret line. That remembered prompt (one, in memory only) is
  forgotten at every Enter (the field may now run another program) and
  learnt again by a read at an empty prompt, and forgotten when capture
  pauses, a read fails or is given up, or capture stops. It is never used
  for a caret line with a prompt mark of its own before the caret that
  differs from it (another program's, such as the shell after a chat app
  exits on Ctrl+D, but also a wrapped row holding `>`, `$` or `->` before
  the caret, which then keeps only that row). An edit that starts from it
  keeps that rule for all its reads. The input runs from the nearest line
  above the caret that starts with the prompt (within 50 lines; else the caret's line)
  through the caret's line, so a wrapped or pasted command over several
  lines or a long pasted prompt is kept and the output above it is not. A
  prompt, the open edit's or remembered, is only used for reads of the field
  it was learnt in. Rows below the caret are kept only down to the prompt's
  **footer**: the first non-blank row below the empty input when the prompt
  was learnt (a chat app's border); a shell normally
  has none, so rows a terminal draws below its caret (a prediction list) are
  never kept, and when the footer is gone (the window was resized) none are.
  A multiplexer's status row below an empty shell prompt (tmux, zellij)
  becomes its footer, and rows above it are then kept. Every
  kept line loses its trailing whitespace (terminals pad rows to their
  width), but never text before the caret. A line window is a "before" only
  with its caret right after its prompt and no input, never while output may
  still be below a prompt; and a "before" read another way (a paragraph read
  of a small terminal buffer) is never paired with a line window's "after".
  A window without a known caret is not kept. Source code
  `text_pattern_lines`; a window over 64 K UTF-16 units is discarded
  (`too_large`).
- A read also returns where its **caret** is, by the text from the caret to
  the end of the range read (a selection's start, flagged as a selection).
- Reads are bound to an **identity**: the verified UIA element (runtime id
  hash) and the native focus (foreground window, focus window, processes).
- The **baseline** is the latest read taken while no edit was open and after
  the last caret move or click settled. A caret key or click discards it until
  its settle read.
- An **open edit** starts at the first edit key. Its "before" is the
  baseline (none when the edit began before a baseline existed: `baseline:
  false`). Each later pause or typing read replaces its "after".
- An edit **ends**:
  - at a caret key or click (`moved`), with its last pause read as "after";
  - after Enter, with its last pause read as "after": `submitted` if the
    read 150 ms after Enter finds the field empty, otherwise `enter`. The Enter
    read only becomes the next baseline, never an edit's "after": in a terminal
    it already shows the command's output. Typing right after Enter does not
    delay that read;
  - when a read returns another identity (`focus_changed`), with its last read
    there; typing in the new field opens an edit without "before";
  - after 10 s without an edit key (`idle`); its "after" becomes the baseline;
  - when capture stops (`stopped`), with its last read.
- A read that fails discards the open edit and the baseline (counted). A read
  that cannot start (provider paced or busy, no decision this time) is retried
  on the next poll, or with the next typing read while typing goes on, and
  given up 2 s after the last key or caret move that asked for it.
- **Keys typed after the last read** are added at its caret from keystroke
  capture: the hook queues each character it admitted for an edit key, with
  that key's place among the edit keys and the native focus it was typed
  into. When an edit ends, the characters of the edit keys after its last
  read are inserted at that read's caret, only when no edit key arrived while
  the read ran, the caret was bare (no selection) with nothing but
  whitespace after it on its line, and every one of those
  keys typed exactly one character into the read's native focus; a
  Backspace, Delete, paste, `VK_PROCESSKEY` or `VK_PACKET` key among them, a
  character typed elsewhere, or one the queue dropped adds nothing. The hook
  sees raw keys, so keys composed by an IME or a dead key are added as the
  plain characters keystroke capture maps them to. Keys after Enter (by the
  edit count taken at the Enter key) belong to the next edit, and an edit
  ended by another field's read gets none. Text after the caret may be an
  app's inline suggestion (ghost text) that the keys overtype rather than
  push along, so keys are never added in front of it; that edit keeps its
  last read.
  `keys_added` records how many were added (all or none of
  `keys_after_read`).
- An edit is stored only if its "after" differs from its "before". With no
  read since it began, its "after" is its "before" with the keys typed since
  added as above; one that cannot be completed that way is dropped and
  counted as `lost`. Stored "before"/"after" keep only the lines that differ
  (the common leading and trailing lines are removed).
- Signals seen in the same 25 ms poll are taken as caret first, then editing.

Reads happen 300 ms after the last edit key, 150 ms after Enter and 250 ms
after a caret key or click, and once a second while typing goes on without a
pause (so a prompt sent right after its last key still has an "after"). None
happen while the user is not typing or moving the caret.

### Worked examples (unit-tested)

| Action | before | after | ended |
|---|---|---|---|
| Click into a chat box, type a prompt, pause, press Enter, box clears | the empty box | the prompt | submitted |
| Select words with the mouse, press Delete, pause, click elsewhere | the paragraph with the words | the paragraph without them | moved |
| Type a command in a terminal, pause, press Enter | the empty prompt line | the prompt line with the command (not its output) | enter |
| Type, pause, type two more characters and press Enter at once | the empty box | the prompt with the last two characters added from keystroke capture | submitted |
| Type in a paragraph, pause, press Enter | the paragraph | the paragraph as typed (the split shows in the next edit's "before") | enter |
| Type, pause, then switch to another app and type there | first field's text | first field's text as last read | focus_changed |

## Components

1. **Hook signals** (`platform/windows.rs`, hook thread). The low-level hooks
   only increment atomic counters, after the existing pause gate: edit keys
   (characters, IME `VK_PROCESSKEY`, Unicode `VK_PACKET`, Backspace, Delete,
   Ctrl+V/X/Z/Y and
   Ctrl+Backspace/Delete), caret keys (navigation keys, Tab, Ctrl+A,
   Ctrl+navigation) and left-button release, and Enter (any modifiers).
   `entered_text::classify_key` makes this classification; it answers a
   different question from the hook's admission classifications (what changes
   or moves the text, not what may move focus), so it is a separate pure
   function. Counters carry no key identity; a held key's auto-repeat counts
   each repeat. For an edit key whose character keystroke capture admitted
   into its text buffer, the hook also queues that character, its edit count
   and a hash of the native focus on a preallocated lock-free channel
   (`EditSignals::note_char`; 256 at most, more are dropped).
2. **Edit tracker** (`entered_text.rs`, platform independent, pure). Consumes
   counter snapshots and read results, produces read requests and finished
   edits. Owns timing, identity binding, the edit lifecycle, line trimming
   and bounds. Driven by the keyboard privacy worker's 25 ms loop; no timers
   of its own.
3. **Caret text read, right after a probe** (`windows_uia.rs`,
   `UiaContext::read_entered_text`; UIA text code in
   `platform/windows_text.rs`). The probe publishes its decision first (so
   keystroke admission is not delayed), then hands the tracker the element it
   just verified. The read uses that live element and repeats the probe's own
   focus recheck (`UiaContext::recheck_focus`, the one implementation both
   use). Only with `enteredTextCapture` on does the probe also cache which
   text patterns the element supports (two Booleans); with it off the probe
   is unchanged.
4. **Event and storage.** Input event type `text_edit`: `text_content` = after;
   nullable columns `text_before`, `edit_detail` (fixed-code JSON: `ended`,
   `source`, `baseline`, `truncated`, `keys_after_read`, `keys_added`) and
   `text_before_redacted_at` (migration `20261116000000`), and `started_at`
   (migration `20261118000000`): the edit's span runs from `started_at` to
   `timestamp`. `started_at` is set for every edit stored since that migration
   and null for other event types and for earlier edits. App name and window
   title come from the verified native focus. `text_before` is deliberately not
   full-text indexed. `/search?content_type=input` returns `text_before`,
   `edit_detail` and `started_at`.
5. **Write-time screening** (`screenpipe-engine/src/input_screening.rs`). At
   the recorder's persistence boundary, every input event is screened with
   `screenpipe_redact::rescreen::decide_ui_event` before Basic PII removal:
   window rules delete, secrets are redacted, unredactable content deletes
   (counted as an intended block). `privacy rescreen` uses the same module and
   the same rule inputs (`input_screening::RuleInputs::from_settings`).
   Events that carry no text, title, URL or app are not screened.
   `text_before` is one of
   the screened fields, and the background redaction worker covers `text_edit`
   rows and `text_before`.
6. **Settings.** `enteredTextCapture` (default false) maps to
   `UiCaptureConfig::capture_entered_text` and the recorder's
   `capture_entered_text`, independent of `disableKeyboardCapture`. No
   settings UI in phase 1. A local deployment turns it on with
   `Deploy-ScreenWise.ps1 -EnteredTextCapture` (recorded in the manifest,
   passed to the GUI at startup as `SCREENWISE_ENTERED_TEXT_CAPTURE=1`, which
   adds to the setting and can never turn it off).
   `SCREENWISE_ENTERED_TEXT_RELAXED_PROCESSES` is set through the deployment
   path too (`-EnteredTextRelaxedProcesses`).

## Privacy rules

- **The read is under the probe's decision.** The probe verifies the focused
  element (explicit non-password state, keyboard focus or the owner's focus
  relaxation, owning process, not excluded, native focus unchanged, same
  element on recheck). The read uses that element; it has no admission logic
  of its own.
- **Acceptance rule for a read**, all required: visual capture allowed, and
  the keyboard privacy generation and native focus unchanged, from the start
  of the probe until after the read (pending messages are dispatched first:
  the focus-change handler is an apartment object whose calls arrive through
  this thread's queue, so this is when focus events are counted); the focus
  recheck after the read finds the same element; the read finished within its
  150 ms budget; the foreground and focus windows still may be contacted.
  Otherwise the text is discarded. The 125 ms keystroke age is not used for
  reads.
- **Only the focused element's text.** The text pattern is the verified
  element's own (never an ancestor's, descendant's or a search result's). The
  caret or selection must lie inside the pattern's document range (the
  element's own text) or nothing is read; a caret Chromium places just
  outside a field's edge (the same visible place in a neighbouring node, as
  in VS Code webview inputs) is moved onto that edge only when no text lies
  between them. The range read is clamped to
  it: Chromium, WebView2 and Electron move range endpoints through the whole
  page, and the clamp keeps a chat box's read out of the conversation around
  it. `GetText` is called with the size limit. The `Value` fallback is used
  only for `Edit` controls under an unrelaxed decision. In multi-line editors
  the paragraph before the caret can hold text the user did not type; in a
  terminal the line window keeps only the input from its prompt, but text
  below the prompt above the caret (output of a command still running while
  the user types ahead) can remain, and an output line that starts with the
  prompt's text (a bare `>` prompt and a quoted text, or a listing of earlier
  commands) within 50 lines above the caret is taken as the prompt, so the
  output after it is kept as "after". Rows below the caret are read but only
  kept above a footer learnt at an empty prompt. A small terminal buffer
  (8 lines or fewer) is still read whole.
- **Typed characters** reach entered text only as keystroke capture admitted
  them (under the same keyboard privacy decision, never while paused), and
  only into an edit read in the same native focus. Characters wait in a
  bounded queue and in the tracker only until a read shows them or the edit
  ends; a pause, a failed read or a stop discards them.
- **Relaxed decisions read only for listed processes** (owner decision 1).
- **Password fields** are never read. A provider can expose a value the screen
  masks (styled PIN fields), so the explicit `IsPassword` gate is the defence,
  not the display.
- **Excluded apps and windows** are never contacted: the probe skips them
  before any UIA call, and the read checks the windows again afterwards.
- **Pause, lock, DRM, schedule**: no reads; the baseline and open edit are
  discarded, never stored later.
- **Before-write screening** (component 5), then Basic PII removal.
- **Bounds**: 4,096 bytes and 50 lines per read window, truncation flagged
  (a line window is cut to its prompt first);
  at most one baseline and one open edit in memory.
- **Content-free logs**: counts and fixed codes only.

## Robustness

- Hooks touch atomics and a preallocated lock-free channel only; UIA work
  stays on the keyboard privacy worker. The app of keystroke text is
  resolved by the input content worker, not the hook.
- A read makes at most eighteen provider requests (six more for each
  selection end moved onto the field's edge, seven more for a line window)
  plus the two of the focus recheck, within a 150 ms budget, and has
  its own per-process breaker scope
  and pacing gap (`uia_breaker::Scope::Read`, the probe's 1 s to 30 s
  schedule), so a slow text provider paces and stops reads, never probes.
  Reads are skipped while a process's probes or reads are paced, its read
  breaker is open or a walk to it is running. The worker makes no probe while
  a read runs, so a keystroke typed during a slow read can still find its
  decision stale (at most the 150 ms budget).
- A hung provider call hangs the worker, and keystroke admission with it, as a
  hung probe does today.
- Events use the existing bounded UI event channel and drop accounting.
- No new threads, timers or COM apartments.

## Diagnostics

The worker logs `entered_text_summary` at most once a minute: reads by outcome
(`text_pattern`, `text_pattern_lines`, `value`, `no_text_source`, `denied`,
`relaxed`, `changed`, `timeout`, `too_large`, `failed`, `not_now`), emitted
edits by `ended` (the recorder can still refuse them), and `lost` edits (ended
before any read, with no keys that could be added to a "before") by `ended`. Read latency and
per-process counts are not recorded yet. `breaker_open_read_processes` is in
the UIA breaker metrics.

## Tests

- Tracker (`entered_text::tests`): the worked examples, a caret move before
  the settle read, another field, idle and stop endings, keys after the last
  read, failed and retried reads, pause discard, bounds and line breaks, key
  classification, the typing read and a quick Enter, lost edits, line
  windows (PowerShell with redirection, `$`, `path ❯`, cmd and boxed
  prompts, a multi-line command at the learnt prompt, no prompt, other line
  breaks, the 50-line reach, padding but not a typed space, rows below a
  shell's caret, an empty prompt learnt with its footer, a wrapped prompt
  with the caret inside it, a missing footer), the learnt prompt cutting its
  edit, an edit begun inside a wrapped input (on its prompt row or a row
  below), the remembered prompt forgotten at Enter and relearnt, and not
  used for a caret line with another prompt (a shell after Ctrl+D, a row
  with `>`), keys added after the last read and to an edit never read, and
  not added (a key without a character, another focus, a key during the
  read, text after the caret such as an inline suggestion), keys after
  Enter, when a line window replaces a paragraph read, line windows as
  "before" only at the prompt (also after an idle end), no "before" read
  another way, typing reads a second apart and a busy provider during typing,
  paragraph reads cut to the paragraph before the caret's (a new line after
  Enter), and an edit's span from its first key to its last or its Enter.
- Breaker: a costly or slow read stops and paces reads but not probes; a
  slow probe paces reads too.
- Storage and screening: `decide_ui_event` with a secret only in
  `text_before`; recorder screening before PII removal and the entered-text
  gate; the redaction worker covers `text_before`.
- Not yet: a live Windows fixture for the UIA read (needs the interactive
  validation readiness gate), and a live trial.

## Known limits

- Paste or cut through a right-click or menu sends no edit key: the edit is
  not seen until a later key.
- Text typed after the last read is added from keystroke capture only when
  every key typed one character at the end of its line (see the model);
  typing in the middle of a line after the last read is not added, and an
  inline suggestion the user had not accepted is stored as the last read
  showed it (a suggestion over several lines is not recognised). Otherwise
  it is missing from "after" (`keys_after_read` counts those keys,
  `keys_added` is 0), and an edit with no read and no "before" is dropped
  (`lost`). The characters come from keystroke capture's own key mapping,
  so they are only as right as its text. Text typed within 150 ms after Enter lands in the Enter read, so it appears as existing text
  in the next edit's "before" (or nowhere, if typing stops there).
- A chat box that clears more than 150 ms after Enter ends the edit as
  `enter`; the prompt then becomes the next edit's "before" (or a new message
  replaces it unseen).
- Apps without a text pattern on the focused element store nothing
  (`no_text_source`).
- Line windows: a prompt without `>`, `❯` or `$` (`%`, `#`) keeps only the
  caret's line; an edit whose typing began without an empty-prompt read,
  in a field with no prompt remembered since the last Enter, learns its
  prompt from its first read's caret line, so a multi-line input typed that
  way keeps only its rows from the caret's, and none below it; a program
  that exits without an Enter key (Ctrl+D, Ctrl+C) leaves its prompt
  remembered: an edit begun on a shell line with no prompt mark of its own
  (a wrapped command's second row) before the next Enter is cut at the
  nearest line starting with the old prompt and may keep that program's
  output; a shell command wrapped below the caret keeps only its rows up to the
  caret's; a chat app's menu row drawn with its prompt mark (an option
  picker) is taken as the prompt; a pasted or output line that starts with
  the prompt's text (`> quote` under a bare `>` prompt) starts the kept text
  there; a very long word-processor paragraph is read as its caret line
  (visual line) with no "before".
- While a process's read breaker is open (1 s to 30 s), reads there cannot
  start; after 2 s the open edit and baseline are given up, counted only as
  `not_now`.
- Input events refused by write-time screening are counted with the keyboard
  admission metrics' recorder exclusions (`RecorderExcluded`), not as a
  separate cause.

## Owner decisions (2026-11-16)

1. **Relaxed processes.** The two keyboard relaxations (focus without keyboard
   focus; unsupported password state) were consented for keystrokes. A read
   under a relaxation is allowed only for processes on a separate explicit list,
   `SCREENWISE_ENTERED_TEXT_RELAXED_PROCESSES`; the owner chose
   two text editors and may list further apps. Every other
   relaxed decision reads nothing (counted as `relaxed`).
2. **Write-time screening scope.** `decide_ui_event` decides every input event
   (`text`, `clipboard`, `text_edit`, and the text fields of other events) at
   the recorder's persistence boundary when content screening is enabled. This
   tightens the existing streams.

## Later phases

- OCR source for apps with no UIA text (caret position from the system caret
  or IME window, then OCR of that line; refuse when no position is known).
- Keystroke tail to close the Enter race in chat boxes.
- Read latency and per-process diagnostics in the sensitive sink.
- Expose `text_edit` in the Pi recordings extension.
