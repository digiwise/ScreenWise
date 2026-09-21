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

Review can establish health counters, warnings, candidate capture gaps, fixed
safe placeholders, lock intervals, transcription status and leftover processes.
Current `/health` samples distinguish requested-but-unavailable transcription
and include frame-link success, update-failure, TTL-eviction and drop-reason
counters. Multi-monitor duplicates do not count as TTL loss.
It cannot establish that arbitrary sensitive pixels/text were absent without a
separately authorized content review or a controlled known-marker test.
