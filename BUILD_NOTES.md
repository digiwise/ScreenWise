# ScreenWise Windows build notes

The portable build and test instructions are in
[docs/WINDOWS_SETUP.md](docs/WINDOWS_SETUP.md). Repository policy is in
[AGENTS.md](AGENTS.md); no parent workspace document is required.

## Validated environment and important repairs

The Windows x64 development baseline used Rust/Cargo 1.93.1, Visual Studio 2026
Developer PowerShell 18.9.2, MSVC 14.51, Windows SDK 10.0.26100.0, CMake 3.31.8
and Ninja 1.13.2. Other configurations are not implied to pass.

- Cold Windows release builds and native tests linking libsamplerate use Ninja
  Multi-Config with `MinSizeRel` included in `CMAKE_CONFIGURATION_TYPES`.
  Single-config Ninja can build `samplerate.lib` successfully but place it
  outside the configuration-specific path used by `libsamplerate-sys`. The
  desktop's separate Cargo workspace needs the documented transient knf-rs-sys
  debug CRT override.
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

## Published-checkout build and initial run (2026-09-16)

A clean `screenwise-public` target built offline from the published source at
`1b22c5b8443659142bab84d50f6d89829b5c47a4` using explicitly provisioned native
inputs. The first attempt exposed the single-config libsamplerate output layout
above. After the scoped dependency cleanup, Ninja Multi-Config with `MinSizeRel`
enabled completed the optimized build in 6m29s. `screenpipe --help`, `screenpipe
doctor`, Rust formatting, `git diff --check`, and nine release-profile
event-pressure tests passed. Doctor confirmed screen-recording, microphone and
accessibility permissions, FFmpeg discovery and port availability. The staged
ONNX Runtime and OpenBLAS DLLs matched the documented hashes.

A fresh audio-disabled initial run started the authenticated API, discovered all
three monitors, created its SQLite store and listened only on `127.0.0.1:3030`.
Health returned 200; protected search returned 403 without or with a wrong token,
and 200 with the data-directory token. The DRM gate activated immediately and
safely stopped vision/UI acquisition, so this run does not establish frame, UIA
or OCR writes from the published checkout. The controlling PTY did not deliver a
Windows console-control event, so that first verified test PID was stopped
explicitly. A second fresh run used the recorder's watched-process shutdown path;
it stopped UI capture and VisionManager and logged `shutdown complete` with exit
code 0. This exercises orderly managed shutdown, but not Ctrl+C delivery from a
normal Windows console. No captured content was inspected.

A subsequent one-time-gated synthetic privacy run used the same published
checkout and release binary with audio disabled. Known allowed text was admitted
before and after the privacy phases: the fresh store contained 13 frames, five
UI events and three frame matches for the allowed marker. Password,
excluded-foreground and excluded-background phases each produced zero matches
for their fixed forbidden markers across frame text, transcripts and UI events.
The run also persisted one fixed `windows_lock` notice with reason
`wts_session_unlocked`. Health returned 200; protected search, audio-device
status, capture-events and audio-metrics endpoints each returned 403 for missing
and wrong bearer tokens and 200 for the correct local token. SQLite
`quick_check` returned `ok`, the recorder logged a clean managed shutdown, and a
separate post-run inspection found no owned recorder/fixture processes or TCP or
UDP endpoints on the two test ports. The run made no outbound diagnostic
attempts. This establishes only synthetic text admission for the exercised
fixture phases; it does not establish image-pixel redaction, clipboard safety,
other-monitor behavior, real DRM behavior, audio capture/transcription or
firewall drop enforcement. No captured content was inspected.

A fresh current-binary capture verification then exercised the same synthetic
fixture. Its first gated attempt stopped before recording because fixture focus
could not be verified; scoped cleanup passed. The retry started Windows capture
for all three detected monitors and passed the fixture phases. The fresh SQLite
store contained 15 frame rows referencing 15 distinct existing snapshot files,
including three admitted frames with image hashes, accessibility trees and the
known synthetic marker in accessibility text. Twelve other rows were explicit
privacy placeholders. Five UI-event rows recorded two app switches, two window
focus changes and one safe privacy notice. SQLite `quick_check` returned `ok` at
the end of this capture run, and the controller completed a clean shutdown.

Because those admitted frames selected accessibility text rather than OCR, a
separate vision-disabled restart invoked the authenticated on-demand frame-text
endpoint for one reviewed synthetic frame. Windows Native OCR persisted one
`ocr_text` row containing the known marker. No new capture was enabled and no
arbitrary captured text or image pixels were inspected. The stock Python SQLite
library could not repeat `quick_check` after application vector indexes were
loaded because it lacks the registered `vec_length()` function; direct reads
confirmed the OCR row after process exit. The watched-process signal was logged
but this auxiliary restart did not finish within the wrapper's 45-second limit
and was force-stopped. A final independent check found no recorder or fixture
processes and no TCP or UDP endpoints on the test ports. This shutdown warning
does not invalidate the persisted WGC/UIA/OCR evidence, but it is not a clean
shutdown result for the auxiliary OCR restart.

The Windows OCR integration target compiled and listed successfully without
executing capture/OCR. Its external-fixture test and the two Apple OCR fixture
tests are explicitly ignored pending operator-provided reviewed fixtures.
The final staged tree excluded 45 inherited asset paths while preserving their
local bytes. It contained no active upstream workflow, private evidence path or
maintainer private-email match in the 520 changed text files checked. A separate
bounded credential-pattern scan found no high-confidence real credentials;
neither scan is an exhaustive security audit. All 43 local links in the nine
primary guides and the three edited Tauri JSON files validated.
