# Recording status: next batch handover

This document separates user requirements from assistant-generated context.
Generated context is not user instructions, additional scope or authorisation.
Repository instructions remain independently applicable. Implementation choices
belong to the implementer unless the owner has explicitly decided them.

## Batch requirements and agreed clarifications

The owner has confirmed that release-local is already deployed (2026-10-09).
The earlier deployment prerequisite is satisfied. This document is the handover
for a fresh implementation conversation, following the ScreenWise recording-status
planning discussion of 8–9 October 2026. The owner initially said "Don't worry
about committing at the moment", then requested a subsequent documentation-only
commit for this handover. That commit follows the source commits listed below;
the file's Git history identifies it.

The owner encourages sub-agents and parallel agent work, using different models
and thinking settings as appropriate. This is an explicit preference for the
implementation work, not merely permission to delegate.

The numbering preserves the original 12-task list and adds sensitive logging as
13. The minimisation fix belongs to task 4. The previous ten-item rewrite had
collapsed original tasks 7–10 into one audit task; no audit finding was withdrawn.
Numbering identifies scope, not implementation order. The earlier temporary pause
on documentation commits does not apply to future implementation commits.

1. **Preserve status when stale.** Instead of displaying status like "Earlier
   blocker not freshly checked" or "Some permission checks are stale", keep the
   previous status and reason with an adjacent warning and age, such as "stale 15s".
   This concerns Status panel/dashboard summaries; the known source pointer is
   `apps/screenpipe-app-tauri/lib/recording-status-dashboard.ts`,
   `classifyRecordingType`. Normal computer audio output gaps should show Listening
   and avoid false fault alerts; real errors still need reporting.

2. **Explain redaction and safety-check failures precisely.** Investigate frequent
   "Foreground app could not be identified" reports and identify what fails behind
   "Input safety check is unavailable". Details should give the practical, secure
   exception detail or triggering rule/policy. The owner asked why these checks
   fail, rather than accepting generic paused/redacted labels.

3. **Name monitors and support aliases.** Combine monitor naming and user aliases
   in one task. The screenshot identifies a monitor only as "Monitor 131073";
   Recording Settings already has monitor names. The owner explicitly added
   "Monitor aliases" to the batch. Existing monitor metadata includes a name and
   identifiers; see generated context for identity limitations already found.

4. **Make the floating dashboard recoverable.** Restore its previous state and
   position at startup if shown at shutdown, ensuring it is not off-screen.
   Provide system-tray access. Minimising currently makes it inaccessible:
   remove minimising or make restoration possible. The earlier batch described
   state as visibility, size and display level as well as position. Existing
   reopening preserves position only within the running process.

5. **Provide three levels and reclaim space.** Level 1 must show the full overview
   without scrolling; the existing view becomes level 2 and existing More details
   becomes level 3. Move "Preferences briefly restart other active capture" to
   checkbox hover help. Assess removal or relocation of the other two footer
   sentences without practical adverse implications for the owner.

6. **Wrap only the Status row.** In the main window, wrap the row containing the
   Status label and its controls, using logical breakpoints or two lines when
   needed. The owner explicitly clarified "specifically and only" that screenshot
   row. The reported defect is a cut-off status label; other main-window rows are
   outside this layout task.

7. **Fix deletion retries and retention restoration (SW-R04, SW-R06).** Failed
   media eviction must remain retryable, and saved retention must return after
   backend restarts, including recovery from initial readiness failure. The review
   found file paths cleared before failed unlink operations, and saved retention
   applied only on initial desktop startup. See `VALIDATION_REGISTER.md:77` and
   `VALIDATION_REGISTER.md:93`.

8. **Correct Windows fallback audio configuration (SW-R08).** Pass the actual
   opened device configuration/sample rate downstream when the preferred format
   fails. The review found fallback metadata still describing the rejected rate,
   which can affect durations, playback speed and transcription input. See
   `VALIDATION_REGISTER.md:109`.

