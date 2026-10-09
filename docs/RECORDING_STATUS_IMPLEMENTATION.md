# Recording-status implementation batch (2026-10-09)

Agreed scope: [handover](RECORDING_STATUS_UI.md), source baseline `1cac89c9e`.
The previous release-local deployment prerequisite is satisfied. Actual deployment
of this candidate and new interactive capture validation remain separate.
All 13 tasks are complete for candidate preparation. Deployment and interactive
validation remain outside this batch.

| Task | Scope | Owner | State |
|---|---|---|---|
| 1 | Preserve stale status; output gaps remain Listening | Dashboard | Implemented |
| 2 | Precise foreground/input safety failure diagnostics | Diagnostics | Implemented |
| 3 | Monitor names, topology-derived automatic names and custom aliases | Dashboard / Backend | Implemented |
| 4 | Dashboard visibility, geometry, level, tray recovery | Dashboard | Implemented |
| 5 | Three display levels and contextual help | Dashboard | Implemented |
| 6 | Wrap only main-window Status row | Dashboard | Implemented |
| 7 | Retryable media eviction and retention restoration | Backend | Implemented |
| 8 | Actual Windows fallback audio configuration | Backend | Implemented |
| 9 | Exact CORS and minimal public readiness | Backend | Implemented |
| 10 | Timestamp metadata conversion preserves captured text | Backend | Implemented |
| 11 | Deployment auto-start after existing checks; prepare-only | Integration | Implemented |
| 12 | Focused regressions, release-local builds, docs, candidate | Integration | Complete |
| 13 | Separate sensitive diagnostics; local launch opt-in enabled | Diagnostics | Implemented |

## Concrete outcomes

1. Failed status refreshes preserve the last known state and reason, with age and
   stale information beside them. Silent output streams remain Listening when
   callbacks are absent; actual device and capture errors retain their failure states.
2. Foreground and input-safety decisions distinguish the stage that failed:
   native foreground sampling, UI Automation focus/ownership/password checks,
   unsupported input, stability and worker availability. Ordinary diagnostics use
   fixed reason codes; unknown or unstable input remains closed to recording.
3. Monitor selectors and status rows expose understandable names, resolution and
   runtime ID, plus saved custom aliases. Automatic labels use verified primary
   status, physical topology and Windows display-connector metadata. Internal
   connector evidence produces “Built-in display” or “Main display (Built-in)”;
   verified external displays can become “Main display (External top left)” or
   “External top right”. Ambiguous metadata uses a neutral display label.
4. The floating dashboard restores visibility, display level and geometry, fits
   saved geometry to available work areas and recovers through the tray. Closing
   hides it, minimization is disabled, and explicit quit flushes geometry before exit.
5. Overview, Sources and All data provide the three dashboard levels. The overview
   groups recording status for quick reading; source/session and preference
   explanations appear as contextual help near the controls they explain.
6. The main window's Status row wraps at narrow widths without changing the layout
   of unrelated settings rows.
7. Media eviction queues deletion work durably before clearing database references.
   Failed file removal survives restart and retries in both desktop and CLI server
   lifetimes; responses distinguish completed
   removal from pending cleanup. Retention uses the same path. Desktop restoration
   retries initial readiness failures and restores persisted settings once per new
   backend instance, without replacing an explicitly configured live policy.
8. Audio stream creation returns the actual opened stream's sample rate and channel
   count after playback starts, including Windows fallback configurations. Failed or
   timed-out startup does not report the rejected preferred configuration as active.
9. Local browser origins are parsed exactly, including supported loopback and Tauri
   schemes, instead of accepting string-prefix lookalikes. Unauthenticated health
   requests expose minimal readiness; authenticated health and health/details retain
   detailed status for supported desktop and integration consumers.
