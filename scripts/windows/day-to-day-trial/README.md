# Windows day-to-day trial launcher

This developer tool starts a private local trial with explicit keyboard and
clipboard capture, ScreenWise's authenticated loopback API, local Parakeet
transcription, DRM/lock privacy gates and existing executable-scoped firewall
rules. It does not create, weaken or remove firewall rules.

The recorder already writes bounded rolling `screenpipe*.log` files into the
selected data directory (50 MiB each, 200 MiB total). The launcher also records
content-free operational samples every 30 seconds under
`<data-dir>/.trial-audit/<session>/`. Those samples contain health states, numeric
pipeline metrics, fixed privacy/loss reason codes and final exact-path process
inventory. They deliberately omit captured pixels, OCR/UIA text, keystrokes,
clipboard values, transcripts, window titles, URLs, bearer tokens and audio-device
names.

The launcher passes `--enable-keyboard-capture` and
`--enable-clipboard-capture`, so a persisted opt-out cannot silently defeat the
trial intent. `/health` reports the requested keyboard/clipboard status and the
actual input recorder state. It also enables deterministic text PII reconciliation;
this reduces retained recognized secrets but is not a privacy guarantee. The
async worker always applies its regex policy. Its optional AI model is licensed
separately from this repository; when that pack is absent, startup records one
explicit reduced-coverage warning and continues with regex-only reconciliation.

Before creating the data directory or starting recording, a Parakeet trial now
runs the exact selected executable's non-recording model check. It verifies the
Parakeet files and initializes the provisioned Silero and speaker models. Any
unavailable model stops the launcher. The same content-free check can be run
directly:

```powershell
.\target\release-local\screenpipe.exe audio models --output json
```

If that check fails, the launcher reports each unavailable component and its
underlying access, integrity or runtime error; it does not collapse native
stderr into a generic PowerShell `NativeCommandError`.

Run from ordinary (non-elevated) PowerShell after the reviewed firewall rules are
installed:

```powershell
.\scripts\windows\day-to-day-trial\Start-ScreenWiseTrial.ps1 `
  -FirewallGroup '<exact-reviewed-rule-group>' `
  -IgnoredWindow @('PasswordManagerApp', 'App::Private title') `
  -IgnoredUrl @('accounts.example.test', 'vault.example.test')
