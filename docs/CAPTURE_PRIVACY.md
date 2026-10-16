# Windows capture decision metadata

Capture pauses and producer conditions now have independent [all-channel interval diagnostics](CAPTURE_DIAGNOSTICS.md), including safe exclusion-rule references. Per-frame metadata below remains a separate observation surface.

New event-driven Windows frames store a JSON object in `frames.capture_privacy`.
It is written atomically with the frame and returned as `capture_privacy` on OCR
search results (`GET /search?content_type=ocr`) and `GET /frames/{id}/context`.
These routes require the local bearer token. OCR search includes accessibility,
hybrid and privacy-placeholder rows, not only text recognized from pixels.
Old rows and other capture producers have `null`; this does not mean capture
was allowed or that no excluded applications existed. There is no backfill.

For example, this **fabricated** decision says an excluded Excel background
window prevented full-monitor acquisition while a safe foreground window was
captured on the same monitor:

```json
{
  "schema_version": 1,
  "outcome": "active_window_only",
  "reason": "excluded_background",
  "blockers": [
    {
      "app": "excel.exe",
      "reasons": ["configured_exclusion"],
      "foreground": false
    }
  ],
  "blockers_truncated": false,
  "foreground_monitor": "monitor_2",
  "is_active_monitor": true,
  "foreground_monitor_changed": false
}
```

`app` is a bounded executable basename obtained from Windows process metadata,
lowercased with `.exe`. It never uses a window title or application description.
Unknown, inaccessible or unsupported names are `null`. It is application
identity metadata and can reveal which applications were running; it contains
no document names, window titles, URLs, configured patterns or executable paths.
Generic timeline notices and local logs keep their existing fixed messages.

`blockers` deduplicates by executable and foreground/background role. It preserves
restrictions observed both before and after acquisition and immediately before
UIA, even when they disappear during the operation. It is limited to 32 entries;
`blockers_truncated` explicitly reports omitted entries. Failed placeholders now
sample native foreground HWND/PID before and after executable lookup, adding a
`foreground: true` blocker with fixed reason `capture_failure`. This is the app
observed at failure, not an exclusion match, a provider-fault attribution or the
owner of an earlier inspected tree. Unknown/changing identity stays null; titles
remain absent. The sample is not included in diagnostic `excluded_apps` and never
comes from previous input/focus events. See the
[post-deployment consumer follow-up](CONSUMER_REPORT_20261013.md).
Monitor samples surrounding this extra lookup merge with earlier provenance;
disagreement or unavailable association makes the active-monitor fields unknown.

Foreground blockers are now observed globally, including an excluded foreground
window on another monitor when this monitor has no permitted foreground fallback.
`foreground=true` means the global foreground window at an observed policy phase;
it does not mean that window overlaps this frame's monitor. Other-monitor
background apps are omitted. A global excluded foreground app does not itself
exclude pixels on all monitors: the top-level outcome/reason explains the local
decision. Both configured matches and unavailable metadata remain explicit, without
excluded window titles or filter strings. If the foreground window cannot be
enumerated, the verified executable may be reported with
`foreground_window_unavailable`; this is uncertainty, not a configured exclusion.

## Active-monitor provenance and filtering

`foreground_monitor` identifies the Windows monitor containing the global
foreground window, using the same `monitor_<id>` spelling as frame `device_name`.
It follows Windows monitor ownership for a window spanning displays, not the
mouse position or configured primary display. IDs are local/session provenance,
not portable physical-display identities. No nearest monitor is invented for a
window that Windows cannot associate with a monitor.

`is_active_monitor` is `true` when the captured monitor owns the foreground window,
`false` when a verified different monitor does, and `null` for unavailable or
changing association. Redacted frames retain this metadata too; it does not
imply capture permission, input activity, visibility of every pixel or work time.
Policy enumeration samples focus before/after; acquisition and pre-UIA evaluations
are merged, with a final monitor check after UIA. If any sampled monitor/knownness
differs, `foreground_monitor_changed=true` and both identity/boolean become null
for that frame, even if focus returns. This is sampled provenance, not proof that
no transition occurred between checks. A changed foreground window during policy
enumeration produces an inconsistent-focus placeholder and the fixed blocker code
`foreground_changed_during_evaluation`.

