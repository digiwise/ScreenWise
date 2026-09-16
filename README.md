# ScreenWise

ScreenWise is an experimental Windows-first fork of **Screenpipe**. The Screenpipe
contributors built the underlying recorder and architecture. This fork builds
on their pinned MIT-licensed baseline, focusing on Windows operation, local
processing and privacy hardening.

> [!WARNING]
> **DEVELOPER EXPERIMENT — NO PRIVACY OR SECURITY GUARANTEES**
>
> Effort has been made to reduce network activity, protect sensitive content and
> improve privacy controls. These are design aims and partially tested changes,
> **not guarantees**. Neither the maintainer nor Digiwise guarantees the privacy,
> security, correctness or safety of this system or its code. Bugs, incomplete
> controls and dependencies may expose or retain sensitive information; recording
> may also lose data. Local processing, authentication, redaction and exclusions
> do not establish that the system is safe for confidential use.
>
> **For developer inspection and experimentation only.** Validation is incomplete.
> Use synthetic or non-sensitive data and read the [known limitations](VALIDATION_REGISTER.md).
> Neither the maintainer nor Digiwise provides support in any way or commits to
> maintenance, fixes, security updates, releases, issue responses or assistance.
> This is not a supported Digiwise product or service.

The intended functionality is local activity recording and indexing, searchable
through an authenticated loopback API and a small desktop UI.

This repository is self-contained. A clone does not require the original parent
workspace, Litepipe, personal machine paths or Codex tooling. Existing binary and
crate names remain `screenpipe` for compatibility. This fork is not affiliated
with the hosted Screenpipe service.

**Status:** development preview with partial validation, not a completed privacy
or offline certification. Read [validation status](VALIDATION_REGISTER.md) for
known issues and untested cases before relying on recording/privacy behavior.

## Intended scope

The following describes the implemented direction, not a security assurance or
proof of complete behavior across every execution path, dependency or platform.

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

The retained recorder runtime does not automatically acquire supported tools or
models from the Internet. Provision native runtimes and models explicitly and
verify the documented checksums. Missing required native inputs can prevent a
build or startup; missing optional models leave dependent features unavailable.
Build tools and explicit provisioning may use the network.

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

Start with [Windows setup](docs/WINDOWS_SETUP.md) and
[CONTRIBUTING.md](CONTRIBUTING.md). From Visual Studio Developer PowerShell,
with native prerequisites explicitly provisioned, the production build is:

```powershell
cargo build --release --locked
```

Add `--offline` after the locked dependency cache is provisioned. Do not run
`cargo update` or silently acquire native prerequisites/models. Native desktop
tests need the separate Ninja Multi-Config and CRT matrix in the setup guide.
Reusable [interactive test sources](scripts/windows/interactive-validation/README.md)
require configuration, offline preparation and fresh owner readiness.

## Architecture and provenance

- The upstream provenance boundary is MIT commit
  `892199f742e46d0c5d9e8c06687b35ca7c2b6547`; the fork preserves that exact
  commit and all its ancestors; ScreenWise development follows that baseline.
- ScreenWise development uses that baseline; current/post-baseline commercial
  Screenpipe implementations were not imported for the fork's changes.
- Litepipe may only be consulted at
  `8969c10723634640ad2e757b8281dca8b0272c2f`, with adaptations documented.
  It is an optional reference, not a dependency.

See [BASELINE_AUDIT.md](BASELINE_AUDIT.md) for the subsystem/network inventory
and [HARDENING_PROGRESS.md](HARDENING_PROGRESS.md) for removal boundaries and
validation evidence.

Repository policy is in [AGENTS.md](AGENTS.md). Preserve [LICENSE.md](LICENSE.md)
and upstream/third-party notices. Source publication and binary/package releases
have separate requirements. Binary/package releases require fork-owned
identifiers and an explicitly reviewed signing and update policy.
Read [distribution scope](docs/DISTRIBUTION_SCOPE.md) for omitted models, media
and test fixtures, retained ancestry and artifact-provenance limitations.
Inherited upstream GitHub workflows are inactive reference files, not enabled
ScreenWise release automation.