```

## Desktop GUI trial

When rebuilding the desktop through the canonical launcher, first export the current
frontend from the already provisioned, locked frontend dependencies:

```powershell
Push-Location .\apps\screenpipe-app-tauri
$env:NEXT_TELEMETRY_DISABLED = '1'
node node_modules\next\dist\bin\next build
Pop-Location
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild
```

The launcher does not execute Tauri's `beforeBuildCommand`.
It can embed an old `out/` export even when the Rust source is current. Avoid
inherited bootstrap/prebuild scripts that acquire dependencies implicitly.
Refresh the reviewed runtime hash only after the frontend export and desktop
build both succeed; an old readiness record does not authorize a new trial.

Build and trial scripts now default to `release-local`. Use the canonical
`RootBuild` and `DesktopBuild` presets for standard builds; do not invoke Cargo
directly for supported tasks. Review `-PlanOnly`, then use `-AllowColdCache` once
for the new profile. Recorder outputs are under `target/release-local`, and GUI
outputs under `apps/screenpipe-app-tauri/src-tauri/target/release-local`.
Provision/stage the reviewed FFmpeg, FFprobe and native DLLs for those directories;
the build launcher does not download or relocate them.

After building, rerun runtime preparation and review its exact hashes and paths.
Existing GUI runtime manifests must be regenerated; the launcher refuses a missing
or mismatched `build_profile`. Configure new executable-scoped firewall rules using
the agreed owner-run elevated block before starting any trial. Old rules for other
profile paths do not cover these executables. Nothing here changes existing rules.
For production-performance trials, build explicitly with `-BuildProfile release`
and pass the same option to runtime preparation and both trial launchers.

The desktop keeps a fixed `.screenwise-session-state` marker and an OS file
lock in each recording directory it uses. On next startup, an active marker
reports an interrupted shutdown in local logs and, once the safe-status writer
is available, in the timeline. Existing SQLite WAL handling and durable audio
reconciliation remain responsible for their respective recovery; the notice
does not assert that every frame, input event or final audio sample was saved.
Known cleanup failures or an exit during resource startup prevent a clean marker.
New capture/Pi setup is refused once exit begins. An unrecognized marker or an
unsafe filesystem link stops startup without replacing it. Do not delete a
marker to bypass a live directory lock.

This tracking applies to the desktop app. A directory without a marker does
not prove an earlier session ended cleanly; pre-upgrade sessions cannot be
classified retrospectively. Marker-access/format failures stop startup with a
fixed diagnostic, so no timeline writer is available in that failed launch.
The CLI's existing audio recovery is unchanged; CLI session-marker tracking
is not covered by these desktop changes.

Pi process inventory runs at app startup and before chat startup. It is advisory
and reports only counts, deduplicating an unchanged result. Unknown or unrelated
RPC processes are never terminated. On Windows, each newly launched managed Pi
tree is assigned to an anonymous kill-on-close Job before execution; that job
covers its descendants during ordinary exit and app termination. This does not
retroactively establish ownership of legacy or independently started processes.
Job ownership controls cleanup, not networking. A real Pi/tool run still needs
a reviewed executable/firewall inventory for any additional helpers it launches.

The GUI trial uses the same content-free audit format while running the desktop
application in the way an owner would normally use it. Build the unsigned GUI,
then prepare its private runtime once:

```powershell
.\scripts\windows\day-to-day-trial\Prepare-ScreenWiseGuiTrialRuntime.ps1
```

The private WebView2 copy is required because firewall-scoping the machine-wide
WebView2 runtime would also affect unrelated applications. The prepared runtime
manifest records exact GUI/WebView2 hashes and the exact FFmpeg, FFprobe and Bun
paths. It is private ignored state under `.local/gui-trial-runtime`; do not
publish it.

After installing a uniquely named outbound block rule for every path in that
manifest, run the non-recording preflight from ordinary PowerShell:

```powershell
.\scripts\windows\day-to-day-trial\Start-ScreenWiseGuiTrial.ps1 `
  -FirewallGroup '<exact-reviewed-GUI-rule-group>' `
  -PreflightOnly
```

Preflight verifies exact hashes and rules, checks local audio models, and makes
one fixed TCP attempt from `screenpipe-app.exe` without sending application
data. It accepts the firewall result only when that scoped attempt fails while
an unscoped PowerShell control reaches the same fixed address. It does not start
recording or WebView2.

Start a freshly authorized GUI acceptance run by omitting `-PreflightOnly`:

```powershell
.\scripts\windows\day-to-day-trial\Start-ScreenWiseGuiTrial.ps1 `
  -FirewallGroup '<exact-reviewed-GUI-rule-group>' `
  -IgnoredWindow @('PasswordManagerApp') `
  -IgnoredUrl @('accounts.example.test')
```

For a visual-only controlled acceptance, use the `-DisableAudio` switch with
`-TranscriptionEngine disabled`; this avoids collecting irrelevant ambient audio.

The launcher creates a fresh directory beneath `.local/gui-trials`, seeds an
explicit privacy profile, and waits while the desktop UI is used normally. It
continuously observes the process tree and endpoints between 30-second
authenticated API samples without captured content. A self-tested local Toolhelp
sampler also records descendant process names every 25 ms so short-lived shell
helpers cannot hide between ordinary process snapshots; its failure is an
explicit trial failure. Choose **Quit** from the ScreenWise tray menu
to finish; closing the main window can leave the tray application running.
The final report requires the expected active configuration, a 403/403/200
bearer-authentication matrix, a loopback-only listener, no observed non-loopback
endpoint, a fixed clean-shutdown marker, and no scoped process left behind.

