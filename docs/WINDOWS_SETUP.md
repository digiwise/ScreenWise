# Windows setup

> [!WARNING]
> **Experimental code: no privacy or security guarantees.** Setup success does
> not establish that recording, exclusions, redaction or network controls are
> safe for confidential use. Use synthetic or non-sensitive data. Read the
> prominent [README notice](../README.md) and [known limitations](../VALIDATION_REGISTER.md).

This is a developer build guide, not an end-user installation or support service.
Neither the repository owner nor Digiwise provides support for this project in
any way. Developers must assess and maintain their own builds and test environments.

ScreenWise can be built from this repository alone. The checkout directory may
have any name; the executable and existing package identifiers remain `screenpipe`
for compatibility. Neither a parent ScreenWise workspace nor Litepipe is required.

## Prerequisites

This is a source distribution. Models, inherited recorded test fixtures and
compiled helpers are not bundled. See [distribution scope](DISTRIBUTION_SCOPE.md)
before acquiring or redistributing artifacts. Some manual tests and benchmarks
require separately supplied fixtures and cannot run from a plain clone.

Provision tools explicitly before building. The validated x64 combination was
Rust/Cargo 1.93.1, Visual Studio 2026 Developer PowerShell 18.9.2, MSVC 14.51,
Windows SDK 10.0.26100.0, CMake 3.31.8 and Ninja 1.13.2. Use the pinned
rust-toolchain.toml; other Visual Studio versions have not been revalidated here.
CMake 4.x failed with some bundled native scripts. CMake 3.31 does not know the
Visual Studio 2026 generator, so select Ninja Multi-Config explicitly. Include
`MinSizeRel`: the Rust release profile maps to that CMake configuration, and
`libsamplerate-sys` searches its configuration-specific output directory.

Required native inputs:

| Input | Layout / requirement |
|---|---|
| OpenBLAS x64 | Root contains `include/cblas.h`, `lib/libopenblas.lib`, `bin/libopenblas.dll` |
| ONNX Runtime 1.22.0 x64 | Root contains `lib/onnxruntime.dll`; the audio build verifies the pinned DLL checksum |
| FFmpeg and FFprobe | Matching installed pair on PATH or beside screenpipe.exe; no runtime download fallback |
| Desktop tools (optional) | Bun/Node and the locked frontend packages; provision required sidecars explicitly |

The accepted x64 ONNX DLL SHA-256 is
`579B636403983254346A5C1D80BD28F1519CD1E284CD204F8D4FF41F8D711559`.
The OpenBLAS DLL used in validation had SHA-256
`B554C45AF7B39154C561FB4879FD784D4928462E9A70335AADD9B1DE3C75E9E2`.
Windows ARM64 requires separately audited artifacts and an explicit
`SCREENPIPE_ORT_DLL_SHA256`; x64 results do not validate ARM64.

Open the Developer PowerShell supplied by your Visual Studio installation,
change to the repository root, and set paths to your provisioned directories:

```powershell
$repoRoot = (Get-Location).Path
$env:OPENBLAS_PATH = 'C:\Dependencies\OpenBLAS\win64' # choose your location
$env:ORT_LIB_LOCATION = 'C:\Dependencies\onnxruntime-win-x64-1.22.0'
$env:CMAKE_GENERATOR = 'Ninja Multi-Config'
$env:CMAKE_CONFIGURATION_TYPES = 'Debug;Release;RelWithDebInfo;MinSizeRel'
$env:PATH = "$(Join-Path $env:OPENBLAS_PATH 'bin');$env:PATH"
cargo build --release --locked
```

Cargo may acquire missing locked dependencies during an ordinary build. To
require a network-free build after explicit dependency provisioning, add
`--offline`. Neither flag supplies the separately provisioned native artifacts.
The build stages ONNX Runtime beside the executable. Copy the provisioned
OpenBLAS DLL beside it for distribution, or retain its `bin` directory on PATH.
Do not download an arbitrary replacement DLL to suppress a load error.

GnuWin32/unzip is only needed by tooling that actually invokes it; it is not a
reason to run old bootstrap scripts or installers. Check applicable tool scripts
before installing optional dependencies. Do not run broad `clean` scripts: some
inherited commands remove provisioned sidecars and unrelated build inputs.

## Build profiles and iteration time

Cargo keeps test/debug and release outputs separately under `target\debug` and
`target\release`; switching profiles does not overwrite the other profile's
artifacts. It also cannot reuse a release object as a test object. Cache reuse is
specific to the compiler, target, profile, enabled features, Rust flags and
relevant build-script environment. Keep the same Developer PowerShell variables
for related runs, and avoid changing features between otherwise equivalent tests.

