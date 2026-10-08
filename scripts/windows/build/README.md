# Canonical Windows Cargo launcher

Use `Invoke-ScreenWiseBuild.ps1` from ordinary PowerShell with a single preset:

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild
```

Compiling presets initialize x64 Visual Studio Developer PowerShell and the provisioned native
paths from ignored `.local\build\config.local.json`. Configure this once per clone:
copy `scripts\windows\build\config.example.json` to that ignored location and set
its three absolute paths to your Developer PowerShell script, OpenBLAS directory
and ONNX Runtime directory. This workstation's local file is already configured.
Do not commit it or download replacement native artifacts implicitly; see
[Windows setup](../../../docs/WINDOWS_SETUP.md).
The launcher uses `--locked --offline` for dependency resolution and never
cleans, relocates or deletes caches. It restores the calling shell's environment
and location on both success and failure. Provision the pinned toolchain and
needed rustfmt/Clippy components explicitly first; it never installs them itself.
The launcher sets `RUSTUP_AUTO_INSTALL=0` for child tool invocations to disable
implicit toolchain installation, as documented in the
[rustup environment reference](https://rust-lang.github.io/rustup/environment-variables.html).

```powershell
# No commands, environment changes or output files: inspect the chosen invocation.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopCheck -PlanOnly

# Batch and format related desktop edits first; check their integration once.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopCheck

# Run the required focused linked tests after the candidate settles.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopTest `
    -TestFilter tauri_bindings_are_current -TestArguments @('--nocapture')

.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest `
    -Package screenpipe-engine -Lib -TestFilter privacy_notices::tests

# One intermediate desktop build. Re-pin runtime/firewall paths after profile migration.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild

# An integration test target, independently of the filter inside that binary.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest `
    -Package screenpipe-engine -TestTarget first_frames_test -TestFilter ordinary_control

# Clippy checks the same default test targets as Check. Warning policy is explicit.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopClippy -WarningsAsErrors

# No Developer PowerShell or native configuration is needed for these.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootFmt
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopTree
$metadata = .\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootMetadata | ConvertFrom-Json
```

| Invocation | Persistent target directory | Profile |
|---|---|---|
| Root check/Clippy/test/build | `<repo>\target` | `dev` / `dev` / `test` / `release-local` |
| Desktop check/Clippy/test | `<repo>\target\desktop-tests` | `dev` / `dev` / `test` |
| Desktop build | `<repo>\apps\screenpipe-app-tauri\src-tauri\target` | `release-local` |

Both `Root` and `Desktop` support `Check`, `Test`, `Build`, `Clippy`, `Fmt`, `Tree`
and `Metadata` presets. Add a package/filter for focused test execution.
`-Workspace` plus `-Mode` remains available as an alternative to `-Task`.
An explicit `-ConfigPath` can select another reviewed local configuration;
without a configuration, the explicit workspace/mode form requires an already
initialized Developer PowerShell environment. Compiling presets require the
one-time config; lightweight presets do not load it.

Choose one target selector: `-Lib`, `-Bin <name>` or `-TestTarget <name>`.
`TestTarget` maps to Cargo's `--test` integration-test binary selector and is
supported by Test, Check and Clippy. `Bin` and `Lib` also support Build. Explicit
selectors replace the default `--tests` in Check/Clippy, keeping the check focused.
The desktop app has no library target. `-TestFilter` filters tests inside the
selected binary; it does not select that binary or limit its dependency compilation.
Clippy uses the persistent check cache and development profile; its first run may
still need additional artifacts. `-WarningsAsErrors` adds `-- -D warnings` and is
only accepted for Clippy. No automatic lint fixes are run.

`Fmt` runs `cargo fmt --all -- --check` for the selected workspace and does not
modify source files. `Tree` resolves the locked graph offline; optional `-Package`
narrows its output. `Metadata` returns format-version 1 JSON with `--no-deps` for
the selected workspace, so it intentionally omits the full resolved dependency
graph. Its output can be piped to `ConvertFrom-Json`. Lightweight presets need no
native libraries or Developer PowerShell, skip the launcher's compilation lock,
and preserve output on the success pipeline. They still record private output
and restore the caller's environment. Cargo's offline environment also applies
to any internal metadata query during formatting. `Jobs`, `Diagnostics`, target
selectors and test-runner/lint options are rejected where they do not apply.

The explicit `SCREENWISE_DESKTOP_CARGO_TARGET_DIR` override remains supported for
desktop checks/Clippy/tests. Choose it before warming the cache; do not change it between
runs. A leftover `CARGO_TARGET_DIR` from a different workspace is temporarily
replaced with the selected canonical path, then restored. The desktop debug CRT
override, Ninja Multi-Config and its four configurations are applied consistently.
Default features remain those of the selected manifest. Arbitrary Cargo arguments
are deliberately unsupported; test-runner arguments go only after `--`.

`-Jobs` defaults to four and controls concurrency without changing the compiled
artifact identity. Existing Rust flags, target and environment profile overrides
are rejected for review rather than silently creating another build variant.
Run explicit feature/target experiments separately and retain their exact commands.

Both workspaces default to `release-local`: no LTO, optimization level 1,
16 code-generation units and incremental compilation. The former `release-dev`
profile is retired. Existing caches are preserved; the new profile requires one
explicitly allowed initial build, then the normal single-preset invocation.
This change does not invalidate or delete the old profile's artifacts.

Executables are now `<repo>\target\release-local\screenpipe.exe` and
`<repo>\apps\screenpipe-app-tauri\src-tauri\target\release-local\screenpipe-app.exe`.
Stage the reviewed native DLLs and sidecars for these exact directories and refresh
runtime/configuration hashes. Existing firewall rules for `release` or `release-dev`
paths do not cover these paths. Complete the owner-run firewall preparation before
any trial; no launcher invocation changes firewall rules or starts recording.
Do not switch back to an old executable just because its rules already exist.

Use `-BuildProfile release` for production-representative timing, throughput,
overload and resource measurements and at the production-release milestones in
[AGENTS.md](../../../AGENTS.md). This is expensive and has its own cached artifacts.
This launcher does not stage sidecars, package, start capture, change firewall rules
or refresh runtime manifests; use the existing trial preparation for those actions.

## Distinguish missing variants from invalidation

Check-mode metadata, linked tests, build-time dependencies and optimized application
builds can require different artifacts even in the same cache. Completing one mode
does not warm every other mode. Each initially missing variant may compile once.
After that, the same command should reuse unchanged dependencies. A changed feature
graph, toolchain, profile, native environment or build script can legitimately
invalidate them. The separate root and desktop workspaces also have different
lockfiles and profiles; they are not interchangeable caches.

Batch edits and reuse successful checks. A test-name filter limits execution, not
compilation. Do not add redundant checks, change flags or clean caches in response
to a slow build. Finish planned edits before starting a linked test/application build.

For an unexpected repeat of an identical invocation, add diagnostics **to the next
already necessary run**, rather than launching an extra build:

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopCheck -Diagnostics
```