### Short GUI capture acceptance

Use the dedicated fixture to check UI Automation persistence and OCR fallback
without reviewing ordinary captured content. Preparation is non-recording:

```powershell
.\scripts\windows\day-to-day-trial\Build-ScreenWiseGuiCaptureFixture.ps1
```

After fresh owner authorization starts a GUI trial, launch the fixture with a
new private output directory and leave it visible briefly:

```powershell
$fixtureRun = 'gui-capture-' + [Guid]::NewGuid().ToString('N')
$fixtureOut = Join-Path $env:TEMP $fixtureRun
& .\scripts\windows\day-to-day-trial\GuiCaptureFixture.exe `
  --run-id $fixtureRun --out-dir $fixtureOut
```

The standard label exposes one fixed UIA marker. A separate custom-painted
surface exposes one fixed OCR marker while the window title selects the existing
hybrid canvas path. The fixture does not read input, clipboard, audio, network,
or captured data. While the recorder is running, evaluate only those fixed
markers:

```powershell
.\scripts\windows\day-to-day-trial\Test-ScreenWiseGuiCaptureEvidence.ps1 `
  -DataDir '<fresh GUI trial data directory>' -Port '<active API port>'
```

This live phase still requires fresh owner readiness because it starts a visible
window during an active capture. The evaluator emits counts and booleans only.

The default private data directory is `.local/day-to-day-trial`. Supply
`-DataDir` to use another private directory. Supply multiple exclusions as a
PowerShell array, as shown above.
Press Ctrl+C once and allow ScreenWise to finish its normal shutdown. The monitor
then records whether the exact recorder, FFmpeg or FFprobe paths remain running.
PowerShell can interrupt a native invocation before it assigns `$LASTEXITCODE`.
The launcher therefore records a successful inferred exit only when it finds a
new `shutdown complete` entry written after that launch; it never treats a stale
log entry as current evidence.

For a concise content-free status while a trial is running, use a separate
PowerShell window:

```powershell
.\scripts\windows\day-to-day-trial\Get-ScreenWiseTrialStatus.ps1
```

It reports pipeline counters and fixed warning categories only. It does not emit
captured text, titles, URLs, clipboard/keyboard values, transcripts or tokens.
Routine per-frame redaction and negative meeting scans are DEBUG-level in current
builds so INFO remains useful for lifecycle and state changes.
The launcher also writes this report as `trial-summary.json` after shutdown. It
includes lock-pause duration, whether frame/audio counters advanced inside the
sampled locked interval, scoped clean-shutdown and process-cleanup evidence, and
failure counts split by safe acquisition stage. Current builds also distinguish
frame-link TTL expiry direction (event without frame versus frame without event).
Linker send-failure counters split full/closed channels across captured-frame,
persisted-event, discarded-event and dropped-trigger messages. A nonzero counter
requires attention even before expiry; it reports failed correlation delivery,
not proof that the captured activity row itself was lost.
An intentional lock/DRM/schedule audio pause remains healthy in `/health`; the
audio-pipeline `transcription_paused` flag shows that acquisition is paused.

Run the non-recording synthetic regression for log offsets, lock-interval
reconstruction, ANSI shutdown parsing and content-free failure/loss counts with:

```powershell
.\scripts\windows\day-to-day-trial\Test-ScreenWiseTrialStatus.ps1
```

## Next short live acceptance run

The 2026-09-24 repairs need one fresh, explicitly authorized live run. Keep it
short: a five-to-eight-minute run is enough. Use a new private `-DataDir`, the
already reviewed executable-scoped firewall group and the existing compact
synthetic fixture. Do not reuse historical captured data as evidence.

The run should establish these content-free conditions:

1. Start normally and confirm authenticated health plus advancing frame, UI and
   audio counters.
