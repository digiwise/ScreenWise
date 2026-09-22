# ScreenWise validation and issue register

Updated 2026-09-22. **Partial validation, not a completed privacy/firewall certification.**
> [!WARNING]
> **No privacy or security guarantees.** These bounded results do not establish
> that the system or code is safe for confidential use. The maintainer and
> Digiwise make no such assurance; see the [README notice](README.md).

This sanitized public summary preserves the scope and unresolved issues. Raw
captures, logs, stores, machine identities and owner interaction records are
private. Historical authored notes are preserved locally under ignored `.local/`.

## Established results

| Area | Bounded result | Limit |
|---|---|---|
| Windows screen/UI | WGC across three displays, UIA trees, Windows OCR and SQLite writes passed | Not every display/app/provider combination |
| Local API | /health passed; missing/wrong/valid bearer requests gave 403/403/200; tested listener was IPv4 loopback | Protected localhost requests still require authentication |
| Local transcription | Microphone and system-output capture, Parakeet transcription, known-phrase search and transcript persistence across restart passed; the current public-checkout build also persisted and found both short selected-USB-output synthetic controls | Not a full-passage accuracy, diarization-quality, every-device or final-tail benchmark |
| Native privacy | Synthetic password markers absent in inspected text/log fields; foreground/background title exclusions and other-monitor fixture controls passed | Browser/custom UIA controls and race boundaries remain open |
| Input switches | Disabled keyboard/clipboard switches produced no input-content rows during synthetic stimuli | Screen/UIA/OCR may still see ordinary displayed text |
| Audio shutdown | One 13.798 s selected-output partial chunk retained both synthetic markers; authenticated search found it after a distinct process restart and found a new chunk | Active-meeting shutdown, same-process restart and every device are not covered |
| Lock/unlock | Audio-disabled real transition passed; a later audio-enabled day-to-day run recorded one 10,770.1 s privacy pause with zero frame or audio-chunk counter advances inside its sampled locked interval, then automatic WGC/audio recovery | Counters and fixed notices were inspected, not captured content; this is not every-pixel proof |
| Safe lock notices | Fixed unlocked/desktop-unavailable/locked reasons persisted through timeline API and local logs; clean stop with no owned processes remaining | Packaged desktop timeline was not visually exercised in this run |
| OS firewall | Exact recorder/media paths had outbound block rules; the fixed no-payload `screenpipe.exe` IPv4 TCP diagnostic was denied while an unscoped control handshake succeeded; helper IPv4/IPv6 localhost access worked | External IPv6 route unavailable; UDP/drop-trace coverage and closeout incomplete |
| Published checkout | Clean offline release build, CLI help/doctor and nine event tests passed; fresh gated runs wrote 15 distinct snapshots, three accessibility-tree frames and Windows Native OCR text, persisted and found both selected-USB-output audio controls, and passed a scoped five-phase Chrome/password/excluded-host/clipboard run; SQLite checks and main capture/audio shutdowns passed | Image-pixel redaction, arbitrary browser/clipboard providers, real DRM, full audio accuracy and every-device behavior were not exercised; the separate vision-disabled OCR restart required forced shutdown after its watched-process signal |

The final lock result's SHA-256 was
`A82FA50F5DD29402319997CEB75EAD9E70DC6FDC03CF663F86907B2CB1AE0F89`.
Its tested recorder SHA-256 was
`26578A85C37C9D90AC85E84DC4F8BCA9FBB3CA7E46B733361113BEB8182AF0B7`.
These identify historical artifacts, not a claim that a new checkout/build has
been tested. Original incomplete runs were retained. Harness repairs included
fresh search-cache bounds, bounded unknown desktop transitions, serialized
fixture commands, safe exception diagnostics and phase evidence checkpoints.

## Long locked day-to-day trial, 2026-09-21

A 12,166.2-second private run remained locked for one 10,770.1-second interval.
The content-free audit showed zero frame and audio-chunk counter advances inside
the sampled locked interval. Fixed notices recorded secure-desktop/lock entry,
discarded UI/audio work and unlock; WGC and both selected audio streams rebuilt
automatically. After unlock the current process reached 519 captured and 519
database-written frames, 44 audio chunks, seven VAD-positive chunks and 18
completed/inserted transcriptions. No transcript, captured text, image, audio,
clipboard or keyboard value was inspected.

