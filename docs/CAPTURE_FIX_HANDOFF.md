# Capture fixes: deployment hand-off (2026-10-09)

## Candidate to deploy

Deploy the **release-local** candidate `20261009-capture-fixes-66814bfb3-dirty`.
It was built from working-tree changes on baseline
`66814bfb34e9c319b9d627c23b7db9d524c0ef33`; that baseline commit alone does not
contain these fixes. Package versions were not incremented, so identify it by label and
hash, not the displayed version alone.

| Artifact | Version | SHA256 |
| --- | --- | --- |
| `target/release-local/screenpipe.exe` | 0.4.15 | `673B06C1C2530FF79F20E98261300CAC62039C5D32D937D9835481576B62EA5A` |
| `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe` | 2.5.28 | `FCCA3D9DBE2772A4DC71BE5A8FA6B35D1849D5546844409F02724D216B009CD7` |

The private candidate manifest is
`.local/build/deployment-candidates/20261009-capture-fixes-66814bfb3.json`.
It pins binaries, sidecars, native inputs, assets, source inputs and the final
502-file frontend export. External-client MCP remains parked and excluded.
The owner confirmed deployment on 9 October. The running desktop executable
matches the candidate hash and its verified process start was
`2026-10-09T06:09:27.871258Z` (17:09:27 AEDT). This is process-start evidence,
not a claim of the exact first capture time. The agent did not deploy or restart
production. Claude Code/the separate client only verifies `/capture-events`
through the API; source, builds and authorised maintenance belong to this task.
No live soak is run here.

## Root causes and resulting behaviour

- Audio's per-buffer classification shared a diagnostic key with recurring gate
  admission. Silence and gate-open observations overwrote each other. Empty
  buffers could also reopen that diagnostic and refresh the usable-input watchdog.
- Keyboard/clipboard worker rejection counters were reused by logging and
  diagnostic publication, while healthy probe admission could overwrite the
  recent rejection. Each poll now consumes a separate numeric rejection cursor
  and publishes one combined outcome.
- The optional accessibility background reader used its internal
  `capture_tree=false` flag as the global user preference, despite paired capture
  being enabled. Only actual session settings now own that preference.
- The dashboard could retain a different observer's old exclusion label while
  attaching the stale age of another check. Retention now requires the same
  observer. Stale means time since that observer's last check, not proven duration
  of the exclusion. Details distinguish configured rules, built-in policy and
  unavailable window metadata; no rule is invented for unknown metadata.

History uses monotonic **one-second dwell** for noisy audio silence/recovery and
transient input-check cause/recovery transitions. Initial conditions and the
first input suppression are reported immediately. This absorbs brief buffer or
poll gaps while retaining sustained transitions; actual admission, samples,
privacy checks, VAD and availability watchdogs are not delayed. Stops,
no-callbacks, degradations, redactions, unlocks and concrete privacy failures
retain immediate transition reporting. There is no global event-rate cap.