2. Play one brief local synthetic speech clip to create a small durable
   transcription backlog. Stop during or immediately after session finalization.
3. Require exit code zero, a fresh `shutdown complete` marker, no
   `audio_shutdown_issue_codes`, and zero confirmed or possible delivery loss.
4. Restart on the same fresh data directory and confirm pending durable segments
   can reconcile and authenticated marker-count search succeeds.
5. In a separate silent compact-fixture phase, exercise password and excluded
   window transitions. Require bounded password-state notices, inspect only the
   numeric `password_content_suppressed_events`, and require zero unexplained
   frame-link TTL expiry and zero forbidden synthetic-marker hits.

Preparation and preflight may run without recording. Playback, capture, window
focus changes and recorder startup remain interactive and require fresh owner
authorization. The controller must wait indefinitely with recording stopped and
its fixture hidden until that authorization arrives.

Review can establish health counters, warnings, candidate capture gaps, fixed
safe placeholders, lock intervals, transcription status and leftover processes.
Current `/health` samples distinguish requested-but-unavailable transcription
and include frame-link success, update-failure, TTL-eviction and drop-reason
counters. Multi-monitor duplicates do not count as TTL loss.
It cannot establish that arbitrary sensitive pixels/text were absent without a
separately authorized content review or a controlled known-marker test.

## Controlled GUI crash/restart check

`Invoke-ScreenWiseGuiRecoveryCheck.ps1` is inert in its default `Prepare` phase.
Use PowerShell 7 for the tested harness. Run the offline regression first:

```powershell
.\scripts\windows\day-to-day-trial\Test-ScreenWiseGuiRecoveryHarness.ps1
python .\scripts\windows\day-to-day-trial\test_recovery_rows.py
.\scripts\windows\day-to-day-trial\Invoke-ScreenWiseGuiRecoveryCheck.ps1 `
  -Phase Prepare -FirewallGroup '<owner-confirmed exact-path group>' `
  -PythonExecutablePath '<absolute path to already provisioned python.exe>'
```

Keep the returned private `plan` path. `Preflight` verifies the pinned GUI/runtime,
process monitor, firewall and no-payload blocked-versus-unscoped TCP control;
it does not start recording or a GUI. The prepared directory remains unused.

```powershell
$plan = '<returned private plan path>'
.\scripts\windows\day-to-day-trial\Invoke-ScreenWiseGuiRecoveryCheck.ps1 -Phase Preflight -PlanPath $plan
# Only after fresh owner readiness; this call waits until the first GUI exits.
.\scripts\windows\day-to-day-trial\Invoke-ScreenWiseGuiRecoveryCheck.ps1 -Phase Start -PlanPath $plan -OwnerReady
```

The lifecycle check disables screen/audio and keyboard/clipboard content and
sets a unique, nonmatching UI include filter. Do not change those settings. It
does not test WGC, audio-tail recovery, real Pi or the filter-column layout with
recorded frames. The API remains bearer-authenticated and loopback-only.
Python uses only its standard library and must already be provisioned. Its
executable and the inspection source are pinned. SQLite inspection is read-only
and returns only aggregate counts: frames, audio chunks and UI events other than
safe notices must be zero. Disabled modalities intentionally omit their health
metric sections; absent sections are accepted only with explicit disabled status,
while incomplete/nonzero sections are rejected. Absence is not reported as a
measured zero. The real UI-recorder counter must still be present and zero.

From a second PowerShell session, once the initial API and persisted-notice
positive control are available, run `Interrupt`. It pins the exact running GUI
process handle/path/start time before terminating that one process, and never
sweeps process names or kills unknown processes. A restart is refused if scoped
processes remain or the completed monitor/authentication audit fails.