Reported frame drops, pipeline stalls, update failures, transcription errors,
queue capacity/loss notices, persistence degradation, audio-shutdown degradation
and confirmed/possible delivery loss were all zero. Shutdown logged its clean
completion, no panic log was created and the final exact-path inventory found no
recorder, FFmpeg or FFprobe process. Three audio timing-gap warnings were handled
by bounded silence insertion; this result does not establish gap-free audio.

The run retained six source-free active-window acquisition placeholders and four
unresolved frame-link TTL expiries. These remain data-availability warnings, not
evidence of sensitive disclosure. Thirty-six intermittent unavailable UIA
password-state checks failed closed by suppressing keyboard/clipboard content.
The optional smart text-PII pack remained absent, so regex-only reconciliation
continued with its existing reduced-coverage warning.

The audit also exposed false/noisy diagnostics. Local wall time was labelled UTC,
rolling files contained ANSI escapes, any fresh batch-transcription backlog made
`/health` degraded, and an intentional lock pause eventually made audio appear
stale. The launcher missed a real clean-shutdown marker for the same log-format
reason. Source changes now use actual UTC and plain rolling logs, treat only the
existing bounded backlog-stall condition as degraded, keep known privacy pauses
healthy while exposing `transcription_paused`, persist a content-free final trial
summary and split future TTL expiries by direction. Per-frame Basic PII success
and negative meeting scans moved to DEBUG; state changes and failures stay visible.
The repaired tree passed 558 engine tests with two ignored, all 181 audio tests,
the separate rolling-log regression, all 23 capture tests, Rust formatting,
PowerShell parsing and a locked offline optimized build. Focused tests now cover fresh
batch backlog, privacy-paused audio without hiding hard queue/database failures,
actual UTC/plain file-log formatting, per-run log offsets, lock-interval counter
plateaus, ANSI shutdown parsing, failure classification and loss counts. The
non-recording synthetic trial-status regression passed. The exact rebuilt
executable (`D445B05CC5E8B5A5C06C765D9C0D81942D4D3BE2D0F3695D89B6322BE75E0AB3`)
passed its six-model check and the existing-rule trial preflight with recording
disabled. The manual run below verified healthy privacy-paused audio, ordinary
fresh backlogs, plain UTC logs and final-summary creation while exposing further
fresh-directory and silent-audio diagnostic defects.

## Manual day-to-day trial, 2026-09-22

A fresh private 1,455.5-second run produced 47 content-free operational samples.
The final sample reported healthy frame and audio states, 823 captured and 823
database-written frames, zero frame drops/stalls/link expiries/update failures,
44 audio chunks, four VAD-positive chunks and three completed/inserted
transcriptions with zero transcription errors. Two pending transcription
segments were reported by the last sample roughly 30 seconds before shutdown;
no captured database content was inspected to infer their later disposition.
Although the final sample was healthy, 32 of 47 samples were degraded and 31
reported stale audio. Numeric correlation showed that chunks continued arriving
about every 31 seconds and VAD rejected almost all of them as silence. The health
clock incorrectly followed transcript database writes instead of this active
consumer heartbeat.

One 60.2-second lock interval had zero frame and audio-chunk counter advances
inside its sampled window. Unlock rebuilt the selected audio streams and capture
resumed. The launcher and corrected log scan both found clean shutdown, exit code
zero, no panic file, no persistence/audio-shutdown degradation, no confirmed or
possible delivery loss and no remaining recorder/media process.

The run recorded no acquisition-failure placeholders or unclassified errors.
All 829 warnings belonged to known fixed categories: 724 excluded-foreground,
nine excluded-background, 23 no-safe-window, 53 fail-closed unavailable-password-
state suppressions, 15 bounded audio timing gaps, four expected unlock audio
recovery notices and one optional smart-PII reduced-coverage notice. No captured
pixels, text, transcripts, audio, input values, titles, URLs or device names were
read for this audit.

