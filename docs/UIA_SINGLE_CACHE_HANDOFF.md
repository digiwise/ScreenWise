# Fresh single-hierarchy UIA acquisition — 2026-10-14

Windows privacy acquisition now obtains applicable properties in one fresh
hierarchy cache instead of joining a structural cache with scoped queries by
RuntimeId. Some Access elements provide usable rule properties without usable
RuntimeIds; that missing join identity previously withheld capture.

Each mandatory acquisition and optional recording acquisition has its own new
cache. No tree, captured text or admission result is reused between attempts.
The existing 750-ms mandatory privacy deadline, 125-ms optional text deadline,
150-ms finalization budget and 1,025-ms accepted-publication ceiling remain.
Invalidated evidence is discarded and retried within the original privacy
deadline. Native identity, focus, revision, DRM and cleanup checks remain.

## Acquisition and privacy coverage

The provider cache uses Raw View so policy-bearing nodes omitted from Control
View cannot silently disappear. Cached control membership, with Windows' normal
default handling and strict Boolean validation, reconstructs the former
Control View for extraction: non-control ancestors are flattened in order.
Non-control policy-bearing nodes are evaluated from attempt-local policy
evidence obtained from the same cache, without adding them to recorded text.
The selected root remains included.
Control-membership property changes invalidate the attempt alongside Name,
Value, ControlType and IsPassword changes, including after a retry subscription.

Cache decoding reads policy properties on their applicable node and uses that
same cache for ancestors, descendants and extension scopes. Missing
RuntimeId alone is no longer a reason to reject an otherwise complete hierarchy.
Required Name properties, malformed structural data, incomplete traversal,
limits and acquisition/native errors continue to fail closed. The owner-approved
individual Value exception is described below. Unknown password
evidence retains the existing conservative text-entry redaction policy.

Nested Window/Document exclusions, Document and entry URL evidence, extension
child/grandchild names and values, terminal/password/decorative ancestor handling
and optional accessibility recording remain in scope. This does not change the
independent keyboard/clipboard admission path.

Acquisition exception: subtree cache requests apply one property union across
their hierarchy. Therefore applicable Name and/or Value properties are acquired
across element types, rather than solely through the old scoped queries. Only
properties needed by applicable rules are requested, and decoding still limits
policy use to the existing rule scopes. The maximum mandatory union contains
five properties (ControlType, IsPassword, IsControlElement, Name and Value);
URL-only acquisition uses four, and optional full-text acquisition uses fifteen.
Scope diagnostics contain fixed reason codes,
bounded rule references and numeric property/node counts, never property values.

Metrics schema 3 retains the existing counters and rejection-code slots. It adds
fixed mode `single_raw_cache_control_projection`, explicit single-cache totals,
and raw/projected node counts. Existing `nodes` counts retained controls;
`raw_nodes` describes the broader traversal. Fixed rejection codes distinguish
malformed control membership; the earlier `raw_only_policy_element` rejection
slot remains present but is no longer emitted. Removed RuntimeId
and cross-query rejection slots remain present with zero counts.

## Evidence and limits

### Raw-only rule evaluation follow-up

Raw-only nested Window titles and Document names/values are evaluated against
applicable exclusions. Extension evidence uses its own two-level meaningful
child/grandchild scope, including nested extension Documents; empty raw layout
wrappers do not consume its levels. The projected ControlView scope still skips
flattened ancestors. Matching evidence suppresses the entire
snapshot. Nonmatching supported evidence can proceed without changing text
extraction or password/decorative/entry pruning.

All available raw and retained Document HTTP(S) URLs are checked against ignored
URLs, including later candidates and those under pruned ancestors, and can supply the
downstream DRM URL when no projected candidate exists. If URL policy applies,
different raw and projected candidates, or differing raw candidates, remain
unknown rather than choosing a URL that could conceal protected content.
Diagnostics use the fixed `multiple_document_urls` reason and an evidence count
in the sensitive diagnostic sink; property values are not logged.

The owner explicitly accepts the privacy risk of individual unavailable or
unknown UIA **Value** properties. They are nonblocking unknown evidence, never
substituted with empty strings. Other available names, values and URLs are still
evaluated; unknown Values cannot erase a known blocker or replace a known DRM
URL. Required Names and structural evidence remain strict. This changes privacy
policy: a blocker available only through an unknown Value can be missed.

