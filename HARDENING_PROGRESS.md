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

The engine product-account increment was committed in `d5f04e890`. The scoped locked
engine check passed, no root-lock change is present, and `git diff --check`
passed. The desktop account/entitlement surface remains intentionally
uncommitted alongside the previously staged desktop telemetry work so its
Tauri-specific validation can be completed in one desktop review.

Desktop follow-up mapping: `RecordingState` and `ServerCore::start` share an
`ArcSwap` cloud-token cell with the engine cloud proxy and Pi executor, while
`recording.rs` invokes `require_app_entitlement` before local recording starts.
The safe desktop sequence is to remove that entitlement gate and token handoff
first (local API bearer authentication is independent), then remove the
frontend account UI/commands and persisted entitlement fields. Do not delete
third-party OAuth/integration state as part of this product-account increment.

The entitlement compatibility gate now always permits local recording; it no
longer changes recording status or returns a subscription error. Its callers
remain temporarily while the desktop cloud-token handoff is removed in
compiler-guided steps. The local bearer-auth path is unaffected.

Desktop recorder/server progress: `RecordingState` no longer carries a cloud
JWT, `ServerCore::start` no longer accepts or seeds one, and Pi is constructed
without a product-cloud credential. The `get_cloud_token` and
`set_cloud_token` Tauri commands were removed. The same desktop change still
needs its locked Tauri validation, which remains blocked at the pre-existing
missing Bun sidecar before Rust application source compilation.

The desktop `open_login_window` command has also been removed, eliminating the
remaining native product-login browser launch. Its generated frontend binding
and UI callers must be removed as part of the same desktop account cleanup;
third-party OAuth window commands remain outside this scope.

The desktop provider tree no longer mounts `AuthGuard` or
`AppEntitlementGate`, so ordinary UI startup and recording no longer depend on
periodic Screenpipe-session validation or account entitlement. The deep-link
handler remains because it also supports the separate third-party OAuth
subsystem.

The active settings lifecycle no longer invokes `set_cloud_token`, and the
enterprise-policy hook no longer invokes `get_cloud_token`. Product-account
tokens are therefore no longer handed to the native recorder or used as a
fallback for enterprise policy; the latter remains an independent enterprise
surface for its own removal pass.

On the latest check, Cargo stops even earlier because the uncommitted desktop
lockfile has incidental resolver drift from the earlier approved cache fetch
and no longer satisfies `--locked`. Do not commit that 800-line lockfile
rewrite or regenerate it without first restoring a minimal, reviewed removal
graph. This is a desktop-validation blocker, not a reason to relax `--locked`.

The desktop manifest's broad formatting check also reports pre-existing style
drift in unrelated desktop and enterprise files. Those formatting-only diffs
are not part of any hardening commit; use changed-file formatting and staged
diff review until a dedicated formatting cleanup is authorized.

### Next subsystem audit: external transcription and AI providers

The local Parakeet path is retained. Deepgram and OpenAI-compatible
transcription span `screenpipe-audio` engine selection, audio-manager options,
batch/realtime workers, and meeting streaming; simple CLI-flag removal would
not remove their reachable network clients. Treat all of those as one audio
subsystem commit after the pending desktop account work, then separately
handle general AI gateways and workflow classification.

The CLI boundary now omits Deepgram and OpenAI-compatible audio engine choices;
`cargo check -p screenpipe-engine --locked` completed after that change. This
is deliberately not committed separately: the provider transport remains in
the audio crate, so the CLI edit stays with the eventual external-transcription
removal commit to avoid a misleading partial hardening claim.

Provider-transport mapping is now complete: `transcription::engine` and
`transcription::stt` instantiate the Deepgram/OpenAI-compatible clients for
live and batch work, while engine retranscription routes pass their configs
through the audio manager. The audio manager and those routes must be changed
together with deletion of the provider modules; otherwise retranscription
would retain a reachable outbound path.

### External-transcription removal in progress

- The audio crate no longer exposes Deepgram or OpenAI-compatible transcription
  engine variants, clients, API-key environment discovery, or model-config
  transport. The provider source modules have been deleted.
- Meeting streaming now permits only the selected local engine or an explicit
  disabled state; its former Screenpipe Cloud and Deepgram WebSocket transports
  and endpoint configuration have been deleted. Local meeting lifecycle and
  persisted-final controls remain.
- `cargo check -p screenpipe-audio --locked` passed after this checkpoint. The
  only new warnings are mechanical unused imports/arguments in the local
  engine and will be corrected before the commit; the established `unused_mut`
  in `core/stream.rs` remains outside this change.
