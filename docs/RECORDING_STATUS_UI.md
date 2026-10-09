# Recording status desktop milestone

The home toolbar **Status** button and existing status deep link open a dialog
using the authenticated `/capture-events` current `active_intervals` snapshot.
The persistent home webview now polls every five seconds in the background, with
an eight-second request timeout. Other webviews poll only while Status is open.
One shared loop per webview serves the toolbar, dashboard and alerts.
Fetch or schema failures clear previous observations and report unavailable status.

## Trustworthy live status increment

The same authenticated response now supplies `live_status` schema version 1.
It reports current-process session start, snapshot time, monotonic session age,
producer check ages and independently observed capture/storage completions.
This panel does not infer success from a historical database row or an open gate.
Server evidence timestamps record successful stage completion, not the captured
item's original timestamp. Displayed elapsed ages use server monotonic ages plus
time since retrieval. The full request duration is also added conservatively to
all producer/evidence ages, so a delayed response cannot make an older check
appear fresh; wall timestamps are explanatory history.

For each reported monitor or numeric device, **Last captured** (or **Last samples
received** for microphone/output) and **Last stored** are separate. Audio samples
include silence: sample receipt does not prove a working microphone, speech or
transcription. Silence and callback failure explanations remain visible alongside
the evidence. No success observed in this session differs from instrumentation
unavailable. A last success is historical evidence, not a claim of continuous
recording, retained bytes or current permission. Sparse keyboard/mouse activity
and idle audio are not diagnosed as recording failures simply because they are old.

Producer-check freshness is separate from condition onset. Missing checks or
checks older than 15 seconds display **Status unavailable — last reported:**
with the historical condition and reasons. Repeated unchanged checks refresh the
latest report while preserving condition onset. Global instrumentation coverage
is not a device/monitor success and remains distinct from scoped evidence. Empty
global rows are labelled instrumentation coverage, not global successful capture;
when scoped successes exist the empty global placeholder is suppressed. No
global success is inferred by aggregating source-specific evidence.

The panel refreshes ages once per second. A retrieved snapshot older than 15
seconds, backward clock change, material disagreement between wall and monotonic
elapsed time, visibility change, fetch error or invalid schema makes current
status unavailable and hides previous evidence. Responses spanning more than
15 seconds, clock discontinuities or a visibility-generation change are rejected.
Eight-second request aborts and five-second retries remain. Unsupported older
recorder responses do not fall back to old permission-only claims. A fresh
response replaces all observations, including after a recorder session restart.

Ten instrumented data types are shown independently: screen images/video,
screenshot storage, OCR, accessibility interface data, keyboard, clipboard,
mouse, microphone audio, computer audio output and audio transcripts. Each
observer retains its numeric monitor/device scope or global scope, condition,
allowlisted reason, rule configuration origin/index/kind, onset and latest report.
Sensitive matching patterns, app identities, titles, URLs, paths and raw exception
text are never rendered by this dialog. Global scope describes a broad observer and is not guessed into one monitor. The agreed Multiple or Unknown selection describes attribution ambiguity; it does not redefine an observation that explicitly applies globally.

An open admission gate is labelled **Capture permitted**, not successful recording.
Silent samples and missing callbacks are distinct states. Missing observations
are explicitly unavailable; no activity-metadata observer is invented. Last checked
indicates snapshot retrieval, not a producer heartbeat. Completion evidence does
not provide continuous recording/persistence confirmation or raw OS exception detail.
The panel preserves multiple independent gates rather than treating one cleared
gate as proof all conditions cleared. Degraded persistence or lost diagnostic
delivery is visible.

The panel also displays validated activity delivery loss/capacity, individual
audio queue capacity and confirmed versus possible delivery losses, and fixed
audio shutdown failure reasons. Latched counts/issues include earlier session
incidents and are not presented as proof loss is continuing at the current time.

The initial milestone added no notifications. The dashboard/alerts increment
below adds notifications without implementing approval, pending-review storage,
Keep/Discard policy or an always-on-top window.

