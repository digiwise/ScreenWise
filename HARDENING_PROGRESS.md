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
  and reviewed. The engine lockfile has the intentional Sentry-only graph
  reduction from `ba67c6283`; the final desktop cleanup leaves both Rust
  lockfiles unchanged and prunes only the removed frontend telemetry graph from
  `bun.lock`.

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

### External-transcription removal complete — `a879dbebe`

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

### Cloud workflow classifier removal complete — `ee07c467b`

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

### Pipes and pipe store removal complete — `79f8f2d57`

- Removed PipeManager startup, Pi-agent installation, scheduled execution,
  built-in pipe installation, pipe-specific permissions, `/pipes` routes,
  registry routes, SQLite pipe persistence, and standalone pipe/install CLI
  commands.
- The ordinary authenticated local recorder, search API, and non-pipe local
  routes remain. `cargo check -p screenpipe-engine --locked` passed with only
  the established unrelated warnings; this source-only removal does not change
  the lockfile.

### Enterprise/team CLI removal complete — `515363355`

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

### External memory export removal complete — `90c17a3bf`

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

### CLI promotional-reminder removal complete — `31d4352bb`

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

### Legacy MCP downloader removal complete — `6094f5eed`

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

### Hosted survey command removal complete — `182f0768a`

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

### Engine test-build validation checkpoint

After the engine-only integration removals, `cargo test -p screenpipe-engine
--lib --locked --no-run` reached native linking but is blocked by the existing
machine prerequisite: `libsamplerate-sys` cannot find the static `samplerate`
library. This is unrelated to the removals; no dependency update or native
library acquisition was attempted. Locked engine checks continue to pass.

### CLI hosted-onboarding prompt removal in progress

The standalone recorder printed a hosted Screenpipe onboarding URL after every
startup. This promotional prompt is unrelated to local capture and is being
removed as a source-only CLI cleanup.

### CLI hosted-release prompt removal in progress

The recorder also prints a GitHub releases link at startup. It is a
non-functional hosted prompt and is being removed independently of local
recording behavior.

### CLI hosted-release prompt removal complete — `95d05d7c0`

- Removed the GitHub releases prompt from recorder startup. Local recorder/API
  startup output remains intact.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` passed with only the established unrelated warnings;
  no lockfile change is required.

### CLI hosted-onboarding prompt removal complete — `8fdf39faf`

- Removed the standalone recorder's hosted onboarding advertisement. Startup
  now reports only local recorder/API state.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` passed with only the established unrelated warnings;
  no lockfile change is required.

### Dead engine Sentry initialization removal complete — `3f1e7692b`

The standalone engine binary still contains an always-false `#[cfg(any())]`
Sentry initialization block, including the retired remote DSN and scope
enrichment. That block has been removed. The following panic hook is
independent and still persists local `last-panic.log` diagnostics.

`cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`, and
`git diff --check` passed. No lockfile change is required.

### Standalone connection CLI removal complete — `e94afe749`

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

### User-supplied MCP server API removal complete

The separate `/mcp-servers` API accepted arbitrary user-defined HTTP and stdio
MCP endpoints, persisted their credentials, performed OAuth redirects, and
proxied tool calls. It is unrelated to a future constrained read-only local
MCP interface, so its engine module, runtime store/mount, and callback auth
exemption are being removed as one isolated subsystem. Connections and browser
integration routes remain unchanged.

- Deleted the `/mcp-servers` HTTP/stdio proxy API and stopped creating its
  credential-backed store at server startup. The removed OAuth callback
  exemption can no longer admit unauthenticated MCP redirect requests.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` pass with only the established unrelated warnings; no
  lockfile change is required.

### Runtime connections HTTP surface removal complete

The engine no longer creates the credential-backed connection manager or
WhatsApp gateway, reconnects a previously paired WhatsApp session, or mounts
the `/connections` HTTP API. The standalone `connections_api` module and
browser bridge state remain deliberately intact for a later coordinated
desktop/browser cleanup. The now-unreachable OAuth and browser-pairing paths
are no longer exempt from local API authentication.

- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` pass with only the established unrelated warnings; no
  root lockfile change is required.

### Dead connections API facade removal complete

With the runtime `/connections` mount gone, the remaining `connections_api`
module was unreachable. Deleted that credential-management, WhatsApp, OAuth,
and browser-pairing API facade and its crate export only. Browser bridge state,
desktop integration, and `screenpipe-connect` remain intentionally untouched
for their later coordinated cleanup.

- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` pass with no root lockfile change. The sole remaining
  `connections_api` text match is an intentionally untouched stale server
  comment, not a module reference.

### No-op engine analytics scheduling removal complete

The server's periodic API request counter and vision/audio pipeline reporting
loops existed solely to feed the now no-op analytics compatibility shim. They
have been removed, including the per-request counting middleware. Local
pipeline metrics remain live for health, WebSocket, and `/vision/metrics` and
`/audio/metrics` consumers; the local resource monitor is unchanged.

- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` pass with only the established unrelated warnings; no
  root lockfile change is required.

### No-op meeting telemetry removal in progress

`meeting_telemetry` only derived hashed meeting metadata for the disabled
analytics shim. Its detector and meeting-route calls have been removed while
preserving meeting detection, lifecycle mutations, events, and persistence.

### No-op meeting telemetry removal complete — `734161a7b`

- Removed the disabled-analytics meeting telemetry adapter and all of its
  detector/meeting-route call sites. Local meeting capture, controls, events,
  and SQLite persistence remain unchanged.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`,
  and `git diff --check` passed; no lockfile change is required.

### No-op analytics shim removal in progress

Removed the remaining no-op analytics compatibility module and its startup,
search, and macOS sleep/wake call sites. Local search and sleep monitoring
behavior remains; only discarded telemetry metadata was removed.

### No-op analytics shim removal complete — `3caaaa73a`

- Removed the no-op analytics shim and all remaining engine call sites. Local
  search, sleep/wake monitoring, and resource monitoring remain intact.
- Corrected obsolete unused meeting variables introduced by the preceding
  telemetry removal. `cargo fmt --all -- --check`,
  `cargo check -p screenpipe-engine --locked`, and `git diff --check` passed;
  no lockfile change is required.

### Remote search-time PII filter removal complete — `cdbc576a6`

The engine's optional `/search?filter_pii=true` path sends OCR,
transcriptions, UI text, input text, and memory content to a
Screenpipe-hosted Tinfoil enclave. Although it is attested and fail-closed on
transport failures, it is not a local redactor and therefore contradicts the
local-only runtime target. The separate pre-persistence local redaction worker
is retained. The immediate engine increment will remove only the remote
search-time client and make an explicit `filter_pii=true` request fail closed
before any search data is returned. The selectable desktop enclave worker is
in dirty desktop files and remains a separate follow-up subsystem.

- Deleted the engine's hosted search-time filter client and direct Tinfoil
  dependency. `filter_pii=true` now returns `501` before search retrieval, so
  it cannot silently return unredacted data or transmit recorded text.
- Regenerated the root lockfile offline solely to remove the one
  `screenpipe-engine -> tinfoil` dependency edge; the package remains locked
  for `screenpipe-redact`, with no package/version update.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`
  in VS Developer PowerShell with Ninja, and `git diff --check` passed. The
  only warnings are the established audio `unused_mut` and Windows
  `CommandExt` import warnings. A non-Developer-PowerShell check first failed
  native bindgen/CMake discovery and was corrected by rerunning in the
  documented build environment.
- The strongest scoped test build, `cargo test -p screenpipe-engine --lib
  --locked --no-run`, again reaches the existing `libsamplerate-sys` link
  blocker because the machine lacks the native static `samplerate` library.
  No dependency or native-library acquisition was attempted.

### Remote CLI PII fallback removal complete — `42f9ce3ae`

The standalone recorder has a second remote text-redaction path: when both
local ONNX and OPF adapters are unavailable, setting `TINFOIL_API_KEY` or
`TINFOIL_BASE_URL` switches the destructive pre-persistence worker to the
hosted enclave. The safe local fallback is already regex-only redaction. The
next engine-only increment will remove the environment-gated enclave fallback
and retain local ONNX, local OPF, and regex-only behavior.

- Removed the Tinfoil imports, environment-variable branch, and hosted
  fallback. If both local AI adapters are unavailable, the worker now always
  performs deterministic local regex-only redaction.
- `cargo fmt --all -- --check`,
  `cargo check -p screenpipe-engine --bin screenpipe --locked` in VS Developer
  PowerShell with Ninja, and `git diff --check` passed, with only the
  established unrelated warnings. The full engine test build remains blocked
  at the existing missing native static `samplerate` library.

The remaining Tinfoil text and image adapters are consumed only by the
desktop server's selectable `pii_backend` path. Because that source file is
already part of the uncommitted desktop integration work, removing those
adapters requires a coordinated desktop privacy commit; it will not be mixed
into the standalone-engine fallback removal.

### Dead team-memory workspace crate removal complete — `cd2ff0da0`

`screenpipe-team-memory` is an unreferenced workspace member. Its markdown
frontmatter format is solely for the removed team/pipe/enterprise memory
stack; no local recorder, search, or desktop code imports it. The next
increment removes this dead crate and its workspace membership only. The
separate Pi enterprise-admin skill remains part of the broader Pi/desktop
integration surface and is not included.

- Removed the unreferenced crate manifest and source, its explicit
  `default-members` entry, and excluded its now-empty path from the `crates/*`
  member glob. No shared dependency versions or root lockfile entries changed.
- `cargo metadata --offline --locked --no-deps`,
  `cargo fmt --all -- --check`, and `cargo check -p screenpipe-engine --locked`
  in VS Developer PowerShell with Ninja passed. The only compiler warnings
  are the established audio `unused_mut` and Windows `CommandExt` import.

### Remaining cloud-sync crate audit — 2026-09-03

`screenpipe-sync` remains compiled through `screenpipe-core`'s `cloud-sync`
feature, which both the engine and the uncommitted desktop manifest still
enable. The engine also uses the feature-gated module for a local stable
machine-ID helper. Removing the crate therefore requires a coordinated
cloud-sync feature extraction (including the dirty desktop manifest), rather
than a safe standalone crate deletion.

### Local crash attribution removal complete — `7f6557685`

The remaining engine `telemetry_context` module no longer drives a live
transport. Its only compiled use collects `SCREENPIPE_*` support/customer/
deployment/embedder identifiers into the local `last-panic.log` record. The
next engine-only privacy increment removes that attribution collection and the
unused telemetry module while retaining the local crash log and panic hook.

- Removed the account/deployment/embedder attribution string from local panic
  records and deleted the now-unused telemetry-context module. Crash message,
  source location, backtrace, rotation, and local-only persistence remain.
- `cargo fmt --all -- --check`,
  `cargo check -p screenpipe-engine --bin screenpipe --locked` in VS Developer
  PowerShell with Ninja, and `git diff --check` passed. The only compiler
  warnings are the established audio `unused_mut` and Windows `CommandExt`
  import.

### Dead resource-monitor telemetry sender removal complete — `c32c72148`

The active resource monitor only samples local process CPU/memory and can
optionally write a local JSON file. Its former PostHog transport, hardware
fingerprinting, GPU-probing subprocesses, and payload construction are all
behind `cfg(any())`. The next engine-only increment deletes that unreachable
remote telemetry code while retaining local diagnostics.

