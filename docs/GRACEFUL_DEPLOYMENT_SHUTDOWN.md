# Graceful local deployment shutdown (2026-10-09)

Normal local deployment now prepares and validates the replacement before asking
the intended installed GUI to quit. The request uses the existing Windows Tauri
single-instance transport. Tray Quit and external Quit share an idempotent cleanup
entry point that blocks new resource startup before stopping recording and server
resources. The existing exit cleanup stops managed Pi and finishes session markers.
An external request reports unsuccessful cleanup through a nonzero process exit.

The quit-only command is handled before Tokio, Tauri, logging, WebView, settings,
models or session initialization. It verifies its own executable and the target
PID/image, finds that process's single-instance window, and sends only the fixed
quit payload with a five-second transport bound. No instance means an inert
failure, never a new application session. Malformed control arguments also exit
through the early branch; control callbacks bypass normal focus handling.

Deployment validates manifest ownership, data/port configuration, registration
configuration, provisioned resources and a completed staging directory before
dispatch. Support requires deployment-recorded protocol metadata and GUI SHA-256,
unchanged GUI metadata, and the static capability marker. Legacy binaries are
inspected without execution. The retained original process handle and creation
identity prevent a PID replacement being mistaken for the intended instance;
native start times are normalized to CIM's microsecond precision. Deployment
requires exit zero and no processes from the installed binary directory before
publication. The shutdown deadline defaults to 60 seconds (explicit range 1–300).
Unknown GUI/recorder/helper executable paths abort rather than assume quiescence.
Failed dispatch, cleanup or timeout aborts without force-killing processes or
replacing in-use files. The existing startup/firewall/model/authentication/runtime
checks remain in the automatic-start flow after publication.
Accepted cleanup can finish after deployment times out; the update does not
cancel it. Missing session markers conservatively fail external shutdown too.

`-PlanOnly` is side-effect-free. `-PrepareOnly` does not request shutdown or start
the app and refuses running installed processes. A staged candidate can remain
after an aborted normal update; persistent recording data stays separate.
The external-client MCP bridge remains parked and excluded. Pi's owned direct
recording API extension remains included.

The currently installed version was inspected without sending a request and lacks
both the capability marker and protocol metadata. One final manual **tray Quit**
is required before the bootstrap deployment. See the
[deployment guide](../scripts/windows/deployment/README.md#graceful-shutdown-during-updates)
for modes, timeout and the exact update flow.

## Validation and candidate

Source baseline: `ed8f3d921` on `screenwise`. Unrelated recording-status frontend
edits that appeared during this task are preserved separately. The owner selected
separate builds; this candidate uses the successful frontend export completed
before those edits, whose 502 files are inventoried privately.

- 112 synthetic deployment checks passed, including support/integrity refusals,
  PID replacement, unavailable helper identity, cleanup failure, dispatch failure,
  deadlines, surviving helpers, staged publication and side-effect-free plans.
- Ten new desktop quit tests passed. A hidden synthetic Windows window exercised
  native transport against the test process only; no app session or focus change
  was involved. The canonical linked run took 5m46s, reusing all 921 external
  artifacts and rebuilding two workspace artifacts.
- Two existing shutdown-marker regressions passed against the same test binary:
  degraded cleanup cannot mark a session clean, and quit during startup blocks
  new startups. That run reused all 921 external and 18 workspace artifacts.
- RootFmt and DesktopFmt passed. DesktopCheck passed in 2m19s with all 1,084
  external artifacts reused and three workspace artifacts rebuilt.
- The 25 Pi direct API synthetic tests and pinned Pi 0.75.4 SDK registration,
  mocked transport and completion-guard check passed. External-client MCP was
  excluded throughout.
- Frontend type checking and all 16 static pages exported successfully through
  the direct provisioned Next invocation, with acquisition hooks skipped. The
  existing `unpdf` import-meta warning remains.

- DesktopBuild passed in 8m38s using `release-local`, one job and the canonical
  locked/offline cache: all 1,080 external artifacts reused, zero new variants or
  unexpected rebuilds, 17 workspace artifacts reused and only the app rebuilt.
  No dependency versions, lockfiles or features changed; no acquisition or
  full-release build was used. The unaffected recorder artifact was reused and
  passed inert version/help/record-help checks without capture.
- The built release-local GUI passed malformed and missing-target quit-only
  smoke checks: both returned exit 2 within ten seconds and created no session,
  data or local-app-data directories. The installed GUI remained running at its
  original PID; no request addressed it.
- Candidate native DLL hashes match configured provisioned inputs. Seven
  executable/DLL/sidecar files and 13 assets are inventoried privately; all 502
  frontend export files retain their recorded hashes. Normal and prepare-only
  plans select the expected modes without acting on the installation.

| Artifact | SHA-256 |
|---|---|
| `target/release-local/screenpipe.exe` (reused) | `56A626B50A856D6B8CBBBFC08C205980EFF8959E33655D9894BF146A395DC680` |
| `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe` | `8BA0B0A6ED33028C8C28DA346B952629F99245A10860BBE0A89F98EB7C5116CF` |

Exact native commands (from repository root):

```powershell
& .\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootFmt
& .\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopFmt
& .\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopCheck -Jobs 1
& .\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopTest -Bin screenpipe-app -TestFilter quit -Jobs 1
& .\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopTest -Bin screenpipe-app -TestFilter shutdown -Jobs 1
& .\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild -Jobs 1
```

Plans were inspected before native compilation; no cold-cache override was needed.
The check/test workspace rebuilds reflect the changed desktop source and adoption
of the already-committed engine source in the check/test cache. No external rebuild
was observed. Real installed IPC,
shutdown during recording and automatic capture restart remain untested in this
batch. No deployment, installed-app shutdown, firewall change, captured-content
inspection or interactive capture validation is authorised by these checks.
