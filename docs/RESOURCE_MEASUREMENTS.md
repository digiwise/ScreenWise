# CPU and memory measurements

Use the final `release` desktop executable for performance benchmarks. Record
its SHA256, source revision plus any uncommitted changes, workload, enabled
modalities, monitor count/resolution, model, and sampling interval. Keep build
and model-loading warm-up separate from steady-state samples. `release-local`
results are useful for iteration but are not release benchmarks. The current
release profile uses size optimization (`opt-level = "s"`), LTO and one codegen
unit; this measurement preparation does not change that profile.

The prepared desktop release executable, SHA256 and successful build evidence
are recorded in [the build notes](../BUILD_NOTES.md). It includes uncommitted
source changes on `7187795ad7554f403a2fa12b65036aa1f99ecaee`; the private candidate
manifest records their hashes. It has not been deployed or benchmarked.

The external resource sampler is opt-in and does not start or modify ScreenWise,
recording, authentication, models or firewall rules. Compile and self-test it:

```powershell
.\scripts\windows\day-to-day-trial\Build-ScreenWiseGuiProcessMonitor.ps1
```

For a separately approved, attended recording trial, use the exact GUI PID from
the fresh launch manifest and a new private output directory. Example (replace
the PID with the verified GUI PID):

```powershell
$resourceOutput = Join-Path (Get-Location) '.local\resource-trial-unique-name'
New-Item -ItemType Directory -Path $resourceOutput -ErrorAction Stop | Out-Null
& .\.local\gui-trial-runtime\tools\GuiProcessTreeMonitor.exe --resources `
  --root-pid 12345 --out-dir $resourceOutput --duration-ms 3600000 --interval-ms 5000
