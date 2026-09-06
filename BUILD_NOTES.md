# Windows build and runtime baseline

Upstream provenance baseline: commit `892199f742e46d0c5d9e8c06687b35ca7c2b6547` (MIT-licensed upstream). The Windows compatibility change described below is commit `a08843c80` on top of that baseline. Do not update dependencies or `Cargo.lock` while reproducing this baseline.

## Historical upstream guidance

- Historical Screenpipe Windows documentation specified Visual Studio 2022. That likely describes a genuinely compatible historical toolchain, not just stale documentation: the native dependencies and their build scripts date from that toolchain generation.
- The old documentation added a GnuWin32 directory to `PATH` unconditionally. Future documentation should make that conditional on the directory actually existing. The real requirement is simply to verify that `unzip` is available on `PATH`.

## Environment-specific findings on this machine

These findings were verified on 2026-09-01 and are not source requirements for every Windows machine:

- Builds must run from Visual Studio Developer PowerShell, not ordinary PowerShell. The developer shell supplies the MSVC compiler, Windows SDK headers, and variables such as `VCINSTALLDIR`, `VCToolsInstallDir`, `WindowsSdkDir`, `INCLUDE`, `LIB`, and `LIBPATH`.
- The installed shell is Visual Studio 2026 Developer PowerShell 18.9.2, using MSVC tools 14.51.36231 and Windows SDK 10.0.26100.0. Historical upstream guidance remains Visual Studio 2022 as noted above.
- CMake 4.4 was incompatible with the older OpenBLAS/MLX native build scripts. CMake 3.31.x works; the verified version is 3.31.8.
- CMake 3.31 does not recognise the Visual Studio 18 / 2026 generator. In a Visual Studio 2026 Developer PowerShell session, use Ninja explicitly:

  ```powershell
  $env:CMAKE_GENERATOR = "Ninja"
  ```

  The verified Ninja version is 1.13.2.
- The verified Rust toolchain is `rustc 1.93.1` / `cargo 1.93.1`. On this machine, launching Developer PowerShell from another shell can omit `C:\Users\clanc\.cargo\bin` from `PATH`; add the existing Rust toolchain path to that session if `cargo` is not found.
- OpenBLAS is required by the default Qwen/ASR native build path. On this machine its distribution root is:

  ```powershell
  $env:OPENBLAS_PATH = "C:\Utils\OpenBLAS\win64"
  ```

  That exact directory directly contains `include\cblas.h` and `lib\libopenblas.lib`. Setting `OPENBLAS_PATH` to `C:\Utils\OpenBLAS` is one level too high for this installation.

## Source and build-system change in this repository

- The bare workspace build originally included `crates/screenpipe-rfdetr-mlx` because the root member glob is `crates/*`. That explicitly Apple-Silicon-only crate then failed under MSVC because vendored MLX uses the GNU/Clang `typeof` extension.
- Commit `a08843c80` (`fix Windows workspace build`) adds explicit `workspace.default-members` containing every current workspace crate except `crates/screenpipe-rfdetr-mlx`. The crate remains a workspace member so its inherited manifest fields and target-gated macOS path dependency continue to work. This is the only source/build-system compatibility change relative to upstream commit `892199f`.
- An earlier attempt to put the crate in `workspace.exclude` did not work: Cargo could no longer parse its inherited package fields as a path dependency. That attempt is not present in the repository.
- The compatibility commit does not alter product functionality, dependency versions, or `Cargo.lock`.

## Reproducible release build

From Visual Studio Developer PowerShell, after setting `OPENBLAS_PATH` and (for Visual Studio 2026) `CMAKE_GENERATOR`, use the locked dependency graph:

```powershell
cargo build --release --locked
```

Do not substitute an unlocked build when validating this baseline.

## OpenBLAS runtime DLL requirement

The build-time import library is not sufficient at runtime. The Qwen/ASR native path links dynamically to the exact DLL filename `libopenblas.dll`. Before the runtime fix, `screenpipe.exe --help` and `--version` exited before Rust `main` with process exit code `-1073741515` (`0xC0000135`, `STATUS_DLL_NOT_FOUND`). Because the Windows loader fails before Clap or logging initialises, the console can appear to exit silently.

For this installation, copy:

