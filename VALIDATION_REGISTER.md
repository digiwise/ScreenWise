# ScreenWise validation and issue register

Updated 2026-10-08. **Partial validation, not a completed privacy/firewall certification.**
> [!WARNING]
> **No privacy or security guarantees.** These bounded results do not establish
> that the system or code is safe for confidential use. The maintainer and
> Digiwise make no such assurance; see the [README notice](README.md).

This sanitized public summary preserves the scope and unresolved issues. Raw
captures, logs, stores, machine identities and owner interaction records are
private. Historical authored notes are preserved locally under ignored `.local/`.

Latest artifacts and validation: [status and deletion integration](#status-and-deletion-integration-release-validation-2026-10-08). The earlier hashes below belong to the previous candidate.

## Production release build milestone, 2026-10-08

Capture-policy implementation `3ea64c8c7` passed canonical `RootBuild` and
`DesktopBuild`, each with `-BuildProfile release`, after plan inspection. Root
completed in 8m24s (785 external reused/zero rebuilt, 11 workspace reused/eight
rebuilt); desktop completed in 13m34s (1,080 external reused/zero rebuilt, ten
workspace reused/eight rebuilt). Both observations were complete, with no new
external variants or unexpected rebuilds. Existing caches/native setup were reused
without cold-cache override, acquisition or cleanup.

The 54 synthetic deployment-script checks passed. The rebuilt release recorder
passed inert version/help/record-help checks; the desktop executable was not
launched. Existing frontend export/type and focused-test results remain applicable.
Both lockfiles were unchanged. Release artifact SHA256 identities:

| Artifact | SHA256 |
|---|---|
| `target/release/screenpipe.exe` | `C6D77DCDA3D095EDD6B2F0FE87ACBA53C06FE2E17255BF76E5990126B890E397` |
| `apps/screenpipe-app-tauri/src-tauri/target/release/screenpipe-app.exe` | `0687C07EAD7B791DD0435BB8839697B7F58C1DF209BD1694EB58767659B12ECB` |

No live capture, interactive validation, production-data read, firewall change,
deployment or restart occurred. The active installation still runs its previous
artifacts. Installed-package preflight, hardware microphone signal and live
transition behaviour remain separate validation steps; compilation does not
resolve the previously observed silent microphone input.

## Launcher concurrency, 2026-10-08

Opt-in root/desktop overlap retains same-cache, desktop shared-staging and default
serial exclusion. Synthetic isolated-worktree validation passed 12 OS-lock,
97 launcher, 18 cache and 84 trial-profile assertions. The two-launcher test held
root stub Cargo while desktop stub Cargo completed, with independent evidence.
Same-cache/evidence-override, desktop staging and junction refusals plus failure
release passed. See [exact commands and limits](BUILD_NOTES.md#launcher-concurrency-protocol-2026-10-08).
Integration repeated all 211 assertions successfully. Real concurrent release
operations used the existing caches with two jobs each: root passed in 33.83s
with 785 external and 19 workspace artifacts reused, none rebuilt. Desktop
passed in 11m52s after a generated Tauri asset timestamp forced one workspace
relink (1,080 external reused/zero rebuilt, 17 workspace reused/one rebuilt).
Speedup measurements and Windows PowerShell 5.1 execution remain unvalidated.

## Microphone admission and pause ordering, 2026-10-08

Microphone acquisition, queued/background/live transcription, SQLite writes and
delayed callbacks now carry modality/device permits independently of visual
exclusions and Windows foreground/protection uncertainty. Output protection is
retained. Default lock, schedule, audio/device disable and explicit manual
all-recording pause still revoke affected work; resume cannot revive old permits.
The authenticated session-only `GET/POST /recording/privacy` exposes the manual
pause flag. Desktop stop sets it before awaiting shutdown, and startup completion
respects newer requests. Version-1 historical protection notices retain their
original microphone meaning; current notices describe screen/output suppression.

Eight policy tests, 15 audio privacy tests, 15 synthetic recorder tests, 18 engine
privacy tests, 21 protection tests and 43 event-driven tests passed. These are
overlapping selections, not an aggregate count. They cover microphone continuity
during visual protection, common-gate rejection, device-generation revocation,
buffered tails, queued database/callback rejection, partial shutdown, queue
pressure/loss, automatic device switch-back and deterministic newer-pause races.
An initial combined engine run exposed three test-status isolation failures;
isolating test writer status repaired them without clearing production degradation.

Six interval-store tests also passed, including warning before rejection without
polling, pressure deduplication, recovery, exact numeric loss and late-registration
replay. A fixed numeric callback now logs diagnostic storage pressure and rejected
deliveries directly, independently of SQLite and Timeline queries.

Independent read-only review used **gpt-6-astra / high**. Its six findings were
fixed: sticky automatic-recovery disablement, desktop start overwriting a newer
pause, unscoped optional macOS tap chunks, device resume overwriting a newer
pause, superseded preference diagnostics and missing direct pressure/loss logs.
The reviewer confirmed resolution
by source inspection; it did not execute tests or inspect captured contents.

Final desktop checking and 11 focused `recording_` tests passed, including the
blocked-start/newer-pause case. They reused all 1,084/921 external artifacts with
none rebuilt. Canonical formatting and whitespace checks passed; both lockfiles
are unchanged.

The final engine `release-local` build passed with all 785 external artifacts
reused and none rebuilt (nine workspace artifacts reused, eight rebuilt).
Inert CLI version/help checks passed. This is not a production-release result.

The final desktop `release-local` build also passed (5m49s), reusing all 1,080
external artifacts with none rebuilt; ten workspace artifacts reused and eight
rebuilt. The desktop application was not launched.

Frontend type checking/export passed with the existing `unpdf` warning. No capture
session, captured-content read, model/dependency acquisition, firewall change or
deployment was performed. Hardware callback/race behavior and macOS/Linux builds
remain unvalidated. Paused OS streams may remain allocated, and the shared model
can reset when switching devices. See [policy/coverage limits](docs/CAPTURE_DIAGNOSTICS.md).

## Capture interval diagnostics, 2026-10-08

Subsequent unaffected-monitor implementation: a consistent verified foreground
on another monitor bypasses its global UIA tree and trigger labels, allowing
admitted local bitmap/OCR work. Pixel admission remains based on every local
overlapping window: excluded foreground, background and spanning windows retain
redaction/isolation behavior. Missing enumeration, unknown native association,
changed HWND/PID within evaluation and monitor changes across policy phases
remain fail closed. Protected-content/lock/schedule policy and audio retention
were not changed; no proposed monitor grouping UI was built.
Passed 69 synthetic screen-policy and 43 event-driven engine tests, with all
433/685 external artifacts reused and zero dependency rebuilds; canonical
formatting passed. The updated engine `release-local` build passed (785 external
artifacts reused, zero rebuilt; 13 workspace artifacts reused, four rebuilt),
followed by inert version/help checks. These tests do not establish live WGC/UIA race behavior,
occlusion correctness or complete privacy coverage. Existing inactive-monitor
preferences still determine whether a worker captures. See
[per-monitor behavior and limits](docs/CAPTURE_PRIVACY.md).

Content-free interval observations now persist independently of capture admission
for screen acquisition/storage/OCR, accessibility, input, microphone/output and
transcription producers. Fixed reasons distinguish protection matches, uncertain
checks, exclusion rules, partial redaction, fallback, silent samples, missing
callbacks and processing deferral. Each source/channel/numeric scope retains
onset and links changed segments; unchanged state is deduplicated. Authenticated
Timeline responses retain currently open explanations across date boundaries and
expose diagnostic capacity/loss status. Existing admission policies are unchanged.

Passed synthetic checks: four config interval-store tests, ten notice persistence
tests, 20 content-protection tests, 41 event-driven capture tests, one engine rule
provenance test, 66 screen-policy tests, 11 keyboard privacy tests and 16 Timeline
tests. The live keyboard test remained ignored. Canonical formatting and whitespace
checks passed. The engine `release-local` build passed with all 785 external
artifacts reused and zero dependency rebuilds (nine workspace artifacts reused,
eight rebuilt). Inert `--version`, `--help` and `record --help` checks passed.
No captured contents were read and no capture session, firewall
change or deployment was performed. See [coverage and remaining
limits](docs/CAPTURE_DIAGNOSTICS.md), including crash tails, provider paths and
configuration-index stability; this is not complete producer or runtime validation.

## Audio segment search and Pi evidence guards, 2026-10-08

Audio search now returns individual background transcription rows rather than
collapsing rows sharing a chunk/offset. Exact FTS matches select the matching row,
and tied segment ordering is stable across merged live/background pages. The API
adds an optional background `transcription_id`; live references retain negative
chunk IDs. Counts use the same speaker/hallucination admission filters. Exporter
references preserve distinct segments instead of deduplicating them by chunk.

Pi exposes bounded audio/text/input `q` searches. An empty exact audio search
scans at most ten pages/1,000 rows under the same time range and request deadline
for possible spelling/spacing variants and nearby text. Approximation and
unfiltered candidates are explicitly labelled. The tool computes displayed
segment timestamps and returns one text copy per reference. Completed-message
guards preserve stored quotations and replace unsupported error/empty claims
with uncertainty. Recording-mode draft streaming waits for this guard. Explicit
supported English date/time ranges constrain requests; other date wording,
summary accuracy and relevance still depend on model behavior. Approximate
matches, retained offsets and speech recognition do not establish exact words
spoken, word timing, call apps, participants, speakerphone status or completeness.

Passed synthetic checks: four SQLite regressions, ten search/API unit tests, one
authenticated router integration, 25 JavaScript retrieval/guard tests, pinned SDK
registration/tool/message hooks and 74 exporter checks. Local inference passed
matching-context, empty-success and request-failure fixtures using the existing
model/runtime. The final four-case run also checked automatic titles with zero
recording requests. Five inert Python runtime/output-isolation checks passed.
Prompt-only trials exposed inconsistent quotes and error claims;
the final guards address those tested paths. No packages/models were downloaded,
recording started or firewall rules changed. Both Rust lockfiles are unchanged.
Build commands, cache observations and artifact milestones are recorded in
[BUILD_NOTES.md](BUILD_NOTES.md).
Quick recorder/desktop builds passed in 3m58s/6m49s. Full recorder/desktop releases
passed in 12m16s/19m35s, reusing all 785/1,080 external artifacts and rebuilding
seven workspace artifacts each. Ten final desktop recording/runtime/event-routing
tests passed. The final extension-only title/list-argument amendments passed
their direct SDK/JavaScript/local-model checks and are embedded in the full GUI
release. Matching Pi assets are in the ignored release output; the running
deployment remains unchanged. Release recorder version/help checks passed without
capture. These builds do not establish live GUI/capture or privacy/firewall behavior.

## Global foreground blockers and active-monitor queries, 2026-10-06

Excluded foreground windows are now evaluated for diagnostic purposes even when
they are on another monitor. Local monitor acquisition rules are unchanged:
another monitor's excluded foreground does not grant a local fallback or by
itself prohibit an otherwise permitted full-monitor frame. Structured blocker
records retain executable basenames and fixed reasons; unavailable identities
stay unknown. Foreground windows absent from enumeration receive a distinct
uncertainty code, not an inferred configured exclusion.

New frame metadata records nullable `foreground_monitor` and `is_active_monitor`,
plus `foreground_monitor_changed`. Before/after enumeration samples and the
capture/pre-UIA/post-UIA monitor samples must agree; differing knownness or monitor
ownership clears the association. Changed foreground identity during enumeration
withholds pixels rather than accepting an inconsistent snapshot. Sampling does
not prove continuous focus between observations. Native monitor ownership is
not the primary display, pointer location or billable activity.

Authenticated OCR search now accepts `active_monitor=true|false`. SQLite applies
the stable typed-boolean filter before pagination and to the result count.
Unknown/legacy/malformed/changing metadata matches neither value; omit the filter
for historical or uncertain coverage. Other content types return a validation
error. Cache identity includes this filter. Redacted active-monitor frames remain
searchable even though their older `focused` flag is false. DigiTrack's guide
documents redaction codes, global versus local blocker roles, the query and its
coverage limits. No new schema migration, authentication change or data backfill
is required.

Background checks passed: 65 capture-policy tests, four SQLite integration tests,
nine search/API unit tests, 41 engine capture tests and one synthetic paired
capture/persistence smoke (120 total). They include another-monitor excluded
foreground diagnostics, sticky unknown monitor transitions, redacted-frame
metadata, legacy null preservation, malformed JSON and filtering/counting/paging
with both empty and FTS queries. Root/desktop formatting passed and neither Rust
lockfile changed. These checks do not validate the new metadata against live
Windows focus; that remains a scoped post-deployment observation. No new GUI,
recording session, synthetic input, model download or firewall mutation was
started for these checks. The owner's running deployment is left in place.

Quick recorder and GUI builds passed in 2m22s and 4m56s. Full release builds
passed in 6m15s and 10m52s, respectively. Every build reused all selected external
dependencies (785 root, 1080 desktop), with seven affected workspace artifacts
rebuilt and zero unexpected external rebuilds. Recorder quick/release version
and help smoke passed without starting capture. The expected macOS-only Swift
skip warning remains. All 585 tracked build inputs checked by file size/UTC
timestamp stayed unchanged; no artifact hashes were calculated. The new binaries
require deployment before any claim about their live foreground diagnostics.

## Capture blockers and safe Explorer fallback, 2026-10-06

The owner accepted the preceding monitor-association correction as verified and
authorized this subsequent enhancement, background tests, full release builds
and publication. New Windows event-driven frames store structured
`capture_privacy` JSON atomically with their frame. Authenticated OCR search and
frame-context responses expose it; old rows remain null with no backfill.
Blockers contain verified executable basenames and fixed reason codes only,
with a 32-entry cap and explicit truncation. No titles, URLs, document paths or
configured filter strings enter this field. Unavailable identities and later
UIA refusals remain unknown rather than being inferred from stale focus.
See [the field and policy reference](docs/CAPTURE_PRIVACY.md).

Ordinary Explorer folder classes may now serve as the already existing allowed
foreground-window-only fallback when excluded background windows overlap the
same monitor. Desktop/taskbar/unknown Explorer surfaces cannot, and explicit
Explorer exclusions still win. Background pixels remain blacked out; other
monitors do not borrow the foreground window. An exclusion discovered immediately
before UIA now invalidates an earlier full-monitor bitmap, preserving the
blocker evidence on a source-free inconsistent-focus placeholder.

Background checks so far: 62 screen/capture policy tests, three real SQLite
migration/persistence/search integration tests, 41 engine capture tests and one
API serializer test passed. The synthetic paired-capture smoke also persisted
the exact disclosure while bypassing accessibility/OCR extraction. The first screen-suite attempt had two incorrect
test expectations (scoped includes leave unrelated apps unrestricted, and
deterministic ordering places unknown blockers first); correcting the assertions
made the rerun pass without changing the established filter semantics. No new
capture session, focus change, synthetic input, model download or firewall
mutation was performed. Private synthetic database/image test artifacts are
not publication content.

The reviewed standalone screen test cache reused all 433 external artifacts.
The new SQLite integration target adopted its existing cache and created two
SQLx variants for its narrower dependency graph; no unexpected external rebuild
was reported. Engine tests reused all 685 external artifacts. Both Rust lockfiles
remain unchanged. A final process-identity guard rejects truncated/invalid UTF-16
path buffers instead of inventing an app identity; its additional regression and
the affected screen-suite rerun passed. Final quick builds passed for recorder
(1m25s, 785 external reused) and GUI (5m00s, 1080 external reused), rebuilding
only affected workspace artifacts. Non-recording recorder version/help smoke
passed. The full recorder release build passed in 6m23s with all 785 external
artifacts reused, seven workspace artifacts rebuilt and a passing non-recording
version/help smoke. The expected macOS-only Swift-skip build warning remains.
The full GUI release build passed in 11m39s, reusing all 1080 external artifacts
and rebuilding seven workspace artifacts. Source sizes/UTC timestamps for 592
recorded build inputs stayed unchanged across release compilation. No artifact
hashes were computed. These are compile and inert CLI checks, not new production
capture or performance measurements. Full and quick builds had zero unexpected
external rebuilds. Canonical root/desktop formatting and staged diff checks passed.

Outstanding: real Explorer-over-excluded-Excel operation and the GUI display of
these new app-level details have not been tested. Current GUI banners retain
their fixed explanation; the structured details are available in the database
and APIs. Per-frame metadata is not exhaustive interval coverage, and a
preexisting allowed-window-to-allowed-window focus race can pair an earlier
bitmap with later UIA text. Broader provider/occlusion/transition coverage,
reconciliation-worker shutdown catch-up and the original privacy/firewall
milestone remain open. The running deployment is not updated by a build/push.

## Windows per-monitor accessibility association, 2026-10-06

A reviewed owner-authorized short real-data export exposed the same foreground
accessibility tree stored against several monitor devices. Source review found
that the Windows tree walker ignored the capture worker's monitor scope. The
screenshot acquisition policy already evaluates each monitor separately; this
finding concerns text, element and metadata provenance and does not establish
that screenshot pixels were copied from another monitor.

The correction admits the foreground accessibility tree only on its owning
monitor, checks ownership again after acquisition, and uses OCR of already
privacy-checked pixels on other monitors. Those background frames no longer
inherit the foreground app/title or focused flag. Unverifiable ownership emits
the existing safe inconsistent-focus placeholder with a content-free local log
reason. A current-tree hash check also prevents OCR or changed-tree frames from
reusing stale accessibility-element references. Screenshot privacy admission,
authentication and historical recorded data are unchanged. A spanning window
has one accessibility owner; visible portions on other monitors use OCR.

Fifteen Windows tree-walker tests and forty event-driven capture tests passed,
including six new ownership, invalid geometry, negative-origin, metadata and
element-reference regressions. Canonical root formatting and `git diff --check`
passed. The first standalone accessibility test target required reviewed initial
cache adoption: four external variants rebuilt, with no unexpected external
rebuild. Its existing unused `press_key` test-helper warning remains. Engine tests
reused all 685 external artifacts. The scoped locked/offline `release-local`
recorder build passed in 3m07s, reusing 785 external artifacts and rebuilding five
workspace artifacts. Both Rust lockfiles remained unchanged.

Non-recording CLI help and three-monitor enumeration passed. Read-only effective
ActiveStore inspection found enforced existing exact-path outbound block rules
for this recorder and its FFmpeg/FFprobe sidecars, preserving loopback ranges.
This inspection is not a new outbound-connectivity test. A short real-data run
with independent foreground-monitor samples was prepared behind a fresh owner
readiness gate. The deployed full-release recorder has not been updated; no
production-artifact behavior claim or firewall mutation is made.

The first owner-approved sixty-second attempt ran under the isolated automation
account rather than the interactive desktop. All 598 foreground samples were
unavailable, UIA initialization failed, privacy gates paused acquisition and no
frames were written. It is an invalid monitor-association test, not a pass.
Health returned 200; protected requests returned 403 without a token, 403 with a
wrong token and 200 with the matching local token. OS endpoint sampling showed
only a loopback listener for the owned recorder and no UDP endpoints. Graceful
console shutdown timed out; the guarded controller terminated its owned process.
Read-only OS inspection confirmed no remaining test process or listener. Existing
privacy evidence was preserved. The controller now refuses to start when the
foreground desktop is unavailable. A non-recording normal-user desktop probe
succeeded; the retry required fresh readiness. Smart text-PII model provisioning
was absent in the isolated account and regex-only coverage was reported; no
models were downloaded and no privacy gate was disabled.

The subsequent owner-approved normal-user retry ran from 07:47:04 to 07:48:06
UTC, with audio and keyboard/clipboard row persistence disabled, all three
monitors selected and Firefox/Excel exclusions retained. It saved 26 frames:
16 accessibility, three background OCR and seven no-safe-active-window privacy
placeholders. All 12 accessibility frames with a stable independently sampled
foreground owner matched their capture monitor; four near focus transitions
were ambiguous and are not claimed as independently verified. No identical
accessibility text occurred across different devices within two seconds.
The three background OCR frames carried no foreground app/title/URL/document
metadata or previous accessibility-element reference, and their 972 elements
were OCR sourced. Accessibility persisted 4,175 elements on the two admitted
monitors. The third monitor had only privacy placeholders, so positive UIA/OCR
capture there remains untested. No element-reference reuse occurred in this run;
the current-tree reference checks have unit-test coverage only.

SQLite `quick_check` returned `ok`. Health/authentication repeated 200/403/403/200.
OS sampling found the exact recorder's loopback-only listener and no UDP
endpoints; no deliberate outbound probe was run. Ctrl+C reached the recorder,
shutdown completed with exit code zero and no forced termination, and both
native inventory and read-only OS inspection found no remaining owned process
or test listener. Audio, password stimuli, image pixels, production release
performance and sustained recording were outside this check's scope.

Follow-up observations: Smart text-PII models were also missing for the normal
user, leaving regex-only reconciliation; image-PII processing was disabled.
UIA keyboard privacy initialization retried once and unknown/stale focus states
were suppressed safely; no keyboard/clipboard rows were persisted. One async
accessibility-reconciliation SQLite-lock warning occurred and reported retry.
At shutdown, 25 of 26 frames had a completed reconciliation marker; the final
frame, captured about two seconds before shutdown, remained unmarked. Capture
applies synchronous regex removal separately, so this observation does not
establish an unredacted disclosure. Review async worker shutdown/catch-up and
content-free backlog reporting before claiming final-tail reconciliation.

**Scoped result:** the two-monitor foreground UIA attribution and background OCR
checks support the monitor-association correction. They do not complete general
privacy validation or update the deployed production recorder. The failed first
attempt and both private evidence directories remain preserved.

## Persistent local deployment preparation, 2026-10-06

Added [deployment scripts](scripts/windows/deployment/README.md) for separate
release binaries and persistent recording data, with explicit scoped firewall
installation/restoration and opt-in login startup. They reuse the prepared
private WebView2 runtime and existing model stores; no Pi package is copied.
Runtime update checks use sizes/timestamps, not cryptographic tamper detection.
The app accepts an explicit pinned shared Pi runtime while mutable configuration,
chats and sessions stay under each recording root. Deployment enables only the
owned read-only recording API extension, with literal loopback URLs, bearer
authentication, bounded responses and no built-in shell/file tools. Ollama's
three executable paths join six app/runtime paths in the deployment scope.

Thirty-eight background synthetic PowerShell checks passed: parsing, local path
boundaries, data/runtime isolation, running-process update refusal, failed-stage
preservation, previous-version retention, changed runtime metadata, nine executable
scopes, conflicting/loopback-blocking rules, firewall inspection errors, shared
package identity/version/dependencies, broad-tool refusal and restoration after
runtime unavailability. Seven pure Rust path/tool/integrity tests and three
linked desktop package-integrity regressions passed. Fifteen mocked JavaScript
transport tests passed, including microsecond dates, cache bounds, pagination,
GET-only scope, secret omission and truncation. The installed MIT Pi 0.75.4 SDK
registered exactly one owned tool and executed it using fabricated fetch only.
No SDK implementation was copied; its package metadata, extension loader/type
contracts, extension guide and CLI argument handling were the scoped reference.
The non-mutating deployment plan passed. Windows PowerShell 5.1 execution was
blocked by that host's script policy; no policy was changed. Login registration,
real deployed GUI startup/shutdown, relocated resource loading and live firewall
validation remain untested. The `release-local` GUI build and full release build
both passed, each reusing 1080 external artifacts and rebuilding only one app
target. The unchanged recorder was not rebuilt. A non-launching deployment then
staged the release candidate, owned extension and manifest with separate empty
recording data, reusing the existing Pi/model/WebView2 stores. No capture, model
download, login registration or firewall mutation occurred. The owner must
install the nine rules before live preflight/startup; tool-enabled GUI retrieval
and shutdown against these new deployment paths remain untested.
The actual staged inventory passed 17,178 size/timestamp/link-boundary checks;
native metadata lookup reduced verification to 9.46 seconds. This surfaced and
fixed a second JSON-date conversion bug in the deployment manifest comparison;
a serialized timestamp regression now covers it. The deployed owned extension
matched the compiled source files; at preparation verification, the recording
directory was empty and contained no Pi package. Metadata matching is not tamper
certification.
The subsequent owner-run Windows PowerShell 5.1 firewall script stopped before
rule creation on a falsely missing long package path. Fixed native inspection
using extended Windows paths while keeping rule paths canonical. Forty-three
synthetic checks now pass, covering long paths and actual change/missing-file
refusal. Actual PowerShell 7 verification and a Windows PowerShell 5.1 native
metadata probe each checked 17,178 files with no mismatch. No firewall mutation,
redeployment or binary rebuild was performed during this correction.

## Local DigiTrack evidence exporter, 2026-10-06

Added [private API review scripts](scripts/windows/evidence-review/README.md)
for an explicit timezone-bearing, end-exclusive period. Per-modality JSON and an
escaped self-contained HTML view retain source references; retrieval failures,
budgets and unknown coverage are visible. No model/cloud call or raw media fetch
is made. Optional frame context and bounded prior notices support inspection,
without claiming an authoritative historical initial state or full snapshot.
Seventy-two synthetic assertions passed, including serialized JSON timestamps,
micro/nanosecond precision, meeting overlap, filtered empty-page advancement,
notice subdivision/saturation, metadata reconciliation, auth failure versus empty,
HTML escaping, secret omission and overwrite refusal. Tests found and fixed
PowerShell's automatic JSON-date conversion and a mock closure scope defect.
Proxy use and redirects are disabled. Comparisons preserve original timestamp
strings but have documented 100 ns precision limits. No captured contents were
read and live API/export compatibility remains untested. The optional
`-OpenReview` switch opens the generated local HTML after saving; script parsing
passed, but automatic browser opening was not exercised during preparation.

## DigiTrack API contract assessment, 2026-10-06

The [integration guide](docs/DIGITRACK_API_INTEGRATION.md) records a source-level
review of time-range evidence, timestamp/provenance fields, authentication and
privacy notices. The current API is usable for limited on-demand enrichment of
DigiTrack's own activity history; it is not a complete or snapshot-consistent
capture-history contract. Findings include missing historical coverage/state
intervals and store identity, capped notices without a cursor, meeting-start
filters instead of overlap selection, and timestamp-only offset pagination.
Accessibility `on_screen` search selects frames but returns combined frame text;
use source/visibility-filtered elements for explicitly visible text.

Canonical locked/offline engine library tests passed: seven privacy-notice,
seven search utility and eight timezone tests. No external/workspace rebuilds,
real capture reads, recording, firewall changes or runtime source edits occurred.
Router-level client integration and complete historical coverage remain untested.
Added nine fabricated JSON response examples and per-channel request targets,
including pagination, overlap and boundary handling. All JSON examples parsed;
they are illustrative contract data, not observations from a live recorder.

## Real Pi GUI attempt, 2026-10-06

**Latest result:** the third fresh, content-disabled `release-local` GUI run
completed two local arithmetic turns, persisted both correct replies in the GUI
chat and Pi session files under its native test root, and recorded zero tool
events. API health and 403/403/200 authentication passed. Native held handles
verified all three Bun workers and all three exact signed System32 console
helpers exited on tray quit. The generic monitor's console/attention flags were
preserved and independently reconciled, not ignored. Exit was zero, marker
`clean-v1`, and a final OS snapshot found no scoped GUI processes or API listeners.
All nine confirmed executable rules remained enforced; no restoration occurred.
The fixed outbound block/control checks and native socket evidence support only
their scoped result. The OS console host has no global block, and no packet trace
or tool-enabled/offline-guarantee claim is made.

The run found duplicated assistant reply text inside injected follow-up history.
Both direct and queued sends now use one production history builder. Forty
affected tests passed with regressions for mirrored text, legacy block-only rows,
deliberate repetition and preserved bounded tool context. Corrected frontend
compile/type/export, `release-local` desktop build and inert scope/model/block-control
preflight passed. The fresh, owner-authorized corrected-build GUI retest also
passed: the prior reply appeared exactly once in injected follow-up history,
four ordered messages and matching replies persisted in both stores, and zero
tool events appeared. The model returned correct fixed equations in code blocks
instead of plain numbers; exact-output instruction adherence failed. The initial
numeric-only checker rejection is preserved and reported, with correct-equation
assessment separated from the unchanged strict history assertion.
Tray quit again produced `clean-v1` and exit zero. Independent held-handle and OS
checks found all three Bun workers and three exact signed console hosts exited,
no scoped GUI processes/API listener, nine enforced rules unchanged, and zero
sampled established non-loopback connections. SQLite count-only inspection found
zero frames, audio chunks and non-notice input events. This was a content-disabled,
tool-free `release-local` check; crash/advisory/tool-enabled coverage remains open.
A background title request timed out before a later
successful title; the model returned correct numbers with unsolicited formatting.
Smart PII pack absence, safe UIA unavailable-state notices, startup item fetching
and a WebView shutdown warning remain known limitations. Other-RPC advisory,
managed Pi crash/restart, tool-enabled retrieval and production packaging remain
untested by this run. Earlier failures and evidence below are preserved.

**Future Pi test preparation, owner-requested:** replace repeated per-trial package
copies with one verified, versioned shared runtime supplied through an explicit,
validated path. Keep trial configuration, chats and session data isolated and add
isolation/integrity regressions. This migration is not implemented or validated;
the completed test is unchanged. Only after validated reuse may redundant,
inactive package copies be removed, with exact-target and non-use checks and all
trial logs, databases, chats, captures and evidence preserved. Active or uncertain
copies and whole trial directories must not be cleaned.

**Local cleanup, 2026-10-06:** the owner explicitly authorized removing the four
inactive completed-trial Pi package copies before shared reuse is implemented;
old trials will not be rerun or reproduced. Exact resolved targets, non-use,
clean-session markers and matching relative paths, sizes and modification times
were verified. The initial hash scan was stopped at the owner's request before
package deletion. Removed 68,616 package files (408,859,368 bytes); retained the
original provisioned runtime. Outside-package trial metadata remained unchanged,
including logs, databases, chats, captures and evidence. Shared-runtime reuse and
its isolation/integrity regressions remain outstanding for future Pi trials.
Six redundant staged model files (708,083,986 bytes) were also removed after
matching the active canonical models; those models and the transfer script remain.
Model comparison had already completed before the no-hashing instruction.
Future trials must reuse canonical model stores rather than duplicate weights.
At the owner's subsequent request, the retired WebView runtime was removed:
917 files (902,728,552 bytes), with matching path/size/timestamp metadata and no
active process references. The current runtime's metadata remained unchanged;
the retired manifest and tools were preserved. No hashes were calculated.
No capture, firewall change, commit or publication was part of this cleanup.

The first provisioned, tool-free GUI conversation attempt failed in the renderer:
the home page called `includes` on a missing `disabledShortcuts` setting. Partial
settings snapshots are now normalized on load and change events, preserving
explicit preferences. A provider-level regression exercises both paths. The
complete frontend Vitest suite passed 436 tests in 46 files; the desktop check
and frontend export/type check passed. A known `unpdf` bundler warning remains.

Source inspection also found settings/chat/Pi work-directory paths bypassing the
native test data root and a calendar publisher starting in the synthetic mode.
The affected renderer paths now use the native app store directory, fail closed
if it is unavailable, and the tool-free mode skips skill installation and calendar
publishing. Existing global files were not inspected, moved or removed. An
explicit recording-directory preference remains separate from the app store root.

The failed GUI attempt passed the 403/403/200 bearer matrix and disabled-capture
health checks. Tray quit produced `clean-v1` and exit zero; the native final audit
and a separate OS inspection found no remaining scoped processes or API endpoints.
The 25 ms monitor recorded three System32 console hosts. A supplementary
held-handle observer failed while replacing its evidence file, so full helper
identity/lifetime validation was not established. A retry observer uses bounded
sharing retries, writes only changed evidence, and checks GUI creation identity;
its live behavior was still pending then. The console host has no global firewall
block; endpoint snapshots are not packet tracing.

The visible **report crash** button only opens a local dialog containing the
error message. It sends nothing and does not open a website. It was traced in
source, not clicked. GUI replies/persistence and complete helper lifetime were
pending then; the other-RPC advisory and managed crash/restart still remain open. The
nine owner-installed executable rules remain in place; no restoration ran.

The second fresh retry stopped before Pi prompts because the renderer's new
absolute-path check required a Tauri command absent from the minimal ACL. That
check now runs locally without path IPC; a regression models the denied command
and verifies it is never called. All 37 affected directory/settings/chat tests
passed. Source preparation also found chat-file access under an isolated app root
needed a dynamic filesystem grant. Setup now grants only its `chats` and
`pi-chat` folders. GUI persistence through those grants was still pending then.
The second run independently passed disabled-capture health, loopback listener,
403/403/200 auth, `clean-v1`, exit zero and OS cleanup; no Pi/console child started.
Calendar publishing was absent. No GUI Pi success is claimed from this attempt.

The owner-requested label change is implemented: both crash boundaries use
**view crash details**, other local diagnostic actions use **view error details**,
and the dialog title is **diagnostic details**, stating that nothing is submitted.
The behaviour remains local. Updated-label visual testing is separate.

## Established results

| Area | Bounded result | Limit |
|---|---|---|
| Windows screen/UI | WGC across three displays, UIA trees, Windows OCR and SQLite writes passed | Not every display/app/provider combination |
| Local API | /health passed; missing/wrong/valid bearer requests gave 403/403/200; tested listener was IPv4 loopback | Protected localhost requests still require authentication |
| Local transcription | Microphone and system-output capture, Parakeet transcription, known-phrase search and transcript persistence across restart passed; a fresh current-build partial-buffer run retained its fixed final-tail marker and found it after process restart | Not a full-passage accuracy, diarization-quality, every-device or same-process restart benchmark |
| Native privacy | Synthetic password markers absent in inspected text/log fields; foreground/background title exclusions and other-monitor fixture controls passed | Browser/custom UIA controls and race boundaries remain open |
| Input switches | Disabled keyboard/clipboard switches produced no input-content rows during synthetic stimuli | Screen/UIA/OCR may still see ordinary displayed text |
| Audio shutdown | A fresh 13.87 s selected-output partial chunk retained both fixed markers; authenticated search found them after a distinct process restart and found a separate control in a new chunk; both shutdowns were clean | Active-meeting shutdown, same-process restart and every device are not covered |
| Lock/unlock | Audio-disabled real transition passed; a later audio-enabled day-to-day run recorded one 10,770.1 s privacy pause with zero frame or audio-chunk counter advances inside its sampled locked interval, then automatic WGC/audio recovery | Counters and fixed notices were inspected, not captured content; this is not every-pixel proof |
| Safe lock notices | Fixed unlocked/desktop-unavailable/locked reasons persisted through timeline API and local logs; clean stop with no owned processes remaining | Packaged desktop timeline was not visually exercised in this run |
| Synthetic pixel redaction | A fresh gated live route check persisted fixed synthetic OCR geometry, changed 100% of the PII ROI while retaining 2.78% of its contrast, preserved ordinary/sentinel controls, returned a source-free missing-OCR placeholder, enforced 403/403/200 bearer behavior and cleaned up | Fixed supplied geometry only; OCR recognition, real-screen content, general PII accuracy and optional RF-DETR detection remain unverified |
| Synthetic DRM | Fresh gated run observed 28 protected-phase samples, retained before/after transcription controls and kept the protected audio/capture markers at zero through recovery; authentication, SQLite and cleanup passed | Synthetic fixture identity and aggregate markers only; real protected media and provider diversity remain untested |
| OS firewall | Exact recorder/media paths had outbound block rules; the fixed no-payload `screenpipe.exe` IPv4 TCP diagnostic was denied while an unscoped control handshake succeeded; helper IPv4/IPv6 localhost access worked | External IPv6 route unavailable; UDP/drop-trace coverage and closeout incomplete |
| Published checkout | Clean offline release build, CLI help/doctor and nine event tests passed; fresh gated runs wrote 15 distinct snapshots, three accessibility-tree frames and Windows Native OCR text, persisted and found both selected-USB-output audio controls, passed scoped browser and native password/exclusion checks, and passed the fixed-geometry authenticated pixel route; SQLite checks and main capture/audio shutdowns passed | OCR-recognized live pixel redaction, arbitrary browser/clipboard providers, real DRM, full audio accuracy and every-device behavior were not exercised; the separate vision-disabled OCR restart required forced shutdown after its watched-process signal |

The final lock result's SHA-256 was
`A82FA50F5DD29402319997CEB75EAD9E70DC6FDC03CF663F86907B2CB1AE0F89`.
Its tested recorder SHA-256 was
`26578A85C37C9D90AC85E84DC4F8BCA9FBB3CA7E46B733361113BEB8182AF0B7`.
These identify historical artifacts, not a claim that a new checkout/build has
been tested. Original incomplete runs were retained. Harness repairs included
fresh search-cache bounds, bounded unknown desktop transitions, serialized
fixture commands, safe exception diagnostics and phase evidence checkpoints.

## Long locked day-to-day trial, 2026-09-21

A 12,166.2-second private run remained locked for one 10,770.1-second interval.
The content-free audit showed zero frame and audio-chunk counter advances inside
the sampled locked interval. Fixed notices recorded secure-desktop/lock entry,
discarded UI/audio work and unlock; WGC and both selected audio streams rebuilt
automatically. After unlock the current process reached 519 captured and 519
database-written frames, 44 audio chunks, seven VAD-positive chunks and 18
completed/inserted transcriptions. No transcript, captured text, image, audio,
clipboard or keyboard value was inspected.

Reported frame drops, pipeline stalls, update failures, transcription errors,
queue capacity/loss notices, persistence degradation, audio-shutdown degradation
and confirmed/possible delivery loss were all zero. Shutdown logged its clean
completion, no panic log was created and the final exact-path inventory found no
recorder, FFmpeg or FFprobe process. Three audio timing-gap warnings were handled
by bounded silence insertion; this result does not establish gap-free audio.

The run retained six source-free active-window acquisition placeholders and four
unresolved frame-link TTL expiries. These remain data-availability warnings, not
evidence of sensitive disclosure. Thirty-six intermittent unavailable UIA
password-state checks failed closed by suppressing keyboard/clipboard content.
The optional smart text-PII pack remained absent, so regex-only reconciliation
continued with its existing reduced-coverage warning.

The audit also exposed false/noisy diagnostics. Local wall time was labelled UTC,
rolling files contained ANSI escapes, any fresh batch-transcription backlog made
`/health` degraded, and an intentional lock pause eventually made audio appear
stale. The launcher missed a real clean-shutdown marker for the same log-format
reason. Source changes now use actual UTC and plain rolling logs, treat only the
existing bounded backlog-stall condition as degraded, keep known privacy pauses
healthy while exposing `transcription_paused`, persist a content-free final trial
summary and split future TTL expiries by direction. Per-frame Basic PII success
and negative meeting scans moved to DEBUG; state changes and failures stay visible.
The repaired tree passed 558 engine tests with two ignored, all 181 audio tests,
the separate rolling-log regression, all 23 capture tests, Rust formatting,
PowerShell parsing and a locked offline optimized build. Focused tests now cover fresh
batch backlog, privacy-paused audio without hiding hard queue/database failures,
actual UTC/plain file-log formatting, per-run log offsets, lock-interval counter
plateaus, ANSI shutdown parsing, failure classification and loss counts. The
non-recording synthetic trial-status regression passed. The exact rebuilt
executable (`D445B05CC5E8B5A5C06C765D9C0D81942D4D3BE2D0F3695D89B6322BE75E0AB3`)
passed its six-model check and the existing-rule trial preflight with recording
disabled. The manual run below verified healthy privacy-paused audio, ordinary
fresh backlogs, plain UTC logs and final-summary creation while exposing further
fresh-directory and silent-audio diagnostic defects.

## Manual day-to-day trial, 2026-09-22

A fresh private 1,455.5-second run produced 47 content-free operational samples.
The final sample reported healthy frame and audio states, 823 captured and 823
database-written frames, zero frame drops/stalls/link expiries/update failures,
44 audio chunks, four VAD-positive chunks and three completed/inserted
transcriptions with zero transcription errors. Two pending transcription
segments were reported by the last sample roughly 30 seconds before shutdown;
no captured database content was inspected to infer their later disposition.
Although the final sample was healthy, 32 of 47 samples were degraded and 31
reported stale audio. Numeric correlation showed that chunks continued arriving
about every 31 seconds and VAD rejected almost all of them as silence. The health
clock incorrectly followed transcript database writes instead of this active
consumer heartbeat.

One 60.2-second lock interval had zero frame and audio-chunk counter advances
inside its sampled window. Unlock rebuilt the selected audio streams and capture
resumed. The launcher and corrected log scan both found clean shutdown, exit code
zero, no panic file, no persistence/audio-shutdown degradation, no confirmed or
possible delivery loss and no remaining recorder/media process.

The run recorded no acquisition-failure placeholders or unclassified errors.
All 829 warnings belonged to known fixed categories: 724 excluded-foreground,
nine excluded-background, 23 no-safe-window, 53 fail-closed unavailable-password-
state suppressions, 15 bounded audio timing gaps, four expected unlock audio
recovery notices and one optional smart-PII reduced-coverage notice. No captured
pixels, text, transcripts, audio, input values, titles, URLs or device names were
read for this audit.

This fresh directory exposed three diagnostics defects. An empty v2 log-offset map
was mistaken for the legacy local-clock format, hiding the lock and shutdown
lines from the generated summary. Version-based parsing and a fresh-directory
regression now fix that case. Comparer-only visual probes also reset the logged
capture state between persisted exclusion placeholders, causing hundreds of
duplicate transition warnings. Probe observations no longer mutate the persisted
transition tracker; a focused regression covers that sequence. A live rerun is
still required to measure the resulting warning reduction. The audio consumer
now advances its existing attempt heartbeat on every received chunk, and health
uses that heartbeat as well as transcript writes. Focused metrics and health
tests cover silent-but-active capture; live confirmation remains outstanding.

## Long manual day-to-day trial, 2026-09-23 to 2026-09-24

A private 8,581-second run produced 277 content-free operational samples. The
audit counted 4,405 captured and database-written frames, 521 UI events and 285
audio chunks. Five chunks passed VAD, nine current-process transcriptions were
completed and inserted, and 205 durable untranscribed segments were reconciled
over 44 sweeps. The final pre-shutdown sample still reported 29 pending segments.
One 798.2-second sampled lock interval had zero frame and audio counter advances.
All 277 API samples were reachable and authenticated. No captured pixels, text,
audio, transcripts, input values, titles, URLs or device names were inspected.

Shutdown did not complete. The log recorded an audio consumer drain timeout and
four unconfirmed recording deliveries; the recorder returned an error instead of
writing its clean-shutdown marker. The old audit used only its final pre-shutdown
health sample and incorrectly reported no audio shutdown degradation or possible
loss. The audit now merges fixed post-sample shutdown and delivery notices, and a
synthetic regression reproduces this ordering. The launcher also gives an
interrupted native process a nonzero inferred exit when no fresh clean marker
exists, instead of serializing an ambiguous null.

Source review located the shutdown delay: completion of an audio session ran the
entire durable transcription backlog inline on the ordered raw-audio persistence
consumer. Reconciliation now runs only on the separate recoverable worker and
receives an immediate wake signal. Shutdown bounds and aborts that worker without
misclassifying already durable segments as lost raw audio. The ordered consumer
remains responsible for confirming raw-audio persistence. Deterministic audio
tests cover the wake and bounded-stop paths; a fresh live shutdown with a backlog
is still required.

The run retained six safe acquisition placeholders: five active-window failures
and one initial privacy-evaluation failure. It also recorded 13 frame-link TTL
expiries, split as seven events without frames and six frames without events.
Windows active-window capture now retries one complete privacy-safe acquisition
after a transient acquisition failure, without retrying or weakening a redaction
decision. Capture drops are classified in the linker, and UI batches deliberately
discarded by privacy or database admission now terminally resolve their frame
correlations. New health and audit fields separate known capture drops from
unexplained TTL expiry. Live confirmation remains required.

The log contained 333 fail-closed unavailable-password-state warnings, 44 bounded
audio timing-gap warnings and two bounded health-computation timeouts. UIA still
suppresses keyboard and clipboard content immediately whenever password state is
unknown, but rapid probe-state flaps are now aggregated into bounded warnings.
A separate fixed diagnostic counts actual content events suppressed by the
password-field gate, including known password focus and fail-closed unknown state.
The next live run must confirm the reduced warning rate and the new suppression
counter without examining content.

## Unsigned desktop GUI acceptance, 2026-10-02

An unsigned Windows desktop build ran for 858.1 seconds against a fresh private
store, an authenticated loopback server on a non-default port and five exact-path
outbound firewall rules. A private WebView2 runtime avoided applying the trial
rule to the machine-wide WebView2 installation. Before capture, one fixed
no-payload TCP attempt from the exact GUI executable was blocked while an
unscoped control reached the same IPv4 endpoint. During the run the GUI process
tree had zero observed established non-loopback TCP connections, zero bound
non-loopback UDP endpoints and one IPv4 loopback API listener. The final
read-only inspection found no tested GUI/media/WebView2 process and no endpoint
on the trial port. This is bounded OS endpoint and one-attempt IPv4 evidence,
not packet-drop tracing, routed IPv6 or UDP proof.

The active recorder configuration matched all expected booleans and exclusion
counts: basic and asynchronous text redaction, DRM pause, keyboard and clipboard
capture, event triggers, all monitors and system-default audio were active.
Missing, wrong and valid bearer credentials returned 403/403/200. The final
content-free sample reported 257 captured and database-written frames, zero
drops, stalls, frame-link expiry or update failures, and 163 linked UI events.
A controlled ordinary input marker persisted once, three clipboard events were
present, and 160 UI events had been inserted at the controlled checkpoint. Five
OCR rows existed, but the controlled screen-text marker had zero OCR matches and
the unified elements table had zero UIA/OCR rows. This run therefore passed WGC,
input and SQLite persistence but did not establish current-GUI UIA/screen-text
capture. No arbitrary captured content was inspected.

Both default audio directions started, local Parakeet loaded, and one fixed
spoken phrase passed VAD, completed transcription, was inserted and matched the
allowlisted phrase query. There were zero transcription errors. The run also
included one 89-second lock interval with zero sampled frame or audio advances;
capture and both audio directions recovered automatically after unlock. Shutdown
returned zero, emitted the fixed completion marker and reported clean vision,
UI-event, redaction and audio-worker completion with no confirmed or possible
delivery loss. The last pre-shutdown sample still reported three durable pending
segments, so this run is not a separate final-tail recovery proof.

The packaged timeline first displayed a connection error against the fresh
store, then loaded when retried several minutes later. Logs showed repeated
WebSocket failures using port 3030 while the test server used its configured
non-default port, followed later by rejected duplicated `data/data` media asset
paths and high-volume frame-navigation errors. An empty or still-populating
timeline may also contribute to the first message; cause is not yet established.
Reproduce the startup timing, dynamic-port WebSocket selection and media asset
path/scope independently before changing product behavior.

The process inventory also observed short-lived Windows PowerShell and conhost
descendants. Source review ties these to local installed-application/icon lookup
commands; their commands enumerate files and Appx packages and do not request
network access. They were not firewall-scoped because those shared system paths
would affect unrelated processes. No endpoint was observed for them, but the
sampling cannot exclude every transient packet; strict all-descendant firewall
proof remains incomplete. The first monitor revision also took 131-159 seconds
between API samples because it repeated expensive CIM and endpoint enumeration
30 times. It now uses process-scoped endpoint queries and a 30-second wall-clock
deadline; its PowerShell and synthetic report regressions pass, but live cadence
confirmation remains open.

The next short GUI attempt passed the fixed acquisition checks before its UI was
stopped: 40 WGC frames reached SQLite, the fixed UIA and OCR controls were each
observed twice, bearer checks returned 403/403/200, and the API listener and
sampled endpoints stayed loopback-only. The timeline later became unresponsive.
Source/evidence correlation found that the reused WebView2 profile had loaded
cached media references from an earlier data store; asset-scope rejection then
produced a high-volume error loop. The process tree also contained short-lived
`setx.exe`, `where.exe` and `conhost.exe` helpers. No endpoint was observed for
those helpers. The owner directed a forced stop, so this attempt did not pass
clean shutdown and cannot close GUI acceptance.

The repaired candidate namespaces timeline caches with an opaque digest of the
active canonical data directory, waits for active API configuration before cache
access and uses a fresh WebView2 profile for each trial. It removes startup
`setx` and replaces `where.exe` path lookup with in-process `PATH` enumeration.
The optimized frontend build, TypeScript validation, 408 Vitest tests, 150 Bun
tests, the content-free GUI harness regression, process-monitor self-test, Rust
formatting and a warning-free locked/offline `release-dev` desktop build passed.
The desktop test cache was moved to the ignored repository-level
`target\desktop-tests` directory and stale moved-cache build-script state was
regenerated. The locked/offline Visual Studio environment then completed the
linked build: both occupied/released port tests and all three Windows
opener-classification tests passed. A fresh interactive run remains required; no
recording or firewall change was made for this repaired candidate.

## Issue register

| ID | Current state / next check |
|---|---|
| SW-V01 | Native foreground DRM policy repaired in source; scoped synthetic Chrome allowed/password/excluded-host transitions passed. Sustained real DRM/audio recovery and broader background/other-monitor transitions remain incomplete. No protected playback bypass. |
| SW-V02 | Explicit audio selection repaired; selected-output live regression passed. Broader fresh/persisted microphone-only, defaults, disable-audio and device changes still need coverage. |
| SW-V03 | **Repaired, regression-tested and scoped live-checked:** earlier runs had two unexpected WGC acquisition failures. The 2026-09-23/24 run retained five active-window and one initial-privacy safe placeholder and later recovered. Windows capture now makes one bounded retry only after an acquisition failure; it never retries a redaction result. The 2026-10-01 native privacy run completed with zero acquisition-failure placeholders. This single clean run does not reproduce the former OS failure. |
| SW-V04 | Microphone transcript rows/phrase hits passed, complete passage accuracy and retention did not. Distinguish ASR substitutions, VAD rejection and actual loss. |
| SW-V05 | Earlier DRM audio tests lacked positive before/after controls; they cannot establish suppression. Retry only with working controls. |
| SW-V06 | Scoped live clipboard evidence passed: an ordinary fixed marker produced three positive rows, the password-phase secret marker produced zero rows, three clipboard UI events were present, and opaque clipboard restoration followed verified recorder stop. Arbitrary providers, applications, formats and race timing remain open. |
| SW-V07 | **Repaired, regression-tested and scoped live-checked:** the orphan lower-level UIA tree producer remains disabled while paired UIA capture and password/focus/input checks stay active. Multi-monitor duplicates are deduplicated. The 2026-09-23/24 run produced 13 TTL expiries. Capture drops now classify their linked event halves, and privacy/database-rejected UI batches terminally resolve their linked frames. The 2026-10-01 native privacy run logged zero unexplained frame-link TTL warnings. Real pressure and longer runs remain open. |
| SW-V08 | Fixture focus/equality limitations addressed with OS identity and stimulus acknowledgements; continue using these guards. |
| SW-V09 | Windows locked-state detection repaired and already-locked/audio-disabled transitions passed. A later audio-enabled run showed a roughly three-hour sampled frame/audio plateau and automatic WGC/input/output recovery after unlock. Broader repeated-lock, DRM and device-change recovery remain open. |
| SW-V10 | General event pressure/recovery/subscriber-loss reporting implemented; 28-event-suite tests passed. Real overload/soak remains open; delivery counts are not unique lost database rows. |
| SW-V11 | **Repaired, regression-tested and partially live-checked:** a 2026-09-23/24 shutdown timed out because session completion ran durable-backlog reconciliation inline on the ordered raw-audio consumer, leaving four raw-audio deliveries unconfirmed. Reconciliation now runs only on its separately bounded, explicitly woken worker. The 2026-10-01 selected-output run stopped cleanly on its first attempt with no shutdown degradation or delivery-loss notice, but it did not deliberately force a durable backlog. A live forced-backlog shutdown, pending recovery, active meeting and same-process restart remain required. |
| SW-V12 | Short stimulus/first-chunk mismatch corrected in preparation; bounded later selected-output tests passed. |
| SW-V13 | Deterministic broadcast-lag integration test repaired; all three locked offline Cargo tests passed. |
| SW-V14 | Nested Windows UIA exclusion gap repaired; scoped silent native privacy retest passed. Broader exclusion variants remain open. |
| SW-V15 | Harness capture-worker readiness race repaired; selected-output live retest passed. |
| SW-V16 | Duplicate lock monitors prevented with one process worker and serialized probes; synthetic tests passed. Repeated live desktop server lifecycle remains open. |
| SW-V17 | **Repaired and live-checked:** partial audio buffer splitting/swallowed final-delivery failure passed deterministic tests and a fresh current-build stop/restart run. The final zero-overlap flush produced one 13.87-second chunk containing both fixed markers; both remained searchable after a distinct process restart. |
| SW-V18 | Typed near-capacity, recovery, confirmed-loss and possible-loss reporting now covers device capture, recording, transcription-result, meeting-tap, meeting-provider, meeting-final and meeting-persistence queues. Fixed content-free notices reach local logs, the general activity timeline and `/capture-events`; deterministic event/audio/engine tests passed. Real overload/soak and the packaged visual timeline remain open. Counts describe queue deliveries, not unique lost database rows. |
| SW-V19 | Audio privacy-transition compensation now covers raw chunk rows/files, combined chunk/transcript/overlap writes, live and reconciled diarization runs/segments, and meeting transcript segments while retaining the database write guard through the stale-generation check and exact cleanup. Deterministic durable-state tests and the full database/audio suites passed. This does not cover the separate speaker-identity mutation in SW-V20 or establish universal transition safety. |
| SW-V20 | **Open, source-review finding:** speaker matching can create a speaker, add an embedding or update a shared speaker centroid while a privacy generation changes. Safely reversing a shared identity requires transactional before-images and ownership-aware compensation; deleting a pre-existing/shared speaker would be unsafe. |
| SW-V21 | Model inspection distinguishes missing, wrong-type and access failures; warnings are transition-deduplicated; `/health` marks requested-but-unavailable transcription; and the trial refuses to record unless the exact executable verifies the local pack. A live post-fix run completed and inserted 18 current-process transcriptions with zero transcription errors; content and accuracy were not inspected. |
| SW-V22 | The same trial had one post-capture privacy-evaluation failure; it persisted a visible safe placeholder and recovered on the next observed frame. No sensitive content was inspected and the generic failure stage does not identify the transient OS cause. Retain as an open reproducibility/observability warning; do not weaken the fail-closed behavior. |
| SW-V23 | The optional smart text-PII model was absent and regex-only reconciliation remained active. Its published CC BY-NC 4.0 license is separate from this MIT repository, so it was not silently provisioned for this Digiwise-associated workflow. Reduced coverage is now explicit in the log and setup guide; neither regex nor AI redaction is a privacy guarantee. |
| SW-V24 | **Repaired, regression-tested, rebuilt and live-checked:** `/health` treated any fresh batch backlog and expected privacy-paused audio staleness as degradation. It now uses the existing bounded backlog-stall predicate, treats a known privacy pause like the vision gate and exposes `transcription_paused`. Two samples during the 2026-09-22 lock pause remained healthy with unchanged frame/audio counters. Focused tests also ensure a pause cannot hide confirmed queue/database failures. |
| SW-V25 | **Repaired, regression-tested and rebuilt; fresh-directory parser rerun pending:** rolling log timestamps now use UTC when suffixed `Z`, file logs omit ANSI, shutdown scanning uses per-run byte offsets, and the launcher writes a content-free final summary. The 2026-09-22 run verified plain UTC logs and summary creation but exposed SW-V26. The Rust file-format test and non-recording synthetic status test cover the corrected parsing paths. Existing legacy runs remain readable. |
| SW-V26 | **Repaired, regression-tested and rebuilt; live check pending:** the fresh 2026-09-22 trial had an empty v2 offset map, which the status script misclassified as legacy and therefore omitted UTC lock/shutdown lines. Launch schema now selects legacy parsing, and the synthetic test covers both non-empty and empty v2 offset maps. |
| SW-V27 | **Repaired, regression-tested and rebuilt; live check pending:** comparer-only visual probes repeatedly reset persisted Windows capture transition state, producing 724 excluded-foreground warnings in one 24-minute run. Probe results no longer mutate or log the persisted state; actual persisted privacy and failure transitions remain visible. |
| SW-V28 | **Repaired, regression-tested and rebuilt; live check pending:** 31 samples reported stale audio while chunks continued arriving and were VAD-rejected as silence. Received chunks now advance the intended consumer/transcription-attempt heartbeat, and `/health` accepts a recent heartbeat without requiring recognized speech or a database insert. |
| SW-V29 | **Repaired and regression-tested; live check pending:** the trial summary previously ignored shutdown failures and delivery-loss notices written after the final health sample. It now merges fixed post-sample diagnostics, reports the exact safe reason codes, and treats a missing clean marker plus null native exit status as failure. |
| SW-V30 | **Repaired, regression-tested and full-application live-checked:** UIA password-state probe flapping produced 333 repetitive warnings. Fail-closed suppression remains immediate, while state-unavailable warnings are bounded and aggregated. A real-hook/real-UIA crate regression exercised two fixed ordinary/password cycles: both ordinary markers were delivered, both password markers remained absent, and all 14 characters in each password phase incremented the unavailable-decision suppression count. The corrected full-application run persisted one ordinary keyboard control, zero password typing or password clipboard markers, and 23 safe suppression notices; authentication, SQLite integrity, clipboard restoration and clean process shutdown passed. Arbitrary providers and all race timings remain untested. |
| SW-V31 | **Repaired, regression-tested and scoped live-checked:** active IPC remains authoritative over persisted port hydration; media uses the configured final directory and active recursive asset scope. After an earlier run reused stale media paths and became unresponsive, cache keys gained an opaque active-data-directory namespace, cache loading began waiting for active API configuration and every trial received a fresh WebView2 profile. In the fresh follow-up, the owner loaded Timeline without the prior connection error, the recorder wrote 395/395 captured/database frames, and shutdown was clean. The visual check did not inspect arbitrary captured content or establish every media/cache path. |
| SW-V32 | **Repaired, regression-tested and scoped live-checked:** asynchronous text reconciliation preserves sanitized accessibility structure only for byte-identical output and still clears derivatives fail-closed when stronger redaction changes text. In the short GUI follow-up, both fixed public UIA and OCR controls were observed twice while 40 WGC frames reached SQLite. This establishes the allowlisted fixture on that build, not general OCR recognition or accessibility-provider coverage. |
| SW-V33 | **Repaired, regression-tested and scoped live-checked:** icon discovery no longer launches PowerShell, startup no longer invokes `setx`, and Pi/FFprobe lookup enumerates `PATH` in-process instead of launching `where.exe`. The fresh follow-up process monitor took 5,048 snapshots at 25 ms, classified eight expected WebView descendants and observed zero unexpected shell starts. Sampled non-loopback TCP/UDP endpoints were zero, shutdown was clean and no scoped process remained. This does not establish unsampled packet behavior or a provisioned Pi child-process scope. |
| SW-V34 | **Production hardening repaired, built and regression-tested:** full startup no longer shell-discovers or forcibly terminates a process that owns the configured API port; an in-process loopback bind waits only for the app's prior socket release and otherwise fails with a safe diagnostic. Windows path/URI opening no longer uses `cmd.exe`; it requires an absolute local path or explicitly allowlisted `shell:AppsFolder`/`ms-settings` URI and uses the native opener. Source-level harness regressions, formatting and a warning-free optimized build passed. Both linked occupied/released port tests and all three linked opener-classification tests passed in the locked/offline Visual Studio environment using the repository-local desktop test cache. |
| SW-V35 | **Repaired, regression-tested and optimized-build checked; visual confirmation pending:** capture placeholders previously exposed only generic redaction/failure labels. Windows records now persist one of eight fixed safe markers and Timeline maps only exact allowlisted markers to content-free explanations for active-window-only masking, exclusion/focus policy decisions and acquisition/privacy-check failures. Focused Rust and Vitest coverage and the combined `release-dev` build passed. No window title, URL or captured value is included, and the reason category does not claim that specific displayed content was sensitive. |
| SW-V36 | **Setup helper regression-tested and owner live-copy checked:** missing Pi prepares an embedded standalone helper under the active data directory and displays an absolute PowerShell-quoted command. A checkout is no longer required. Six isolated Rust tests, the linked desktop recovery-message test and a synthetic provisioning test passed, including conflicting-file preservation and junction refusal. On Oct 5 the owner verified complete command copying, selectable instructions, 15-second persistence and explicit dismissal in the pinned `release-local` GUI under verified firewall scope. The copied command was not run. Real package acquisition, AI-provider provisioning, installer behavior and Pi execution remain separate and untested. |
| SW-V37 | **Banner and Pi copy defects repaired, regression-tested and owner visually confirmed:** the safe reason now has its own row above the measured media viewport. The owner confirmed no image/banner overlap and successful Pi command copying, selection, persistence and dismissal in the Oct 5 `release-local` retest. Nine focused frontend tests and TypeScript/export checks had passed. Tray quit returned zero, left a clean marker and no scoped process or active listener; no unexpected shell helper was observed. A separate controls overlay partly obscured the bottom Recording status label; SW-V40 tracks its structural repair and subsequent owner visual confirmation. Native Pi inventory remains advisory and never kills unknown processes. |
| SW-V38 | **Background tested; scoped full-GUI interruption/restart passed:** a per-directory OS-locked marker distinguishes clean and interrupted sessions. The Oct 5 release-local check preserved an active marker after deliberate interruption, persisted the fixed recovery notice on restart, and ended with owner-confirmed timeline text, tray exit zero and a clean marker (SW-V42). Known cleanup failure or exit during resource startup leaves it active; further startups are refused once quitting begins. Unknown bytes/unsafe links are preserved and stop startup. Windows Job containment has module coverage; real Pi descendants were not exercised by this GUI run. Existing SQLite/audio recovery remains separate; pre-marker sessions and CLI lifecycle tracking are not covered. |
| SW-V39 | **Trial-monitor reporting repaired; targeted regression passed:** PowerShell JSON date decoding caused culture-formatted notice queries to return 400, and post-exit `TIME_WAIT` entries were incorrectly flagged as active endpoints. UTC RFC3339 normalization restored the live safe-notice query; active endpoint filtering retains live/unknown states and reports closed remnants separately. The synthetic GUI harness passed in PowerShell 7; Windows PowerShell 5.1 script execution was policy-blocked and its policy was preserved. Historical samples remain intact. The retest also recorded one unexplained frame-link expiry requiring a separate content-free investigation; no captured payload was inspected to diagnose it. |
| SW-V40 | **Filter-column overflow repaired and owner visually confirmed:** the bottom-inset placement could let a tall controls column escape above the slider and cover the preceding Recording status row. The production viewport now anchors at the slider top and scrolls within 60 pixels. Six rendered-component regressions passed for empty/short/tall parent layouts. The owner confirmed the Recording status label looked good in the last Oct 5 GUI recovery run. The visual follow-up is complete for that layout; content capture was disabled, so broader recorded-media layouts are not claimed as tested. |
| SW-V41 | **Correlation-delivery observability added; expiry cause unresolved:** all identified nonblocking linker sends now distinguish full/closed outcomes for four fixed message classes, with count-only health fields and bounded fixed warnings. Sixteen focused linker tests passed, including reverse-order discard and actual full/closed/success sends; trial-status regression covers all eight failure counters before expiry and older builds without the fields. Correlation failure does not itself prove an activity row was lost. No live reproduction or cause of the earlier single unexplained orphan is claimed. |
| SW-V42 | **Controlled GUI recovery passed, with content disabled:** the Oct 5 pinned release-local run used a fresh isolated directory and unique nonmatching UI filter. Deliberate exact-process interruption left the active marker; launcher/monitor completion preceded a single verified restart. The token, controlled settings and initial SQLite notice survived. Exactly one fixed recovery notice persisted, the owner confirmed it in Timeline, and tray quit returned zero/clean-v1. Both phases passed 403/403/200 authentication, with no sampled non-loopback endpoint, unscoped helper or leftover scoped process; independent final OS inspection confirmed no scoped process or active API endpoint. Read-only aggregate checks found zero content rows and final SQLite quick_check passed. A disabled-health-section harness assumption was corrected during the same attempt without changing app/capture settings; 83 PowerShell regression groups and one Python read-only helper regression passed. Summary attention status remains for known UIA/PII/startup/exit warnings; no panic, notice persistence degradation or delivery loss was reported. Firewall rules remain installed. Real Pi, acquisition/audio crash tails and production-release claims remain separate; the owner subsequently confirmed the Recording status label (SW-V40). |

Pi provisioning follow-up (2026-10-05, SW-V43): the pinned MIT Pi 0.75.4 package
installed explicitly (123 dependencies; two postinstalls left blocked), and the
owner-selected Apache-2.0 `ministral-3:latest` model was downloaded through the
existing Ollama 0.34.0 service. Its recorded digest is
`1922accd5827ebe6829e536369195db25eaf664528dc66206d646ea3bb386b71`.
Bundled Bun reported Pi 0.75.4. The managed child now overrides startup acquisition,
update and telemetry opt-ins; desktop compile and the focused command-environment
regression passed, with zero external dependency rebuilds. After owner-confirmed
provider rules were independently verified on Oct 6, a fresh tool-free Pi RPC
run passed two short arithmetic replies, ordered synthetic assistant persistence,
EOF exit zero and exact verified System32 console-host exit. Scoped Bun's fixed
TCP probe failed while the unscoped control connected; enforced native rules and
loopback listener inspection supplemented final zero non-loopback TCP/UDP counts.
Earlier zero-child and model-format failures remain evidence; six Python and 17
firewall-validator cases now pass. The console host is a verified OS helper outside
the application firewall scope, not proof of its packet isolation. A bounded
tool-free GUI mode is prepared: three focused Rust tests, launcher/staging
regressions, DesktopCheck and the release-local build passed with zero external
dependency rebuilds. The previous unrelated DesktopFmt failures remain.
GUI-managed conversation, other-RPC advisory
and normal/crash cleanup remain untested. No capture or GUI launched, no rules
removed and no commit/push occurred. Provisioning and headless RPC do not establish
GUI containment or useful activity retrieval.

## Automated interactive repair validation, 2026-10-01

The exact rebuilt release executable identified in `BUILD_NOTES.md` passed a
fresh 71-check normal-user preflight with the existing exact-path outbound block
rules. Preparation first exposed stale staged controller sources: the preflight
could validate a configured helper hash without verifying the helper file that
the launchers would execute. Preflight now resolves that helper, checks its
SHA-256 before creating an evidence directory, and records the successful pin
check. Source staging tests cover this ordering. After source-only restaging and
local fixture rebuilding, 48 manifest checks, 15 staging tests (one expected
symlink-privilege skip), 112 controller tests and 14 plan tests passed.

A fresh selected-output run wrote six audio chunks and eight transcription rows.
Each of the two fixed local speech markers appeared once in persisted search
results. Missing, wrong and valid bearer credentials produced 403/403/200,
`/health` returned 200, SQLite `quick_check` returned `ok`, and shutdown completed
on the first attempt. Fixed-pattern log inspection found no audio-shutdown
degradation, consumer-drain timeout, delivery-loss/possible-loss, persistence
degradation, acquisition-failure placeholder or frame-link TTL warning. This run
did not force a durable transcription backlog at shutdown and is not a complete
speech-accuracy or final-tail test.

A separate silent native privacy run completed its allowed, password,
excluded-foreground, excluded-background and final allowed phases. Allowed frame
deltas were two before and one after the protected phases; protected phases had
zero allowed-marker deltas, and every forbidden synthetic-marker delta remained
zero. The store held 14 frames and five UI events and passed SQLite
`quick_check`; authentication again produced 403/403/200 and shutdown was clean.
Log inspection found zero acquisition-failure placeholders, unexplained
frame-link TTL warnings, persistence degradation or panic files. One logical
password-state-unavailable warning appeared on two log surfaces. Because the run
was silent and input-free, it did not exercise the keyboard/clipboard
actual-suppression counter, image-pixel redaction, real DRM or arbitrary providers.

A subsequent native input run initially failed safely because the synthetic
fixture's managed `SendInput` structure did not match the 64-bit Windows ABI. No
forbidden marker persisted, cleanup passed, and the controller reported the run
as incomplete. The fixture now includes the native union's larger `MOUSEINPUT`
member and self-tests the expected 40-byte x64 layout. In the corrected fresh run,
one fixed ordinary keyboard marker persisted in `ui_events`, both fixed password
typing and password clipboard marker deltas were zero, and the safe password-
suppression counter increased by 23. Missing, wrong and valid bearer credentials
again returned 403/403/200, SQLite `quick_check` returned `ok`, and recorder stop,
clipboard restoration, fixture closure and exact-process cleanup all passed.

The final-tail controller then forced graceful stop while a selected-output
partial buffer was still below its normal 62-second emission threshold. The
pre-stop checkpoint contained zero chunks. Shutdown produced exactly one
13.87-second chunk, and both fixed markers joined that chunk. A distinct process
reopened the same fresh store, authenticated with the same 403/403/200 matrix,
found both original markers, persisted a separate restart control in chunk 2 and
shut down cleanly. Neither phase used force, reported unresolved workers or
shutdown issues, recovered a device, or crossed a privacy transition. Fixed-
pattern inspection found zero queued-work-discard, unconfirmed-worker, incomplete-
shutdown, persistence-degradation, possible-loss or panic markers. An independent
exact-path check found no tested process or listener after completion; only closed
loopback connections in `TIME_WAIT` remained temporarily under PID 0.

After the earlier audio and native-privacy controllers exited, an independent
normal-account exact-path inventory found no tested recorder, FFmpeg, FFprobe or
fixture process, no TCP or UDP endpoint owned by those paths, and no listener on
either configured test port. Captured screen, audio and transcript bodies were
not inspected; only fixed synthetic-marker counts and content-free diagnostics
were used.

## Outstanding test plan

| ID | Required scope |
|---|---|
| SW-T01 | Extend the passed audio-disabled lock transition to audio cancellation/recovery and stale-generation suppression; retain independent OS observations. |
| SW-T02 | **Scoped complete:** the exact Chrome rule and 74-check preflight passed; fixed allowed browser controls persisted while the browser-password and excluded-host markers remained at zero. Exact-path Chrome inventory and cleanup passed. This does not establish universal browser secrecy or actual firewall packet drops. |
| SW-T03 | **Scoped synthetic run complete:** after an initial pre-recording asset-pin refusal, the rebuilt fixtures were reviewed, repinned and passed the 71-check preflight. The fresh gated rerun observed 28 protected-phase samples, persisted one before and one after speech control, and kept the protected audio marker and all forbidden capture markers at zero through the final recheck. Authentication, SQLite integrity, first-attempt shutdown, fixture cleanup and independent process/endpoint checks passed. This is synthetic fixture evidence; sustained real protected playback remains separate and no DRM bypass is permitted. |
| SW-T04 | App-only and App::Title exclusions, rapid transitions, enumeration failure and race stress. |
| SW-T05 | **Scoped complete:** fixed ordinary clipboard text persisted, the password-phase secret marker remained at zero, recorder stop preceded opaque clipboard restoration, and cleanup passed. Live provider-failure, unknown-focus and non-text-format faults remain untested beyond deterministic checks. |
| SW-T06 | **Scoped complete:** three pinned local speech WAVs and the preferred 60-second collector passed 77 offline tests and a 16-asset pin closure. The fresh gated live run stopped before normal emission, retained its baseline and final-tail markers together in one 13.87-second partial chunk, found both after a distinct process restart, persisted a new restart control and completed both shutdowns cleanly without loss/degradation markers. Active-meeting shutdown, diarization quality and same-process restart remain separate checks. |
| SW-T07 | The long locked run covered combined WGC/UI/input/audio/STT counters, lock recovery and loss diagnostics. OCR output, unplug/replug, monitor changes, active overload and content accuracy still require controlled coverage. |
| SW-T08 | Fixed no-payload `screenpipe.exe` IPv4 TCP attempt passed blocked-versus-unscoped control. Usable external IPv6 and UDP controls plus OS drop tracing remain open. |
| SW-T09 | Desktop/WebView/MCP/optional executable scope: inventory relevant child processes before extending firewall claims. |
| SW-T10 | Owner-confirmed disposition/restoration of exact test rules, independent read-only final inspection and original milestone closeout. |
| SW-T11 | Visual desktop status and broader safe-notice audit across acquisition/audio/input failures and independent queues. |
| SW-T14 | **Scoped fixed-geometry live check complete:** after 140 relevant offline tests and a fresh 71-check normal-user preflight, three synthetic snapshot rows exercised the authenticated on-demand frame route. The clear positive control was byte-identical; 100% of the fixed PII ROI changed and retained 2.78% contrast; ordinary/sentinel changes were 0%/0.04%. A separately verified missing-OCR row produced a source-free placeholder changing 99.20% of the frame. Bearer checks, SQLite integrity, shutdown, listener closure and independent process/endpoint checks passed. Three focus/app/status UI rows were counted but their contents were not inspected. OCR recognition, real-screen privacy, async RF-DETR and general PII accuracy remain open. |
| SW-T12 | Extend the passing deterministic privacy-generation tests with live transition timing and the SW-V20 speaker-identity path. Inspect durable state as well as returned results; do not generalize the fixed persistence paths to universal stale-generation suppression. |
| SW-T13 | **Scoped complete:** diagnostic evidence showed every ordinary denial in the failing full-app run was stale. A narrower real-hook/real-UIA test separated provider-call duration from completed-probe gaps and reproduced tree work delaying the shared STA beyond the 75 ms permit. Password polling now has a dedicated STA and 25 ms target interval; the permit lifetime and fail-closed checks are unchanged. Two automatic ordinary/password cycles delivered 2/2 ordinary markers, 0 password markers and 14 suppressions per password phase, with zero ordinary denials and a 46.8 ms maximum probe gap. The corrected full-app controller then persisted one ordinary UI-event control, zero fixed password markers and 23 suppression notices, with authentication, SQLite integrity, clipboard restoration and clean shutdown passing. Arbitrary controls/providers and all race timings remain outside this scoped result. |

## Reuse and conduct

Use [the configurable test sources](scripts/windows/interactive-validation/README.md)
and [Windows setup](docs/WINDOWS_SETUP.md). Prepare in the background; request fresh
readiness with recording stopped and no full-screen takeover. No captured private
contents may be read without current authorization. Use explicit model/tool setup,
keep auth enabled, preserve loopback, and never change global firewall settings.
The three original local firewall rules remain installed pending owner disposition;
this public summary deliberately does not provide machine-specific removal commands.
A newly configured harness must pass its offline checks and a new authorized live
run; historical passes do not validate its portability changes.

## Scoped browser and clipboard validation, 2026-09-18

The first gated attempt ended incomplete before any phase with
`process_inventory_unavailable`. Chrome had exited between a Toolhelp snapshot
and its identity query, and child processes briefly outlived the root process.
Cleanup still stopped the recorder and fixture, restored the clipboard, closed
the loopback server and reached later exact-path process quiescence. The harness
was repaired to retry only that bounded exit race while retaining fail-closed
behavior for persistent unknown identities, and to allow exact-path Chrome
children a bounded drain after root shutdown. The complete controller suite then
passed 105 tests and the live preflight passed all 74 checks.

The fresh retry completed all five verified phases. Fixed public browser markers
had positive deltas of one each, the ordinary clipboard marker had a positive
delta of three, and the allowed clipboard control had a positive delta of one.
The browser-password, excluded-host and password-clipboard secret markers all
remained at zero in the final aggregate query. The store contained 12 frames and
eight UI events (two app switches, three clipboard events, one privacy notice and
two window-focus events); `PRAGMA quick_check` returned `ok`. Missing, wrong and
valid bearer requests returned 403/403/200 for each protected endpoint, `/health`
returned 200, and the recorder reported no persistence degradation.

Cleanup passed for the recorder, clipboard, exact-path browser process tree,
loopback fixture server, focus release, native fixture and final process
quiescence. A separate post-run inventory found zero scoped processes. Network
inspection found no listening or established TCP connections and no UDP endpoint
for the scoped paths; 45 TCP entries were only `TIME_WAIT`. The exact Chrome
outbound block rule was active with the reviewed non-loopback IPv4/IPv6 scope.
The batch made no deliberate browser-originated outbound attempt and collected no
packet-drop evidence (`outbound_attempt_samples` was zero), so this result must
not be described as proof that Chrome was offline. Only aggregate counts for
fixed synthetic markers were inspected; captured contents were not read.

## Status and deletion integration release validation, 2026-10-08

Twenty-four frontend tests, type checking/static export, three synthetic database
byte/store tests, five context-ordering fixture checks, both formatting presets
and DesktopCheck passed. Source review corrections cover delivery/shutdown alerts,
request-scoped cleanup results, unknown outcomes and cache invalidation.

Settled production builds passed with existing caches and reviewed baseline
adoption for the pinned Tauri feature change. Root: 6m31s, 785 external reused/
zero rebuilt, 12 workspace reused/seven rebuilt. Desktop: 11m01s, 1,065 external
reused/15 expected new variants, ten workspace reused/eight rebuilt, zero unexpected
rebuilds. Low memory headroom required staggered heavy builds despite opt-in
concurrency. The actual Tauri generated context and all 494 asset outputs predate
the application's dependency reference; next-run cache freshness is not claimed.

| Artifact | SHA256 |
|---|---|
| Recorder release | 7F6F38C2F3292A8B0F9C691A8C360C54DD7F01D47476AA856DC5D61CC05DEB2E |
| Desktop release | 5855BE901559CC9DD727603B2B3A2E2C7AE391DB5D894BA789D7388142616B9F |

The rebuilt recorder passed inert CLI checks. No desktop launch, live capture,
production-store read/migration, deployment, restart or firewall change occurred.
Runtime microphone signal, monitor transitions and installed-package preflight
remain unvalidated. Complete permanent Discard and pending-review access control
remain outside these scoped repairs. See the three milestone documents and the
updated data control discussion. No production performance or PS5.1 claim is made.
