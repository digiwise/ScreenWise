# Windows UIA admission and optional text — 2026-10-14

This candidate separates complete privacy assessment from optional UIA recording
text. It is a developer trial candidate, not a privacy or performance guarantee.
The owner requested implementation, automated validation, a desktop-only
`release-local` build and a commit; deployment and live capture are separate.

## Decision contract

All existing native app/title/include/private-window, nested Window/Document,
extension, URL, protected-content and target/focus checks remain applicable.
Mandatory UIA evidence is fresh for each attempt. No trees or admission decisions
survive between attempts. Unknown mandatory evidence continues to withhold the
image and OCR and produces the existing disclosure placeholder.

After complete policy acquisition, optional full UIA text has a separate budget.
A text-only timeout, provider error or acquisition limit discards all partial
recording text, nodes and hashes. An admitted image then uses the existing OCR
fallback. Complete policy URL metadata is retained for the final URL/DRM checks.
A detected exclusion still blocks, including one discovered by the recording
pass. Revision changes invalidate policy evidence; retries reacquire it within
the original privacy deadline. Native identity/focus changes, handler removal
failures and failed final verification never become permission.

Acquisition exception: when protected-content URL checks are enabled, their URL
evidence is always part of the mandatory fresh acquisition, including windows
whose earlier bitmap metadata already had a URL. Positive protected-content
evidence from that bitmap is also retained as a blocker if the later tree has
no URL or a different URL. This can add URL acquisition or increase withholding
in those windows; optional text failure must not erase a known blocker. The
existing complete structural cache and extension child/grandchild queries remain
unchanged; this candidate does not narrow production rule coverage.

| Phase | Windows budget | On failure |
|---|---|---|
| Mandatory privacy evidence | 750 ms from attempt start | Unknown; withhold if an applicable rule needs the unavailable evidence |
| Optional full recording text | 125 ms | Discard partial text; OCR fallback only after verified admission |
| Final verification/handler cleanup | 150 ms | Withhold |

Successful publication is capped at 1,025 ms from attempt start; a late optional
provider return cannot start a new admission window beyond that ceiling.
UIA connection/transaction timeouts track the remaining phase budget, respect
Windows' 50 ms minimum,
and reject another native request when insufficient time remains. These checks
bound accepted results and configured client waits; they cannot cancel a
permanently hung COM provider or promise a hard wall-clock return bound. The
existing temporary timing checkpoint still requires an external watchdog before
being claimed as hardened diagnostics.

Windows mandatory admission no longer inherits the adaptive accessibility-text
walk's 250 ms override or long application cooldown. Other platforms and the
independent keyboard/clipboard focus/password gate retain their existing paths.
Longer deadlines alone do not establish the owner's target: at least 70% of all
UIA checks must complete successfully within half the allowed time.

## Numeric diagnostics and compatibility

Windows results distinguish verified mandatory privacy from complete,
unavailable or configuration-skipped optional text. Per-frame accessibility
diagnostics identify `privacy_admission` or `optional_text`, with fixed cause and
processing codes and existing bounded rule references. Partial UIA strings are
not logged or persisted. Existing authenticated numeric metrics retain the
whole-attempt counters and add distinct privacy/text measurements; stage/rule
timings remain available. Consumers should not equate optional text failures
with withheld screen/OCR frames.

Mandatory acquisition samples are provisional and can include discarded
revisions. Published acquisition measurements count only returned verified
snapshots; separate published wall-time buckets include optional text and final
verification. Histogram boundaries include 125 and 375 ms, half of the former
250 ms and current 750 ms privacy budgets. These walker measurements do not
include the engine's later bitmap/focus checks or OCR/storage work.

## Validation and handoff

The settled candidate passed canonical `RootFmt`, 247 accessibility checks
(23 live checks ignored), all 24 synthetic capture checks and 58 engine capture
regressions. Tests cover discarded partial text, preserved policy URLs, observed
URL conflicts despite a text timeout, retained positive bitmap DRM evidence,
revision invalidation, original deadline boundaries, finalization failure,
untyped admission rejection and Screen/OCR diagnostic continuity. The capture
suite includes synthetic frame/placeholder persistence smoke coverage.