- Deleted the compile-disabled PostHog sender and its payload construction,
  including the stale telemetry-context dependency. The active local monitor
  remains unchanged.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`
  in VS Developer PowerShell with Ninja, and `git diff --check` passed with
  only the established audio `unused_mut` and Windows `CommandExt` warnings.

### CLI AI preset/provider audit — 2026-09-03

The `pipe models` CLI persists OpenAI, Anthropic, arbitrary custom URL,
Screenpipe Cloud, ChatGPT OAuth, and local Ollama presets for the Pi/pipe
system. That system is also represented in the existing dirty desktop files.
Removing only cloud presets would leave an incoherent partially supported
assistant configuration surface; retain-vs-remove local Ollama/Pi is a
coordinated product capability decision for the later desktop provider pass.

### Cloud search flag removal complete — `ff1168bf1`

The `include_cloud` search parameter survived the cloud-sync runtime removal
but no longer has an implementation. The next engine-only increment makes an
explicit `include_cloud=true` request fail closed before retrieval, preserving
a clear local-only API contract rather than silently ignoring the request.

- `include_cloud=true` now returns `501` before data retrieval; ordinary
  local search behavior and query caching remain unchanged.
- `cargo fmt --all -- --check`, `cargo check -p screenpipe-engine --locked`
  in VS Developer PowerShell with Ninja, and `git diff --check` passed with
  only the established audio `unused_mut` and Windows `CommandExt` warnings.

### Windows native samplerate validation — 2026-09-04

The debug engine test build had previously failed because `libsamplerate-sys`
could not find its vendored `samplerate.lib` when CMake used single-config
Ninja. Rebuilding that crate under `Ninja Multi-Config` produced the expected
`out\\build\\Release\\samplerate.lib`, and the engine test linker consumed it.

- `cargo clean -p libsamplerate-sys` and
  `cargo test -p screenpipe-engine --lib --locked --no-run` were run in Visual
  Studio Developer PowerShell with `CMAKE_GENERATOR=Ninja Multi-Config`.
  The samplerate missing-library error is resolved.
- The debug test executable still fails later for an unrelated existing native
  configuration mismatch: `knf-rs-sys` supplies debug-CRT objects while
  `whisper-rs-sys` supplies release-CRT objects (`LNK2038` and
  `CrtDbgReport`). `cargo test -p screenpipe-engine --lib --release --locked
  --no-run` completed successfully in 3m 44s, which validates the documented
  optimized Windows profile. No source or dependency changes were made.

### Release workspace validation follow-up — 2026-09-04

`cargo build --release --locked` with `CMAKE_GENERATOR=Ninja Multi-Config`
advanced past the samplerate build and engine compilation, but stopped in the
separate `screenpipe-audio-eval` tooling. It still imports the removed
Deepgram transcription module and calls the former five-argument
`TranscriptionEngine::new` constructor. The next isolated increment repairs
or removes those obsolete evaluation-only paths; the local recorder runtime
was not implicated.

### Local audio evaluation cleanup — `5d6ccc459`

The audio evaluation binaries are not recorder runtime dependencies, but the
workspace build compiles them. The pipeline replay tool retained a paid
Deepgram/Screenpipe Cloud smoke path after external transcription removal,
making the hardening boundary incomplete and breaking the build.

- Removed the provider replay CLI, credential lookup, cloud endpoint override,
  and provider-only report fields. The replay harness now exercises only the
  local diarization and SQLite/search path; its documentation no longer
  advertises provider credentials or outbound smoke tests.
- Updated the transcription evaluation binary to the current three-argument
  `TranscriptionEngine::new` API.
- `cargo build -p screenpipe-audio-eval --release --locked` completed
  successfully in Visual Studio Developer PowerShell with Ninja Multi-Config.

### Cloud OCR fallback removal — `b912bea5b`

`screenpipe-screen` still routed the legacy `unstructured` OCR engine through
the hosted Unstructured API, sending captured frames when its credential was
present. The local recorder already has platform-native and Tesseract OCR
engines, so the historical configuration value can safely fall back locally.

- Removed the direct `screenpipe-connect` dependency and changed the legacy
  `unstructured` selection to execute the platform-default local OCR engine.
  Existing serialized configurations remain readable; no captured frame is
  sent to an external OCR service.
- `cargo fmt --all -- --check`, `git diff --check`, and
  `cargo check -p screenpipe-screen --locked --offline` in Visual Studio
  Developer PowerShell passed. The release test target could not be linked in
  this offline environment because its uncached `memory-stats` dev dependency
  would require a crates.io download; no dependency fetch was attempted.
- The full `cargo build --release --locked --offline` workspace build passed
  in 7m 42s with only the established audio `unused_mut` and Windows
  `CommandExt` warnings.

### Dead hosted OCR helper removal — `6946e3fde`

After the screen crate stopped calling the hosted Unstructured OCR path, its
`screenpipe-connect` module had no remaining callers. The helper still read
`UNSTRUCTURED_API_KEY` and could upload captured-frame bytes if reused.

- Deleted the unreachable hosted OCR module and its public export. No local
  OCR implementation or configuration compatibility behavior changed.
- `cargo fmt --all -- --check`, `git diff --check`, and locked offline checks
  of both `screenpipe-connect` and `screenpipe-engine` passed in Visual Studio
  Developer PowerShell. The established audio `unused_mut` and Windows
  `CommandExt` warnings remain.

### Windows debug native CRT alignment — `01a68d313`

The debug `screenpipe-engine` test executable linked `knf-rs-sys` C++ objects
built with the Debug MSVC CRT against Whisper C++ objects intentionally built
with the Release CRT. This caused `LNK2038` and `CrtDbgReport` failures after
the samplerate library was fixed.

- Added a `profile.dev` package override that disables debug assertions only
  for `knf-rs-sys`; its build script consequently selects the Release CMake
  profile, matching Whisper without changing package versions or production
  profiles.
- After `cargo clean -p knf-rs-sys`,
  `cargo test -p screenpipe-engine --lib --locked --no-run` successfully linked
  the debug test executable. The full test run executed 531 tests: 525 passed,
  2 were ignored, and 4 unrelated `logging` tests failed because Windows does
  not expose the active file's new length after `File::flush`; that logging
  behavior will be repaired separately.

### Windows rolling-log flush repair — `cf74d2a5f`

With the debug engine test binary linkable, four rolling-log tests showed that
`File::flush` alone does not make active-file length visible to independent
Windows metadata reads. The writer now calls `sync_data` after flushing so
rotation, cleanup, diagnostics, and tests observe durable current sizes.

- `cargo test -p screenpipe-engine --lib --locked` passed: 529 tests passed,
  2 ignored, and no failures. This also confirms the debug native CRT alignment
  through an actual test run.

### Windows screen-test dependency scoping — `ff4011989`

The `screenpipe-screen` Windows test target attempted to download the uncached
`memory-stats` crate even though it is used solely by the macOS
`apple_leak_bench` benchmark.

- Scoped `memory-stats` to macOS dev dependencies, leaving the benchmark
  available on its supported platform while removing it from Windows test
  resolution.
- Moved the Windows-independent OCR benchmark/example's `strsim` dependency
  behind an explicit `ocr-bench` feature, so it is not resolved for normal
  unit tests. `cargo test -p screenpipe-screen --lib --locked --offline`
  passed on Windows: 99 passed, 0 failed.

### Desktop local-only conversion — `b20ffe638`

The existing desktop/Tauri changes remove remote telemetry, crash reporting,
account gating, and cloud-token propagation as one cohesive local-only
boundary. Its first locked offline check exposed a packaging configuration
error: the installed Bun sidecar exists under `src-tauri/binaries`, while the
Windows Tauri config referenced the config-root path.

- Corrected the Windows `externalBin` entry to `binaries/bun`.
- Removed obsolete bundled-FFmpeg resource globs: this local-first build
  relies on explicitly installed FFmpeg on the launch `PATH`, and the old
  source-tree resources no longer exist.
- Removed the likewise-absent `vcredist` resource glob; the toolchain runtime
  is not a checked-in application resource.
- Removed the stale checked-in OpenBLAS resource glob. The existing Windows
  runtime baseline stages `libopenblas.dll` beside the executable from the
  explicit `OPENBLAS_PATH`; installer-side staging remains a separate
  packaging follow-up rather than a source-tree glob that prevents checks.
- Replaced the Windows desktop prebuild's automatic FFmpeg/OpenBLAS archive
  acquisition with explicit tool discovery: FFmpeg must be on `PATH`, and
  native Tauri builds validate the documented `OPENBLAS_PATH`. The Bun
  sidecar copy destination now matches the configured `binaries/bun` prefix.
- Validation passed: `bun run build` completed the production static export;
  `cargo check --locked --offline` completed for `screenpipe-app`. The frontend
  build reported only the existing `unpdf` dynamic-import warning.
- Removed the final frontend PostHog initialization and project key; the
  compatibility provider is deliberately inert while remaining UI call sites
  are retired in later scoped work.

### Dead desktop analytics transport removal — `c300aa08e`

The desktop now compiles `analytics_local.rs`, an inert compatibility module,
instead of the former remote analytics implementation. The old `analytics.rs`
was consequently unreferenced but still contained PostHog and attribution
network code.

- Removed that dead transport.
- `cargo check --locked --offline`, `cargo fmt --all -- --check`, and
  `git diff --check` passed. Only the established audio/engine warnings and
  unrelated existing desktop warnings remain.

### Dead desktop Sentry scaffolding removal — `f47b9f3c1`

The desktop binary still retained compile-disabled Sentry initialization and
scope-enrichment blocks, including the retired remote DSN and settings
metadata, plus comments describing Sentry tagging. The active panic hook is
local-file logging and is independent of those blocks.

- Removed both dead blocks, the associated unused variables, and stale Sentry
  commentary.
- `cargo check --locked --offline`, `cargo fmt --all -- --check`, and
  `git diff --check` passed, with only established unrelated warnings.

### Third-party integrations audit — in progress

The remaining integration surface combines generic external OAuth and service
connections, Google/ICS calendar fetching, ChatGPT OAuth, and locally useful
browser-control code under the same historical `connections` naming. The
removal sequence will first isolate external credential/network features from
the local browser/capture path, then remove the former without treating all
connection code as disposable.

### Product telemetry and crash-reporting removal — complete

The final cleanup removes the remaining product instrumentation rather than
retaining inert shims. It incorporates the completed desktop Sentry handoff
above without changing the separate third-party integration audit.

- Removed all frontend `posthog-js`, `posthog-js/react`, `@sentry/react`, and
  `tauri-plugin-sentry-api` imports, calls, providers, compatibility aliases,
  telemetry-only state/effects, consent UI, identity/settings fields, and test
  mocks. Removed the corresponding packages and unreachable Bun graph.
- Removed the desktop analytics compatibility module and its lifecycle/event
  call sites, analytics UUID/environment setup, generated Sentry permission
  entries, telemetry-only build timestamp, and the obsolete
  `--disable-telemetry` CLI/config surface.
- Removed the root workspace's unused `sentry` declaration and the remaining
  compile-disabled hardware/transport payload code from the resource monitor.
  The monitor still samples local process CPU/memory, emits local debug logs,
  and optionally maintains its bounded rotating resource JSONL file.
- Preserved local browser and engine structured logs plus the independent panic
  hook and `last-panic.log`. Legacy analytics keys remain only inside a serde
  compatibility fixture so older settings files continue to load.
- User-configured PostHog/Sentry service connectors, their UI labels/icons, and
  local secret-redaction patterns are not product telemetry and remain. Static
  URL-detection fixture strings also remain test data.

Verification on Windows on 2026-09-04, with frontend checks repeated on
2026-09-07:

- `bun ./node_modules/typescript/bin/tsc --noEmit` passed.
- Focused Vitest passed 2 files and 10 tests. The production Next static export
  passed (with only the existing `unpdf` dynamic-import warning), and its output
  contained no telemetry SDK or ingest-endpoint marker.
- `cargo check -p screenpipe-engine --locked --offline` and desktop
  `cargo check --locked --offline` passed. `screenpipe-config` passed 26/26
  tests; `screenpipe-engine --lib` passed 529 tests with 2 ignored.
- `cargo fmt --all -- --check`, `git diff --check`, and the full Visual Studio
  2026 Developer PowerShell `cargo build --release --locked --offline` passed;
  the release build took 7m 58s and reported only the two established
  audio/engine warnings. `screenpipe.exe record --help` contains no telemetry
  option.
- `Cargo.lock` and `apps/screenpipe-app-tauri/src-tauri/Cargo.lock` are
  unchanged. `bun.lock` removes only the telemetry packages and unreachable
  transitive graph; its `react-is` lines relocate already-present 16.13.1 and
  17.0.2 resolutions rather than adding or upgrading a version.
- The checkout's `bun run` launcher reports a corrupted `node_modules/.bin`
  remapping before starting `typecheck` or `next build`; direct execution of
  the installed TypeScript, Vitest, and Next entrypoints passed.

### Desktop binding-freshness follow-up — 2026-09-07

- Downloaded locked `assert-json-diff 2.0.2` and the remaining already-locked
  desktop test helpers. Neither `Cargo.lock` nor
  `apps/screenpipe-app-tauri/src-tauri/Cargo.lock` changed.
- The first desktop debug test build used `Ninja Multi-Config`, which resolved
  the known `libsamplerate-sys` layout requirement, but linking failed because
  this separate Cargo workspace does not inherit the root workspace's
  `knf-rs-sys` profile override. Its debug-CRT objects conflicted with
  `whisper-rs-sys` release-CRT objects (`LNK2038`, `RuntimeLibrary`, and
  `_CrtDbgReport`).
- A release-profile retry with single-config Ninja was not a valid substitute:
  it failed because `libsamplerate-sys` could not find the static `samplerate`
  library. Native-linking tests must use Ninja Multi-Config even when the Rust
  profile is release.
- The locked offline debug build and link succeeded with Ninja Multi-Config and
  transient Cargo setting
  `--config 'profile.dev.package."knf-rs-sys".debug-assertions=false'`. The
  first execution then returned `STATUS_DLL_NOT_FOUND`; `dumpbin /dependents`
  identified `libopenblas.dll`, so the process also requires
  `C:\Utils\OpenBLAS\win64\bin` on `PATH`. `OPENBLAS_PATH` alone covers only
  compile/link discovery. `onnxruntime.dll` was already staged correctly.
- With that complete environment, `tauri_bindings_are_current` ran offline:
  0 passed, 1 failed, 180 filtered. The assertion showed that regeneration
  would remove unrelated cloud/account bindings `getCloudToken`,
  `openLoginWindow`, and `setCloudToken`. The generated diff was inspected and
  reverted to preserve this commit's telemetry-only scope. The binding failure
  is now exercised and understood rather than dependency-blocked.
- The exact reusable command and failure-avoidance matrix are recorded in
  `BUILD_NOTES.md` and the workspace `AGENTS.md`.

### Remaining desktop cloud-sync boundary — 2026-09-08

The engine sync CLI, service, and HTTP routes are already gone, but the desktop
still registers the consumer cloud-sync command/state module and starts its
sync and cloud-archive tasks. The overlay also retries a persisted sync
password, while the storage UI and onboarding/account surfaces still call the
retired local `/sync` and `/archive` routes plus Screenpipe-hosted cloud-sync
subscription and checkout endpoints. The checked-in Tauri bindings expose the
same dead commands and types.

`screenpipe-engine` otherwise enables `screenpipe-core/cloud-sync` only to use
the stable machine-ID helper when annotating local memory records. That helper
can move to a plainly named, always-available core module without changing its
existing `~/.screenpipe/machine_id` persistence or any historical database
columns. Local retention is implemented beside the desktop sync code but uses
the authenticated loopback `/retention/configure` route and must be extracted
and preserved.

The generic `screenpipe-sync` workspace crate is not yet unused: the optional
desktop `enterprise-build` path imports it directly for enterprise uploads.
The consumer `cloud-sync` feature edge and default-workspace build edge can be
removed now; deleting the crate itself is deferred until the separately scoped
enterprise removal proves that final caller gone.

### Desktop consumer cloud-sync removal — complete

- Removed desktop cloud-sync/cloud-archive command registration, managed state,
  startup tasks, password retry, settings stores, UI, generated command types,
  and stale `/sync` and `/archive` endpoint tests and documentation. The account
  page no longer advertises or controls data, pipe, memory, or connection sync.
- Extracted the authenticated local retention startup call into `retention.rs`;
  it still uses the resolved local API bearer token and `/retention/configure`.
  No retention behavior or database schema was removed.
- Moved the stable machine-ID helper to `screenpipe-core::machine_id` without
  changing its `~/.screenpipe/machine_id` path or value format. Local memory
  records continue to use it.
- Removed the engine and desktop `cloud-sync` feature edges and the cloud-only
  core modules and direct dependency declarations. `screenpipe-sync` is no
  longer a default workspace member, but remains a workspace crate and an
  optional desktop dependency because `enterprise-build` still imports it;
  deletion is deferred to the separately scoped enterprise-service removal.
- Preserved historical sync columns and old settings-file keys by making no
  destructive migration or persisted-settings rewrite. The vision design note
  now marks its multi-machine sync discussion as retired compatibility history.
- Tracked-code searches found no remaining consumer sync command/state names,
  settings fields, local sync/archive route calls, OpenAPI operations, or
  active `cloud-sync` feature edge. Screenpipe-hosted checkout/subscription
  calls remain only in product-account/provider sales surfaces and are assigned
  to the next account/provider commits. Static URL-detection fixture text and
  the enterprise-only `screenpipe-sync` caller are non-consumer exceptions.

Verification on Windows on 2026-09-08:

- Root `cargo check -p screenpipe-engine --locked --offline` and desktop
  `cargo check --locked --offline` passed in Visual Studio Developer
  PowerShell. The full root `cargo build --release --locked --offline` passed
  with Ninja in 10m 02s and only the two established audio/engine warnings.
- `cargo test -p screenpipe-core --lib --locked --offline` ran 286 tests: 285
  passed and the unrelated, time-sensitive
  `pipes::tests::test_should_run_cron_stale_last_run_waits` failed. This is
  recorded rather than hidden and belongs to the later pipe removal.
- Root `cargo fmt --all -- --check`, direct TypeScript `tsc --noEmit`, the
  direct Next production static export, and `git diff --check` passed. Next
  reported only the existing `unpdf` `import.meta` warning. The separate
  desktop fmt check still reports only its pre-existing `main.rs` `icons`
  module-order drift.
- The exact documented native desktop binding test built, linked, and ran. It
  failed only because checked-in bindings still contain `getCloudToken`,
  `openLoginWindow`, and `setCloudToken`; a generated comparison found no other
  command or type drift. Those three account bindings are intentionally removed
  in the immediately following product-account commit, where the test must pass.
- Initial locked checks correctly rejected the changed direct dependency lists.
  Both lockfiles were refreshed offline. Inspection shows only removals from the
  `screenpipe-core` dependency arrays; no package/version was added, removed, or
  upgraded because the remaining workspace/enterprise graph still uses those
  packages. `bun.lock` is unchanged.

### Product-account boundary — 2026-09-08

The native `get_cloud_token`, `set_cloud_token`, and `open_login_window`
commands are already absent from the Tauri command registry. Only their three
stale checked-in TypeScript bindings remain. Valid frontend callers still reach
those missing commands through the Account settings page, onboarding login
gate, login dialog, provider upsells, and a dead entitlement-gate component.

The active settings provider also persists and refreshes a Screenpipe product
`user` session through `https://screenpi.pe/api/user`, installs a hosted-API
401 interceptor, accepts login/purchase deep links, polls subscription state,
and exposes account, entitlement, Stripe, credit, referral, and hosted API-key
metadata through Rust and generated TypeScript settings types. The tray still
shows plan/upgrade items and emits the checkout event. The Rust recording gate
is already an unconditional local allow, so the remaining native entitlement
parsers and UI tests are unreachable policy scaffolding rather than a retained
capability.