Value diagnostics distinguish known strings, UIA-confirmed unavailability,
unknown variants, getter failures and sentinel-check failures. Exact cumulative
counts appear in `value_evidence_counts`; fixed reasons and numeric VARIANT tags
also use the sensitive diagnostic sink, whose unchanged-record deduplication is
preserved. `CheckNotSupported` identifies the reserved absence sentinel only for
diagnostic classification; failure to prove absence no longer grants a false
empty value or rejects an otherwise complete attempt. No current-property or
ValuePattern fallback, extra acquisition or availability-property request is used.
Microsoft documents the [unsupported-property semantics](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-propertiesforclients)
and [sentinel check](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation-checknotsupported).

Optional text reconciliation preserves only this attempt's known mandatory URL
when every previously selected Document source now has unavailable/unknown
Value evidence. Source positions are bounded, attempt-local RawView traversal
ordinals; Document count and revision must remain unchanged. Known empty,
non-HTTP or changed source values still require fresh privacy, as do count/source
mismatches. Neither property values nor admission results are reused across
attempts. Unknown optional values do not mask a newly observed blocker.

Raw layout-only wrappers have an independent hard traversal depth cap of 128.
Projected ControlView depth and meaningful policy depth retain the configured
cap, at most 35; policy-bearing raw-only nodes consume policy depth. The 5,000
node cap, original phase deadlines and complete-traversal checks remain. No
RuntimeId lookup, retained tree or reused admission result is introduced.

### Local per-process metrics

Authenticated loopback `/vision/uia-metrics` explicitly includes verified
executable basenames with attempts, completed/failed/denied counts, fixed failure
reason counts and stage calls/total/max microseconds. Attribution checks the
native HWND/PID before and after an uncached basename lookup and compares current
native metadata; changed/incomplete identities and logical aliases remain
unknown. Successful-attempt attribution runs while invalidation handlers remain
active, before the final deadline/focus/revision/cleanup checks; it never supplies
capture permission. Failure-only attribution cannot turn a result into admission.

At most 32 named process buckets are retained in memory, plus unknown and
overflow. The process registry preserves totals and reconciles buckets under one
lock; its snapshot and existing atomic global counters can briefly differ while
attempts complete concurrently. No paths, titles, URLs, PID/HWND or captured
contents appear in process metrics. The default/export snapshot API is anonymous;
only the dedicated local API opts into names, which are never added to notices.
Counters describe finished capture-tree inspections, including repeated stage
visits/retries; independent keyboard/clipboard focus checks are not included.
Permanently hung COM calls cannot finish their counters: the existing external
watchdog gap in AGENTS.md remains unresolved.

### Follow-up validation

Final source validation passed 278 accessibility tests (23 live tests ignored),
58 capture-admission regressions and the fixed/bounded local metrics endpoint
test. Scope/type/depth helpers and synthetic extractor tests cover matching and
nonmatching raw-only evidence, nested extension scopes and wrapper bounds,
unknown Values before/after known blockers, optional URL reconciliation,
metadata uncertainty, overflow/total conservation, anonymous exports and
concurrent process recording. These do not construct a complete cached UIA COM
tree or validate live provider events/performance.

Astra reviewed the final combined Value, hierarchy, URL and process-metrics
diff before the desktop build. Findings on retained extension-grandchild
properties, optional unknown-URL reconciliation and attribution placement were
fixed; the final source review approved with no remaining blocker. RootFmt and
diff checks passed; neither Rust lockfile changed.

Private canonical evidence: `20261014T080840774Z-Root-Fmt-e38d5de0`,
`20261014T080842641Z-Root-Test-5434f945` (accessibility),
`20261014T080852567Z-Root-Test-9663a0ad` (capture),
`20261014T080943109Z-Root-Test-6f4426ff` (endpoint). All external dependencies
were reused in these checks. The Astra-approved desktop-only `release-local`
build passed in 10m41s, reusing all 1,080 external artifacts with zero external
rebuilds; 13 workspace artifacts reused and five rebuilt. Evidence:
`20261014T081058757Z-Desktop-Build-b93f1c6a`. No CLI build, deployment or owner
desktop trial is included.