- Next: remove the engine recording/retranscription provider configuration,
  then run locked engine/audio validation and inspect the lockfile for a
  dependency reduction only.

### External-transcription removal complete — pending commit

- Removed Deepgram and OpenAI-compatible batch transcription engines, request
  clients, CLI API-key option, and their engine/audio-manager/retranscription
  configuration paths. Local Whisper, Qwen, and Parakeet paths remain.
- Removed the Screenpipe Cloud and Deepgram live-meeting WebSocket transports.
  Meeting streaming now uses only the selected local engine or is disabled;
  its retained compatibility token/endpoint fields are forcibly empty and do
  not initiate network activity.
- The intentional root `Cargo.lock` delta removes `tokio-tungstenite` and its
  native-TLS transport edges from `screenpipe-audio`; no packages were upgraded.
- `cargo check -p screenpipe-engine --locked`, `cargo fmt --all -- --check`,
  and `git diff --check` passed. The focused audio test build remains blocked:
  `cargo test -p screenpipe-audio --lib --locked --offline --no-run` cannot
  use the existing uncached `infer v0.15.0` dev dependency. No network fetch
  or dependency update was used to bypass that prerequisite.

### Cloud workflow classifier removal complete — pending commit

- Removed the opt-in cloud workflow-classifier module, its Screenpipe gateway
  endpoint/token handling, and its recorder startup task. Captured activity is
  no longer sent to a classifier service or used to emit cloud-derived workflow
  events.
- `cargo check -p screenpipe-engine --locked` and `git diff --check` passed;
  no lockfile change is required for this source-only removal.

### Next subsystem audit: pipes and pipe store

The recorder currently constructs a `PipeManager` at startup, installs built-in
pipes, restores/schedules execution, registers pipe-specific bearer permissions,
and mounts local `/pipes` plus remote pipe-registry routes. The standalone CLI
also installs, runs, publishes, and searches pipes. This is one automation
subsystem: remove its startup, API, CLI, persistence, permissions middleware,
and registry routes together while retaining the ordinary authenticated local
capture/search API. No pipe source has been changed in this audit checkpoint.

### Pipes and pipe store removal complete — pending commit

- Removed PipeManager startup, Pi-agent installation, scheduled execution,
  built-in pipe installation, pipe-specific permissions, `/pipes` routes,
  registry routes, SQLite pipe persistence, and standalone pipe/install CLI
  commands.
- The ordinary authenticated local recorder, search API, and non-pipe local
  routes remain. `cargo check -p screenpipe-engine --locked` passed with only
  the established unrelated warnings; this source-only removal does not change
  the lockfile.

### Enterprise/team CLI removal complete — pending commit

- Removed the `screenpipe team` command and its direct Screenpipe enterprise
  API client. This source-only change does not affect the local recorder or
  its bearer-protected loopback API.
- `cargo check -p screenpipe-engine --locked` and `git diff --check` passed;
  no lockfile change is required.

### Desktop Bun sidecar environment repair

- Restored the ignored Windows Tauri sidecar at
  `apps/screenpipe-app-tauri/src-tauri/binaries/bun-x86_64-pc-windows-msvc.exe`
  by copying the already-installed local Bun 1.4.0. Its SHA-256 matches the
  source binary: `627D2E4775C24BDEDEE2CD7CCC18DCADAE061E5345274AB6E3C4C797927BFB8F`.
  No download, dependency update, or tracked source change was made.
- Tauri's prior missing-sidecar prerequisite is therefore satisfied. A direct
  Tauri bundle validation remains blocked earlier because this checkout has no
  locally resolvable Tauri CLI executable; the independent desktop Cargo check
  remains blocked by the existing uncommitted desktop `Cargo.lock` drift under
  `--locked`.

### Next subsystem audit: external memory export

`external_memory_sync` is an enabled-integration scheduler, not recorder
storage: it reads the local `memories` table every five minutes and writes a
digest into Claude Code's `CLAUDE.md` and Codex's `AGENTS.md`. It is exposed by
the memories API, retains an engine scheduler field, and is started by the
desktop server. Remove those engine, API, and desktop-startup references as one
integration subsystem; preserve the ordinary local memories database/API.

### External memory export removal complete — pending commit

- Removed the external-memory scheduler, the authenticated
  `/memories/sync-external` trigger, and the desktop server startup hook. The
  recorder no longer copies captured-memory content into Claude Code or Codex
  instruction files.
- Local memories persistence and ordinary authenticated CRUD endpoints remain.
  `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` passed. The desktop crate cannot yet be checked under
  `--locked` because its pre-existing lockfile drift is independent of this
  source-only removal.

