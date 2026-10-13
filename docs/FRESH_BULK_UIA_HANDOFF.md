# Fresh bulk UIA candidate — 2026-10-14

Developer-only trial candidate. Privacy/security aims remain incompletely
validated; this document is not a safety guarantee. Deployment and the 20-minute
comparison require the owner's confirmation. No new capture or fixture GUI
session has been started during implementation.

## Change and coverage

Windows admission now obtains fresh per-attempt UIA bulk property caches instead
of thousands of provider child/sibling calls. Native app/title/include/private,
HWND/PID and monitor checks remain. Required property errors, unknown native
metadata, inconsistent query joins, budgets and changed revisions fail closed.
Invalidated evidence is reacquired within the original deadline; no tree or
admission result is reused across capture attempts. Handler cleanup precedes
publication and cannot authorize a late result.

| Production check | Fresh acquisition |
|---|---|
| Native app/title/include/private | Existing native checks and identity rechecks |
| Nested Window and Document exclusions | Applicable scoped Name/Value queries |
| Extension popup fallback | Scoped child and grandchild Name/Value queries |
| Ignored URL and DRM URL dependency | Fresh Document Value evidence; existing extracted-URL policy |
| Password and unknown entry control | Explicit cached password typing, existing subtree redaction |
| Keyboard/clipboard focus admission | Existing independent fresh focus/password/generation checks |

Per-attempt event subscriptions use MTA and serialized registration/removal.
Their overhead is included in the existing 250 ms deadline. Native provider
waits remain 50 ms; caller deadlines do not guarantee provider cancellation.

Intentional conservative changes: unsupported required names/Document values,
malformed structural property types, incomplete native identity/title metadata,
and inconsistent scoped-query key/value evidence now reject admission explicitly.
These can increase withholding; they do not authorize additional content.

## Acquisition exceptions and diagnostics

1. **Hierarchy structure:** RuntimeId, ControlType and IsPassword across the
   complete ControlView tree preserve ancestor pruning, ordering and completeness
   limits. Document/Edit/ComboBox Value is separately scoped where applicable.
2. **Extension shallow hierarchy:** two child levels can contain arbitrary
   element types, so their Name/Value/RuntimeId acquisition is broader than only
   Window/Document roles. It remains local to each extension subtree.
3. **Accessibility recording:** a separate fresh 15-property bulk acquisition
   retains existing UIA text/geometry recording; privacy-only acquisition does
   not silently remove that capability. Recording acquisition failures still
   withhold when required privacy evidence is unresolved.

Scope diagnostics contain fixed reasons, applicable configuration rule IDs,
property/rule/node counts, and truncation flags; no captured strings. Bounded
diagnostic deduplication stores only rule IDs, not permission or trees.
Authenticated `GET /vision/uia-metrics` exposes numeric totals, latency buckets,
stop-reason counts and acquisition/registration/cleanup counters. The historical
`provider_calls` field counts bulk acquisition calls; separate fields account
for root acquisition, event registration and removal. Reason codes are fixed.

No AccessKit/uiautomation-rs implementation or dependency was copied or added;
their architecture/API inspection informed the standalone experiments only.

## Baseline and comparison

The primary baseline is the owner-requested single snapshot after Windows
restart, bounded to ScreenWise startup at **2026-10-13 13:20:07.656 UTC**
(00:20:07 AEDT on 14 October). It covers about 456 seconds: 355 frames, 193
failed placeholders and 195 UIA-timeout-cause rows. These are independent
counts; a cause count must not be equated with placeholder count. The same-device
recovery proxy found 13 last-failure-to-admitted transitions, mean 1,404 ms,
maximum 3,101 ms. This does not prove readable capture or correct admission.
Old-build admission latency is unavailable, not zero. No unattended baseline
sampling was continued. Older pre-restart evidence is contextual only.

Private evidence: `.local/uia-bulk-validation/20261013-fresh-bulk/`.
Current baseline hashes:

| Artifact | SHA256 |
|---|---|
| Deployed GUI | `1428BE0186BC623369A378DF815885DC15B95B8AF771A5C26B253F822A2914C8` |
| Deployed recorder | `08E69304DC8223A8C76EB3F5F78A1C88A1E2B3292245F62AEB354CD7CCF12BF4` |

After confirmation, the comparison collector will run for 1,200 seconds from the
new run's start, using authenticated loopback metric GETs and read-only aggregate
SQL. It writes numeric-only JSONL, keeps bearer tokens in memory, and never
selects captured text, event payloads, titles, URLs or app labels. Use active
work/manual fixture phases for interpreting recovery; unattended idle results
are not equivalent to the owner's active baseline. New latency is measured
from counters/buckets; exact old/new admission latency improvement cannot be
claimed without an old latency measurement.

