# ScreenWise

ScreenWise is a Windows-first personal activity recorder derived from the last
MIT-licensed Screenpipe baseline. It records and indexes activity locally so it
can be searched through an authenticated loopback API and a small desktop UI.

## Product boundary

ScreenWise retains:

- Windows Graphics Capture with multi-monitor support
- Windows UI Automation/accessibility capture and native OCR
- local microphone and system-audio capture
- explicitly provisioned local transcription
- local SQLite storage, full-text search, retention, and meeting privacy controls
- deterministic regex and explicitly provisioned ONNX text redaction
- a bearer-authenticated API bound to loopback
- optional local Pi chat through a fixed Ollama loopback endpoint
- local structured logs, resource diagnostics, and `last-panic.log`

ScreenWise does not provide product accounts, subscriptions, cloud sync,
telemetry, crash upload, hosted AI/transcription/redaction, pipe automation,
third-party service connectors, or enterprise/fleet services.

The application does not acquire tools or models from the Internet. Required
native runtimes and model files must be provisioned explicitly and must pass the
documented checksum checks before their features become available. Missing
optional artifacts leave the affected feature disabled; local recording remains
available.

## Local API

The recorder exposes its API on `127.0.0.1` (port `3030` by default). API bearer
authentication must remain enabled. Except for the small documented startup-safe
surface, clients send:

```text
Authorization: Bearer <local-api-token>
```

For a custom recorder directory, retrieve the matching token with:

```powershell
screenpipe auth token --data-dir <recorder-data-directory>
```

This local token is independent from local model configuration.

## Build and validation

Build from Visual Studio Developer PowerShell using the pinned Windows
prerequisites and environment documented in [BUILD_NOTES.md](BUILD_NOTES.md).
The production validation command is:

```powershell
cargo build --release --locked --offline
```

Do not run `cargo update` or silently download missing prerequisites. Native
desktop tests require the separate Ninja Multi-Config and CRT matrix documented
in [BUILD_NOTES.md](BUILD_NOTES.md).

## Architecture and provenance

- The upstream provenance boundary is MIT commit
  `892199f742e46d0c5d9e8c06687b35ca7c2b6547`.
- Current or post-MIT Screenpipe source is not used.
- Litepipe may only be consulted at
  `8969c10723634640ad2e757b8281dca8b0272c2f`, with adaptations documented.

See [BASELINE_AUDIT.md](BASELINE_AUDIT.md) for the subsystem/network inventory
and [HARDENING_PROGRESS.md](HARDENING_PROGRESS.md) for removal boundaries and
validation evidence.
