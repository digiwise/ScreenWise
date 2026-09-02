# ScreenWise hardening progress

This log is the handoff record for the local-first hardening work after the
Windows baseline. It deliberately excludes captured-data details. Preserve the
untracked `smoke-baseline-20260901/` and `smoke-fixed-20260901/` directories.

## Provenance and constraints

- Upstream source boundary: MIT commit `892199f742e46d0c5d9e8c06687b35ca7c2b6547`.
- Litepipe may only be consulted at `8969c10723634640ad2e757b8281dca8b0272c2f`;
  no Litepipe code has been used in this work.
- Stay on branch `screenwise`; use locked Cargo commands and do not run
  `cargo update`.
- Keep lockfiles unchanged unless a dependency change is explicitly justified
  and reviewed. The engine lockfile now has an intentional Sentry-only graph
  reduction from `ba67c6283`; the desktop lockfile is being reviewed as part
  of its separate Sentry-plugin removal.

## Completed before this continuation

- Removed automatic CLI NPM update checks (`53aa73c8b`).
- Replaced runtime FFmpeg download/setup mutation with explicit provisioning
  (`98e73e33b`).
- Forced normal CLI and desktop recording configuration to disable telemetry
  (`d5d43e966`), while legacy analytics/Sentry code remains compiled.
- Replaced runtime diarization and Silero VAD model downloads with explicit
  local-model requirements (`2dc701249`).
- On 2026-09-02, a locked release build passed under VS Developer PowerShell,
  CMake 3.31.8, Ninja, OpenBLAS, and the pinned ONNX Runtime location.

## Offline smoke result — 2026-09-02

An elevated, executable-specific Windows Firewall outbound-block rule was
installed temporarily for `target\\release\\screenpipe.exe`, then removed by the
operator after the test. With pre-staged models and FFmpeg/FFprobe beside the
release executable, the bounded recorder run had:

- HTTP 200 from local `/health`.
- HTTP 200 from bearer-authenticated local `/search`.
- No non-loopback TCP connections owned by the recorder during the check.
- No panic, ONNX-compatibility, or transcription-error log markers.

This validates the tested cached local recorder path under an OS firewall deny
rule. It does not prove that no code attempts an outbound request that the OS
blocked, nor does it replace a longer soak or quality assessment.

## Current work: remove telemetry and crash-reporting network code

### Audit findings

The forced-off switch is not sufficient: PostHog/Sentry code remains compiled
in the engine and Tauri desktop app. Key engine files are:

- `crates/screenpipe-engine/src/analytics.rs` — PostHog HTTP client/events.
- `crates/screenpipe-engine/src/telemetry_context.rs` — analytics identity and
  deployment-context environment variables.
- `crates/screenpipe-engine/src/meeting_telemetry.rs` — meeting event payloads.
- `crates/screenpipe-engine/src/resource_monitor.rs` — local resource monitor
  plus PostHog transport; retain only demonstrably useful local diagnostics.
- `crates/screenpipe-engine/src/bin/screenpipe-engine.rs` and server/routes —
  Sentry initialization/panic reporting and analytics call sites.

The Tauri desktop app also has Rust analytics/Sentry initialization and many
web-UI PostHog call sites. It has its own manifest and lockfile. Remove this
surface in a separate commit after the engine-only removal builds cleanly.

### Desktop telemetry increment in progress (uncommitted)

The following uncommitted files route browser PostHog imports to a local inert
module without touching dozens of UI components:

- `apps/screenpipe-app-tauri/lib/posthog-disabled.ts`
- `apps/screenpipe-app-tauri/lib/posthog-disabled-react.tsx`
- `apps/screenpipe-app-tauri/tsconfig.json`
- `apps/screenpipe-app-tauri/next.config.mjs`

The aliases cover both `posthog-js` and `posthog-js/react`; the local module
implements the existing capture/identity/consent methods as no-ops. The app
has no installed `node_modules` and Bun is unavailable on this host, so only
static configuration validation is currently possible. Next actions: validate
the aliases with the approved frontend package manager/environment; replace
the Tauri Rust `analytics.rs` transport with a local no-op or remove its call
sites; then remove/compile out direct Sentry initialization and its manifest
dependencies. Do not commit this desktop increment with the engine commit.

The Tauri module declaration now explicitly selects `analytics_local.rs`, a
local-only compatibility implementation. The original `analytics.rs` remains
in the tree temporarily but is no longer compiled. Its removal can follow once
the desktop build is available for validation.

Direct desktop Sentry initialization, plugin registration, tracing layer,
remote panic reporting, and scope enrichment are now behind `cfg(any())`; the
local panic log remains active. `sentry` and `tauri-plugin-sentry` have been
removed from the desktop manifest. On 2026-09-02,
`cargo check --manifest-path apps/screenpipe-app-tauri/src-tauri/Cargo.toml
--locked` reached the Tauri build-script stage after the approved fetch of its
already locked `divanshu-go/muda` revision. It regenerated the schema files
without Sentry permissions; the app capability still named `sentry:default`,
which is being removed before the final locked check. No dependency version is
being changed to work around the cache prerequisite.

The final locked Tauri check now reaches the existing packaging prerequisite
and fails before Rust application source compilation because
`bun-x86_64-pc-windows-msvc.exe` is absent. This is not a telemetry error and
must be supplied through the approved desktop build setup; the missing Bun
sidecar should not be downloaded implicitly by this hardening pass. The check
did successfully regenerate the Tauri schemas without Sentry permissions, and
the stale `sentry:default` application capability has been removed.

