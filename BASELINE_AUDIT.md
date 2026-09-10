# Windows functional and network baseline

Provenance baseline: upstream Screenpipe `892199f742e46d0c5d9e8c06687b35ca7c2b6547` (MIT). No Litepipe code was used for the fixes or audit recorded here. Litepipe remains fixed at reference commit `8969c10723634640ad2e757b8281dca8b0272c2f`.

## Functional audit (2026-09-01)

The release binary was built with `cargo build --release --locked` and exercised on Windows 11 with a fresh, repository-local data directory and API auth enabled.

| Subsystem | Result | Evidence |
| --- | --- | --- |
| Screen capture | Pass | WGC started and JPEG snapshots plus frame rows were produced. |
| Multi-monitor | Pass | Three displays (2560x1600, 1920x1080, 1920x1080) were discovered, captured, and reported healthy. |
| Windows accessibility/UIA | Pass | Native hooks and UIA worker started; populated trees were captured from Teams and ChatGPT. |
| OCR | Pass | Windows OCR ran with `en-GB`; search returned captured frame text. |
| Database creation/writes | Pass | Fresh `db.sqlite`, WAL and SHM files were created; frame and UI-event writes were observed. |
| Local HTTP API/search | Pass | `/health` returned 200. Authenticated `/search` returned captured content. |
| Local API authentication | Pass | `auth token --data-dir <recorder-dir>` retrieved the persisted `sp-...` key; bearer search returned 200 and an unauthenticated identical request returned 403. The key is local API protection, not an LLM provider credential. |
| Audio capture | Pass | Default microphone and loopback output were active and AAC/MP4 chunks were written through FFmpeg. |
| Transcription | Pass | Parakeet CPU model loaded successfully; authenticated audio search returned local live transcript rows. No ONNX compatibility panic occurred. |
| Diarization/meeting audio | Pass (smoke) | Segmentation and WeSpeaker sessions initialized, a Teams meeting was detected, and meeting transcript rows were emitted. Speaker-label quality was not benchmarked. |
| Input/clipboard | Pass (presence) | Native keyboard/mouse hooks ran; health reported UI recorder active, clipboard capture enabled, and UI-event rows inserted. Clipboard contents were not deliberately generated for this privacy-sensitive smoke test. |
| Pause/resume | Partially audited | Schedule and DRM pause state were reported through health, but an interactive pause/resume cycle was not exercised. |
| Shutdown | Pass | Ctrl+C stopped audio, UIA, vision, meeting detection, and completed shutdown cleanly. |
| Restart | Pass | The same smoke data directory had already survived two failed prerequisite launches, then initialized normally; the persisted API key remained retrievable. A second full post-capture restart is still worth a longer soak test. |

Default locations are `%LOCALAPPDATA%\screenpipe` for shared audio models/VAD cache and `%USERPROFILE%\.screenpipe` for the default recorder data directory. `--data-dir` moves the recorder database, media, logs, pipes, and local secret store to the selected directory. The smoke evidence directories are intentionally untracked because they contain captured private data and generated binaries/media.

## ONNX Runtime finding

The locked Rust graph uses `ort`/`ort-sys` `2.0.0-rc.10`, which validates ONNX Runtime `1.22.x`. The audio build script correctly acquired Microsoft's CPU-only `onnxruntime-win-x64-1.22.0`, but did not copy its DLL beside a bare Cargo-built CLI executable. Windows then found `C:\Windows\System32\onnxruntime.dll` version 1.17.1. The build now stages the acquired 1.22 DLL into the Cargo profile directory, where application-directory DLL search precedence makes runtime selection deterministic. No dependency or lockfile change was required.

## Outbound network inventory

This inventory distinguishes observed baseline startup behavior from optional code paths. Telemetry was disabled for the smoke run.

