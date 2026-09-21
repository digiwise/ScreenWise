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

Review can establish health counters, warnings, candidate capture gaps, fixed
safe placeholders, lock intervals, transcription status and leftover processes.
Current `/health` samples distinguish requested-but-unavailable transcription
and include frame-link success, update-failure, TTL-eviction and drop-reason
counters. Multi-monitor duplicates do not count as TTL loss.
It cannot establish that arbitrary sensitive pixels/text were absent without a
separately authorized content review or a controlled known-marker test.