### Next concrete increment

1. Remove the remaining desktop `sentry:default` capability declaration.
2. Complete a locked Tauri check, then inspect the lockfile solely for the
   expected removal graph; do not accept incidental upgrades.
3. Delete the now-uncompiled desktop Rust analytics source if the check passes.
4. Run scoped formatting, static frontend configuration checks, diff/lock
   checks, then update this log and `BUILD_NOTES.md`/`BASELINE_AUDIT.md` before
   the desktop-only commit.

### Engine increment complete — `ba67c6283`

- Replaced the engine PostHog client/event implementation with inert
  compatibility shims while its cloud-facing owners are removed separately.
- Removed the direct engine `sentry` dependency and compiled out its tracing
  integration; local structured logs and `last-panic.log` remain active.
- Removed the resource monitor's HTTP client, identity/hardware payload, and
  PostHog send path. Its optional local resource log remains.
- Regenerated `Cargo.lock` with `cargo check --offline` because removing the
  direct Sentry dependency changes the lock graph. The delta removes Sentry
  0.36 and unreachable transitive packages; it does not upgrade packages.
- `cargo check -p screenpipe-engine --locked` passed after the update, with
  only the pre-existing audio `unused_mut` and Windows `CommandExt` warnings.
- `cargo test -p screenpipe-engine --lib --locked` could not complete on this
  host: the existing `libsamplerate-sys` build cannot find a native
  `samplerate` static library. The test-only crate download was approved and
  succeeded; this is a native test prerequisite failure, not a Rust source or
  lockfile failure from the telemetry change.

## Still pending after telemetry work

1. Runtime SHA-256 verification was committed in `2f7801e9b` as a separate
   audio-model increment. It streams all three explicitly staged artifacts before cache or
   ONNX/VAD loading, uses the already locked `sha2 0.10.9` package, and has a
   one-line root-lock metadata delta. `cargo check -p screenpipe-audio --locked`
   passed with only the pre-existing `unused_mut` warning. Its focused unit
   test could not start because the existing `infer 0.15.0` dev dependency is
   uncached and the network sandbox blocks static.crates.io. Silero
   source/tag/licence provenance is still operator-provided because its former
   baseline `master` URL was mutable; the code enforces the recorded bytes but
   does not claim unverified provenance.
2. Remove cloud sync, accounts/login/cloud proxy, external providers and
   transcription, pipes/workflow automation, connections/integrations/team and
   enterprise services, and unneeded cross-platform/model subsystems — one
   subsystem per commit, preserving Windows capture/UIA/OCR, local audio,
   SQLite search, and bearer-protected loopback API.
3. Run pause/resume, restart/soak, and diarization-quality assessments. Record
   only aggregate/non-sensitive evidence.

### Next subsystem audit: cloud sync

Initial source mapping is complete but no cloud-sync source has been changed.
The engine owns the CLI command and service startup in
`crates/screenpipe-engine/src/bin/screenpipe-engine.rs`, with supporting cloud
search, archive/upload, and provider modules under `crates/screenpipe-engine`.
The archive implementation also performs data deletion after sync and must be
handled deliberately rather than removed by a broad file deletion. The next
commit should disable/remove only the externally connected sync command,
startup, routes, and client transport while retaining local retention cleanup;
then compile the engine and inspect any now-unreachable core sync dependency
graph separately.

### Cloud-sync removal complete — `22a71dc3f`

- Removed the `sync` CLI command, record-time sync flags and service startup,
  cloud search metadata, sync/archive API routes, and memory-delete tombstone
  path.
- Deleted the unreferenced cloud-search, cloud archive, sync API/provider, and
  SFTP-sync implementation files. The independent local retention routes and
  cleanup module remain compiled.
- `cargo check -p screenpipe-engine --locked` passed after the route and state
  removal (only the established audio `unused_mut` warning appeared). The
  engine still compiles `screenpipe-sync` through broader core dependencies;
  dependency-graph pruning is a separate reviewed change, not an excuse to
  alter locked versions.

### Next subsystem audit: product accounts, login, and cloud proxy

Initial mapping separates Screenpipe-product account code from the later
third-party connections/integrations removal. The engine CLI `Login`/`Logout`/
`Whoami` commands and `cli/login.rs`, desktop login/entitlement components and
commands, server cloud-JWT state, and the `/v1/chat/completions` proxy are one
product-account subsystem. Remove those as one commit while preserving the
authenticated local API and local configuration. Do not include Google,
calendar, MCP, browser, or other third-party OAuth connections in that commit;
they are their own planned subsystem.

The engine-side product-account increment is now in progress: the `login`,
`logout`, and `whoami` CLI commands, the cloud JWT state, and the localhost
`/v1/chat/completions` cloud proxy have been removed. The local API bearer-key
configuration is unchanged. `cargo check -p screenpipe-engine --locked` has
completed after the edit; desktop account UI and commands remain for their
separate desktop increment.

The engine product-account increment is ready to commit. The scoped locked
engine check passed, no root-lock change is present, and `git diff --check`
passed. The desktop account/entitlement surface remains intentionally
uncommitted alongside the previously staged desktop telemetry work so its
Tauri-specific validation can be completed in one desktop review.