Several later-removal systems currently read the product account only as a
credential or paywall input: Screenpipe cloud AI/transcription, hosted pipes,
remote-device sync, third-party integration upsells, usage/team hooks, and
enterprise policy. They can be decoupled now without enabling a new network
path; their implementations are removed in their separately scoped commits.
ChatGPT OAuth state and provider credentials are independent and must not be
removed in this account commit. The local API bearer key is held in recording
settings and the local secret store, not in the product `user` object, and must
remain unchanged.

Removing the typed Rust `user` field remains settings-compatible because
`SettingsStore.extra` is a flattened catch-all: older serialized `user` data is
accepted and round-tripped as unknown legacy JSON rather than causing a
deserialization failure or migration. Frontend code will likewise stop reading
or refreshing it without rewriting the stored object solely to delete history.

Implementation and post-change evidence:

- Removed the desktop product-account ingress route, login and purchase deep
  links, settings refresh/interceptor, entitlement gates, account/referral
  settings and onboarding UI, tray plan/upgrade state, subscription polling,
  checkout/referral calls, account-conditioned updater state, and their tests.
  The onboarding flow now begins with local recorder permissions.
- Removed the typed native/frontend `user` and `credits` objects and the dead
  entitlement helpers. A native round-trip test proves an older serialized
  `user` object remains readable and preserved in the flattened legacy JSON
  map. No migration deletes historical account data.
- Removed the stale generated `getCloudToken`, `openLoginWindow`, and
  `setCloudToken` bindings after confirming their native commands were already
  absent. `tauri_bindings_are_current` now passes with the documented Windows
  native-test matrix. The generated pending-update binding was also refreshed
  after deleting its obsolete account-auth-required state.
- Later-removal subsystems no longer read or receive the product account token:
  hosted AI/usage, pipes, remote-device monitoring, integrations, team, and
  enterprise policy remain isolated for their own commits. ChatGPT OAuth and
  third-party credentials were not conflated with the Screenpipe account.
- Tracked searches find no remaining product login commands/callers, desktop
  `/api/user`, checkout/purchase/referral route, plan/entitlement field use, or
  account/referral settings route. Remaining `auth_required` names belong to
  the browser extension's preserved local API bearer authentication. Hosted AI
  subscription code under `packages/ai-gateway` is part of the next provider
  boundary, not a desktop product session.
- The local API bearer configuration and secret store were unchanged. Root
  `Cargo.lock`, desktop `Cargo.lock`, and desktop `bun.lock` are unchanged at
  SHA-256 `52102E97DFD54A20340F1AF03EAB3D7B8F3EA2AACE39EB6910C4537445FAC57E`,
  `02B00FA9019FE750A8892C7B729DCC0C55E32388383228A504B4F59D5F3EAEBB`, and
  `758A49562E49967F8B91F1169D64DA7C74242051CF1544889734BE0E47A3701D`.

Verification on Windows on 2026-09-08:

- `cargo fmt --all -- --check`: passed from the root workspace. Direct
  `rustfmt --edition 2021 --check` passed for every changed Rust file except
  `main.rs`; the full desktop `cargo fmt --all -- --check` still reports only
  the documented pre-existing `icons` module-order difference in `main.rs`.
- Locked/offline desktop `cargo check`: passed in Visual Studio Developer
  PowerShell with the normal Ninja/OpenBLAS/ORT environment.
- `store::tests`: 17 passed with Ninja Multi-Config, the transient
  `knf-rs-sys` dev CRT override, and OpenBLAS on runtime `PATH`.
- `tauri_bindings_are_current`: 1 passed with the same documented native-test
  matrix.
- Direct TypeScript `tsc --noEmit`: passed.
- Direct Vitest run: 42 files and 536 tests passed.
- Direct Next production build: passed; it emitted only the existing `unpdf`
  `import.meta` warning.
- `cargo build --release --locked --offline`: passed in 51.68 seconds from the
  root workspace with the normal release environment.
- `git diff --check`: passed. The complete source diff and all three lockfiles
  were inspected before commit.

### Hosted Screenpipe AI gateway boundary — 2026-09-09

The Screenpipe product account is gone, but the desktop and core Pi runtimes
still construct an implicit `screenpipe-cloud` provider backed by
`api.screenpipe.com`, write its product token into Pi configuration and process
environment, install a gateway-only web-search extension, and expose hosted
model discovery, usage, media analysis, OCR analysis, and suggestions in the
desktop UI. The independently deployable `packages/ai-gateway` worker remains
the implementation for those hosted chat, search, transcription, voice,
provider-proxy, usage, and subscription endpoints.

This boundary is separable from local Pi orchestration: Pi installation,
sessions, local Screenpipe API skills, and the existing `native-ollama`
provider use only the authenticated loopback recorder API and Ollama at
`http://localhost:11434/v1`. Existing persisted `screenpipe-cloud`, `pi`, or
`pi-agent` presets can be accepted as legacy input and mapped non-destructively
to a local Ollama preset rather than retaining a hidden network fallback or
deleting settings history. Direct OpenAI, Anthropic, ChatGPT OAuth, arbitrary
remote-model, and hosted Tinfoil redaction paths are distinct later removal
scopes; deterministic local regex/ONNX redaction is not part of this change.

The previously removed external transcription transports and workflow
classifier have not reappeared in the recorder runtime. Their remaining legacy
serialized setting names and test fixtures do not provide implementations and
will be handled in compatibility-aware cleanup commits after this gateway
boundary.

Post-change evidence:

- Removed the independently deployed `packages/ai-gateway` worker and its own
  Bun lockfile, plus all desktop/core hosted model discovery, token propagation,
  quota/subscription UI, cloud media/OCR analysis, hosted web search, and cloud
  suggestion paths. No active source retains `api.screenpipe.com`,
  `api.screenpi.pe`, `SCREENPIPE_API_URL`, the removed commands, or their
  generated bindings; matches left in captured-text test data are inert fixtures.
- Pi remains available with a local default at
  `http://localhost:11434/v1` / `ministral-3:latest`. Legacy hosted preset IDs
  and providers are read non-destructively and resolved to that local provider;
  the obsolete `screenpipe` entries are removed from Pi `models.json` and
  `auth.json` while unrelated third-party providers and credentials are kept.
- Local recorder OCR, the local API bearer token/secret store, deterministic
  local redaction, persisted settings, and historical database fields were not
  removed. Direct OpenAI, Anthropic, ChatGPT OAuth, generic remote endpoints,
  hosted Tinfoil, external-transcription compatibility, and desktop pipes remain
  isolated for their own commits. Litepipe was not consulted.
- Root and desktop `Cargo.lock` changes remove only the now-unused direct
  `arc-swap` edges from `screenpipe-core` and `screenpipe-app`; the transitive
  package remains for retained crates. Their reviewed SHA-256 values are
  `24406636290302B382F08F98DBA4BFE32E7881B3CD5C4761955D47A50FF4D1FE` and
  `4A5E7E23BD4BEA7B34F3265FC3FFB5FA3B62A0B2FEC793C3D022C08B97C0DCCB`.
  The desktop Bun lockfile is unchanged at
  `758A49562E49967F8B91F1169D64DA7C74242051CF1544889734BE0E47A3701D`.

Verification on Windows on 2026-09-09:

- `cargo fmt --all -- --check`: passed from the root workspace.
- Locked/offline root checks for `screenpipe-core` and `screenpipe-engine`, and
  the locked/offline desktop check: passed in Developer PowerShell with the
  documented normal Ninja/OpenBLAS/ORT environment.
- `tauri_bindings_are_current`: 1 passed with Ninja Multi-Config, the transient
  `knf-rs-sys` dev CRT override, and OpenBLAS on runtime `PATH`.
- Desktop Pi model configuration tests: 15 passed; the legacy preset settings
  migration regression: 1 passed; local suggestion tests: 9 passed with 3
  benchmarks ignored. Engine preset tests: 9 passed.
- Direct TypeScript `tsc --noEmit`: passed. The first direct Vitest run exposed
  three stale hosted-account quota-copy assertions (533 passed); after replacing
  them with provider-neutral assertions, the full rerun passed 42 files and 535
  tests. Direct Next production build passed with only the existing `unpdf`
  `import.meta` warning.
- `cargo build --release --locked --offline`: passed in 11m 29s from the root
  workspace with the documented release environment and only known warnings.
- `git diff --check`: passed. Modified-source diffs, generated bindings and
  capability schema, deletion inventory, dependency manifests, both Cargo
  lockfiles, and the desktop Bun lockfile were inspected before commit.

### Remaining external AI/provider boundary — 2026-09-09

After removal of the Screenpipe gateway, direct OpenAI and Anthropic API-key
providers, the arbitrary OpenAI-compatible `custom` provider, ChatGPT OAuth,
and hosted Tinfoil text/image redaction remain reachable. The model providers
span desktop/core Pi configuration, pipe preset resolution, the engine preset
CLI, settings and rewind editors, startup validation, model-catalog fetches,
generated types, and persisted enterprise preset policy. ChatGPT additionally
has native OAuth commands, secret-store tokens, onboarding/connections UI, and
refresh/model-discovery clients. Tinfoil has selectable settings, desktop worker
construction, engine/config flags, a remote redaction adapter crate surface,
environment credentials, an example probe, and a pinned SDK dependency. All can
send recorded content or credentials to external services and are outside the
local Windows-first product.

This boundary does not require removing Pi: `native-ollama` already has a
complete local configuration and process path through
`http://localhost:11434/v1`. Existing remote presets and privacy-backend values
must remain deserializable, so they will be migrated or interpreted as local
without deleting stored settings. Deterministic regex and ONNX redaction remain.
The separate OpenAI-compatible audio-transcription configuration is excluded,
as are ordinary URL privacy filters and the local API bearer secret. Shared HTTP
clients and broad desktop HTTPS capability still serve retained or
later-removal subsystems; only provider-specific edges can be narrowed here.

Post-change evidence:

- Removed the native ChatGPT OAuth module, token refresh/client commands,
  onboarding and connection UI, preset helper, generated command/type bindings,
  and provider-specific capability URLs. Searches find provider endpoints only
  inside compatibility fixtures that prove old settings are migrated locally.
- Removed direct OpenAI, Anthropic, and arbitrary custom-model selection,
  credential injection, model discovery, and request paths from desktop, core
  Pi, pipe preset resolution, and engine preset CLI code. ScreenWise now writes
  and launches only the fixed loopback Ollama provider at
  `http://localhost:11434/v1`; the editor no longer promises a configurable
  endpoint that the runtime would ignore.
- Existing hosted/direct/unknown presets remain readable. Store, pipe, and CLI
  boundaries map them to `native-ollama` / `ministral-3:latest`, force the
  loopback URL, and clear or ignore provider credentials without deleting the
  historical record or touching the local API bearer token.
- Removed hosted Tinfoil text/image clients, examples, configuration branches,
  endpoint/key handling, and the pinned Rust SDK. Legacy `piiBackend` strings
  deserialize but resolve to deterministic local regex plus ONNX/OPF redaction;
  no database or settings migration was made destructive.
- The retained Pi distribution still requires `@anthropic-ai/sdk` as an eager
  package dependency even when running Ollama, so removing that JavaScript
  package would remove local Pi rather than a selectable provider. ScreenWise
  always passes `--provider ollama` and removes its known legacy remote entries
  from Pi configuration. It preserves unrelated user-managed entries in the
  shared Pi config; fully isolating Pi into an app-owned provider registry is a
  later retained-capability architecture decision.
- The inbound, authenticated localhost OpenAI-compatible audio transcription
  endpoint is retained; it is not an outbound transcription provider. The
  previously removed external transcription transports and workflow classifier
  have not reappeared. Broad HTTPS capability remains until the separately
  reachable integration and enterprise clients are removed. Litepipe was not
  consulted.
- Root and desktop `Cargo.lock` changes contain removals only: the Tinfoil SDK
  plus its now-orphaned dependency graph. No package version changed. Reviewed
  SHA-256 values are
  `8C58CF60F1D1E51346CDBEF830666D56E43502BC7A9F593D45D20A1A30242E94`
  and
  `4CE096095FF1430E626D3EB4AD755C6C48EF9606138DA4098836710B5F15CB0E`.
  The desktop Bun lockfile is unchanged at
  `758A49562E49967F8B91F1169D64DA7C74242051CF1544889734BE0E47A3701D`.

Verification on Windows on 2026-09-10:

- `cargo fmt --all -- --check`: passed from the root workspace.
- Locked/offline root checks for `screenpipe-core` and `screenpipe-engine`, and
  the locked/offline desktop check: passed in Developer PowerShell with the
  documented normal Ninja/OpenBLAS/ORT environment.
- `tauri_bindings_are_current`: 1 passed with Ninja Multi-Config, the transient
  `knf-rs-sys` dev CRT override, and OpenBLAS on runtime `PATH`.
- Desktop Pi tests: 24 passed and 1 environment/process test was ignored.
  Desktop store tests: 19 passed. Engine preset tests: 9 passed, including the
  legacy-provider migration regression.
- The first full `screenpipe-redact` run exposed two Windows-only path
  assertions and scheduler-dependent in-memory SQLite worker tests. The path
  checks now compare path components, and the worker fixture uses one in-memory
  connection plus a bounded readiness wait. The full rerun passed 90 library
  tests and 3 worker integration tests; 1 doc test was ignored.
- Direct TypeScript `tsc --noEmit`: passed. A Bun-hosted direct Vitest attempt
  reproduced the checkout's launcher/runtime interop problem (`z.object` was
  undefined in one suite; 41 files and 533 tests passed before exit 1). Running
  the installed Vitest entrypoint with Node passed all 42 files and 533 tests.
  The installed Next entrypoint production build passed with only the existing
  `unpdf` `import.meta` warning.