This fresh directory exposed three diagnostics defects. An empty v2 log-offset map
was mistaken for the legacy local-clock format, hiding the lock and shutdown
lines from the generated summary. Version-based parsing and a fresh-directory
regression now fix that case. Comparer-only visual probes also reset the logged
capture state between persisted exclusion placeholders, causing hundreds of
duplicate transition warnings. Probe observations no longer mutate the persisted
transition tracker; a focused regression covers that sequence. A live rerun is
still required to measure the resulting warning reduction. The audio consumer
now advances its existing attempt heartbeat on every received chunk, and health
uses that heartbeat as well as transcript writes. Focused metrics and health
tests cover silent-but-active capture; live confirmation remains outstanding.

## Issue register

| ID | Current state / next check |
|---|---|
| SW-V01 | Native foreground DRM policy repaired in source; scoped synthetic Chrome allowed/password/excluded-host transitions passed. Sustained real DRM/audio recovery and broader background/other-monitor transitions remain incomplete. No protected playback bypass. |
| SW-V02 | Explicit audio selection repaired; selected-output live regression passed. Broader fresh/persisted microphone-only, defaults, disable-audio and device changes still need coverage. |
| SW-V03 | Earlier runs had two unexpected WGC acquisition failures. The long day-to-day run added six active-window acquisition failures while exclusions forced active-window-only capture. Source-free placeholders were retained and later captures recovered. The transient OS/window cause remains unidentified. |
| SW-V04 | Microphone transcript rows/phrase hits passed, complete passage accuracy and retention did not. Distinguish ASR substitutions, VAD rejection and actual loss. |
| SW-V05 | Earlier DRM audio tests lacked positive before/after controls; they cannot establish suppression. Retry only with working controls. |
| SW-V06 | Scoped live clipboard evidence passed: an ordinary fixed marker produced three positive rows, the password-phase secret marker produced zero rows, three clipboard UI events were present, and opaque clipboard restoration followed verified recorder stop. Arbitrary providers, applications, formats and race timing remain open. |
| SW-V07 | The orphan lower-level UIA tree producer remains disabled while paired UIA capture and password/focus/input checks stay active. Multi-monitor duplicate frame correlations are deduplicated. A long post-fix run still produced four residual TTL expiries; future metrics now distinguish event-without-frame from frame-without-event so the remaining path can be located. |
| SW-V08 | Fixture focus/equality limitations addressed with OS identity and stimulus acknowledgements; continue using these guards. |
| SW-V09 | Windows locked-state detection repaired and already-locked/audio-disabled transitions passed. A later audio-enabled run showed a roughly three-hour sampled frame/audio plateau and automatic WGC/input/output recovery after unlock. Broader repeated-lock, DRM and device-change recovery remain open. |
| SW-V10 | General event pressure/recovery/subscriber-loss reporting implemented; 28-event-suite tests passed. Real overload/soak remains open; delivery counts are not unique lost database rows. |
| SW-V11 | Cooperative shutdown and worker/persistence ownership repaired; selected-output partial-tail/process-restart passed. Active meeting and same-process manager restart still need live checks. |
| SW-V12 | Short stimulus/first-chunk mismatch corrected in preparation; bounded later selected-output tests passed. |
| SW-V13 | Deterministic broadcast-lag integration test repaired; all three locked offline Cargo tests passed. |
| SW-V14 | Nested Windows UIA exclusion gap repaired; scoped silent native privacy retest passed. Broader exclusion variants remain open. |
| SW-V15 | Harness capture-worker readiness race repaired; selected-output live retest passed. |
| SW-V16 | Duplicate lock monitors prevented with one process worker and serialized probes; synthetic tests passed. Repeated live desktop server lifecycle remains open. |
| SW-V17 | Partial audio buffer splitting/swallowed final-delivery failure repaired; synthetic tests and one complete live partial chunk passed. |
| SW-V18 | Typed near-capacity, recovery, confirmed-loss and possible-loss reporting now covers device capture, recording, transcription-result, meeting-tap, meeting-provider, meeting-final and meeting-persistence queues. Fixed content-free notices reach local logs, the general activity timeline and `/capture-events`; deterministic event/audio/engine tests passed. Real overload/soak and the packaged visual timeline remain open. Counts describe queue deliveries, not unique lost database rows. |
| SW-V19 | Audio privacy-transition compensation now covers raw chunk rows/files, combined chunk/transcript/overlap writes, live and reconciled diarization runs/segments, and meeting transcript segments while retaining the database write guard through the stale-generation check and exact cleanup. Deterministic durable-state tests and the full database/audio suites passed. This does not cover the separate speaker-identity mutation in SW-V20 or establish universal transition safety. |
| SW-V20 | **Open, source-review finding:** speaker matching can create a speaker, add an embedding or update a shared speaker centroid while a privacy generation changes. Safely reversing a shared identity requires transactional before-images and ownership-aware compensation; deleting a pre-existing/shared speaker would be unsafe. |
| SW-V21 | Model inspection distinguishes missing, wrong-type and access failures; warnings are transition-deduplicated; `/health` marks requested-but-unavailable transcription; and the trial refuses to record unless the exact executable verifies the local pack. A live post-fix run completed and inserted 18 current-process transcriptions with zero transcription errors; content and accuracy were not inspected. |
| SW-V22 | The same trial had one post-capture privacy-evaluation failure; it persisted a visible safe placeholder and recovered on the next observed frame. No sensitive content was inspected and the generic failure stage does not identify the transient OS cause. Retain as an open reproducibility/observability warning; do not weaken the fail-closed behavior. |
| SW-V23 | The optional smart text-PII model was absent and regex-only reconciliation remained active. Its published CC BY-NC 4.0 license is separate from this MIT repository, so it was not silently provisioned for this Digiwise-associated workflow. Reduced coverage is now explicit in the log and setup guide; neither regex nor AI redaction is a privacy guarantee. |
| SW-V24 | **Repaired, regression-tested, rebuilt and live-checked:** `/health` treated any fresh batch backlog and expected privacy-paused audio staleness as degradation. It now uses the existing bounded backlog-stall predicate, treats a known privacy pause like the vision gate and exposes `transcription_paused`. Two samples during the 2026-09-22 lock pause remained healthy with unchanged frame/audio counters. Focused tests also ensure a pause cannot hide confirmed queue/database failures. |
| SW-V25 | **Repaired, regression-tested and rebuilt; fresh-directory parser rerun pending:** rolling log timestamps now use UTC when suffixed `Z`, file logs omit ANSI, shutdown scanning uses per-run byte offsets, and the launcher writes a content-free final summary. The 2026-09-22 run verified plain UTC logs and summary creation but exposed SW-V26. The Rust file-format test and non-recording synthetic status test cover the corrected parsing paths. Existing legacy runs remain readable. |
| SW-V26 | **Repaired, regression-tested and rebuilt; live check pending:** the fresh 2026-09-22 trial had an empty v2 offset map, which the status script misclassified as legacy and therefore omitted UTC lock/shutdown lines. Launch schema now selects legacy parsing, and the synthetic test covers both non-empty and empty v2 offset maps. |
| SW-V27 | **Repaired, regression-tested and rebuilt; live check pending:** comparer-only visual probes repeatedly reset persisted Windows capture transition state, producing 724 excluded-foreground warnings in one 24-minute run. Probe results no longer mutate or log the persisted state; actual persisted privacy and failure transitions remain visible. |
| SW-V28 | **Repaired, regression-tested and rebuilt; live check pending:** 31 samples reported stale audio while chunks continued arriving and were VAD-rejected as silence. Received chunks now advance the intended consumer/transcription-attempt heartbeat, and `/health` accepts a recent heartbeat without requiring recognized speech or a database insert. |

