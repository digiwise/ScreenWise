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
.\target\release\screenpipe.exe audio models --output json
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

The launcher creates a fresh directory beneath `.local/gui-trials`, seeds an
explicit privacy profile, and waits while the desktop UI is used normally. It
continuously observes the process tree and endpoints between 30-second
authenticated API samples without captured content. Choose **Quit** from the ScreenWise tray menu
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