10. Timezone conversion changes only timestamp metadata in known response structures.
    Captured OCR, transcripts, UI Automation properties and other content remain
    untouched, even when their strings or property names resemble timestamps.
    Caller-defined raw SQL results bypass conversion and retain UTC values.
11. Deployment selects startup automatically after its existing validation checks;
    prepare-only remains available and diagnostic opt-out is preserved. Failure
    propagation and candidate manifest validation have synthetic regression coverage.
12. Focused regressions cover stale status, monitor naming, geometry, durable cleanup,
    retention startup/restart recovery, audio startup metadata, health/CORS boundaries,
    timestamp preservation, deployment behavior and diagnostic privacy behavior.
    The focused checks, final frontend export and both canonical release-local builds
    passed. The candidate inventory records the exact artifacts below.
13. Sensitive diagnostics use an explicitly opted-in, separate session-file sink.
    The next local deployment candidate enables that opt-in by default, with an
    explicit opt-out; preparing the candidate does not alter the installed app or
    start recording. Ordinary logs receive only content-free lifecycle notices.

## Limits and operational behavior

Monitor labels describe verified connector metadata and current topology. “Built-in”
means an internal connector, which can also belong to an all-in-one computer; it
cannot establish a laptop chassis. Missing, failed, mirrored or ambiguous metadata
remains unknown. Runtime monitor IDs and name/geometry-based alias keys are not
immutable hardware identities: topology or resolution changes can require alias
re-entry, and historical runtime IDs cannot reliably identify a physical panel.
Aliases are withheld when their key is ambiguous.

Sensitive session files are plaintext and can contain private window/URL metadata,
provider exceptions and capture-decision context. Access protection restricts the
Windows directory to its owner and Local System, but is not encryption and does not
protect against the same user, administrators, malware or backups. The bounded
queue can drop records, each process has a write cap, and abrupt exit can lose its
queued tail. Existing session directories have no automatic retention/deletion and
need deliberate local cleanup. The sink does not deliberately collect screenshots,
audio, clipboard, keystrokes, tokens or password-field values; exception and window
metadata can nevertheless be sensitive. See [CAPTURE_PRIVACY.md](CAPTURE_PRIVACY.md)
for the exact lifecycle, boundaries and opt-in behavior.

The owner added an explicit numeric-loopback policy for development and test
listeners. Next dev and Vite/Vitest configuration now bind or disable listeners
accordingly; synthetic listener checks intercept socket creation instead of opening
servers. The Node firewall prompt's exact trigger remains unconfirmed. No firewall
rules were changed and Node was not permitted through the firewall. Evidence and
limitations are in [DEVELOPMENT_LISTENERS.md](DEVELOPMENT_LISTENERS.md).

Native checks and synthetic tests cannot establish behavior on every real audio
configuration, display topology or UI Automation provider. This batch does not
perform a new interactive capture session, launch the candidate for live validation,
or deploy it. Those remain separate owner actions.

## Validation and candidate evidence