- Source: `C:\Utils\OpenBLAS\win64\bin\libopenblas.dll`
- Destination: `D:\Data\NoSync\Repos\ScreenWise\screenpipe\target\release\libopenblas.dll` (beside `target\release\screenpipe.exe`)

Equivalent command from the repository root:

```powershell
Copy-Item C:\Utils\OpenBLAS\win64\bin\libopenblas.dll `
  target\release\libopenblas.dll
```

Putting `C:\Utils\OpenBLAS\win64\bin` on the launching process's `PATH` also allows the Windows loader to find the DLL, but application-local deployment is more deterministic. Future Windows packaging/distribution must bundle `libopenblas.dll` beside every distributed `screenpipe.exe` (or deliberately install and expose an ABI-compatible DLL directory on `PATH`). Merely setting `OPENBLAS_PATH` does not solve runtime loading because its `bin` child is not searched automatically.

The source and deployed DLLs verified on this machine are both 51,117,073 bytes and have SHA-256 `B554C45AF7B39154C561FB4879FD784D4928462E9A70335AADD9B1DE3C75E9E2`.

## Verification on 2026-09-01

- Visual Studio Developer PowerShell reported `VSCMD_VER=18.9.2`, `VisualStudioVersion=18.0`, MSVC tools 14.51.36231, and Windows SDK 10.0.26100.0. `INCLUDE`, `LIB`, and `LIBPATH` contained the corresponding MSVC and SDK directories.
- `cmake --version`: 3.31.8.
- `ninja --version`: 1.13.2.
- `OPENBLAS_PATH=C:\Utils\OpenBLAS\win64`; both `include\cblas.h` and `lib\libopenblas.lib` exist.
- The required `target\release\libopenblas.dll` exists beside `screenpipe.exe`, and its SHA-256 matches the installed source DLL.
- `cargo build --release --locked` completed successfully (release profile, 1m 16s for this verification run). The only Rust warnings were the existing `unused_mut` in `screenpipe-audio` and unused Windows `CommandExt` import in `screenpipe-engine`.
- `target\release\screenpipe.exe --help` printed the CLI usage and exited 0.
- `target\release\screenpipe.exe --version` printed `screenpipe 0.4.15` and exited 0.
- A bare `target\release\screenpipe.exe` printed usage and exited 2 because a subcommand is required. Normal recording is started with `record`.
- A bounded normal-launch smoke test ran `record` with audio, vision, and telemetry disabled on port 31337. The process remained running after 12 seconds and `http://127.0.0.1:31337/health` returned HTTP 200, version 0.4.15, and status `healthy`; the verification process was then stopped.
- `Cargo.lock` has no diff relative to upstream commit `892199f` or the current `HEAD`.

No product functionality, dependencies, or `CONTRIBUTING.md` were modified as part of this documentation pass.

## Local-first hardening

- The standalone CLI no longer queries the NPM registry at startup or during its periodic terminal reminder rotation. Updating is now an explicit user/package-management action; `SCREENPIPE_NO_UPDATE_CHECK` is obsolete.
- Missing FFmpeg no longer triggers a version query, archive download, extraction, or shell-profile edit. Runtime discovery accepts a preinstalled matching FFmpeg/FFprobe pair on `PATH`, an application-bundled pair, or an existing sidecar installation; otherwise it logs explicit setup guidance and disables dependent functionality.
- Product telemetry and crash reporting have been removed from the engine and
  desktop application rather than merely forced off. The legacy CLI switch,
  persisted analytics settings, PostHog UI events/identity code, Sentry browser
  and Tauri plumbing, and their frontend packages are gone. Existing settings
  files may still contain the retired keys; serde ignores them for backward
  compatibility.
- Diarization and Silero VAD models must now be explicitly pre-staged; missing files produce their exact cache path and expected SHA-256 instead of starting background downloads. Verified artifacts: `segmentation-3.0.onnx` = `B78FC48113BB46FD247AE6A9AEA737079550C647638DB961DF7E0E1E9F4BA62E`, `wespeaker_en_voxceleb_CAM++.onnx` = `C46FAD10B5F81E1AA4A60C162714208577093655076C5450F8C469E522EC54EF`, and `silero_vad_v5.onnx` = `1A153A22F4509E292A94E67D6F9B85E8DEB25B4988682B7E174C65279D8788E3`.
- Audio startup now streams each explicitly provisioned diarization or Silero
  artifact through SHA-256 before caching or loading it. A mismatch is a
  startup error, not a fallback download. This uses the existing locked
  `sha2 0.10.9` package; the sole root-lock delta is the direct audio-crate
  dependency entry. The Silero checksum establishes the accepted artifact
  bytes, but its original moving `master` URL did not supply sufficient
  immutable source/version provenance; operators must record the source,
  release/tag, and licence for their separately acquired binary.