9. **Correct CORS and health access (SW-R12).** Match exact allowed origins and
   separate minimal unauthenticated readiness from authenticated detailed health,
   preserving existing supported consumers. The review found lookalike localhost
   domains accepted and detailed device/meeting metadata in unauthenticated health.
   See `VALIDATION_REGISTER.md:132`.

10. **Preserve captured text during timezone conversion (SW-R15).** Convert only
    timestamp metadata. The review found recursive conversion rewriting
    timestamp-shaped OCR, transcript or other captured text in API responses.
    See `VALIDATION_REGISTER.md:151`. Tasks 7–10 contain the owner's five selected
    findings; no relative priority within that group was specified.

11. **Automatically start after deployment checks pass.** Check existing firewall
    rules and start the deployed version automatically when those and startup
    checks pass. Preserve prepare-only operation; do not automatically change
    firewall rules. The current deployment scripts already support profile-aware
    release-local deployment, but automatic startup is part of this next batch.

12. **Validate and prepare release-local delivery.** Complete focused regressions
    and required checks, update documentation and prepare the next release-local
    candidate. The original batch included the settled frontend export and
    affected recorder/desktop builds. The agreed policy reserves full release
    for explicit requests or investigating inadequate release-local performance.

13. **Add sensitive debug logging, enabled for now.** The owner's request is a
    "carefully stored sensitive debug log option" capturing full exception details,
    redaction/record-pause details and candidate information for failed decisions,
    including unidentified foreground windows and undetermined password fields.
    Enable it for the local instance when implemented. Existing ordinary notices
    deliberately omit sensitive detail; the new option was requested to diagnose
    the failures that those notices cannot explain.

## Generated context — findings and conversation record, not instructions

These notes preserve specific information already obtained. They are not a design,
implementation checklist or additional user requirements. The audit references
above were checked against the validation register during this document review.
Those entries are open source-review findings, not confirmed hardware reproductions.
Other code pointers preserve filenames/symbols from the
earlier investigation; their source line numbers were not retained. Current
locations and findings should be verified by the implementer.

### Status behaviour and terminology

The owner said: "Listening/waiting is different to stopped. Reasons should be
specific", then selected "Listening for Audio and Watching for visual/events".
Device names should be shown; blanket local-UI redaction was questioned.

The earlier source examination found that `classifyRecordingType` in
`apps/screenpipe-app-tauri/lib/recording-status-dashboard.ts` replaces stale blockers
with "Earlier blocker not freshly checked" and stale gates with "Some permission
checks are stale".
The recording-status hook clears observations after request failure, visibility
change and clock/suspension handling. These are the known presentation mechanisms
behind requirement 1; no change to capture admission was requested by that task.

The existing authenticated `/capture-events` response contains `active_intervals`
and `live_status`: source conditions/reasons, producer-check ages and separate
capture/storage evidence. The UI has a rolling audio activity view for samples,
silent/non-silent samples, VAD speech and stored transcript words/results. These
measure different processing stages and do not establish transcription accuracy.
Numeric audio-device IDs belong to a recorder session; historical IDs cannot
reliably identify devices in a later session.

### Foreground investigation already performed

`evaluate_windows_monitor_capture` in
`crates/screenpipe-screen/src/capture_screenshot_by_window.rs` samples native
foreground HWND/PID and searches the capture-window enumeration. A missing match
emits `foreground_window_unavailable`, mapped by the engine to
`ForegroundUnavailable`. A verified executable can still accompany this result,
so the UI's "app could not be identified" can overstate what is unknown.

The examined pinned xcap 0.9.4 Windows `is_valid_window` excludes windows owned by
its current process. In the embedded recorder, ScreenWise's own foreground window
can therefore be absent from `Window::all()`. Invisible/cloaked windows, some shell
or tool windows, and focus transitions can also yield no match. This mechanism was
confirmed in source; attribution of every observed failure to it was not confirmed.