Existing timeline and recent-data deletion callers also recognise
`file_cleanup_incomplete`: they show validated committed deletion counts and
incomplete media cleanup without a success toast or raw server error content. Pending media counts are distinct from recorded deletion failures, scheduled retries are stated only when confirmed, and unknown cleanup outcomes never imply zero media deletion or zero remaining files.
Timeline caches are cleared after partial database deletion. Backup/external copy
erasure and physical overwrite are not claimed.

## Evidence coverage and limits

| Data type | Successful operation that advances evidence |
|---|---|
| Screen | Actual primary monitor pixels acquired; privacy placeholders do not count |
| Screenshot storage | Primary JPEG write and associated frame transaction completed |
| OCR and interface data | Primary paired capture completed with that text source and committed its transaction |
| Microphone and computer audio output | Nonempty admitted samples received, including silent samples; encoded file and privacy-checked chunk registration tracked separately |
| Audio transcripts | Queued transcription result observed and its privacy-checked transcript transaction completed, independently |
| Keyboard, clipboard and mouse | Admitted event of the specific type received; matching batch row committed |

OCR/interface capture evidence is conservative: a failed paired write does not
report their earlier processing as a separate success. Secondary HD video,
reconciliation/import and live-provider transcription paths are not fully
instrumented by this increment. Missing evidence is not proof that those paths
stopped. Global capability rows do not identify an active device or aggregate
monitor success. Numeric device IDs are local to the process session.

The evidence map and producer-check map are bounded. Truncation or a poisoned
evidence lock makes evidence unavailable rather than silently claiming complete
coverage. Producer checks do not create capture permits or change privacy gates.

The earlier status milestone passed its isolated frontend checks and release
builds. This increment adds canonical-checkout regressions: 26 parser tests and
13 dialog behavior tests passed, covering independent completion evidence,
unsupported instrumentation, producer-check freshness, invalid/sensitive fields,
silence, API failures, suspended/late requests, visibility changes, backward clock
changes, request-latency accounting, global capability versus scoped success and
session replacement. TypeScript no-emit check passed. Static export and both production release builds passed; exact artifact results are in the validation register. No GUI, live recording, deployment or production
data read occurred in these tests.

## Audio activity and transcription throughput

`live_status.audio_activity` schema version 1 adds a bounded 60-second reporting
window per microphone/computer audio output and numeric device. It separates
sample duration, silent/non-silent sample duration, VAD-processed audio,
VAD-detected speech, uncertain classification, accepted transcript words/results
stored, and audio deferred before VAD. Non-silent samples are not a speech
detector result. VAD speech measures samples classified as speech by the existing
detector; it is not proof that speech was transcribed correctly. Detector errors
make VAD coverage unavailable, and uncertain classification remains separate
from silence. Stored words use whitespace counting; their counts describe
throughput, are language-dependent and are not accuracy scores.

The window counts stages when observed, processed or committed, rather than
following an aligned capture timeline. Different stage counters can represent
different audio. Delayed processing can contribute more than one minute of audio
duration to the reporting window; the UI does not silently clamp values or
calculate a transcript-to-speech ratio. A newly started recorder reports its
shorter observed window. Null counters mean a stage has not been observed or
its VAD coverage was invalidated after a detector failure;
they display unavailable/partial coverage rather than zero or silent recording.
Missing or unavailable bounded reports do not reuse earlier counters. Aggregates
use 100 ms buckets, so the window boundary has bucket resolution. Device
zero remains unknown, and mic/output devices are not aggregated or paired.

Estimated speech in active inference tasks describes whole active VAD-positive
processing tasks. A task can contain several segments; its full classified speech
duration may include speech already partly processed, so this is not exact remaining
speech duration or a complete backlog. These tasks exclude
pre-VAD queues, audio deferred before VAD and results waiting for database
storage. Thus zero pending inference does not establish an empty transcription
pipeline. The oldest active task age includes conservative request elapsed time and
advances while a snapshot remains fresh; rolling durations/counts stay the
reported snapshot values until the next response.

