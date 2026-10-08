# ScreenWise Windows build notes

The portable build and test instructions are in
[docs/WINDOWS_SETUP.md](docs/WINDOWS_SETUP.md). Repository policy is in
[AGENTS.md](AGENTS.md); no parent workspace document is required.

## Production release validation (2026-10-08)

The settled capture-policy candidate `3ea64c8c7` now has production release
builds in addition to the focused and `release-local` checks below. Reviewed
`-PlanOnly` for `RootBuild` and `DesktopBuild` with `-BuildProfile release`, then
ran both sequentially using the existing canonical caches, locked/offline setup
and four-job default. No cold-cache override, cache reset or acquisition was needed.

- `RootBuild -BuildProfile release`: passed in 8m24s; 785 external artifacts
  reused, zero rebuilt; 11 workspace artifacts reused, eight rebuilt.
- `DesktopBuild -BuildProfile release`: passed in 13m34s; 1,080 external artifacts
  reused, zero rebuilt; ten workspace artifacts reused, eight rebuilt.
- `Test-ScreenWiseDeployment.ps1`: 54 synthetic checks passed. Rebuilt recorder
  `--version`, `--help` and `record --help` passed. Both lockfiles remain unchanged.

Reuse the already successful frontend type/export and focused test results;
no relevant source changed. These are compilation, synthetic deployment-script
and inert CLI results, not live hardware or installed-package validation. No
desktop app/capture session was launched and the running deployment was unchanged.

The existing launcher has a global compilation lock and refuses concurrent
invocations. An isolated-worktree concurrency improvement is being developed
separately; it was not used for these builds. Review shared staging and cache
boundaries before overlapping independent root/desktop builds.

## Microphone admission and lifecycle ordering (2026-10-08)

The settled policy separates microphone permits from visual exclusions and
foreground protection/uncertainty. Output retains its protection gate; lock,
schedule, audio disable, device disable and explicit all-recording pause still
apply. Desktop manual stop revokes before awaited shutdown. Explicit resume
checks request revisions so older startup/resume completions cannot overwrite
newer pauses. Automatic device recovery uses a separate lifecycle gate.
See [policy and limits](docs/CAPTURE_DIAGNOSTICS.md).

Use the canonical launcher with `RootTest -Lib` and `-TestArguments
@('--test-threads=1')`: `screenpipe-config` / `audio_privacy::tests`,
`screenpipe-audio` / `privacy_` and `core::run_record_and_transcribe::tests`,
`screenpipe-engine` / `privacy_`, `drm_detector::tests` and
`event_driven_capture`. These passed eight, 15, 15, 18, 21 and 43 tests
respectively (the selections overlap). Sources and stores are synthetic; no
hardware capture or local model is opened. The combined engine run initially
failed three writer assertions because another test had latched process-wide
degradation. Test writers now own isolated status; production degradation
remains latched. The corrected selection passed.

Six interval-store tests also passed. A final audit found pressure/loss could
cross thresholds between API polls without a local warning. A dependency-free
fixed numeric callback now reports pressure, recovery and rejected deliveries
at mutation, outside the store mutex and independently of SQLite. The tests
verify warning before rejection without polling, deduplication, exact loss counts,
recovery, late-registration replay and the callback lock boundary.

The first standalone audio library target lacked a launcher baseline and linked
test executable. After reviewing its plan and native setup, `-AllowColdCache`
adopted 13 external variants. Old dependency timestamps and a previously unset
`VCINSTALLDIR` accounted for the stale artifacts. The necessary diagnostic
rerun reused all 552 external artifacts with none rebuilt; subsequent engine
selections reused all 685. No dependency or lockfile changed. The audio-only
test feature selection retains an existing unused `sample_rate` warning.

The frontend export used `NEXT_TELEMETRY_DISABLED=1` and
`node node_modules/next/dist/bin/next build`, without invoking acquisition or
sidecar-staging prebuild scripts. Type checking/export passed with the existing
`unpdf` warning. Runtime milestones use `RootBuild -Package screenpipe-engine
-Bin screenpipe` and `DesktopBuild`, both defaulting to `release-local`.

Final `DesktopCheck` passed with all 1,084 external artifacts reused and none
rebuilt. `DesktopTest -Bin screenpipe-app -TestFilter recording_ -TestArguments
@('--test-threads=1')` passed 11 tests, including the deterministic newer-pause
startup regression, with all 921 external artifacts reused and none rebuilt.
Canonical `RootFmt`/`DesktopFmt` and whitespace checks passed.

The final engine `RootBuild -Package screenpipe-engine -Bin screenpipe` passed
in 3m19s under `release-local`: 785 external artifacts reused, none rebuilt;
nine workspace artifacts reused and eight rebuilt. Inert `--version`, `--help`
and `record --help` checks passed against that rebuilt executable.

The final `DesktopBuild` passed under `release-local` in 5m49s: all 1,080
external artifacts reused, none rebuilt; ten workspace artifacts reused and
eight rebuilt. No desktop application or recording session was launched.

## Capture interval diagnostics (2026-10-08)

The subsequent unaffected-monitor change uses `RootTest -Package
screenpipe-screen -Lib -TestFilter capture_` and `RootTest -Package
screenpipe-engine -Lib -TestFilter event_driven_capture`. These exercise native
association sampling decisions, excluded foreground/background/spanning geometry
and omission of foreign UIA/trigger labels with fabricated snapshots. The existing
`release-local` engine build is the compilation milestone; no live capture or
UI implementation is authorized by these checks.

Use the canonical locked/offline launcher with `RootTest -Lib` for
`screenpipe-config` / `capture_diagnostics`, `screenpipe-engine` /
`privacy_notices::tests`, `drm_detector::tests`, `event_driven_capture::tests`
and `capture_diagnostic`, `screenpipe-screen` / `capture_`, and
`screenpipe-a11y` / `keyboard_privacy` (package / test filter respectively).
The standalone config test selection required initial cache adoption after
reviewing `-PlanOnly`: 20 new external variants, no unexpected rebuilds.
The engine check-only variant had no matching successful baseline; it was
left unwarmed in favor of the established engine library-test target.
Rust formatting edits used `cargo fmt --all`; canonical `RootFmt` verified them.
The Timeline test is `node node_modules/vitest/vitest.mjs run --config
vitest.config.mts components/rewind/__tests__/privacy-notice-track.test.tsx`
from the desktop frontend directory. Build the settled engine using
`RootBuild -Package screenpipe-engine -Bin screenpipe` (`release-local`).
These checks use synthetic fixtures and inert CLI help/version invocations;
they do not authorize capture, content inspection or deployment.
See [coverage and limits](docs/CAPTURE_DIAGNOSTICS.md).

## Audio segment retrieval and Pi grounding (2026-10-08)

Use the canonical locked/offline launcher and established caches. Focused
unoptimized tests exercise SQLite segment identity, FTS matches, count admission,
stable background/live paging and the authenticated search router:

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-db -TestTarget audio_segment_search_test
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter routes::search::tests
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -TestTarget audio_search_api_test
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopCheck
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopTest -Bin screenpipe-app -TestFilter recording_
```

The first three selections passed four, ten and one synthetic tests respectively;
the opt-in API fixture server is ignored in the ordinary integration run. They
reused all 399/685/685 external artifacts with no dependency rebuilds. The initial
API fixture target required explicit new-target cache adoption and failed on a
Windows socket-address type; using `SocketAddr` repaired it. The corrected run
passed in 59s. No lockfile or dependency upgrade was needed.

Twenty-five mocked JavaScript transport/guard tests, pinned Pi SDK registration,
tool/message-hook checks and 74 PowerShell evidence-export checks passed. Three
opt-in local-model checks passed for synthetic matching context, empty success
and HTTP failure. Five inert Python checks cover shared-runtime identity,
missing dependencies/entrypoint and isolated fresh output. Prompt-only trials
had inconsistent quotation, timing and
error interpretation; the settled candidate uses bounded variant retrieval and
completed-message guards. Trials reuse the provisioned packages/model and read
`recording-prompt.txt` directly without app rebuilds. See the
[Pi test guide](scripts/windows/deployment/pi/README.md) for commands and limits.

The recorder `release-local` build passed in 3m58s, reusing 785 external artifacts
and rebuilding seven workspace artifacts. Root and desktop format checks passed.
Desktop checking passed in 2m54s, and in 2m22s after the completed-answer routing
correction; each reused 1,084 external artifacts. The initial default desktop
test selection compiled the normal app and test executable (7m59s, nine selected
checks passed). The settled `-Bin screenpipe-app` selection passed ten checks in
2m46s, reusing 921 external artifacts and rebuilding only the application test
artifact. Prefer that narrower selection for this change; filters alone do not
avoid compiling the normal executable for integration tests. No external
dependency rebuilds or new variants occurred. The existing cache also had a
matching binary-selector baseline. Recorder version/top-level help/record-help
smoke checks passed without capture; recording flags are on the subcommand.
The desktop `release-local` build passed in 6m49s, reusing 1,080 external artifacts
and rebuilding seven workspace artifacts. No new external variants or unexpected
dependency rebuilds occurred.
The subsequent extension-only title correction passed the 25 transport/guard
tests, pinned SDK hooks and four final local inference cases (title, context,
empty and request failure). The model initially used retrieval for a title and
sent a keyword array; the SDK now disables title tools and the adapter accepts
bounded lists by searching the first term. These trials required no app build.
The final release GUI embeds the corrected extension pair and settled prompt.
The full recorder release build passed in 12m16s, reusing all 785 external
artifacts and rebuilding seven workspace artifacts. No new variants or unexpected
external rebuilds occurred. Its version/help/record-help checks passed without
starting capture; the expected macOS-only Swift skip warning remains.
The full desktop release passed in 19m35s, reusing all 1,080 external artifacts
and rebuilding seven workspace artifacts. Both full builds had zero new external
variants or unexpected rebuilds. Matching owned Pi assets were staged only in
the ignored release build output and checked against source. All 504 final
Rust/manifest/lockfile/Pi input sizes and UTC timestamps stayed unchanged through
the desktop release build. Artifact verification used sizes/timestamps; no
artifact hashes were calculated. The existing running deployment was not updated,
and no new GUI/capture session was started.
The candidate requires matching GUI and recorder binaries plus the owned Pi
extension assets; a stale asset pair is rejected at runtime. These are synthetic
and local inference checks, not a new live capture or firewall/privacy validation.

## Foreground blockers and active-monitor filter (2026-10-06)

The additive metadata/filter changes use the existing canonical caches and
configured Developer PowerShell/native environment. Validation commands:

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootFmt
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopFmt
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-screen -Lib -TestFilter capture_
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-db -TestTarget capture_privacy_test
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter routes::search::tests
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter event_driven_capture::tests
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-capture -Lib -TestFilter capture_privacy
```

