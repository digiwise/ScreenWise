# Local Windows deployment

## TLDR

From the repository root, quit any deployed ScreenWise instance, then run:

```powershell
# Ordinary PowerShell: reuse built release binaries and the provisioned shared Pi.
& .\scripts\windows\deployment\Deploy-ScreenWise.ps1
# Elevated PowerShell: install/verify the nine exact executable block rules.
& .\scripts\windows\deployment\Set-ScreenWiseDeploymentFirewall.ps1
# After confirming the rule output, ordinary PowerShell:
& .\scripts\windows\deployment\Start-ScreenWise.ps1 -PreflightOnly
& .\scripts\windows\deployment\Start-ScreenWise.ps1
```

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
production `release` outputs, separate from Cargo build directories. It is not
an installer or a claim of production readiness. See the repository README and
validation register for the experimental/support boundaries.

From the repository root in ordinary PowerShell:

```powershell
& .\scripts\windows\deployment\Deploy-ScreenWise.ps1 -PlanOnly
& .\scripts\windows\deployment\Deploy-ScreenWise.ps1
```

By default this reuses already-built release artifacts. It copies the GUI,
recorder, Bun, FFmpeg/FFprobe, native DLLs and GUI assets to
`.local/deployment/release`. It reuses the private WebView2 directory from the
existing `.local/gui-trial-runtime/runtime.json`, without copying it. Required
artifacts must have been explicitly provisioned; nothing is downloaded.
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

Deployment only prepares files. It does not start capture or alter firewall rules.
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

`-Build` refreshes the frontend and invokes the canonical locked/offline root and
desktop release builds, with `-PlanOnly` before each. Dependency provisioning is
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
policy rather than changing policy or silently bypassing it. `-Launch` explicitly starts
after deployment and still requires the rules to be installed first. Startup
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