With at least 30 seconds of observed window, available VAD classification,
at least ten seconds of detected speech and no more than three stored words,
an informational hint says: **Observed VAD speech; little transcription stored.
Results may still be processing or deferred.** It makes no same-audio, fault,
transcript accuracy or one-sided-conversation claim. Existing producer reasons,
silence and delivery loss remain independent evidence. Stale snapshots, failed
requests, visibility changes and invalid counters hide audio metrics under the
same freshness rules.

Focused parser/UI checks cover separate devices/input/output, missing/partial
coverage, stage distinction, delayed durations, bounded safe numeric fields,
private field rejection, informational hint thresholds and stale metric hiding.
Final validation results for this increment are recorded with its release milestone.

## Compact dashboard, background alerts and status log

The toolbar shows coloured Visual, Audio and Input/activity group icons with
state shapes, accessible labels and counts. Opening Status shows compact group
cards; expand a group, then a data type, to inspect monitor/device evidence,
specific reasons/rule references and audio stage measurements. Individual scopes
remain independent. A group summary is not a claim every source shares its state.

Green indicates recent verified success; capture-only evidence explicitly says
storage is unconfirmed. Delayed storage alone does not indicate recording.
Blue listening/quiet indicates fresh operational observations with no recent
input, not a verified working microphone. Amber pause, red error and grey unknown
have distinct shapes as well as colours. Stale permission/blocker checks cannot
be overridden by another observer's success. App/window activity metadata is
listed explicitly as uninstrumented, rather than invented as healthy.

The home webview is the background notification leader; other windows cannot
produce duplicate native reminders. Unexpected operational failure, persistent
verification uncertainty or unavailable status triggers one subdued in-app warning
after 60 seconds. One combined continuing incident repeats at most every 15
minutes, rather than notifying separately for cascading sources or changing
reasons. Recovery, recorder session changes, clock discontinuities and suspension
reset the observation period. Existing cumulative loss counts are historical,
not automatically repeated ongoing-error notifications. Known manual/schedule/
lock/disabled/exclusion pauses, silence and ordinary sparse input are not repeated
fault warnings. Stale expected-pause observations cannot hide a fresh failure.

Native reminders use the existing Capture stalls preference and granted OS
notification permission; the new monitor never requests permission. The local
warning remains available when permission is denied or native delivery fails.
Notifications contain fixed status descriptions, not captured content or source
identities. A local home heartbeat every ten seconds suppresses the older restart
popup path; its 30-second monotonic expiry restores legacy fallback. This affects
notification ownership only, not capture gates, health checks or recovery.
Background warnings depend on the desktop/home webview remaining alive and its
timers running. They are not an independent OS service or an always-visible tray
monitor; suspension cannot guarantee delivery at the one-minute deadline.

**Status log** is an expandable list, not a chart. Filter by inclusive local dates
(up to 32 days), logical group, data type, condition and monitor/device/global
scope, optionally with a numeric source ID. Source/condition filters apply to
typed observations; older system notices may lack this attribution. Fixed state,
allowlisted reason/rule references and onset/report times are shown; stored free
text, app names, titles and paths are never rendered. This is persisted transition
history, not continuous capture proof or historical audio metric graphs.

The authenticated API scans bounded pages with an exact timestamp/ID cursor.
Filtered-empty pages still advance, including equal timestamps and sub-millisecond
precision. Load older notices to continue; more history is explicitly indicated.
At most 5,000 loaded matches remain displayed; newer displayed rows are removed
with an explicit notice as older pages load. Refresh returns to newest history.
Missing/invalid diagnostic persistence is marked incomplete. Filters abort old
requests so stale responses cannot restore previous selections.

