# Windows interactive-validation kit

> **Developer-only and unsupported.** This kit is for maintainers who can audit
> and adapt it themselves. The project owner and Digiwise provide no setup,
> validation, troubleshooting, or operational support for it.

This directory contains the reusable Windows controllers, synthetic fixtures,
generators, launchers, and offline tests developed for ScreenWise interactive
validation. The scripts are inert by default. They do not start the recorder,
open UI, play audio, lock Windows, change firewall rules, or download anything
unless a reviewed live run is prepared and explicitly authorized.

The archived sources were authored locally. No post-MIT Screenpipe source,
current commercial source, or Litepipe source was fetched or copied. The
`original_sha256` fields in `source-manifest.json` preserve the hashes of the
September 2026 source snapshot. The `sha256` fields cover the current portable
adaptations. Historical working copies and private run evidence are not part of
this kit and must never be used as current authorization or current evidence.

## Verify and stage sources

`stage.py` checks every allowlisted UTF-8 source, rejects traversal and reparse
points, and verifies the source manifest. Its default operation is read-only:

```powershell
python .\stage.py --check
python -W error -m unittest -v .\test_stage.py
```

Staging is a separate explicit operation. Run it from this directory and point
it at the root of the current `screenpipe` clone:

```powershell
python .\stage.py --stage --repo-root ..\..\..
```

The historical target directory names are retained because the tail and lock
controllers have a reviewed dependency layout:

| Archive tree | Staged tree |
| --- | --- |
| `prep/**` | `target/interactive-prep-20260915-01a09e45/**` |
| `tail/**` | `target/background-meeting-20260916-01a09e45/**` |
| `lock/**` | `target/lock-transition-20260916-01a09e45/**` |
| `common/**` | `target/interactive-validation/common/**` |

The names are compatibility paths only. Staging works from an arbitrary clone
directory and contains no user name or machine-specific repository path.
Existing files with different bytes make the whole preflight fail before any
copy. The tool never overwrites, deletes, or executes a file.

## Configure one machine

Copy `common/config.example.json` to an ignored local file such as
`target/interactive-validation/config.local.json`. Replace every angle-bracket
placeholder. A live run fails closed when any required field is missing.

The configuration names these machine-specific inputs explicitly:

- absolute repository, Python, PowerShell 7 (`pwsh.exe`), Visual Studio developer-shell,
  OpenBLAS, ONNX Runtime, and release-directory paths;
- exact input and output device identities and two distinct local ports;
- the SID of the intended non-elevated recorder account;
- the existing outbound-block firewall group and its exact remote-address
  scope; and
- reviewed SHA-256 pins for the recorder, FFmpeg, FFprobe, native libraries,
  local models, and shared launcher helper used by the selected run.

The scripts do not discover or install a Visual Studio edition, Python runtime,
native library, model, device, or firewall rule. Prepare those dependencies
outside the kit, record their exact paths and identities, and review the pins.
The preflight only checks the supplied configuration and existing machine state.
It also hashes the shared launcher helper that the live controllers execute and
fails before creating evidence if that file does not match the configured pin.
It requires an enabled, enforced outbound block for each configured executable;
it never weakens, bypasses, creates, or modifies a firewall rule. API
authentication remains enabled and is tested with missing, wrong, and matching
bearer tokens.

The sample hashes are placeholders by design. Do not replace them by blindly
hashing whatever binary happens to be present. Establish artifact provenance,
build or acquire the intended version, review it, and only then record its
SHA-256. `package_preparation.py` can produce a source inventory or an explicit
generated-artifact candidate, but its schema is
`screenwise.prepared-controller-pins-candidate.v1`; live controllers reject it.
Review the candidate independently before producing the controller's required
final pin manifest. The helper creates no owner gates.

## Rebuild generated prerequisites

The archive intentionally omits EXEs, WAVs, databases, logs, captures, models,
tokens, owner nonces, pin manifests, and results. After source-only staging,
rebuild only what the chosen mode needs:

