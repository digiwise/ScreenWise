# Brief synthetic WGC overlay isolation check

## Minimal independent-process proof of concept (2026-10-09)

### Stored target state changes

The minimal runner also supports `prepare --state-changes`. This mode requires
a fresh run directory and new owner consent. It runs three bounded cases in
separate test processes: an ordinary non-topmost magenta foreground window
covering the target, minimising the target, and closing the target. Each case
first obtains the real production `WindowsCaptureTarget`, verifies initial
acceptance, stores the matching xcap window, and checks a cyan baseline image.
Only then does the fixture repaint green and perform the requested change.

The test verifies the changed native state, probes the stored xcap window and
then calls the real `verify_windows_capture_target` on the stored production
selection. This order preserves the raw probe's cached WGC item: enumeration
during production revalidation can drop xcap windows and remove that cache entry.
It reports old cyan, last-painted green, replacement magenta, blank, other or
capture-error outcomes as fixed labels and aggregate fractions. Green pixels
after minimisation/closure do not establish frame freshness. Raw results and
the subsequent production validator decision are recorded separately.

The locked xcap WGC frame receive has a 200ms timeout. The controller additionally
bounds each signal wait and test process, closes its exact owned process handles,
and each fixture self-closes after twenty seconds. No replacement-window or
monitor image acquisition is attempted, and no image is saved. The test invokes
the real production selector and validator but does not run recorder persistence,
the full production pipeline, UIA capture or an in-flight race. Metadata
enumeration by the production validator remains part of its normal behavior.

The previously passing opaque-overlay case remains a separate selectable mode;
its preserved executable, consumed consent and result are historical evidence.
They do not authorise any state-change trial.

The fresh-consented state-change extension passed in 9.10s on the owner desktop:
ordinary foreground cover returned last-painted green target pixels (1.0) and
no replacement magenta (0.0); minimise and close returned capture errors.
The real production validator rejected all three stored targets. Each baseline
control was 1.0, requested native states were verified, and no replacement or
monitor acquisition was attempted. No screenshots were saved or persistence
exercised. Evidence is private in `20261009-overlay-state-08`.
The preceding `state-07` launch failed at baseline with `0x80070424` under the
sandbox account. The owner's capture service was running. Metadata-only desktop
preflight and the unchanged fresh-consented retry passed outside the sandbox;
use that verified execution context. RootFmt and two offline tests passed,
with two live tests ignored by default; narrow compile 57.84s, all 433 external
artifacts reused and zero rebuilt. Production candidate binaries were unchanged.

The current minimal route is `SimpleOverlayFixture.cs`, `build-simple.ps1` and
`simple_overlay.py`. Earlier controller/fixture sources and the history below
remain available as failed-attempt context. This replacement uses two independent
forms: a native-maximised cyan target and a 160×160 topmost magenta overlay in
another process. It has no owner-click, foreground prerequisite, native owner
mutation or parent-process handle coupling. Each form closes after twenty
seconds; the runner also terminates only its own two process references in
`finally`. No click or typing is required.

Build using `build-simple.ps1 -OutputDirectory <absolute ignored directory>`.
Prepare using `simple_overlay.py prepare` with the same explicit `--run-root`,
`--fixture`, `--test-binary` and repeated `--runtime-dir` arguments shown below.
Preparation checks the fixture's inert self-test and Rust test listing and pins
the source, binaries and provisioned runtime DLLs. After fresh owner consent,
run `simple_overlay.py execute --run-root <prepared directory> --owner-ready
<exact new response>` on the owner's active interactive desktop. Preparation
does not create any windows or capture pixels.

The existing Rust test still checks separate PIDs, native maximisation,
visibility, geometry, topmost/Z order and both positive color controls around
two in-memory `xcap::Window::capture_image` acquisitions. It no longer requires
NOACTIVATE or a zero native owner: a framework owner in the independent overlay
process does not make that overlay part of the selected target process.
No images are saved. This proves only the stated synthetic opaque-overlay case
if the live test passes; it does not establish general window isolation.

