# ScreenWise validation and issue register

Updated 2026-10-02. **Partial validation, not a completed privacy/firewall certification.**
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
| Local transcription | Microphone and system-output capture, Parakeet transcription, known-phrase search and transcript persistence across restart passed; a fresh current-build partial-buffer run retained its fixed final-tail marker and found it after process restart | Not a full-passage accuracy, diarization-quality, every-device or same-process restart benchmark |
| Native privacy | Synthetic password markers absent in inspected text/log fields; foreground/background title exclusions and other-monitor fixture controls passed | Browser/custom UIA controls and race boundaries remain open |
| Input switches | Disabled keyboard/clipboard switches produced no input-content rows during synthetic stimuli | Screen/UIA/OCR may still see ordinary displayed text |
| Audio shutdown | A fresh 13.87 s selected-output partial chunk retained both fixed markers; authenticated search found them after a distinct process restart and found a separate control in a new chunk; both shutdowns were clean | Active-meeting shutdown, same-process restart and every device are not covered |
| Lock/unlock | Audio-disabled real transition passed; a later audio-enabled day-to-day run recorded one 10,770.1 s privacy pause with zero frame or audio-chunk counter advances inside its sampled locked interval, then automatic WGC/audio recovery | Counters and fixed notices were inspected, not captured content; this is not every-pixel proof |
| Safe lock notices | Fixed unlocked/desktop-unavailable/locked reasons persisted through timeline API and local logs; clean stop with no owned processes remaining | Packaged desktop timeline was not visually exercised in this run |
| Synthetic pixel redaction | A fresh gated live route check persisted fixed synthetic OCR geometry, changed 100% of the PII ROI while retaining 2.78% of its contrast, preserved ordinary/sentinel controls, returned a source-free missing-OCR placeholder, enforced 403/403/200 bearer behavior and cleaned up | Fixed supplied geometry only; OCR recognition, real-screen content, general PII accuracy and optional RF-DETR detection remain unverified |
| Synthetic DRM | Fresh gated run observed 28 protected-phase samples, retained before/after transcription controls and kept the protected audio/capture markers at zero through recovery; authentication, SQLite and cleanup passed | Synthetic fixture identity and aggregate markers only; real protected media and provider diversity remain untested |
| OS firewall | Exact recorder/media paths had outbound block rules; the fixed no-payload `screenpipe.exe` IPv4 TCP diagnostic was denied while an unscoped control handshake succeeded; helper IPv4/IPv6 localhost access worked | External IPv6 route unavailable; UDP/drop-trace coverage and closeout incomplete |
| Published checkout | Clean offline release build, CLI help/doctor and nine event tests passed; fresh gated runs wrote 15 distinct snapshots, three accessibility-tree frames and Windows Native OCR text, persisted and found both selected-USB-output audio controls, passed scoped browser and native password/exclusion checks, and passed the fixed-geometry authenticated pixel route; SQLite checks and main capture/audio shutdowns passed | OCR-recognized live pixel redaction, arbitrary browser/clipboard providers, real DRM, full audio accuracy and every-device behavior were not exercised; the separate vision-disabled OCR restart required forced shutdown after its watched-process signal |

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

## Long manual day-to-day trial, 2026-09-23 to 2026-09-24

A private 8,581-second run produced 277 content-free operational samples. The
audit counted 4,405 captured and database-written frames, 521 UI events and 285
audio chunks. Five chunks passed VAD, nine current-process transcriptions were
completed and inserted, and 205 durable untranscribed segments were reconciled
over 44 sweeps. The final pre-shutdown sample still reported 29 pending segments.
One 798.2-second sampled lock interval had zero frame and audio counter advances.
All 277 API samples were reachable and authenticated. No captured pixels, text,
audio, transcripts, input values, titles, URLs or device names were inspected.

