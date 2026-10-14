# Standalone Access RuntimeId probe

Production code and recording settings are unchanged by this standalone probe.
Default identity mode requests only RuntimeId, control type and IsControlElement;
it does not request names, values, URLs, password data, screenshots or text.
Comparison mode additionally acquires Name, Value and IsPassword in transient
UIA caches, inspecting their types without converting or printing values.
RuntimeId arrays are compared in memory and never printed. No focus change,
input, listeners or event subscriptions.

Build with the existing Windows 0.58 dependency artifact (no downloads):

```powershell
.\scripts\windows\uia-runtimeid-probe\Build.ps1 -WindowsRlib <existing-windows-0.58-rlib> -WindowsNativeLib <existing-windows.0.52.0.lib>
```

Direct `rustc` is used because the canonical Cargo launcher does not support
this isolated source target. Its configured Developer PowerShell and already
compiled dependencies are reused. Output is ignored `.local/uia-runtimeid-probe`.

Open Access to the affected view and run from your own PowerShell:

```powershell
.\scripts\windows\uia-runtimeid-probe\Run.ps1 -ExecuteInteractive
```

The command authorizes the read-only live check. Keep the Access view steady
until it completes (three passes, at most 15 seconds per child). Use `-ProcessId`
if several Access instances have windows. Without `-ExecuteInteractive`, the
script only previews. No live desktop run is part of the build/self-test.

Each pass uses fresh COM/UIA setup and a 10-second cooperative deadline, with
500-ms provider timeouts. The parent terminates only its own child at 15 seconds
if a provider hangs. This diagnostic budget is longer than production; it does
not demonstrate suitability within the production admission deadline.

Numeric JSONL phases: 0 = control-view hierarchy cache; 1 = Document/Edit/ComboBox
bulk query with control-view cache filter (production query shape);
2 = same query with raw-view cache filter; 3 = cross-acquisition ID comparisons.
Each phase reports cached VARIANT types, control-type/type counts, cached/direct
agreement, null/error/duplicate IDs, repeated direct-ID agreement for missing
cached IDs, direct-call total/max time and phase time. VARIANT 8195 is the expected
integer array; 0 is empty and 13 is an interface value. An interface value alone
does not prove the UIA NotSupported sentinel. Failed/incomplete phases are marked
`complete:0`; subsequent phases are skipped. Saved results are numeric only.

Stable, unique direct IDs resolving missing cached IDs support a bounded fallback
candidate. Different IDs, missing direct IDs, or high direct-call latency do not.
Cross-acquisition mismatches may also reflect UI changes; repeat with a steady
window before interpreting them as a provider or query-filter fault. This probe
does not evaluate privacy rules or grant capture permission. Production admission
and revision/focus checks still need separate validation before any fix.

## Single-hierarchy acquisition comparison

```powershell
.\scripts\windows\uia-runtimeid-probe\Run.ps1 -ExecuteInteractive -Compare
```

Three passes alternate acquisition order. Each mode creates fresh automation and
root objects and retains no previous tree or result. Phase 4, mode 0 uses the
former acquisition shape: structural hierarchy, scoped Document/Edit/ComboBox
values, scoped Window/Document names, then cache decoding/join diagnostics.
Mode 1 acquires structural properties, names and values together in one subtree
cache and reads applicable properties directly from each cached node; no ID join
or direct RuntimeId calls are needed. Both use Full element mode/control view.

Reports separate setup, acquisition, decoding and total microseconds, provider
acquisition counts, property VARIANT counts grouped by numeric control/property
IDs, missing evidence and 375/750-ms timing flags including setup. Mode 0 also
reports time to its first identity rejection: production would stop there,
whereas this diagnostic continues to quantify the complete acquisition shape.
Mode 0 adds cached ControlType to scoped results for numeric classification.

Mode 1 intentionally acquires Name/Value across all types, which is broader than
the former scoped acquisition. Values stay in transient UIA caches and are not
converted to text or saved. Both modes inspect only availability/type, not rule
matches, complete projection consistency, extension-specific scope, event
revision or final focus checks. No allowed/blocked decision is produced; a
complete cache with unsupported required properties is still unresolved.
Therefore this compares acquisition cost, not end-to-end privacy admission.
The parent watchdog is 30 seconds per comparison child; each mode has a 10-second
cooperative acquisition deadline. Keep Access steady until the three passes end.
