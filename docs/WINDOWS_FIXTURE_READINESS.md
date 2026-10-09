# Windows fixture readiness: prevent repeat inconclusive runs

This guide is for future agents preparing interactive ScreenWise fixtures. Read
[the interactive-validation guide](../scripts/windows/interactive-validation/README.md#reliable-windows-gui-fixtures)
and [AGENTS.md](../AGENTS.md#interactive-validation) first. This document grants
no consent, recording permission, desktop switching or sandbox exception.

## Recorded failure and what is actually known

On 2026-10-09 the consented window-overlay attempt did not produce its verified
`ready.json`. The controller stopped its exact owned fixture child before the
capture test ran. The result was inconclusive, not an overlay-isolation pass or
failure. The original controller used the same
`inconclusive_fixture_focus_or_desktop` error for an early process exit and a
readiness deadline. That message alone cannot establish the root cause.

The root cause of this attempt must be recorded by the investigating main task
after evidence distinguishes launch/argument rejection, desktop rejection,
startup exception, foreground refusal and incorrect maximised/readiness state.
Do not describe a suspected sandbox or focus issue as confirmed.

The second attempt supplied the missing native observations: target/overlay
visible, native maximisation true and activation accepted, but foreground did
not match. That establishes loss of foreground after accepted activation.
The third, separately consented owner-click attempt passed those predicates and
removed target topmost, then failed the independent-overlay ownership assertion
before acquiring pixels. WinForms had assigned an implicit owner during Show.
Clear and verify native overlay ownership before publishing readiness; keep the
capture test's independent-window requirement. Neither failure is an isolation
pass. The first attempt's cause remains unknown.

A fourth attempt's overlay child exited before publishing its HWND. Its own
exit/output were not retained, so that exact cause is unknown too. At the owner's
request, a gpt-6-astra agent replaced the coupled fixture with two independent
simple forms and the existing WGC test. Removing unnecessary owner mutation,
parent-handle coupling, click and global-focus requirements produced a fresh-
consented pass in 3.53s: all colour controls 1.0, overlay colour in target 0.0.
Both owned processes closed. Prefer this minimal proof of concept for this
specific pixel-isolation question; do not carry forward unrelated fixture
predicates as capture requirements. Keep checks needed to establish valid
positive controls, native identity/maximisation/geometry/Z-order and cleanup.

## Before asking for fresh consent

The subsequent stored-target extension exposed a launch-context regression:
all three baseline WGC acquisitions failed with `0x80070424` under the sandbox
account, before any state mutation. The owner's capture service was running;
the literal service error was not evidence of an uninstalled service. The
unchanged test passed all three cases in 9.10s after metadata-only desktop
preflight and fresh consent in the owner context. Preserve executor/account
parity for preparation and execution, including WGC access, not only session ID.

1. Reuse the documented fixture/launch path. Compile and run no-UI self-tests,
   then offline controller tests. Exercise live argument validation and output
   directory expectations without constructing a window. A parser-only self-test
   does not establish that the actual controller invocation will be accepted.
2. Run metadata-only desktop preflight through the **same launcher, executor,
   account/session and environment** that will launch the live child. Check the
   child process's own window station, thread desktop and active input desktop.
   A successful preflight outside the sandbox does not validate a later launch
   inside it. Do not silently cross this boundary; use the reviewed, permitted
   execution context and diagnose a rejection before seeking readiness.
3. Verify the target fixture's state model: a borderless maximised-looking
   rectangle must not be assumed to satisfy native `IsZoomed`. Keep native
   maximised state and geometry as separate booleans. Do not silently replace
   the production predicate with a looser test-only predicate.
4. Pin the settled source, helper, test binary and runtime inputs. Prepare a fresh
   private run directory and readiness record. Keep fixtures stopped and hidden
   while awaiting the owner's fresh response.

## During a consented attempt

Use the existing short, bounded foreground/readiness window. Keep the UI message
pump responsive. Require the exact target HWND/PID and intended state; a process
existing or `IsWindowVisible=true` is insufficient evidence of owner visibility
or successful activation. Preserve the capture test's positive controls.

Report safe, fixed categories separately:

- Process creation failure.
- Early fixture exit, including its fixed numeric exit category.
- Active-desktop rejection.
- Startup exception, with a fixed stage/category rather than arbitrary exception
  text, window titles or private paths.
- Readiness timeout, distinguishing wrong foreground, not maximised, overlay not
  ready and readiness-file delivery failure using booleans/counts only.

The original overlay fixture's explicit startup return codes were: `2` for
invalid invocation/output location, `3` for inaccessible/wrong desktop, and `4`
for a caught startup exception. Verify these against current source; do not
discard them behind one generic timeout. Prefer a content-free final phase
record written by the helper, including its safe exit/stage information.

If Windows refuses activation, stop the owned fixtures and use the documented
owner-click fallback only with a new readiness gate. Increasing the timeout or
reusing consent is not a fix for a wrong desktop or invalid invocation.

## Required regression checks and handoff

Use mocked/injected process results and a simulated clock for controller cases:
early exit, desktop rejection, startup failure, wrong foreground, non-maximised
target, ready identity mismatch and successful ready delivery. Assert that no
capture starts before verified readiness and that failure stops only owned
children. These checks must not show UI or consume owner readiness.

After an inconclusive run, preserve evidence and consumed readiness. Record the
confirmed cause, exact repair, relevant offline checks, verified execution
context and limits in the fixture README/validation record; link this guide.
Refresh changed pins and request fresh consent before any interactive retry.
Do not claim successful capture isolation until positive controls and the actual
window-only capture result establish it.
