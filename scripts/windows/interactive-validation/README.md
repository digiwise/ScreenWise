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