- Engine telemetry hardening now removes the compiled PostHog transport and
  direct Sentry dependency. The local resource monitor retains optional local
  diagnostics only, and panic diagnostics remain in `last-panic.log` without
  remote reporting. The intentional dependency removal prunes the related
  Sentry packages from `Cargo.lock`; no package versions were upgraded.
  The earlier native `libsamplerate-sys` test prerequisite was subsequently
  satisfied by the validated Developer PowerShell environment; current engine
  test results are recorded below.
- Final desktop telemetry cleanup removed every product PostHog/Sentry import,
  call site, compatibility shim, package, native lifecycle adapter, settings
  field, and generated permission entry. Local browser/engine logs, local
  process CPU/memory diagnostics (including the opt-in rotating resource JSONL
  file), and `last-panic.log` remain active. User-configured PostHog/Sentry
  service connectors and secret-redaction patterns are separate features and
  intentionally remain.
- Verification on 2026-09-04, with the frontend typecheck/build repeated on
  2026-09-07: direct Bun TypeScript checking passed; two focused
  Vitest files passed 10/10 tests; the production Next export passed; locked
  offline engine and desktop checks passed; `screenpipe-config` passed 26/26;
  `screenpipe-engine --lib` passed 529 with 2 ignored; and the full Visual
  Studio 2026 Developer PowerShell `cargo build --release --locked --offline`
  passed in 7m 58s. The release CLI no longer advertises
  `--disable-telemetry`, and the production frontend artifacts contain no
  product telemetry SDK or ingest-endpoint markers. `bun run` currently fails
  before its target starts because this checkout's Bun executable reports a
  corrupted `node_modules/.bin` remapping; invoking the installed TypeScript,
  Vitest, and Next entrypoints directly succeeded.
- On 2026-09-07, `assert-json-diff 2.0.2` and the other already-locked desktop
  test dependencies were fetched without changing either Rust lockfile. The
  binding-freshness test then built, linked, and ran offline. It failed its
  assertion because the checked-in bindings retain three unrelated
  cloud/account commands (`getCloudToken`, `openLoginWindow`, and
  `setCloudToken`) that the current Rust registry does not export. Regeneration
  was inspected and reverted because accepting those removals would cross this
  telemetry-only change's scope.
- `Cargo.lock` and the desktop Rust lockfile are unchanged. The Bun lock delta
  removes only the three direct telemetry packages and their unreachable graph;
  Bun relocates already-present `react-is` 16.13.1/17.0.2 resolutions without
  introducing a new version or incidental upgrade.
- Cloud synchronization, cloud-search metadata, cloud archive upload, and
  SFTP remote-sync CLI paths have been removed from the engine. The
  authenticated localhost API retains its independent local retention routes;
  nothing now uploads captured data before retention cleanup.
- The engine no longer offers Screenpipe-cloud `login`, `logout`, or `whoami`
  commands and no longer proxies local `/v1/chat/completions` requests to the
  Screenpipe service. This does not alter the separate local bearer token used
  to protect the loopback API.
- Deepgram and OpenAI-compatible audio transcription clients, their CLI key
  input, and the Screenpipe Cloud/Deepgram live-meeting WebSocket transports
  have been removed. The retained audio paths are local models only. The
  root lockfile reduction removes the audio crate's `tokio-tungstenite` and
  native-TLS transport edges without upgrading any dependency versions.
- The cloud workflow-classifier polling task and Screenpipe gateway client have
  been removed. Local capture does not send recent activity to derive workflow
  events.
- The pipe automation subsystem has been removed: no recorder startup task,
  CLI command, local API route, registry client, or scheduled agent execution
  remains. This does not alter the authenticated local capture and search API.
- The enterprise/team cloud CLI has been removed; local recording does not
  query teammates' captured data through Screenpipe enterprise endpoints.

## Windows native Cargo test recipe (2026-09-07)