### Next subsystem audit: CLI promotional reminders

The standalone engine binary starts a five-minute terminal reminder loop. Its
rotating prompts direct users to Screenpipe's hosted desktop site, an `npx`
MCP download, a remote pipe bundle, login/cloud sync, and a hosted survey.
None is required for local recording, so remove the loop and its CLI startup
hook as one source-only subsystem.

### CLI promotional-reminder removal complete — pending commit

- Removed the periodic standalone-CLI reminder task and all of its hosted,
  external-package, pipe, login/cloud-sync, and survey prompts. Recording and
  the ordinary command-line interface do not emit promotional actions.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` passed with only the established unrelated warnings.
  No lockfile change is required.

### MCP subsystem audit

The legacy `screenpipe mcp` CLI downloads an unpinned package tree from GitHub's
moving `main` branch and directs execution through `uv`. Separately, the newer
`/mcp-servers` API persists user-defined HTTP/stdio server configurations,
OAuth credentials, and proxy-tool execution for the Pi agent. These are not a
single safe deletion: retaining a future read-only local MCP interface requires
an explicit capability boundary, while the current proxy supports arbitrary
external tools and transports. No MCP source has been changed in this audit.

### Legacy MCP downloader removal complete — pending commit

- Removed the `screenpipe mcp` CLI command and its GitHub/`uv` downloader. It
  can no longer fetch moving `main`-branch MCP package content or create an
  externally executable MCP setup directory.
- The separate user-defined MCP proxy has not been changed; its future
  read-only/local capability boundary remains an explicit design decision.
- Initial validation exposed one missed `mod browser;` declaration after the
  helper deletion; it was corrected immediately. `cargo fmt --all -- --check`,
  `cargo check -p screenpipe-engine --locked`, and `git diff --check` now pass
  with only the established unrelated warnings; no lockfile change is required.

### Hosted survey command removal in progress

The standalone `screenpipe survey` command opens the hosted Screenpipe survey
in a browser. It is not part of the local recorder, so its CLI module, command,
dispatch arm, and parser test are being removed as one source-only subsystem.

### Hosted survey command removal complete — pending commit

- Removed the hosted survey command, its browser-launch helper, and parser
  test. The standalone recorder CLI no longer opens a Screenpipe web page.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` passed with only the established unrelated warnings;
  no lockfile change is required.

### External AI provider preset audit

The CLI preset subsystem still accepts OpenAI, Anthropic, custom HTTP,
Screenpipe Cloud, and ChatGPT OAuth providers, alongside local Ollama. The
same provider vocabulary is consumed by the currently dirty desktop settings
and Pi-agent integration. Removing only the CLI choices would leave the
external AI gateway reachable through the desktop, while removing all but
local inference is a material retained-capability decision. No provider source
has changed in this audit.

### CLI hosted-onboarding prompt removal in progress

The standalone recorder printed a hosted Screenpipe onboarding URL after every
startup. This promotional prompt is unrelated to local capture and is being
removed as a source-only CLI cleanup.

### CLI hosted-release prompt removal in progress

The recorder also prints a GitHub releases link at startup. It is a
non-functional hosted prompt and is being removed independently of local
recording behavior.

### CLI hosted-release prompt removal complete — pending commit

- Removed the GitHub releases prompt from recorder startup. Local recorder/API
  startup output remains intact.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` passed with only the established unrelated warnings;
  no lockfile change is required.

### CLI hosted-onboarding prompt removal complete — pending commit

- Removed the standalone recorder's hosted onboarding advertisement. Startup
  now reports only local recorder/API state.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` passed with only the established unrelated warnings;
  no lockfile change is required.

### Dead engine Sentry initialization removal complete — pending validation

The standalone engine binary still contains an always-false `#[cfg(any())]`
Sentry initialization block, including the retired remote DSN and scope
enrichment. That block has been removed. The following panic hook is
independent and still persists local `last-panic.log` diagnostics.

`cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`, and
`git diff --check` passed. No lockfile change is required.

### Standalone connection CLI removal complete — pending commit

The `screenpipe connection` command manages third-party integration credentials
and probes a local WhatsApp gateway/browser registry. It is separate from the
runtime connections API, which remains unchanged in this narrowly scoped CLI
removal.

- Removed the `screenpipe connection` parser, its command dispatcher, and the
  direct credential-management implementation. The runtime connections API and
  desktop integration surfaces are untouched.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` pass with only the established unrelated warnings; no
  lockfile change is required.