Current candidate: `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe`.
SHA256: `E1D408426A39CFFAE4EE4FCFFEF1D0E7729F17A2A34B18AC72C3C875DA63EE49`.
Built from the reviewed working tree based on `a4e0721a1`, before the commit
saving this follow-up. Only the existing unused retry-helper/test-key warnings
remain. Actual provider events, cached COM-tree integration and post-deployment
latency/recovery are unverified. Likely remaining blockers are provider timeouts
or hangs, repeated revision invalidation, resource bounds and differing known
Document URLs. The independent input focus checks are unchanged.

Validation and the desktop-only release-local build are recorded in
[the register](../VALIDATION_REGISTER.md). Live provider correctness, revision
delivery, useful capture recovery and the 70%-within-half-budget target remain
unverified for this follow-up; it has not been deployed by the agent.

### Historical initial single-cache evidence

The owner-run standalone Access probe found 260 hierarchy nodes, of which 16
provided no usable cached or direct RuntimeId. Six comparison passes across two
runs returned no additional required-property type failures under the probe's
limited checks. Five single-cache passes took 189–245 ms including setup; one
first-pass outlier took 1,159 ms. The three-acquisition diagnostic path took
573–728 ms, but would reject identity before completing its diagnostic work.

These are standalone acquisition measurements, not production admission or
privacy validation. They exclude full rule evaluation, event subscriptions,
revision/focus checks, optional text and finalization. Three warmed passes are
not evidence of the 70%-within-half-budget target across applications. Live
provider event coverage, embedded browsers/extensions, rapid changes, cross-
monitor focus, OCR recovery and permanently hung COM calls remain validation
gaps. Production now acquires Raw View and control membership, so those earlier
Control View probe timings do not directly measure this candidate. Separate
raw-wrapper and meaningful-depth limits apply as described above. The temporary timing checkpoint
in AGENTS.md remains applicable.

The standalone numeric probe source is preserved in
`scripts/windows/uia-runtimeid-probe/`. It does not alter production admission,
recording settings or focus; its live execution requires an owner-run command.
No private probe results or recording contents are included in source.

## Validation and delivery

Canonical RootFmt, standalone probe formatting and `git diff --check` passed;
both Rust lockfiles and dependency manifests are unchanged. Final automated
validation passed 256 accessibility tests (23 live tests ignored), 45 engine
capture-admission tests, one fixed/numeric metrics endpoint test and 24 synthetic
capture/persistence tests. No owner desktop, capture or fixture was inspected
during these tests.

Astra reviewed before the desktop build. Review findings concerning omitted
raw-only policy elements and missing control-membership invalidation were fixed;
the final focused source review reported no remaining blocker. Built-in agent
creation reached its thread limit, so the Astra reviews used the installed Codex
CLI with read-only sandboxing. This is source/synthetic validation: actual
provider cache construction, raw ancestor projection through COM, property-change
delivery and production performance still require an owner-run trial.

Private canonical evidence IDs: `20261014T061402471Z-Root-Test-11835468`
(accessibility), `20261014T061504990Z-Root-Test-dd1d72cf` (engine),
`20261014T061633599Z-Root-Test-0fbb65bf` (metrics),
`20261014T061648809Z-Root-Test-9beb8389` (capture),
`20261014T061546676Z-Root-Fmt-cfeffa50` (formatting). No external dependencies
were rebuilt in those runs. No new dependencies, UIA worker architecture or
cross-attempt cached permission were introduced.

Canonical locked/offline desktop-only `release-local` build passed in 13m12s
after final Astra review and all required tests. It reused all 1,080 external
artifacts with zero external rebuilds; 13 workspace artifacts were reused and
five rebuilt. Evidence: `20261014T061913361Z-Desktop-Build-05d1b443`.
The retained legacy retry helper still produces its existing dead-code warning.

Desktop executable: `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe`.
SHA256: `8D86E6A1C1044E3ADE7004D7AC2120A9B1435412AEA1D47DFFD7CAA537C3A657`.
The artifact was built from the reviewed working tree based on `a9b597ab5`,
before the commit saving these changes. No CLI or full-LTO release build was run.

Deployment is a separate owner action; no live capture or fixture run is part of
this change. The existing owner-run deployment and archived-runtime rollback
procedures still apply. If an owner trial finds incorrect admission or a material
regression, roll back to the exact archived deployment rather than weakening
the privacy checks.
