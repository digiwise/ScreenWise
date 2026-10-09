# Local Windows deployment

## TLDR

From the repository root, quit any deployed ScreenWise instance, then run:

```powershell
# Reuse built release-local binaries and start when all checks pass.
& .\scripts\windows\deployment\Deploy-ScreenWise.ps1
```

For first-time firewall preparation, or to stage without capture, use
`Deploy-ScreenWise.ps1 -PrepareOnly`. If startup reports missing rules, run
`Set-ScreenWiseDeploymentFirewall.ps1` explicitly in elevated PowerShell, then
run `Start-ScreenWise.ps1 -PreflightOnly` and `Start-ScreenWise.ps1` from ordinary
PowerShell after confirming the rule output.

Leave Ollama running on loopback with `ministral-3` already installed. Pi is
included with read-only recording retrieval; no shell/file-writing tools are
enabled. Data and chats persist in `.local/production-data`. Quit using the tray.
Later updates can use `Deploy-ScreenWise.ps1 -Build`; startup at login is opt-in
with `-RegisterStartup`. Nothing downloads unless you explicitly request
`-ProvisionPi` for a missing package. Full details follow.

To see the API evidence DigiTrack could retrieve, while ScreenWise is running,
use PowerShell 7:

```powershell
pwsh -NoProfile -File .\scripts\windows\evidence-review\Export-ScreenWiseEvidence.ps1 `
  -Start '2026-10-06T10:00:00+11:00' -End '2026-10-06T10:30:00+11:00' `
  -IncludeFrameContext
```

Open the printed output directory's `index.html` locally. JSON files retain the
records and source references; incomplete retrieval is flagged. This makes no
model calls or uploads. [Exporter options and limits](../evidence-review/README.md).

---

## Full deployment guide

`Deploy-ScreenWise.ps1` prepares a persistent developer installation from the
selected build outputs, separate from Cargo build directories. The default is
`release-local`; use `-BuildProfile release` to select an explicitly requested
full release candidate. It is not
an installer or a claim of production readiness. See the repository README and
validation register for the experimental/support boundaries.

From the repository root in ordinary PowerShell:

```powershell
& .\scripts\windows\deployment\Deploy-ScreenWise.ps1 -PlanOnly
& .\scripts\windows\deployment\Deploy-ScreenWise.ps1
```

By default this reuses already-built `release-local` artifacts and automatically
starts the installed version only after the startup launcher accepts all checks. It copies the GUI,
recorder, Bun, FFmpeg/FFprobe, native DLLs and GUI assets to
`.local/deployment/release`. It reuses the private WebView2 directory from the
existing `.local/gui-trial-runtime/runtime.json`, without copying it. Required
artifacts must have been explicitly provisioned; nothing is downloaded.
The deployed directory keeps its existing name for compatibility: the manifest
records the actual build profile. Switching profile preserves installed executable
paths, startup registration and firewall rule identities. Both legacy `release`
and new `release-local` manifests are accepted; there is no fallback to another
profile if an artifact is missing. Inspect `-PlanOnly` for the selected source paths.
Models are reused from the application's existing model stores, never copied.
Pi **is included** through an explicit shared package path, pinned to 0.75.4.
Deployment reuses the previous deployment's path, the versioned shared store
`.local/pi-runtimes/0.75.4/pi-agent`, or the single remaining provisioned package
under `.local/pi-execution-validation`. It never copies the package into each
data directory. Use `-PiRuntimePath` to select a package explicitly. Missing or
ambiguous runtimes fail before deployment; `-ProvisionPi` explicitly downloads
the pinned package once through the existing provisioner. App version/dependency
checks remain, and invalid explicit paths never fall back to another package.
Configuration, chats and sessions remain separate under the recording data root.
Treat the shared runtime as immutable; timestamp/size changes require redeployment.

The deployment mode gives Pi only the owned `screenwise_recordings` tool:
authenticated loopback GET queries for screen/UIA/OCR, elements, audio, input,
meetings/transcripts, frame context/metadata and safe notices. Requests require
explicit time bounds; raw media, arbitrary URLs, API mutations and built-in
bash/read/write/edit tools are unavailable. Captured content remains untrusted
evidence. [Tool scope and limits](pi/README.md) apply; retrieval may be partial.

Deployment starts the installed version automatically through `Start-ScreenWise.ps1`
after all existing firewall and startup checks pass. `-PrepareOnly` stages files
without checking startup prerequisites or starting capture. No deployment mode
creates or changes firewall rules. If startup fails, the newly prepared installation
remains available; the command reports the prerequisite failure. Resolve it and
run `Start-ScreenWise.ps1` rather than rebuilding or redeploying.
The first deployment creates a unique group for nine exact executable paths:
six ScreenWise/runtime executables and the installed Ollama app, server and model
runner. Existing Ollama rules remain; new deployment rules do not replace them.
Existing rules for Cargo build outputs do not cover deployed executables.
In **elevated PowerShell**, install the new rules:

```powershell
& .\scripts\windows\deployment\Set-ScreenWiseDeploymentFirewall.ps1
```

Inspect the printed rules and confirm all nine are found in ActiveStore before
launching. The script blocks remote IPv4 outside 127.0.0.0/8 and IPv6 outside
::1, for all protocols/profiles; loopback remains available. It refuses conflicting
same-name rules. Valid existing rules are verified without replacement. A failed
installation rolls back only rules created by that invocation.

Then in **ordinary PowerShell**:

