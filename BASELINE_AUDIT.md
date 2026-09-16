# ScreenWise subsystem and network baseline

> [!WARNING]
> **Privacy and security are aims, not guarantees.** This is a scoped engineering
> record, not a security audit or certification. Neither the maintainer nor
> Digiwise guarantees this system or code is private, secure or safe. See the
> [README notice](README.md) and [known limitations](VALIDATION_REGISTER.md).

Source boundary: upstream Screenpipe MIT commit
`892199f742e46d0c5d9e8c06687b35ca7c2b6547`. No Litepipe implementation was used
in the documented changes. See [AGENTS.md](AGENTS.md) for optional reference rules.

## Retained local capabilities

Windows WGC/multi-monitor capture, UIA/accessibility and OCR, microphone/system
capture, explicitly provisioned local transcription, SQLite/full-text search,
retention and privacy controls remain. The API requires local bearer auth and
binds to loopback. Local structured logs and panic diagnostics remain on disk.

## Removed or bounded network paths

| Path | Current boundary |
|---|---|
| CLI update checks and FFmpeg fallback acquisition | Removed; setup and updates are explicit |
| ONNX Runtime and supported audio/redaction model acquisition | Explicit provisioning with accepted checksum checks, no runtime fallback download |
| PostHog/Sentry product telemetry and crash upload | Removed; local diagnostics retained |
| Product accounts, subscriptions, hosted completion/transcription/OCR | Removed; local API auth is independent and remains required |
| Cloud/SFTP sync, cloud archive/search and remote-device metadata | Removed from retained product paths |
| Pipes/workflow automation, third-party integrations and enterprise services | Removed from retained product paths |
| mDNS discovery | Removed |
| Desktop external resources | Local/scoped resource policy; opening an external link is an explicit owner action |
| Optional local Pi/Ollama | Fixed local endpoint; not a hosted AI gateway |

The above is a source/runtime boundary summary, not complete packet-level proof.
Build tools, package managers and separately invoked provisioning may access the
network. They are not covered by a recorder-only firewall rule.

## Observed validation and limits

Initial Windows runs established multi-monitor WGC, populated UIA trees, Windows
OCR, SQLite writes, local audio/Parakeet transcription and authenticated search.
Missing or wrong bearer tokens failed. Diarization initialization was exercised;
speaker-label accuracy was not benchmarked. Later scoped tests cover native
password/exclusion behavior, audio partial-tail persistence/restart and a real
lock/unlock with audio disabled. Read [VALIDATION_REGISTER.md](VALIDATION_REGISTER.md)
for exact boundaries and outstanding defects.

A configured firewall plus no sampled external sockets is insufficient to claim
all outbound traffic is blocked. Active helper IPv4 probes and loopback controls
have scoped evidence; recorder-originated probes, useful external IPv6/UDP
coverage, broader executable scope and final firewall closeout remain incomplete.