This increment passed 89 focused frontend checks, 13 engine notice checks and one
native notification-ownership regression, plus TypeScript, production frontend
export, formatting, desktop check and both production release builds. Exact cache
counts and artifact identities are in the [validation register](../VALIDATION_REGISTER.md#compact-recording-status-release-validation-2026-10-09).
Installed GUI appearance and native notification delivery have not been exercised;
no deployment or new capture session was performed.

## Named sources and floating dashboard

The owner selected Listening for operational quiet audio and Watching for visual
or event capture awaiting new input. Green Recording (or Capturing when recent
storage is unconfirmed), blue Listening/Watching, amber Paused, red Error and grey
Unconfirmed have distinct icons. Stopped is not a separate dashboard state:
explicit user/setting/schedule pauses identify their cause; missing callbacks and
producer failures are errors; outdated evidence remains unconfirmed. A failed
status request cannot establish that the backend process is absent.

Positive user-preference observations are setting transitions, not recurring
producer checks. Fresh aggregate privacy admission verifies their operating gates.
It also covers the positive output-audio content-protection observation because
the aggregate admission reads that gate. Power observations require current
operational evidence. Explicit current-session user-preference pauses remain
Paused until changed; stale operational failures or unverifiable protection checks
never silently become active. Details identify the observer, scope, reason/rule,
last report and last operational check; policy snapshots are labelled separately.

Quiet audio uses a complete 60-second observation window with at least 59 seconds
of silent samples and at most one second of non-silent samples, plus a fresh
same-device silent observation and capture evidence. This accommodates small
noise without claiming speech, microphone audibility or transcript accuracy.
Watching without a recent new image requires fresh operational capture checks;
fresh privacy permission alone is insufficient. Unchanged screens do not imply
an audio-style continuous-sample failure. Processing/storage remain separate.

Actual audio-device labels are bounded, escaped current-session metadata from the
authenticated status response, matched by channel and session-local device ID.
Invalid or ambiguous name metadata falls back to unknown without breaking numeric
capture evidence. Device names do not need blanket redaction in the local UI.
This increment exposes them as explicit source metadata rather than generic
diagnostic text. Historical naming needs a persisted session/source mapping first:
old IDs must not be matched to a new session's device. Names therefore are not yet
copied into persisted notices, diagnostic logs or historical rows; this is an
implementation boundary, not a decision to permanently hide names from history.

The separate Recording dashboard opens from the adjacent toolbar button. It is
always on top, initially in the primary monitor's lower-right work area, movable,
resizable and hideable. Reopening an existing window preserves its position.
Six primary data types are visible; derived types expand by group, whose icon
still reflects hidden problems. Status opens the existing detailed panel locally.
The floating window polls independently while mounted without becoming a duplicate
notification leader or mounting onboarding/deeplink lifecycle handlers.

Quick controls offer explicit Start all/Pause all, current-session per-device
audio start/pause, and saved screen, keyboard, clipboard and pointer recording
preferences. Audio control resolves the current authenticated device list by exact
ID and type; displayed labels are never reconstructed into control identities.
Preference changes are saved before applying and may briefly restart other active
capture. Applying preferences does not start an absent/manually paused session or
clear an existing privacy/device pause. Requests are acknowledged separately from
observed recording; failures do not create optimistic green status.

Keyboard, clipboard and pointer switches control event persistence. Their hooks
can still trigger permitted visual capture. Pointer off filters click/move/scroll
rows independently, with old settings preserving the previous enabled default.
Screenshot storage, OCR and paired accessibility follow visual capture;
transcription follows its audio sources. No independent derived-data or activity
metadata control is invented. Keep/Discard and review-before-keeping remain separate.

## Prepared follow-up after deployment (2026-10-09)

The owner requested preparation and investigation only. Do not implement these
changes until the current `release-local` candidate has been deployed. They are
not part of that candidate; the build/source remain fixed.

### Preserve last status when stale

Keep the last reported status, icon and specific reason visible, with an adjacent
warning such as **stale 15s**. Label it as last reported so old Recording does not
claim current recording. Staleness supplements rather than replaces the status.
Never-seen status still needs an explicit unknown state. Request failures remain
visible separately, alongside the older status if available.

Keep the last successful snapshot separately from transport freshness: the current
poller clears observations on failure, visibility change and suspension, while the
classifier replaces old blockers with an unknown summary. Preserve per-source
observations through those cases, without applying old device IDs to a new session.
Calculate age from the producer check/response time, not when the condition began.
Distinguish durable settings state from periodic operational checks, and account
for the producer's expected reporting cadence. Show source-specific staleness;
one stale observer should not silently relabel every fresh source. Stale display
must not change capture/privacy admission or make an old sample a fresh success.

### Foreground identification investigation

The monitor-specific `foreground_unavailable` reason comes from
`evaluate_windows_monitor_capture` in
`crates/screenpipe-screen/src/capture_screenshot_by_window.rs`: it samples the
native foreground HWND/PID and searches the capture-window enumeration for it.
If it was not seen, it emits `foreground_window_unavailable`, which the engine
maps to `ForegroundUnavailable`. This does not necessarily mean the application
identity is unknown: the code can attach a verified executable to that blocker.

The existing pinned xcap 0.9.4 source's Windows `is_valid_window` deliberately
omits all windows belonging to the current process. In the desktop's embedded
recorder, ScreenWise's own GUI can therefore be absent from `Window::all()` while
foreground. Invisible/cloaked windows, some shell/tool windows and focus transitions
can also produce a missing match. This is a confirmed mechanism and a likely
recurring cause while looking at ScreenWise; the existing reason cannot establish
the cause of every occurrence. No external implementation was copied or changed.

A bounded diagnostic-log sample on 9 October, approximately 09:05–10:41 Sydney,
contained 192 `foreground_unavailable` mentions. These are not 192 distinct
episodes: monitor/channel notices and transitions can repeat. Only fixed reason
counts/timestamps were extracted, without printing captured content or log text.
A subsequent authenticated status-only query showed the reported monitor's
current window policy was fresh and redacted for excluded background/no safe
window, rather than foreground identification. That later sample does not explain
the earlier screenshot's exact cause.

Prepare separate reasons for missing native foreground, changed foreground,
known window absent from the capture list and recognised recorder/shell windows.
Recognise known identities independently of capture eligibility; retaining a
redaction may still be correct. Preserve before/after focus checks, exclusions and
monitor uncertainty. Any adjustment to capture admission needs separate review;
renaming an unknown state is not permission to capture it.

### Monitor names and input safety detail

Monitor metadata already includes `id`, `stable_id`, `name`, resolution and primary
status, and Recording Settings displays names. Reuse verified current monitor
names in status, with position/resolution for duplicate names and numeric IDs in
technical details. Optional user aliases can be considered later. Hardware/topology
changes require identity handling before naming historical observations.

The input safety check is the UI Automation password-field probe in
`crates/screenpipe-a11y/src/platform/windows_uia.rs`. It checks native focus,
the focused UIA element, keyboard-focus ownership and PID, the explicitly supported
`IsPassword` property, and unchanged focus/element at the end. Failed calls,
unsupported properties, missing decisions and mismatches currently collapse into
unavailable paths; `.ok()?` discards the underlying error/stage. The hook also
uses DecisionUnavailable when the visual privacy gate is closed. The current
generic label therefore cannot identify which step failed in the screenshot.

Prepare bounded stage/reason diagnostics and practical error codes for native
focus, worker readiness, UIA retrieval, ownership validation, password-property
support/read and final focus comparison. Existing stale-decision, worker-lock,
generation and focus-mismatch reasons should remain distinct. Explain whether
keyboard content, clipboard content or click-element text was suppressed, instead
of implying that every input event or producer stopped. Do not record element
names, values, window titles or arbitrary exception payloads in generic notices.
Keep password checks fail closed.

### Normal gaps in computer audio output

The backend already treats Windows/macOS output receive timeouts as non-fatal:
`run_record_and_transcribe.rs` explicitly notes that nothing playing can mean no
callbacks, reports AudioProcessing/NoCallbacks and continues. The dashboard and
alert classifier currently treat every NoCallbacks observation as a failure.
That is a UI/alert interpretation mismatch, rather than a fatal backend timeout.

Show ordinary output gaps as **Listening — waiting for computer audio output**,
including time since the last sample and a separate check-age warning when needed.
No samples is different from received silent samples: invent no silent duration,
speech classification or inferred playback. Do not generate repeated fault alerts
merely for quiet output. Keep microphone timeouts, explicit device removal/stream
failure, backend failure, user/policy pauses and genuinely missing producer
verification distinct. Do not suppress a real error because a device is output.

After deployment, implement these together with regressions for preserved stale
statuses, session/device identity, ordinary output gaps versus explicit failure,
named monitors and specific input/foreground failure stages. Validate against the
actual local-profile candidate; no new capture or interactive trial is authorised
by this preparation document.

### Additional batch scope agreed in chat (2026-10-09)

The next implementation batch also includes these owner-requested dashboard tasks:

- Persist dashboard visibility, position, size and selected summary/detail level.
  Restore it on startup if visible at the previous shutdown. Validate geometry
  against current monitor work areas and DPI; recover a usable on-screen position
  when a monitor is disconnected or the saved placement no longer fits. Restoring
  the dashboard must not start or resume recording.
- Add a system-tray action to show/reopen the floating recording dashboard.
- Add monitor aliases, upgrading the earlier optional-alias proposal to included
  scope. Keep aliases separate from current runtime IDs and expose the underlying
  identity in details; do not attach an alias to an uncertain historical monitor.
- Make specifically and only the main-window row containing the Status label
  shown in the owner's screenshot wrap, with a logical two-line layout when needed.
  Preserve readable labels and access to every control on that row at narrow widths
  and increased display scaling. Other main-window rows are outside this task.
- Add level 1 as a complete compact overview without scrolling at the intended
  default size. The existing view becomes level 2, with existing More details as
  level 3. Summaries must expose mixed states, errors, pauses and staleness rather
  than hide them behind one green group icon. Keep clear access to drill-down and
  recording controls.
- Remove the permanent footer from level 1. Put the preference-restart explanation
  in hover/focus help on the relevant checkboxes. Preserve the practical distinction
  that device controls apply to the current session in help on those controls,
  without permanently taking space. Keep derived/source relationships in level 3
  labels/help, where they affect interpretation. Remove duplicate footer sentences.

These extend the prepared stale-status, foreground/input diagnostics, named-monitor
and normal-output-gap work above. The same next batch also includes owner-priority
findings SW-R04, SW-R06, SW-R08, SW-R12 and SW-R15 from the validation register,
and deployment that automatically starts only after existing firewall rules and
the full startup preflight pass, with an explicit prepare-only option. Finish with
focused regressions, required checks, one settled local-profile build/export,
documentation and commits. Actual deployment/interactive validation remain separate
actions. This section records scope only; none of these new changes is implemented.

### Consolidated next implementation batch

This list consolidates the scope above without dropping the owner's priorities:

1. Truthful status presentation: preserve last reported state/reason with an
   adjacent staleness age; keep ordinary computer-output audio gaps Listening and
   suppress inappropriate fault alerts, while retaining real errors.
2. Specific foreground and input-safety diagnostics: distinguish identification,
   enumeration, focus-transition, password-property and worker/check failures,
   preserving capture/privacy gates and practical bounded error details.
3. Monitor identification: existing monitor names plus user aliases, duplicate-name
   disambiguation, technical IDs in details and sound identity/session handling.
4. Floating-dashboard lifecycle/access: restore prior visibility, position, size
   and view level with on-screen/DPI checks, and add a system-tray launcher.
5. Three-level dashboard: full compact overview without scrolling at level 1,
   current view at level 2 and existing More details at level 3; move the permanent
   footer into relevant control help/detail labels.
6. Responsive layout only for the main-window row containing Status in the
   screenshot; wrap that row's controls at logical breakpoints.
7. Reliable deletion and retention lifecycle: SW-R04 durable failed-eviction retry
   jobs and SW-R06 restored saved retention after every backend restart.
8. Correct Windows fallback audio configuration/sample-rate propagation (SW-R08).
9. Exact CORS origins and minimal unauthenticated versus detailed authenticated
   health (SW-R12), preserving existing supported consumers.
10. Convert only timestamp metadata, preserving captured text verbatim (SW-R15).
11. Deployment verifies existing firewall rules and complete startup preflight,
    then starts automatically only when both pass; explicit prepare-only option.
    Missing/conflicting rules leave it stopped, with useful instructions and no
    automatic firewall mutation.
12. Focused regressions and required checks; one settled frontend export and
    affected release-local builds, updated validation/docs, reviewable commits and
    preparation of the next deployment candidate.

Implementation remains deferred until the current candidate is deployed, as agreed.