Astra's final source review approved the candidate before the desktop build;
subscription cleanup, metadata ordering, late-result classification and URL/DRM
findings were fixed before approval; deadline and URL/DRM regressions have
deterministic test coverage. This is source review and synthetic validation,
not live provider validation.

Private canonical evidence IDs: `20261014T010546051Z-Root-Test-e5509708`
(accessibility), `20261014T010615624Z-Root-Test-fee44113` (capture),
`20261014T010803422Z-Root-Test-94bed5ba` (engine), and
`20261014T010610962Z-Root-Fmt-e458723f`. External artifact reuse was respectively
264, 433 and 685, with zero external rebuilds. The new capture-test binary needed
one reviewed cache adoption; no cache was cleared or relocated.

Canonical locked/offline `DesktopBuild -Jobs 1` passed in **10m56s** using
`release-local`, after final Astra review and settled tests. It reused all 1,080
external artifacts with zero external rebuilds; 13 workspace artifacts were
reused and five rebuilt. Private evidence:
`20261014T011052659Z-Desktop-Build-e0e422d3`. Frontend source/export inputs and
both Rust lockfiles are unchanged. The retained legacy retry helper emits a
non-fatal dead-code warning; the existing live-test key helper does likewise.

| Desktop artifact | SHA256 |
|---|---|
| `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe` | `6F208BFFA7E1ECE35A7A4F310C560A2453DA1CD74FD6502B026C436F73BE1D6E` |

The artifact contains the reviewed working-tree code based on `00454cdbe`;
the implementation commit saves that code. No CLI build, deployment, recording
restart, desktop inspection or new capture session is part of this change.
Dependencies are unchanged. The existing CLI artifact was not refreshed.

Live gaps include provider event completeness/delay, embedded-browser and
password-manager transitions, OCR load/recovery, cross-monitor focus changes,
permanent COM hangs and the 70%-within-half-budget performance target. Synthetic
tests and a build do not establish those results. The next owner-run comparison
must measure admitted capture recovery and mandatory privacy latency separately
from optional text acquisition.

### Evidence rejection diagnostics

Bulk client-side evidence rejections emit `UIA bulk evidence rejected` with a
fixed reason code, numeric reason ID, fixed timing stage and cumulative count.
Ordinary logs sample the first rejection and every 64th occurrence per reason;
the authenticated `/vision/uia-metrics` endpoint exposes exact cumulative
`evidence_rejection_codes`/`evidence_rejection_counts` arrays. The existing
opt-in private diagnostic sink also records `uia_bulk_evidence_rejection` using
only the fixed reason ID/code and stage; its normal deduplication still applies.

Codes distinguish malformed RuntimeId variants/arrays/lengths, duplicate IDs in
query/value/hierarchy results, invalid control/password/string properties,
conflicting or missing scoped projections, projections outside the hierarchy,
negative child counts and a poisoned registration lock. Counts cover both
mandatory and optional acquisition and count rejection occurrences, not unique
attempts. Native COM HRESULT failures retain their separate diagnostic category.
No property values, runtime IDs, titles, process names or captured content are
logged by this change. Admission behavior and deadlines are unchanged. Existing
historical failures cannot be retrospectively assigned these new codes; live
verification requires an owner-deployed updated desktop artifact.

Validation: canonical accessibility library tests passed (248 passed, 23 live
tests ignored), RootFmt and `git diff --check` passed. Desktop-only locked/offline
`release-local` build passed in 8m42s, reusing 1,080 external artifacts with zero
external rebuilds. Evidence IDs: `20261014T022055207Z-Root-Test-2d88a02a`,
`20261014T022135649Z-Root-Fmt-8a21ac32`,
`20261014T022149598Z-Desktop-Build-1768747c`. Updated desktop SHA256:
`10921D0B64888B5D0D12D86D523A86CEE59B89F8BA2DB3972D3B1181DB7F492D`.
No deployment or new capture session was performed for this diagnostic update.