Fixture compilation/self-test, Python syntax and canonical RootFmt passed.
The narrow canonical integration build passed two offline evaluator tests with
the live case ignored in 7.33 seconds: 433 external artifacts reused, zero
rebuilt; five workspace artifacts reused and two rebuilt. No live result is
claimed by preparation, and the old child's precise startup failure remains
unknown.

The replacement was subsequently run with fresh owner consent and **passed**:
3.53s total / 0.88s native test; target, overlay and covered-patch colour controls
were 1.0, overlay-colour fraction in the target was 0.0. Separate PID/native-max/
geometry/topmost/Z checks passed, and both owned processes closed. No screenshots
were saved. Private evidence is in the fresh `20261009-overlay-simple-06` run.
No window-state mutation during acquisition or complete engine/UIA claim follows
from this scoped opaque-overlay result.

Developer-only and unsupported. This helper does not start or stop ScreenWise,
use its database/API, acquire a monitor image, inspect accessibility, inject
keyboard/clipboard input, play audio, use network or change firewall state.
The public sources are newly authored locally; no upstream implementation was
copied. The existing locked xcap 0.9.4 `wgc` window path is called directly.

The interactive phase creates a maximised borderless cyan target and a separate-process
160×160 magenta topmost window over its centre. It acquires only
those two HWNDs with `xcap::Window::capture_image`. The separate overlay capture
must contain its ordinary positive control. The target must retain its cyan
positive control, including the patch beneath the verified overlap, and exclude
the overlay sentinel. A missing/black control is inconclusive. Native checks
verify each process identity, native maximised state, unchanged rectangles and the
overlay's styles/position above the target before and after each acquisition.
The minimal test performs two acquisitions: the selected target and the overlay
positive control. It does not require the target to be the global foreground
window, matching production selection of a visible native maximised window.
It does not mutate window state or force an in-flight state-change race.
Enumeration reads metadata; no other window image is acquired.

Images are evaluated in memory and discarded. The fresh ignored run directory
retains only source/runtime pins, numeric HWND/PID readiness, fixed verdicts,
aggregate pixel fractions and test runner output. No screenshots are saved.
The test can observe unexpected overlay pixels in memory if WGC isolation fails;
this uncertainty is part of the consent request. It does not prove isolation of
transparent/layered windows, owned popups, notifications, other backends or UIA.

## Inert preparation

Read the [shared readiness guide](../README.md). Finish/freeze affected crate
sources and wait for other Cargo work to release the canonical build lock.
No manifest, lockfile or production source edits are needed.
Read the [fixture readiness troubleshooting guide](../../../../docs/WINDOWS_FIXTURE_READINESS.md)
before preparing a retry. No earlier readiness response authorises another run.

```powershell
# Builds a fixture and runs its no-Form/no-native-UI self-test only.
.\scripts\windows\interactive-validation\window-overlay\build.ps1 `
  -OutputDirectory '<absolute ignored .local helper-build directory>'

# Use provisioned Python; controller tests use temporary synthetic files/mocks.
python -W error -m unittest discover `
  -s scripts/windows/interactive-validation/window-overlay -p test_overlay_control.py -v

# Inspect before compiling; the default run executes two pixel evaluator tests,
# and leaves the interactive case ignored. No window or capture is started.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest `
  -Package screenpipe-screen -TestTarget window_overlay_isolation -Jobs 1 -PlanOnly
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest `
  -Package screenpipe-screen -TestTarget window_overlay_isolation -Jobs 1
```

Select the exact integration-test executable from that successful canonical
build's artifact metadata, then independently review fixture/controller/test
source and provenance before approving the generated pins. If a matching test
cache baseline is absent, diagnose it and use the repository's reviewed
`-AllowColdCache` adoption policy; do not bypass the launcher or acquire tools.
Only that integration binary, not the recorder, is run for the live check.