## Outstanding test plan

| ID | Required scope |
|---|---|
| SW-T01 | Extend the passed audio-disabled lock transition to audio cancellation/recovery and stale-generation suppression; retain independent OS observations. |
| SW-T02 | **Scoped complete:** the exact Chrome rule and 74-check preflight passed; fixed allowed browser controls persisted while the browser-password and excluded-host markers remained at zero. Exact-path Chrome inventory and cleanup passed. This does not establish universal browser secrecy or actual firewall packet drops. |
| SW-T03 | Sustained real/synthetic DRM distinction and audio recovery; repair the DRM fixture's similar command re-entry pattern before reuse. |
| SW-T04 | App-only and App::Title exclusions, rapid transitions, enumeration failure and race stress. |
| SW-T05 | **Scoped complete:** fixed ordinary clipboard text persisted, the password-phase secret marker remained at zero, recorder stop preceded opaque clipboard restoration, and cleanup passed. Live provider-failure, unknown-focus and non-text-format faults remain untested beyond deterministic checks. |
| SW-T06 | Audio/meeting completeness, diarization quality, active-meeting shutdown, pending recovery and same-process restart. |
| SW-T07 | The long locked run covered combined WGC/UI/input/audio/STT counters, lock recovery and loss diagnostics. OCR output, unplug/replug, monitor changes, active overload and content accuracy still require controlled coverage. |
| SW-T08 | Fixed no-payload `screenpipe.exe` IPv4 TCP attempt passed blocked-versus-unscoped control. Usable external IPv6 and UDP controls plus OS drop tracing remain open. |
| SW-T09 | Desktop/WebView/MCP/optional executable scope: inventory relevant child processes before extending firewall claims. |
| SW-T10 | Owner-confirmed disposition/restoration of exact test rules, independent read-only final inspection and original milestone closeout. |
| SW-T11 | Visual desktop status and broader safe-notice audit across acquisition/audio/input failures and independent queues. |
| SW-T12 | Extend the passing deterministic privacy-generation tests with live transition timing and the SW-V20 speaker-identity path. Inspect durable state as well as returned results; do not generalize the fixed persistence paths to universal stale-generation suppression. |

