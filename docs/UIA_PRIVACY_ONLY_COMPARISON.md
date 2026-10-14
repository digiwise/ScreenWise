# Windows UIA privacy-only comparison — 2026-10-14

This opt-in comparison removes optional UIA text acquisition from an inspection.
It retains fresh mandatory privacy evidence, applicable exclusion/URL/DRM rules,
the individual unknown-Value policy, traversal completeness and limits, original
privacy deadlines, revision retries, native identity/focus checks and finalization.
Images and OCR remain eligible after those checks. UIA text is unavailable in
privacy-only mode; independent keyboard/clipboard and audio gates are unchanged.
No tree or admission result is reused across attempts.

## Owner-run comparison

Deploy the reviewed desktop candidate separately. Quit ScreenWise from its tray
and wait for it to exit before each mode; a mode selection does not modify an
already-running process. From the repository root, start the normal comparison:

```powershell
.\scripts\windows\deployment\Start-ScreenWise.ps1 -UiaComparisonMode PrivacyAndText
```

Use Code, Access and ChatGPT while attended, with comparable windows and activity.
Record a bounded numeric sample using the collector below. Quit ScreenWise, then
repeat that activity with:

```powershell
.\scripts\windows\deployment\Start-ScreenWise.ps1 -UiaComparisonMode PrivacyOnly
```

Starting normally, or explicitly selecting `PrivacyAndText`, restores text mode.
The launcher confines the setting to its GUI child and clears inherited opt-ins
for normal mode. It never edits the persistent recording settings. Direct
launches can opt in with `SCREENWISE_UIA_PRIVACY_ONLY=1`; absent or `0` is normal.
Other values produce a fixed warning and use normal mode. A VisionManager
resolves the setting once and preserves it across monitor restarts.

The authenticated local `/vision/uia-metrics` endpoint reports actual finished
attempts by mode in `process_metrics.total.mode_attempt_counts` and each process
bucket's `mode_attempt_counts`. The fixed order is
privacy-and-text, privacy-only. Verify privacy-only attempts are present and
recording acquisition counts remain zero in that fresh process. No attempts
means the selected mode has not yet been exercised, not that capture succeeded.

The existing read-only collector now retains mode, failure-only timings, retry
causes, native timeout counts and phase budgets, while dropping basenames and
other free-form fields. Its fixed process selector codes are 0 other/unknown,
1 Code, 2 Access and 3 ChatGPT. From a second console, with provisioned Python:

```powershell
python .\scripts\windows\day-to-day-trial\numeric_uia_trial.py `
  --deployment .\.local\deployment\deployment.json `
  --output .\.local\uia-comparison\normal.jsonl `
  --seconds 180 --interval 10 --since-utc '<this ScreenWise startup in UTC>'