Shutdown did not complete. The log recorded an audio consumer drain timeout and
four unconfirmed recording deliveries; the recorder returned an error instead of
writing its clean-shutdown marker. The old audit used only its final pre-shutdown
health sample and incorrectly reported no audio shutdown degradation or possible
loss. The audit now merges fixed post-sample shutdown and delivery notices, and a
synthetic regression reproduces this ordering. The launcher also gives an
interrupted native process a nonzero inferred exit when no fresh clean marker
exists, instead of serializing an ambiguous null.

Source review located the shutdown delay: completion of an audio session ran the
entire durable transcription backlog inline on the ordered raw-audio persistence
consumer. Reconciliation now runs only on the separate recoverable worker and
receives an immediate wake signal. Shutdown bounds and aborts that worker without
misclassifying already durable segments as lost raw audio. The ordered consumer
remains responsible for confirming raw-audio persistence. Deterministic audio
tests cover the wake and bounded-stop paths; a fresh live shutdown with a backlog
is still required.

The run retained six safe acquisition placeholders: five active-window failures
and one initial privacy-evaluation failure. It also recorded 13 frame-link TTL
expiries, split as seven events without frames and six frames without events.
Windows active-window capture now retries one complete privacy-safe acquisition
after a transient acquisition failure, without retrying or weakening a redaction
decision. Capture drops are classified in the linker, and UI batches deliberately
discarded by privacy or database admission now terminally resolve their frame
correlations. New health and audit fields separate known capture drops from
unexplained TTL expiry. Live confirmation remains required.

The log contained 333 fail-closed unavailable-password-state warnings, 44 bounded
audio timing-gap warnings and two bounded health-computation timeouts. UIA still
suppresses keyboard and clipboard content immediately whenever password state is
unknown, but rapid probe-state flaps are now aggregated into bounded warnings.
A separate fixed diagnostic counts actual content events suppressed by the
password-field gate, including known password focus and fail-closed unknown state.
The next live run must confirm the reduced warning rate and the new suppression
counter without examining content.

## Unsigned desktop GUI acceptance, 2026-10-02

An unsigned Windows desktop build ran for 858.1 seconds against a fresh private
store, an authenticated loopback server on a non-default port and five exact-path
outbound firewall rules. A private WebView2 runtime avoided applying the trial
rule to the machine-wide WebView2 installation. Before capture, one fixed
no-payload TCP attempt from the exact GUI executable was blocked while an
unscoped control reached the same IPv4 endpoint. During the run the GUI process
tree had zero observed established non-loopback TCP connections, zero bound
non-loopback UDP endpoints and one IPv4 loopback API listener. The final
read-only inspection found no tested GUI/media/WebView2 process and no endpoint
on the trial port. This is bounded OS endpoint and one-attempt IPv4 evidence,
not packet-drop tracing, routed IPv6 or UDP proof.

The active recorder configuration matched all expected booleans and exclusion
counts: basic and asynchronous text redaction, DRM pause, keyboard and clipboard
capture, event triggers, all monitors and system-default audio were active.
Missing, wrong and valid bearer credentials returned 403/403/200. The final
content-free sample reported 257 captured and database-written frames, zero
drops, stalls, frame-link expiry or update failures, and 163 linked UI events.
A controlled ordinary input marker persisted once, three clipboard events were
present, and 160 UI events had been inserted at the controlled checkpoint. Five
OCR rows existed, but the controlled screen-text marker had zero OCR matches and
the unified elements table had zero UIA/OCR rows. This run therefore passed WGC,
input and SQLite persistence but did not establish current-GUI UIA/screen-text
capture. No arbitrary captured content was inspected.

Both default audio directions started, local Parakeet loaded, and one fixed
spoken phrase passed VAD, completed transcription, was inserted and matched the
allowlisted phrase query. There were zero transcription errors. The run also
included one 89-second lock interval with zero sampled frame or audio advances;
capture and both audio directions recovered automatically after unlock. Shutdown
returned zero, emitted the fixed completion marker and reported clean vision,
UI-event, redaction and audio-worker completion with no confirmed or possible
delivery loss. The last pre-shutdown sample still reported three durable pending
segments, so this run is not a separate final-tail recovery proof.