All 120 checks passed (65 screen, four SQLite, nine API/search, 41 engine,
one paired persistence smoke). Every suite reused all selected external
dependencies; no new variants or unexpected rebuilds were reported. The second
engine test selection reused its just-compiled library test binary. Direct
`rustfmt --edition 2021` was used only to format the affected Rust files before
the canonical format checks. No recording, GUI interaction, model provisioning
or firewall mutation was performed. Active-monitor behavior needs an observation
after deploying the new binaries; see [the field reference](docs/CAPTURE_PRIVACY.md).

The recorder quick build passed in 2m22s (785 external reused, seven workspace
rebuilt); GUI quick build passed in 4m56s (1080 external reused, seven workspace
rebuilt). No external rebuilds or new variants were reported. Non-recording
recorder `--version` and `--help` checks passed. The full recorder release build
passed in 6m15s, again reusing all 785 external artifacts and rebuilding seven
workspace artifacts. Its inert CLI checks passed; the expected macOS-only Swift
skip warning remains. Commands for the affected build profiles:

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootBuild -Package screenpipe-engine -Bin screenpipe
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootBuild -BuildProfile release
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild -BuildProfile release
```

The full GUI release build passed in 10m52s, reusing all 1080 external artifacts
and rebuilding seven workspace artifacts. Both full builds had zero new external
variants or unexpected rebuilds. The 585 recorded Rust/manifest/migration/lockfile
input sizes and UTC timestamps remained unchanged from completed testing through
release compilation. Artifact verification uses file sizes/timestamps, without
artifact hashes. These are compilation and synthetic/inert checks; no new live
capture or GUI was launched and the persistent deployment was not replaced.

## Capture-blocker metadata and Explorer fallback (2026-10-06)

Background validation uses the configured canonical Developer PowerShell/native
environment, with locked/offline dependencies and the existing root target cache:

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootFmt
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopFmt
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-screen -Lib -TestFilter capture_
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-db -TestTarget capture_privacy_test
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter event_driven_capture::tests
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter capture_privacy_search_api
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-capture -Lib -TestFilter capture_privacy
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootBuild -Package screenpipe-engine -Bin screenpipe
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootBuild -BuildProfile release
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild -BuildProfile release
```

The standalone screen/capture tests and new database integration target initially
used `-AllowColdCache` after reviewing `-PlanOnly`, existing artifacts and stable
native paths. This adopted their selected variants without cleaning caches.
The screen suite initially failed two assertions, corrected to preserve existing
scoped-include semantics and stable blocker ordering. Final counts are 62 screen,
three SQLite integration, 41 capture-engine, one API serializer and one paired
capture smoke: 108 passing checks. The SQLite target created two narrower SQLx
variants; all other targeted suites reused external dependencies. No unexpected
external rebuild was reported. A final screen-suite rerun covers rejected
truncated process paths. Root/desktop formatting passed; neither lockfile changed.
The paired capture smoke writes a synthetic JPEG and verifies real atomic
SQLite persistence without acquiring the desktop, running UIA or extracting OCR.

The first quick recorder build passed in 2m16s (785 external artifacts reused,
zero rebuilt). The final process-path guard required the affected suite and quick
recorder build again; the final recorder build passed in 1m25s with the same
external reuse and four workspace artifacts rebuilt. The GUI quick build passed
in 5m00s, reusing 1080 external artifacts and rebuilding seven workspace
artifacts. Non-recording recorder `--version` and `--help` smoke checks passed.
The full recorder release build passed in 6m23s, reusing 785 external artifacts,
rebuilding seven workspace artifacts and reporting no unexpected rebuilds.
Its non-recording version/help smoke passed. The existing macOS-only
Apple Intelligence Swift-skip build warning remains expected on Windows.
The full GUI release build passed in 11m39s, reusing 1080 external artifacts,
rebuilding seven workspace artifacts and reporting no unexpected rebuilds.
Sizes and UTC timestamps for all 592 recorded Rust/manifest/migration/lockfile
inputs remained unchanged across release builds; artifact hashes were not
computed. Release compilation does not update the persistent deployment.
No new capture session or GUI was launched. The owner accepted the previous
monitor correction; the additional Explorer/background live case remains
unverified. See [capture decision metadata](docs/CAPTURE_PRIVACY.md) and the
[validation register](VALIDATION_REGISTER.md) for scope and limitations.

## Shared Pi deployment and API review preparation (2026-10-06)

The changed desktop target passed `DesktopCheck`, `DesktopFmt`, three linked
`local_pi_integrity` regressions and seven standalone pure Rust helper tests.
`DesktopTest` adopted its existing canonical test cache once after the launcher
reported no matching successful baseline; 921 external artifacts were reused
and none rebuilt. The `DesktopBuild` quick `release-local` build passed in
4m02s, reusing 1080 external artifacts and rebuilding only the app target.
The pinned SDK loader and mocked recording tool executed through that profile's
bundled Bun without a model, real API or GUI.

Deployment scripts passed 38 synthetic checks; the private evidence exporter
passed 72, and its Pi GET transport passed 15. These cover source/runtime/data
isolation, package identity, restricted tool arguments, output limits and
timestamp/pagination pitfalls. The exact commands are in the deployment and
evidence-review guides. Runtime scripts use file sizes/timestamps rather than
large-file hashes. Models, Pi and WebView2 are reused without package/model
copies. New deployment paths still require explicit owner-run firewall rules
and live startup/shutdown/retrieval validation. No capture was started.

The full canonical `DesktopBuild -BuildProfile release` passed in 10m41s,
reusing 1080 external artifacts, rebuilding one app target and no dependencies.
The unchanged recorder release executable was retained without a root rebuild.
The non-launching deployment staged these binaries and owned extension files,
reusing one Pi 0.75.4 package and the existing model/WebView2 stores. New deployed
paths require the owner's separate nine-rule firewall installation before any
live preflight/startup. Deployment did not register login startup or start capture.
The actual 17,178-file staged inventory and owned extension source comparisons
passed. Native metadata lookup completes in 9.46 seconds; a serialized manifest
regression covers PowerShell's automatic date conversion. No binary rebuild was
needed for these PowerShell-only corrections.
The owner's Windows PowerShell 5.1 firewall preflight exposed a long-path false
negative: ordinary .NET Framework FileInfo reported an unchanged 261-character
package path missing. Native metadata now uses extended Windows path spelling,
retaining canonical executable rule paths and all size/timestamp/link checks.
Forty-three synthetic deployment checks passed, including >260-character files,
real metadata changes and missing files. Actual 17,178-file checks passed in
PowerShell 7; a read-only Windows PowerShell 5.1 native metadata probe also
reported zero mismatches. No redeployment, Rust rebuild or firewall mutation
was required for this correction.

## Current iteration profile (2026-10-05)

Commit preparation, 2026-10-06: both canonical `RootFmt` and `DesktopFmt`
checks passed, superseding the earlier desktop formatting failures recorded
below. The side-chat canonical locked/offline root `release` build completed
with exit zero; its desktop `release` build was still running at this check.
The existing bounded `release-local` smoke and regression results below remain
the runtime evidence; no new capture or interactive test was started.

### Provisioned Pi renderer correction (2026-10-06)

The third fresh content-disabled GUI run completed both local arithmetic turns
with the correct numeric answers. GUI chat files and Pi session files retained
the ordered replies under the isolated native app root. No tool events appeared.
Native held-handle monitoring verified three bundled Bun workers and their three
signed System32 console hosts; all exited on tray quit. The launcher returned
zero, the marker was `clean-v1`, and final OS inspection found no remaining GUI
scope process or API listener. Missing/wrong/valid bearer requests returned
403/403/200. All nine owner-installed rules remained enforced; fixed scoped
outbound block/control evidence and socket snapshots are retained separately.
This validates bounded `release-local`, tool-free GUI execution, not tool-enabled
activity retrieval, production packaging, crash cleanup or packet-level isolation.

The run exposed duplicate assistant text in the injected follow-up history:
plain content and its display blocks were both appended. Direct and queued sends
now share a history builder that uses full plain text once, preserves tool
context, and falls back to text blocks for older rows. Forty affected tests
passed, including the actual persisted duplicate shape, deliberate repetition,
legacy block-only text and tool-result limits. The corrected frontend compile,
type check and export passed. Its canonical `release-local` desktop build passed
in 3m27s (1,080 external reused/zero rebuilt, 17 workspace reused/one rebuilt),
with GUI SHA-256
`8863C9DA605EBFA147E8F8A4AC5A5808B1AC4A8B4B23AF51C8D653EF9DD5376C`.
The inert candidate preflight passed existing rule/model/runtime identity and
fixed block/control checks; no GUI, capture or inference started in that preflight.
The subsequent fresh, owner-authorized GUI retest passed the strict one-occurrence
history check and matching four-message GUI/Pi persistence. Both fixed arithmetic
equations were correct, but the model ignored plain-number output instructions.
The original numeric-only checker rejected the extra operands; the preserved
failure is reported separately from corrected-history/persistence acceptance.
Native held handles verified three Bun workers and their three signed console
hosts exited; tray quit gave `clean-v1`, exit zero, no scoped GUI processes/API
listener. Read-only SQLite counts found zero frames, audio chunks and non-notice
UI events. Nine rules remained enforced with the fixed block/control result and
zero sampled established non-loopback connections. Raw generic attention/console
flags remain preserved and reconciled against the independent native proof.
One chat-title request timed out and a later request succeeded. The model added
formatting despite exact-output instructions. Smart PII model absence, safe UIA
failure notices, a startup fetch error and WebView exit warning remain recorded.
Other-RPC advisory and managed Pi crash/restart remain separate outstanding checks.

The preceding fresh GUI retry failed before Pi prompts: `is_absolute` was not
in the minimal Tauri ACL. The directory helper now checks absolute-path syntax
locally without that IPC, and its regression models the denied permission.
The 37 affected directory/settings/chat tests passed. The same candidate updates
the requested local diagnostic labels to **view crash details** / **view error
details**, with a **diagnostic details** dialog that states nothing is submitted.
Source review added dynamic filesystem access only for active-root `chats` and
`pi-chat` folders, needed for isolated trial persistence; no broader root or new
IPC permission was added. GUI persistence was still pending at that stage.