```powershell
$repoRoot = (Resolve-Path ..\..\..).Path
Push-Location $repoRoot
& .\target\interactive-prep-20260915-01a09e45\fixtures\build.ps1 -SelfTest
& .\target\interactive-prep-20260915-01a09e45\audio\generate.ps1
& .\target\interactive-prep-20260915-01a09e45\audio\validate.ps1
& .\target\background-meeting-20260916-01a09e45\generate-speech.ps1
python .\target\background-meeting-20260916-01a09e45\validate-speech.py
Pop-Location
```

The fixture builder accepts `-Compiler <absolute csc.exe path>` if the Windows
Framework compiler is elsewhere. Generation writes files only; it performs no
playback. Review every generated artifact before creating final pins.

## Run offline tests

These suites use mocks and temporary files. They do not start recording, UI,
playback, Windows lock, firewall work, or downloads:

```powershell
$repoRoot = (Resolve-Path ..\..\..).Path
Push-Location (Join-Path $repoRoot 'target\interactive-prep-20260915-01a09e45')
python -W error -m unittest -v test_plan.py
Pop-Location

Push-Location (Join-Path $repoRoot 'target\interactive-prep-20260915-01a09e45\controller_v1')
python -W error -m unittest -v test_coordinator.py test_drm_sequence.py test_evidence.py test_fixture_transport.py test_live.py test_windows_controls.py
Pop-Location

Push-Location (Join-Path $repoRoot 'target\background-meeting-20260916-01a09e45\preparation')
python -W error -m unittest -v test_tail_acceptance_60s.py test_tail_acceptance.py test_tail_collector_60s.py
Pop-Location

Push-Location (Join-Path $repoRoot 'target\lock-transition-20260916-01a09e45')
python -W error -m unittest -v test_lock_acceptance.py test_lock_live.py
Pop-Location
```

### Synthetic pixel-redaction evaluator

`prep/pixel_redaction_eval.py` is the pure evaluator used by the fixed synthetic
pixel API check. It compares a baseline and result image at caller-supplied,
disjoint PII, ordinary-control and unrelated-sentinel rectangles. A pass needs
the high-contrast PII rectangle to change and lose most of its local contrast,
while the ordinary and sentinel rectangles stay within a per-pixel compression
tolerance. Its separate failure
comparison requires a source-free replacement across the frame and within the
PII rectangle. Output contains only fixed statuses, booleans and aggregate
fractions; it never emits pixel values or image data. It requires Pillow in the
configured Python runtime. The offline tests make
temporary synthetic JPEGs and do not start the recorder or GUI:

```powershell
$repoRoot = (Resolve-Path ..\..\..).Path
Push-Location (Join-Path $repoRoot 'scripts\windows\interactive-validation\prep')
python -W error -m unittest -v test_pixel_redaction_eval.py
Pop-Location
```

The fresh-gated `pixel-privacy` controller starts the current release with audio,
vision, keyboard and clipboard capture disabled, inserts only fixed synthetic
JPEG and OCR geometry into its new controller-owned database, and fetches the
three cases through the authenticated loopback `/frames/:id?redact_pii=true`
route. The cases are an ordinary clear-frame control, one fixed redaction box,
and a second frame with deliberately absent OCR metadata that exercises the
normal source-free failure response. It checks missing/wrong/valid bearer status,
allowlisted redaction headers, aggregate pixel metrics, clean shutdown, listener
closure and exact-path process quiescence. It opens no test window and requires
no owner action after the fresh readiness response. The normal UI recorder can
still write focus, app-switch and fixed privacy-status metadata; do not inspect
those row bodies. This establishes neither OCR
recognition accuracy, async model-worker behavior, general PII coverage nor a
privacy guarantee.

## Prepare and authorize a live run

Live work requires all of the following, in this order:

1. Source archive verification and source-only staging.
2. Rebuilt prerequisites and independently reviewed runtime/source pins.
3. A passing `preflight.ps1 -ConfigPath <local-config>` while the recorder and
   fixtures are stopped.
4. A new unique run ID and a newly created, unconsumed readiness record.
5. Review of the exact launcher, controller, evaluator, configuration, pins,
   intended UI/audio/lock phases, and cleanup behavior.
6. The owner voluntarily supplies the fresh phase-specific response. Waiting
   remains indefinite; elapsed time is never consent.
7. `-ExecuteInteractive`, `-ConfigPath`, the fresh `-OwnerReady` value, and the
   same run ID are supplied together to the chosen launcher.

