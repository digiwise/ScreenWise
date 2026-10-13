# DigiTrack consumer report after the 2026-10-13 afternoon deployment

These are owner-supplied aggregate observations from authenticated loopback GETs
to `/search`, `/meetings` and `/capture-events`, with `timezone=utc`. No production
API, captured content, private logs or live desktop state was read for this
follow-up. Deployment identity was not independently hash-verified in this pass.
Times below are AEDT (UTC+11).

The reported restart stopped the session at 14:46:35 and reported unlocked at
14:47:20. This is a later checkpoint than the earlier
[budget repair investigation](UIA_CAPTURE_REGRESSION_20261013.md).

| Observation | 14:47–15:12 | 16:05–16:30 |
|---|---:|---:|
| Keyboard key rows | 395 | 962 |
| Keyboard text rows | 115 | 110 |
| Active-monitor failed placeholders / frames | 419 / 452 | 531 / 558 |
| Failed placeholders with foreground attribution | 0 / 419 | 0 / 531 |

Keyboard text was absent in the reported 13:40–14:05 interval and is now present.
The later active-monitor samples still had no readable frame text; all three
monitors were affected. Failed frames report `accessibility_inspection` and
`uia_walk_timeout`. Unresolved rules were ignored-window indices 0, 1, 4–22
(`uia_subtree_exclusion`) and builtin index 0 (`drm_url`). This is evidence of
continued loss, not restored Screen/OCR coverage.

The earlier interval had 60 focused-element failures, 52 focus-verification
failures, 38 native-focus-unavailable, 36 focus-ownership failures, 30 generation
changes, 24 unavailable checks and six stale checks across keyboard/clipboard
suppression notices. Notice counts do not count exact missing keystrokes.

Content-protection pauses were reported at 15:46:37, 16:02:43 and 16:34:42.
Output device ordinal 2 repeatedly reported no callbacks and recovery. Microphone
admission/processing notices reportedly cycled 267 times in 25 minutes. Audio
search returned little. The API and pagination were consistent; active-monitor
provenance followed focus and matched device names.

## Source diagnosis and scoped repairs

`paired_capture::walk_accessibility_tree` constructs and drops a walker on each
attempt, inside an engine blocking-pool task. Windows initialization, UIA cache
construction and native policy checks consume the walk's 250 ms deadline.
The tree provider makes individual cached root/child/sibling calls, and partial
trees are discarded. The previous budget repair removed an early cutoff but did
not reduce traversal work. Dropping the walker also discards provider backoff.
These source costs are plausible contributors; this report does not measure
their individual contribution or prove that persistent initialization alone will
make large trees fit the deadline.

Failed placeholders now obtain a separate, fresh native foreground sample at
the failure phase. The HWND and PID must agree before and after executable lookup.
`capture_privacy.blockers` carries its sanitized executable basename,
`foreground: true` and the fixed `capture_failure` reason. Unknown/changing
identity stays null. This sample is distinct from the earlier inspected target
and is not a matching exclusion or evidence that this application caused the
provider failure. It is omitted from diagnostic `excluded_apps`. Window titles
remain absent under the [blocker metadata policy](CAPTURE_PRIVACY.md).
Monitor samples surrounding the lookup merge with earlier phases; disagreement
or unavailable association invalidates active-monitor provenance.

Generic notice messages group rules by origin and kind, retaining counts and
the explanation that unresolved checks withhold Screen/OCR. All exact indices
remain in `diagnostic.rules`. This reduces duplicated message text, not notice
row counts or input-search paging. Existing stored rows are not rewritten;
notice API reconstruction uses the current formatter.

The Windows content-protection precheck uses native foreground/process identity
and rechecks it; it does not walk UIA. Its pauses therefore require a separate
native-identity diagnosis. Output no-callback timeouts are non-fatal in the
Windows recorder path; silence can legitimately yield no callbacks, so separate
device/worker lifecycle events are needed to explain the reported restarts.
Microphone AudioProcessing transitions between silent samples and usable samples
also use admission/gate-cleared notice codes. Those rows alone do not prove
microphone permit revocation. Current microphone permission generations are
independent of foreground content protection. Audio behavior remains unresolved.

## Proposed persistent-worker follow-up

Replace per-attempt blocking-pool construction with one owned Windows UIA thread
per monitor. Construct COM/UIA/cache objects, use them and release them on that
thread. Supply current configuration, monitor geometry and exact selected target
with each request. Keep only provider setup/cooldown state between attempts;
never reuse tree text, target identity or permission. Bound pending work, reject
obsolete requests, and prevent a timed-out request's eventual reply from being
attached to a newer capture. A shutdown must not move apartment-owned objects
onto another thread. Native timeouts still cannot promise provider cancellation.

This design needs owner approval under the repository's material-architecture
rule. Approval is pending. Deterministic checks should cover changing targets and
configuration, late replies, queue capacity, shutdown and cooldown. Fresh live
UIA/input validation would remain a separate readiness-gated step. Increasing
limits or accepting incomplete privacy inspection is not part of this proposal.

## Validation

RootFmt and `git diff --check` passed. Canonical locked/offline synthetic checks
passed 53 final engine capture tests, 13 diagnostic tests, one foreground identity
sampling test and one SQLite notice storage/query test. A preliminary 46-test
`capture_` engine selection also passed and overlaps the final suite; these
counts must not be summed as unique tests. All external artifacts were reused,
with zero external rebuilds. The final engine run rebuilt three workspace
artifacts in 35.21 seconds and reused 685 external artifacts. The foreground
sampler and notice formatter tests remain applicable after the final wrapper
change, which only brackets attribution with existing monitor-provenance sampling.
Both Rust lockfiles are unchanged.

No recorder/desktop application build, commit, deployment/restart, capture,
interactive UIA/input inspection or firewall change was performed. The code is
an uncommitted source follow-up; existing application artifact hashes describe
the preceding build, not these edits. Useful live Screen/OCR restoration,
input completeness, content-protection pauses and audio delivery remain open.
