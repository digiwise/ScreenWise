# ScreenWise interactive privacy fixtures

These locally authored assets are for a later, explicitly orchestrated interactive validation. Building and `--self-test` do not show windows or reach clipboard/input APIs. The fixtures do not access the recorder, network, audio, media, or captured data. Every marker is synthetic.

## Build and non-UI validation

From PowerShell:

```powershell
.\build.ps1 -SelfTest
```

This uses the installed 64-bit .NET Framework compiler offline. The compiled windows-subsystem executables may not display self-test stdout, so the build script relies on their exit codes.

## Native privacy fixture contract

Launch only during an authorized interactive run:

```powershell
.\PrivacyFixture.exe --run-id RUN_ID --out-dir ABSOLUTE_OUTPUT_DIRECTORY
```

`RUN_ID` and every `phaseId` must match `[A-Za-z0-9._-]{1,80}`. The output directory must not already exist. On showing the compact, non-topmost window, the fixture atomically creates `ready.json`. It creates `commands` and `acks` beneath the output directory. It refuses to overwrite any ready or acknowledgement file.

Submit each action once as a new `commands\NAME.json` file. Write it under a temporary extension and rename it to `.json` when complete:

```json
{"runId":"validation-20260915-a","phaseId":"plain-001","action":"plain"}
```

The command `runId` must exactly match the launch value, which rejects stale commands from another run. The fixture writes `acks\plain-001.json` with `runId`, `phaseId`, `action`, UTC timestamp, `success`, `verifiedForeground`, numeric `foregroundHwnd`, `visible`, `excludedVisible`, and an allowlisted `errorCode` when needed. `foregroundHwnd` is the actual `GetForegroundWindow` handle only when `verifiedForeground` is true and is `0` otherwise. It lets the coordinator distinguish the allowed main window from the excluded child, which share a process ID. The acknowledgement never includes window text, field values, paths, or captured context. Each command path is handled once and each phase ID must be unique within the output directory. A duplicate phase is rejected before its action runs and its immutable original acknowledgement remains unchanged.

Supported actions:

- `idle`: show the main window and focus the ordinary field.
- `plain`: restore and focus the ordinary fake value `public cedar garden`.
- `password`: restore and focus the masked fake value `hidden tulip waterfall`.
- `plain-clipboard-target`: clear and focus the ordinary target without touching the clipboard.
- `password-clipboard-target`: clear and focus the password target without touching the clipboard.
- `plain-copy`: reset the source to the known synthetic marker, verify plain-field focus, preserve the prior clipboard in memory, and issue real Ctrl+C through `SendKeys`.
- `password-copy`: reset the source to the known synthetic marker, verify password-field focus, and issue real Ctrl+C. Windows normally rejects password copying; this is acknowledged as `clipboard_copy_not_observed` and does not establish fixture ownership.
- `plain-paste`: verify plain-field focus and issue real Ctrl+V only when the clipboard sequence still matches a successful fixture-owned synthetic copy. It never reads the clipboard contents.
- `password-paste`: verify password-field focus and issue real Ctrl+V only when the clipboard sequence still matches a successful fixture-owned synthetic copy. It never reads the clipboard contents.
- `restore-clipboard-after-recorder-stop`: requires `"recorderState":"stopped"`, then restores the in-memory pre-copy clipboard only when the sequence still matches the fixture's last successful copy. If another application changed it, the newer value is preserved and the stale in-memory backup is discarded.
- `excluded-foreground`: show and focus `::SW EXCLUDED Synthetic Fixture`, containing `forbidden violet orchard`.
- `excluded-background`: keep the excluded window open but focus the allowed main field.
- `browser-handoff`: minimize the native fixture for an explicitly controlled browser phase.
- `hide`: hide the fixture and its excluded child without affecting any unrelated application.
- `release-focus`: identical to `hide`; use it before an operator question.
- `show`: show the fixture and focus its ordinary field.
- `close`: close the fixture.

`close`, Escape, and normal window close never restore the clipboard. While a restorable snapshot is pending, all close paths are blocked with `clipboard_restore_required` or a fixed local status. The coordinator must stop the recorder, send `restore-clipboard-after-recorder-stop` with `"recorderState":"stopped"`, and check the acknowledgement before closing. `clipboard_changed_external_preserved` is a terminal safe result: the later external value remains untouched and the stale in-memory backup is discarded, so close may proceed. The coordinator must send `release-focus`, verify `visible:false` and `excludedVisible:false`, and only then ask the owner a readiness question. Clipboard contents are never serialized, logged, compared, or included in acknowledgements.

Owner readiness is an indefinite operator gate: no elapsed time may imply that the owner is ready. Fixture acknowledgements must use a bounded coordinator timeout; a missing acknowledgement requires safe recorder stop and an incomplete result, never continued recording or inferred owner readiness. Optional audio cues are future coordinator behavior and may occur only after explicit owner readiness; these fixtures have no audio behavior.

`verifiedForeground` reports the immediate result of `GetForegroundWindow` plus target focus where applicable. It is an acknowledgement, not proof of sustained focus; the coordinator should record its own bounded observations.

## Synthetic DRM identity contract

The output filename `Netflix.exe` exists solely to exercise application-name detection. It contains no protected content, video, audio, browser, bypass logic, or network code. Launch it explicitly:

```powershell
.\Netflix.exe --run-id RUN_ID --phase-id PHASE_ID --out-dir ABSOLUTE_OUTPUT_DIRECTORY
```

It remains open until Escape, normal window close, or an explicit command. It has no duration, readiness timeout, or run-number behavior. On showing, it creates `drm-ready-PHASE_ID.json` with the IDs, process ID, UTC timestamp, immediate `verifiedForeground`, numeric `foregroundHwnd`, `visible`, and its command directory names. `foregroundHwnd` is `0` unless foreground verification succeeded. An existing ready file for that phase is rejected and never overwritten.

DRM commands use the same JSON shape under `drm-commands`, with actions `hide`, `release-focus`, `show`, or `close`. Acknowledgements appear under `drm-acks` and include `verifiedForeground`, numeric `foregroundHwnd`, and `visible`, using the same zero-on-unverified rule. Duplicate phase IDs are rejected before action. Before asking an owner-readiness question, the coordinator must send `release-focus` and use a bounded wait for a successful acknowledgement with `visible:false`; failure requires safe recorder stop and an incomplete result. These commands affect only this fixture's own window.

## Browser fixture contract

Serve `browser-fixture.html` only from a loopback-only server chosen by the coordinator. Example routes may use `http://127.0.0.1:PORT/browser-fixture.html` as allowed and `http://localhost:PORT/browser-fixture.html` as excluded. Query fields `runId`, `phaseId`, and optional `case=excluded` are rendered for operator confirmation. Host `localhost` or `case=excluded` shows `forbidden cyan orchard`; other hosts show `public bronze hill`. The ordinary marker is `public browser cedar`; the masked password marker is `hidden coral orchard`.

The page is self-contained and its CSP blocks connections, forms, and media. It exposes `document.documentElement.dataset.fixtureReadyUtc` and `dataset.fixtureCase` for a browser controller. A `file:` load is treated as the allowed case for static inspection.

## Provenance

The interface was redesigned after reading only the approved legacy source fixtures in `target\firewall-validation-20260914-01a09e45`. No captured outputs, logs, results, screenshots, upstream Screenpipe source, current commercial source, or Litepipe code were used. Existing legacy files remain unchanged.