```powershell
python scripts/windows/interactive-validation/window-overlay/overlay_control.py prepare `
  --run-root '<repo>/.local/interactive-window-overlay/<new unique run id>' `
  --fixture '<exact built OverlayFixture.exe>' `
  --test-binary '<exact built window_overlay_isolation test exe>' `
  --runtime-dir '<explicit provisioned DLL directory, if needed>'
```

Preparation checks SHA256 pins, fixture `--self-test`, a metadata-only active
console/window-station/input-desktop check, and test `--list`, then
writes a fresh readiness record and prints its exact owner response. DLLs in
explicit runtime directories are pinned too. No UI or capture runs. The
fixture uses the GUI subsystem and console test children use CREATE_NO_WINDOW,
so neither inert preparation nor the test runner creates a console overlay. The
exact live run ID and output path are checked by `--launch-preflight` through
the same Python launcher/environment, without directory creation or any window.
readiness wait is indefinite with the helper stopped. **Request fresh owner
consent only after preparation passes and source/runtime pins are reviewed.**
An inaccessible desktop must be diagnosed before requesting consent; the helper
never switches desktops or uses an implicit launcher workaround. Previous gates
and elapsed time cannot satisfy readiness. The running recorder is
untouched throughout; do not imply this helper has paused production recording.

## Fresh-gated execution

The owner should be ready for a brief foreground/focus change and two synthetic
windows on the active console desktop. No click or typing is needed. Current
fixture scope deliberately excludes remote/non-console sessions.

The optional **owner-click** fallback is a separate prepared mode and needs its
own fresh consent. Add `--owner-click` to the inert `prepare` command. Its scope
pin records eight seconds for click/readiness, eight seconds for the two
acquisitions, and a twenty-second fixture self-close failsafe. The mode and
bounds are hashed, and the fresh response starts `READY overlay-click`; execute
cannot switch a prepared mode. Use a new helper-build directory to preserve the
previous attempt's executable and private records.

After consent, the cyan target is temporarily topmost so it can be seen over the
current app. **Click the cyan background once within eight seconds**, away from
the magenta square. The target records its own left-button MouseDown, removes
its topmost state, and becomes ready with native maximisation and the separate
unowned overlay visible. The overlay remains
topmost. No synthetic input or AttachThreadInput is used. The UI pump remains
responsive while waiting. A missing click stops the owned
fixture before capture; another attempt requires another fresh gate.

```powershell
# Example shape only; this command is not consent.
python scripts/windows/interactive-validation/window-overlay/overlay_control.py execute `
  --run-root '<same fresh run directory>' `
  --owner-ready '<fresh exact response>' --reviewed-pins
```

Execution rechecks pins/inert preflight and atomically consumes that run's gate
before creating windows. Automatic focus/ready acquisition is bounded to three
seconds; owner-click readiness is bounded to eight seconds. The two-capture test
is bounded to eight seconds. The automatic fixture self-closes after twelve
seconds; owner-click mode has a twenty-second failsafe. The controller terminates
only its exact owned target process on completion/failure. Normal target closure
closes the exact overlay child; if the target is forcibly terminated, the overlay
observes its pinned parent process handle and closes on its next 100ms UI timer
tick. It also has a twenty-second self-close bound. The test process is killed on timeout. No live soak
or multi-minute wait is performed. A failed attempt is inconclusive unless
positive controls establish an actual sentinel leak; prepare a new run and
obtain new consent before any retry. Preserve all private evidence.

## Preparation evidence (2026-10-09)

