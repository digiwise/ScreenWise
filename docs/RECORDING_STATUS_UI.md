# Recording status desktop milestone

The home toolbar **Status** button and existing status deep link open a dialog
using the authenticated `/capture-events` current `active_intervals` snapshot.
It polls every five seconds only while open, with an eight-second request timeout.
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

This milestone adds no approval, pending-review storage, Keep/Discard policy,
notifications or always-on-top window. Those can build on the observer model.

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
