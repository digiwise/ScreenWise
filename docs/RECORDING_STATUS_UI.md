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
