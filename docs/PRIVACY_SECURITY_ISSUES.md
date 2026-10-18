# Privacy and security issues

This companion to [privacy design and limits](PRIVACY_BOUNDARIES.md)
separates implementation gaps and review questions from deliberate recording
policy. Source review date: 17 October 2026, revision `68a1be820`. These entries
are not evidence that private content was exposed on a particular installation.
No new capture, live UI inspection or captured-content review was performed.

The [validation register](../VALIDATION_REGISTER.md) is the reference record.
This document explains its open privacy-relevant entries in plain language, with
their original P1/P2 review priority. Where the two disagree, the register
wins.

## Existing gaps confirmed in the current source

### URL exclusions do not cover every input record

**Scenario:** A developer excludes a banking or administration URL from screen
capture, but enables keyboard or clipboard database recording. Text typed or
pasted into that page can still reach input records because their persistence
path evaluates window ignore patterns without the frame URL policy. Frame
content screening does not screen input rows.

This is existing issue **SW-R01** (P1). Keep keyboard/clipboard storage disabled or
use an app/window exclusion or explicit all-recording stop for that interval;
URL exclusion alone is insufficient. The fix needs URL-aware input admission
with failure handling and a synthetic excluded-page regression.

Sources: [input admission](../crates/screenpipe-engine/src/ui_recorder.rs),
[recording configuration](../crates/screenpipe-engine/src/recording_config.rs),
[existing register](../VALIDATION_REGISTER.md#system-wide-source-review-and-owner-priorities-2026-10-09).

### Monitor selection can expand instead of stopping capture

**Scenario (before 2026-10-18):** A developer selected only an external display,
then its identity changed or it disconnected. When recording started and no
selected monitor started, a compatibility fallback could start all connected
displays, including a laptop screen intended to remain outside capture. The
manager also treated an empty selection as permitting all monitors.

This is existing issue **SW-R03** (P1). The 2026-10-18 candidate removes desktop
monitor selection in favour of all monitors or screen-off. Legacy selective
settings become off until all monitors are explicitly enabled, and a main-window
notice announces the change until dismissed. Lower-level filters no longer treat
an empty selection as all displays or fall back after startup failure. See
[scope and validation](RESOURCE_MEASUREMENTS.md#screen-scope); live migration
and hot-plug validation remain pending.

Source: [monitor admission and startup fallback](../crates/screenpipe-engine/src/vision_manager/manager.rs).

### Schedule modality choices are not enforced

**Scenario:** A developer sets an audio-only work-hours rule expecting the screen
to remain unrecorded. The schedule monitor enforces whether the current time is
inside a rule, but does not apply its audio-only/screen-only modality selection.
Otherwise enabled sources can therefore remain admitted.

This is existing issue **SW-R02** (P1). The owner selected simple on/off windows
in the 2026-10-18 candidate. New windows apply to all otherwise-enabled sources;
the source-mode selector is removed. Legacy source-specific windows block all
recording during their hours until explicitly converted or replaced, including
when overlapping an all-source window. A saved window that ends at or before its
start blocks its whole day; the editor no longer saves such windows, and a
main-window notice lists saved blocking windows. Source and privacy preferences
still apply. Schedule transitions remain polled every 30 seconds, rather than enforced
at an exact wall-clock instant; live transition validation remains pending.

Source: [schedule monitor](../crates/screenpipe-engine/src/schedule_monitor.rs).

### Removing media does not revoke all copies

**Scenario:** A developer views a frame, then deletes its recording interval or
evicts its media. Extracted frame files, background-retention cache state and
copies already held by clients can outlive the source operation. The ordinary
file-serving helper still emits `Cache-Control: public, max-age=604800`; the
separate PII-redaction response uses `no-store`.

This is existing issue **SW-R05** (P2). **Repaired 2026-10-17, synthetic
tests only:** frames are now served `private, no-store`, the database is checked
before any cached copy, and extracted copies are deleted with their cache entry,
on deletion, eviction and retention, and at startup. Copies already delivered to
a client are unaffected. The cache header does not by itself bypass API
authentication or prove that a shared cache stored the response. It does permit
retention that deserves review for sensitive media. A fix needs consistent
sensitive-response caching policy, derived-file invalidation and tests of a
previously viewed frame after deletion and retention.

The broader [data control audit](DATA_CONTROL_AUDIT.md) also describes partial
shared-media deletion and surviving derivatives. Treat that audit as a record
of scoped findings, not proof that every historical finding remains unchanged.

Sources: [frame responses](../crates/screenpipe-engine/src/routes/frames.rs),
[manual data operations](../crates/screenpipe-engine/src/routes/data.rs),
[background retention](../crates/screenpipe-engine/src/retention.rs).

### Other open register issues

- **SW-R09 (P2): delayed audio startup can outlive a capture stop.** Audio
  starts from an unowned task after a one-second delay, and stopping the session
  does not cancel it, so devices and workers can start after a quick stop. The
  manual pause gate still denies samples, so this is not established as content
  capture. **Repaired 2026-10-17 (synthetic tests):** stop now cancels or joins the
  start. Source: [capture session](../apps/screenpipe-app-tauri/src-tauri/src/capture_session.rs).
- **SW-R10 (P2): chat deletion can fail silently.** A locked or inaccessible
  chat file can remain on disk while the UI reports it deleted. **Repaired
  2026-10-17:** failures now keep the chat listed and are reported. Source:
  [chat storage](../apps/screenpipe-app-tauri/lib/chat-storage.ts).
- **SW-R11 (P2): an in-flight save can recreate a deleted chat.** Deleting a
  conversation while a response is streaming can let its autosave write it back.
  Source: [conversation persistence](../apps/screenpipe-app-tauri/components/hooks/use-chat-conversations.ts).
- **SW-R13 (macOS): audio fallback drops app exclusions.** If Process Tap
  startup fails, output audio falls back to a path without per-app exclusions.
  Not re-checked on macOS for this review. Source:
  [audio stream startup](../crates/screenpipe-audio/src/core/stream.rs).

## Issues found since the original review

### UIA requests load the target application

**Scenario:** A developer uses an application with a large or constantly
updating accessibility tree. Each privacy walk asks that application to build
its whole tree in one request, and the work runs on the application's own UI
thread. When ScreenWise's deadline expires it abandons the request, but the
application keeps building the answer. Repeated walks, plus focus checks on
keystrokes, can make it noticeably slow. This was observed with one desktop
application on 16 October 2026. This is register issue **SW-R19**.

Per-window backoff now stops repeated walks and probes after timeouts, and
property changes to Name and Value no longer restart walks. A single request is
still unbounded work for the target. A fuller fix would fetch the tree in
bounded steps with a per-request node budget. Sources:
[bulk acquisition](../crates/screenpipe-a11y/src/tree/windows_bulk.rs),
[UIA backoff](../crates/screenpipe-a11y/src/uia_breaker.rs).

### Unexplained heap-corruption crash

One deployed build stopped on 16 October 2026 with Windows exception
`0xc0000374` (heap corruption) in `ntdll.dll`, after about an hour of normal
capture. It is the only ScreenWise crash recorded in 14 days. The cause has not
been established: a crash dump exists locally but has not been analysed.
Corruption in a process that holds captured content is worth treating as a
security question until explained. This is register issue **SW-R20**.

Dump analysis classified it as a double free inside Windows UI Automation's
own I/O thread. At that moment ScreenWise's browser URL lookup was running three
times at once (once per monitor), each searching from the desktop root, while a
keystroke check tore down a provider connection after a .NET timeout. The
lookup now searches only the browser's own window, and a second walk of a
process already being walked waits for it instead of overlapping. A single dump
cannot prove the race, so SW-R20 stays open until it has gone without
recurring.

A separate worker cleanup defect was repaired in the 2026-10-18 candidate:
both input UIA workers previously called `CoUninitialize` while their UIA
interfaces were still alive. A thread-bound scope guard now releases those
interfaces first, including error and panic cleanup. Synthetic COM-apartment
tests passed without contacting app providers. This does not attribute the
recorded heap corruption or close SW-R20; see [cleanup scope](RESOURCE_MEASUREMENTS.md#uia-worker-cleanup).

## Open owner decisions with privacy impact

- **Remaining separate exclusion lists.** Some components keep their own lists
  of protected or excluded apps. Separate lists can drift apart.
- **Broad ignore patterns.** A pattern without `::` matches anywhere in an app
  name or title, so a short word can exclude far more than intended, or a
  misspelt one can silently exclude nothing. App-scoped patterns avoid this.

## Residual contact with excluded applications

Until 17 October 2026 the browser URL lookup searched from the desktop root
for the browser's process, so it walked the accessibility trees of every
window stacked above the browser, excluded apps included (register issue
**SW-R21**). It now opens only the foreground browser window. With that
repaired, native checks prevent explicit UIA probes and other covered calls
into recognised excluded targets. However, the global UIA focus-changed subscription
can still involve an excluded app's provider when Windows prepares an event,
even though the handler only invalidates state. This is a limit on a strict
"no provider contact" claim, not evidence that the subscription stores that
app's content.

A possible follow-up is a native WinEvent focus hook, with regressions for
privacy-state invalidation and a controlled provider-contact trial. See
[excluded-app contact boundaries](CAPTURE_PRIVACY.md#no-contact-with-excluded-apps-2026-10-16).

## A review question requiring a concrete reproduction

### Name and Value changes during UIA acquisition

UIA Name/Value changes no longer invalidate an ongoing walk. The implementation
rechecks acquired text against rules and reconciles policy/recording URLs, but
the optional recording-text phase can fail or be skipped during backoff.

The remaining question is whether a realistic document or URL transition can
hide an actual configured blocker across those paths and cause prohibited
content to be persisted. No such sequence was established in this review.
Removing invalidation alone is insufficient evidence of a leak. Equally, the old
behaviour was not a real protection: a UIA failure that happens to withhold a
frame, and so incidentally prevents a disclosure, is not a privacy improvement.
The question is whether the policy itself admits prohibited content.

A useful synthetic check must exercise a stable foreground identity, changing
document Name/Value evidence, optional-text failure/backoff and screening both
enabled and disabled, then assert which policy-relevant content is retained.
This is an unconfirmed review lead, not a confirmed vulnerability.

Sources: [invalidation properties](../crates/screenpipe-a11y/src/tree/windows_bulk.rs),
[privacy and optional-text reconciliation](../crates/screenpipe-a11y/src/tree/windows.rs).

## Deliberate boundaries that are not classified as defects here

Capturing an eligible window's covered surface, retaining accessibility text
beyond the visible viewport, explicitly configured UIA fail-open, trusting
listed untitled windows and retaining screened native titles on eligible
UIA-failure placeholders are documented policy choices. They can retain
confidential content, but increased retention alone does not establish an
unintended policy bypass.

In particular, the partly covered background-window path should not be described
as copying excluded-window pixels: it captures the eligible window's own
surface. A controlled visual test remains useful to verify the Windows capture
backend and compositing behaviour; source inspection is not a live result.

For additional historically recorded issues and validation limits, use the
[validation register](../VALIDATION_REGISTER.md). This review does not close
those issues or certify other capture paths, dependencies or platforms.