Use authenticated `GET /search?content_type=ocr&active_monitor=true` for a first
pass containing known active-monitor screen/UIA/OCR **and placeholder** records.
`active_monitor=false` selects known other-monitor records. Filtering occurs
before pagination and applies to `pagination.total`; omit the filter for all
records. Null, missing, malformed and explicitly changing associations match
neither boolean filter. The option requires `content_type=ocr`; incompatible
types are rejected. Other modalities and historical coverage require separate
queries. A filtered empty result does not establish an empty or inactive period.
Old records have no active-monitor provenance and need an unfiltered fallback.

| Outcome | Meaning |
|---|---|
| `full_monitor` | The Windows window-exclusion policy admitted monitor pixels. |
| `active_window_only` | Allowed foreground-window pixels on this monitor; the rest is blacked out with a disclosure banner. |
| `redacted` | Source-free disclosure frame; real pixels and text extraction withheld. |
| `capture_failed` | Source-free failure frame; acquisition or policy evaluation failed. |

Top-level reasons distinguish `excluded_background`, `active_window_excluded`,
`no_safe_active_window`, `inconsistent_focus`, and acquisition stages
`initial_privacy_evaluation`, `monitor_acquisition`, `active_window_acquisition`,
`post_capture_privacy_evaluation`. `full_monitor` has no top-level reason.
These describe window privacy admission, not subsequent image/text PII redaction,
lock notices or the complete recording lifecycle. Query recording-status events
separately to understand pauses, failures and gaps.

Blocker codes identify configured exclusions (`configured_exclusion`), include
filter rejection (`outside_include_filter`), built-in exclusions
(`builtin_exclusion`, `recorder_ui`), unverified window/URL metadata
(`window_metadata_unavailable`, `browser_url_unavailable`), URL exclusions
(`configured_url_exclusion`), and unsuitable foreground fallback surfaces
(`shell_surface`, `builtin_application_skip`). Later UIA gates use
`private_browsing`, `ignored_window`, `not_in_include_list`, `blocked_url`,
`builtin_application_skip` or `monitor_unverified`. No excluded pattern or URL
is included to explain a match.

`no_safe_active_window` records (from 2026-10-16) also carry `no_safe_window`,
which says why no window could be captured on its own:

| Field | Meaning |
|---|---|
| `phase` | `initial` or `post_capture` evaluation |
| `path` | `scoped` (foreground-window plan) or `selected_window` (maximised-window plan) |
| `cause` | On a monitor without the foreground window: `no_safe_window_on_monitor` (no window there may be captured alone) or `safe_windows_hidden` (each such window lies beneath a maximised or monitor-covering window that may not be captured). Otherwise `foreground_unsuitable` (the foreground window may not be captured alone), `selected_not_enumerated`, `selected_minimized`, `selected_not_on_monitor`, `selected_unsuitable` or `unclassified` |
| `foreground_skip` | Fixed reason the foreground or selected window was unsuitable (`shell_surface`, `builtin_application_skip`, `window_metadata_unavailable`) |
| `window_app` | Verified executable basename of the foreground (scoped) or selected window, else `null` |
| `foreground_on_monitor` | Whether the native foreground window is on this monitor; `null` when unknown |
| `visible_windows`, `excluded_visible_windows` | Enumerated windows overlapping this monitor, and how many were excluded or unverified |

It never contains titles, URLs, window handles or geometry, and it is omitted
for every other outcome. A `selected_not_enumerated` cause means the maximised
window chosen by z-order is absent from the capture library's window list (for
example a tool window without a title, or one owned by the recorder).

**Monitor without the foreground window (owner policy 2026-10-16).** When an
excluded or unverified window overlaps such a monitor, ScreenWise no longer
withholds the whole monitor if another window there may be captured alone. It
captures the topmost such window (z-order) as `active_window_only`, with the rest
of the monitor blacked out, and marks the record
`"active_window_choice": "topmost_safe_window"`. Windows Graphics Capture records
only that window's own surface, so a partly overlapping excluded window is never
copied into the bitmap. A maximised or monitor-covering window that may not be
captured hides everything beneath it, so the monitor is then withheld
(`safe_windows_hidden`). This frame has no UIA inspection and no app or title
labels (as for other-monitor full captures); content screening and the
post-capture recheck still apply. A local foreground window keeps priority: if it
may not be captured alone, the monitor is withheld rather than falling back to a
background window.

