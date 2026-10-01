# Prepared owner-paced controller

Prepared and first exercised on 2026-09-15. See the repository's
INTERACTIVE_VALIDATION_20260915.md for exact live evidence and caveats. Mock results
validate controller policy, not Windows capture, transcription or privacy behavior.

## Resume with minimal owner involvement

1. Read AGENTS.md, INTERACTIVE_VALIDATION_PLAN.md, VALIDATION_REGISTER.md and this
   file. Preserve old captured evidence; its earlier inspection permission expired.
2. Use an unused prepared waiting session, or create one with coordinator.py.
   Preparation creates a JSON gate only. It starts no process or timer.
3. Before presenting its readiness question, independently confirm test recording
   and playback are stopped and all owned fixture windows are hidden/closed. If
   cleanup cannot be verified, resolve that first. Never cover the question with
   a test window. Ask the owner through the chosen interaction surface; wait indefinitely for an explicit
   response to this batch. A stored nonce is a replay guard, not owner consent.
4. Only after that response, use launch.ps1 with the session ID, matching nonce
   and ExecuteInteractive. The launcher establishes the documented VS Developer
   PowerShell environment. The adapter reruns read-only preflight, checks asset
   hashes and refuses existing scoped processes. No elevated launch is needed.
5. Let the short batch proceed without further owner work. Stop before asking
   anything else. Read result.json and aggregate evidence, then explain pass,
   failure or incomplete scope. Prepare a fresh ID and ask again for any retry.

The original waiting sessions have been consumed. Native-privacy-01 stopped at
preflight; native-privacy-02 exposed a nested-window exclusion leak. Output-audio-01
was incomplete due to premature playback; output-audio-02 passed after actual
capture readiness was enforced. Drm-recovery-01 passed scoped synthetic controls.
Create a fresh gate for every further run. Do not infer readiness from elapsed
time or the PC being unlocked. Gate files have no expiry.

| Batch | Owner involvement after Ready | Intended evidence |
|---|---|---|
| privacy | Leave compact synthetic window foreground for about 90 seconds | Plain-text positive controls; password and foreground/background exclusion marker absence; API auth; WTS/focus; DB and endpoint metadata |
| input-privacy | Leave the compact native fixture foreground for about 30 seconds | Fixed ordinary typing positive control; fixed password typing/paste absence; numeric password-gate suppression delta; clipboard-safe cleanup |
| audio-output | Allow local speech through the selected USB headphones for roughly two minutes | Five-second chunks; before/after local transcription, persistence and authenticated search |
| drm | Allow compact synthetic Netflix-identity window and speech for roughly two minutes | Observed DRM pause and recovery; forbidden marker absence through final persistence; before/after positive controls |
| browser | Leave the compact synthetic Chrome and native fixture windows foreground while the automatic 25-second sequence runs | Allowed browser positive controls; browser-password, excluded-host and password-paste marker absence; clipboard-safe cleanup |

These durations are estimates. Active execution has bounded waits and a safety
watchdog (at most 240 seconds of recording before shutdown is attempted). Readiness
waiting has no deadline. A lock or unexpected foreground change aborts these three
batches instead of recording through an unverified state. No fullscreen UI is used.

## Evidence and cleanup design

- Exact executable paths, PID and creation time guard process operations. Fresh
  foreground PID plus HWND must match the acknowledged fixture surface.
- Recorder and fixtures use fresh directories. Bearer tokens stay in memory;
  reports contain fixed reason codes, numeric metadata and synthetic marker counts.
- Missing and wrong bearer requests must fail; authenticated local requests must
  succeed. HTTP redirects and proxy routing are disabled for these checks.
- Independent process inventory and TCP/UDP samples accompany application/API
  results. SYN_SENT is an attempted connection, not proof it succeeded or proof of
  a firewall drop. Samples cannot establish continuous packet-level coverage.