```

Use a new output such as `privacy-only.jsonl` and the new startup bound for the
second mode. The collector performs read-only authenticated loopback requests
and aggregate SQLite queries; it never starts capture, changes mode or returns
recorded content. It refuses to overwrite evidence. Counter totals describe
finished inspections since that process started; use endpoint deltas for a
subinterval, and do not subtract counters across restarts. Cumulative maxima
cannot be differenced. Keep the PC attended during the intended activity.

Compare completed/attempts and failure reasons for each process, mandatory
privacy latency, total inspection wall time, failed-attempt stage costs, retry
causes, and numeric failed-frame recovery. Count published mandatory completions
within 375 ms against **all** attempts when assessing the 70% target. The
collector's `admission_latency_ms` now measures published mandatory privacy
completion; `inspection_wall_latency_ms` measures all finished inspections,
including optional text and failures. Neither is image acquisition latency.

## Targeted failure diagnostics

The bounded per-process registry exposes separate failed and completed stage
calls, total microseconds and maximum accumulated stage time per inspection.
Repeated visits within an inspection count separately; maxima can include
several visits. Fixed retry categories distinguish privacy invalidation, text
invalidation, text evidence reconciliation and cleanup invalidation. Numeric
native timeout counters are separate from deadline expiry. Unknown attribution
and overflow retain numeric aggregates; named cardinality remains at most 32.
The default/exported snapshot omits process names, and generic timing warnings
do not acquire process identity or captured content.

Failed-stage counters include every failed inspection reason, including provider,
limit and monitor failures, and accumulated retry/cleanup work. They are not
timeout-only counters. Native HRESULT and retry counts are observations anywhere
in the finished inspection, not proof of its terminal cause; `timeout_stage` in
the generic trace still describes the first observed timeout, which can recover.
Failed-stage costs are not subdivided by failure reason: when several reasons
occur, do not attribute all of their accumulated time to timeouts. Generic
timeout warnings are sampled at the first diagnostic and every 64th thereafter;
`timing.timeout_diagnostic_count` and registry aggregates retain exact counts.

Phase and overall remaining-budget diagnostics describe failures; they are
observations, not extensions to any deadline. Provider calls can exceed their
configured timeout, and an in-process timer cannot interrupt a COM call that
never returns. The existing temporary diagnostics checkpoint in AGENTS.md
still applies. No persistent worker or permission cache is introduced.

Process metrics schema 2 adds `uia_monitor_unverified` as failure reason index
8, followed by `unknown` at 9. Unverified focus/monitor checks are counted as
failed checks, matching the existing global failed classification; capture
behavior is unchanged. The top-level acquisition schema remains 3. Budgets are
sampled at the terminal acquisition exit before fallback cleanup and diagnostic
native rechecks; late successful-result release overruns have their own failed
exit checkpoint. Zero minima have meaning only when their sample count is nonzero.

## Initial comparison reference and limits

The preceding owner-deployed build's first 298 seconds recorded Code 37/59
completed (62.7%, 22 timeouts), Access 19/25 (76%, six timeouts), and ChatGPT
25/25 (100%). No provider/evidence rejection was recorded in that snapshot.
Global Value observations included 33,622 UIA-confirmed unavailable observations
and zero unknown observations; these were not per-process counts. These are
descriptive initial figures, not a matched workload benchmark.

Its 29 timeout traces spent 5,152 ms in mandatory hierarchy acquisition and
4,581 ms in optional recording acquisition. Hierarchy acquisition dominated 22
traces, recording seven. Those traces lacked process attribution. This
comparison tests whether removing the second acquisition improves admission;
it does not establish that acquisition time alone caused every failure.

Synthetic source tests and a desktop release-local build do not establish live
provider event delivery, equivalent activity, correct image admission, actual
capture recovery, or the latency target. A fresh attended comparison remains
an owner action. If incorrect admission or material regression occurs, return
to normal mode and use the archived deployment rollback procedure as needed;
do not relax privacy checks to complete the trial.

## Validation and artifact

Combined source passed Astra review before the desktop build. Accessibility:
280 passed, 23 live tests ignored. Capture admission: 45 passed. Mode parsing and
configuration: two passed. Authenticated metrics endpoint: one passed. Numeric
collector: 11 passed. Deployment-script synthetic checks: 125 passed. RootFmt,
diff and both lockfile checks passed. Tests exercise configuration preservation,
failure/completion separation, attribution bounds, unknown/overflow conservation,
retry/HRESULT vectors, decisive budget retention and content-free collection.
They do not construct a live cached COM tree or exercise provider events.

Private canonical evidence: `20261014T090504348Z-Root-Fmt-e6eabfa7`,
`20261014T090518705Z-Root-Test-5e039ef0` (accessibility),
`20261014T090618385Z-Root-Test-d80e49e5` (capture),
`20261014T090858413Z-Root-Test-2ccd64b6` (mode),
`20261014T090912030Z-Root-Test-2d789e8b` (endpoint).
All external dependencies were reused in those checks. Both Rust lockfiles and
frontend source remain unchanged.

The Astra-approved desktop-only release-local build passed in 4m52s. It reused
all 1,080 external artifacts (zero external rebuilds), with 13 workspace artifacts
reused and five rebuilt. Evidence: `20261014T091004890Z-Desktop-Build-779899f7`.

Candidate: `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe`.
SHA256: `305A7774707EA92DF616C535B3AE37CF8528C7086351A079CF29DEBDF352A1CC`.
Built from the reviewed working tree based on `34c87df70`, before the commit
saving this comparison. Only the existing unused helper warnings remain.
No CLI build, deployment, recording restart or owner desktop trial is part of
source delivery. Live optional-text result counts remain global, not per-process;
process failures/native observations provide attribution for this comparison.