The packaged timeline first displayed a connection error against the fresh
store, then loaded when retried several minutes later. Logs showed repeated
WebSocket failures using port 3030 while the test server used its configured
non-default port, followed later by rejected duplicated `data/data` media asset
paths and high-volume frame-navigation errors. An empty or still-populating
timeline may also contribute to the first message; cause is not yet established.
Reproduce the startup timing, dynamic-port WebSocket selection and media asset
path/scope independently before changing product behavior.

The process inventory also observed short-lived Windows PowerShell and conhost
descendants. Source review ties these to local installed-application/icon lookup
commands; their commands enumerate files and Appx packages and do not request
network access. They were not firewall-scoped because those shared system paths
would affect unrelated processes. No endpoint was observed for them, but the
sampling cannot exclude every transient packet; strict all-descendant firewall
proof remains incomplete. The first monitor revision also took 131-159 seconds
between API samples because it repeated expensive CIM and endpoint enumeration
30 times. It now uses process-scoped endpoint queries and a 30-second wall-clock
deadline; its PowerShell and synthetic report regressions pass, but live cadence
confirmation remains open.

The next short GUI attempt passed the fixed acquisition checks before its UI was
stopped: 40 WGC frames reached SQLite, the fixed UIA and OCR controls were each
observed twice, bearer checks returned 403/403/200, and the API listener and
sampled endpoints stayed loopback-only. The timeline later became unresponsive.
Source/evidence correlation found that the reused WebView2 profile had loaded
cached media references from an earlier data store; asset-scope rejection then
produced a high-volume error loop. The process tree also contained short-lived
`setx.exe`, `where.exe` and `conhost.exe` helpers. No endpoint was observed for
those helpers. The owner directed a forced stop, so this attempt did not pass
clean shutdown and cannot close GUI acceptance.

The repaired candidate namespaces timeline caches with an opaque digest of the
active canonical data directory, waits for active API configuration before cache
access and uses a fresh WebView2 profile for each trial. It removes startup
`setx` and replaces `where.exe` path lookup with in-process `PATH` enumeration.
The optimized frontend build, TypeScript validation, 408 Vitest tests, 150 Bun
tests, the content-free GUI harness regression, process-monitor self-test, Rust
formatting and a warning-free locked/offline `release-dev` desktop build passed.
The desktop test cache was moved to the ignored repository-level
`target\desktop-tests` directory and stale moved-cache build-script state was
regenerated. The locked/offline Visual Studio environment then completed the
linked build: both occupied/released port tests and all three Windows
opener-classification tests passed. A fresh interactive run remains required; no
recording or firewall change was made for this repaired candidate.

## Issue register

