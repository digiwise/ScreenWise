# Maximised-window capture plan (2026-10-09)

The owner authorised a minimal Windows implementation: screenshots and
accessibility use the same selected maximised top-level window on each monitor.
Accessibility keeps its current full-tree behaviour within that window. UIA may
therefore expose covered or off-screen elements belonging to the selected app;
this scope was explicitly accepted. No general occlusion analysis is added.

## Selection and admission

Sample bounded native Z-order for each monitor. The first relevant visible,
non-minimised, non-cloaked regular window must be maximised. Small always-on-top
overlays are ignored during selection, as requested. A topmost window covering
the full monitor counts by geometry, even without a maximised flag; a lower
maximised window cannot bypass it. Ambiguous native samples fail closed.

The selected window need not have global focus. A visible maximised window on
one monitor remains eligible when focus is on another monitor. Screenshot/UIA
identity and selected metadata do not switch to the globally focused app; tests
cover foreign focus metadata and exact target propagation. The fixture's focus
readiness check is a test setup requirement, not a production selection rule.

Evaluate app/title/include/URL and built-in restrictions for the selected window.
Keep session, privacy-generation, lock, schedule and user-preference guards.
Missing selected metadata or an unverifiable selected browser URL cannot grant
capture. Lower covered windows do not decide this selected window's admission.
For non-foreground browsers, existing foreground URL lookup cannot be assumed
to describe the selected HWND; unavailable verification remains fail closed.

Acquire the existing locked xcap WGC window item for that exact HWND, composing
only its image onto a black monitor canvas. Never crop a monitor image or retry
with unrestricted monitor capture. Carry the selected HWND/PID into UIA and start
from that root; there is no fallback to a different foreground window. OCR uses
the same selected image. Selected metadata labels and budgets use that target.

Before and after acquisition/UIA and before persistence, require the same
selection, HWND/PID, monitor geometry, maximised state and relevant Z-order.
Changed samples discard the result. Existing generation and title/URL rechecks
remain. Endpoint samples cannot prove there was no change and return between
checks; window-specific acquisition is intended to isolate pixel source even
when another app changes Z-order. The synthetic check below tests that scoped
property, without claiming continuous observation or a forced in-flight race.

## Minimal implementation and diagnostics

Production changes cover screenshot selection/acquisition, TreeWalkerConfig and
Windows UIA root selection, and event-driven engine target propagation, with
required call-site updates and regressions. A serde-skipped runtime target
carries native identity without exposing HWND/PID in frame/API metadata.
Deliberate scoping reports active_window_only with fixed reason
maximised_window_only / reason code capture_maximised_window_only. The UI labels
this "Maximised window only", without inventing an exclusion rule or blocker.
Other restrictions retain their own reasons and rule references.

Deterministic tests cover stable selection, identity/PID/maximised/Z-order
changes, ignored small topmost overlays, geometrically full-screen topmost
windows, and screenshot/UIA target propagation. No dependency or lockfile change
is planned; no commercial Screenpipe or Litepipe implementation is copied.

## Brief synthetic source-isolation check

The locked xcap 0.9.4 Windows wgc feature creates a window-specific
GraphicsCaptureItem. Empirical isolation still requires the owner's fresh
consent. The [prepared helper](../scripts/windows/interactive-validation/window-overlay/README.md)
creates only an opaque cyan target and a small magenta topmost
overlay in a separate process. Two fixed window-only acquisitions verify the
overlay positive control and target content underneath the overlay. This minimal
test does not mutate Z-order/window state during acquisition or require global
foreground focus. State/identity changes retain deterministic coverage. There is no
monitor acquisition, input injection, audio, recording session or live soak.

Images are evaluated in memory and discarded; only aggregate verdicts and pins
are retained privately. Compile and inert self-tests precede runtime/source pin
review. Metadata-only desktop preflight passed outside the restricted executor.
Ask for consent only after final preparation succeeds, then consume one fresh
readiness gate. Waiting leaves fixtures stopped. Failures need a new gate.

Four freshly consented attempts stopped before capture started. The first cause
was not recorded; the second lost foreground after accepted activation; the
third passed owner-click readiness but WinForms had assigned an implicit overlay
owner; the fourth child exited without retained failure details. The controller
stopped owned fixtures; those attempts produced no isolation evidence.
The owner requested a simpler test: an overlay in a separate process, exact
window/PID/maximisation/bounds/Z checks and colour controls, without unnecessary
global-focus prerequisites. The owner then authorised a gpt-6-astra agent to
replace the fixture from scratch. Its minimal independent forms avoid owner
mutation and parent-handle coupling; no click is needed. C# self-test/Python syntax,
two Rust evaluator tests, RootFmt and ten pins passed. The narrow rebuild took
seven seconds, reusing all 433 external artifacts with zero rebuilt. The owner
provided fresh consent and ensured no sensitive content was on screen.

The replacement passed in 3.53 seconds: target, overlay and underlying patch
positive-control fractions were 1.0; magenta fraction in the selected target
image was 0.0. Native separate-PID/maximisation/geometry/topmost/Z checks passed,
and both owned processes closed. Images were discarded in memory. This is scoped
empirical evidence for an opaque overlay in another process, using the production
window-capture primitive; it does not prove continuous title/URL/state checks,
forced in-flight changes or complete engine/UIA behaviour.

This check does not prove isolation for transparent/layered surfaces, owned
popups, notifications, other backends or UIA. Any concrete new privacy trade-off
requires the owner's decision. The scoped result is developer evidence, not a
general privacy guarantee. Clancy deploys the final candidate; the separate
client performs production event-rate checks.

## Stored target after pre-acquisition state changes

The freshly consented extension passed three cases on the owner desktop in
9.10 seconds. Each case stored the real production selection and verified a
cyan baseline before changing native state. An ordinary non-topmost foreground
cover left raw WGC returning only the target's last-painted green pixels
(fraction 1.0, replacement magenta 0.0). Minimising or closing the stored target
produced capture errors. The real production `verify_windows_capture_target`
rejected all three changed targets. No replacement-window or monitor acquisition
was attempted, and no images were saved.

This distinguishes pixel isolation from conservative selection validation:
the tested ordinary cover did not appear in raw target pixels, but a changed
selection is still discarded by production. It does not prove recorder
persistence, continuous freshness, title/URL changes between checks or mutations
during acquisition. The first extension launch failed at baseline under the
sandbox account; the unchanged retry passed after metadata-only preflight and
fresh consent in the owner desktop context. Both attempts remain recorded.

## Owner decision: retain state checks as defence in depth

Keep the existing checks for another relevant window appearing in front,
minimisation, closure and related selection/state changes for now. The scoped
tests show target-only pixels or capture errors in these cases; the additional
checks remain conservative defence in depth, rather than the demonstrated
mechanism preventing replacement-window pixels.

If these checks cause false rejections or other bugs, explicitly consider
removing or narrowing the affected check instead of automatically repairing it
or adding more guards. This is permission to consider that option, not a current
runtime removal. Distinguish redundant selection/state checks from target
identity and current content/privacy admission checks; review the concrete
trade-off before changing the latter. The existing candidate remains unchanged.
