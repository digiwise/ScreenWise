# Recording status desktop milestone

The home toolbar **Status** button and existing status deep link open a dialog
using the authenticated `/capture-events` current `active_intervals` snapshot.
It polls every five seconds only while open, with an eight-second request timeout.
Fetch or schema failures clear previous observations and report unavailable status.

Ten instrumented data types are shown independently: screen images/video,
screenshot storage, OCR, accessibility interface data, keyboard, clipboard,
mouse, microphone audio, computer audio output and audio transcripts. Each
observer retains its numeric monitor/device scope or global scope, condition,
allowlisted reason, rule configuration origin/index/kind, onset and last change.
Sensitive matching patterns, app identities, titles, URLs, paths and raw exception
text are never rendered by this dialog. Global scope describes a broad observer and is not guessed into one monitor. The agreed Multiple or Unknown selection describes attribution ambiguity; it does not redefine an observation that explicitly applies globally.

An open admission gate is labelled **Capture permitted**, not successful recording.
Silent samples and missing callbacks are distinct states. Missing observations
are explicitly unavailable; no activity-metadata observer is invented. Last checked
indicates snapshot retrieval, not a producer heartbeat. Current diagnostics do not
provide continuous recording/persistence confirmation or raw OS exception detail.
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

Validation in an isolated source worktree: TypeScript no-emit check passed;
15 parser regressions, four dialog behavior checks and five deletion-result
checks passed. Tests cover differing microphone/screen conditions, missing
observers, unsafe responses, rule provenance, silence and API failure. Final
frontend export, desktop checking and the production desktop build passed; no GUI,
live recording, deployment or production data read occurred in these tests.
