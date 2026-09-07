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