The frontend compile/type/export passed with the existing `unpdf` warning.
The canonical desktop check passed in 1m46s (1,084 external reused/zero rebuilt).
The next `release-local` desktop build passed in 3m23s (1,080 external reused/zero
rebuilt); GUI SHA-256 is
`EDB32BE9A811A9B1B1C86179407402A7ADEB8E975B4712EAD509AB9B9C4A5134`.
Narrow main.rs formatting passed after sorting the added import. Previous
artifacts/pins and both failed attempts are preserved; no commit/push occurred.
The second GUI's health/authentication/clean-shutdown/OS cleanup checks passed
independently of its startup failure. The third attempt used fresh readiness.

The first tool-free provisioned GUI attempt reached Pi but failed in the home
renderer because a partial settings snapshot omitted `disabledShortcuts`.
Settings load/change normalization and native app-directory resolution now cover
settings, chat persistence, Pi/title work directories and large-context files.
Unavailable native scope does not fall back to global files. The synthetic mode
also skips skill installation and calendar publishing. Read the failed attempt's
scope and remaining checks in [the register](VALIDATION_REGISTER.md).

The focused frontend regression set passed 32 tests; the complete Vitest suite
passed 436 tests in 46 files. A direct local Next build, with telemetry disabled
and no prebuild/acquisition hook, passed compilation, type checking and export;
the existing `unpdf` import.meta warning remains. The canonical desktop check
passed in 1m54s with 1,084 external artifacts reused and zero rebuilt.
The canonical `release-local` desktop build passed in 3m26s with 1,080 external
artifacts reused and zero rebuilt. Its GUI SHA-256 is
`6D62AB374340FA643359261296B5B97332244E6F3FEAFCCBF2C5AC8C7F122741`.
Both Rust lockfiles are unchanged. DesktopFmt still reports pre-existing
commands/browser/overlay formatting differences; the added line was formatted
and narrow pi/main checks passed. No unrelated formatting was applied.

Before the third attempt, the repinned, inert GUI/provider preflight passed exact existing executable/rule
scope, localhost provider ownership/model identity and the fixed block/control
probe. It started no GUI or recording. Conversation/persistence and the observer
repair were still pending then; the third run's results above supersede that state.
No firewall rule was added or removed, and no commit
or push occurred. Full production-release/installer testing remains separate.

Both Rust workspaces now use `release-local` for routine executable builds:
optimization level 1, no LTO, 16 code-generation units and incremental compilation.
The former `release-dev` profile is removed. Its references below describe actual
historical results and do not authorize reusing that artifact for a current run.
Full `release` remains explicit for production artifacts and performance evidence.
Supported Windows Cargo tasks use the canonical launcher; see
[its guide](scripts/windows/build/README.md). Trial defaults and controlled-test
configuration now select the matching profile directory and refuse stale or
mismatched runtime configuration before launch. The source archive's three changed
helper/config hashes were refreshed; original snapshot hashes were preserved.

Background validation passed in PowerShell 7 and Windows PowerShell 5.1:
86 launcher stub checks, 18 synthetic cache checks, 74 trial-profile checks and
the synthetic GUI trial harness regression in each shell. The profile checks
exercise early runtime rejection and the actual canonical profile/path gate in
isolation, with no recording, outbound probe or firewall changes. Locked/offline
metadata inspection accepted both manifests (17 root packages, one desktop
package); all 52 source-archive files verified. Archive tests passed 14 tests with
one Windows symlink-privilege skip. `git diff --check` passed.

The first application builds subsequently passed through the canonical launcher:

```powershell
& .\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild -AllowColdCache
& .\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootBuild -Package screenpipe-engine -Bin screenpipe -AllowColdCache
```

Both selected `release-local`, locked/offline dependencies and the configured
Developer PowerShell/native environment. Cold-profile adoption was reviewed
before building. The desktop build passed in 14m27s (1,080 external new variants,
18 workspace artifacts); the recorder passed in 9m57s (785 external new variants,
17 workspace artifacts). Both cache reports recorded zero unexpected external
rebuilds, and neither build emitted compiler warnings or errors. These cold runs
are not a comparison of incremental build time or production performance.

Current executable SHA-256 pins:

- GUI: `FBDAD57E336A2FEA25AE6F1697A03BF84BB523AB8801ACC2411F18857BE8728B`
- Recorder: `5F4183B61CCAC65826409807DC0D6C50C7EA0E2C07B569ACBD26599CF3EDCD3D`

Reviewed FFmpeg, FFprobe, Bun, OpenBLAS and ONNX Runtime assets were hash-checked
and staged beside the matching new executables. The existing pinned private
WebView2 runtime and self-tested process monitor were reused. The GUI runtime
manifest and six executable pins were regenerated; the old manifest and private
evidence were preserved. No dependencies were downloaded and neither Rust
lockfile changed.

No application or capture session was started during this preparation. Read-only
OS inspection found no scoped process or test-port listener and all firewall
profiles enabled. Existing rules did not cover the five new executable paths;
the reused private WebView2 path retained its existing blocks. A unique six-rule
installation block and matching restoration block were prepared, syntax-checked
and scope-reviewed. The owner subsequently confirmed installation of six rules;
read-only ActiveStore inspection verified every exact scope and an enforced
status for each rule. The matching non-recording GUI preflight passed: its fixed
outbound TCP attempt was blocked while the unscoped control succeeded. This is
one fixed-endpoint diagnostic, not complete packet-level validation. The owner
then provided fresh readiness for the bounded GUI check described below.
Restoration has not run. Production-release, packaging and installer validation
remain separate.

### GUI banner and Pi copy retest (2026-10-05)

The pinned `release-local` GUI ran in a fresh private directory with audio and
transcription disabled, the reviewed exclusions, authenticated loopback API and
the owner-confirmed six-rule group. The owner confirmed that the safe explanation
banner was above the placeholder image with no overlap, the Pi setup button
copied the complete absolute command, the instructions were selectable, and the
notice persisted for 15 seconds and could be dismissed. The command was not run;
no Pi package or provider was provisioned. The UI helper read the controls but
failed with unavailable input geometry, so these were owner-operated checks.

Tray quit returned exit code zero and `clean-v1`; read-only OS inspection found
zero scoped processes, active API TCP endpoints, listeners or UDP endpoints.
The last operational sample recorded 123 captures and 123 SQLite writes, no
dropped frames or stalls, and one unexplained frame-link expiry (two other
expired event halves were attributed to known capture drops). The bearer matrix
was 403/403/200; socket sampling observed no non-loopback connection or listener.
The 25 ms process monitor recorded 2,265 snapshots, eight descendants and zero
unexpected shell starts. These are bounded observations, not a privacy guarantee
or a final database-content audit. OCR/audio positive controls were not run.

Two monitor defects were repaired after the run: JSON timestamps decoded as
`DateTime` now normalize to UTC RFC3339 before API queries, and closed TCP
`TIME_WAIT` remnants no longer count as active post-exit endpoints. Total and
`TIME_WAIT` counts remain visible; live and unknown states remain actionable.
The corrected live notice query succeeded without persistence degradation or
reported delivery loss. Regression cases exercising the actual monitor
assignments passed with the synthetic GUI harness in PowerShell 7. The Windows
PowerShell 5.1 suite was blocked by script execution policy, which was not changed.
Historical audit output is preserved with a separate correction note.

The filter-column bottom inset was identified as a source of upward overflow
into the preceding Recording status row. A shared production viewport now
anchors at the slider top and scrolls within 60 pixels; six rendered-component
regressions passed. The owner subsequently confirmed that the Recording status
label looked good in the last GUI recovery run, completing the visual label
follow-up for that layout. That run had content capture disabled; no broader
recorded-media layout coverage is claimed. The unexplained frame-link expiry
remains under investigation.
Startup WebView fetch failure, optional Smart PII reduced coverage and
a WebView class-unregistration warning at exit were also observed. Forced
interruption/restart and real Pi/provider execution remain separate tests.

### Controlled GUI interruption/restart (2026-10-05)