- `cargo build --release --locked --offline`: passed in 10m 19s from the root
  workspace with the documented release environment and only known warnings.

### Desktop pipes and workflow automation boundary — 2026-09-10

The engine pipe HTTP API, scheduler routes, and CLI automation commands are
already removed, but the desktop still starts `PipeManager`, creates and
recovers the pipes directory, runs its scheduler, bridges pipe stdout into
agent events, and starts the suggestion scheduler. Generated capabilities and
Tauri E2E commands still grant sidecar pipe operations. The frontend continues
to poll removed `/pipes` endpoints and exposes pipe store/install/share/update,
configuration, run history, scheduled/upcoming sections, and pipe-specific chat
conversation state. Core still exports the pipe manager, bundled pipe skills,
workflow event types, meeting-trigger emissions, preset scans, and pipe disk
accounting.

These paths are stale and separable from retained local chat. Standalone Pi
sessions, their local Screenpipe API/search bridge, title generation, and the
fixed-loopback Ollama provider do not require desktop pipe navigation, hosted
store behavior, pipe scheduling, or pipe-specific event routing. Ordinary
recording schedules and meeting privacy/detection also do not require workflow
trigger emission. Historical pipe files and settings can remain on disk and in
serialized compatibility fields; this removal will not delete user data.

### Desktop pipes and workflow automation removal complete — 2026-09-10

- Removed desktop `PipeManager` startup/shutdown, scheduler recovery, built-in
  pipe installation, pipe-output event bridging, suggestion scheduling,
  pipe-specific Tauri commands/capabilities, disk accounting, workflow event
  types/emissions, engine preset scans, and the dormant core pipe executor.
- Removed pipe navigation, store/install/share/update/configuration UI,
  notification mute actions, favorites, remote monitoring, onboarding,
  scheduling/run-history views, generated bindings, and E2E helpers that called
  removed `/pipes` or pipe-stream commands. Current docs and bundled local API
  skills no longer advertise pipe automation.
- Retained local Pi chat and its fixed-loopback Ollama configuration, local
  capture/search/MCP access, owned-browser support, ordinary meeting detection,
  and local API bearer authentication. Old chat files with `kind` or
  `pipeContext` remain readable as ordinary chats. A compatibility-only
  deduplication exemption is carried forward when those files are saved so
  repeated historical runs with the same templated prompt remain individually
  accessible; focused storage/store regression tests passed (47 tests). Old
  notification/settings keys and historical pipe database tables/files are
  ignored rather than migrated or deleted. No database migration was added.
- Root and desktop lockfiles only remove `cron` and pipe-only direct dependency
  edges from `screenpipe-core`, plus the now-unused `chrono` edge from
  `screenpipe-events`. No package version changed and neither Bun lockfile
  changed.
- `cargo check --locked --offline` for the root default members passed in the
  documented Visual Studio Developer PowerShell environment. A prior ordinary
  shell `--workspace` attempt failed in native setup/target-gated crates and is
  not counted as validation. Focused locked offline core/events/database tests
  passed: 228 tests total.
- The documented Ninja Multi-Config desktop binding check passed when run alone.
  A full parallel desktop test run compiled and passed 158 tests, ignored 4,
  but its freshness test raced the binding-export test and read the generated
  file while it contained one byte; the required isolated rerun passed. The
  initial locked run also correctly refused the stale desktop lock until a
  controlled offline resolution pruned the removed dependencies.
- Direct TypeScript `tsc --noEmit` passed. The first full Vitest run exposed an
  orphan test for a deleted component; after cleanup the final direct Node
  Vitest run passed all 35 files and 403 tests. The installed Next production
  entrypoint passed with only the existing `unpdf` `import.meta` warning.
- The Node skill-generation attempt failed because the ESM script references
  `__dirname`; running the installed Bun entrypoint regenerated the checked-in
  skill binding successfully. `cargo fmt --all -- --check`, `git diff --check`,
  and `cargo build --release --locked --offline` passed; the release build took
  9m 17s with only established unrelated warnings.

### Third-party connections and calendars boundary — 2026-09-10

The engine connections facade is already removed, but `screenpipe-connect`
still compiles a large external-service registry, provider credential models,
OAuth clients, remote proxy/sync helpers, and Google/calendar transports. The
desktop still registers OAuth, Google Calendar, ICS/webcal, calendar polling,
remote-sync commands, schedulers, settings, and frontend entry points. These
paths retain hosted and third-party network destinations, credentials, and
generated command bindings despite no longer serving the Windows-first local
recorder.

The retained browser module is a distinct local capability: engine routes and
Pi chat use `BrowserRegistry`/`OwnedBrowser` for user-directed browser capture
and control. Native Windows/macOS calendar reads are also local OS integration,
and opt-in mDNS is local-network discovery. This commit will therefore remove
generic connectors, third-party OAuth, remote sync, Google/ICS calendar
transports, and the outgoing arbitrary MCP proxy while keeping those three
local capabilities in `screenpipe-connect`. Ordinary URL/window privacy
filters, local meeting detection, recorder settings, the local secret store,
and local API bearer authentication remain out of scope. Historical
connection/calendar settings and secrets will be ignored, not migrated or
deleted.

### Third-party connections and calendars removal complete — 2026-09-10

- Removed the generic service registry and its provider implementations,
  credential/OAuth lifecycle, remote proxy and sync clients/schedulers,
  Google Calendar and ICS/webcal transports, WhatsApp gateway, outgoing
  user-defined MCP proxy, desktop command/startup surfaces, navigation,
  onboarding, settings, chat discovery, citations, and generated bindings.
- Retained `screenpipe-connect` only for owned-browser control, native
  Windows/macOS calendar access, and opt-in local-network mDNS. A final review
  found that Coming Up still called an already-removed calendar HTTP route;
  it now uses the retained `calendar_get_events` Tauri command and has focused
  success/failure coverage. Local Pi/Ollama, URL/window privacy filtering,
  local API bearer authentication, and the encrypted local secret store remain.
- Removed legacy OAuth/connection secret import and refresh at startup without
  deleting old files, settings, SQLite rows, or keys. Permission hardening for
  existing secret-like files remains. Historical chat citation metadata still
  normalizes through its compatibility path.
- Removed the desktop helper that would have registered an unpinned
  `screenpipe-mcp@latest` process with the local bearer token. The audited local
  API/browser skill now documents only existing local routes and local
  transcription engines; generated skill content was refreshed with Bun.
- Root lock resolution removed 64 package records and desktop lock resolution
  removed 59. A tuple/source/checksum comparison found no added packages and no
  surviving package version, source, or checksum changes. All five Bun
  lockfiles are unchanged.
- `cargo fmt --all -- --check`, `git diff --check`, and the root default-member
  `cargo check --locked --offline` passed. Locked offline library tests for
  connect, core, secrets, and engine passed 691 tests with 2 ignored. The
  isolated Ninja Multi-Config `tauri_bindings_are_current` test passed with the
  documented CRT override and OpenBLAS runtime path.
- Direct TypeScript checking passed; direct Vitest passed all 32 files and 378
  tests after deleting stale connection-prompt assertions. The installed Next
  production entrypoint passed with only the existing `unpdf` `import.meta`
  warning. `cargo build --release --locked --offline` passed in 8m 04s in the
  documented Visual Studio Developer PowerShell/Ninja release environment with
  only established unrelated warnings.

### Enterprise services and obsolete sync crate boundary — 2026-09-10

Enterprise behavior remains runtime-reachable through the desktop
`enterprise-build` feature and through hidden-UI policy code that is compiled
even in consumer builds. The feature starts a five-minute worker that reads
local OCR, audio, input/UI events, frames, and memories, then sends JSONL to
Screenpipe enterprise ingest or presigned storage endpoints. The frontend also
contains licence activation, policy/heartbeat, team-management, fleet/update
controls, and hosted enterprise navigation. Enterprise packaging workflows,
the separately licensed `ee` tree, bundled team skills, and local MCP team
proxy tools retain additional hosted paths and credentials.

`screenpipe-sync` now has exactly one code consumer: the enterprise upload
implementation. Removed consumer cloud sync and archive paths no longer use
it, so the crate, its feature edge, and its exclusive dependencies can leave
with the enterprise boundary. The local machine-ID helper is already in
`screenpipe-core` and remains required by local memories and database records;
it will be preserved. Historical `machine_id`, `sync_id`, `synced_at`, team,
enterprise, and update-policy data will remain readable or inert rather than
being destructively migrated. The separate consumer updater, local API bearer
authentication and secret store, native calendar, owned-browser control,
Pi/Ollama, capture, search, privacy, and local diagnostics are outside this
removal.

### Enterprise services and obsolete sync crate removal complete — 2026-09-10

- Removed enterprise telemetry/upload workers, licence/team commands, policy
  locks, hidden-agent UI controls, fleet/update branches, hosted team APIs and
  MCP tools, enterprise desktop/package UI, generated bindings, release/SDK
  workflows, packaging configuration, Intune tooling, and the separately
  licensed `ee` source/SDK tree. Consumer startup, tray/window behavior, and
  the ordinary official-build updater remain.
- Removed `screenpipe-sync` after confirming its last consumer was the deleted
  enterprise upload path. The root and desktop locks only remove
  `screenpipe-sync`, `wiremock`, `assert-json-diff`, `deadpool`, and
  `deadpool-runtime`; there are no added lock lines or surviving package
  version/source/checksum changes. Desktop also drops the enterprise-only
  optional edge while the shared transitive package remains where still used.
  Bun locks are unchanged. Removed root-workspace exclusions for the already
  empty, untracked `screenpipe-integrations` and `screenpipe-team-memory`
  placeholders; neither contained source or affected the resolved workspace.
- Preserved `screenpipe-core`'s local machine-ID helper, local memory/database
  callers, historical sync columns/migrations, local API bearer authentication,
  the encrypted secret store, native calendar, owned browser, Pi/Ollama, and
  all capture/search/privacy/logging paths. Historical enterprise/team settings
  and `enterprise.json` are not deleted; they are no longer interpreted.
  Previously generated `screenpipe-team` Pi skills are retired through the
  existing deprecated generated-skill cleanup before a session starts.
- Root `cargo check --locked --offline` passed. Focused Pi tests passed 3 tests.
  The exact Ninja Multi-Config desktop suite passed 153 unit tests with 4
  ignored plus the shutdown integration test; the isolated generated-binding
  freshness test passed. Direct TypeScript checking passed, direct Vitest
  passed 32 files and 378 tests, and the installed Next production entrypoint
  passed with only the existing `unpdf` `import.meta` warning.
- `cargo fmt --all -- --check`, `git diff --check`, and the final documented
  Ninja `cargo build --release --locked --offline` passed; the release build
  took 8m 32s with only established unrelated warnings. An earlier release
  attempt was deliberately interrupted after review produced a final source
  edit and is not counted as validation.

### External transcription settings residue boundary — 2026-09-10

The external transcription transports removed in `a879dbebe` have not been
reintroduced, but desktop configuration still exposes and accepts
`screenpipe-cloud`, Deepgram, `deepgram-live`, and arbitrary
OpenAI-compatible transcription selections, credentials, endpoints, headers,
model discovery, and raw-audio upload controls. Startup/store resolution only
falls back for the removed Screenpipe cloud engine or a missing Deepgram key;
a configured Deepgram or OpenAI-compatible value can still survive as the
selected engine even though the Windows-first recorder has no supported remote
transport.

This compatibility cleanup will remove selectable remote-provider UI and
credential handling, normalize every removed engine/provider to a supported
local transcription engine before startup, and keep legacy serialized fields
readable without sending them anywhere. The authenticated local inbound
`POST /v1/audio/transcriptions` route is a separate localhost API and remains.
Historical database provider/engine strings and speaker labels also remain so
old recordings and meetings stay readable. Local Whisper, Parakeet, Qwen, and
platform-gated local engines are retained.

### External transcription settings residue removal complete — 2026-09-10

- Removed desktop selections, credential fields, endpoint/model discovery,
  diagnostics, raw-audio upload controls, and stale guidance for Screenpipe
  Cloud, Deepgram, and arbitrary remote transcription servers. Retired
  credential fields remain deserializable for old `store.bin` files but are no
  longer serialized or exported in the generated TypeScript settings contract.
- Centralized normalization of persisted settings and engine overrides so only
  supported local engines reach recorder startup. Retired and unknown values
  resolve to local Whisper Turbo (quantized); meeting live notes use the
  selected local engine or remain disabled. Startup persists the normalized
  choices without deleting historical database provider/engine columns or
  labels.
- Removed inert meeting-streaming credential/endpoint plumbing and the retired
  cloud-audio E2E seed. Confirmed no Deepgram transport/dependency or external
  transcription client was reintroduced. The authenticated localhost
  `POST /v1/audio/transcriptions` endpoint, local model acquisition, local
  Whisper/Parakeet/Qwen paths, audio capture, and speaker/meeting behavior
  remain.
- `screenpipe-config` passed 29 locked offline tests. Focused engine recording
  configuration tests passed 14 tests and `cargo check -p screenpipe-engine
  --locked --offline` passed. The exact Ninja Multi-Config desktop suite passed
  152 unit tests with 4 ignored plus its shutdown integration test; generated
  bindings were regenerated and `tauri_bindings_are_current` passed.
- Direct TypeScript checking passed. Direct Vitest passed all 32 files and 378
  tests. The installed Next production entrypoint passed with only the existing
  `unpdf` `import.meta` warning. Root default-member `cargo check --locked
  --offline`, `cargo fmt --all -- --check`, and `git diff --check` passed.
- A direct `screenpipe-audio` library-test attempt could not start offline
  because its dev-only `infer 0.15.0` package is not cached; no network fetch or
  dependency change was made to bypass that limitation. The changed audio
  integration compiled in the root check and desktop test matrix. Cargo and Bun
  lockfiles are unchanged. The documented Ninja `cargo build --release
  --locked --offline` passed in 8m 49s with only the established unrelated
  warnings.

### Final workspace/model reachability boundary — 2026-09-10

Post-removal `cargo metadata --no-deps` shows that
`screenpipe-audio-eval` and `screenpipe-meeting-eval` are standalone developer
harnesses: no production package depends on either, but both are included in
the default bare-workspace build. They pull the full local audio/model and
engine graphs into ordinary validation even though they are only invoked by
explicit evaluation commands. They will remain workspace members so the tools
and workspace inheritance stay available, but will leave `default-members`.

The remaining model and cross-platform edges are reachable or structurally
required. Windows uses the local Qwen/Parakeet and ONNX redaction features;
DirectML is a Windows-capable optional audio path. Apple Intelligence,
Parakeet-MLX, Metal/CUDA/Vulkan options, and `screenpipe-rfdetr-mlx` are
target/feature-gated, and the MLX detector must remain a workspace member for
manifest inheritance even though Windows excludes it from default builds.
`screenpipe-connect` remains used by owned-browser control, native calendar
access, and opt-in local-network mDNS. No crate, model, feature, or target-gated
source beyond the two evaluation default-member entries is proven safe to
remove in this audit. The audit also found an unused direct `md5` dependency
left on `screenpipe-engine` by cloud sync and an unused duplicate desktop
`hostname` edge; engine health/database diagnostics retain the actual
`hostname` dependency.

