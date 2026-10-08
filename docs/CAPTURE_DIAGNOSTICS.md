# Capture interval diagnostics

This is a scoped implementation record, not a privacy or security guarantee.
The subsequent microphone-policy change separates microphone admission from
visual/protected foreground checks. Output audio retains the existing protection
gate; configured visual exclusions do not gate microphone admission.

## Microphone policy and revocation

Microphone and output acquisition now carry separate permit generations. Lock
(unless audio record-while-locked is explicitly enabled), schedule, audio disable
and explicit all-recording pause still deny both. The authenticated
`GET/POST /recording/privacy` exposes the session-wide `all_recording_paused`
boolean; POST rejects unknown fields. Clearing it clears only that flag. Desktop
manual stop sets it before awaiting lifecycle work; only a successful explicit
manual start clears it if no newer pause request arrived during startup.
Internal graceful shutdown/restart does not silently
clear the flag. A process restart begins a new session; this flag is not a new
persisted preference or exclusion scope.

Device pause/disable/stop revoke an opaque device generation before awaited
shutdown. Automatic startup reopens a separate lifecycle gate without clearing
user disablement or overriding a newer stop. Resume cannot revive previous
buffered, queued or serialized work.
Callbacks hoist atomic contexts, and downstream recording, live transcription,
SQLite writes and delayed cache callbacks validate original permits. Live worker
cleanup and overlap history are scoped by device so output suppression does not
clear admitted microphone state. The shared background model session may reset
when switching devices; that can add processing latency. Historical reconciliation
without authoritative acquisition modality remains conservatively output-gated.
No fresh microphone permit is minted to widen such historical work.

Privacy suppression stops callback sample admission and persistence; a worker or
OS stream may remain allocated while paused. Synthetic broadcast-stream and
SQLite tests exercise this path without opening a hardware endpoint or validating
live CPAL/WASAPI behavior. The optional macOS Process Tap and Linux PulseAudio
sources carry the same scoped permits but are not platform-compiled here.
Existing lock/unlock stream-rebuild behavior remains.
New content-protection notices exclude microphone; version-1 historical notices
retain their original shared-gate meaning. Other permission/device errors still
independently affect microphone availability. The bounded device registry fails
closed on unknown identity, exhaustion or poisoned state.

## Storage and interpretation

Diagnostic queue pressure transitions and confirmed rejection counts also emit
fixed numeric local logs directly at store mutation, independently of SQLite and
Timeline polling. The reporter is registered before writer startup observations
and replays existing pressure/loss on its first registration. It runs outside the
store mutex, uses no captured payload and never re-enqueues its own warning.
Concurrent callbacks may log snapshots out of observation order, and first
registration can replay a racing transition. Serialized counters remain correct;
strict log chronology is not established.

Capture producers enqueue closed-enum observations in
`screenpipe-config::capture_diagnostics`. The existing independent privacy writer
drains them approximately every 100 ms into SQLite `ui_events` with event type
`privacy_notice`. Admission of captured content is not required to write notices.
The authenticated local `/capture-events` route returns the reconstructed fixed
messages and structured `diagnostic` fields; Timeline Recording status renders
them. `active_notices` retains current explanations when onset predates the
selected day. `active_intervals` exposes current producer observations.

An interval key is `(channel, source, scope)`. Scope is global, a numeric monitor
ID, or an opaque session-local audio-device ordinal. The authenticated audio
device listing includes `diagnostic_id` to resolve that ordinal locally. Zero
means the ordinal could not be assigned; it is not a verified device identity.
Hardware labels, private paths, titles, URLs and captured content are absent from
generic logs and notice messages. Verified excluded executable basenames appear
only in structured local activity metadata requiring authenticated access.

Each changed condition/reason/rule records `observed_at_ms`, `since_ms` and
`previous_since_ms`, in UTC epoch milliseconds. The next record for the same key
closes the previous observed segment at its observation time. Subtraction gives
the observed segment duration. Unchanged observations are deduplicated; the
stored onset remains available until another observation changes the state.
Overlapping sources remain independent: one gate clearing does not establish
that a device or channel resumed. Graceful writer shutdown closes known producer
segments as `stopped/session_stopped`. An abrupt exit leaves an interval open;
do not invent its end or infer a precise crash time from it. There is no backfill.