Native-linking tests need a different generator setup from the normal release
build. Use Visual Studio Developer PowerShell and `Ninja Multi-Config` for test
binaries that include `libsamplerate-sys`; single-config Ninja fails because
that crate expects `out\build\Release\samplerate.lib`. If the crate has already
been built with the wrong generator, clean only it with
`cargo clean -p libsamplerate-sys` before retrying.

The root workspace already forces `knf-rs-sys` to the release MSVC CRT in dev
tests. The desktop app is a separate Cargo workspace and does not inherit that
profile override, so its debug tests need the transient Cargo setting shown
below. Without it, `knf-rs-sys` uses the debug CRT while `whisper-rs-sys` uses
the release CRT, producing `LNK2038` and `_CrtDbgReport` errors. Merely adding
`--release` while keeping single-config Ninja is not sufficient; that attempt
failed earlier at the missing `samplerate` static library.

`OPENBLAS_PATH` locates headers and the import library at build time. The linked
test process separately needs `C:\Utils\OpenBLAS\win64\bin` on `PATH`, or it
exits with `STATUS_DLL_NOT_FOUND`. The audio build script stages
`onnxruntime.dll` in the active target profile directory.

From `screenpipe\`, the exercised locked offline desktop binding command is:

```powershell
$env:OPENBLAS_PATH = "C:\Utils\OpenBLAS\win64"
$env:CMAKE_GENERATOR = "Ninja Multi-Config"
$env:ORT_LIB_LOCATION = "D:\Data\NoSync\Repos\ScreenWise\screenpipe\apps\screenpipe-app-tauri\src-tauri\onnxruntime-win-x64-1.22.0"
$env:PATH = "C:\Utils\OpenBLAS\win64\bin;$env:PATH"
cargo --config 'profile.dev.package."knf-rs-sys".debug-assertions=false' test `
  -p screenpipe-app `
  --manifest-path apps\screenpipe-app-tauri\src-tauri\Cargo.toml `
  --locked --offline tauri_bindings_are_current -- --nocapture
```

This command built and linked successfully on 2026-09-07 and then reported the
separate checked-in binding drift described above. The cached test graph now
includes locked `assert-json-diff 2.0.2`; neither Rust lockfile changed.

## Functional baseline fixes and verification (2026-09-01)

- Root cause of the transcription panic: the locked `ort 2.0.0-rc.10` expects ONNX Runtime 1.22.x. `screenpipe-audio/build.rs` had already downloaded the correct Microsoft CPU package to `apps/screenpipe-app-tauri/src-tauri/onnxruntime-win-x64-1.22.0`, but bare Cargo builds did not stage its DLL beside `screenpipe.exe`. Windows consequently loaded `C:\Windows\System32\onnxruntime.dll` 1.17.1. The build script now copies the pinned runtime DLL into the active Cargo profile directory. This preserves `Cargo.lock` and dependency versions.
- The earlier untracked `crates\screenpipe-audio\onnxruntime-win-x64-1.22.0.zip` was the build script's temporary download artifact. It is not required after extraction, does not belong in source control, and was absent when this pass began. The extracted package is already ignored. Future work should download into a target/cache location and clean the archive after extraction.
- `screenpipe auth token` previously searched only the default data directory, while `record --data-dir ...` persisted the server key in the selected directory. `screenpipe auth token --data-dir <same-dir>` now resolves the matching secret store. In the smoke run, unauthenticated `/search` returned 403 and the same request with that token returned 200.
- The CLI's retention self-request was also receiving 403 after authenticated-localhost behavior was enabled. It now attaches the already resolved local bearer key; this is an internal consistency fix, not a security relaxation.
- `cargo build --release --locked` completed successfully in 14m 24s. The only warnings remained the existing `unused_mut` in `screenpipe-audio` and unused Windows `CommandExt` import in `screenpipe-engine`. `Cargo.lock` remained unchanged.
- Live verification used a fresh custom data directory, API authentication, telemetry disabled, local Parakeet, two audio devices, and all three monitors. The Parakeet model loaded successfully, authenticated audio search returned real transcript rows, and no `PANIC`, `panicked`, ONNX mismatch, or transcription error appeared in the run log. Ctrl+C completed a clean shutdown.
- Full subsystem and outbound-network evidence is recorded in `BASELINE_AUDIT.md`. `CONTRIBUTING.md` remains intentionally unchanged.