An empty window title counts as `window_metadata_unavailable` only when a
configured rule could read the title for that app: an unscoped or `::Title`
ignore pattern, an `App::Title` pattern for that app, an include list or ignored
URLs (title heuristics). With only `App::` ignore patterns the decision is the
same for every possible title, so an empty title is not uncertainty. The same
rule decides whether an untitled window may be captured alone (owner request
2026-10-16). An empty app name is always uncertain. Unscoped ignore patterns
match the app or the title, so with any unscoped pattern configured, untitled
windows remain unverified; `App::` patterns avoid that.

`untitledSafeApps` (Settings → Privacy → Apps with untitled windows, empty by
default) lists executables, matched exactly (case-insensitive, `.exe`
optional), whose untitled windows count as verified anyway. Their app name is
still checked against every rule; titled windows are unaffected.

**Taskbars (owner policy 2026-10-16).** Explorer's taskbars (`Shell_TrayWnd`,
`Shell_SecondaryTrayWnd` owned by `explorer.exe`) are blacked out in every
admitted capture: full-monitor, active-window-only and maximised-window frames.
Their bounds are sampled before and after acquisition and once more before the
frame is returned, and the union is masked (an auto-hide taskbar can move). If a
visible taskbar's bounds cannot be read, the frame fails closed
(`post_capture_privacy_evaluation`). A taskbar in the background is therefore
never a blocker and never forces active-window-only capture. A focused taskbar
(just clicked) keeps the existing checks, because its accessibility tree names
other windows, including excluded ones. Flyouts, previews, the Start menu and
notifications are separate windows and are not masked; they follow the normal
rules.

## No contact with excluded apps (2026-10-16)

An app or window excluded from capture is also not contacted: no UIA call,
URL lookup or clipboard render is directed at it. One shared gate,
`screenpipe_capture_policy::target`, answers this from native identity alone
(executable name and window title, read without calling into the target). It
covers protected apps (canonical password managers, credential viewers and
system UI hosts), the configured ignore list and the include list. An
unidentified process is not contacted. Each capture path adds its own
evidence-based rules (URLs, private windows, DRM, nested windows) on top.

Callers on Windows:

- accessibility tree walk: the protected-app list now comes from the shared
  gate (unchanged entries, plus the macOS/Linux keychain names);
- keyboard privacy probe: judged on the foreground window's raw and
  normalised app names before any UIA call; an excluded app denies input
  (its keystrokes were already discarded by the hook);
- click context: read only when the click lies inside the foreground window
  and that window passes the gate (no `WindowFromPoint`, which messages the
  window);
- focused element after a tree capture: read only while focus is still the
  window that passed the filters;
- clipboard text: read only when the clipboard owner's process passes the
  gate. The owner is usually a hidden window, so only rules that need no title
  apply to it; title-scoped rules (`App::Title`) rely on the copy-time check;
- browser URL lookup: skipped for an excluded browser (its exclusion already
  withholds it);
- meeting detection: apps excluded whatever their title are dropped before
  scanning, and each window is checked by its native title before
  `ElementFromHandle`. The include list applies here too, so with an include
  list only included meeting windows are scanned.

Residual contact: the recorder's global UIA focus-changed subscription. Its
handler makes no calls and only invalidates state, but UIA delivers every focus
change, including in excluded apps, and preparing the event's element can
involve that app's provider. Replacing it with a WinEvent hook (`EVENT_OBJECT_FOCUS`), which needs no
provider, is an open owner decision. Untested: a live run proving the absence
of calls into a specific excluded app.

## Native metadata on UIA-failure placeholders (2026-10-16)

Owner decision: a frame withheld only because UIA could not finish
(`accessibility_inspection` with a retrieval cause such as `uia_backoff`,
`uia_walk_timeout` or `uia_provider_failure`) may keep the walker's natively
read app name and title in `frames.app_name`/`window_name`. Pixels, OCR,
accessibility text and URLs stay withheld. The title is the one the walker
read in that attempt after every native rule passed (protected apps,
ignore/include patterns, private-window titles), for a window whose identity
was rechecked afterwards.