Conditions distinguish admission, complete suppression, failure, frame and partial text/context redaction,
active-window-only fallback, processing deferral, observed silent audio and stop.
Admission means this producer's gate allows work, not proof of successful capture.
Silent buffers mean samples arrived without usable signal; they do not mean the
microphone was disabled. Batch/live-session and power-profile transcription
deferral are separate from audio acquisition. Existing shutdown, recovery and
queue-delivery notices remain available.

## Safe rule references

Windows window-policy observations and UI input exclusion observations identify
matching ignore rules by source list, zero-based index and fixed matching kind:
app substring, app-and-title substring, legacy app-or-title substring, or URL
domain rule. Window indices refer to the original effective configuration list,
including empty entries. Sensitive matching text stays in that local
configuration; resolve the reference there. Indices are stable while that list
is unchanged, not immutable across reordered/replaced configurations. The source
is the effective list, not proof of whether a CLI flag or desktop setting supplied
it. Include-admission, built-in exclusions and URL title heuristics carry fixed
rule kinds; a refusal without a verified matching rule has no fabricated reference.
No raw pattern, matched title or URL is serialized in a generic notice.

`frames.capture_privacy.rule_matches` preserves the same references with verified
executable basenames and foreground role. Existing per-frame decision metadata
remains additive; it is not a substitute for pause notices when no frame exists.

## Producer coverage and limits

| Producer | Implemented observations | Limits |
|---|---|---|
| Shared privacy admission | Initial state and changed lock, schedule and content-protection flags for screen acquisition, JPEG storage, OCR, accessibility, keyboard, clipboard, pointer, microphone, output and transcription | Multiple flags are reported together; audio record-while-locked preference is respected |
| Windows content protection | Matched protected rule, unavailable identity, changed HWND/PID, browser URL unverified during retained pause, access denied, failed process check, gate cleared | Paused polling is approximately two seconds; shorter foreground changes may be missed; a matched rule is not proof of cryptographic DRM |
| Per-monitor engine | Privacy/power pause, inactive monitor, JPEG-storage-disabled power state (acquisition/OCR remain separate), capture success/failure/timeout | Failure/timeout classification is bounded; arbitrary OS/provider exceptions are not exhaustively distinguished |
| Windows window policy | Full frame, active-window fallback, redaction, acquisition/check failure, blockers and matching ignore/URL rule references | UIA race refusals may have no verified app or matching rule; unverified refusals do not invent rule detail |
| Accessibility | Tree found/unavailable/filtered/different monitor, password-state availability, active-input tree deferral and disabled tree preference | Provider/budget absence is a failed check, not necessarily permission denial; no unverified app identity is inferred |
| Keyboard and clipboard | Password/unknown-focus suppression and worker-reported unavailable, contended, generation-changed, focus-mismatch and stale-check losses | Hook counters stay atomic-only; aggregation reports observed losses, not exact per-keystroke interval timing |
| UI event persistence | Shared privacy gates, input/clipboard preferences, ignored-window rule references, batch persistence success/failure and partial pointer context redaction | Exclusion observations occur on routed events; mouse/input provider errors are not exhaustively classified |
| Audio devices | User pause/stop/resume, start failure, stream worker failure, teardown/recovery and successful start | Generic teardown is distinguished from user stop; device removal/fallback provenance, all disabled-device monitor paths and callback-specific failures remain incomplete |
| Audio signal/processing | Silent buffers versus usable samples, privacy-invalidated queued work and live/batch transcription deferral | Output no-callback timeout is distinct from silent samples; deferred backlog completion, STT/VAD exceptions and every compensation/discard path remain incomplete |
| Permissions | Existing lost/needed/restored events mapped to affected channels without retaining raw OS reason text | Depends on existing platform producers; keychain loss does not imply capture suppression |
| Session end | Known intervals closed on graceful writer stop | Forced termination and startup failures before the writer exists can lose the tail |

The in-memory queue and key map are bounded to 4096 entries. The authenticated
response exposes `diagnostic_delivery`, with an 80% capacity warning and numeric
rejected-observation count; Timeline displays both. Overflow or poisoned
state latches degraded status; unchanged state is not repeatedly queued. SQLite
writes retry with bounded backoff. Fixed logs contain all drained observations
before persistence attempts, so a writer timeout can still leave a content-free
explanation locally. Failed writes and writer shutdown failures latch degraded
timeline status. A queue loss or process crash can still prevent complete durable
interval coverage. Only one recorder/privacy writer should consume the process
store. These limits remain visible; this change does not establish complete
coverage of every stop, error, redaction or capture producer.

No new recording, foreground interaction, captured-content inspection, firewall
change or deployment is part of these source and synthetic checks.