| ID | Current state / next check |
|---|---|
| SW-V01 | Native foreground DRM policy repaired in source; scoped synthetic Chrome allowed/password/excluded-host transitions passed. Sustained real DRM/audio recovery and broader background/other-monitor transitions remain incomplete. No protected playback bypass. |
| SW-V02 | Explicit audio selection repaired; selected-output live regression passed. Broader fresh/persisted microphone-only, defaults, disable-audio and device changes still need coverage. |
| SW-V03 | **Repaired, regression-tested and scoped live-checked:** earlier runs had two unexpected WGC acquisition failures. The 2026-09-23/24 run retained five active-window and one initial-privacy safe placeholder and later recovered. Windows capture now makes one bounded retry only after an acquisition failure; it never retries a redaction result. The 2026-10-01 native privacy run completed with zero acquisition-failure placeholders. This single clean run does not reproduce the former OS failure. |
| SW-V04 | Microphone transcript rows/phrase hits passed, complete passage accuracy and retention did not. Distinguish ASR substitutions, VAD rejection and actual loss. |
| SW-V05 | Earlier DRM audio tests lacked positive before/after controls; they cannot establish suppression. Retry only with working controls. |
| SW-V06 | Scoped live clipboard evidence passed: an ordinary fixed marker produced three positive rows, the password-phase secret marker produced zero rows, three clipboard UI events were present, and opaque clipboard restoration followed verified recorder stop. Arbitrary providers, applications, formats and race timing remain open. |
| SW-V07 | **Repaired, regression-tested and scoped live-checked:** the orphan lower-level UIA tree producer remains disabled while paired UIA capture and password/focus/input checks stay active. Multi-monitor duplicates are deduplicated. The 2026-09-23/24 run produced 13 TTL expiries. Capture drops now classify their linked event halves, and privacy/database-rejected UI batches terminally resolve their linked frames. The 2026-10-01 native privacy run logged zero unexplained frame-link TTL warnings. Real pressure and longer runs remain open. |
| SW-V08 | Fixture focus/equality limitations addressed with OS identity and stimulus acknowledgements; continue using these guards. |
| SW-V09 | Windows locked-state detection repaired and already-locked/audio-disabled transitions passed. A later audio-enabled run showed a roughly three-hour sampled frame/audio plateau and automatic WGC/input/output recovery after unlock. Broader repeated-lock, DRM and device-change recovery remain open. |
| SW-V10 | General event pressure/recovery/subscriber-loss reporting implemented; 28-event-suite tests passed. Real overload/soak remains open; delivery counts are not unique lost database rows. |
| SW-V11 | **Repaired, regression-tested and partially live-checked:** a 2026-09-23/24 shutdown timed out because session completion ran durable-backlog reconciliation inline on the ordered raw-audio consumer, leaving four raw-audio deliveries unconfirmed. Reconciliation now runs only on its separately bounded, explicitly woken worker. The 2026-10-01 selected-output run stopped cleanly on its first attempt with no shutdown degradation or delivery-loss notice, but it did not deliberately force a durable backlog. A live forced-backlog shutdown, pending recovery, active meeting and same-process restart remain required. |
| SW-V12 | Short stimulus/first-chunk mismatch corrected in preparation; bounded later selected-output tests passed. |
| SW-V13 | Deterministic broadcast-lag integration test repaired; all three locked offline Cargo tests passed. |
| SW-V14 | Nested Windows UIA exclusion gap repaired; scoped silent native privacy retest passed. Broader exclusion variants remain open. |
| SW-V15 | Harness capture-worker readiness race repaired; selected-output live retest passed. |
| SW-V16 | Duplicate lock monitors prevented with one process worker and serialized probes; synthetic tests passed. Repeated live desktop server lifecycle remains open. |
| SW-V17 | **Repaired and live-checked:** partial audio buffer splitting/swallowed final-delivery failure passed deterministic tests and a fresh current-build stop/restart run. The final zero-overlap flush produced one 13.87-second chunk containing both fixed markers; both remained searchable after a distinct process restart. |
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
| SW-V29 | **Repaired and regression-tested; live check pending:** the trial summary previously ignored shutdown failures and delivery-loss notices written after the final health sample. It now merges fixed post-sample diagnostics, reports the exact safe reason codes, and treats a missing clean marker plus null native exit status as failure. |
| SW-V30 | **Repaired, regression-tested and full-application live-checked:** UIA password-state probe flapping produced 333 repetitive warnings. Fail-closed suppression remains immediate, while state-unavailable warnings are bounded and aggregated. A real-hook/real-UIA crate regression exercised two fixed ordinary/password cycles: both ordinary markers were delivered, both password markers remained absent, and all 14 characters in each password phase incremented the unavailable-decision suppression count. The corrected full-application run persisted one ordinary keyboard control, zero password typing or password clipboard markers, and 23 safe suppression notices; authentication, SQLite integrity, clipboard restoration and clean process shutdown passed. Arbitrary providers and all race timings remain untested. |
| SW-V31 | **Repaired, regression-tested and scoped live-checked:** active IPC remains authoritative over persisted port hydration; media uses the configured final directory and active recursive asset scope. After an earlier run reused stale media paths and became unresponsive, cache keys gained an opaque active-data-directory namespace, cache loading began waiting for active API configuration and every trial received a fresh WebView2 profile. In the fresh follow-up, the owner loaded Timeline without the prior connection error, the recorder wrote 395/395 captured/database frames, and shutdown was clean. The visual check did not inspect arbitrary captured content or establish every media/cache path. |
| SW-V32 | **Repaired, regression-tested and scoped live-checked:** asynchronous text reconciliation preserves sanitized accessibility structure only for byte-identical output and still clears derivatives fail-closed when stronger redaction changes text. In the short GUI follow-up, both fixed public UIA and OCR controls were observed twice while 40 WGC frames reached SQLite. This establishes the allowlisted fixture on that build, not general OCR recognition or accessibility-provider coverage. |
| SW-V33 | **Repaired, regression-tested and scoped live-checked:** icon discovery no longer launches PowerShell, startup no longer invokes `setx`, and Pi/FFprobe lookup enumerates `PATH` in-process instead of launching `where.exe`. The fresh follow-up process monitor took 5,048 snapshots at 25 ms, classified eight expected WebView descendants and observed zero unexpected shell starts. Sampled non-loopback TCP/UDP endpoints were zero, shutdown was clean and no scoped process remained. This does not establish unsampled packet behavior or a provisioned Pi child-process scope. |
| SW-V34 | **Production hardening repaired, built and regression-tested:** full startup no longer shell-discovers or forcibly terminates a process that owns the configured API port; an in-process loopback bind waits only for the app's prior socket release and otherwise fails with a safe diagnostic. Windows path/URI opening no longer uses `cmd.exe`; it requires an absolute local path or explicitly allowlisted `shell:AppsFolder`/`ms-settings` URI and uses the native opener. Source-level harness regressions, formatting and a warning-free optimized build passed. Both linked occupied/released port tests and all three linked opener-classification tests passed in the locked/offline Visual Studio environment using the repository-local desktop test cache. |
| SW-V35 | **Repaired, regression-tested and optimized-build checked; visual confirmation pending:** capture placeholders previously exposed only generic redaction/failure labels. Windows records now persist one of eight fixed safe markers and Timeline maps only exact allowlisted markers to content-free explanations for active-window-only masking, exclusion/focus policy decisions and acquisition/privacy-check failures. Focused Rust and Vitest coverage and the combined `release-dev` build passed. No window title, URL or captured value is included, and the reason category does not claim that specific displayed content was sensitive. |
| SW-V36 | **Repaired, regression-tested and optimized-build checked; explicit real provisioning pending:** missing Pi produced a short developer error without a runnable recovery command. The error now includes the exact repository provisioner command and active data directory. The provisioner pins the Pi package, uses a fresh staging directory, validates the expected local layout, refuses to overwrite unexpected content and requires an explicit developer-run download. Its synthetic regression, focused desktop Rust message test and the combined `release-dev` build passed. AI-provider provisioning and a real Pi start remain separate. |