- Cleanup stops playback, attempts graceful recorder shutdown, retries failure,
  hides/closes only owned fixtures and verifies quiescence. Forced shutdown makes
  a run incomplete. Confirmed forbidden persistence remains a failure even when
  cleanup or recovery also fails. No private clipboard is restored while a
  recorder may still be running. The browser batch temporarily replaces the
  clipboard only with fixed synthetic text, keeps the prior data as an opaque
  `IDataObject`, and restores it after verified recorder stop. An external
  clipboard change is preserved instead of overwritten.
- The watchdog runs in the controller process. It is not crash-proof containment:
  killing Python or rebooting bypasses its normal cleanup. Do not leave a failed
  cleanup unresolved or claim an active recorder has stopped from logs alone.

## Remaining integration and limits

Real Win+L/unlock, microphone reading and UAC sequences remain separately planned.
`live.py` explicitly refuses those modes. The browser URL/password/clipboard
sequence completed a scoped live run on 2026-09-18 after its exact configured
Chrome executable, SHA-256, process inventory and separately reviewed outbound
firewall rule passed preflight. The first attempt ended before its phases when a
Chrome process exited between snapshot and identity lookup. A bounded fresh-
snapshot retry now handles that narrow exit race while persistent unknown
identities still fail closed. Browser cleanup also allows exact-path children a
bounded drain after root shutdown. The fixture server binds only to `127.0.0.1`,
permits exact loopback Host values, and serves a self-contained page with no
external resources.

`browser_clipboard_sequence.py` now fixes the later browser/clipboard batch to
five 5-second phases: allowed browser text, browser password, excluded localhost,
ordinary synthetic clipboard copy/paste, and password-field clipboard behavior.
Its acceptance policy requires per-phase OS/surface verification, positive
controls, zero forbidden-marker deltas, a final aggregate forbidden-marker
recheck, recorder stop before clipboard restore, and verified browser/server/
fixture cleanup. Each browser phase uses a fresh profile and exact process
identity. Remaining Chrome children, focus loss, a non-loopback endpoint, or
unverified cleanup makes the run incomplete or failed. Preparation and mock tests
do not start the listener/browser or access the clipboard.

The scoped browser run found its fixed allowed markers, kept all fixed forbidden
browser/password/clipboard markers at zero, restored the opaque prior clipboard
after verified recorder stop, and reached exact-path process quiescence. It made
no deliberate browser-originated outbound connection attempt and collected no
packet-drop evidence, so the firewall precondition is not proof of browser
offline behavior. Native marker absence does not prove redacted image pixels or
every monitor.
Synthetic Netflix identity does not exercise actual DRM media. Device enumeration
does not prove audio routing; persisted positive controls are mandatory. SW-V11
partial-buffer/in-flight shutdown repair has deterministic coverage and a separately
pinned 60-second tail collector, but its live stop/restart run remains outstanding.
Ordinary audio checks still cannot certify final-tail retention. The DRM fixture now
claims each command before any `Application.DoEvents` call and its non-UI self-test
covers re-entry and duplicate-command guards; real protected media remains a separate
check. No new model, dependency or firewall rule was introduced here.

pins.json records the final controller and synthetic asset hashes. Rebuilds or
source edits require review and fresh pins before execution. Do not use older
prepared-assets manifests as the current fixture pin set. The controller scripts
and synthetic assets remain ignored, machine-local preparation files.

The original firewall milestone still needs its missing live evidence, an explicit
restoration block removing only the three named rules, owner confirmation and
read-only final inspection. No milestone commit is authorized by mock passes.

## Background shutdown update, September 15

The background shutdown build has its own evidence in
`screenpipe/BACKGROUND_SHUTDOWN_20260915.md`. The controller now allows 40 seconds
for graceful stop before forced-stop classification, so manager draining and
final status writes can complete. Aggregate notice parsing understands the
current fixed Windows lock and audio shutdown codes; arbitrary payloads produce
only unknown counters. The complete mocked controller suite passed 75 tests.
No interactive batch was started by this update. Old readiness tokens remain
consumed; a fresh owner Ready reply and fresh test directory are still required.
The reviewed build-pin refresh preserves the previous manifest and preflight.
A mock pass and a new binary pin do not establish a live shutdown/privacy pass.