The [per-gate state/reason-code table](CAPTURE_EVENT_FLAP_FIX.md#expected-post-deployment-checks)
is the client checklist. In a quiet ten-minute window, expect a handful of
microphone `capture_silent_input/capture_silent_input` and
`capture_gate_cleared/capture_gate_open` transitions. Keyboard/clipboard should
report `capture_suppressed` with the specific input-check reason and one
`capture_gate_cleared/capture_gate_open` per sustained recovery, rather than
alternating per check. Output audio has the same noisy-classification repair.
Availability/operation event codes and `/capture-events` response shape remain
unchanged.

## Visible maximised windows

Selection is per monitor and does not require global focus. A visible maximised
window remains eligible with focus on another monitor. Screenshots, OCR and UIA
use the same selected HWND/PID; accessibility retains its current full-tree
behaviour within that window. Restrictions use the selected app/title/URL.
Unverified background browser URLs remain fail closed. Small topmost overlays
are ignored for selection; full-monitor topmost surfaces count. Identity,
geometry, selected-window and privacy checks discard changed samples.

The dashboard reports `active_window_only / maximised_window_only`, with fixed
reason code `capture_maximised_window_only`, as **Maximised window only**. Policy
permission alone does not claim a recent successful capture. See the
[implementation and limits](VISIBLE_WINDOW_CAPTURE_PLAN.md).

## 11:30 AEDT increase

The best-supported installed candidate was `f61be168e`, prepared 10:45 and
copied 10:53 AEDT. No canonical build/deployment evidence identifies a new build
at 11:30. That revision changed reconciliation/storage, not this classifier or
event emission. Per-buffer classification events originated in `3ea64c8c7` on
8 October; the mechanism predates the jump. Exact running-process identity and
the cause of the increased alternation remain unproven. See
[historical evidence](CAPTURE_EVENT_FLAP_FIX.md#1130-aedt-increase-evidence-and-limits).

## Historical cleanup

Under the later explicit owner authorisation, the live database was pruned only
for `2026-10-08T22:03:57Z` through `2026-10-09T01:02:17Z`. Removed 18,500 flood
rows; retained 1,024 original target transitions and all 2,078 other notices
unchanged. Four authenticated API pages confirmed 3,102 survivors. Genuine
stops, no-callbacks, degradations, redactions and unlocks survived.

No replacement rows were fabricated and no captured contents were queried.
The original cleanup changed only its authorised period; the separately
authorised additional periods are listed below. A private removed-row archive and receipt were
verified before exact-row deletion. The archive covers typed notice columns,
not a complete database backup. Retained interval references may point to
removed timestamps; this is a conservative pruning, not a lossless reconstructed
debounced history.

The owner's additional authorisation extended the same strict cleanup to other
proven floods. Three separately previewed, fingerprinted and archived ranges
were applied, leaving their other notices byte-for-byte unchanged:

| Additional UTC interval, 9 October | Removed | Retained target transitions | Other notices preserved |
| --- | --- | --- | --- |
| 01:02:17–04:00:00 (12:02:17–15:00 AEDT) | 9,271 | 961 | 1,802 |
| 04:00:00–05:00:00 (15:00–16:00 AEDT) | 4,440 | 417 | 896 |
| 05:00:00–05:15:00 (16:00–16:15 AEDT) | 838 | 128 | 186 |

Additional removals total **14,549**: microphone 14,259, keyboard 145, clipboard
145. Additional survivors total **4,390**: 1,506 target transitions and 2,884
other notices. Including the original range, removed **33,049** and retained
**7,492** notices within the four cleaned ranges. Earlier outside-period history
contained 125 notices with no proposed removals. The fixed cutoff is 05:15 UTC;
later events are untouched, and the running old build can still create new flaps.

Unknown/non-typed diagnostic shapes (49 across the outside-original inventory)
remain untouched. Output device 2 had 34 sample-classification events and 15
rapid adjacent changes; this small dataset did not establish a historical flood,
so it was preserved along with its genuine no-callbacks, recovery/session stops
and stream failure. Rapid PII-redaction/focus/native-check alternations may be
real and were also preserved. Private additional archives/receipts are under
`.local/maintenance/capture-events-20261009-additional-{01,02,03}/`.

The final **one-off pre-fix cleanup after deployment is complete**. It used the
verified start of the hash-matching desktop process as a conservative cutoff:
`[2026-10-09T05:15:00Z, 2026-10-09T06:09:27.871258Z)`. After preview and a fresh
private archive, removed 3,268 rows (microphone 3,148, keyboard 60, clipboard 60).
Retained 426 target transitions and all 651 other notices unchanged, for 1,077
survivors. Total across five ranges: **36,317 removed; 8,569 retained**.
The archive/receipt is private in
`.local/maintenance/capture-events-20261009-post-deploy-01/`.
Everything at or after the cutoff remains untouched, including the client's
post-deployment verification window. No recurring pruning is scheduled. See the
[runbook and completion record](POST_DEPLOY_CAPTURE_EVENT_CLEANUP.md).

## Validation and interactive result

Passed: config 59, screen 131, accessibility 191 (23 live tests ignored), engine
capture 45 and notice/API persistence 13, audio producer/recovery/shutdown 16
serial, maintenance 5, frontend 106 unique checks, TypeScript, RootFmt and
16-page export. Audio's initial parallel failures came from shared-global test
interference; serial execution passed without source changes. A non-Send engine
error lifetime found during compilation was repaired before settled tests.

Canonical release-local RootBuild passed in 4m33s, reusing all 785 external
artifacts. DesktopBuild passed in 7m53s, reusing all 1,080 external artifacts.
Neither rebuilt external dependencies or changed either Rust lockfile.

Four freshly consented synthetic fixture attempts stopped before pixel
acquisition. The second established native maximisation and visibility but lost
target foreground after accepted activation. The third owner-click attempt
passed readiness, then found WinForms had implicitly assigned an owner to the
overlay. The fourth overlay child exited before publishing readiness; its exact
failure was not retained. At the owner's request,
a gpt-6-astra agent replaced the fixture with two independent simple forms,
without owner mutation, parent-handle coupling, global focus or a click gate.
The replacement was prepared with ten verified pins and fresh owner consent.
Its C# self-test/Python syntax, RootFmt and two Rust evaluator tests passed;
the narrow integration build took seven seconds, reusing all 433 external artifacts.

The freshly consented minimal test **passed in 3.53 seconds** on the existing
production WGC window primitive, with a native-maximised cyan target and a
topmost magenta overlay in another process. Target control, overlay control and
the covered target patch each had colour fraction **1.0**; overlay-colour fraction
in the target capture was **0.0**. Native PID/maximisation/bounds/Z-order checks
passed around the two acquisitions. Both owned processes exited successfully.
Evidence is private under `.local/interactive-window-overlay/20261009-overlay-simple-06/`.
No screenshots were saved: fixed-colour images were evaluated in memory and
discarded. This test did not force window-state changes during acquisition.
State/identity changes have deterministic source coverage. Scoped synthetic
evidence does not establish isolation for all transparent/layered windows,
popups, notifications, other backends or UIA. No multi-minute capture or
production event-rate verification is performed here.

The owner subsequently requested three pre-acquisition changes after storing
the target. The extension **passed all three cases in 9.10 seconds**, with
fresh consent on the verified owner desktop:

| Changed state | Raw stored-window WGC result | Real production validator |
| --- | --- | --- |
| Ordinary non-topmost foreground window covers target | Last-painted green target fraction 1.0; replacement magenta fraction 0.0 | Rejected changed selection |
| Target minimised | Capture error | Rejected stored target |
| Target closed | Capture error | Rejected stored target |

Each cyan baseline fraction was 1.0 and the requested changed native state was
verified. No replacement-window or monitor image acquisition was attempted.
This directly exercises `verify_windows_capture_target`, but does not exercise
recorder persistence or a mutation during acquisition. Last-painted pixels do
not establish continuous frame freshness. Exact source/runtime pins and fixed
aggregate results are private in `20261009-overlay-state-08`; no images were saved.
The test-only extension build passed two offline checks, leaving both live tests
ignored by default, in 57.84s with all 433 external artifacts reused. RootFmt
passed. Production source and candidate binaries did not change.

The first extension attempt (`state-07`) failed before any mutation: baseline
WGC returned `0x80070424` under the sandbox account. It was launched in a
different account context from the passing test. The owner's capture service
was running; the error did not establish a missing installation. Metadata-only
desktop preflight and the fresh-consented unchanged retry passed outside the
sandbox. The failed attempt remains preserved and supplies no state-change evidence.

The owner's follow-up decision is to retain cover/minimise/close and related
selection/state validator checks as defence in depth for now. If an affected
check proves buggy, consider removing or narrowing it instead of automatically
fixing it or adding guards. No runtime check has been removed. This decision
concerns potentially redundant state checks; target identity and current
content/privacy admission have distinct purposes. See the
[recorded decision](VISIBLE_WINDOW_CAPTURE_PLAN.md#owner-decision-retain-state-checks-as-defence-in-depth).
