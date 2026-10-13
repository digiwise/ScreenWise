# Fresh bulk UIA restart checkpoint

Historical pause checkpoint, superseded by the resumed
[candidate handoff](FRESH_BULK_UIA_HANDOFF.md). Counts and hashes below describe
the paused state, not the delivered artifacts or post-Windows-restart baseline.

Stopped at the owner's request before production compilation or deployment.
All changes are saved in the working tree, **uncommitted and unvalidated as an
application candidate**. Preserve the earlier foreground-attribution and compact
notice edits; they predate this implementation. No capture session, recording
restart, firewall change, or fixture GUI launch was performed.

## Saved work

- `crates/screenpipe-a11y/src/tree/windows_bulk.rs` and Windows tree integration:
  fresh per-attempt bulk caches, scoped rule queries, event revision invalidation,
  original-deadline retries, explicit handler cleanup before publication, required
  property/identity checks, and numeric metrics. MTA initialization replaces STA;
  no tree/admission result survives an attempt. Added tests are **not yet run**.
- Engine DRM evidence request and authenticated `/vision/uia-metrics` endpoint;
  paired-capture diagnostic no longer prints the app name.
- Standalone manual fixture and guide: [FRESH_BULK_UIA_FIXTURE.md](FRESH_BULK_UIA_FIXTURE.md).
  Executable `target/uia-privacy-fixture/FreshBulkPrivacyFixture.exe` built and inert
  self-tests passed. SHA256:
  `288C675C33504A4932FD6DB344B25276F0018E17CCD5BA7350F4EC26295ED1F2`.
  4 KiB exercises freshness; 64/320 KiB exercise the 16 KiB property limit, not
  aggregate-limit success. Live provider compatibility remains untested.
- Numeric baseline/comparison collector and six synthetic tests under
  `scripts/windows/day-to-day-trial/numeric_uia_trial.py`; rollback helper and four
  isolated PlanOnly tests under `scripts/windows/deployment/`.

Private baseline evidence and the previous deployment manifest are saved under
`.local/uia-bulk-validation/20261013-fresh-bulk/`. Baseline start: preceding
20 minutes had 1,333 frames, 1,072 failed placeholders, 1,057 UIA timeouts.
The subsequent 120-second sample series completed. Old deployment admission
latency is unavailable, **not zero**. Recovery is a same-device aggregate proxy,
not proof of readable capture or correct admission.

## Remaining before delivery

1. Compile Windows bindings and finish independent review of cleanup/partial
   subscriptions, MTA lifetime, extension keyed consistency, precise provider
   stages, final deadline/revision checks, and acquisition exception diagnostics.
   Correct telemetry accounting of subscription/removal calls and cleanup errors.
2. Run canonical RootFmt, focused accessibility/engine/privacy tests and needed
   desktop checks. Then sequential single-job RootBuild and DesktopBuild using
   `release-local`, established caches, `--locked --offline`; no dependency changes.
   Resolve compiler/test failures before claiming readiness. No Cargo build is
   currently active. Check both lockfiles and preserve unrelated source edits.
3. Record final artifact hashes, acquisition exceptions and validation gaps;
   deliver the fixture and stop for the owner's deployment confirmation.
4. Only after confirmation, deploy and run the requested 20-minute numeric-only
   comparison. Advise rollback on incorrect admission/material regression.

The key unmeasured risk is per-attempt event registration/removal consuming much
of the original 250 ms deadline. Do not infer production performance from the
55 ms one-rule probe. Broader acquisition is needed for hierarchy structure and
existing accessibility recording; scoped rule queries must preserve coverage.
URL decoration still uses the first extracted HTTP(S) Document URL; additional
Document URL coverage and asynchronous event completeness remain gaps.

For a later owner-authorized rollback, select the exact `release-previous-*`
archive printed by deployment and run `Restore-ScreenWisePrevious.ps1` with
`-ManifestBackup .local/uia-bulk-validation/20261013-fresh-bulk/previous-deployment.json`
and `-PreviousRuntime <that-archive>`; inspect `-PlanOnly` first. It validates
baseline GUI/CLI hashes, requests graceful shutdown, restores application files,
preserves data/firewall/native dependencies, and does not automatically restart.
Then run `Start-ScreenWise.ps1 -PreflightOnly` before an authorized restart.
