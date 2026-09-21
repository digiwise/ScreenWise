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

## Day-to-day trial repair checkpoint (2026-09-21)

A private roughly nine-minute Windows trial produced 213 captured and 213 written
frames, 138 UI events and 16 audio chunks, with zero reported frame drops, pipeline
stalls, persistence degradation, event-delivery loss, near-capacity audio notices,
audio-shutdown degradation, panics or owned processes after exit. All 16 audio
chunks were rejected by VAD and no transcript was produced, so the run did not
establish live transcription. Captured content was not inspected.

The trial exposed an orphan low-level UIA tree producer (150 bounded-queue-full
warnings), duplicate multi-monitor frame correlations recorded as 101 false TTL
evictions, repeated Parakeet-unavailable warnings, ambiguous model file errors,
and a null launcher exit code despite a logged clean shutdown. The engine now
keeps UIA focus/password/input checks while disabling that unconsumed tree
producer, deduplicates resolved frame correlations, exposes frame-link counters
in `/health`, distinguishes requested-but-unavailable transcription, reports
model access errors accurately, logs Parakeet availability transitions once,
and requires the exact executable's non-recording model preflight before a
Parakeet trial. The launcher infers exit 0 only from a new clean-shutdown marker.

One post-capture privacy evaluation failed closed into a visible placeholder and
recovered on the next observed frame. Ten unavailable password-state checks
suppressed keyboard/clipboard content as designed. These are retained warnings,
not proof of unsafe disclosure. The optional smart text-PII pack was absent, so
regex-only reconciliation ran; the pack's separate noncommercial license was not
silently accepted or provisioned.

The repaired tree passed an offline release build. The exact release binary
verified all provisioned audio models. Full locked offline library suites passed:
screenpipe-engine 554 passed, 2 ignored; screenpipe-audio 179 passed, 1 ignored.
The day-to-day launcher preflight then passed against the exact active firewall
group with `AudioModelsReady=true` and recording disabled. No post-fix live
capture or transcription run is claimed.

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

## Fixed outbound firewall diagnostic (2026-09-16)

The release CLI now has a hidden developer diagnostic,
`diagnostic outbound-tcp-probe`. It accepts no address or other network
arguments and makes exactly one TCP connection attempt to `1.1.1.1:443` with a
five-second limit. A completed handshake is closed immediately; the diagnostic
does not perform DNS, TLS, HTTP, application reads or application writes.

The locked offline release build passed. The active Windows Firewall rule named
`ScreenWise-Offline-20260916-1b22c5b84-recorder` was enabled, outbound, blocking
all non-loopback IPv4 and IPv6 addresses, and scoped to the exact rebuilt
`screenpipe.exe` path. A one-attempt unscoped PowerShell control completed the
TCP handshake to the fixed endpoint. The ScreenWise diagnostic then returned
`PermissionDenied`, reported zero application bytes and exited 2. This
comparative result, together with the exact ActiveStore rule inspection,
establishes blocking for this IPv4 TCP attempt. It is not packet-drop tracing,
continuous coverage, a UDP result or an externally routed IPv6 result.

## Current-build selected-output transcription (2026-09-16)

The rebuilt release binary then passed the prepared short `audio-output` batch
with the exact selected device `Headphones (2- Realtek USB2.0 Audio)`. The
controller waited for the capture handle before playing two fixed local speech
fixtures through Windows default output. Six audio chunks and nine local
transcription rows were written. The fixed `silver cedar` and `amber window`
markers each appeared once in persisted transcription rows and were each found
through authenticated local search. No unrelated marker appeared. The fresh
store's SQLite `quick_check` returned `ok`; protected API checks retained the
403/403/200 missing/wrong/valid bearer behavior; playback stopped and the
recorder, audio workers and fixture shut down cleanly on the first attempt. An
independent post-run check found no owned processes and no TCP or UDP endpoints
on the two test ports. No captured audio or transcript body was inspected beyond
the allowlisted synthetic-marker counts. This establishes the two known phrases,
selected-device routing and persistence/search, not full-clip transcription
accuracy, diarization quality, every audio device or final-tail preservation.
The final staged tree excluded 45 inherited asset paths while preserving their
local bytes. It contained no active upstream workflow, private evidence path or
maintainer private-email match in the 520 changed text files checked. A separate
bounded credential-pattern scan found no high-confidence real credentials;
neither scan is an exhaustive security audit. All 43 local links in the nine
primary guides and the three edited Tauri JSON files validated.

## Audio privacy and queue diagnostics (2026-09-16)

The audio persistence path now retains its database write exclusion through the
post-commit privacy-generation check and performs exact compensating cleanup
before returning a stale result. Covered writes are raw chunk rows and their
files, combined chunk/transcript writes and overlap edits, live and reconciled
diarization runs and segments, and live meeting transcript segments. The tests
force generation invalidation after commit and inspect durable state, including
restoring pre-existing chunk state and overlap transcript text. Speaker identity
matching remains a separate open boundary: it can add an embedding, update a
shared centroid or create a speaker, and safe reversal needs transactional
before-images plus ownership-aware compensation.

Audio delivery diagnostics now use fixed queue enums and numeric counters only.
The device-capture, recording, transcription-result, meeting-tap,
meeting-provider, meeting-final and meeting-persistence queues report an 80%
near-capacity transition, 50% recovery, exact confirmed drops where known, and
separate possible-loss counts when shutdown or task failure prevents completion
from being verified. Fixed allowlisted messages are written to local logs and
the existing general activity timeline; current latched state is exposed by
`/capture-events`. The deterministic suites passed 35 event tests, 93 database
tests, 179 audio tests with one ignored test, and six scoped engine notice tests.
The documented Developer PowerShell environment also completed a locked offline
release build in 7m17s.
After installing the exact dependency graph from the unchanged `bun.lock`, the
timeline component test passed 5/5 and `tsc --noEmit` passed. The complete
frontend command then passed 37 Vitest files/404 tests and 13 Bun files/150
tests. This includes the previously excluded text-overlay file after removing
obsolete generic-click and persistent-underline expectations; the second stale
exclusion named an OCR test removed by the existing frame-text hook replacement.
Both exclusions were removed. Renaming the Vitest config to the explicit ESM
`.mts` extension also removed the Vite CommonJS deprecation warning. These tests
used installed Bun 1.4.0, now also declared by the manifest and packaging pins;
the lockfile was not rewritten. Real overload/soak and visual packaged-timeline
checks remain outstanding.

The reusable interactive controller now also contains an inert 25-second
browser/password/clipboard sequence with five fixed phases, strict positive and
forbidden synthetic marker deltas, and verified cleanup requirements. Its 82
offline Python tests passed. It does not start a listener or browser and does
not read or modify the clipboard. Live execution stays disabled until the
controller can own and verify the browser process tree and the exact browser
executable receives a separately reviewed firewall scope.