Previously completed milestone: 73 synthetic deployment checks passed, including
automatic startup selection, prepare-only, failure propagation, diagnostic opt-out
and manifest type validation. The development-listener regression has two passing
checks without opening listeners; the separate MCP fixture changes were inspected
but their tests were not run. At that milestone, dependencies did not resolve from
the MCP package; Vitest and TypeScript were available in the desktop frontend's
separate dependency tree. The owner subsequently parked the external-client MCP
server (2026-10-09): its builds and tests are now excluded, not outstanding candidate
requirements. Pi's direct recorder API extension remains in scope. See the
[canonical policy](../AGENTS.md#external-client-mcp-server--parked-2026-10-09).

Source milestone: `feabd15721d2e2b8462d5f64f41710bb18e15240`. Deployment startup
and listener-policy milestones are `a17770805` and `fc82384bf` respectively.

| Focused validation | Passing checks | Evidence / boundary |
|---|---:|---|
| Dashboard, stale status, monitor topology/aliases | 74 | Eight Vitest files, one worker |
| Fixed input/foreground reason presentation | 2 | Separate reason-presentation Vitest file |
| Loopback listener setup | 2 | Actual installed Vite setup with listen intercepted; no listener opened |
| Deployment/startup/manifest decisions | 73 | Synthetic deployment suite; no installed files changed |
| Sensitive sink and lifecycle reporter | 5 | Bounds, deduplication, relay and Windows DACL |
| UIA input safety | 16 | `platform::windows_uia`; 23 live tests ignored |
| Durable media eviction | 3 | `retention_` database tests, including failed unlink and later retry |
| Audio fallback/open handshake | 7 | `wasapi_format_tests` |
| Monitor metadata and foreground/rule diagnostics | 4 | Synthetic connector/ambiguity and fixed diagnostic decisions |
| Engine privacy notices, CORS/health, timezone | 25 | 13 notices, 3 access-boundary and 9 timezone checks |
| Desktop bindings, retention and dashboard geometry | 6 | Binding generation plus final five-check integration run |

That is 217 distinct passing focused checks; repeated runs and ignored/live tests
are not counted. Final TypeScript no-emit checking, the 16-page static export,
RootFmt, DesktopFmt and DesktopCheck also passed. No acquisition hooks were run.
The existing unpdf import-meta and audio unused-variable warnings remain.

The desktop retention fixture initially failed because accepted Windows sockets
inherited nonblocking mode; blocking reads with a bounded timeout repaired that
fixture. Database fixture teardown received a bounded retry for Windows sharing
violations. Production behavior assertions were retained. A final source review
also added the existing retry-worker lifetime guard to the CLI server entry point;
its queue behavior is covered above, but no live CLI capture was started.

Independent review covered the API/privacy boundary, diagnostic sink, dashboard
geometry and monitor metadata. Findings were resolved, including accounting for
window decorations, alerting on the new failure reasons, relaying fixed sink warnings
to GUI file tracing, and avoiding an unsupported laptop-chassis inference.

Both final application builds passed from source milestone `feabd1572`, using
one job, the canonical caches and the release-local profile:

| Build | Cargo elapsed | External artifacts reused / new variants | Unexpected rebuilds |
|---|---:|---:|---:|
| RootBuild | 7m59s | 784 / 1 | 0 |
| DesktopBuild | 14m13s | 1,078 / 2 | 0 |

The root build reused ten workspace artifacts and rebuilt nine; desktop reused
nine and rebuilt nine. Cache adoption was explicitly reviewed for the local
config dependency edge and Windows display API features. No cache was cleared,
no version was upgraded, and no full-release build was used. See
[BUILD_NOTES.md](../BUILD_NOTES.md#recording-status-batch-2026-10-09) for the full
check sequence and corrected intermediate failures.

The recorder passed inert `--version`, `--help` and `record --help` checks. The
GUI executable was not launched. Native DLL hashes match the configured provisioned
OpenBLAS and ONNX Runtime inputs. FFmpeg, FFprobe, Bun and 13 desktop asset files
are present.

Prepared private inventory:
`.local/build/deployment-candidates/20261009-release-local-feabd1572.json`.
It records all seven executable/DLL/sidecar hashes, assets, native input checks,
source identity and validation evidence. It is ignored by Git. Sensitive diagnostics
are enabled for the next deployment; the running installation was not changed.

| Artifact | SHA-256 |
|---|---|
| `target/release-local/screenpipe.exe` | `56A626B50A856D6B8CBBBFC08C205980EFF8959E33655D9894BF146A395DC680` |
| `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe` | `A42027CA02EB06500F5C822B49CCC52FFEA2CF0CD73AAD72BE842527DB47C7A9` |

Normal and prepare-only deployment plans were inspected. Neither plan copied files,
provisioned dependencies, changed firewall rules or started capture. Actual deployment
and new interactive validation remain separate; the candidate is prepared, not installed.