## Automated interactive repair validation, 2026-10-01

The exact rebuilt release executable identified in `BUILD_NOTES.md` passed a
fresh 71-check normal-user preflight with the existing exact-path outbound block
rules. Preparation first exposed stale staged controller sources: the preflight
could validate a configured helper hash without verifying the helper file that
the launchers would execute. Preflight now resolves that helper, checks its
SHA-256 before creating an evidence directory, and records the successful pin
check. Source staging tests cover this ordering. After source-only restaging and
local fixture rebuilding, 48 manifest checks, 15 staging tests (one expected
symlink-privilege skip), 112 controller tests and 14 plan tests passed.

A fresh selected-output run wrote six audio chunks and eight transcription rows.
Each of the two fixed local speech markers appeared once in persisted search
results. Missing, wrong and valid bearer credentials produced 403/403/200,
`/health` returned 200, SQLite `quick_check` returned `ok`, and shutdown completed
on the first attempt. Fixed-pattern log inspection found no audio-shutdown
degradation, consumer-drain timeout, delivery-loss/possible-loss, persistence
degradation, acquisition-failure placeholder or frame-link TTL warning. This run
did not force a durable transcription backlog at shutdown and is not a complete
speech-accuracy or final-tail test.

A separate silent native privacy run completed its allowed, password,
excluded-foreground, excluded-background and final allowed phases. Allowed frame
deltas were two before and one after the protected phases; protected phases had
zero allowed-marker deltas, and every forbidden synthetic-marker delta remained
zero. The store held 14 frames and five UI events and passed SQLite
`quick_check`; authentication again produced 403/403/200 and shutdown was clean.
Log inspection found zero acquisition-failure placeholders, unexplained
frame-link TTL warnings, persistence degradation or panic files. One logical
password-state-unavailable warning appeared on two log surfaces. Because the run
was silent and input-free, it did not exercise the keyboard/clipboard
actual-suppression counter, image-pixel redaction, real DRM or arbitrary providers.