Example shape only; it is not authorization to execute:

```powershell
& .\launch.ps1 -RunId '<new-run-id>' `
  -ConfigPath '<absolute local config path>' `
  -OwnerReady '<fresh exact response>' `
  -ExecuteInteractive
```

Without `-ExecuteInteractive`, launchers only print a preview. Without a fresh
owner response or complete configuration, execution stops. A previous response,
archived readiness file, test result, or old evidence never authorizes a new
run. Readiness waits have no timeout and must keep recording stopped and fixtures
hidden. Unexpected state requires safe stop and a new gate.

## Reliable Windows GUI fixtures

A fixture used as live privacy evidence must run on the owner's active
interactive desktop. `IsWindowVisible` can be true for a window created in a
background or otherwise inaccessible desktop, so it is only a diagnostic. After
the readiness gate is consumed, create the compact fixture with explicit app,
visible and topmost styles, show and position it, and then verify both the
foreground top-level HWND and the intended focused control. Use a short bounded
focus timeout after launch. If Windows refuses activation, stop safely and use a
new readiness gate for a deliberate owner-click fallback; do not leave an active
recording waiting indefinitely behind another window.

Keep the fixture's UI message pump responsive throughout UIA inspection. In
particular, do not call `SendKeys.SendWait` or perform a complete synthetic input
burst on the target UI thread: doing so can block the UIA provider, expire a
valid privacy decision and create a false application failure. Generate input
from a background worker while the UI thread continues to dispatch messages,
and confirm the intended control still owns focus before the first input.

Native `SendInput` declarations must match the Windows ABI. The `INPUT` union
must accommodate its largest member, including `MOUSEINPUT`; on 64-bit Windows
the complete `INPUT` structure is 40 bytes. Fixture self-tests must assert the
native structure size, require `SendInput` to report every submitted entry, and
record a content-free Windows error when it returns zero or a partial count. An
input-injection failure ends the fixture phase before ScreenWise results are
interpreted.

Use short fixed non-sensitive ASCII markers and deterministic pacing. A valid
privacy result requires all of the following: successful injection, persistence
of an ordinary-field positive-control marker, no protected marker, and the
expected positive suppression-counter delta. Absence of a protected marker by
itself is inconclusive. Prefer two ordinary/password focus cycles so the test
also exercises recovery after leaving a protected field. Keep timing evidence
content-free: probe duration, completed-probe gap, decision age and categorized
suppression counts are sufficient to distinguish target-provider stalls,
scheduler delays and ScreenWise admission failures.

Compile and self-test fixtures before refreshing manifest hashes and prepared
pins. Run the offline controller/archive suites and normal-session preflight
before requesting readiness. Treat desktop launch, focus acquisition, target UI
thread stalls, ABI/input injection and positive-control failures as harness
failures, distinct from a verified ScreenWise privacy failure.

## Optional spoken lock cues

`say-status.ps1` accepts only `lock-now`, `unlock-now`, `finished`, or `aborted`.
It prints a fixed phrase by default and speaks only with explicit `-Speak`:

```powershell
.\say-status.ps1 -Status lock-now
.\say-status.ps1 -Status lock-now -Speak
```

Use speech only when the owner explicitly opted into it for that run. Keep it
off during readiness waits, meetings, audio measurements, and any scope that
excludes playback.

## Scope and limits

The controllers preserve bounded aggregate checks developed during the 2026
Windows validation work. Prior runs informed their failure handling, but they do
not prove a current build, machine, firewall, privacy path, meeting, audio route,
or lock recovery. A source-only stage has no compiled fixtures, media, runtime
pins, or current consent. Live results remain scoped to the selected mode and
the evidence the evaluator actually checked.

The prepared native `input-privacy` mode uses only fixed synthetic typing and
clipboard values. It requires a positive ordinary-field UI-event marker, zero
password-marker persistence, and a positive numeric
`uia_password_content_suppressed` counter delta. Log parsing returns only that
fixed reason's aggregate number. The prepared tail controller uses three locally
generated, hash-pinned WAVs and a separate 16-asset pin manifest. Neither mode
may run without a fresh gate and normal-user preflight.