- No metadata for an unverified, unidentified or protected window, or when
  native policy metadata was incomplete.
- App name only (`native_app`) when a URL rule could not be checked (any
  `ignoredUrls` entry: a page title could name a blocked site), for terminal
  and console hosts (titles carry command lines), when the title is empty,
  when it names a password-manager brand (the brand alone suffices for a
  title), and on any content-screening hit on the title text (secret, DRM
  domain, configured keyword; screened even when frame screening is
  disabled).
- Otherwise app and PII-redacted title (`native_title`).

`capture_privacy.native_metadata` carries only that fixed code. Accepted
trade-off: rules that inspect content inside the window (ignore patterns
matching nested windows or documents, Document URLs, DRM URLs) were not
checked, so a title can be stored for a window whose inner content would have
blocked the frame; the title itself passed every rule that reads titles.
Other withheld frames (excluded, focus, monitor, no safe window) store no
metadata.

## Excluded background windows

When the foreground excluded app belongs to monitor A, B/C can retain admitted
monitor pixels if their own enumerated overlapping windows pass policy. A stable,
verified other-monitor association skips the global foreground UIA tree and its
labels; B/C use local bitmap OCR without borrowing A's app/title/text. This does
not make inactive monitors automatically safe or enable workers disabled by
existing capture preferences. Missing foreground enumeration, unknown native
monitor association or inconsistent sampling produces a placeholder. Monitor
association changes across acquisition/pre-UIA phases also withhold the bitmap.
Configured app exclusion remains separate from global protected-content, lock
and schedule gates. The subsequent [microphone-policy change](CAPTURE_DIAGNOSTICS.md)
separates microphone admission from visual checks; retention policy is unchanged.

Non-minimized windows overlapping a monitor conservatively affect its policy,
even if another window appears to cover them completely. This implementation
does not prove occlusion. An excluded background window permits only a safe
foreground window on that same monitor; if global focus belongs to another
monitor, only the topmost window there that may be captured alone (see above),
unless a covering window that may not be captured hides it.
Minimized and nonoverlapping windows do not force this fallback.

Ordinary File Explorer folder windows (`CabinetWClass`, `ExploreWClass`) can now
serve as the safe foreground fallback. Explorer's desktop/taskbar/unknown shell
surfaces cannot. Explicit user exclusions still take precedence. The remaining
monitor pixels stay black; no background-window bitmap is acquired. A new
background exclusion discovered immediately before UIA invalidates an already
acquired full-monitor bitmap and produces a placeholder.

This is per-persisted-frame evidence, not an exhaustive application-presence
timeline: throttling, identical-image/content deduplication and recording pauses
can omit intervals. It does not identify historical blockers. Focus changes can
still pair an earlier allowed-window bitmap with a later allowed-window UIA tree;
exact screenshot/tree window identity across those transitions needs separate
work. Broader provider failures, protected playback and privacy accuracy are not
guaranteed by these diagnostics or the deterministic tests.

## Sensitive decision diagnostics (2026-10-09)

Tasks 2 and 13 add an explicit opt-in sink, separate from ordinary logs, typed
notices, SQLite, API responses and support-log exports. Set
`SCREENWISE_SENSITIVE_DEBUG=1` **before starting the process** to enable it;
unset it or use any other value to disable it at the next process start. The
Windows deployment launcher carries this choice in its deployment manifest.
The next local candidate enables it; preparing that candidate does not start
capture or change the running installation.

On Windows the files are under
`%LOCALAPPDATA%\ScreenWise\sensitive-debug\session-<pid>-<timestamp>\decisions.jsonl`.
A fresh, never-reused session directory is created with a protected DACL granting
only its owner and LocalSystem full access, inherited by its children. No data is
written if that setup fails. Unix directories/files use 0700/0600. These are
plain-text diagnostics, not encryption, and do not protect against the same user,
administrative takeover, malware running as that user or backups. They are not
automatically exported or served. Existing sessions are deliberately not deleted
by the recorder; remove them manually when the diagnosis is finished. Disable
the option when no longer needed. Each process is capped at 8 MiB; repeated
identical category/details are deduplicated for 30 seconds. Details larger than
32 KiB are explicitly omitted (records have a 64 KiB ceiling). A bounded
128-record queue drops with content-free count warnings instead of blocking
capture; writer failures/limits also produce a fixed ordinary warning through the engine
tracing sink, including Windows GUI builds without stderr. A typed reporter
accepts only fixed lifecycle states and loss counts; early notices are buffered
and replayed when the engine registers its reporter. Abrupt
process exit can lose queued tail records. Multiple sessions accumulate until
manually removed.

