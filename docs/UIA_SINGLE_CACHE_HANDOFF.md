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
Non-control policy-bearing nodes still reject admission, preserving the old
scoped-query completeness behavior. The selected root remains included.
Control-membership property changes invalidate the attempt alongside Name,
Value, ControlType and IsPassword changes, including after a retry subscription.

Cache decoding reads policy properties on their applicable node and uses that
same cache for ancestors, descendants and extension scopes. Missing
RuntimeId alone is no longer a reason to reject an otherwise complete hierarchy.
Unsupported required policy properties, malformed structural data, incomplete
traversal, limits and native errors continue to fail closed. Unknown password
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
`raw_nodes` describes the broader traversal. New fixed rejection codes distinguish
malformed control membership and a raw-only policy element. Removed RuntimeId
and cross-query rejection slots remain present with zero counts.

## Evidence and limits

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
Control View probe timings do not directly measure this candidate. Node/depth
limits apply to the complete raw traversal and are consequently more conservative
than limits on the former control-only hierarchy. The temporary timing checkpoint
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
