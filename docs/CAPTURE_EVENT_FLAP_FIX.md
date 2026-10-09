# Capture-state flapping repair and hand-off (2026-10-09)

## Cause and scope

The diagnostic store already deduplicated identical observations, but audio
classified each buffer independently as silent/open. Mixed buffer sequences
therefore represented every classification change as a new interval. Empty
buffers also incorrectly counted as non-silent, reopening admission and
refreshing the usable-audio watchdog without a real sample.

Keyboard/clipboard shared another collision: the worker emitted an admitted
probe, then replayed accumulated rejected-input counts into the same gate as
suppressed. Counters described previous rejected delivery; they were not a new
privacy decision. The worker now consumes rejection counts once per poll using
separate diagnostic cursors and emits one combined observation for that poll.
Periodic numeric log summaries cannot replay those counts into gate state.

History uses a monotonic, one-second dwell for microphone/output AudioProcessing
silence changes and InputPrivacy keyboard/clipboard recovery. The first input
rejection is immediate. Changes among transient input rejection reasons need a
second of sustained observations; password and concrete probe failures remain
immediate. This spans many audio buffers/UIA polls without hiding prolonged
silence or failed checks. Initial state is immediate. Live observations,
heartbeats, sample measurement, privacy permits, VAD, storage and capture
admission remain immediate. History records confirmation time in observed_at_ms
and the first candidate observation in since_ms; previous_since_ms refers to
the previous published interval. No new metrics contain captured content.

Only these source/channel keys are filtered. Stops, session/device recovery,
failed captures, no-callbacks, redactions, lock/unlock notices, user preferences,
transcription deferrals, monitor operation and persistence retain immediate
transitions. Pointer redaction describes actual per-element decisions and is
left immediate. The `/capture-events` response shape and existing reason/state
values are unchanged. No dependency, lockfile or model changes are involved.

## Accessibility status correction

The optional input worker's background tree reader is intentionally disabled
when the paired screenshot/accessibility pipeline owns UIA. Its internal
`capture_tree=false` flag incorrectly published Accessibility/UserPreference/
Global as Disabled, overwriting the real session setting. It now publishes no
feature preference. If that optional reader is enabled, its activity is reported
as CaptureOperation, separately from the session's actual UserPreference.
Settings → Recording → Screen recording controls the paired visual pipeline;
its description now mentions accessibility and OCR. No capture setting was
automatically enabled or changed by this repair.

## Historical cleanup explicitly authorised later by the owner

The later instruction to delete the flood superseded the initial no-deletion
constraint for 2026-10-08T22:03:57Z–2026-10-09T01:02:17Z only. After five
deterministic maintenance regressions and a read-only plan, the live transaction
removed 18,500 narrowly allowlisted rows: 18,132 microphone/device1 AudioProcessing
silent/open notices and 184 each keyboard/clipboard InputPrivacy notices.
It retained 1,024 original notices representing initial/sustained transitions
and all 2,078 non-target notices. All 3,102 surviving notices were compared
byte-for-byte under the write transaction. A subsequent authenticated API query
returned those 3,102 notices over four pages, with no remaining page.

Preserved state totals include 372 capture_stopped, seven recording_degraded,
26 capture_no_callbacks, 478 capture_redacted and two unlocked. Exact original
IDs/payloads and timestamps of those notices remain unchanged. The private
archive was flushed before deletion; its hash and the survivor hash are recorded
in an ignored receipt. The archive contains removed notice fields, not a full
database snapshot or a ready-made restoration of all columns/FTS. Recorded
screen, input and audio contents were not queried; no vacuum or broad range
deletion was run.

This is pruning original historical notices, not a lossless reconstruction of
debounced capture. Historical transition gaps cannot prove continuous callback
or check availability. Retained rows keep original since_ms/previous_since_ms,
which may reference removed transition times. No invented replacement events or
retrospective sample/permit claims were written. Other periods remain unchanged.

## Expected post-deployment checks