The sink records full provider/OS exception representations and decision stages,
focused native HWND/PID candidates, filtered-window app/title/URL metadata,
policy matches and foreground transitions. It never deliberately reads or logs
UIA Name/Value/password text to diagnose the password probe, nor captures
keystrokes, clipboard values, pixels, audio or bearer tokens. Arbitrary provider
exception strings and window metadata may themselves be sensitive. Do not attach
these files to issues or commit them.

Ordinary input status now identifies native focus sampling, focused UIA element,
focus ownership, IsPassword read versus unsupported property value, final focus
verification, worker initialization and a closed visual privacy gate. These are
fixed allowlisted reasons; exceptions remain exclusively in the sensitive sink.
The capture-window path distinguishes an unavailable native foreground sample
from a sampled foreground absent from xcap's eligible-window list, and identifies
the recorder's own process as recorder UI. This addresses an established xcap
own-process exclusion mechanism without assuming it explains every historical
incident. Executable lookup uses limited process-query rights and a full-length
path buffer. Unknown identities and unstable focus continue to deny admission.

Synthetic regression coverage is provided for reason presentation, fail-closed
input stages, missing/recorder/unlisted foreground classification, storage bounds,
deduplication and the Windows directory DACL. Native/live-provider and installed
GUI validation remain separate; no new interactive capture was authorized here.

## UIA failure reporting and retry transitions (2026-10-13)

The deployed bounded-UIA candidate produced frequent `accessibility_incomplete`
placeholders. This is inspection failure, not evidence that Windows failed to
look up an executable for every frame: the old skipped result supplied no verified
executable identity, so the engine deliberately used a null blocker app. A selected
bitmap window can also belong to a different monitor from global foreground focus.
Do not infer an app from an earlier focus sample.

The old engine mapped the unknown blocker code to `no_safe_window` and reused
`post_capture_privacy_evaluation` as the stage. It also published preliminary
bitmap permission into the same WindowPolicy interval before UIA finished. Each
failed retry therefore reopened that interval and generated another failure;
screen and OCR each received a notice. The reported 270 combined-reason notices
were 135 paired screen/OCR reports, not 270 distinct capture attempts. The reported
40 failures were a subset of the 47 withheld frames. The 50/47/3 active-window
counts matched a lower bound ten seconds earlier than the displayed five-minute
window; metadata projection does not establish how the external CLI selected it.

The reporting-only candidate rejected every tree stop as a capture failure. The
current source separates retrieval from rule verification, as described below.
These specific causes remain available in frame/status diagnostics:

| Blocker / diagnostic reason | Meaning |
|---|---|
| `uia_walk_timeout` | Walk deadline cannot admit another native request, or Windows returned `UIA_E_TIMEOUT`; includes admission work before traversal. |
| `uia_node_limit` | Another element would exceed the node budget. |
| `uia_depth_limit` | Another navigation step would exceed the depth budget. |
| `uia_property_text_limit` | One decoded property would exceed its byte limit. |
| `uia_aggregate_text_limit` | Decoded text would exceed the remaining tree/context byte budget. |
| `uia_provider_failure` | A non-timeout native provider/timeout-configuration/navigation request failed. |
| `uia_backoff` | This UIA context's provider retry cooldown is active. |
| `uia_initialization_failed` | UI Automation initialization failed after native window checks. |

The typed cause survives tree-walk results, per-frame JSON stored atomically in
SQLite, current status and persisted notice diagnostics returned by the API.
The legacy `accessibility_incomplete` code is explicitly recognized as incomplete
inspection rather than no safe window. Existing historical rows are unchanged;
specific historical causes cannot be reconstructed from the collapsed code.