### Final workspace/model reachability audit complete — 2026-09-10

- Removed the two evaluation-only crates from `workspace.default-members`
  while retaining them as explicit workspace members. Removed the unused
  cloud-sync `md5` dependency from `screenpipe-engine` and the unused duplicate
  desktop `hostname` edge. Engine health and database diagnostics retain their
  required `hostname` dependency.
- Complete root and desktop lock diffs contain only the `md5 0.7.0` package
  record and the corresponding `screenpipe-engine`/desktop dependency-list
  entries; no surviving version, source, or checksum changed. Bun lockfiles
  are unchanged. Offline lock regeneration itself could not run because the
  cached `cidre` Git checkout lacks `refs/remotes/origin/HEAD`; the reviewed
  removals were applied directly and both locked offline Cargo checks validate
  the resulting resolutions.
- Final residue searches found no active cloud-sync, product-account/login,
  hosted AI/transcription, pipe scheduler/store, generic connector/OAuth,
  enterprise upload, team, or fleet command path. Compatibility-only
  `include_cloud` rejection, historical `cloud://` frame records, retired
  provider normalization, and deprecated generated-skill cleanup remain so old
  clients/data/settings fail safely or stay readable without network access.
- Preserved network-capable boundaries are outside the removed subsystems and
  remain explicit: the consumer updater, user-initiated support/log upload,
  user-opened links and owned-browser navigation, opt-in LAN mDNS, and pinned
  model/tool acquisition when local artifacts are missing. Pi/chat is retained
  in genuinely local Ollama-only mode; browser control and native calendar
  remain local capabilities. No owner architecture choice is required for the
  simplification sequence completed here.
- `cargo metadata --no-deps` confirms neither evaluation crate is a default
  member. Root and desktop `cargo check --locked --offline`, `cargo fmt --all
  -- --check`, and `git diff --check` passed with only the established
  unrelated warnings. The documented Ninja `cargo build --release --locked
  --offline` passed in 8m 27s.

### Offline PII model provisioning boundary — 2026-09-10

Selecting Smart PII removal currently starts runtime HTTPS acquisition of the
local text ONNX and image RF-DETR weights when their cache files are absent or
invalid. The downloads are checksum-verified, but they violate the product
rule that ScreenWise itself never initiates Internet requests. If the preferred
text ONNX path fails, startup also attempts the approximately 2.8 GB OPF model;
there is no Windows quality comparison demonstrating that this heavyweight
fallback improves on the approximately 167 MB ONNX pack. ONNX covers every OPF
label plus the additional `Sensitive` class, while deterministic regex already
runs before local model inference.

This boundary will remove OPF and every PII adapter HTTP client, retain the
published source locations and pinned SHA-256 manifests as provisioning
instructions, and require the corresponding locally verified ONNX text or
RF-DETR image pack before each Smart feature can be enabled. Basic regex
remains model-free.
Startup will independently reverify configured Smart models so files removed
after enablement cannot trigger network or falsely start an unverified worker.
The obsolete backend selector is removed rather than retained for baseline
settings compatibility; no database migration is involved. The macOS-only MLX
adapter remains target-gated but its automatic acquisition path will also be
removed so the no-request invariant holds across retained source.

### Offline PII model provisioning complete — 2026-09-10

- Removed the OPF adapter, feature edges, integration test, pinned Git
  dependency, and its Candle/OPF dependency graph. The ONNX text model is now
  the only supported semantic text path; deterministic regex remains the
  always-local, model-free fallback.
- Removed automatic Hugging Face acquisition from the ONNX text, RF-DETR ONNX
  image, and target-gated RF-DETR MLX adapters. Each adapter now performs a
  side-effect-free, streaming SHA-256 check of explicitly provisioned files
  before loading. Missing or invalid text files leave the text worker in
  regex-only mode; missing or invalid image files prevent that image worker
  from starting. No PII verification/load path creates directories, writes
  partial files, or opens a network client.
- Added a read-only desktop model-status command and settings UI that lists the
  exact external download source, destination, and expected SHA-256 for every
  file. Text and image Smart features are independently gated on their own
  verified model packs. Basic regex remains selectable without any model.
- Removed the obsolete `piiBackend` setting, CLI flag, restart key, and
  generated TypeScript field. Per the product decision that baseline data has
  no migration value, no compatibility shim or database migration was added;
  Serde simply ignores that now-unknown field in an old settings document.
- Root and desktop Cargo lockfiles remove OPF plus its now-orphaned Candle,
  GEMM, tokenization, Metal, and support packages. Retained package versions,
  sources, and checksums did not change; remaining dependency-list edits are
  Cargo's expected disambiguation cleanup after duplicate versions vanished.
  Reviewed lockfile SHA-256 values are
  `5E8481B13C17A93B8C622046CA61D4AC3EA12CA345F0D9FDAD6E9A20659EE269`
  and
  `930F536C2F2101581B356E770C31830C4BBAD515842009E5C660FE64C0AEC9D0`.
  The desktop Bun lock is unchanged at
  `758A49562E49967F8B91F1169D64DA7C74242051CF1544889734BE0E47A3701D`.
- Searches find no executable OPF, `opf-text`, `load_or_download`,
  `ensure_model_present`, PII `reqwest`, or `piiBackend` edge. Historical
  references remain only in earlier audit notes. The separate macOS/AArch64
  Parakeet MLX build-time shader acquisition is not PII or Windows-reachable
  and remains a later global no-Internet hardening boundary. Litepipe was not
  consulted.

Verification on Windows on 2026-09-10:

- `cargo fmt --all -- --check`, locked/offline root checks, and `git diff
  --check`: passed. `screenpipe-redact` with the supported `onnx-cpu` feature
  passed 96 unit tests and 3 integration tests; 1 doc test was ignored.
- A first DirectML-feature test build reached linking but failed on
  `OrtSessionOptionsAppendExecutionProvider_DML` because the checked-in ORT
  1.22.0 runtime is the documented CPU build. It is not counted as a passed
  DirectML validation; the supported CPU matrix above passed.
- The full desktop native run passed 151 tests and ignored 4, except the known
  concurrent binding-export/freshness race: the freshness test observed the
  generated file while the exporter had truncated it. Regeneration passed,
  and the required isolated `tauri_bindings_are_current` rerun passed with
  Ninja Multi-Config, the transient `knf-rs-sys` CRT override, and OpenBLAS on
  runtime `PATH`. An earlier binding attempt outside Developer PowerShell
  failed on missing `stdint.h` and is not counted.
- Direct Node Vitest passed all 33 files and 380 tests. The Bun-hosted attempt
  reproduced the documented Zod launcher interop failure after 32 files and
  378 tests and is not counted. Direct TypeScript `tsc --noEmit` passed, and
  the installed Next production build passed with only the established
  `unpdf` `import.meta` warning.
- `cargo build --release --locked --offline` passed in 6m 15s from the root
  workspace using Visual Studio Developer PowerShell and the documented
  Ninja/OpenBLAS/ORT release environment, with only established warnings.

### Remaining application-controlled Internet boundary — 2026-09-10

After PII acquisition removal, runtime reachability still finds five
ScreenWise-controlled non-loopback request classes: desktop update
check/download/rollback, hosted support-log upload, hosted changelog fetch,
automatic Google/Gstatic favicon images, and the local Pi bootstrap fallback
that downloads PortableGit when Bash is missing. None is needed for Windows
capture, OCR, local audio/transcription, SQLite search, local authenticated API,
or local Ollama chat. Local structured logs and `last-panic.log` remain useful
without upload; local application icons can replace remote favicons; missing
Git Bash can be reported as an explicit prerequisite instead of acquired.

This combined boundary will remove those independently safe egress paths and
their stale UI/settings/bindings/dependencies together to avoid repeated full
desktop link cycles. It deliberately preserves loopback HTTP/WebSocket traffic,
local API bearer authentication, local Ollama, owned-browser control, captured
URL handling/privacy filters, and explicit user-directed links opened in the
system browser. Audio model acquisition is larger and remains a separate
explicit-provisioning commit. Baseline database/settings migration is not a
constraint; obsolete updater settings can be removed rather than shimmed.

Pi package bootstrap is a separate material boundary: desktop startup and the
core executor can still invoke Bun package installation, while a running Pi
agent can intentionally use its shell and owned-browser tools to reach remote
sites. This commit removes only the unrelated PortableGit fallback. A verified,
offline Pi distribution and the policy for user-directed agent network tools
need an owner decision after the independent model/build acquisition paths are
closed.

### Application-controlled desktop egress closure complete — 2026-09-10

- Removed the Tauri updater implementation, plugin, permissions, endpoints,
  signing configuration, update/rollback commands, pending-update state,
  settings, banners, menu actions, E2E harness, and generated TypeScript
  bindings. The application no longer checks, downloads, installs, or rolls
  back releases. Explicit system-browser links remain user-directed.
- Removed hosted support-log uploads from normal, onboarding, and failure UI,
  then removed the residual arbitrary signed-URL upload command. Local log
  persistence, log-folder access, resource diagnostics, and `last-panic.log`
  remain available without transmission.
- Removed hosted changelog fetching/dialog/deep links, automatic Google/Gstatic
  favicon requests, and Google Fonts imports. Timeline/settings now use local
  application or Lucide icons; small overlay windows use Windows-local
  Cascadia Mono/Consolas fallbacks.
- Removed the unused frontend/native Tauri HTTP plugin and its unrestricted
  HTTP permission. A complete frontend reachability audit now finds only
  loopback ScreenWise API/helper/Ollama requests. Remaining external URLs are
  inert text or explicit user actions through the system/owned browser.
- Removed PortableGit download, verification, extraction, cache cleanup, and
  fallback execution. Pi now discovers user-provisioned Git Bash or `bash.exe`
  on `PATH` and reports the prerequisite when absent; this does not remove the
  separately retained local Pi/Ollama capability.
- Regenerated the command bindings and Windows/desktop Tauri schemas. Because
  Tauri only regenerates host-platform schema files, the identical obsolete
  updater/HTTP permission nodes were mechanically pruned from the checked-in
  macOS/Linux schemas and all six JSON schema files were parse-validated.
  Searches find no updater command/plugin/permission, hosted log-upload edge,
  changelog runtime route, remote favicon/font load, Tauri HTTP plugin, or
  PortableGit acquisition path.
- No database migration or settings compatibility shim was added. Obsolete
  updater settings are simply gone, consistent with the decision not to
  migrate baseline Screenpipe data.
- The root Cargo lock is unchanged at
  `5E8481B13C17A93B8C622046CA61D4AC3EA12CA345F0D9FDAD6E9A20659EE269`.
  The reviewed desktop Cargo lock removes only the updater, HTTP plugin, their
  orphaned packages, and dependency features enabled solely by those plugins;
  its SHA-256 is
  `BC527C35497626ACB7B9CD303033CDB4FA3A86B9BE93E347CEA18FB2AB0B70A4`.
  The Bun lock removes only the two matching plugins and their private API
  entries; its SHA-256 is
  `2BF22C910039145D023EAF256589C76CB5DF66E1A7912A7E51C7F40FF1568BDA`.

- Root and desktop `cargo fmt --all -- --check`, `cargo check -p
  screenpipe-core --locked --offline`, and `git diff --check`: passed. The
  desktop formatter initially exposed existing formatting drift in files
  touched by the removal; applying rustfmt made both checks pass.
- The locked/offline native binding export passed using Ninja Multi-Config,
  the transient `knf-rs-sys` CRT override, and OpenBLAS on runtime `PATH`.
  The required isolated `tauri_bindings_are_current` check passed after the
  final manifest/schema changes, as did the focused tray-shutdown test.
- The full desktop binary suite passed 144 tests and ignored 4 except for the
  documented concurrent exporter/freshness truncation race; its isolated
  freshness rerun passed. This failure is recorded rather than counted as a
  full-suite pass.
- Direct Node Vitest passed 33 files and 380 tests. Direct TypeScript
  `tsc --noEmit` passed after the final frontend changes. The installed Next
  production build passed after the final dependency/font removal with only
  the established `unpdf` `import.meta` warning.
- `cargo build --release --locked --offline` passed from the root workspace in
  5m 26s under the documented Developer PowerShell Ninja/OpenBLAS/ORT release
  environment, with only established warnings.
- A generic all-platform `cargo metadata --offline` lock-refresh attempt could
  not use an uncached Linux-only `alsa` package. The Windows-target desktop
  `cargo check --offline` then regenerated the lock and schemas successfully;
  every subsequent native command above used `--locked --offline`.

Audio model acquisition, build-script tool/runtime acquisition, and Pi package
bootstrap are explicitly not claimed complete here and remain the next audited
boundaries.

## Cloud archive and remote-device persistence boundary (2026-09-10)

Pre-change reachability: cloud archive persistence remained in `screenpipe-db`
through `cloud_blob_id`, archive-only orphan queries, and archive branches in
deletion cleanup. The engine also exposed `/data/device-storage` and
`/data/delete-device`, which only counted or deleted records by machine ID and
had no desktop callers. The write queue and database manager still exposed
cloud-sync insertion/marking operations, and fresh migrations created
`sync_id`/`synced_at` columns and indexes. These surfaces are not part of the
local recorder. `machine_id` is retained for local record provenance and
search/filtering, including memory device metadata. Baseline database
compatibility is not required.

Post-change evidence: cloud archive migration, cloud blob references, archive
DB helpers, remote-device routes, database-manager sync methods, write-queue
sync operations, and remote-frame response handling were removed. Fresh
migrations now create only the retained `machine_id` columns; obsolete sync
indexes/migrations and all `sync_id`/`synced_at` SQL plumbing are gone. Local
machine identity, memory device metadata, retention, and ordinary local
memories CRUD/search remain unchanged. No baseline database migration
compatibility is retained by owner decision. No retained version, source, or
checksum changed. Litepipe was not consulted.

Verification on Windows on 2026-09-10:

- Scoped DB/engine source residue searches found no cloud placeholder, sync
  column, sync queue operation, remote-device route, or sync-memory helper.
  `machine_id` query/filter and memory-device paths remain.
- `cargo check -p screenpipe-db --locked --offline` and
  `cargo check -p screenpipe-engine --locked --offline` passed.
- `cargo test -p screenpipe-db --test db_config_test --locked --offline`
  passed all 5 migration/configuration tests. The full locked/offline DB suite
  passed after replacing a test-only hard-coded `/tmp` path with a unique path
  below `std::env::temp_dir()`; the affected performance suite passed 11 tests,
  failed 0, and ignored 1.
- Whole-workspace root and desktop formatting plus `git diff --check` passed
  after integrating the concurrent subsystem edits.