A test-name filter controls which tests execute after compilation. It does not
make a small test binary: for example, a filtered `screenpipe-engine --lib` run
still compiles and links the complete Engine library test target. Finish and
format all related edits before starting that build, and avoid separately running
a dependency's broad suite when the higher-level focused regression already gives
the required evidence.

The desktop application is a separate Cargo workspace and its test binary pulls
in the full Tauri, capture and audio graph. Reuse one persistent repository-local
target directory for its checks and tests; a fresh directory turns even a
filtered test into a cold build. The ignored location below follows Cargo's
normal target-tree convention and can be overridden explicitly for a constrained
machine:

```powershell
$repoRoot = (Get-Location).Path
if ([string]::IsNullOrWhiteSpace($env:SCREENWISE_DESKTOP_CARGO_TARGET_DIR)) {
    $env:SCREENWISE_DESKTOP_CARGO_TARGET_DIR = Join-Path `
        $repoRoot 'target\desktop-tests'
}
$env:CARGO_TARGET_DIR = $env:SCREENWISE_DESKTOP_CARGO_TARGET_DIR
```

Keep the same toolchain, features and native environment when reusing that
cache. Keep it at the short repository-level path shown above: putting it below
the desktop workspace's own `target` tree can exceed CMake/MSVC object-path
limits while compiling libsamplerate. It may consume several gigabytes; clear
it only deliberately when no build is active. During intermediate editing,
catch type and integration errors without the expensive final test link:

```powershell
cargo --config 'profile.dev.package."knf-rs-sys".debug-assertions=false' check `
  -p screenpipe-app `
  --manifest-path apps\screenpipe-app-tauri\src-tauri\Cargo.toml `
  --tests --locked --offline
```

After related edits settle, run the required focused linked tests once against
the same target directory. `cargo check` does not establish runtime behavior,
and a test-name filter reduces execution rather than first-build compilation.

The root development profile optimizes dependencies at level 2 without their
debug symbols. This improves runtime and limits artifact size, but makes an
uncached test dependency graph more expensive to compile. The full `release`
profile uses full LTO and one code-generation unit and is intentionally slow.
Use it for a settled evidence candidate. For intermediate optimized executable
checks, the incremental `release-dev` profile is available:

```powershell
cargo build --profile release-dev --locked
```

The workspace permits 16 parallel build jobs. That can be counterproductive on
a machine that begins paging. If compilation creates sustained memory pressure,
retry a comparable cleanly identified build with a lower per-command limit and
record which setting is faster on that machine, for example:

```powershell
cargo test -j 8 -p <affected-crate> --lib <focused-filter> --locked
```

Do not treat eight jobs as a repository requirement; available memory and native
compiler load determine the useful value. Avoid a workspace-wide `cargo clean`
to address ordinary slowness. If a native package genuinely has incompatible
cached configuration, clean only that exact package as described below.

## Optional local AI chat

AI Chat requires the pinned Pi runtime in the same ScreenWise data directory
used by the application. ScreenWise never downloads or repairs this runtime in
the background. Close ScreenWise, open PowerShell in the repository and run:

```powershell
.\scripts\windows\Provision-ScreenWisePi.ps1 `
  -DataDir '<the ScreenWise data directory shown by the app>'
```

The explicit provisioning command downloads
`@earendil-works/pi-coding-agent@0.75.4` and its dependencies into
`<data-dir>\pi-agent`, verifies the expected entrypoint, version and required
runtime dependencies, and then promotes the completed staging directory. It
refuses to overwrite an existing incomplete or unexpected `pi-agent` directory;
review and move that directory aside manually before retrying. Restart
ScreenWise after provisioning succeeds. A selected local AI provider such as
Ollama must also be provisioned and running separately.

## Native tests

For builds and tests that link libsamplerate, use Ninja Multi-Config. If that
package was previously built with the wrong generator, use the exact package
spec `cargo clean -p 'libsamplerate-sys@0.1.12' --release` for release output
(omit `--release` for debug output); do not clean the whole workspace. The
unqualified cleanup did not remove the cached release artifacts in the fresh
published checkout.

```powershell
$env:CMAKE_GENERATOR = 'Ninja Multi-Config'
$env:CMAKE_CONFIGURATION_TYPES = 'Debug;Release;RelWithDebInfo;MinSizeRel'
$env:PATH = "$(Join-Path $env:OPENBLAS_PATH 'bin');$env:PATH"
cargo test -p screenpipe-events --lib --locked --offline
cargo --config 'profile.dev.package."knf-rs-sys".debug-assertions=false' test `
  -p screenpipe-app `
  --manifest-path apps\screenpipe-app-tauri\src-tauri\Cargo.toml `
  --locked --offline tauri_bindings_are_current -- --nocapture
```