| Action / destination | Code path and trigger | Runtime need | Control / setup assessment |
| --- | --- | --- | --- |
| NPM version query: `https://registry.npmjs.org/screenpipe/latest` | The baseline automatic CLI startup check was removed after this audit. | Not required. | No version request is made during CLI startup or reminder rotation. Updates are an explicit user/package-management action. |
| FFmpeg version/download: ffmpeg-sidecar latest check, then `https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip` | The baseline fallback was removed after this audit. Discovery accepts an existing matching FFmpeg/FFprobe pair only. | One-time tool acquisition only. | Preinstall and expose the pair on `PATH`, or bundle it beside the executable. Missing tools now produce setup guidance without network or filesystem mutation. |
| ONNX Runtime: `https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-win-*-1.22.0.zip` | `screenpipe-audio/build.rs`; build-time when the pinned runtime package is absent. | Build/setup only; not needed after packaging. | Cache/package the pinned DLL; `CARGO_NET_OFFLINE`, `SCREENPIPE_SKIP_ONNX_DOWNLOAD`, or `ORT_SKIP_DOWNLOAD` suppresses download. |
| Speaker models: baseline used moving raw GitHub URLs for `segmentation-3.0.onnx` and `wespeaker_en_voxceleb_CAM++.onnx` | Runtime download was removed. Both MIT-baseline artifacts are verified by SHA-256 before ONNX Runtime loads them. | One-time explicit setup only. | Pre-stage under `%LOCALAPPDATA%\screenpipe\models`; a missing or mismatched file fails locally with no network fallback. |
| Silero VAD v5: baseline used a moving `snakers4/silero-vad` raw `master` URL | Runtime download was removed and the accepted artifact hash is verified before loading. | One-time explicit setup only. | Pre-stage `silero_vad_v5.onnx` under `%LOCALAPPDATA%\screenpipe\vad`; record immutable source/tag/licence provenance when acquiring it, because the baseline URL was mutable. |
| Parakeet: Hugging Face repo `istupakov/parakeet-tdt-0.6b-v3-onnx` through `audiopipe`/`hf-hub` | transcription model initialization/cache refresh. Successful run loaded the cached model. | One-time model acquisition. | Pre-stage/cache during setup; runtime code first attempts cache-only and background acquisition when unavailable. |
| PostHog `https://us.i.posthog.com` and Sentry ingest | Product telemetry and crash-reporting transports, lifecycle/event call sites, settings, shims, permissions, and frontend dependencies were removed from the engine and desktop app. | Not required. | Local logs, local process CPU/memory diagnostics and their optional rotating JSONL file, and `last-panic.log` remain available. User-configured PostHog/Sentry connectors and secret-redaction rules are unrelated and remain. |
| Screenpipe cloud sync, cloud archive, and SFTP remote sync | CLI sync commands, service startup, cloud-search metadata, sync/archive routes, and client transports were removed. | Not required for the local recorder. | Local retention remains available without any upload prerequisite. Cloud proxy, providers, and integrations remain separate follow-up removals. |
| Screenpipe product account and cloud-completions proxy | Engine account CLI commands, cloud JWT state, and the `/v1/chat/completions` proxy were removed. | Not required for local capture/search. | The localhost API's distinct bearer authentication remains required and unchanged. Desktop account UI is a separate desktop build increment. |
| External audio transcription and live meeting streaming | Deepgram/OpenAI-compatible batch clients plus Screenpipe Cloud/Deepgram meeting WebSocket clients were removed. | Not required. | Local Whisper, Qwen, and Parakeet transcription remain; meeting overlays use the selected local engine only. |
| Cloud workflow classifier | The optional classifier that uploaded recent activity to the Screenpipe gateway was removed. | Not required. | No recorder startup task sends activity off-device to derive workflow events. |
| Pipes and pipe registry | The pipe agent, scheduler, registry/store client, API, CLI, persistence, and permissions middleware were removed. | Not required. | Local capture/search routes do not install, run, or schedule automation. |
| Enterprise/team CLI | Direct Screenpipe enterprise device/search/record queries were removed. | Not required. | The local recorder has no team-cloud command path. |
| mDNS multicast | Server discovery path; only when `--enable-mdns`/environment opt-in is set. | Not required. | Off by default; observed skipped for loopback-only server. |

With FFmpeg and all pinned models pre-staged, product telemetry removed, sync
off, and local transcription selected, the recorder's capture/search path can
operate locally. The automatic CLI NPM update check has been removed. A later
network-deny soak test should confirm this at the OS firewall layer after the
remaining surprise automatic paths are removed.

## Recommended first removal/hardening tranche

The first tranche is complete: automatic CLI update checks and FFmpeg downloads
were removed, model artifacts became explicit/checksummed setup inputs, and
product telemetry/crash reporting was removed while local diagnostics stayed
intact. Continue to isolate remaining cloud, account, provider, automation, and
integration surfaces in separate reviewable commits because their reach is
broader.

The final telemetry-removal verification on 2026-09-04, with frontend checks
repeated on 2026-09-07, passed direct TypeScript checking, 10 focused frontend
tests, the production Next export, locked offline
engine and desktop checks, 26 configuration tests, 529 engine library tests
(2 ignored), and a full locked offline release build in Visual Studio 2026
Developer PowerShell. The production frontend scan found no product telemetry
SDK or ingest-endpoint marker. Both Rust lockfiles remained unchanged; the Bun
lockfile pruned only the removed telemetry dependency graph, with no new
package version. An optional Tauri binding-freshness test could not start
offline because locked dev dependency `assert-json-diff 2.0.2` was not cached
at the time of that commit. On 2026-09-07 that exact version and its remaining
already-locked test graph were fetched without a lockfile change. The test then
built, linked, and ran under `--locked --offline`; it failed the freshness
assertion because regeneration would remove three unrelated cloud/account
bindings (`getCloudToken`, `openLoginWindow`, and `setCloudToken`). That
pre-existing drift was not accepted into the telemetry cleanup.

The later product-account cleanup removed those obsolete generated bindings.
On 2026-09-10 the checked-in bindings were regenerated from the current native
command registry and the isolated `tauri_bindings_are_current` test passed with
the documented locked/offline native test matrix.

The reproducible Windows test setup differs from the normal release build:
native-linking test targets use Visual Studio Developer PowerShell plus
`CMAKE_GENERATOR=Ninja Multi-Config` so `libsamplerate-sys` produces the
expected `out\build\Release\samplerate.lib`. The separate desktop Cargo
workspace also needs the transient setting
`--config 'profile.dev.package."knf-rs-sys".debug-assertions=false'` to align
its C++ runtime with `whisper-rs-sys`, and the test process needs
`C:\Utils\OpenBLAS\win64\bin` on `PATH`. Single-config Ninja failed at the
samplerate static library; debug desktop linking without the override failed
with MSVC CRT mismatches; omitting the runtime `PATH` failed with
`STATUS_DLL_NOT_FOUND`. Exact commands are recorded in `BUILD_NOTES.md` and the
workspace `AGENTS.md`.
