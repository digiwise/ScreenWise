# Privacy and security issues

This companion to [privacy design and limits](PRIVACY_BOUNDARIES.md)
separates implementation gaps and review questions from deliberate recording
policy. Source review date: 17 October 2026, revision `68a1be820`. These entries
are not evidence that private content was exposed on a particular installation.
No new capture, live UI inspection or captured-content review was performed.

## Existing gaps confirmed in the current source

### URL exclusions do not cover every input record

**Scenario:** A developer excludes a banking or administration URL from screen
capture, but enables keyboard or clipboard database recording. Text typed or
pasted into that page can still reach input records because their persistence
path evaluates window ignore patterns without the frame URL policy. Frame
content screening does not screen input rows.

This is existing issue **SW-R01**. Keep keyboard/clipboard storage disabled or
use an app/window exclusion or explicit all-recording stop for that interval;
URL exclusion alone is insufficient. The fix needs URL-aware input admission
with failure handling and a synthetic excluded-page regression.

Sources: [input admission](../crates/screenpipe-engine/src/ui_recorder.rs),
[recording configuration](../crates/screenpipe-engine/src/recording_config.rs),
[existing register](../VALIDATION_REGISTER.md#system-wide-source-review-and-owner-priorities-2026-10-09).

### Monitor selection can expand instead of stopping capture

**Scenario:** A developer selects only an external display, then its identity
changes or it disconnects. When recording starts, if no selected monitor starts,
a compatibility fallback can start all connected displays, including a laptop
screen intended to remain outside capture. An empty selection is also treated
as permitting all monitors by the manager.

This is existing issue **SW-R03**. An empty selection is not a reliable stop
control. A future fix should preserve explicit selection, report unavailable
monitors and require an explicit choice before broadening scope.

Source: [monitor admission and startup fallback](../crates/screenpipe-engine/src/vision_manager/manager.rs).

### Schedule modality choices are not enforced

**Scenario:** A developer sets an audio-only work-hours rule expecting the screen
to remain unrecorded. The schedule monitor enforces whether the current time is
inside a rule, but does not apply its audio-only/screen-only modality selection.
Otherwise enabled sources can therefore remain admitted.

This is existing issue **SW-R02**. Disable unwanted sources explicitly; do not
rely on a schedule's modality label. A fix needs distinct source gates and tests
at rule transitions. Schedule transitions are also sampled periodically, rather
than enforced at an exact wall-clock instant.

Source: [schedule monitor](../crates/screenpipe-engine/src/schedule_monitor.rs).

### Removing media does not revoke all copies

**Scenario:** A developer views a frame, then deletes its recording interval or
evicts its media. Extracted frame files, background-retention cache state and
copies already held by clients can outlive the source operation. The ordinary
file-serving helper still emits `Cache-Control: public, max-age=604800`; the
separate PII-redaction response uses `no-store`.

This is existing issue **SW-R05**. The cache header does not by itself bypass API
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

## Residual contact with excluded applications

Native checks now prevent explicit UIA probes and other covered calls into
recognised excluded targets. However, the global UIA focus-changed subscription
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
Removing invalidation alone is insufficient evidence of a leak, and a UIA
failure that merely prevents otherwise permitted capture is not a meaningful
privacy guarantee.

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