Only the final inspected window-policy decision is published. Repeated identical
failures refresh the live check while keeping their original onset and producing
no new history rows. A changed cause, genuine recovery or later failure is still
reported immediately. Screen/OCR/accessibility remain independent channels;
notice counts must not be treated as a count of distinct captures. Throttled
attempts do not fabricate recovery. Verified content-dedup observations are
published after the final checks even if no new frame is written.

Incomplete trees still withhold screenshots and OCR. No limit was relaxed,
provider work added or identity inferred to make capture succeed. These reporting
changes do not establish that bounded traversal meets useful capture coverage
for real large applications. That regression remains an investigation priority.

A metadata-only projection found selected-window app candidates in the older
private policy records. Candidate identity was therefore available at selection
time even though the skipped tree result supplied none to the failed frame.
These samples do not establish final frame attribution or the missing stop cause.

The previous tree-stop debug message was absent from the ordinary UTC-day log.
The enabled sensitive sink recorded window policy, input probes and browser
policy in the reported interval, but did not have a tree-stop category. New
`uia_tree_stop` records retain the HWND, fixed cause, elapsed time and actual capped
limits, including the native provider timeout. `uia_walk_progress` retains visited
node count, decoded byte count and deepest attempted level without retaining node text. `uia_provider_request_failure`
retains a fixed request stage, HRESULT and full exception representation exclusively
in the opt-in private sink; timeout configuration is shared with other UIA probes.
`uia_inspection_refusal` covers initialization errors, target/monitor verification,
window and subtree policy rejection, pre-walk expiry and bounded-walk failure. It
retains the already sampled HWND/PID/app/window-title candidate, selected target,
monitor bounds, configured budgets and configured include/ignore policy even when
that candidate is withheld from ordinary frame/status metadata. Candidates are
diagnostic evidence, not verified app attribution. URL-policy refusal records retain
the already extracted URL and window candidate; target-check failures retain expected
and current selection, and native selection errors retain their exception stage.
Policy pattern samples are capped at 64 per list, with full counts and truncation flags. No additional UIA text, password
value, keystroke or clipboard read is performed to enrich this record. New ordinary
stop notices retain fixed codes rather than exception strings or window titles.
Sink opt-in, queue/storage bounds and repeat deduplication still apply; an absent
record is not proof that no failure happened. The generic status-history parser
removes executable identities; authenticated decision details retain their
existing permitted basename metadata.

Validation: RootFmt, 214 accessibility tests (23 live tests ignored), 48 scoped
engine capture tests, 14 notice-storage/API tests, 58 focused status/notice UI
tests, no-emit TypeScript and the 16-page frontend export passed. Tests use
synthetic providers/strings and temporary databases; no recording was started.
The release-local RootBuild passed in 9m08s: 785 external artifacts reused, zero
rebuilt, and 11 workspace artifacts reused/eight rebuilt. Recorder `--version`,
`--help` and `record --help` exited zero without recording. The release-local
DesktopBuild passed in 15m24s: 1,080 external artifacts reused, zero rebuilt,
and ten workspace artifacts reused/eight rebuilt. Builds ran sequentially with
one job; this task's compiler processes ran at reduced priority after resource
monitoring. No other application was stopped or reprioritized.

| Initial reporting artifact (release-local, subsequently owner-deployed) | SHA256 |
|---|---|
| `target/release-local/screenpipe.exe` | `1284D911A9B794F99158F20C4CE4B0E5E8131572C7ADC9C5827672FAA6086260` |
| `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe` | `B76A2D8A75BB98663BB0CCAC5B89A6D39011335D09A03627AD3E5D78A5C5377D` |

Private canonical evidence: `20261012T230604671Z-Root-Fmt-c6995c00`,
`20261012T230303828Z-Root-Test-53ade810` (accessibility),
`20261012T225157994Z-Root-Test-454ae080` (capture),
`20261012T230047743Z-Root-Test-3628940e` (notice storage),
`20261012T230643248Z-Root-Build-f82b6870`,
`20261012T231639118Z-Desktop-Build-28ab08cb`. Both Rust lockfiles are unchanged;
no new dependency variants, unexpected external rebuilds or dependency acquisition
occurred. Frontend export retains the existing `unpdf` import-meta warning.