Every actual run saves its command plan, compiler versions, native input locations,
manifest/lockfile/toolchain hashes, Cargo output and exit status below ignored
`target\build-diagnostics`. Diagnostics additionally enable Cargo's fingerprint
invalidation messages and verbose compiler commands. Compare `identity.json` across
equivalent invocations, then use `cargo.log` to locate the first dirty dependency
and its reason. These records aid diagnosis; they are not a complete fingerprint of
every source, Cargo configuration, compiler environment or native artifact byte.
Keep them private: logs may contain local paths or test output. Never commit them.

## Automatic cache reporting and cold-cache gate

Every compiling preset consumes Cargo's structured artifact messages and prints
external dependency reuse/rebuild counts separately from workspace code. It records
`cache-summary.json`, `cache-inputs.json` and dirty-fingerprint reasons in `cargo.log`.
Rendered compiler diagnostics and ordinary test output remain visible. Repeated
artifact messages are deduplicated. Cached build-script output is not counted as
a rebuild; native compiler work inside build scripts is not measured separately.

The gate requires a successful equivalent launcher baseline and the recorded
dependency output files before invoking the compilation command. First-time cache
adoption, a new profile/target/configuration or missing artifacts requires explicit
permission in the invocation:

```powershell
# Review before adopting an existing cache or allowing an expected new variant.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild -PlanOnly
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild -AllowColdCache
# Subsequent matching runs use the normal single preset.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild
```

Existing caches from before this feature need the explicit flag once; it allows
their reuse as well as any required compilation, and never clears or moves them.
Do not routinely append the flag. Lightweight presets have no cold-cache gate.
Blocked runs write `cache-gate.json`; native shell setup and version/identity
inspection may already have run, but Cargo compilation has not started.

Successful, complete observations save baselines under the selected persistent
target's `.screenwise-cache-history`. Failed test commands, malformed/missing Cargo
observations and incomplete input discovery cannot establish a successful baseline.
The signature covers mode/profile/cache/target selection, features, compiler
versions, repository Cargo manifests/lockfiles, ancestor/user Cargo configuration,
native file size/mtime and selected compiler environment. Jobs, test execution
filters/arguments and diagnostics verbosity are excluded because they do not choose
a different dependency compilation graph. Artifact keys additionally track the
package, target, reported profile and enabled features.