The owner subsequently authorised cleanup of other proven floods. The same
strict policy and separate preview/archive/transaction checks removed a further
14,549 rows from 2026-10-09T01:02:17Z through 05:15:00Z in three bounded ranges.
All 1,506 retained target transitions and 2,884 other notices in those ranges
were verified unchanged. Total removal across all four ranges is 33,049.
Older non-flood history, unknown shapes, uncertain privacy/focus alternations,
the small output-audio dataset remained untouched by those four ranges.
Exact periods, counts and limits are in [the hand-off](CAPTURE_FIX_HANDOFF.md#historical-cleanup).

After owner-confirmed deployment, the final authorised one-off range ended at
the verified hash-matching desktop process start, 2026-10-09T06:09:27.871258Z.
Removed another 3,268 flood rows; preserved 426 target transitions and 651 other
notices unchanged. Five ranges total 36,317 removed and 8,569 retained. All
events at or after that conservative cutoff, including post-fix API verification
evidence, remain untouched. Claude Code verifies via the API only; implementation,
builds and authorised cleanup are handled by this task.

| Gate / channel / source | State / reason_code | Expected frequency |
| --- | --- | --- |
| Mic or output samples / audio_processing | capture_silent_input / capture_silent_input | Initial silence, then once per sustained silent interval; enter after one second if already active. Brief buffer bursts do not generate repeated recovery pairs. |
| Mic or output samples / audio_processing | capture_gate_cleared / capture_gate_open | Initial real input, or once after one second of sustained recovery from silence. Empty buffers cannot produce it. |
| Keyboard and clipboard / input_privacy | capture_suppressed / capture_input_check_unavailable, capture_input_check_stale, capture_input_generation_changed, capture_input_focus_changed, capture_input_worker_contended | First rejected interval immediately; sustained cause changes once after one second. Repeated per-poll rejections/reopens do not alternate history rows. |
| Keyboard and clipboard / input_privacy | capture_gate_cleared / capture_gate_open | Initial healthy state, then once per sustained one-second recovery. Actual admission is not delayed. |
| Keyboard and clipboard / input_privacy | capture_suppressed / capture_password_field or specific probe failure | Immediate real transition; unchanged state deduplicates. |
| Accessibility / user_preference | capture_gate_cleared / capture_gate_open (Screen recording on); capture_suppressed / capture_disabled (off) | Session configuration snapshot/change, not an internal background-worker poll. Other gates may still pause capture. |
| Genuine availability/operation gates | capture_stopped, recording_degraded, capture_no_callbacks, capture_redacted, unlocked and their existing reasons | Immediate genuine changes, unchanged observations deduplicated; not subject to noisy-gate dwell. |

A quiet ten-minute window should have a handful of mic transitions rather than
hundreds. Actual sustained signal or privacy changes can legitimately produce
more; this is not a global event-rate cap. Clancy deploys the final candidate;
the separate client performs the post-deploy query. This task runs no production
soak or timed multi-minute capture.

## 11:30 AEDT increase: evidence and limits

The best-supported installed candidate before the increase was
`f61be168e95c910ca350a27c8806ae16fbf4ca50`, release-local. Its preserved GUI hash
was `40568A99DBD9AFE5DBF18BFDD9874C7E0E588AFE5C76069958292A20F2E245EB`.
The candidate was prepared at 10:45:15 AEDT and its preserved deployment copy
created at 10:53:35. Owner-confirmed deployment documentation supports that
candidate; a copy timestamp does not prove the exact running process at 11:30.
No canonical build/deployment evidence identifies a new build at 11:30. An
unrecorded restart or other runtime activity cannot be excluded.

Per-buffer classification events originated in `3ea64c8c7` on 8 October at
17:38:03 AEDT. `5db08f058` at 01:01:49 on 9 October added activity-duration
measurement using the existing classifier, without changing its diagnostic
branches. `f61be168e` changed reconciliation/storage behaviour, not that
classifier or its event emission. The mechanism predates the increase. Historical
events prove the flood but do not establish why alternation became more frequent
around 11:30; deployment causation remains unconfirmed.

## Validation and remaining packaging

Config's 59 unit tests passed, including simulated buffer/probe timing, device
isolation, monotonic versus wall-clock time and genuine-signal bypass. The settled
accessibility suite passed 191 tests; 23 live tests were left ignored. Sixteen
audio producer/recovery/shutdown tests passed serially, including empty-buffer
classification. The first parallel audio run failed due to shared global privacy
state; serial execution passed without source changes. Five cleanup regressions
passed. Existing unused press_key/sample_rate warnings remain.

The status UI passed 106 unique focused checks and TypeScript/export validation.
Independent read-only review found no blocking issue; it identified the cleanup
history/restore limitations documented above. An intermediate release-local
RootBuild passed, reusing all 785 external artifacts with none rebuilt. The final
candidate also includes the separately authorised maximised-window implementation.
The screen suite passed 131 tests, including five new selection regressions;
45 engine capture tests passed, including exact screenshot/UIA ownership and
selected metadata, and 13 notice/API persistence tests passed. Two synthetic
pixel evaluator checks passed, with the interactive acquisition left ignored.
All focused native runs reused their external dependencies without rebuilding
them. The engine first compile caught a non-Send boxed error across an await;
it was dropped synchronously before async failure handling, and the rerun passed.
RootFmt passed after the repair. Final release-local RootBuild passed in 4m33s
(785 external reused, zero rebuilt); DesktopBuild passed in 7m53s (1,080 external
reused, zero rebuilt). Exact candidate versions/hashes and interactive evidence
are recorded in [the hand-off](CAPTURE_FIX_HANDOFF.md).
The [master checklist](CAPTURE_FIX_MASTER_CHECKLIST.md) tracks those milestones.