Validation is synthetic; no fresh desktop/UIA inspection, capture, restart,
deployment, historical cleanup or firewall change is authorized by this fix.


### UIA budget regression repair (2026-10-13)

The [subsequent investigation and repair](UIA_CAPTURE_REGRESSION_20261013.md)
records confirmed owner-deployed identity, sanitized authorized diagnostics,
keyboard-probe work/freshness bounds, actual-deadline traversal checks and separate
sampled-target executable attribution. RootFmt and 317 distinct synthetic tests
passed, followed by sequential single-job release-local recorder and desktop
builds. The note records repaired artifact hashes and remaining live-validation
limits. These builds did not deploy, restart or start capture; useful real-window
coverage remains unverified. The earlier artifact and build records below retain
their historical scope.

### Accessibility retrieval versus privacy verification (2026-10-13)

Native app/title, include-list, private-window and monitor/target checks run before
UIA initialization. Retrieval failure preserves its specific cause and discards
all incomplete accessibility text. It does not by itself reject screenshot
storage or OCR. Remaining applicable rules determine image admission:

- Ignored-window rules with an applicable app scope and nonempty title also check
  nested Window/Document nodes and extension popups. Unscoped rules apply to any
  app; a scoped rule for a different app does not require its tree.
- Configured ignored URL entries check accessibility Document URLs, including in
  embedded/non-browser apps. Passing a native address-bar check does not prove
  these additional document checks passed.
- Enabled protected-content URL checks require document URL data unless the exact
  selected bitmap owner has independently acquired URL metadata.
- Unknown/truncated native metadata or changed target/focus identity cannot grant
  admission from an earlier native-policy sample.

An unresolved check withholds the real bitmap and all OCR; only the existing
generated disclosure placeholder can be persisted. Otherwise the admitted bitmap
follows normal storage and OCR fallback, subject to the remaining final privacy
and user gates. Native include/title/private checks already passed without UIA do
not become unresolved merely because retrieval failed. No partial tree text or
password-node data is retained as accessibility content.

The per-frame capture_privacy.accessibility object contains cause, processing
(withheld, proceeding, or withheld_by_other_check), up to 32 unresolved_rules
references and a rules_truncated flag. This is separate from actual rule_matches:
an unresolved check is not a matched exclusion. References retain the original
configuration index, including empty entries. Fixed kinds explain nested
window/document exclusions, document URL exclusions, native window identity/title
verification or protected-content URL policy. Truncation never opens admission.

Accessibility status remains failed with its actual cause even when Screen/OCR
admission passes. Screen/OCR notices and Recording status explicitly say whether
storage/processing proceeds despite retrieval failure or is blocked by identified
unresolved checks. Generic notices contain no titles, URLs or pattern values. The
opt-in sensitive sink records uia_image_admission with the complete unresolved
reference list and already-sampled native context. Initialization and unclassified
retrieval errors retain detailed exceptions and stages there; existing private
provider/tree/policy diagnostics remain.

Identical cause, disposition and rule sets remain deduplicated. For 135 attempts
with one unchanged blocked decision on one monitor, the window-policy Screen/OCR
pair produces two notices total, plus one accessibility failure transition. A
permitted image disposition produces no failed Screen/OCR notices; accessibility
still emits its failure transition. Actual cause/rule/disposition changes produce
new transitions. Other channels/gates can independently produce notices, so this
is not a universal notice total for a five-minute period.

RootFmt, 219 accessibility tests (23 live tests ignored), 12 shared diagnostic
tests, 88 engine capture tests, 14 notice tests, 72 frontend tests, TypeScript
checking, the 16-page static export and inert recorder --version smoke passed.
Serial single-job release-local builds passed: recorder 7m14s (785 external
reused/zero rebuilt), desktop 15m42s (1,080 external reused/zero rebuilt). Both
lockfiles are unchanged. The existing unpdf import.meta warning remains.

Artifact SHA-256:

- Recorder: D483130FA4F5520CB3EFCE2491A483254165F6268C7F7E988C4BD678343FA48E
- Desktop: FE823E90D88083FADC10290A3FD4FBC3A724FF6DDC3C3980ECC658F002CCC181