The root workspace already provides the knf-rs-sys CRT override; the desktop is
a separate workspace and needs the transient setting. Missing runtime PATH can
produce STATUS_DLL_NOT_FOUND even after successful linking. Changing to release
tests does not solve the single-config samplerate layout mismatch.

## Local models and recording

Model acquisition is explicit and separate from recording. Runtime code verifies
supported files and fails locally rather than downloading missing artifacts.
For Parakeet, the manifest, immutable revision and expected file hashes are in
[`provisioning.rs`](../crates/screenpipe-audio/src/models/provisioning.rs).
On Windows its model directory is
`%LOCALAPPDATA%\screenpipe\audio-models\parakeet-tdt-0.6b-v3`.
Silero VAD is under `%LOCALAPPDATA%\screenpipe\vad`; segmentation and speaker
models are under `%LOCALAPPDATA%\screenpipe\models`. Consult their source
manifests for the accepted filenames/checksums. Redaction provisioning is
documented by [`screenpipe-redact`](../crates/screenpipe-redact/src/provisioning.rs).
Record each model's source revision, checksum and applicable license; a checksum
alone does not establish redistribution permission. Do not commit model caches.

After provisioning the local audio files, verify the exact built executable
without starting capture or downloading anything:

```powershell
.\target\release\screenpipe.exe audio models --output json
```

The command checks the pinned Parakeet hashes and initializes the local Silero,
speaker-embedding and speaker-segmentation models. A nonzero exit means a
requested audio/transcription trial should not start. Model access errors are
reported separately from missing files so execution-identity and permission
problems are not misdiagnosed as absent artifacts.

The optional smart text-PII model has its own license, which may be narrower than
this repository's MIT license. Review that license before provisioning or use.
Without it, `--async-pii-redaction` logs a reduced-coverage warning and retains
regex-only reconciliation. Neither mode is a privacy guarantee.

Recording is an explicit action and may capture sensitive data. Use `--help`
to choose devices, local engines and privacy settings before starting. Default
data is `%USERPROFILE%\.screenpipe`; use a fresh `--data-dir` for tests.
The matching local API token is retrieved with:

```powershell
.\target\release\screenpipe.exe auth token --data-dir '<your-test-directory>'
```

Keep that output private. Protected API requests require the bearer token even
on localhost. `/health` is startup-safe; the normal listener is `127.0.0.1:3030`.
Never disable authentication to repair a test or model configuration problem.

## Fixed outbound firewall diagnostic

Developers validating an executable-scoped outbound firewall rule can run:

```powershell
.\target\release\screenpipe.exe diagnostic outbound-tcp-probe
```

This hidden diagnostic makes exactly one TCP connection attempt to the fixed
address `1.1.1.1:443`, waits at most five seconds and closes a successful
handshake immediately. It performs no DNS lookup or application-protocol
exchange, sends and receives no application data, and accepts no destination,
port, payload, retry or timeout argument. Exit code 0 means the TCP handshake
completed; exit code 2 means it did not. Validate reachability separately with
an unscoped process and inspect the exact active OS firewall rule: the
diagnostic's own failure alone does not prove that the firewall caused it.

## Desktop packaging

Desktop build dependencies and native sidecars must be provisioned separately.
From `apps/screenpipe-app-tauri`, with Developer PowerShell and the variables above,
the validated unsigned NSIS command used:

```powershell
$env:CMAKE_GENERATOR = 'Ninja Multi-Config'
$env:CMAKE_CONFIGURATION_TYPES = 'Debug;Release;RelWithDebInfo;MinSizeRel'
node node_modules\@tauri-apps\cli\tauri.js build `
  --config src-tauri\tauri.prod.conf.json `
  --config src-tauri\tauri.windows.conf.json `
  --bundles nsis --no-sign --ci -- --locked --offline
```

The Windows override must follow the production config. Tauri may acquire its
packaging tools separately; Cargo's `--offline` does not control that acquisition.
Provision those caches beforehand for a fully disconnected packaging operation.
The package stages OpenBLAS, ONNX Runtime and the provisioned Bun sidecar. FFmpeg
remains an external prerequisite. No signed public release is claimed by the
historical unsigned packaging test.

See [validation status](../VALIDATION_REGISTER.md) and the
[interactive test guide](../scripts/windows/interactive-validation/README.md).