The reusable [recovery controller](scripts/windows/day-to-day-trial/README.md#controlled-gui-crashrestart-check)
prepares an unused private directory and waits for fresh owner readiness. It
disables screen/audio and keyboard/clipboard content, applies a unique nonmatching
UI include filter, verifies a persisted-notice positive control, interrupts only
its verified GUI process handle, and waits for launcher/monitor completion before
allowing a single controlled restart. Existing settings, token and SQLite rows
must survive. Acceptance requires the fixed interruption notice in current logs
and the authenticated timeline API, owner visual confirmation, a clean marker,
403/403/200 bearer checks and no scoped processes/active endpoints afterward.
The owner-authorized two-launch check completed on the pinned `release-local`
candidate. It does not exercise real Pi, audio tails or WGC during interruption.

The PowerShell 7 recovery regression passed 83 groups, including actual launcher
nonempty-directory refusal and store-write bypass, receipt tampering, unsafe
settings and hash-preserved synthetic sentinels.
The actual health/log validators and post-restart controlled-settings comparison
also have offline acceptance/refusal coverage; unrelated GUI defaults may change.
One Python regression passed for the read-only SQLite aggregate helper, including
unchanged database bytes and refusal to create a missing database. The inherited
Python ignore rule has explicit exceptions for these two reusable source files.
The scripts parse in Windows PowerShell 5.1, but its existing execution policy
blocked the full suite; the
policy was left unchanged. The GUI/trial-status suites passed. The launcher now
records existing rolling-log offsets for restart and a completion receipt after
its monitor finishes, preventing stale phase-one output from representing phase two.

Four linker message classes now expose full/closed send failures separately in
fixed warnings and count-only health fields. Trial summaries flag all eight
counters immediately, including before TTL expiry. The canonical root linker
suite passed 16 tests in 1m45s, reusing 685 external artifacts and rebuilding none;
initial adoption of the existing test cache was explicitly reviewed. The scoped
`release-local` CLI build passed in 2m27s, reusing 785 external artifacts with none
rebuilt. Thirteen focused frontend tests, TypeScript and static export checks
passed (the known `unpdf` export warning remains). No captured payload was inspected
and the earlier orphan's cause remains unresolved.

The settled canonical `DesktopBuild` passed in 6m01s, reusing 1,080 external
artifacts with none rebuilt. The `release-local` GUI SHA-256 is
`3406F220CB45B50B0D2128EFB8EBAE9A2C761E8307DD47BC2C7AF6EACF823AEE`;
the recorder SHA-256 is
`60ACE945B29A42A934AC99ADB6B439D10B14AA9C1076909B998D69165090BD35`.
The existing six exact-path firewall rules were independently inspected and
still matched. Non-recording preflight passed: the GUI's fixed IPv4 attempt was
blocked while the unscoped control succeeded. Fresh owner readiness then gated
the isolated run; it is now stopped.
The known `unpdf` warning, Windows PowerShell policy limitation and unresolved
historical correlation expiry remain explicit. No firewall change, provisioning,
commit or push occurred.

The exact owned GUI handle was deliberately interrupted (exit -1), leaving the
active session marker. After the first launcher and process monitor completed,
the single permitted restart preserved the token, controlled capture/privacy
settings and initial SQLite notice. Exactly one fixed interruption notice was
returned by the authenticated API; its fixed text appeared twice in current
logs. The owner confirmed its explanation in Timeline's Recording status and
quit from the tray. Exit zero and `clean-v1` followed. Both phases passed the
403/403/200 authentication matrix and observed zero non-loopback TCP/UDP endpoints
or API listeners. Independent final OS inspection found zero scoped processes,
active API TCP endpoints or UDP endpoints. Rules remain installed.

A harness assumption failed during the first launch: disabled vision/audio omit
their health metric sections rather than reporting zero counters. The validator
was corrected without changing the app, binary or capture settings; absent whole
sections are accepted only with disabled status, while incomplete/nonzero present
sections still fail. The new regression cases and read-only SQLite positive
control passed before continuing the same authorized attempt. Aggregate checks
before interruption, after restart and after exit found zero frames, audio chunks
or ordinary UI events. Final SQLite `quick_check` passed. Missing metrics are not
claimed as measured zeros; the database supplies the independent content check.

Each phase summary retained attention status, two error-level lines and seven
warning-level lines, with no panic file, persistence degradation or reported
delivery loss. Observed categories included initial WebView fetch failure, UIA
initialization retry/password-state unavailability, optional Smart PII reduced
coverage, missing Pi provisioning and WebView class-unregistration at exit; the
restart also emitted the expected recovery warning. These are not an error-free
trial claim. Actual captured-content recovery, real Pi descendants,
historical linker expiry and production-release validation
remain outstanding. Private evidence and the validation-only harness amendment
are retained locally; no captured payload was examined.

### Explicit Pi provisioning and bounded execution (2026-10-05–06)

The owner authorized provisioning and selected the default local model. Pi
`@earendil-works/pi-coding-agent@0.75.4` (MIT registry metadata) installed into a
fresh private test root using existing user Bun 1.4.0. Its hash matched the bundled
Bun, but its existing path was not subject to the runtime outbound block; no
blocked executable was moved or rule weakened. Bun installed 123 dependencies
and left two postinstall scripts blocked. The scoped bundled Bun returned version
0.75.4. Existing Ollama 0.34.0 downloaded the owner-selected 6,022,236,616-byte
`ministral-3:latest` model, digest
`1922accd5827ebe6829e536369195db25eaf664528dc66206d646ea3bb386b71`.
Its provider listener was independently inspected at IPv4 loopback port 11434.
Additional exact provider rules were owner-confirmed on Oct 6 and independently
found enforced in ActiveStore alongside the existing six GUI runtime rules.

Pi's documented startup acquisition/update/telemetry opt-outs are now explicitly
set on its managed child command. DesktopCheck passed in 2m40s with 1,084 external
artifacts reused and zero rebuilt; its existing cache was explicitly adopted
after the launcher refused a missing recorded baseline. The focused command-env
regression passed (one test) in 4m31s with 921 external artifacts reused and zero
rebuilt; the existing linked-test cache was similarly reviewed before adoption.
DesktopBuild passed in 4m02s with 1,080 external artifacts reused and zero rebuilt.
The release-local GUI SHA-256 is
`2344652BAE49FDE33670273A41C7D8D32CF8DB1B83FAB247EF0906E11B2791F8`.
The prior runtime manifest was preserved and its GUI pin updated. The direct
Pi-file rustfmt check passed; the full DesktopFmt check failed on unrelated
existing formatting in commands/browser/overlay files, which were left intact.

The new tool-free headless RPC harness passed three Python tests and 17 native
firewall-validator regression cases; its inert preparation and owner-confirmation
refusal passed without inference. Pi runtime staging into a fresh GUI trial passed
synthetic copy/version/dependency/reparse/duplicate/source-preservation checks,
and the existing GUI harness passed after the launcher extension. The ordinary
nonempty-directory refusal is unchanged. These are preparation results, not a
conversation or GUI-owned process-containment pass. Runtime inference and the
separate fresh-gated GUI normal/crash cleanup check remain pending. No inference or
captured-content request, GUI launch, agent firewall mutation, commit or push
occurred during this preparation.

On Oct 6 the owner-run provider installer refused changed executable bytes before
creating rules. Independent inspection confirmed a signed Ollama Inc. update
from 0.34.0 to 0.35.1 at the same three scoped paths, unchanged model digest and
loopback-only provider listener. Both firewall stores contained none of the
proposed provider rules. The original installer was preserved privately and only
its three reviewed hashes refreshed; parsing and its actual read-only precreation
guards passed. Rule names, address ranges and restoration scope are unchanged.
The subsequent owner confirmation was verified independently before inference;
no automatic rule change or acquisition followed from this refresh.

The first headless Pi run completed two replies, persisted its synthetic marker
and exited normally, but the overall harness failed its zero-child assertion on
a Windows console host. An isolated minimal Bun command reproduced the exact
Microsoft-signed System32 helper without Pi/inference. The repaired harness
requires its live exact image/hash, direct-parent identity, held handle and exit;
it still rejects other children. Two further attempts failed model-format
acceptance (a misspelled marker, then a correct integer with a terminal period).
These failures are preserved; the revised arithmetic acceptance explicitly
permits one terminal punctuation character and makes no exact-format claim.

The final fresh headless attempt passed two completed arithmetic replies,
ordered assistant-message persistence, zero tool events, EOF exit zero and
verified console-host exit. Six Python regressions and 17 firewall-scope cases
passed. All four inference executable paths had exact enforced non-loopback
IPv4/IPv6 blocks. A fixed no-payload TCP attempt from scoped Bun failed while the
same-endpoint unscoped control connected; final native TCP/UDP snapshots found
zero non-loopback active connections/bindings for the scoped paths. Ollama's
listener stayed loopback-only. This is bounded Pi RPC evidence, not a GUI Job,
tool-execution, packet-trace or universal isolation claim; the OS console host
was not globally firewall-blocked. The idle test model was explicitly unloaded
afterward through the local API, leaving the Ollama service in place.

A dedicated opt-in, tool-free GUI validation mode is prepared for the next
fresh owner gate. It disables discovery and tools through CLI flags, skips bash
setup, restricts PATH and bounds local model context/output; the launcher requires
content capture disabled and an explicit synthetic include filter. GUI-owned
normal/crash cleanup and advisory warnings with a separate controlled RPC remain
untested. DesktopCheck passed after correcting a test-only missing type
qualification (1,084 external artifacts reused, zero rebuilt); three focused
validation-mode tests passed in 2m05s (921 reused, zero rebuilt). The GUI launcher
regression includes 32 capture/filter combinations and staging regression passed.
DesktopBuild passed in 4m10s (1,080 external reused, zero rebuilt). Its new
release-local GUI SHA-256 is
`E96DA04B05B2E6B3AC40CF5BCBC60855DA7D091FD8C873EA268BF20F8EE5207D`;
the previous runtime manifest was preserved before repinning. Direct pi.rs
formatting passed; the same unrelated full-DesktopFmt failures remain.
The inert GUI preflight passed on that hash with blocked GUI probe/successful
unscoped control, pinned 25 ms monitor and capture disabled. Provider preflight
again verified signed hashes, three exact enforced rules, unchanged installed
model digest and loopback listener. Fresh owner readiness is still required.
No GUI or capture started during these background checks. The scoped
firewall rules remain installed; no commit or push was made.

## Validated environment and important repairs

The Windows x64 development baseline used Rust/Cargo 1.93.1, Visual Studio 2026
Developer PowerShell 18.9.2, MSVC 14.51, Windows SDK 10.0.26100.0, CMake 3.31.8
and Ninja 1.13.2. Other configurations are not implied to pass.

- Cold Windows release builds and native tests linking libsamplerate use Ninja
  Multi-Config with `MinSizeRel` included in `CMAKE_CONFIGURATION_TYPES`.
  Single-config Ninja can build `samplerate.lib` successfully but place it
  outside the configuration-specific path used by `libsamplerate-sys`. The
  desktop's separate Cargo workspace needs the documented transient knf-rs-sys
  debug CRT override.
- OpenBLAS headers/libraries and its runtime DLL are separate requirements.
  Put its bin directory on runtime PATH or bundle the DLL beside the executable.
- Explicitly provision ONNX Runtime 1.22.0. The audio build verifies and stages
  the accepted DLL so Windows does not load its incompatible system copy.
- Build scripts and recorder runtime do not silently acquire FFmpeg, ONNX Runtime
  or supported local models. Missing inputs fail with setup guidance.
- The Windows packaging configuration stages OpenBLAS/ORT/Bun; FFmpeg is an
  external prerequisite. An unsigned NSIS build passed historically, not a
  signed/public release certification.
- Root default workspace members intentionally exclude the Apple-only MLX
  target from ordinary Windows builds without removing its workspace metadata.

## Verification history (summary)

September 2026 work established WGC/multi-monitor, UIA/OCR, SQLite, local audio/
Parakeet and authenticated loopback operation, followed by removal of hosted
services, telemetry and implicit downloads. Later privacy/audio fixes passed
scoped unit/build checks and selected live regressions. The latest bounded
lock/unlock and audio tail/restart results and remaining failures are in
[VALIDATION_REGISTER.md](VALIDATION_REGISTER.md).

The current public-note versions deliberately omit private machine paths, device identifiers,
process IDs, raw logs and captures. Detailed authored pre-publication notes are
preserved locally under ignored `.local/maintainer-notes/`; they are not required
build inputs, current consent or independently reproducible public test artifacts.
## Publication checkpoint checks (2026-09-16)

The reviewed source passed `cargo build --release --locked --offline` in the
documented Developer PowerShell environment (Ninja, 6m55s), Rust formatting and
the non-recording CLI `--help` smoke check. Both Rust lockfiles stayed unchanged.
With Ninja Multi-Config and OpenBLAS on PATH, targeted offline native tests passed:
9 event tests, 4 audio shutdown tests, 3 channel-lag tests, 9 Windows lock-logic
tests and 5 privacy-notice persistence tests. These use synthetic state/data.

Frontend checks passed 3 timeline notice tests and 7 URL-detection tests using
the new nine-case synthetic fixture. These are not real-world OCR accuracy results.
The publication-history helper passed 9 synthetic tests, including mixed old and
noreply commit identities. The portable harness previously passed 224 offline
tests with one Windows symlink-privilege skip; all 44 source manifest entries
were rechecked. Ten PowerShell files were parsed during portability preparation.

Existing warnings remain: unused audio `mut`, an unused Windows CommandExt
import, a test-build unused `sample_rate`, and the Vite CommonJS deprecation.
An initial help check truncated stdout early and returned a nonzero exit; reading
the complete help output returned exit 0. No capture, audio playback, model download,
live privacy test or firewall change was performed for this checkpoint. A fresh
clone's cold build, packaging and omitted-fixture tests are not claimed to pass.

Source review identified the pre-existing audio privacy/persistence race recorded
as SW-V19. This checkpoint preserves incomplete validation, not privacy certification.

## Long-trial repair checkpoint (2026-09-24)

A content-free audit of a 2h23m manual run exposed an audio shutdown timeout,
four unconfirmed raw-audio deliveries, six safe acquisition placeholders, 13
frame-link TTL expiries and excessive unavailable-password-state warnings. The
ordered raw-audio consumer no longer performs durable-backlog transcription
inline at session completion. A separate reconciliation worker is explicitly
woken and independently bounded at shutdown. Windows capture makes one bounded
retry only for an acquisition failure, frame linking classifies known capture
drops and terminally resolves deliberately discarded UI batches, and UIA keeps
fail-closed content suppression while aggregating probe-state warning noise.

The day-to-day audit now merges fixed shutdown diagnostics written after its
last health sample and infers a nonzero interrupted-native exit when no clean
marker exists. Full locked offline library suites passed: engine 560 passed with
two ignored, audio 182 passed with one ignored, screen 109 passed, and Windows
accessibility 180 passed with 22 ignored. The synthetic trial-status regression,
PowerShell parsing and Rust formatting also passed. Live confirmation of the
repaired shutdown, acquisition/link diagnostics and password-warning aggregation
remains pending and is not implied by these deterministic results.

The exact optimized executable produced from this tree has SHA-256
`6045423BF06302EBA57D0200DC1B6AB7CDF5A2C69B268D5059B4827260989EE1`.
Its complete help output and non-recording six-model check returned exit zero.
The ignored local controller configuration was repinned to that artifact. Its
normal-user preflight deliberately refused under the Codex sandbox token because
that token is not the configured recorder account; the same noninteractive
preflight must pass from the owner's ordinary PowerShell before live execution.

## Automated repair validation checkpoint (2026-10-01)

The owner reran the current preparation under the configured PowerShell 7 and
normal Windows account. A stale staged launcher helper exposed a preflight gap:
the configuration carried a reviewed helper SHA-256, but preflight did not hash
the helper that live launchers would execute. Preflight now resolves and verifies
that file before creating evidence and records the successful check. Its source
regression test enforces the ordering. After source-only synchronization and
local fixture rebuilding, the preparation passed 71 checks; the source manifest,
staging, controller and plan suites passed 48, 14, 105 and 13 checks/tests
respectively, with one expected symlink-privilege skip.

The rebuilt release executable then passed two fresh gated runs. Selected USB
output produced six audio chunks and eight transcription rows; both fixed speech
controls persisted and were found through authenticated search. The separate
five-phase native privacy run wrote 14 frames and five UI events while every
forbidden marker remained absent. Both stores passed SQLite `quick_check`; both
runs retained 403/403/200 missing/wrong/valid bearer behavior and stopped cleanly
on their first attempt. Fixed-pattern log inspection found no panic,
acquisition-failure placeholder, unexplained frame-link TTL warning, persistence
degradation, audio-shutdown degradation or delivery-loss notice. Password-state
unavailability was bounded to one logical warning represented on two log
surfaces, rather than sustained repetition.

A normal-account post-run inventory found no tested recorder/media/fixture
process, no scoped TCP or UDP endpoint and no listener on the two configured test
ports. At that checkpoint the runs had not forced a durable transcription backlog
at shutdown or generated keyboard/clipboard input in the password phase. Both
were subsequently exercised by the controlled runs below. No captured content
was inspected.

## Remaining controlled-check preparation (2026-10-01)

Background-only preparation made the three follow-up checks repeatable without
starting capture or displaying a test window. A new `input-privacy` controller
mode uses fixed synthetic ordinary typing, password typing and password paste.
It requires a positive ordinary UI-event marker, zero password-marker persistence
and a positive numeric `uia_password_content_suppressed` delta. Its log reader
returns only the fixed aggregate counter and fails closed on a malformed matching
line. Clipboard restoration remains gated on verified recorder stop.

The DRM fixture now claims every command before an action can call
`Application.DoEvents`, preventing its timer from executing the same command
again during message-loop re-entry. The fixture's non-UI self-test covers pump
re-entry and duplicate claims. The existing synthetic image-redaction unit check
now also proves that a filtered region and an unrelated PNG sentinel remain
pixel-exact, bounding the black rectangle to the selected region. These are
synthetic mechanics checks; they do not establish behavior for real protected
media or detector accuracy.

The separate JPEG response path now has a deterministic synthetic regression
that feeds fixed OCR coordinates through PII-region detection, checks substantial
decoded-pixel change inside the resulting padded region and allows only bounded
JPEG drift at a distant ordinary control pixel. Edge clamping, outside-bounds
handling and six samples from the source-free failure placeholder are also
checked. In the documented Developer PowerShell environment, all 70 focused
`screenpipe-core` PII tests and all eight focused engine image-redaction tests
passed, as did 13 focused `screenpipe-redact` image/worker tests. This does not
exercise OCR recognition accuracy or the optional RF-DETR model.

The portable harness now includes a pure fixed-region pixel evaluator and a
fresh-gated headless live sequence. Its tests require a changed, strongly blurred
synthetic PII region, preserved ordinary/sentinel regions, a source-free failure
result, a persisted positive control, the 403/403/200 bearer matrix and complete
cleanup. The controller inserts fixed synthetic JPEG and OCR rows into only its
fresh store, so the live route does not depend on OCR recognition. A separately
verified missing-OCR row exercises the application's normal fail-closed response
without adding a production failure-injection hook. Audio, desktop, keyboard and
clipboard capture are disabled; the normal UI recorder can still persist focus,
app-switch and fixed privacy-status metadata.

The fresh run `pixel-20261001-0753` passed after a 71-check normal-user preflight.
The authenticated frame route reported one redacted region: every pixel in the
fixed PII ROI changed, retained luminance contrast fell to 2.78%, the ordinary ROI
had zero above-tolerance changes and the distant sentinel changed by 0.04%. The
verified missing-OCR case replaced 99.20% of the whole source image and 98.21% of
the PII ROI with the safe failure placeholder. A byte-identical ordinary clear
frame supplied the positive control. Missing, wrong and valid bearer requests
returned 403/403/200; `/health` and all four standard protected endpoint checks
passed. The fresh store held only the three synthetic frame rows, zero audio rows
and three UI metadata/status rows, and SQLite `quick_check` returned `ok`.

Recorder shutdown, listener closure and exact-path process quiescence all passed.
Independent inspection found no scoped process and no TCP or UDP endpoint on the
API port; the fixed failure/degradation log scan had zero hits. No UI-event body
was inspected. This validates only fixed persisted geometry through the live
on-demand frame route. It does not establish OCR recognition accuracy, async
RF-DETR worker behavior, real-screen privacy or general PII coverage.

The current synthetic DRM preparation remained runnable after these additions:
the fixture self-test, 95 controller tests and a refreshed 71-check normal-user
preflight passed. No readiness gate was created and no recording, fixture window,
audio playback or interactive phase was started.

A subsequent fresh gated synthetic DRM run passed the scoped pause/recovery
contract. Its first launch refused before gate consumption or recording because
the two freshly rebuilt fixture executables no longer matched their reviewed
generated-asset pins. After those self-tested files were repinned and the
71-check normal-user preflight passed again, the controller observed 28 protected-
phase samples. The fixed before and after speech controls each persisted once;
the protected speech marker, protected capture markers and their post-recovery
recheck all remained zero. The selected output device was exact, `/health`
returned 200, and four protected endpoint families each returned 403/403/200 for
missing, wrong and valid bearer credentials.

The fresh store recorded five audio chunks, six transcription rows, 11 frames
and three UI events and passed SQLite `quick_check`. Recorder shutdown succeeded
on its first attempt; playback, fixture and DRM cleanup, exact-path process
quiescence and the clean-shutdown check all passed. Independent post-run inspection
found no scoped process and no TCP or UDP endpoint on the API port. A log-only
fixed-pattern scan found no panic, queued-work discard, unconfirmed worker,
incomplete audio shutdown, persistence degradation, possible loss, acquisition
failure or DRM pause/resume failure. Only fixed synthetic marker counts and
content-free metadata were inspected. This does not exercise real protected
media, DRM-provider diversity, continuous network observation or DRM bypass.

Three fixed tail-test WAVs were generated directly to the ignored preparation
directory with no playback. Independent validation confirmed their hashes,
mono 16-bit 22.05 kHz PCM form, bounded levels and expected durations. The
preferred stop/restart collector passed its 77 offline tests and a separate
16-asset pin manifest now closes over the collector, evaluator, generator,
validation metadata and speech assets.

The fresh live partial-buffer run then passed the scoped final-tail contract.
Before stop, the 60-second configuration had written zero chunks. Graceful stop
before its 62-second normal-emission threshold produced exactly one 13.87-second
selected-output chunk, measured by `ffprobe`, and both fixed baseline and tail
markers joined that chunk. A distinct recorder process reopened the same fresh
store, found both markers through authenticated search, persisted and found a
separate restart control in a new chunk, and shut down cleanly. Both processes
exited zero without force, unresolved workers, shutdown issues, device recovery
or privacy transitions; bearer checks were 403/403/200 before stop and after
restart. Fixed-pattern inspection of five local log files found no queued-work
discard, unconfirmed worker, incomplete audio shutdown, persistence degradation,
possible-loss or panic marker. Post-run inspection found no tested process or
listener; closed loopback connections remained briefly in `TIME_WAIT`. This is
process-level partial-buffer recovery, not same-process `AudioManager::restart`,
active-meeting shutdown, diarization quality or every-device coverage.
The preserved first live result still contains the preparation-era
`collector_live_verified: false` value because the collector returned that value
for mocked and real backends alike. A subsequent reporting-only correction marks
only an explicit Windows execution as live, and only after all cleanup succeeds;
failure and cleanup-degraded results remain unverified. The original evidence was
not rewritten.

The first live `input-privacy` attempt remained privacy-safe but was incomplete:
the password-suppression aggregate increased by 23, both forbidden synthetic
markers remained absent and cleanup passed, while the ordinary typing marker
appeared only in a frame and not in `ui_events`. The fixture had re-focused the
ordinary control immediately before `SendKeys`, invalidating the deliberately
short UIA privacy permit and causing a fail-closed race. The typing action now
requires the controller's already verified stable focus and does not re-focus;
a source regression check enforces that ordering. A corrected fixture rerun
produced the same safe result and ruled that race out as the full cause.

A production defect consistent with the remaining failure was in the UIA probe:
it timestamped the decision before several cross-process provider calls. A probe taking longer than the
75 ms maximum age therefore stored an already-expired ordinary-field permit.
The completion timestamp is now taken only after all UIA calls and focus checks
finish. Generation and native-focus comparisons remain unchanged, so focus
changes during or after the probe still fail closed. A regression test simulates
a probe longer than the maximum age and requires a fresh permit at completion.
All 203 `screenpipe-a11y` library tests completed with 181 passing and 22 ignored;
the locked offline release build and 71-check normal-session preflight passed.
The affected live rerun remained privacy-safe but produced the same missing
ordinary UI-event control, so the completion timestamp defect was real but not
the sole live cause on this machine. The suppression warning now includes only
bounded numeric counts for five fixed admission-denial classes: unavailable
decision, worker-lock contention, generation change, native-focus mismatch and
stale decision. No key, clipboard value, UIA string, title or identifier enters
that diagnostic.

The narrow Windows live regression then reproduced the remaining defect inside
`screenpipe-a11y` without starting the recorder or creating a capture store. The
real low-level keyboard hook and UIA worker used compact fixed ordinary/password
controls. Before repair, UIA property calls took at most 5.6 ms but completed
probe gaps reached 94.9 ms because the same STA also performed accessibility-tree
work; an ordinary character was denied as stale while password input remained
fully suppressed. Password-state polling now runs on a dedicated STA, while the
75 ms fail-closed permit lifetime and native-focus/generation checks remain
unchanged. Its target interval was reduced from 50 to 25 ms after the first
passing run left only 7.5 ms of observed scheduling margin.

The final gated two-cycle regression delivered both fixed ordinary markers,
delivered zero fixed password markers, and counted all 14 characters in each
password phase as unavailable-decision suppressions. It recorded no stale or
other ordinary denial; maximum probe duration was 14.5 ms and maximum completed-
probe gap was 46.8 ms. The complete locked offline `screenpipe-a11y` library
suite passed 183 tests with 23 ignored. One documented Developer PowerShell
release build completed in 5m00s; the resulting executable has SHA-256
`5652BB3E297B43259B6293BE8DBB5821FA168C91E3A2637A7010401524280A2E`.
After repinning only that ignored local artifact, normal-account preparation
passed 71 checks. The first full-application retry failed safely before accepting
synthetic input because the fixture's managed `INPUT` union omitted the larger
native `MOUSEINPUT` member required for the 64-bit ABI. It persisted no forbidden
marker, restored the clipboard and completed clean process cleanup. The fixture
now models the complete native union and self-tests the required 40-byte x64
layout; archive and controller regression tests cover the background-input and
UI-thread-pump path.

The corrected fresh `input-privacy` run passed the scoped full-application check.
One fixed ordinary keyboard marker reached `ui_events`; fixed password typing and
password clipboard markers both remained at zero; and the safe password-gate
aggregate increased by 23. Missing, wrong and valid bearer credentials returned
403/403/200, SQLite `quick_check` returned `ok`, and six frames and five UI events
were retained. Recorder stop succeeded on the first attempt, the clipboard was
restored, the fixture closed, clean shutdown passed and no owned process remained.
This validates only the fixed native synthetic controls, not arbitrary providers
or all race timings.

## Day-to-day trial repair checkpoint (2026-09-21)

A private roughly nine-minute Windows trial produced 213 captured and 213 written
frames, 138 UI events and 16 audio chunks, with zero reported frame drops, pipeline
stalls, persistence degradation, event-delivery loss, near-capacity audio notices,
audio-shutdown degradation, panics or owned processes after exit. All 16 audio
chunks were rejected by VAD and no transcript was produced, so the run did not
establish live transcription. Captured content was not inspected.

The trial exposed an orphan low-level UIA tree producer (150 bounded-queue-full
warnings), duplicate multi-monitor frame correlations recorded as 101 false TTL
evictions, repeated Parakeet-unavailable warnings, ambiguous model file errors,
and a null launcher exit code despite a logged clean shutdown. The engine now
keeps UIA focus/password/input checks while disabling that unconsumed tree
producer, deduplicates resolved frame correlations, exposes frame-link counters
in `/health`, distinguishes requested-but-unavailable transcription, reports
model access errors accurately, logs Parakeet availability transitions once,
and requires the exact executable's non-recording model preflight before a
Parakeet trial. The launcher infers exit 0 only from a new clean-shutdown marker.

One post-capture privacy evaluation failed closed into a visible placeholder and
recovered on the next observed frame. Ten unavailable password-state checks
suppressed keyboard/clipboard content as designed. These are retained warnings,
not proof of unsafe disclosure. The optional smart text-PII pack was absent, so
regex-only reconciliation ran; the pack's separate noncommercial license was not
silently accepted or provisioned.

The repaired tree passed an offline release build. The exact release binary
verified all provisioned audio models. Full locked offline library suites passed:
screenpipe-engine 554 passed, 2 ignored; screenpipe-audio 179 passed, 1 ignored.
The day-to-day launcher preflight then passed against the exact active firewall
group with `AudioModelsReady=true` and recording disabled. No post-fix live
capture or transcription run is claimed.

## Published-checkout build and initial run (2026-09-16)

A clean `screenwise-public` target built offline from the published source at
`1b22c5b8443659142bab84d50f6d89829b5c47a4` using explicitly provisioned native
inputs. The first attempt exposed the single-config libsamplerate output layout
above. After the scoped dependency cleanup, Ninja Multi-Config with `MinSizeRel`
enabled completed the optimized build in 6m29s. `screenpipe --help`, `screenpipe
doctor`, Rust formatting, `git diff --check`, and nine release-profile
event-pressure tests passed. Doctor confirmed screen-recording, microphone and
accessibility permissions, FFmpeg discovery and port availability. The staged
ONNX Runtime and OpenBLAS DLLs matched the documented hashes.

A fresh audio-disabled initial run started the authenticated API, discovered all
three monitors, created its SQLite store and listened only on `127.0.0.1:3030`.
Health returned 200; protected search returned 403 without or with a wrong token,
and 200 with the data-directory token. The DRM gate activated immediately and
safely stopped vision/UI acquisition, so this run does not establish frame, UIA
or OCR writes from the published checkout. The controlling PTY did not deliver a
Windows console-control event, so that first verified test PID was stopped
explicitly. A second fresh run used the recorder's watched-process shutdown path;
it stopped UI capture and VisionManager and logged `shutdown complete` with exit
code 0. This exercises orderly managed shutdown, but not Ctrl+C delivery from a
normal Windows console. No captured content was inspected.

A subsequent one-time-gated synthetic privacy run used the same published
checkout and release binary with audio disabled. Known allowed text was admitted
before and after the privacy phases: the fresh store contained 13 frames, five
UI events and three frame matches for the allowed marker. Password,
excluded-foreground and excluded-background phases each produced zero matches
for their fixed forbidden markers across frame text, transcripts and UI events.
The run also persisted one fixed `windows_lock` notice with reason
`wts_session_unlocked`. Health returned 200; protected search, audio-device
status, capture-events and audio-metrics endpoints each returned 403 for missing
and wrong bearer tokens and 200 for the correct local token. SQLite
`quick_check` returned `ok`, the recorder logged a clean managed shutdown, and a
separate post-run inspection found no owned recorder/fixture processes or TCP or
UDP endpoints on the two test ports. The run made no outbound diagnostic
attempts. This establishes only synthetic text admission for the exercised
fixture phases; it does not establish image-pixel redaction, clipboard safety,
other-monitor behavior, real DRM behavior, audio capture/transcription or
firewall drop enforcement. No captured content was inspected.

A fresh current-binary capture verification then exercised the same synthetic
fixture. Its first gated attempt stopped before recording because fixture focus
could not be verified; scoped cleanup passed. The retry started Windows capture
for all three detected monitors and passed the fixture phases. The fresh SQLite
store contained 15 frame rows referencing 15 distinct existing snapshot files,
including three admitted frames with image hashes, accessibility trees and the
known synthetic marker in accessibility text. Twelve other rows were explicit
privacy placeholders. Five UI-event rows recorded two app switches, two window
focus changes and one safe privacy notice. SQLite `quick_check` returned `ok` at
the end of this capture run, and the controller completed a clean shutdown.

Because those admitted frames selected accessibility text rather than OCR, a
separate vision-disabled restart invoked the authenticated on-demand frame-text
endpoint for one reviewed synthetic frame. Windows Native OCR persisted one
`ocr_text` row containing the known marker. No new capture was enabled and no
arbitrary captured text or image pixels were inspected. The stock Python SQLite
library could not repeat `quick_check` after application vector indexes were
loaded because it lacks the registered `vec_length()` function; direct reads
confirmed the OCR row after process exit. The watched-process signal was logged
but this auxiliary restart did not finish within the wrapper's 45-second limit
and was force-stopped. A final independent check found no recorder or fixture
processes and no TCP or UDP endpoints on the test ports. This shutdown warning
does not invalidate the persisted WGC/UIA/OCR evidence, but it is not a clean
shutdown result for the auxiliary OCR restart.

The Windows OCR integration target compiled and listed successfully without
executing capture/OCR. Its external-fixture test and the two Apple OCR fixture
tests are explicitly ignored pending operator-provided reviewed fixtures.

## Fixed outbound firewall diagnostic (2026-09-16)

The release CLI now has a hidden developer diagnostic,
`diagnostic outbound-tcp-probe`. It accepts no address or other network
arguments and makes exactly one TCP connection attempt to `1.1.1.1:443` with a
five-second limit. A completed handshake is closed immediately; the diagnostic
does not perform DNS, TLS, HTTP, application reads or application writes.

The locked offline release build passed. The active Windows Firewall rule named
`ScreenWise-Offline-20260916-1b22c5b84-recorder` was enabled, outbound, blocking
all non-loopback IPv4 and IPv6 addresses, and scoped to the exact rebuilt
`screenpipe.exe` path. A one-attempt unscoped PowerShell control completed the
TCP handshake to the fixed endpoint. The ScreenWise diagnostic then returned
`PermissionDenied`, reported zero application bytes and exited 2. This
comparative result, together with the exact ActiveStore rule inspection,
establishes blocking for this IPv4 TCP attempt. It is not packet-drop tracing,
continuous coverage, a UDP result or an externally routed IPv6 result.

## Current-build selected-output transcription (2026-09-16)

The rebuilt release binary then passed the prepared short `audio-output` batch
with the exact selected device `Headphones (2- Realtek USB2.0 Audio)`. The
controller waited for the capture handle before playing two fixed local speech
fixtures through Windows default output. Six audio chunks and nine local
transcription rows were written. The fixed `silver cedar` and `amber window`
markers each appeared once in persisted transcription rows and were each found
through authenticated local search. No unrelated marker appeared. The fresh
store's SQLite `quick_check` returned `ok`; protected API checks retained the
403/403/200 missing/wrong/valid bearer behavior; playback stopped and the
recorder, audio workers and fixture shut down cleanly on the first attempt. An
independent post-run check found no owned processes and no TCP or UDP endpoints
on the two test ports. No captured audio or transcript body was inspected beyond
the allowlisted synthetic-marker counts. This establishes the two known phrases,
selected-device routing and persistence/search, not full-clip transcription
accuracy, diarization quality, every audio device or final-tail preservation.
The final staged tree excluded 45 inherited asset paths while preserving their
local bytes. It contained no active upstream workflow, private evidence path or
maintainer private-email match in the 520 changed text files checked. A separate
bounded credential-pattern scan found no high-confidence real credentials;
neither scan is an exhaustive security audit. All 43 local links in the nine
primary guides and the three edited Tauri JSON files validated.

## Audio privacy and queue diagnostics (2026-09-16)

The audio persistence path now retains its database write exclusion through the
post-commit privacy-generation check and performs exact compensating cleanup
before returning a stale result. Covered writes are raw chunk rows and their
files, combined chunk/transcript writes and overlap edits, live and reconciled
diarization runs and segments, and live meeting transcript segments. The tests
force generation invalidation after commit and inspect durable state, including
restoring pre-existing chunk state and overlap transcript text. Speaker identity
matching remains a separate open boundary: it can add an embedding, update a
shared centroid or create a speaker, and safe reversal needs transactional
before-images plus ownership-aware compensation.

Audio delivery diagnostics now use fixed queue enums and numeric counters only.
The device-capture, recording, transcription-result, meeting-tap,
meeting-provider, meeting-final and meeting-persistence queues report an 80%
near-capacity transition, 50% recovery, exact confirmed drops where known, and
separate possible-loss counts when shutdown or task failure prevents completion
from being verified. Fixed allowlisted messages are written to local logs and
the existing general activity timeline; current latched state is exposed by
`/capture-events`. The deterministic suites passed 35 event tests, 93 database
tests, 179 audio tests with one ignored test, and six scoped engine notice tests.
The documented Developer PowerShell environment also completed a locked offline
release build in 7m17s.
After installing the exact dependency graph from the unchanged `bun.lock`, the
timeline component test passed 5/5 and `tsc --noEmit` passed. The complete
frontend command then passed 37 Vitest files/404 tests and 13 Bun files/150
tests. This includes the previously excluded text-overlay file after removing
obsolete generic-click and persistent-underline expectations; the second stale
exclusion named an OCR test removed by the existing frame-text hook replacement.
Both exclusions were removed. Renaming the Vitest config to the explicit ESM
`.mts` extension also removed the Vite CommonJS deprecation warning. These tests
used installed Bun 1.4.0, now also declared by the manifest and packaging pins;
the lockfile was not rewritten. Real overload/soak and visual packaged-timeline
checks remain outstanding.

The reusable interactive controller now also contains an inert 25-second
browser/password/clipboard sequence with five fixed phases, strict positive and
forbidden synthetic marker deltas, and verified cleanup requirements. Its 82
offline Python tests passed. It does not start a listener or browser and does
not read or modify the clipboard. Live execution stays disabled until the
controller can own and verify the browser process tree and the exact browser
executable receives a separately reviewed firewall scope.

## Unsigned Windows desktop build (2026-10-02)

The documented Visual Studio 2026 Developer PowerShell, OpenBLAS, ONNX Runtime,
Ninja and locked/offline Cargo environment produced the unsigned release desktop
executable and NSIS bundle. The executable SHA-256 was
`9BF38535F717F6F3FE9D16D6B4165507CF94B93284F66692D2A5451E87F9EA29`.
Tauri was invoked through its installed JavaScript entry point because the local
Bun-generated `.bin` launcher was corrupt; dependencies and `bun.lock` were not
reinstalled or changed. The frontend now uses its configured local/system
monospace stack instead of fetching Google Inter during the build.

The optimized Next static export, TypeScript check, release Rust compile and
unsigned NSIS packaging passed. The focused privacy-default Vitest regression
and both content-free PowerShell trial regressions also passed. Existing
warnings remained for `unpdf` `import.meta` bundling and unused/dead Rust paths;
neither stopped the build. See `VALIDATION_REGISTER.md` for the bounded GUI live
result and its unresolved timeline, UIA/screen-text and transient-helper limits.

## GUI trial follow-up candidate (2026-10-02)

The first GUI trial's three bounded findings were traced to concrete local
paths. Persisted/default settings could overwrite the active non-default API
port during frontend initialization. Vision output appended `data` to an
already-final data directory, while Tauri's static asset scope did not include a
custom trial directory. Async text reconciliation also cleared sanitized UIA
tree JSON and accessibility elements even when its output was byte-identical.
Finally, Windows icon discovery launched PowerShell for Appx and recursive file
enumeration, creating unscoped shared-system descendants.

The candidate makes active IPC authoritative after initialization, retains the
configured/environment port in the native cold fallback, writes media directly
to the configured final directory and grants the asset protocol only that
run's recursive media tree. Unchanged async-redaction results now retain the
already-sanitized structure; changed results still clear structured derivatives
fail-closed. Icon discovery now uses the registry, Start Menu shortcuts and
in-process bounded directory inspection without spawning a shell.

Background validation completed without starting recording:

- 39 Vitest files/406 tests, 13 Bun files/150 tests and `tsc --noEmit` passed.
- The desktop suite executed 134 passing tests with four ignored; its sole
  failure was a bytewise CRLF mismatch in the generated-binding guard. The guard
  now normalizes line endings and its focused rerun passed. This included the
  active-port fallback and no-shell nested-shortcut regressions.
- The redaction crate passed 101 unit and three integration tests; one doc test
  remained intentionally ignored. The engine final-media-directory regression
  passed in the documented native environment.
- The fixed public-marker GUI capture fixture and count-only evaluator passed
  their native Windows PowerShell 5.1 self-tests. They access no clipboard,
  keyboard hook, audio device, recorder data or network during preparation.
- The locked offline root release build passed in 10m36s. A standalone desktop
  `cargo build --release` reached link but failed with a Tauri/MSVC CRT link
  mismatch; it is not claimed as a pass. The documented Tauri path,
  using the already-built frontend through the direct installed Node entry point,
  then produced the unsigned executable and NSIS bundle successfully. The GUI
  SHA-256 is `837620145F897D8D59B24E1D1F3CC316B7362618C7769A6ECEBFF5DC586A72BE`.
- The private runtime manifest was refreshed. Its five exact executable paths
  passed firewall-rule inspection and the fixed scoped-blocked/unscoped-control
  TCP preflight on port 31579; local audio-model preflight also passed. The
  preflight explicitly reported `recording_started: false`.

One short interactive GUI run remains. It should use a fresh data directory and
the fixed-marker fixture to confirm initial non-default-port timeline loading,
readable new media, positive UIA and OCR marker counts, and absence of transient
PowerShell/conhost or other unexpected network-capable children. No arbitrary
captured content needs review. These live behaviors are not claimed by the
background results above.

## GUI cache and process-scope repair candidate (2026-10-02)

The short GUI follow-up wrote 40 WGC frames and observed both fixed UIA and OCR
controls twice. SQLite writes, 403/403/200 bearer behavior, the loopback-only
listener and sampled absence of non-loopback endpoints passed. The timeline then
became unresponsive while a shared WebView2 profile replayed cached media paths
from an earlier data store. The process monitor also observed `setx.exe` and
`where.exe` helpers (with `conhost.exe` descendants), although none had an
observed network endpoint. The GUI was force-stopped at the owner's direction,
so clean shutdown did not pass and the run is not acceptance evidence for the
repaired candidate.

Frontend timeline cache keys now include an opaque SHA-256 namespace derived
from the active canonical data directory. Cache loading waits for the native API
configuration and refuses to use a cache without that namespace. Each GUI trial
also receives a new WebView2 user-data directory. Startup no longer persists an
Ollama origin with `setx`, and Pi/FFprobe discovery enumerates `PATH` in-process
instead of launching `where.exe`. Routine successful PII-redaction messages were
reduced to debug level while privacy, failure and possible-loss notices retain
their existing warning/status paths.

The same audit removed two broader shell risks. A busy configured API port is
now checked by an in-process loopback bind and fails safely; ScreenWise no longer
uses `netstat`/`taskkill` (or the Unix equivalents) to terminate an unknown port
owner. Windows note and shell targets use the native Tauri opener, require an
absolute local path or an explicitly allowlisted `shell:AppsFolder`/
`ms-settings` URI, and never pass user data through `cmd.exe`.

Background validation completed without starting recording:

- The optimized Next export and TypeScript validation passed. The complete
  frontend suites passed 40 Vitest files/408 tests and 13 Bun files/150 tests.
  The pre-existing `unpdf` direct-`import.meta` webpack warning remains.
- The content-free GUI harness regression and rebuilt process-monitor self-test
  passed. They enforce the isolated WebView profile and reject the removed
  `setx`, `where`, shell port-cleanup and `cmd.exe` opener paths.
- Rust formatting passed. The documented locked/offline Visual Studio native
  environment produced a warning-free `release-dev` desktop build. Its exact
  executable SHA-256 is
  `3F39E62CE8D6201ED0C28D978A90085F56566D501BC5DBC6706A2577775A1A8E`.
- The private runtime manifest was refreshed for that exact `release-dev`
  executable, its adjacent Bun sidecar, the release FFmpeg/FFprobe support files
  and an isolated private WebView2 runtime. No firewall rule was created or
  changed.

The desktop test cache was moved to the ignored repository-level
`target\desktop-tests` directory. This avoids the sandbox identity restriction
of the former per-user cache and keeps native CMake/MSVC paths short enough for
libsamplerate. After discarding only moved-cache build-script state, the locked,
offline Visual Studio environment completed the linked desktop test build. Both
occupied/released port tests and all three Windows opener-classification tests
passed. A fresh short GUI run still remains required for timeline/media
behavior, helper absence and clean shutdown.

## GUI diagnostic and Pi provisioning candidate (2026-10-02)

The next short GUI trial completed its fixed UIA/OCR control, authenticated
loopback, SQLite-write, process-scope and clean-shutdown checks, but exposed two
usability gaps. Privacy placeholders displayed only a generic redaction/failure
label, and attempting AI Chat with no Pi runtime produced a brief message with
no directly runnable recovery command.

Windows placeholder records now carry one of eight fixed, content-free policy or
failure markers. Timeline maps only exact allowlisted markers to explanatory
copy; it never renders arbitrary frame/OCR text as a diagnostic. The messages
distinguish active-window-only masking, an excluded active window, unavailable or
inconsistent focus, monitor/active-window acquisition failure, and initial or
follow-up privacy-check failure. This reveals a safe reason category, not a
window title, URL, captured value or claim that a particular item was sensitive.

Missing or invalid Pi provisioning now reports the exact repository PowerShell
command with the active ScreenWise data directory. The explicit provisioner
installs `@earendil-works/pi-coding-agent@0.75.4` into a fresh staging directory,
checks its entrypoint/version/runtime dependencies, refuses to replace unexpected
existing content, and promotes the directory only after validation. ScreenWise
still performs no implicit download. AI-provider setup remains separate.

Background checks performed without starting recording passed: the focused Pi
desktop Rust test, the Windows capture-marker Rust test, two exact-allowlist
Vitest cases, TypeScript checking, PowerShell parsing and the synthetic Pi
provisioning regression. The documented locked/offline Visual Studio environment
also produced the `release-dev` desktop executable in 8m48s; its SHA-256 is
`DB5D90E01A16D3C6D17A6F8008A4E86F13CA1766959405225B0E3C461EC5A5D9`.
Live visual confirmation of the new Timeline wording and a real explicitly
authorized Pi download/start remain outstanding.

## GUI diagnostics and interrupted-session recovery (2026-10-05)

A fresh audio-disabled `release-dev` GUI trial confirmed the allowlisted
excluded-window explanation and the complete local Pi provisioning command.
It exposed an explanation banner obscured by timeline navigation, a setup
toast that expired too quickly to read, and an inherited Pi startup sweep that
launched `taskkill.exe` and a console helper. No successful termination of an
unrelated process was demonstrated. The banner has been moved below navigation,
the setup message remains selectable until dismissed, and the sweep was removed.
Native Pi inventory is count-only, best-effort and advisory; inaccessible process
metadata is reported as incomplete rather than proof that no other RPC exists.

The run lasted 240 seconds and recorded 141 captured/141 written frames in the
last sample, with zero sampled drops, stalls, TTL/update failures, loss notices
or acquisition failures. Missing, wrong and valid bearer credentials produced
403/403/200. The listener was loopback-only, and the monitor observed no external
TCP/UDP endpoint. The existing five-rule firewall group was independently
inspected and unchanged. Tray quit completed cleanly with no scoped process or
listener left. One startup WebView fetch error and a shutdown WebView class-
unregistration warning remain noted; they did not prevent this run's timeline
load or process cleanup. Audio and new positive UIA/OCR controls were not tested.

The candidate adds a durable per-directory active/clean marker held with an OS
file lock. An interrupted launch emits a closed, content-free recovery notice
through the existing status writer independently of capture admission. Known
capture, audio, status-writer or managed Pi cleanup failure prevents marking
the session clean. Existing SQLite and durable audio recovery retain ownership
of stored data; there is no automatic destructive rebuild or recovery guarantee.
Fresh managed Windows Pi trees use a kill-on-close Job assigned before execution.
Other or legacy RPC processes are only warned about, never killed by a sweep.

Background marker tests passed eight cases, including forced synthetic process
termination/restart, a cleanup-failure latch, content-free storage errors and a
startup/exit race. An atomic startup lease blocks new capture/Pi setup once exit
begins and leaves the marker incomplete if setup was still running. Native RPC classification passed
five tests, and the synthetic GUI harness passed. Two isolated Windows Job tests
passed, covering owned child and wrapper-descendant cleanup while leaving an
unrelated synthetic process alive. The persistent setup-toast regression passed.
A normal-account native snapshot recognized a hidden synthetic Bun RPC
sleeper, excluded it when its owned child handle was supplied, and stopped only
that sleeper. This exercised no Pi SDK, capture or network activity.
TypeScript checking and the refreshed optimized frontend export also passed (with the
known `unpdf` warning). The focused real-subscriber SQLite persistence test
passed, reconstructing only fixed recovery text, as did the six existing safe
notice tests and the closed recovery-payload regression. The settled desktop
`cargo check --tests` passed without warnings using the canonical test cache.
The warning-free `release-dev` build passed in 5m18s. Its GUI SHA-256 is
`D1449C915142E43F3A66C908085C689AEA133059044C49C3F9D06277FC85EE22`.
The refreshed normal-account non-recording preflight passed exact artifact/rule
and process-monitor pins, a blocked GUI-originated fixed IPv4 TCP attempt and a
successful unscoped control. It reported `recording_started: false`; audio-model
checking was deliberately skipped for this audio-disabled follow-up.
Live confirmation of the repaired layout/toast and the full app interruption/
restart path needs fresh readiness. Real Pi provisioning/provider execution
remains untested. Sessions predating marker tracking cannot be classified
retrospectively, and the desktop marker does not add CLI lifecycle tracking.

## GUI message follow-up (2026-10-05)

The second audio-disabled GUI run used the preceding `D1449C91…` `release-dev`
executable and lasted 489.1 seconds. Its last sample contained 308 captured/308
written frames, with no reported drops, stalls, frame-link TTL/update failure,
delivery loss, acquisition-failure placeholder or queue-capacity notice.
Bearer checks again returned 403/403/200; sampled external TCP/UDP endpoints and
non-loopback API listeners were zero. The five exact-path firewall rules were
independently inspected and unchanged. Tray quit returned exit zero, left
`clean-v1`, and left no scoped process or API endpoint. The 25 ms process monitor
observed no unexpected shell starts. These are scoped observations, not a general
privacy/network guarantee.

The owner's supplied screenshots established two UI failures: the reason notice
overlapped the placeholder image label, and the app's global selection CSS
prevented selecting the Pi instructions. The notice now occupies a separate
layout row above the measured image/video viewport. The persistent Pi notice
now offers native clipboard copying of just the command, selectable text and a
read-only command field; a failed copy reports a manual-copy fallback.
The desktop embeds the standalone PowerShell provisioner and prepares it under
the active data directory without overwriting conflicting files or following
junctions. Its displayed command uses quoted absolute paths independently of a
checkout. Pi remains optional for AI Chat, with explicit package acquisition,
preinstalled Bun and separate local provider/model setup.

Nine focused Vitest tests passed, as did TypeScript checking, the frontend export
(with the known `unpdf` warning), six standalone Rust helper tests and the
synthetic provisioner test. The canonical `DesktopCheck` passed warning-free in
1m52s, reusing 1,084 dependency artifacts with none rebuilt. The new launcher
initially refused the existing cache because it lacked a launcher baseline;
after reviewing the unchanged native configuration and cache location,
`-AllowColdCache` explicitly adopted that cache. No dependencies were downloaded
and no cache was cleaned or moved.

The linked desktop regression
`pi::tests::missing_pi_message_gives_an_exact_recovery_command` passed through
`Invoke-ScreenWiseBuild.ps1 -Task DesktopTest -TestFilter
missing_pi_message_gives_an_exact_recovery_command -AllowColdCache`. This run
took 6m15s, reused 919 dependency artifacts and compiled two missing Windows
dependency variants; no unexpected dependency invalidation was reported.
After moving the Copy command action above the potentially long instructions,
the nine focused frontend tests and TypeScript check passed again.

Startup WebView fetch failure, one UIA privacy-initialization retry and one
optional smart-PII reduced-coverage notice remain noted. UI automation returned
incomplete controls/background pixels, so the owner's screenshots supplied
visual evidence; further screenshots were avoided. Audio, new privacy positive
controls, full-app crash/restart, real Pi/provider execution, and the repaired
banner/copy-button visual check were not performed in this follow-up.

The refreshed frontend export and settled canonical
`Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild -AllowColdCache` passed. The
warning-free `release-dev` build took 5m57s and reused 1,080 dependency artifacts
with none rebuilt. Its desktop SHA-256 is
`0C50DB401F4EA1B2CD985E2422BCA5689341D44BD716D3E7ADEB064C913EEDD4`.
The ignored runtime manifest now pins this candidate at the existing exact
executable path; this is not a production-release or installer validation.
The normal-account, non-recording preflight passed against that hash: all five
existing exact-path rules and the process-monitor pin matched, the GUI-originated
fixed IPv4 TCP probe was blocked, and the unscoped control succeeded. It reported
`recording_started: false`; audio models were deliberately not checked for this
audio-disabled follow-up. A fresh owner readiness gate is required before the
prepared banner/native-copy GUI check starts. No package download, firewall
change, new capture session, commit or push occurred during these repairs.