```powershell
.\scripts\windows\day-to-day-trial\Invoke-ScreenWiseGuiRecoveryCheck.ps1 -Phase Interrupt -PlanPath $plan
.\scripts\windows\day-to-day-trial\Invoke-ScreenWiseGuiRecoveryCheck.ps1 -Phase Restart -PlanPath $plan
# From the second session, while the restarted GUI is open:
.\scripts\windows\day-to-day-trial\Invoke-ScreenWiseGuiRecoveryCheck.ps1 -Phase VerifyNotice -PlanPath $plan
```

The single-use receipt permits reopening only that fresh interrupted directory,
after verifying its active marker, launch evidence, settings hash and inert
settings. Ordinary trials still refuse non-empty directories. Restart preserves
the SQLite store, token and settings. Acceptance requires the fixed interruption
notice in the authenticated timeline API and restarted-session log, plus the
initial persisted notice rows and token surviving restart. The owner opens
Timeline, expands Recording status and confirms the explanation, then quits
through the tray. After the restart launcher returns:

```powershell
.\scripts\windows\day-to-day-trial\Invoke-ScreenWiseGuiRecoveryCheck.ps1 `
  -Phase Finish -PlanPath $plan -OwnerNoticeSeen
```

`Finish` requires clean exit/marker, completed process monitoring, no owned
processes or active API endpoints, expected bearer behavior and no sampled
non-loopback endpoint. Closed `TIME_WAIT` entries remain diagnostic remnants,
not active endpoints. Owner confirmation is recorded separately. An incomplete
or failed attempt remains evidence; do not reset its plan, overwrite it, reuse
its receipt or delete its data to turn it into a pass. Prepare a new attempt.
Firewall restoration remains a separate owner-run operation; this controller
does not create or remove rules. No readiness timeout starts the GUI automatically.

## Synthetic Pi RPC check

Provision Pi explicitly with `Provision-ScreenWisePi.ps1` and provision the default
Ollama model separately. The runtime needs a loopback-only Ollama listener on
port 11434. Inventory and owner-confirm exact non-loopback IPv4/IPv6 outbound
blocks for the selected Bun, Ollama app/service and model-server executables.
Use a separately named group for provider rules; leave existing ScreenWise blocks
unchanged. Run the offline harness tests with an already provisioned Python:

```powershell
python .\scripts\windows\day-to-day-trial\test_pi_rpc_smoke.py
.\scripts\windows\day-to-day-trial\Test-ScreenWisePiRpcHarness.ps1
```

Use PowerShell 7 for the wrapper. Omitting `-Run` is inert preparation:

```powershell
$arguments = @{
    PackageRoot = '<absolute provisioned pi-agent directory>'
    OutputDirectory = '<new absolute directory under repository .local>'
    PythonExecutable = '<absolute path to already provisioned python.exe>'
    ProviderFirewallGroup = '<owner-confirmed provider rule group>'
    RuntimeFirewallGroup = '<owner-confirmed bundled Bun rule group>'
}
.\scripts\windows\day-to-day-trial\Invoke-ScreenWisePiRpcCheck.ps1 @arguments
# Only after the owner confirms installation:
.\scripts\windows\day-to-day-trial\Invoke-ScreenWisePiRpcCheck.ps1 @arguments -Run -OwnerFirewallConfirmed
```

The wrapper verifies enforced exact-path address/protocol scopes, listener
ownership and the already installed model. It makes one fixed no-payload TCP
attempt from Bun and the same-endpoint unscoped control before inference. It
downloads nothing and changes no firewall rules. Optional `-BunExecutable` and
`-OllamaDirectory` select reviewed existing installations; defaults use the
`release-local` bundled Bun and per-user Ollama. Do not execute `pi_rpc_smoke.py`
directly to bypass this native gate.

The RPC child has tools and context/resource discovery disabled, uses isolated
config with startup acquisition/telemetry disabled and only localhost Ollama,
and requests two short arithmetic replies. Correct numbers with at most one
terminal period/exclamation mark are accepted; model spelling and exact format
adherence are not the behavior under test. It verifies both completed assistant
replies in order in the synthetic persisted session,
closes stdin and requires normal process exit. Output contains counts, booleans
and fixed failure codes; stderr is counted without exposing its contents.
The 8,192-token context and 32-token output limits bound this smoke test and do
not change application settings. Fresh failed artifacts remain evidence.
This does not test GUI PiManager/Windows Job ownership, arbitrary tool descendants,
useful activity retrieval, packet-level isolation or production performance.
Those require separately gated controlled runs and exact helper inventory.

The reused pinned process monitor samples at 25 ms and must observe the Pi root
with zero tool descendants for this tool-free check. Windows can create one
hidden console host even for a minimal Bun command with `CREATE_NO_WINDOW`.
The wrapper requires the actual System32 image's valid Microsoft signature;
a second native sampler verifies the live child's exact image/hash, direct
parent and held process handle, and requires its exit. No name-only exception
is accepted; the shared GUI monitor still reports console hosts as unexpected
until the selected test independently establishes their identity. The OS console
host is outside the application firewall scope; this does not prove packet-level
isolation for that helper. No global System32 firewall rule is added.
Final native TCP/UDP endpoint
checks supplement the rule inspection and outbound control; they are snapshots,
not packet tracing. Ollama is an independently installed service and is left
running. No process-name sweep or unrelated process cleanup is performed.

For a subsequent fresh, owner-gated GUI trial, pass
`-ProvisionedPiSource '<already provisioned pi-agent directory>'` to
`Start-ScreenWiseGuiTrial.ps1`. The launcher stages only the declared reviewed Pi
runtime after enforcing its ordinary fresh-directory guard, validates the pinned
version/required metadata and refuses reparse points or an existing destination.
It downloads nothing. This option cannot be combined with a recovery receipt.
Keep audio/vision/input disabled and use a unique nonmatching UI include filter
for a lifecycle-only Pi test. Add `-ToolFreePiValidation` for the controlled GUI
conversation/lifecycle test: the managed Pi uses CLI flags to disable tools,
extensions, skills, templates, themes and context-file discovery, limits PATH to
the provisioned Bun directory, skips bash setup and uses an 8,192-token context
and 64-token text-only output bound. The launcher refuses this mode unless
audio, vision and input content are disabled and an explicit include filter is
provided. It records the mode and clears inherited validation opt-ins for ordinary
trials. Normal chat settings remain separate. This tests real local conversation
and owned-process lifetime, not activity retrieval or tool execution.
The renderer obtains the app store root from native IPC, including an explicit
`SCREENPIPE_DATA_DIR`; settings, chat files, title/chat work directories and large
context files use that root. A custom recording-directory preference is separate.
Missing or invalid native scope fails closed rather than using global files.
The tool-free mode also skips skill installation and calendar publishing.
The GUI's **report crash** action opens a local diagnostic dialog only; it does
not submit the error or logs to anyone.
Test staging without opening a GUI using
`Test-ScreenWisePiRuntimeStaging.ps1`. Actual tool execution needs further exact
helper inventory/firewall scope; a model instruction to avoid tools is not an
execution boundary for the application's normally enabled tools.

### Required preparation for future Pi trials

**Planned, not implemented:** replace the per-trial package-copying path described
above with one verified, versioned shared Pi runtime. Keep the already approved
test and its evidence unchanged. Before a future Pi trial:

1. Add an explicit shared-runtime path, retaining path, reparse-point, pinned
   version and runtime-integrity checks. Do not bypass existing validation.
2. Keep each fresh trial's configuration, chats and session data isolated from
   other trials and the shared package. Treat the runtime package as immutable.
3. Add regression tests for trial isolation, runtime tampering/version mismatch,
   and rejected unsafe paths, then validate actual reuse before relying on it.
4. After reuse passes, remove only redundant package copies proven inactive.
   Check the exact resolved targets and active use first. Preserve all logs,
   databases, chats, captures and evidence; never remove a whole trial directory
   or clean anything active or whose non-use cannot be established.
