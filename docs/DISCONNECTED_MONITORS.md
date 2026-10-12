# Disconnected monitor recording and status (2026-10-12)

SW-R16's obsolete monitor rows retained lock/redaction checks for days and could
affect the dashboard summary after Windows changed its native display IDs.
Session lock applies to the whole session; an old per-monitor check is not
evidence that just that display is still locked.

The recorder watcher now reconciles a successful monitor enumeration before
starting replacement producers. It stops removed capture and HD tasks, including
HD-only handles, and retires current diagnostic states with the fixed
`monitor_disconnected` reason. This also reconciles scopes whose tasks already
exited and diagnostic rows retained across a desktop recording-session restart.
Queued and persisted diagnostic history remains available in the Status log.
Reconnection of the same native ID clears its retired current observations and
capture/storage evidence; fresh producers must report again. Existing monitor
selection and privacy/policy gates still control recording startup.

The authenticated `live_status.monitor_topology` field reports bounded numeric
IDs, nullable connection state, last successful enumeration time per monitor,
its monotonic age, and topology availability/check age. It contains no screen
content, window titles or hardware names. A historical scope never observed in
the current session's enumeration has an unavailable last-seen time.
Windows/Linux successful empty xcap enumeration means all monitors are removed;
permission, enumeration and task errors do not establish removal. macOS's
ambiguous no-monitors error remains unavailable. DRM/schedule paths that skip
enumeration also mark topology unavailable rather than infer disconnection.

Fresh confirmed disconnected scopes do not contribute to overview icons,
data-type/group summaries or recording-error notifications. Floating Sources/
All data and the expanded data-type drill-down show **Disconnected** with a
last-seen timestamp when available. Their old observer reasons are available in
history rather than presented as a current lock/error. With no connected
recording monitors, the summary says so. Current monitors with stale producers
remain visible with stale/unconfirmed status. Missing, unavailable or stale
topology never silently hides a source. The frontend topology age limit is
90 seconds to cover the existing macOS 60-second watcher backstop; ordinary
Windows polling remains five seconds. Transport time is included in check ages.

## Validation and limits

200 distinct focused checks passed: 124 frontend checks across seven Vitest
files, all 62 config unit checks, one engine recorder-task lifecycle check and
13 existing notice/API persistence checks. The final TypeScript no-emit check,
canonical RootFmt and 16-page Next static export passed. The existing `unpdf`
import-meta warning remains. Frontend tests use one worker and the repository's
explicit loopback-only Vite configuration; no acquisition hooks were invoked.

Native commands use the canonical Windows launcher, its configured native inputs,
locked/offline resolution, established caches and two jobs:

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-config -Lib -Jobs 2 -TestArguments @('--test-threads=1')
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter disconnected_monitors -Jobs 2 -TestArguments @('--test-threads=1')
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter privacy_notices::tests -Jobs 2 -TestArguments @('--test-threads=1')
```

All 52 external config-test artifacts and all 685 external engine-test artifacts
were reused; no external variants or unexpected rebuilds occurred. The settled
engine lifecycle test compiled changed workspace dependencies in 2m09s; the
subsequent notice suite reused all 16 workspace artifacts. The first config run
hit existing shared-global interference under parallel test execution. A serial
rerun exposed a new test's incorrect first-row assumption, corrected to find its
exact synthetic ID among retained scopes. The settled serial suite passed.
These failed attempts are not counted as validation. Source formatting used
`rustfmt --edition 2021` on the four changed Rust files because the launcher only
checks formatting; the final canonical RootFmt passed. Sandbox realpath access
blocked the initial Rust/Vite attempts; authorized host-context retries passed.

Recorder `RootBuild -Jobs 2` passed in the **release-local** profile in 4m09s:
785 external artifacts reused, zero rebuilt/new variants/unexpected rebuilds;
11 workspace artifacts reused and eight rebuilt. Evidence:
`20261012T074649454Z-Root-Build-740dcb80`. The resulting
`target/release-local/screenpipe.exe` passed inert version/help/record-help smoke
checks (version 0.4.15), without starting recording. SHA256:
`C8A2D378482A88A7905DBAA76059A462BD6A972CF715BBC3675C18A7AE1A4236`.

Desktop `DesktopBuild -Jobs 2` passed in **release-local** in 7m19s:
1,080 external artifacts reused, zero rebuilt/new variants/unexpected rebuilds;
ten workspace artifacts reused and eight rebuilt. Evidence:
`20261012T075137959Z-Desktop-Build-940245f5`. The desktop artifact is
`apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe`
(version 2.5.28). SHA256:
`D4D5C038B2B158B487CBF679DC445A38E37BB8A497358F949DF28C4AD412705B`.
Both builds used the settled source changes on baseline `4f06e66f4`; only
documentation changed after the builds. Neither artifact was deployed or launched
for recording. Both Rust lockfiles are unchanged.

Validation uses synthetic metadata, pending recorder tasks and synthetic stores.
Hardware disconnect/reconnect, installed GUI appearance and in-flight capture
completion after physical removal remain untested. No deployment, new capture,
recording restart, firewall change or private captured-content inspection is
part of this change. This is scoped implementation evidence, not a privacy or
security guarantee.