### Audio model acquisition boundary — 2026-09-10

Runtime reachability finds four remaining audio acquisition edges: desktop
startup sets an optional Hugging Face mirror and launches a prefetch (including
an incorrect Whisper fallback for Parakeet/Qwen selections); missing Whisper
models call the Hugging Face client; missing CPU/MLX Parakeet and Qwen models
call `audiopipe` background download; and an otherwise-unused generic model
downloader plus ignored live-Hugging-Face tests retain network clients. Silero
VAD and diarization are already side-effect-free local loads with pinned
checksums from earlier commits.

This boundary will remove every recorder-initiated audio model download and
the mirror/prefetch UI/settings/bindings, without weakening local capture or
transcription. The supported Windows pack will be Parakeet int8 at a stable
ScreenWise local directory, verified against a pinned three-file SHA-256
manifest before load. Whisper and Qwen have no vetted immutable manifest in
this repository; their code will not be deleted in this safe pass, but missing
artifacts must fail locally instead of initiating acquisition. Whether to
retain those additional engines through new reviewed manifests or remove them
is a separate capability-retention decision. Baseline settings/database
compatibility is not required, so obsolete mirror state can be deleted.

Owner decision recorded on 2026-09-10: ScreenWise has no baseline database or
settings data that must be retained and will not support migration from the
baseline Screenpipe product. Future subsystem removals may therefore delete
dead schema, columns, settings fields, and compatibility shims when reachability
shows they are no longer used. Such cleanup remains scoped and reviewed; this
decision does not authorize touching the private smoke-test directories.

Audio model acquisition was closed in the same boundary. Desktop startup no
longer sets `HF_ENDPOINT` or prefetches any model, and the Chinese mirror setting
was removed from Rust, TypeScript, UI, tests, and generated bindings. Missing
Whisper, Qwen, and MLX weights now produce a local unavailable/disabled result
without spawning acquisition. The unused generic HTTP downloader, live
Hugging-Face test, and evaluation prefetch were deleted. Silero VAD and speaker
segmentation/embedding retain their pinned local SHA-256 checks; obsolete
download-named APIs and recovery code that deleted a provisioned file after an
ORT load error were replaced with read-only local verification.

The supported Windows transcription pack is CPU Parakeet at
`%LOCALAPPDATA%\screenpipe\audio-models\parakeet-tdt-0.6b-v3`. Its files are
operator-provisioned from the immutable model revision
`8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce` and verified before load:

- `encoder-model.int8.onnx` —
  `6139D2FA7E1B086097B277C7149725EDBAB89CC7C7AE64B23C741BE4055AFF09`
- `decoder_joint-model.int8.onnx` —
  `EEA7483EE3D1A30375DAEDC8ED83E3960C91B098812127A0D99D1C8977667A70`
- `vocab.txt` —
  `D58544679EA4BC6AC563D1F545EB7D474BD6CFA467F0A6E2C1DC1C7D37E3C35D`

The desktop exposes a read-only status command and manual instructions; on
Windows it refuses a Parakeet selection until all three files verify. Fresh
desktop and CLI settings now default transcription and meeting live notes to
disabled, unsupported identifiers fail closed to disabled, and hardware-tier
fallbacks no longer silently select an unverified Whisper model. This preserves
audio capture without requiring transcription weights. The Windows high-tier
safety check was corrected so verified CPU Parakeet is permitted while explicit
MLX remains non-Windows and Low/Mid tiers remain guarded.

No Screenpipe source outside the MIT boundary and no Litepipe source was used.
The model metadata is an artifact manifest only; no external source code was
adapted. Whisper/Qwen/MLX remain cache-only local implementations without a
reviewed ScreenWise checksum manifest and are not represented as the supported
Windows provisioning path. Their retain/remove/manifest boundary remains a
separate capability decision.

Verification on Windows on 2026-09-10:

- Root and desktop `cargo fmt --all -- --check` passed, and `git diff --check`
  passed.
- A locked/offline root check of `screenpipe-audio` (Parakeet and Qwen features)
  plus `screenpipe-audio-eval` passed in Visual Studio Developer PowerShell with
  Ninja Multi-Config and the documented OpenBLAS/ORT environment.
- `screenpipe-config` tests passed: 28 passed. Focused
  `screenpipe-engine` recording-config tests passed: 14 passed.
- Desktop store tests passed: 15 passed. The native binding export passed and
  `tauri_bindings_are_current` passed alone using the exact Ninja Multi-Config,
  transient `knf-rs-sys` CRT override, and runtime OpenBLAS `PATH` matrix.
- Direct TypeScript typechecking passed. Vitest passed 33 files / 380 tests.
  The production Next build passed with the existing `unpdf` `import.meta`
  warning only.
- `cargo build --release --locked --offline` passed from Visual Studio Developer
  PowerShell with normal single-config Ninja in 5m25s.
- The focused `screenpipe-audio` unit-test command did not start because the
  already-locked dev-only `infer 0.15.0` crate source is not cached and
  `--offline` correctly refused network access. The four new deterministic
  provisioning tests are therefore not claimed run. The production library,
  feature combinations, dependent engine, desktop, and release graph did
  compile successfully.
- Reviewed lockfile changes are limited to removing the direct
  `screenpipe-audio -> reqwest 0.13.3` and
  `screenpipe-audio-eval -> audiopipe` dependency edges. No retained package
  version, source, or checksum changed. Final SHA-256 values were root Cargo
  `CCD65FDCCDE94F9BE23068FF4C8D6CAC8A2D4D1960329747C94049E5B0766A7F`,
  desktop Cargo
  `996D3482DF84390D1CC0DD4919F357CFF1A79A67B66A139309D6483BA2E42B77`,
  and unchanged Bun
  `2BF22C910039145D023EAF256589C76CB5DF66E1A7912A7E51C7F40FF1568BDA`.

### Custom remote OCR boundary — 2026-09-10

Pre-change reachability: the screen capture crate exports `CustomOcrConfig`
and `OcrEngine::Custom`, converts the equivalent serializable DB variant, and
dispatches it from the common OCR path. The provider JPEG-encodes captured
images and sends them with language metadata and a bearer API key to an
arbitrary configured HTTP(S) URL. Current CLI parsing and desktop settings do
not expose this selection, but public Rust capture callers can still activate
it. This is the screen crate's sole `reqwest` caller; base64 remains required
for local realtime vision serialization. Two ignored integration tests,
coverage metadata, and a checked-in OpenAPI schema retain the obsolete API.

This boundary removes the remote provider/configuration/dispatch, generated
schema and dead tests, and the direct network dependency. Native Windows OCR,
local Tesseract and target-gated Apple OCR remain. The owner explicitly waived
baseline DB/settings compatibility, so no serialized Custom variant or
migration shim will be retained. No database content or smoke evidence is read
or deleted, and no post-MIT upstream or Litepipe source is consulted.

The `Unstructured` enum/parser value is also a dead compatibility alias from
the earlier hosted OCR removal: its only remaining behavior selected the
existing native OCR implementation. It has no distinct local capability and
is removed in this boundary together with the dead serialized variant.

Post-change evidence: `CustomOcrConfig`, the custom provider/module/export,
its HTTP client and OCR dispatch, both DB/screen enum variants, obsolete
ignored tests, and Custom OpenAPI/coverage entries are removed. Tracked-source
searches find no Custom OCR symbol or `reqwest` use in the screen crate;
`Unstructured` remains only in the regression test proving old remote-provider
configuration is rejected. Browser URL observation/normalization and ordinary
URL privacy filters remain local operations. Local OCR enum conversion and
serialization round trips are covered for all three retained engines.

Scoped verification on Windows in Visual Studio Developer PowerShell:

- `cargo fmt -p screenpipe-screen -p screenpipe-db -- --check` and scoped
  `git diff --check` passed. A root formatting check observed concurrent
  formatting work in `screenpipe-engine/src/retention.rs`; final whole-change
  formatting is left to the integrating agent.
- `cargo check -p screenpipe-screen --offline` passed in 38s while refreshing
  the intended lock removal. The desktop offline check passed in 2m11s with
  Ninja Multi-Config and the documented transient CRT/OpenBLAS/ORT matrix.
- `cargo test -p screenpipe-screen --lib --locked --offline utils::tests --
  --nocapture` passed 19 tests (including the two new OCR contract tests and
  existing browser-title utility tests).
- `cargo test -p screenpipe-screen --test windows_vision_test --locked
  --offline test_process_ocr_task_windows -- --nocapture` passed its native
  Windows OCR fixture test. No live desktop capture was started.
- The initial locked test correctly refused the stale lock before the
  intentional refresh. Complete lock inspection finds exactly one removed
  dependency line, `screenpipe-screen -> reqwest 0.13.3`, in each Cargo lock;
  no package version, source, or checksum changed. Root SHA-256 is
  `FA69784870CFC06AE3CF018E0DFA3E34B85CABDB31D413F1FCF8A508A989AD18`,
  desktop is
  `134BA7DAAE25A408C57D5BD27873F92958D16931F54F657AE1972EDDBB24BEAF`,
  and the unchanged Bun lock is
  `2BF22C910039145D023EAF256589C76CB5DF66E1A7912A7E51C7F40FF1568BDA`.
- Full scoped source/schema/lock diffs were inspected. No Tauri command or
  generated desktop binding changed in this boundary. Final release and
  integration validation is pending the integrating agent's combined commit.

### Windows build artifact acquisition boundary — 2026-09-10

Pre-change reachability finds that every build of `screenpipe-audio` executes
tool discovery unrelated to the audio crate and, when Bun is missing, invokes
`npm install -g bun` on Windows (or a shell-piped remote installer elsewhere).
The same Windows build script downloads and extracts ONNX Runtime 1.22.0 with
`curl`/`unzip` whenever its repository-local package is absent. These paths are
reachable during an ordinary Windows Cargo build, can modify the developer
machine globally, and contradict explicit provisioning even though offline
environment flags suppress the ORT download.

The local DLL-staging fix remains required: the locked `ort` crate dynamically
loads ONNX Runtime and a bare Cargo-built recorder must receive the validated
1.22.0 DLL beside its executable instead of selecting the incompatible system
1.17.1 DLL. This boundary will remove tool/runtime acquisition and archive
handling while preserving staging from an explicitly supplied
`ORT_LIB_LOCATION` or the documented repository-local package. It will not
change the locked ORT version, modify cross-platform `download-binaries`
features, or touch the separate macOS metallib/pre-build sidecar acquisition
paths; those have different target and packaging implications.

The Windows build boundary is complete. `screenpipe-audio` no longer probes for
Bun, invokes npm or a shell installer, launches curl/unzip, creates archives,
or modifies the repository-local runtime package. It accepts an explicitly
provisioned `ORT_LIB_LOCATION` (with the documented repository-local package as
a local fallback), verifies `lib\onnxruntime.dll`, and stages it beside the
active Cargo profile exactly as before. The audited x64 DLL hash is enforced as
`579B636403983254346A5C1D80BD28F1519CD1E284CD204F8D4FF41F8D711559`.
ARM64 remains buildable only when the operator supplies its separately audited
1.22.0 DLL hash through `SCREENPIPE_ORT_DLL_SHA256`; no unreviewed artifact is
accepted.

The obsolete `which 7.0.3` build dependency and its orphan `env_home 0.1.0`
package were removed from both Cargo locks. `sha2 0.10.9` was already a direct
audio dependency and is reused by the build script, so no version, source, or
checksum was introduced or changed. The Bun lockfile is unchanged. No
post-boundary Screenpipe or Litepipe source was consulted.

Verification on Windows on 2026-09-10:

- Root and desktop `cargo fmt --all -- --check` and `git diff --check` passed.
- The build script was compiled and exercised directly against the configured
  local runtime; staging passed. Its missing-runtime path failed locally with
  the expected explicit-provisioning message and performed no external action.
- Root `cargo check -p screenpipe-audio --features parakeet --offline` passed
  after the reviewed lock refresh. The desktop `screenpipe-app` check passed
  offline using Ninja Multi-Config, the transient `knf-rs-sys` CRT override,
  the configured ORT path, and OpenBLAS runtime `PATH`.
- `cargo build --release --locked --offline` passed in Visual Studio Developer
  PowerShell with normal Ninja in 5m23s, verifying and staging the provisioned
  x64 DLL. Only the established audio `unused_mut` and engine `CommandExt`
  warnings remained.
- Final lock SHA-256 values are root Cargo
  `4DDBDDABA9C8D74B29C4D8F20A71C547F6176DCD115F44027370746B27A17D65`,
  desktop Cargo
  `5F3263CA1412F9C592702B54AB2DAD49AE23366BA432FC536CE6ADD8295026B3`,
  and unchanged Bun
  `2BF22C910039145D023EAF256589C76CB5DF66E1A7912A7E51C7F40FF1568BDA`.

### External transcription settings residue boundary — 2026-09-10

Reachability audit confirmed that Deepgram and generic OpenAI-compatible
transcription clients are absent, but their retired credentials, endpoints,
model and header settings remained in `RecordingSettings`, desktop settings
tests, and frontend debug-log scrubbing. No retained local transcription
engine, local API bearer credential, or local Ollama setting reads those
fields. Because ScreenWise does not preserve baseline settings compatibility,
the dead serialized settings surface can be removed instead of retained as
deserialize-only compatibility fields.

The cleanup removes the fields, defaults, compatibility fixtures, named remote
engine tests, and desktop secret-key residue. Unsupported engine identifiers
still fail closed to disabled local transcription through the generic local
engine normalization; the selected-local-engine meeting provider normalization,
local model selections, localhost Ollama, and authenticated localhost
`/v1/audio/transcriptions` API remain unchanged.

Evidence: scoped searches after the removal find no Deepgram/OpenAI-compatible
transcription setting, generated binding, or frontend caller; remaining
OpenAI-compatible names describe the local inbound API or platform-gated Apple
Intelligence and are outside this boundary. Scoped rustfmt completed for the
modified Rust files and `git diff --check` passed. The requested locked offline
Cargo test/check could not start before lock refresh because concurrent reviewed
dependency-removal work had made `Cargo.lock` intentionally stale; Cargo
reported the `--locked` refusal and no lockfile was changed here.

### Pi runtime acquisition boundary — 2026-09-10

The retained Pi chat executor previously repaired, upgraded, and installed its
JavaScript package tree in the background with Bun and an npm fallback. Those
startup paths could resolve the Pi and Anthropic SDK packages from the Internet
and rewrote the local package-lock state. Pi itself can remain local: its RPC
process is configured only for loopback Ollama and can execute a fully
provisioned package directory.

ScreenWise now validates the pinned Pi runtime and direct dependency tree under
`<data-dir>/pi-agent` rather than acquiring or repairing it. Missing, corrupt,
or version-mismatched packages produce explicit provisioning instructions; they
do not block the recorder or remove local Pi/Ollama capability. This boundary
does not authorize arbitrary Pi shell/browser tool actions; those remain the
separate user-directed-agent policy decision.

### Desktop pipe/workflow residue boundary — 2026-09-10

