# UIA capture regression investigation (2026-10-13)

This note records a bounded source, deployment-identity and authorized diagnostic investigation. It does
not establish restored live capture coverage or privacy/security guarantees.

## Confirmed deployment and supplied report

Read-only deployment metadata records `release-local` deployment at
`2026-10-13T02:27:48.4616056Z`. The installed GUI has file/product version
`2.5.28`. Installed binaries match the corresponding current build outputs and
the public accessibility/privacy candidate hashes:

| Artifact | SHA-256 |
|---|---|
| GUI | `FE823E90D88083FADC10290A3FD4FBC3A724FF6DDC3C3980ECC658F002CCC181` |
| Recorder | `D483130FA4F5520CB3EFCE2491A483254165F6268C7F7E988C4BD678343FA48E` |

The owner reports that API queries for `2026-10-13T02:31:26Z` through
`2026-10-13T02:36:46Z` returned 113 OCR rows, with 110 reporting active-monitor
`capture_failed`/UIA walk timeout conditions across three monitors. The supplied
search result contained zero keyboard input rows and zero audio rows, alongside notices. These are
owner-supplied observations. The investigation did not query the production API
or read captured contents. After fresh owner authorization, a separate bounded
diagnostic projection inspected only numeric and fixed-stage fields for the
reported interval; the sanitized findings appear below. Matching hashes rule out a stale
installed binary in this comparison, but do not identify the timeout cause.

## Deterministic source defects

The keyboard privacy refresh used a 50 ms walk budget while provider request
admission requires at least 50 ms remaining. That combination cannot complete the required focused-probe sequence: any
elapsed work reduces the remaining budget below the native minimum. Usually
the first request is refused; even an initial zero elapsed reading cannot
leave room for the final verification request. This is a concrete source defect compatible
with missing keyboard evidence; it does not prove every absent keyboard row
has this cause.

The correction uses a dedicated 250 ms keyboard-probe budget and a minimal
cache for password state, process identity and keyboard focus. It obtains the
focused element with `GetFocusedElementBuildCache`, verifies final focus and
compares element identity. Required cached properties reject default values.
Cached password evidence is stamped at acquisition and refused if verification
leaves it older than 75 ms; the 250 ms work budget does not extend permission
freshness.

The traversal completion path also applied provider-request admission after
the final provider call had returned. A completed tree finishing between
200 and 250 ms could therefore be discarded because less than an additional
50 ms remained, even though no further provider request was needed. Completion
checks enforce the actual walk deadline separately from provider request
admission. The repair permits a provider request while actual walk time remains,
using the valid native 50 ms timeout floor, then checks every reply, including
a null reply, against the actual 250 ms deadline before decoding or accepting it.
A request begun near the deadline can therefore remain in flight beyond it;
native timing does not establish a hard 50 ms wall-clock overshoot bound.
Existing 14 cached properties, 5,000-node and depth-35 limits, bounded decoded
text, and fail-closed privacy decisions remain. Deterministic fake-timing
regressions cover request admission, late replies, completion and the keyboard
probe budget. The source still constructs the walker/UIA cache per frame and
discards its backoff state. That outstanding performance limitation requires
a separate persistent-worker design with explicit COM/thread-affinity ownership.
It is deferred rather than treated as a requirement ambiguity in this repair;
this change does not make a broad architecture change.

Failure attribution also gains a separate additive
`capture_privacy.accessibility.sampled_target_app` field. Its executable basename
comes from the exact failed target HWND/PID after a fresh native identity check.
It carries no title and does not populate app/window placeholder fields or claim
that this application matched an exclusion rule or caused the failure.

## Authorized interval diagnostic projection

The bounded numeric/fixed-stage projection recorded these findings:

| Recorded diagnostic | Count / distribution |
|---|---|
| Tree timeout stops | 262 |
| Stops near the reserved 50 ms cutoff | 250 |
| Stops at least 50 ms before the deadline | 8 |
| Stops past the walk deadline | 3 |
| Stops below the native minimum | 1 |
| Timeout walk-progress records | 268; visited nodes 0-362, mean 145; 173 visited 50-199 |
| Deduplicated native `UIA_E_TIMEOUT` records | 11: root 4, child 4, sibling 3 |
| Input provider-budget records | 16 |
| Native focus-unavailable records | 2 |

These are diagnostic records, not total provider-request counts: the sink has
repeat deduplication and caps. No successful-completion counts were inspected.
The 250 cutoff records support the reservation-defect diagnosis but do not
prove that those attempts would complete successfully with the repair. Genuine
native provider timeouts are also present. Missing Screen/OCR evidence therefore
cannot all be assigned to one defect from this projection.

## Evidence boundaries and outstanding validation

The source corrections address these two budget defects. They do not prove
that real provider calls fit the configured limits, that all reported Screen/OCR
failures have this cause, or that useful capture/input coverage is restored.
Zero audio rows do not by themselves establish a recording defect without
evidence of speech or system playback in that interval. The reported audio/noise
behavior remains a live-validation question; this repair adds no audio policy
or audio source change. Notice counts span channels and reason transitions,
and diagnostic sink deduplication/caps mean they are not provider-request totals.

Native provider timeouts and unresolved privacy rules remain separate possible
causes. Live restoration requires fresh authorized evidence. No new capture,
interactive UIA/input inspection or production API reads are included; diagnostic
access was limited to the freshly authorized interval projection above.

The inspected DigiTrack checkout has no ScreenWise HTTP consumer integration.
Its History surface queries DigiTrack SQLite repositories, converts local picker
times to UTC and filters spans by time overlap. No ScreenWise query filter,
pagination offset or response classifier in that checkout explains this direct
API report.

## Repair validation

RootFmt passed. Focused synthetic checks passed 227 accessibility tests (23
live tests ignored), one inspected-target attribution test and 89 engine capture
regressions: 317 distinct tests, zero failures. The earlier 45-test capture
selection is a subset of the 89 and is not counted twice. All external build
artifacts were reused; none were rebuilt during these test runs. A final source
review found no actionable findings in the scoped implementation diff; it was
source-only and does not establish live restoration.

Sequential, single-job `release-local` builds passed. Recorder build: 4m11s,
785 external artifacts reused/zero rebuilt, 14 workspace reused/five rebuilt.
Desktop build: 8m55s, 1,080 external reused/zero rebuilt, 13 workspace reused/five
rebuilt. Inert recorder `--version` (0.4.15) and `--help` checks passed without
starting recording. Both Rust lockfiles are unchanged.

| Repaired candidate artifact | SHA-256 |
|---|---|
| GUI | `1428BE0186BC623369A378DF815885DC15B95B8AF771A5C26B253F822A2914C8` |
| Recorder | `08E69304DC8223A8C76EB3F5F78A1C88A1E2B3292245F62AEB354CD7CCF12BF4` |

A metadata-only candidate inventory verifies seven runtime files, 13 assets,
502 current frontend files, both native-library input hashes and the graceful
quit marker. Existing frontend export, unchanged sidecars/native libraries and
Pi source hashes are reused. No frontend change or parked external-client MCP
work is part of this repair. No deployment, quit request, GUI launch, restart or
new capture was performed. The hashes in the first table identify the owner's
reported deployment; this second table identifies the repaired build candidate.

The owner can deploy the prepared candidate as a separate step from the repository
root using the existing launcher:

```powershell
pwsh -File scripts/windows/deployment/Deploy-ScreenWise.ps1
```

The launcher reuses the built binaries, verifies runtime/deployment prerequisites,
requests graceful shutdown of the owned deployment and starts the replacement.
That step changes the running deployment and is not part of the evidence above.