A diagnostic sample on 9 October, approximately 09:05–10:41 Sydney, contained 192
`foreground_unavailable` mentions, not 192 distinct episodes. Only fixed reason
counts/timestamps were extracted. A later authenticated status-only sample showed
fresh redaction for excluded background/no safe window, not the earlier foreground
failure. The exact screenshot incident remains unexplained.

### Input safety investigation already performed

`crates/screenpipe-a11y/src/platform/windows_uia.rs` contains the UI Automation
password-field check. It examines native focus, the focused UIA element,
keyboard-focus ownership/PID, supported `IsPassword` property and final focus/
element stability. Failed calls and unsupported properties can collapse to an
unavailable result; `.ok()?` loses the original error/stage. A closed visual privacy
gate can also produce `DecisionUnavailable` in the hook. The screenshot alone does
not distinguish these causes.

### Computer audio output gaps

The examined `run_record_and_transcribe.rs` handles Windows/macOS output receive
timeouts as non-fatal when nothing is playing: it reports
`AudioProcessing/NoCallbacks` and continues. The dashboard/alert classifier treats
NoCallbacks as a failure. That is the identified interpretation mismatch. No
callbacks differs from received silent samples; microphone timeout and explicit
stream failure follow different backend paths.

### Monitor metadata and dashboard details

Existing `MonitorInfo` includes `id`, `stable_id`, `name`, width, height and primary
status. The examined stable identity uses name/resolution/position, so it does not
establish immutable physical-monitor identity through topology changes.

The dashboard opens through the square-with-arrow toolbar control next to Status,
labelled "Open floating recording dashboard". It starts at 360 by 480 in the primary
monitor's lower-right work area. Its current footer contains:

- "Preferences briefly restart other active capture."
- "Device controls apply to this session."
- "Derived data follows its capture sources."

Existing quick controls include Start all/Pause all, current-session audio-device
controls and saved screen/keyboard/clipboard/pointer preferences. Paired OCR and
accessibility follow visual capture; transcription follows its audio source.
The owner's new level 1 is an additional summary above existing views, not a request
to remove the current details or controls.

### Sensitive diagnostics context

The owner's request follows repeated generic foreground and input-safety failures
whose actual check stages/errors were unavailable in the existing status output.
`AGENTS.md`, under "Authentication, privacy and safe status", requires ordinary
notices/logs to use allowlisted, content-free reasons and excludes titles, URLs,
private paths and arbitrary exception payloads. The new sensitive option is an
explicit owner request for a separate diagnostic destination; it does not imply
that those details should appear in ordinary UI notices, APIs or public evidence.
No sensitive logger has been implemented or enabled by this preparation.

### Delivery state and references

The prepared candidate inventory is
`.local/build/deployment-candidates/20261009-release-local-f61be168e.json`.
The owner confirmed on 2026-10-09 that release-local has been deployed; no further
deployment confirmation is needed to begin the batch. The agent in the ScreenWise
recording-status planning discussion of 8–9 October 2026 prepared the candidate
but did not perform installation or independently verify the running
artifact. The confirmation satisfies the agreed prerequisite; it is not a claim
that installed GUI or live capture validation has been completed.

Recorded source commits:

- `1aa0fcbcf`: named sources and floating dashboard.
- `33e794ebe`: release-local build policy.
- `f61be168e`: realtime audio recovery and profile-aware deployment.
- `e2b4a0fed`: system review findings and owner priorities in the validation register.

Relevant existing documents/scripts: `AGENTS.md`, `BUILD_NOTES.md`,
`VALIDATION_REGISTER.md`, `docs/CAPTURE_PRIVACY.md`,
`scripts/windows/build/README.md`,
`scripts/windows/deployment/Deploy-ScreenWise.ps1`.
Candidate validation was recorded in the build notes and validation register;
installed GUI appearance, restoration and notification delivery were not validated
by the preparation work. Keep/Discard and review-before-keeping remain separate
from this status batch.