if ($LASTEXITCODE -ne 0) { throw 'Resource sampling failed; evidence is incomplete' }
```

Recording retains the fresh readiness gate. The sampler itself requires no
screen, UIA, clipboard, audio, command-line, executable-path or database reads.
Toolhelp inventories process IDs, parent IDs and basenames in memory; the
resource output contains only timestamps, process IDs/start times and numeric
counters. It never emits names, titles, paths, tokens or captured content.
It writes only `process-resources.jsonl` with create-new protection. Keep these
files private. The existing 25 ms process-start audit is unchanged; resource
mode defaults to five seconds, accepts 1–60 seconds, and requires a bounded
duration of 1 second–24 hours. It retains only the prior sample, caps counter
readings at 4,096 processes and reports truncation.

## Reading the output

- `cpu_cores` is processor-time delta divided by monotonic elapsed time:
  1.0 means one logical processor fully occupied. Tree
  `cpu_machine_percent_sampled` divides the sum by `logical_processors` and
  multiplies by 100. The denominator is the sampler's .NET logical processor
  count; unusual processor-group/affinity configurations need separate care.
- CPU is null on a process's first sample, PID reuse or a counter reset. Compare
  `cpu_sample_count` with `process_count`; totals include only measurable deltas.
  New, exited, inaccessible and short-lived workers can make CPU totals partial.
- `working_set_bytes` measures resident pages, including shared pages.
  `private_bytes` measures private committed memory, including nonresident
  memory. Neither is a heap allocation/leak counter. Tree working-set sums can
  count shared pages more than once; do not call them unique physical RAM use.
- `root_present`, `unavailable_count` and `truncated` expose incomplete samples.
  PID plus creation time prevents a reused PID being counted as the old process.
  Previously observed surviving children remain tracked after parent exit.
  A missed/inaccessible ancestry link can leave descendants unobserved. A
  process that starts and exits between snapshots is missed. External model
  servers and target applications are outside the GUI descendant tree.
- `sampling_duration_ms` reports collection cost, not the sampler's total CPU
  overhead. Sampling is sequential, not an atomic process-tree snapshot. This
  harness has synthetic validation only; live overhead remains unmeasured.

Compare matched idle, ordinary use and sustained workloads; retain raw samples
for medians/percentiles, peaks and memory trend. Investigate a rising private
commit trend before calling it a leak. Correlate CPU spikes with the existing
numeric UIA/screening metrics and actual capture success so missing work cannot
appear to be an optimization.

## Screen scope

Desktop settings now offer screen recording on **all connected monitors** or
off. Enabling it also includes displays connected later, under the existing
capture/privacy gates. Monitor aliases remain available for timeline labels.
Old selective-monitor preferences become screen-off until the user enables
all-monitor recording; audio and input settings are preserved. Hardware tier
defaults no longer select a primary display. Lower-level explicit monitor
filters still fail closed on empty/unavailable selection and cannot fall back
to all displays after capture-start failure. Hardware hot-plug and installed
settings UI behaviour still require live validation.

## UIA worker cleanup

The input privacy and general UIA workers now own successful STA initialization
with a thread-bound guard declared before their COM interfaces. Interfaces are
released before `CoUninitialize` on normal exit, failed initialization paths,
early returns and panic unwinding. Nested successful initialization is balanced;
a failed attempt to join a different apartment does not uninitialize it.
Tests cover these cases without creating UIA contexts or contacting application
providers. The accessibility suite passed 289 tests; 23 live tests stayed ignored.

This repairs a concrete teardown-order defect. It does not establish the cause
of SW-R20, bound provider work, cancel hung COM calls or prove crash recurrence
has stopped. SW-R19 and SW-R20 remain open for their existing follow-up.

## Simple scheduling

The desktop schedule is now on/off time windows for all otherwise-enabled
sources. It never enables audio, vision, keyboard or clipboard sources that
their own settings disable. Keyboard and clipboard history remain available,
with existing saved preferences preserved.

The stored `recordMode` field remains for compatibility; new windows use `all`.
An active legacy source-specific or unknown-mode window blocks every source,
even when overlapping an all-source window. Its settings warning and explicit
**Use all enabled sources** action let the owner convert it deliberately.
Changing its time or merely enabling scheduling does not convert it. Replacing
rules with a preset is also an explicit choice. Invalid windows block their day.

The new **Always On** preset ends at `24:00`, fixing the old preset's missing
23:59 minute (SW-R14). The editor displays this as end-of-day midnight. Existing
saved `23:59` ends retain their original hours until edited or replaced. Other
windows remain end-exclusive and overnight ranges are unsupported.
The initial gate is set before capture startup, and the existing global pause
invalidates queued audio. Transitions are polled every 30 seconds; this is not
an exact wall-clock boundary guarantee. Installed UI and live transitions remain
unvalidated. No modality-specific audio gate or permit-layout change is needed.

## Input URL admission follow-up

SW-R01 remains open: frame URL exclusions do not protect stored keyboard or
clipboard content. A proper input fix must carry applicable URL policy into
fresh input admission; event text filtering or a previously captured frame's
URL is insufficient. Keep raw input storage disabled when relying on URL
exclusions until this path is repaired.

The next synthetic matrix should include an allowed-page positive control,
excluded-page typing and clipboard denial, missing/expired URL evidence, focus
and browser-tab changes within one window, and delayed batch admission. Match
policy against original metadata before sanitizing persisted records. Preserve
existing app/password exclusions and independent trigger/storage preferences.
Freshness work must run outside low-level input hooks, respect existing provider
backoff and never search other apps or the desktop root. Measure provider work
and input latency alongside correctness. Native browser/provider behaviour
still needs an explicitly approved live fixture run.

## Prepared live checks

Before interpreting performance results, verify screen-off, all-monitor startup,
disconnect/reconnect and stop/restart using synthetic fixtures. Confirm capture
success and privacy-blocker counts alongside CPU/RAM, target-app responsiveness,
and clean shutdown. A low CPU reading with missing captures is not a pass.

The non-interactive source review also rechecked the open-issue register.
URL exclusions for keyboard/clipboard rows (SW-R01), audio merge cancellation
(SW-R07), streaming chat deletion races
(SW-R11), target-side bulk UIA work (SW-R19), and crash recurrence (SW-R20)
remain separate work. This preparation does not close them. The browser lookup,
walk-lease, cache-deletion and audio-start repairs already in the candidate still
need their documented runtime checks. Use existing synthetic fixtures and the
interactive harness; do not start a recording session to fill these gaps without
fresh readiness.