This is a conservative preflight, not Cargo's full fingerprint. Artifact presence
cannot prove freshness: dependency source changes, native headers, environment
variables not represented in the signature or timestamp problems can still cause
rebuilds. A previously known external artifact rebuilding during a matching warm
run triggers one live warning and an exact count in the summary. Newly observed
variants and changed workspace code are reported separately. The warning is issued
when Cargo reports the completed artifact; it does not cancel the run.

Use the evidence to identify the first dirty dependency, fix avoidable configuration
drift, add a regression check and record the remedy. Confirm reuse during the next
necessary equivalent run, rather than launching an extra build for diagnosis.

## Opt-in overlap of root and desktop compilation

Compiling invocations remain serial by default. Add `-Concurrent` to **both**
invocations to allow one root and one desktop operation to overlap using their
existing distinct caches. Inspect both plans first. Start only builds already
needed for settled source; overlap does not justify warming a second variant.

```powershell
# Inspect first, then omit -PlanOnly in two separate PowerShell processes.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootBuild -BuildProfile release -Concurrent -Jobs 2 -PlanOnly
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild -BuildProfile release -Concurrent -Jobs 2 -PlanOnly
```

Use overlap when available memory, disk and CPU make it worthwhile, such as a
long root optimisation/link stage leaving CPU capacity for desktop compilation.
Keep the **sum** of job limits within the reviewed machine budget (the example
retains the usual total of four). Cargo jobs bound Cargo scheduling, not linker
threads, native subprocesses or memory. Reduce the second build's `-Jobs` further
when appropriate; do not start overlap under memory/disk pressure. There is no
automatic resource detector or late linker-stage scheduler. This flag changes
neither features/profile nor cache signature; native-environment checks,
cold-cache gates, artifact reporting and output locations still apply.

The plan exposes `build_locks`; compiling attempts save `build-locks.json` in
their unique evidence directory. The protocol uses held OS file handles:

- The canonical `target\build-diagnostics\launcher.lock` is exclusive for serial
  runs and shared for opted-in runs. Serial runs refuse overlap, including with
  the older launcher using that canonical lock. Conflicts fail promptly; they
  do not queue or terminate processes.
- Each selected target has an exclusive `.screenwise-launcher.lock`, across
  modes, profiles and evidence locations. Same-cache attempts are refused even
  when evidence roots differ. Paths containing junctions/symlinks are refused.
- Every desktop compiling operation also holds the checkout's exclusive
  `.local\build\locks\desktop-staging.lock`. Different desktop caches cannot
  overlap: `src-tauri/build.rs` writes `windows-runtime`, command generation and
  Tauri generated files outside the selected Cargo target.

Handles are released on failure and success; leftover lock files are normal
and must not be removed to bypass exclusion. Root Windows staging occurs in
`crates/screenpipe-audio/build.rs`: the verified ORT DLL goes into the selected
target/profile derived from `OUT_DIR`. Engine `build.rs` does not copy recorder
sidecars into desktop source. Provisioned ORT/OpenBLAS inputs are read-only
consumers here. Native dependency build outputs remain under Cargo targets;
Cargo's own locks protect tool/registry caches. Do not provision or mutate shared
native/tool/dependency stores during overlap.

The launcher calls Cargo directly. It does not invoke Tauri CLI
`beforeBuildCommand` or frontend `prebuild`, which can write shared frontend,
sidecar and asset outputs and perform acquisition. Frontend export, binding
generation, sidecar preparation, packaging, provisioning and direct Cargo commands
do **not** participate in these locks; finish their inputs before building and
do not run them during overlap. The older launcher's custom evidence-root lock
is also outside this protocol. Adopt the updated launcher for all participants.
Do not change source/scripts during a build or run against a production cache
solely to validate overlap.

## Launcher regression checks

```powershell
.\scripts\windows\build\Test-ScreenWiseBuildLauncher.ps1
.\scripts\windows\build\Test-BuildLocks.ps1
.\scripts\windows\build\Test-CargoCache.ps1
.\scripts\windows\build\Test-TrialBuildProfiles.ps1
```

These checks use synthetic native input files and stub Cargo/rustc executables.
The trial-profile checks also execute early rejection and canonical profile/path
gates in isolation, before any process, recording, network or firewall action.
They verify cache selection, focused target arguments, Clippy warning flags,
lightweight operation without native setup, JSON output, rejected overrides,
exit propagation, and environment/location restoration without running a compiler
or changing genuine Cargo artifacts. The launcher suite uses stub caches and
separate evidence in its checkout, exercising its canonical scheduling lock.
Run it in an isolated worktree while real builds are active elsewhere. The lock
suite uses a synthetic repository/cache layout and a hidden PowerShell process.
The suites cover overlapping launcher processes, same-cache rejection across
evidence roots, desktop staging exclusion, serial exclusion, reparse-path refusal
and lock release after partial acquisition or Cargo failure.
Small generated fixtures and diagnostic logs
remain under ignored `target`. They do not replace a real build of the application.
