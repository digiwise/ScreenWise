# ScreenWise Windows build notes

The portable build and test instructions are in
[docs/WINDOWS_SETUP.md](docs/WINDOWS_SETUP.md). Repository policy is in
[AGENTS.md](AGENTS.md); no parent workspace document is required.

## Validated environment and important repairs

The Windows x64 development baseline used Rust/Cargo 1.93.1, Visual Studio 2026
Developer PowerShell 18.9.2, MSVC 14.51, Windows SDK 10.0.26100.0, CMake 3.31.8
and Ninja 1.13.2. Other configurations are not implied to pass.

- Release builds use Ninja; native tests linking libsamplerate need Ninja
  Multi-Config. The desktop's separate Cargo workspace needs the documented
  transient knf-rs-sys debug CRT override.
- OpenBLAS headers/libraries and its runtime DLL are separate requirements.
  Put its bin directory on runtime PATH or bundle the DLL beside the executable.
- Explicitly provision ONNX Runtime 1.22.0. The audio build verifies and stages
  the accepted DLL so Windows does not load its incompatible system copy.
- Build scripts and recorder runtime do not silently acquire FFmpeg, ONNX Runtime
  or supported local models. Missing inputs fail with setup guidance.
- The Windows packaging configuration stages OpenBLAS/ORT/Bun; FFmpeg is an
  external prerequisite. An unsigned NSIS build passed historically, not a
  signed/public release certification.
- Root default workspace members intentionally exclude the Apple-only MLX
  target from ordinary Windows builds without removing its workspace metadata.

## Verification history (summary)

September 2026 work established WGC/multi-monitor, UIA/OCR, SQLite, local audio/
Parakeet and authenticated loopback operation, followed by removal of hosted
services, telemetry and implicit downloads. Later privacy/audio fixes passed
scoped unit/build checks and selected live regressions. The latest bounded
lock/unlock and audio tail/restart results and remaining failures are in
[VALIDATION_REGISTER.md](VALIDATION_REGISTER.md).

The current public-note versions deliberately omit private machine paths, device identifiers,
process IDs, raw logs and captures. Detailed authored pre-publication notes are
preserved locally under ignored `.local/maintainer-notes/`; they are not required
build inputs, current consent or independently reproducible public test artifacts.
## Publication checkpoint checks (2026-09-16)

The reviewed source passed `cargo build --release --locked --offline` in the
documented Developer PowerShell environment (Ninja, 6m55s), Rust formatting and
the non-recording CLI `--help` smoke check. Both Rust lockfiles stayed unchanged.
With Ninja Multi-Config and OpenBLAS on PATH, targeted offline native tests passed:
9 event tests, 4 audio shutdown tests, 3 channel-lag tests, 9 Windows lock-logic
tests and 5 privacy-notice persistence tests. These use synthetic state/data.

Frontend checks passed 3 timeline notice tests and 7 URL-detection tests using
the new nine-case synthetic fixture. These are not real-world OCR accuracy results.
The publication-history helper passed 9 synthetic tests, including mixed old and
noreply commit identities. The portable harness previously passed 224 offline
tests with one Windows symlink-privilege skip; all 44 source manifest entries
were rechecked. Ten PowerShell files were parsed during portability preparation.

Existing warnings remain: unused audio `mut`, an unused Windows CommandExt
import, a test-build unused `sample_rate`, and the Vite CommonJS deprecation.
An initial help check truncated stdout early and returned a nonzero exit; reading
the complete help output returned exit 0. No capture, audio playback, model download,
live privacy test or firewall change was performed for this checkpoint. A fresh
clone's cold build, packaging and omitted-fixture tests are not claimed to pass.

Source review identified the pre-existing audio privacy/persistence race recorded
as SW-V19. This checkpoint preserves incomplete validation, not privacy certification.

The Windows OCR integration target compiled and listed successfully without
executing capture/OCR. Its external-fixture test and the two Apple OCR fixture
tests are explicitly ignored pending operator-provided reviewed fixtures.
The final staged tree excluded 45 inherited asset paths while preserving their
local bytes. It contained no active upstream workflow, private evidence path or
maintainer private-email match in the 520 changed text files checked. A separate
bounded credential-pattern scan found no high-confidence real credentials;
neither scan is an exhaustive security audit. All 43 local links in the nine
primary guides and the three edited Tauri JSON files validated.