Pre-change evidence: tracked E2E probes still called the removed `/pipes/list`
route; MCP notifications required `pipe_name` and advertised pipe execution;
desktop validation, onboarding, settings restart tracking, standalone-chat
localStorage/comments, skills, coverage, TESTING, and README still described
retired pipe/workflow or enterprise/shared-pipe behavior.

Post-change evidence: those probes, MCP pipe/context/open-in-chat fields,
dead persisted validation fields, `starting_pipes`, workflow-events UI residue,
pipe-execution localStorage, the orphaned scheduler example, stale test/coverage
entries, and obsolete
enterprise/shared-pipe marketing claims are removed. Local notifications,
links/deeplinks, local Pi/Ollama chat, owned-browser behavior, and media-pipe
terminology remain. Canonical `.claude` skills were regenerated into the
checked-in desktop skill module. No Pi or database Rust files were touched.

### Product/docs residue boundary — 2026-09-10

Pre-change evidence: README and migration docs still advertised encrypted
cloud sync/archive, paid subscriptions and checkout, cloud-model choices,
third-party OAuth/app integrations, Teams/enterprise/fleet deployment, and
shared-pipe workflows. TESTING retained cloud-sync, subscription, OAuth,
connection, remote-management, and integration checks.

Post-change evidence: stale README pricing/account/sync/team claims and the
corresponding TESTING sections were removed. Retired docs pages for cloud
archive, ChatGPT subscription, third-party connections, Teams, Intune, and
remote sync were removed from the migration docs set and navigation. The docs
validator now treats historical links to retired slugs as intentional and no
longer requires a deleted connection-reference registry. Local MCP/API,
owned-browser, native meeting/calendar, Apple Intelligence, and Ollama/Pi
documentation remain. No CONTRIBUTING content or captured evidence was read or
changed.

### Local-only AI provider boundary — 2026-09-10

Pre-change reachability: desktop AI presets retained arbitrary provider URLs
and provider credential fields, remote-provider migration branches, and UI,
diagnostic, title-test, usage-label, icon, and validation residue. Pi selected
loopback Ollama at execution time, but it still read and merged the user's
global Pi provider/auth configuration and could fall back to an unverified
global Pi executable. The settings schema also retained the obsolete
Screenpipe-cloud account user ID. These settings are independent of the local
recorder API bearer key.

Post-change evidence: presets have one provider (`native-ollama`) and contain
only local model, prompt, and token controls; no arbitrary endpoint or AI
provider credential is serialized, displayed, normalized, or tested. Pi
  requires an explicitly provisioned `<data-dir>/pi-agent` runtime whose pinned
  version and required dependency tree pass local validation,
writes an Ollama-only `models.json` below `<data-dir>/pi-agent-config`, sets
`PI_CODING_AGENT_DIR` for the child, and removes inherited external-provider
credential variables before launch. Its sole endpoint remains the fixed
`LOCAL_OLLAMA_URL` loopback address. The Pi config protocol's
`openai-completions` value is retained only because Pi uses that name for
Ollama's local wire format; it does not select a hosted provider. The local
recorder `apiKey`, `SCREENPIPE_LOCAL_API_KEY`, and authenticated loopback API
paths remain intact. Cloud account `userId` and compatibility migration code
are removed, as the owner waived legacy settings compatibility. Ordinary
user-directed browser URLs are unchanged.

Initial verification: direct TypeScript checking passed and the focused
chat-title Vitest suite passed 41/41. Scoped Rust formatting/checks and final
source/lock inspection are pending concurrent database cleanup completion.

### MCP token-discovery acquisition boundary — 2026-09-10

Pre-change reachability: MCP startup attempted to discover the local recorder
bearer token by invoking `bun x screenpipe@latest` from several guessed desktop
locations and then `npx screenpipe@latest` through both Node-adjacent and
`PATH` fallbacks. Those package-manager commands could contact the npm registry
and install code during ordinary MCP startup. They were not required for MCP's
authenticated loopback API client.

Post-change evidence: MCP token discovery now accepts the explicit
`SCREENPIPE_LOCAL_API_KEY` (plus the existing deprecated local alias) and may
read a plaintext token from the local SQLite secret store through an installed
local `sqlite3` executable. It no longer invokes Bun, npm, npx, or a package
specifier. Authentication remains required and missing credentials still fail
loudly rather than weakening the local API. Scoped searches and `git diff
--check` passed. Package-local typecheck/tests were unavailable because that
package's development dependencies are not installed; the consolidated
frontend validation below covers the checked-in desktop application instead.

### Clean ScreenWise persistence baseline boundary — 2026-09-10

Owner decision: historical Screenpipe databases and settings have no retained
value, and ScreenWise will not support migration from the baseline product.
The pre-change audit found cloud archive/sync columns and write helpers still
present solely for old data, plus desktop settings migrations for renamed
fields, restart-notification defaults, CoreAudio defaults, missing presets,
chat history, and old shortcut values.

Post-change evidence: fresh database migrations now create only the current
local schema; cloud blob/sync identifiers, timestamps, indexes, manager methods,
write-queue operations, route parameters, and compatibility migrations are
removed. `machine_id` remains because current local frames, UI events, inputs,
and memories use it. Desktop settings are deserialized as the current
ScreenWise schema without baseline alias or one-time migration rewrites; new
stores still receive complete defaults, and runtime platform detection remains
an active invariant. The unreachable OCR-to-frames `migration_worker` API was
also removed; current fresh-database migrations already define the retained
schema and no workspace caller used that worker. No private smoke data was
inspected or modified.

### Consolidated local-only product boundary — 2026-09-10

The final reachability audit found no application-initiated public Internet
request in the Windows recorder or retained Pi/Ollama path. Production HTTP
calls are limited to the authenticated ScreenWise loopback API, the desktop
loopback notification service, and Ollama on loopback. Owned-browser navigation
and ordinary help links remain explicit user actions. Owner decision on
2026-09-10: the no-Internet invariant applies to the running application, not
to building or packaging it. Build/CI acquisition may remain where required
and is audited separately from runtime egress.

Active docs and regression surfaces were aligned with that architecture:
hosted provider endpoints and credentials, one-click third-party connection
installers, pipe-store automation, cloud archive/sync, and product account
claims are gone. The docs now require explicit provisioning for local tools,
models, Pi, Ollama, and MCP. Native calendar setup opens the local calendar
dialog instead of navigating to the removed Connections settings page. Final
cleanup also removed the unreachable Claude/Codex memory-export renderer; it
had no network code or runtime caller, and existing external files are not
modified.

Consolidated verification on Windows on 2026-09-10:

- Root and desktop `cargo fmt --all -- --check`, direct TypeScript checking,
  docs validation, and `git diff --check` passed. The docs validator reports 33
  active pages, 50 API paths, and no connection registry entries.
- The full frontend Vitest suite passed: 32 files and 362 tests. The production
  Next build completed successfully. E2E, core-engine, and unified generated
  coverage reports all pass their freshness checks; the unified report was
  regenerated after the reviewed source-map removals.
- Root locked/offline core tests passed 128/128 plus the doc test, and focused
  engine search tests passed 7/7. Locked/offline DB/core/engine checks passed
  after removal of the obsolete migration worker. Earlier in this boundary the
  full database suite and focused Windows capture/OCR suites also passed.
- `tauri_bindings_are_current` passed in isolation using Ninja Multi-Config,
  the transient `knf-rs-sys` dev CRT override, and OpenBLAS on runtime `PATH`.
  The settings-store suite passed 10/10 under the same native matrix. The first
  binding exporter invocation omitted `UPDATE_TAURI_BINDINGS=1`, so it wrote
  only a temporary export and the following freshness assertion failed on the
  stale account bindings. Regeneration with the explicit flag then succeeded,
  followed by the passing isolated freshness run.
- `cargo build --release --locked --offline` passed in Visual Studio Developer
  PowerShell with normal Ninja in 6m05s. Only the established audio
  `unused_mut` and engine `CommandExt` warnings remained.
- Root and desktop Cargo lock diffs each remove only the direct
  `screenpipe-screen -> reqwest 0.13.3` edge; no package version, source, or
  checksum changed. Final lock SHA-256 values are root Cargo
  `FA69784870CFC06AE3CF018E0DFA3E34B85CABDB31D413F1FCF8A508A989AD18`,
  desktop Cargo
  `134BA7DAAE25A408C57D5BD27873F92958D16931F54F657AE1972EDDBB24BEAF`,
  and unchanged desktop Bun
  `2BF22C910039145D023EAF256589C76CB5DF66E1A7912A7E51C7F40FF1568BDA`.

### Final workspace dependency audit — 2026-09-10

Pre-change reachability found four inherited dependency declarations with no
workspace consumers: `tokenizers`, `cc`, `http-cache-reqwest`, and
`reqwest-middleware`. The retained PII crate declares its optional tokenizer
dependency directly, and the desktop build crate declares its own `cc` build
dependency. Removing these root declarations changes no runtime or feature
edge. The audio and meeting evaluation crates remain isolated developer
regression harnesses rather than production dependencies and are retained;
target-gated Apple and MLX crates are also retained because workspace
inheritance and supported non-Windows builds are not evidence of dead code.
`cargo check --workspace --exclude screenpipe-rfdetr-mlx --locked --offline`,
root formatting, and `git diff --check` passed. No Cargo or Bun lockfile changed;
the removed declarations had no package edge of their own.

### Current-schema UI-event timestamp boundary — 2026-09-10

Pre-change reachability found one remaining database compatibility shim:
`UiEventRow` decoded timestamps as arbitrary strings, accepted three historical
formats, and silently converted malformed values to the Unix epoch. Current
UI-event writes always serialize `DateTime<Utc>` with `to_rfc3339()`. Because
baseline databases are unsupported, reads can use SQLx's strict current-schema
`DateTime<Utc>` decoding directly. Nullable frame text-source and accessibility
visibility fields remain because current capture paths produce unknown values;
active OCR/video fallbacks likewise remain part of current local recording.
The batch UI-event integration test now reads inserted events through the typed
query path and verifies the exact UTC timestamp. It passed 3/3 under Visual
Studio Developer PowerShell with the documented native environment; locked
offline DB/engine checks, root formatting, and `git diff --check` also passed.
No lockfile changed. A final exact-HEAD
`cargo build --release --locked --offline` passed under normal Ninja in 5m35s
with only the established audio `unused_mut` and engine `CommandExt` warnings.

### Desktop WebView runtime-egress boundary — 2026-09-10

Pre-change reachability confirms that the Rust recorder and retained local
services do not initiate public Internet requests, but three desktop defense
gaps remain. The shared Markdown renderer emits ordinary `http(s)` image
sources as `<img>` elements, and the two notification renderers bypass the
shared image component entirely, so displaying untrusted Markdown can trigger
an automatic WebView request. The `localFetch` wrapper also accepts arbitrary
absolute HTTP URLs despite every retained caller targeting the configured
ScreenWise loopback API. Finally, the Tauri configuration has no CSP and grants
application capabilities to every HTTPS origin; those grants do not initiate a
request themselves but leave the runtime invariant unenforced at the WebView
boundary.

This increment will centralize all Markdown image rendering and replace public
network images with an explicit user-open control, reject non-loopback and
wrong-port absolute URLs in `localFetch`, add a loopback/local-resource CSP,
and narrow remote capability access to the exact local development origin. It
preserves packaged assets, explicitly scoped local files, data/blob media, the
authenticated ScreenWise API, the desktop notification service, local Ollama,
local WebSockets, and user-directed external links opened by the operating
system. It does not change build-time dependency or artifact acquisition.

Post-change evidence:

- Every React Markdown surface now uses the shared media-aware renderer. Public
  HTTP(S) images, including raw-HTML images, are rendered as an explicit
  user-open control without an image request; remote `srcset` values are
  discarded. Relative/scoped local files, `asset:`, Tauri asset hosts,
  `data:image`, `blob:`, and loopback images remain available.
- `localFetch` now parses absolute input and accepts only plain HTTP to
  `localhost`, `127.0.0.1`, or IPv6 loopback on the configured API port. Public
  hosts, deceptive hostnames, HTTPS, WebSocket schemes, and wrong ports fail
  before `fetch`. Relative API routes and bearer injection are unchanged.
- The base Tauri CSP limits connect, image, media, worker, frame, object, base,
  and form destinations. Tauri's default compile-time nonce/hash injection
  remains enabled for the statically exported Next scripts; no global
  `unsafe-inline` script permission was added. Beta and production overrides
  now inherit this policy instead of resetting it to `null`. Remote capability
  access is limited to the exact checked-in localhost development origins on
  ports 1420 and 3000; explicit OS-browser URL opening remains user-directed.
- The focused network-boundary suite passed 9/9 and the full frontend Vitest
  suite passed 371/371 across 35 files. Direct TypeScript checking and the Next
  production build passed; Next emitted only the established `unpdf`
  `import.meta` warning. The first policy-test run failed because Vitest's
  transformed `import.meta.url` was not a file URL, and a later raw-image test
  initially exposed the missing shared default URL transform; both test-harness
  findings were corrected before the passing full run.
- Locked/offline desktop `cargo check` passed under Visual Studio Developer
  PowerShell and normal Ninja. The isolated binding-freshness test passed under
  the documented Ninja Multi-Config, transient `knf-rs-sys` CRT override, and
  OpenBLAS runtime `PATH` after cleaning only the generator-stale
  `libsamplerate-sys` build artifacts. The normal root
  `cargo build --release --locked --offline` passed. A supplemental separate
  desktop release build under single-config Ninja was attempted but is not
  counted as passed: it reached the already documented
  `libsamplerate-sys` static-library layout failure, and no unvalidated release
  generator/profile substitution was used.
- Root and desktop Cargo manifests/locks, the frontend manifest/Bun lock, and
  dependency versions are unchanged. The generated Tauri capability snapshot
  contains only the reviewed localhost-origin reduction. No private smoke data
  was inspected or changed.

### Desktop least-privilege capability boundary — 2026-09-11

Pre-change reachability: the single desktop capability inherited broad Tauri
v1 migration grants that no retained caller needs. It permits arbitrary
frontend shell execution through `sh -c`, unrestricted arguments to `open`,
`cmd`, and the historical `screenpipe` sidecar, all shell spawn operations,
frontend process exit/restart, WebView creation/devtools, and filesystem access
across all of the user's home, AppData, application, resource, download, and
temporary directories. The only `sh -c` caller runs the optional macOS
`pmset` battery check for automatic Apple Intelligence summaries; it is not
part of Pi/Ollama. The checked-in Claude/Cursor/Codex MCP-config scanner has no
importer, so its private configuration-file scopes are unreachable residue.
The Rust-owned recorder processes, global shortcuts, windows, tray, and owned
browser do not require matching frontend IPC grants.