Private build evidence IDs: 20261013T014425218Z-Root-Build-8e276132 and
20261013T015726849Z-Desktop-Build-68e055b0. No new capture, desktop UIA/input
inspection or deployment is part of this change; useful live capture coverage
and the brief startup disconnected-status report remain unresolved.

### Deployed timeout diagnosis (2026-10-13)

Owner deployment metadata matched the GUI hash recorded in the reporting
follow-up at that historical checkpoint. The owner subsequently deployed the
accessibility/privacy candidate: installed GUI and recorder hashes match the
artifact pins above. The [subsequent budget-regression investigation](UIA_CAPTURE_REGRESSION_20261013.md)
records the supplied failure report and limits.
Read-only diagnostic projections exposed two separate timeout defects: Windows
returned `UIA_E_TIMEOUT` (`0x80131505`) during root and navigation calls, but the
reporting helper classified it as a generic provider failure; remaining walk time
also produced timeout-setting values below the native minimum. An isolated COM
setter-only probe, with no element requests or desktop inspection, reproduced
`E_INVALIDARG` for 1–49 ms samples and success for 50, 51 and 100 ms. Microsoft
[documents the native timeout error](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-error-codes);
the 50 ms setter boundary here is local experimental evidence.

That historical follow-up stopped before another provider request when less than
50 ms remained. The [current full-deadline budget correction](UIA_CAPTURE_REGRESSION_20261013.md#deterministic-source-defects)
supersedes that reservation rule, while preserving native timeout classification
and retaining the exact
HRESULT and call stage in sensitive diagnostics. Existing time/node/depth/text
limits and incomplete-tree text rejection remain in force. This corrects manufactured
provider errors and misleading reason changes; it does not establish useful live
capture coverage or eliminate genuine provider timeouts.

Canonical focused `screenpipe-a11y` tests (`windows_uia`, `Jobs=1`, locked/offline)
passed 33 tests, with 23 live tests ignored and 182 unrelated tests filtered.
Root formatting passed; 264 external dependencies were reused, none rebuilt.
Private evidence IDs: `20261013T000836840Z-Root-Fmt-d67488af` and
`20261013T000849257Z-Root-Test-f93656f9`. At that checkpoint the source correction had
not been rebuilt into the GUI or deployed. The current accessibility/privacy
candidate includes it, with validation recorded above, and was subsequently
owner-deployed. That deployment does not establish useful live coverage; see the
[budget-regression investigation](UIA_CAPTURE_REGRESSION_20261013.md).
No new capture, input, desktop UIA inspection, restart or firewall change occurred.

The reported startup disconnection remains unconfirmed: recording producers
started for the enumerated monitors, and the inspected persisted notices contain
no monitor-disconnected transition. Cold focus-policy capture is reported as
`monitor_inactive`, which does not establish hardware disconnection. Existing
logs do not preserve the exact initial topology/status response, so that brief
UI report cannot be reconstructed from this evidence.

### Fresh bulk admission candidate (2026-10-14)

The [candidate handoff](FRESH_BULK_UIA_HANDOFF.md) records fresh per-attempt UIA
bulk caches, event-revision invalidation, original-deadline retries and cleanup
before publication. Trees and admission decisions do not survive an attempt.
Native identity, focus and fail-closed checks remain. Scoped property queries
retain existing rule semantics; structural hierarchy, extension fallback and
accessibility recording require the documented broader acquisition. Fixed rule
IDs, reasons and aggregate counts replace content-bearing acquisition logging.
The authenticated numeric endpoint supports the pending comparison. Synthetic
tests/builds do not prove provider event completeness or useful live capture;
deployment and the 20-minute numeric trial await owner confirmation.

## Content screening fields (2026-10-15)

With content screening enabled, each persisted Windows frame's metadata includes
`content_screening` (`outcome`, fixed `reasons`, `fail_open`, `redacted_regions`,
`ocr_forced`, `ocr_ms`, `screen_ms`, `redact_ms`, `hits_ocr_only`,
`hits_a11y_only`, `hits_both`); a withheld frame's placeholder uses the redaction
reason `content_screening`, and `accessibility.processing` may be
`screened_fail_open`. The field is omitted when screening is disabled, so earlier
metadata is unchanged. Semantics, limits and permitted aggregate queries are in
[content screening](CONTENT_SCREENING.md).