```powershell
& .\scripts\windows\deployment\Start-ScreenWise.ps1 -PreflightOnly
& .\scripts\windows\deployment\Start-ScreenWise.ps1
```

The launcher verifies runtime file sizes/timestamps, exact active rules, enabled
firewall profiles, an unused API port, local audio models and the pinned Pi
package before capture. Ollama must be running only on loopback port 11434, with
the local `ministral-3` model present. Neither Ollama nor its model is downloaded
or started implicitly. Its API/data/model stores remain those already configured.
Metadata comparison follows the owner's local update preference; it is not
cryptographic tamper detection. The production API port defaults to **31579**.
Bearer authentication remains enabled. Capture uses all monitors, default audio,
local Parakeet transcription, keyboard and clipboard capture, PII redaction,
DRM pause and lock suppression. Firefox and Excel exclusions are seeded on first
launch; change initial patterns with deployment's `-IgnoredWindow` option.
Subsequent launches preserve GUI settings and records. Do not remove exclusions
for a password manager merely to obtain more evidence.

Data, chats, SQLite stores and diagnostic logs persist in `.local/production-data`.
This is separate from deployed binaries and trial directories. Quit through the
tray for clean shutdown. Closing the launch PowerShell does not stop the GUI.
The launcher itself is not the trial harness: it does not continuously sample
processes/endpoints or generate a full trial audit. Existing application safe
status/logging and crash markers remain; absence of a warning does not establish
complete capture or firewall enforcement. A scoped live deployment check remains
necessary before claiming runtime validation of these new paths.

To rebuild and update later, quit the deployed app first, then:

```powershell
& .\scripts\windows\deployment\Deploy-ScreenWise.ps1 -Build
```

`-Build` exports the frontend with provisioned Node/packages, skipping acquisition
prebuild hooks, and invokes the canonical locked/offline root and desktop builds
with the selected `-BuildProfile` and `-PlanOnly` before each. Full release builds
are reserved for explicit owner requests or inadequate local-profile performance;
ordinary deployment does not require them. Dependency provisioning is
separate. Running installed processes prevent an update; the script never kills
them. Existing data and previous application binaries/assets are retained. After
a successful replacement, `release-previous-*` archives exclude `bun.exe`,
`ffmpeg.exe`, `ffprobe.exe`, `libopenblas.dll` and `onnxruntime.dll`. Failed
replacement restores the complete previous runtime; manual rollback from a
retained archive requires separately provisioning those third-party binaries.
To reuse the
installation at Windows login, explicitly pass `-RegisterStartup`; it registers
the same fail-closed launcher under your HKCU Run key, using the console
PowerShell executable that ran deployment. It refuses a restricted/AllSigned
policy rather than changing policy or silently bypassing it. `-Launch` remains a compatibility alias for the default automatic startup; it cannot
be combined with `-PrepareOnly`. Startup
requires this checkout and its reused WebView2 directory to remain in place.

To remove **only this deployment's rules**, use elevated PowerShell:

```powershell
& .\scripts\windows\deployment\Set-ScreenWiseDeploymentFirewall.ps1 -Remove
```

The script verifies the expected identities before removal and absence afterward.
It does not reset Windows Firewall or delete other groups. The launcher refuses
to start until its rules are installed again. All generated/private state stays
under `.local/`; none belongs in a public commit.

Background synthetic regression checks (no recording, firewall mutation, startup
registration, model downloads or GUI launch):

```powershell
& .\scripts\windows\deployment\Test-ScreenWiseDeployment.ps1
```

## Sensitive diagnostics option

This local deployment batch defaults `-SensitiveDebugLogging $true` for the owner's
requested diagnostic period. It records the boolean in the deployment manifest;
startup enables `SCREENWISE_SENSITIVE_DEBUG=1` only for the GUI child process.
Use `-SensitiveDebugLogging $false` when preparing a deployment to turn it off.
Older manifests without the option remain off, and startup removes any inherited
opt-in before applying the manifest. This does not edit settings in a running app.
Sensitive diagnostics are separate from ordinary safe notices and logs; they may
contain private decision context and must not be published. See
[the sensitive diagnostics guide](../../../docs/CAPTURE_PRIVACY.md#sensitive-decision-diagnostics-2026-10-09) for storage and limits.
Preparation here does not start a diagnostic capture or inspect existing captures.

## Prompt Parakeet transcription

New deployment stores use `transcriptionMode: "realtime"` with the existing
Parakeet engine and 30-second audio chunks. Completed chunks are processed during
calls as well as outside audio sessions. Parakeet inference shares the existing
model mutex; this does not enable the separate live-meeting provider or add a
second model. Longer audio is still split into 30-second inference windows.

Existing stores keep their explicit settings. To select this behaviour in an
existing deployment, turn **Batch Transcription** off in Recording settings;
the existing settings flow applies the change through a recording restart.
Keep Parakeet, the 30-second duration and the disabled live-meeting provider.
Do this at an agreed interruption point, after deploying the recovery change.
Editing a running application's `store.bin` directly is not part of this procedure.

Both realtime and batch modes own the same durable recovery worker. It skips
active audio sessions, considers unfinished chunks older than ten minutes and
waits two minutes between sweeps. The ten-minute threshold affects fallback
recovery, not the normal realtime attempt. Existing stop/restart ownership,
privacy checks, candidate limits and model serialization remain in place.
This deliberately adds no adaptive CPU policy, larger batches or new queue.

The worker lifecycle regression exercises both modes without hardware capture or
model loading when run. Live call quality, CPU peaks and end-to-end transcript latency
require a freshly authorised representative call; compilation is not that evidence.