## Reuse and conduct

Use [the configurable test sources](scripts/windows/interactive-validation/README.md)
and [Windows setup](docs/WINDOWS_SETUP.md). Prepare in the background; request fresh
readiness with recording stopped and no full-screen takeover. No captured private
contents may be read without current authorization. Use explicit model/tool setup,
keep auth enabled, preserve loopback, and never change global firewall settings.
The three original local firewall rules remain installed pending owner disposition;
this public summary deliberately does not provide machine-specific removal commands.
A newly configured harness must pass its offline checks and a new authorized live
run; historical passes do not validate its portability changes.

## Scoped browser and clipboard validation, 2026-09-18

The first gated attempt ended incomplete before any phase with
`process_inventory_unavailable`. Chrome had exited between a Toolhelp snapshot
and its identity query, and child processes briefly outlived the root process.
Cleanup still stopped the recorder and fixture, restored the clipboard, closed
the loopback server and reached later exact-path process quiescence. The harness
was repaired to retry only that bounded exit race while retaining fail-closed
behavior for persistent unknown identities, and to allow exact-path Chrome
children a bounded drain after root shutdown. The complete controller suite then
passed 105 tests and the live preflight passed all 74 checks.

The fresh retry completed all five verified phases. Fixed public browser markers
had positive deltas of one each, the ordinary clipboard marker had a positive
delta of three, and the allowed clipboard control had a positive delta of one.
The browser-password, excluded-host and password-clipboard secret markers all
remained at zero in the final aggregate query. The store contained 12 frames and
eight UI events (two app switches, three clipboard events, one privacy notice and
two window-focus events); `PRAGMA quick_check` returned `ok`. Missing, wrong and
valid bearer requests returned 403/403/200 for each protected endpoint, `/health`
returned 200, and the recorder reported no persistence degradation.

Cleanup passed for the recorder, clipboard, exact-path browser process tree,
loopback fixture server, focus release, native fixture and final process
quiescence. A separate post-run inventory found zero scoped processes. Network
inspection found no listening or established TCP connections and no UDP endpoint
for the scoped paths; 45 TCP entries were only `TIME_WAIT`. The exact Chrome
outbound block rule was active with the reviewed non-loopback IPv4/IPv6 scope.
The batch made no deliberate browser-originated outbound attempt and collected no
packet-drop evidence (`outbound_attempt_samples` was zero), so this result must
not be described as proof that Chrome was offline. Only aggregate counts for
fixed synthetic markers were inspected; captured contents were not read.
