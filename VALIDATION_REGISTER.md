# ScreenWise validation and issue register

Updated 2026-09-16. **Partial validation, not a completed privacy/firewall certification.**
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
| Local transcription | Microphone and system-output capture, Parakeet transcription, known-phrase search and transcript persistence across restart passed | Not a full-passage accuracy or diarization-quality benchmark |
| Native privacy | Synthetic password markers absent in inspected text/log fields; foreground/background title exclusions and other-monitor fixture controls passed | Browser/custom UIA controls and race boundaries remain open |
| Input switches | Disabled keyboard/clipboard switches produced no input-content rows during synthetic stimuli | Screen/UIA/OCR may still see ordinary displayed text |
| Audio shutdown | One 13.798 s selected-output partial chunk retained both synthetic markers; authenticated search found it after a distinct process restart and found a new chunk | Active-meeting shutdown, same-process restart and every device are not covered |
| Lock/unlock | Audio-disabled real transition passed: 11.890 s confirmed locked plateau, frame count 7->7 and UIA count 11->11; synthetic before/after capture/search passed | Included-window gate remained active; not isolated lock-gate or every-pixel proof |
| Safe lock notices | Fixed unlocked/desktop-unavailable/locked reasons persisted through timeline API and local logs; clean stop with no owned processes remaining | Packaged desktop timeline was not visually exercised in this run |
| OS firewall | Exact recorder/media paths had outbound block rules; helper IPv4 probes failed while an unscoped control succeeded; helper IPv4/IPv6 localhost access worked | External IPv6 route unavailable; active recorder/UDP/drop-trace coverage and closeout incomplete |

The final lock result's SHA-256 was
`A82FA50F5DD29402319997CEB75EAD9E70DC6FDC03CF663F86907B2CB1AE0F89`.
Its tested recorder SHA-256 was
`26578A85C37C9D90AC85E84DC4F8BCA9FBB3CA7E46B733361113BEB8182AF0B7`.
These identify historical artifacts, not a claim that a new checkout/build has
been tested. Original incomplete runs were retained. Harness repairs included
fresh search-cache bounds, bounded unknown desktop transitions, serialized
fixture commands, safe exception diagnostics and phase evidence checkpoints.

## Issue register

| ID | Current state / next check |
|---|---|
| SW-V01 | Native foreground DRM policy repaired in source; sustained DRM/audio recovery, background/other-monitor and browser transitions remain incomplete. No protected playback bypass. |
| SW-V02 | Explicit audio selection repaired; selected-output live regression passed. Broader fresh/persisted microphone-only, defaults, disable-audio and device changes still need coverage. |
| SW-V03 | Two unexpected WGC acquisition failures observed. Safe placeholders were retained; possible secure-desktop/focus confounding is not an established cause. Reproduce with controlled OS state. |
| SW-V04 | Microphone transcript rows/phrase hits passed, complete passage accuracy and retention did not. Distinguish ASR substitutions, VAD rejection and actual loss. |
| SW-V05 | Earlier DRM audio tests lacked positive before/after controls; they cannot establish suppression. Retry only with working controls. |
| SW-V06 | Ordinary clipboard-content positive control remains missing. Audit deferred read/focus permits using fake values only; no secret payload logging. |
| SW-V07 | Earlier audio-gap/UIA-drop/pending-status warnings need load and no-speech lifecycle diagnosis. They do not establish a broken microphone. |
| SW-V08 | Fixture focus/equality limitations addressed with OS identity and stimulus acknowledgements; continue using these guards. |
| SW-V09 | Windows locked-state detection repaired and already-locked regression passed; later audio-disabled transition passed. Audio recovery remains open. |
| SW-V10 | General event pressure/recovery/subscriber-loss reporting implemented; 28-event-suite tests passed. Real overload/soak remains open; delivery counts are not unique lost database rows. |
| SW-V11 | Cooperative shutdown and worker/persistence ownership repaired; selected-output partial-tail/process-restart passed. Active meeting and same-process manager restart still need live checks. |
| SW-V12 | Short stimulus/first-chunk mismatch corrected in preparation; bounded later selected-output tests passed. |
| SW-V13 | Deterministic broadcast-lag integration test repaired; all three locked offline Cargo tests passed. |
| SW-V14 | Nested Windows UIA exclusion gap repaired; scoped silent native privacy retest passed. Broader exclusion variants remain open. |
| SW-V15 | Harness capture-worker readiness race repaired; selected-output live retest passed. |
| SW-V16 | Duplicate lock monitors prevented with one process worker and serialized probes; synthetic tests passed. Repeated live desktop server lifecycle remains open. |
| SW-V17 | Partial audio buffer splitting/swallowed final-delivery failure repaired; synthetic tests and one complete live partial chunk passed. |
| SW-V18 | **Open:** internal audio queues lack complete near-capacity/loss timeline reporting. General event-bus diagnostics do not cover meeting-tap lag/per-device drops or count all potentially discarded finals. |
| SW-V19 | **Open, source-review finding:** audio privacy permits are checked around asynchronous persistence, but a lock/DRM/schedule transition during a database write can leave an already admitted audio chunk, transcript or meeting segment stored. A later stale-permit check does not roll back the write. Atomic admission/persistence or verified compensating cleanup and deterministic transition-race tests are needed; no claim that queued audio is completely discarded on a privacy transition. |

## Outstanding test plan

| ID | Required scope |
|---|---|
| SW-T01 | Extend the passed audio-disabled lock transition to audio cancellation/recovery and stale-generation suppression; retain independent OS observations. |
| SW-T02 | Browser URL/domain exclusions and HTML password controls, with local synthetic pages and positive controls. |
| SW-T03 | Sustained real/synthetic DRM distinction and audio recovery; repair the DRM fixture's similar command re-entry pattern before reuse. |
| SW-T04 | App-only and App::Title exclusions, rapid transitions, enumeration failure and race stress. |
| SW-T05 | Ordinary clipboard positive control, password/unknown-focus filtering, provider failure and clipboard restoration/fault recovery. |
| SW-T06 | Audio/meeting completeness, diarization quality, active-meeting shutdown, pending recovery and same-process restart. |
| SW-T07 | Combined WGC/UIA/OCR+audio/STT soak, unplug/replug, monitor changes, backlog and loss diagnostics. |
| SW-T08 | Recorder-originated safe outbound attempt, usable external IPv6 and UDP controls, OS drop evidence. Sampling alone is insufficient. |
| SW-T09 | Desktop/WebView/MCP/optional executable scope: inventory relevant child processes before extending firewall claims. |
| SW-T10 | Owner-confirmed disposition/restoration of exact test rules, independent read-only final inspection and original milestone closeout. |
| SW-T11 | Visual desktop status and broader safe-notice audit across acquisition/audio/input failures and independent queues. |
| SW-T12 | Deterministic privacy-generation changes during audio file/chunk, transcript and meeting-segment persistence; verify durable data as well as returned results. Address SW-V19 before claiming stale-generation suppression. |

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