## Validation and artifacts

RootFmt passed. Accessibility tests passed 238 checks with 23 live tests ignored;
engine capture tests passed 54 checks, plus one numeric-endpoint and one
notice-store/query check. Earlier attribution/notice tests passed 13 diagnostic
checks and one native sampling simulation. Numeric collector tests passed six;
isolated rollback PlanOnly tests passed four. DesktopCheck passed in 2m22s,
reusing 1,084 external artifacts with zero rebuilt. These are synthetic/source
integration and compilation results, not live provider validation. Both Rust
lockfiles are unchanged; there are no dependency or frontend source changes.

Canonical locked/offline `RootBuild -Jobs 1` passed in 4m34s using
`release-local`: 785 external reused, zero rebuilt; 11 workspace reused, eight
rebuilt. Inert version/help/record-help checks passed (`screenpipe 0.4.15`).
Canonical locked/offline `DesktopBuild -Jobs 1` passed in 8m35s using
`release-local`: 1,080 external reused, zero rebuilt; ten workspace reused,
eight rebuilt. No full-LTO release build or candidate GUI launch was performed.
Changes are saved uncommitted on `screenwise`, based on `dba4aea22`.

| Candidate artifact | SHA256 |
|---|---|
| Recorder: `target/release-local/screenpipe.exe` | `22BE839BF6D99CFC9C2BA549ACEDD23BFD48AA42C38FF6D73C1614B4413CA191` |
| Desktop: `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe` | `3DF596A6B6BE5CF9C1080F9E02BD4525E35B99DA40BF3541651E3C1EFED850A0` |

Private canonical build evidence IDs: `20261013T134421145Z-Root-Build-e8d2c85b`
and `20261013T134906759Z-Desktop-Build-d80395f1`. Both profiles used established
caches without cold-cache overrides, downloads, dependency changes or cleanup.
Independent source review resolved required-property typing, scoped-query
consistency, handler cleanup ordering and numeric accounting findings; no
remaining admission blocker was reported within that review's source scope.
Live behavior remains subject to the gaps below.

The [separate manual fixture](FRESH_BULK_UIA_FIXTURE.md) is built and inert-tested.
Executable: `target/uia-privacy-fixture/FreshBulkPrivacyFixture.exe`.
SHA256: `F4D8418F5D1FAB70DBA09DD292D09693DB3C7636EF9BA7FB262E3CD118B87544`.
It supplies configuration deltas, six allow→block→allow cases, rapid mutations,
focus switches and password controls. Successful freshness uses 4 KiB; 64/320
KiB exercise property-limit failure. Out-of-process provider compatibility and
real rule transitions remain manual validation gaps.

Additional gaps: existing URL decoration checks the first extracted HTTP(S)
Document URL, not all Documents; existing ancestor extraction pruning semantics
are retained; UIA events are asynchronous/provider-dependent. Per-attempt
registration/removal can consume most of the deadline. The simple menu probe's
55 ms acquisition does not predict these production-rule timings. Input
completeness, audio recovery and DRM hardware behavior are not validated here.

## Owner deployment and rollback

After reviewing the final candidate, deploy the already-built `release-local`
artifacts from the repository root. The numeric trial should disable the older
content-bearing sensitive debug sink; numeric scope/metric reporting remains:

```powershell
& .\scripts\windows\deployment\Deploy-ScreenWise.ps1 -SensitiveDebugLogging $false
```

The deployment launcher retains previous application binaries/assets and requests
graceful shutdown; it does not change firewall rules or download dependencies.
Record the exact `release-previous-*` archive it prints. No deployment command
has been executed for this candidate.

If admission is incorrect or performance materially regresses, stop the trial
and **roll back** rather than weakening privacy gates. With the exact archive
path printed by deployment:

```powershell
$previous = '<exact release-previous archive path>'
$manifest = '.local\uia-bulk-validation\20261013-fresh-bulk\previous-deployment.json'
& .\scripts\windows\deployment\Restore-ScreenWisePrevious.ps1 -PreviousRuntime $previous -ManifestBackup $manifest -PlanOnly
& .\scripts\windows\deployment\Restore-ScreenWisePrevious.ps1 -PreviousRuntime $previous -ManifestBackup $manifest
& .\scripts\windows\deployment\Start-ScreenWise.ps1 -PreflightOnly
& .\scripts\windows\deployment\Start-ScreenWise.ps1
```

Restore checks baseline GUI/CLI hashes and rejects directory links/junctions,
requests graceful shutdown without forced kills, restores application files,
and preserves data/firewall/native dependencies. It validates the saved manifest
before suggesting restart. Never select a guessed archive or bypass a failed
preflight. Owner confirmation remains required before deployment/trial startup.