A subsequent native input run initially failed safely because the synthetic
fixture's managed `SendInput` structure did not match the 64-bit Windows ABI. No
forbidden marker persisted, cleanup passed, and the controller reported the run
as incomplete. The fixture now includes the native union's larger `MOUSEINPUT`
member and self-tests the expected 40-byte x64 layout. In the corrected fresh run,
one fixed ordinary keyboard marker persisted in `ui_events`, both fixed password
typing and password clipboard marker deltas were zero, and the safe password-
suppression counter increased by 23. Missing, wrong and valid bearer credentials
again returned 403/403/200, SQLite `quick_check` returned `ok`, and recorder stop,
clipboard restoration, fixture closure and exact-process cleanup all passed.

The final-tail controller then forced graceful stop while a selected-output
partial buffer was still below its normal 62-second emission threshold. The
pre-stop checkpoint contained zero chunks. Shutdown produced exactly one
13.87-second chunk, and both fixed markers joined that chunk. A distinct process
reopened the same fresh store, authenticated with the same 403/403/200 matrix,
found both original markers, persisted a separate restart control in chunk 2 and
shut down cleanly. Neither phase used force, reported unresolved workers or
shutdown issues, recovered a device, or crossed a privacy transition. Fixed-
pattern inspection found zero queued-work-discard, unconfirmed-worker, incomplete-
shutdown, persistence-degradation, possible-loss or panic markers. An independent
exact-path check found no tested process or listener after completion; only closed
loopback connections in `TIME_WAIT` remained temporarily under PID 0.

After the earlier audio and native-privacy controllers exited, an independent
normal-account exact-path inventory found no tested recorder, FFmpeg, FFprobe or
fixture process, no TCP or UDP endpoint owned by those paths, and no listener on
either configured test port. Captured screen, audio and transcript bodies were
not inspected; only fixed synthetic-marker counts and content-free diagnostics
were used.

## Outstanding test plan