Fixture compilation and no-UI self-test passed. The nineteen offline controller
regressions passed, covering exact/private path scope, changed pins, fresh
one-use readiness, review requirement and removal of old capture environment
from inert preflight, fixed startup-code sanitisation, process creation failure,
early exit, desktop/startup
rejection, non-native-maximised/overlay timeouts, readiness
identity mismatch, successful readiness and owned-child cleanup. These use
simulated clock values and mocked processes; no capture starts before readiness.
Owner-click regressions cover exact preflight/live argument parity, phase-specific
consent, pinned mode/bounds, missing click, failure to remove target topmost and
the eight-second deadline using simulated time. The C# no-UI self-test covers
the same readiness predicate without constructing a Form or calling native UI.
Canonical `RootTest -Package screenpipe-screen -TestTarget
window_overlay_isolation -Jobs 1` passed two offline evaluator tests and left the
interactive case ignored. The new integration selection required reviewed
one-time cache adoption after the missing-baseline gate; 433 external artifacts
were reused and zero rebuilt, with two workspace artifacts reused/five rebuilt.
Compiler time was 1m39s. Source/runtime pin generation and no-UI preparation
passed in a fresh private run directory. The metadata-only
desktop check failed in the restricted executor and passed after automatic
approval outside the sandbox. Preparation and fresh-consented execution must
use that verified active-desktop context.

The first fresh-consented attempt created its fixture output directory but never
delivered `ready.json`. The controller stopped the exact owned child before the
capture test ran. The old generic timeout did not record child exit code or
native maximised/foreground booleans; the actual startup/focus cause remains
unconfirmed. This establishes a diagnostic gap, not an isolation result.

The repaired helper explicitly requests native `SW_MAXIMIZE` and still requires
native `IsZoomed`, exact foreground ownership and a visible overlay. It writes
atomic fixed boolean readiness observations, catches UI startup exceptions
without a dialog, and emits only fixed startup-stage/desktop categories. The
controller separates process creation, early exit and readiness timeout, records
the numeric child exit category, discards arbitrary stderr, and preserves only
allowlisted startup codes and booleans. The next fresh gated run will distinguish
foreground refusal, incorrect native maximisation and startup exceptions.
If activation is refused, stop and prepare the documented owner-click fallback
with a new consent gate. No live retry or successful overlay isolation is claimed
by this repaired preparation.

The second consented automatic attempt confirmed **foreground loss after accepted
activation**: target and overlay visible, native maximisation true,
`activation_accepted=true`, but `foreground_matches=false`. It never reached
capture. The old category `fixture_foreground_refused` is preserved in that
private record; new diagnostics distinguish accepted activation followed by
loss from an initially refused activation. The first attempt's cause remains
unknown.

The third consented attempt reached owner-click readiness with native maximisation
and the target topmost state removed, but stopped before capture: the WinForms
overlay had a native owner, violating the independent-window positive control.
No isolation result was obtained. The next minimal fixture uses a distinct owned
child process for the magenta overlay. It avoids the taskbarless hidden owner,
explicitly clears the overlay's top-level owner after showing it, and verifies
`GW_OWNER==0` before readiness. [SetWindowLongPtrW's documented
GWLP_HWNDPARENT operation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowlongptrw)
sets the owner of a top-level window; it does not reparent a child window here.
Rust independently checks the separate PIDs, unowned overlay, geometry/Z-order
and native maximisation before and after both WGC captures. Global foreground
is diagnostic metadata only. Prior executables and consumed gates remain private
evidence; this repair needs new reviewed pins and fresh owner consent.

The minimal separate-process repair passed fixture compilation/no-UI self-test,
eighteen mocked controller regressions, and two canonical offline image evaluator
tests (interactive ignored). The narrow rebuild took 46.30s and reused all 433
external artifacts; five workspace artifacts were reused and two rebuilt.
Fresh mode-specific pins and exact-argument/desktop metadata preflight passed
outside the sandbox. No capture was run during this preparation.

The following consented attempt stopped before click readiness and pixel capture:
the separate overlay child exited before publishing its HWND. The parent exit
code was zero, but the child's exit code/output were not retained, so its actual
failure remains unknown. The diagnostic repair retains only that owned child's
numeric exit code, fixed startup stage and an allowlisted exception-type category.
Arbitrary output is discarded. Owner-clear failure now returns child code 5;
an exception returns 4. This repair passed compilation/no-UI self-test and nineteen
mocked controller tests. It leaves the Rust test and ownership checks unchanged;
no additional UI or capture attempt is claimed.