This increment will remove the unused shell, process, and CLI plugins; route
explicit external opens through the retained opener plugin; use the existing
native local-path opener for meeting artifacts; remove the orphaned integration
scanner; and replace migrated/default capability sets with caller-proven
commands and a single `$HOME/.screenpipe/**` static filesystem scope. Paths
selected or dropped by the user keep Tauri's per-selection runtime scopes.
Local Pi/Ollama, the authenticated recorder API, attachments and exports,
notifications, permission flows, native shortcuts, deep links, meeting notes,
and owned-browser navigation remain in scope.

Post-change evidence:

- The frontend shell, process, and unused CLI plugins are gone from their Rust
  and TypeScript manifests, native initialization, capability catalog, and
  generated schemas. The desktop Cargo lock removes only those three packages
  and their shell-only `os_pipe`, `shared_child`, `sigchld`, and `signal-hook`
  dependencies; the Bun lock removes only its two direct plugin entries and
  nested API aliases. The root manifest and lockfile are unchanged.
- All retained user-directed HTTP(S) opens now use the protocol-limited opener
  plugin, while meeting artifacts use the existing Rust-owned local-path
  command. The only custom opener scheme is the macOS System Settings link.
  No shell/process/CLI plugin reference, `exec-sh` caller, or importer of the
  deleted hardcoded integration scanner remains.
- The desktop capability now grants only caller-proven event, app-version,
  resource-close, menu, window, path, notification, store, filesystem, dialog,
  opener, OS, and permission-flow commands. Its sole static filesystem scope is
  `$HOME/.screenpipe/**`. The pinned installed Tauri dialog and drag/drop code
  was inspected locally and dynamically adds user-selected paths to the
  filesystem scope, preserving attachments and exports outside that directory.
- Direct TypeScript checking passed. The focused runtime-policy and Markdown
  suites passed 7/7; the full frontend Vitest suite passed 372/372 across 35
  files; and the Next production build passed with only the established
  `unpdf` `import.meta` warning. The separately invoked, normally excluded Bun
  text-overlay suite passed the changed opener test and 26/28 tests overall;
  its two failures are pre-existing unrelated DOM/class assertions and are not
  reported as passing.
- Root and desktop formatting checks passed. Locked/offline desktop `cargo
  check` passed under Visual Studio Developer PowerShell and normal Ninja. The
  binding-freshness test passed under the documented Ninja Multi-Config,
  transient `knf-rs-sys` CRT override, and OpenBLAS runtime `PATH`, after
  cleaning only `libsamplerate-sys`. The normal root `cargo build --release
  --locked --offline` passed with only the two established native warnings.
- The generated capability snapshot exactly matches the narrowed source
  capability. Full diff, root/desktop Cargo and Bun lockfile, residual-reference,
  and whitespace inspections found no unrelated dependency or private-smoke
  changes.

### Inbound listener and recorder-authentication boundary — 2026-09-11

Pre-change reachability: the main recorder HTTP/WebSocket API binds to
`127.0.0.1` and enables bearer authentication by default, but the retained
`listenOnLan` setting and `--listen-on-lan` CLI option deliberately change the
bind address to `0.0.0.0`. LAN mode forces authentication, while loopback mode
still permits authentication to be disabled. Even with authentication enabled,
`/ws/health`, `/audio/device/status`, every `/frames/*` route, and the stale
`/notify` exemption bypass the bearer check. The frame exemption includes
recorded image and text content and is not an acceptable public boundary.

The separate desktop focus/notification/helper server is hard-bound to
`127.0.0.1`, but has no authentication and permits every browser origin; its
removal/replacement cost is being audited separately and will not be obscured
inside this recorder-server change. The `screenpipe-connect` mDNS daemon is
runtime-reachable through an explicit CLI/environment opt-in and opens LAN
multicast sockets. The optional `screenpipe-apple-intelligence` `fm-server`
binary is not part of a default desktop build, but its `server` feature binds
an unauthenticated API to `0.0.0.0`.

This increment will remove LAN binding and mDNS discovery, remove the optional
standalone `fm-server`, make recorder API authentication non-disableable, and
reduce unauthenticated recorder routes to the minimal non-sensitive liveness
endpoint required during startup. It preserves authenticated HTTP/WebSocket
access, local Pi/Ollama, the embedded macOS Apple Intelligence routes inside the
authenticated recorder API, owned-browser control, and the separate loopback
desktop helper while its replacement boundary is documented.

Post-change evidence:

- The recorder and desktop now construct `SCServer` only with
  `127.0.0.1`. The `listenOnLan` persisted setting and CLI wiring are gone.
  Recorder authentication no longer has a boolean enable/disable state: startup
  must resolve a non-empty local API key or fail closed, and the desktop reports
  `auth_enabled: true` even during its key-seeding startup window.
- Only `/health` bypasses recorder authentication. `/ws/health`,
  `/audio/device/status`, `/notify`, and every `/frames/*` route now pass through
  the bearer/cookie/query-token check. The engine route test explicitly proves
  that unauthenticated `/search` and `/frames/1` return 403 while `/health`
  remains public.
- All retained frontend raw-frame image callers now append the local token;
  ordinary frame/context/text requests continue to use authenticated
  `localFetch`. Ignored live WebSocket diagnostics now require
  `SCREENPIPE_LOCAL_API_KEY`, and the API skill plus generated desktop copy no
  longer describe the removed exemptions.
- The mDNS implementation, runtime/CLI/environment activation, and `mdns-sd`
  dependency are removed. Both Cargo locks drop only `mdns-sd`, `if-addrs`, and
  the now-unneeded `log` edge on `mio`. The optional Apple `fm-server` binary,
  its `server` feature, and its server-only Axum/UUID/stream dependencies are
  removed; the embedded Apple Intelligence query route remains behind the main
  authenticated recorder listener.
- Locked/offline root checks for the changed engine/config/connect/Apple crates
  and the locked/offline desktop check passed. The focused engine endpoint test
  passed (1 run, 5 model-dependent tests ignored), including the new auth
  assertions. After staging the repository's checksum-verified ONNX Runtime
  1.22 DLL beside the debug test binary, all 5 tag endpoint tests passed. The
  transcription endpoint test compiled and ran with 3 passes, 2 failures, and
  1 ignored test; its two stale assertions concern removed external-engine and
  OpenAI-compatibility behavior, not authentication, and are recorded rather
  than hidden.
- Root and desktop formatting, the direct TypeScript typecheck, the focused API
  URL Vitest suite (3 tests), the generated-binding export, and the native
  `tauri_bindings_are_current` test passed. The production Next build also
  passed. The full frontend Vitest run had 34 passing suites and 370 passing
  tests; one suite failed during setup in the pre-existing validation test
  because its imported `z` value was undefined.
- The ignored live frame-stream, event-WebSocket, and first-frame diagnostics
  compile with their new explicit `SCREENPIPE_LOCAL_API_KEY` requirement; they
  were not executed because they require a separately running recorder.
- The normal locked/offline root release build passed under Developer
  PowerShell with Ninja and the documented OpenBLAS/ORT environment. Lockfile
  inspection found only the dependency removals described above; the root
  manifest and Bun lockfile are unchanged.
- Residual listener/configuration searches find no recorder mDNS, LAN-bind, or
  authentication-disable edge. They do still find the separately launched
  `packages/screenpipe-mcp` HTTP wrapper: it is not the recorder or desktop
  helper, defaults to loopback, and retains an explicit LAN option that refuses
  to start without its own API key. That optional package was outside this
  recorder-authentication change and remains a distinct future boundary.

#### Desktop helper listener audit

The remaining desktop helper is
`apps/screenpipe-app-tauri/src-tauri/src/server.rs`: an unauthenticated Axum
server fixed to loopback, normally port 11435, with wildcard CORS. It is not the
recorder/search API. Its reachable jobs are:

- `/focus` forwards second-instance arguments and deep links. Windows and macOS
  already use Tauri's single-instance plugin, while Linux currently relies on
  this early HTTP fallback.
- `/notify` and notification CRUD are used by Rust notification producers and
  the desktop notification bell.
- `/app-icon` and `/installed-apps` expose native application discovery to
  several settings/timeline surfaces.
- `/window-size` duplicates an existing Tauri command. `/inbox` and `/log` have
  no retained production caller found by the reachability scan and appear to be
  legacy local integration bridges.

Complete removal is a medium-sized IPC migration, not a safe one-line listener
deletion. It requires extracting the shared `bind_listener` helper used by the
recorder, replacing app-icon and installed-app discovery with typed Tauri
commands, moving notification CRUD and internal producers to direct Rust/Tauri
events, deleting the redundant/uncalled routes, removing helper startup,
shutdown, port configuration and generated binding, and rewriting its E2E
tests. A separate platform decision is required for Linux second-instance/deep-
link forwarding; on the Windows-first product, the existing native plugin
already covers the primary target.

Hardening while it remains is smaller but still substantive: issue an
unguessable per-process token through Tauri IPC and require it on every route;
replace wildcard CORS with exact packaged/development origins; cap and validate
payloads, dimensions, paths, and window names; and remove the current behavior
that can forcibly terminate an unrelated process occupying the fixed port.
Using an OS-assigned port or same-user named-pipe/local-socket transport would
further reduce browser-origin and fixed-port exposure.

Recommended boundary: remove the simple HTTP jobs first (icons, installed apps,
window sizing, notification CRUD/producers, inbox, and log) through typed native
IPC. Then either remove `/focus` with the helper on the Windows-first target or
retain only a narrowly authenticated focus bridge until Linux receives a native
single-instance transport. Hardening the entire current multipurpose server is
less attractive than this staged removal because most routes already have a
natural in-process replacement.

### Windows-only desktop helper removal — 2026-09-11

Pre-change boundary and reachability: the product owner has selected a
Windows-only runtime boundary, so the helper's Linux second-instance fallback
no longer needs a replacement. Windows already uses Tauri's native
single-instance plugin. The port 11435 listener remains reachable at desktop
startup and still carries notification display/history, app-icon lookup,
installed-app discovery, redundant window resizing, focus forwarding, and the
uncalled inbox/log bridges. Its notification producers are all inside the
desktop process; its retained frontend consumers already have Tauri IPC.

This increment will remove the listener rather than harden it. App icons,
installed-app discovery, and notification history/display will move to typed
Tauri commands or direct in-process calls. The native Windows single-instance
plugin remains responsible for focus/deep-link forwarding. The redundant and
uncalled routes, helper lifecycle/configuration, HTTP clients, generated
bindings, documentation, and listener-specific E2E tests will be removed.

Post-change evidence:

- Deleted the port 11435 Axum server, its startup/shutdown state, early HTTP
  single-instance probe, `SCREENPIPE_FOCUS_PORT`, wildcard CORS, and the focus,
  inbox, log, window-size, notification, app-icon, and installed-app routes.
  Windows focus and deep-link forwarding remain on
  `tauri-plugin-single-instance`; no replacement listener was introduced.
- Added typed Tauri commands for notification display/history, native app-icon
  data URLs, and cached installed-app discovery. Rust notification producers
  now call the same in-process service through an initialized `AppHandle`.
  Frontend icon consumers share a promise cache and never fetch a loopback URL.
- Removed the helper-specific focus-server E2E test and port launcher setup;
  notification and privacy flows now exercise native IPC. Removed the public
  helper notification instructions from both API-skill sources and regenerated
  the checked-in skill content and TypeScript command bindings.
- Removed the desktop's direct `axum 0.6`, `http 0.2`, and `tower-http 0.4`
  dependency edges. The desktop lockfile pruned only that obsolete duplicate
  HTTP stack and normalized dependency names that no longer require version
  disambiguation. Root `Cargo.lock` and Bun lockfiles are unchanged.
- Residual searches find no active `11435`, `SCREENPIPE_FOCUS_PORT`,
  `get_app_server_config`, `/installed-apps`, or `/app-icon` runtime edge. The
  two remaining 11435 strings are intentionally malformed historical OCR
  samples in URL-detection benchmark data, not executable configuration.
- Validation passed: `cargo fmt --all -- --check`; desktop locked/offline
  `cargo check` under the documented native-test environment (after one
  unlocked/offline incremental lockfile regeneration); generated binding test
  `tauri_bindings_are_current`; notification rewrite tests (15 tests);
  frontend and E2E TypeScript checks; focused markdown network-image Vitest
  (4 tests); Next production build; `git diff --check`;
  and root `cargo build --release --locked --offline` in Developer PowerShell
  with the normal Ninja/OpenBLAS/ORT environment (9m21s).
- A standalone `cargo generate-lockfile --offline` attempt could not resolve
  the cached `cidre` git repository's missing remote-HEAD reference. The
  existing lockfile was instead updated incrementally by `cargo check
  --offline`, then every subsequent Cargo command used `--locked --offline`.
  The release build repeated only the documented audio `unused_mut` and engine
  Windows `CommandExt` warnings. The Next build repeated the existing `unpdf`
  direct-`import.meta` warning. An initial focused Vitest invocation from the
  repository root used the wrong relative entrypoint and failed before loading
  tests; rerunning from the desktop directory passed.

### Residual helper and MCP listener boundary — 2026-09-11

Pre-change reachability found that removing the desktop's port 11435 listener
left three active callers behind: the browser-extension popup and options page
still POST to `/focus`, and the standalone `screenpipe-mcp` package still POSTs
notifications to `/notify`. Those requests can no longer succeed. The same MCP
package also retains an independently launched Streamable HTTP server which can
bind to `0.0.0.0`; its loopback requests bypass authentication, and its upstream
recorder requests do not attach the now-mandatory local API bearer token.

This increment will remove the dead focus and notification callers while
preserving browser pairing and stdio MCP. The optional MCP HTTP transport will
be fixed to `127.0.0.1`, require an explicit bearer token for every endpoint,
and forward that same token to the authenticated recorder. Its LAN option,
loopback authentication bypass, stale documentation, tests, generated browser
extension output, and obsolete listener assumptions in CI will be removed.

Post-change evidence:

- Removed the browser extension's dead `/focus` calls and changed its server
  action to an explicit retry; pairing still starts through the authenticated
  recorder API and tells the user to approve it in ScreenWise. Rebuilt the
  checked-in extension JavaScript.
- Removed the MCP `send-notification` tool and its guaranteed-failing port
  11435 request. Stdio MCP and all unrelated tools remain intact.
- Fixed the optional MCP HTTP wrapper to `127.0.0.1`, rejected the former LAN
  option, required an explicit API key at startup, authenticated `/health` and
  `/mcp`, removed wildcard browser CORS, and forwarded the same bearer token to
  the recorder. A regression test observes the forwarded header.
- Updated the package documentation, native notification comments, desktop
  deep-link comment, and legacy CI log assertion. Residual executable searches
  find no port 11435 edge; the former LAN flag remains only in fail-closed
  parser tests and rejection code.
- The MCP package's exact locked dependencies and browser extension Chrome
  types were installed from Bun's offline cache after the initial sandboxed
  install could not use Bun's temporary directory. No dependency or lockfile
  changed. MCP TypeScript build and 20 tests passed; browser extension build
  and standalone TypeScript check passed; the full desktop TypeScript check,
  `cargo fmt --all -- --check`, and `git diff --check` passed.