| ID | Required scope |
|---|---|
| SW-T01 | Extend the passed audio-disabled lock transition to audio cancellation/recovery and stale-generation suppression; retain independent OS observations. |
| SW-T02 | **Scoped complete:** the exact Chrome rule and 74-check preflight passed; fixed allowed browser controls persisted while the browser-password and excluded-host markers remained at zero. Exact-path Chrome inventory and cleanup passed. This does not establish universal browser secrecy or actual firewall packet drops. |
| SW-T03 | **Scoped synthetic run complete:** after an initial pre-recording asset-pin refusal, the rebuilt fixtures were reviewed, repinned and passed the 71-check preflight. The fresh gated rerun observed 28 protected-phase samples, persisted one before and one after speech control, and kept the protected audio marker and all forbidden capture markers at zero through the final recheck. Authentication, SQLite integrity, first-attempt shutdown, fixture cleanup and independent process/endpoint checks passed. This is synthetic fixture evidence; sustained real protected playback remains separate and no DRM bypass is permitted. |
| SW-T04 | App-only and App::Title exclusions, rapid transitions, enumeration failure and race stress. |
| SW-T05 | **Scoped complete:** fixed ordinary clipboard text persisted, the password-phase secret marker remained at zero, recorder stop preceded opaque clipboard restoration, and cleanup passed. Live provider-failure, unknown-focus and non-text-format faults remain untested beyond deterministic checks. |
| SW-T06 | **Scoped complete:** three pinned local speech WAVs and the preferred 60-second collector passed 77 offline tests and a 16-asset pin closure. The fresh gated live run stopped before normal emission, retained its baseline and final-tail markers together in one 13.87-second partial chunk, found both after a distinct process restart, persisted a new restart control and completed both shutdowns cleanly without loss/degradation markers. Active-meeting shutdown, diarization quality and same-process restart remain separate checks. |
| SW-T07 | The long locked run covered combined WGC/UI/input/audio/STT counters, lock recovery and loss diagnostics. OCR output, unplug/replug, monitor changes, active overload and content accuracy still require controlled coverage. |
| SW-T08 | Fixed no-payload `screenpipe.exe` IPv4 TCP attempt passed blocked-versus-unscoped control. Usable external IPv6 and UDP controls plus OS drop tracing remain open. |
| SW-T09 | Desktop/WebView/MCP/optional executable scope: inventory relevant child processes before extending firewall claims. |
| SW-T10 | Owner-confirmed disposition/restoration of exact test rules, independent read-only final inspection and original milestone closeout. |
| SW-T11 | Visual desktop status and broader safe-notice audit across acquisition/audio/input failures and independent queues. |
| SW-T14 | **Scoped fixed-geometry live check complete:** after 140 relevant offline tests and a fresh 71-check normal-user preflight, three synthetic snapshot rows exercised the authenticated on-demand frame route. The clear positive control was byte-identical; 100% of the fixed PII ROI changed and retained 2.78% contrast; ordinary/sentinel changes were 0%/0.04%. A separately verified missing-OCR row produced a source-free placeholder changing 99.20% of the frame. Bearer checks, SQLite integrity, shutdown, listener closure and independent process/endpoint checks passed. Three focus/app/status UI rows were counted but their contents were not inspected. OCR recognition, real-screen privacy, async RF-DETR and general PII accuracy remain open. |
| SW-T12 | Extend the passing deterministic privacy-generation tests with live transition timing and the SW-V20 speaker-identity path. Inspect durable state as well as returned results; do not generalize the fixed persistence paths to universal stale-generation suppression. |
| SW-T13 | **Scoped complete:** diagnostic evidence showed every ordinary denial in the failing full-app run was stale. A narrower real-hook/real-UIA test separated provider-call duration from completed-probe gaps and reproduced tree work delaying the shared STA beyond the 75 ms permit. Password polling now has a dedicated STA and 25 ms target interval; the permit lifetime and fail-closed checks are unchanged. Two automatic ordinary/password cycles delivered 2/2 ordinary markers, 0 password markers and 14 suppressions per password phase, with zero ordinary denials and a 46.8 ms maximum probe gap. The corrected full-app controller then persisted one ordinary UI-event control, zero fixed password markers and 23 suppression notices, with authentication, SQLite integrity, clipboard restoration and clean shutdown passing. Arbitrary controls/providers and all race timings remain outside this scoped result. |

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
