# One-off cleanup of remaining pre-fix capture events

Prepared at the owner's request; this document does not deploy, restart, run
recurring pruning or supply a deployment time. The running old recorder can
continue creating flood rows after the completed **2026-10-09T05:15:00Z** cutoff.

## Completed one-off run (9 October 2026)

The owner confirmed deployment. Metadata verification found the running desktop
hash matched the fixed candidate and process start was
`2026-10-09T06:09:27.871258Z` (17:09:27 AEDT). This is the conservative cleanup
cutoff; exact first capture time was not inferred. The half-open interval from
05:15 UTC to that cutoff was previewed and applied with a fresh private archive.

Removed 3,268 original flood rows: microphone 3,148, keyboard 60, clipboard 60.
Preserved 426 target transitions and 651 other notices byte-for-byte unchanged.
The flushed removed-row archive SHA256 is
`e268cd28a4ffc49831a1747a8196f2c862f4b775aa29d2869c0559c9836eaf92`;
survivor SHA256 is
`c2d8f842fc7147d5e356ae0ec388048dd447017e1dffcca073980ef8a46131ab`.
Plan, archive, deployment evidence and receipt remain private under
`.local/maintenance/capture-events-20261009-post-deploy-01/`.
All events at or after the cutoff were excluded from this maintenance. The
one-off cleanup is finished; the procedure below is retained for provenance
and does not authorise another run or recurring pruning.

## Procedure used

After Clancy deploys the exact candidate in [the hand-off](CAPTURE_FIX_HANDOFF.md),
record the confirmed UTC boundary between the old recorder and the fixed
recorder. Use observed process/recorder startup and shutdown evidence from that
deployment. A candidate preparation timestamp, binary modification timestamp or
estimated planned time is not this boundary. If uncertain, end at the last
confirmed old-recorder time and leave the uncertain interval untouched.

Run the existing maintenance utility once for remaining **pre-fix** intervals,
starting at 05:15 UTC and ending at that confirmed boundary. Its ranges are
half-open: the end timestamp and all later events are preserved. Do not include
the separate client's post-deployment verification window. If more than four
hours elapsed, split into adjacent ranges no longer than four hours; each needs
a fresh private archive and preview. Do not overlap the already cleaned ranges.

From the repository root, with provisioned Python and the confirmed recording
directory, use this command shape. Replace the placeholders with actual values;
the example deliberately supplies no assumed restart time:

```powershell
python scripts/windows/maintenance/prune_capture_event_flaps.py `
  --db '<confirmed recording directory>/db.sqlite' `
  --archive '<fresh private .local/maintenance/post-deploy range directory>' `
  --start '<2026-10-09T05:15:00Z, or next range start>' `
  --end '<confirmed pre-fix boundary, or earlier four-hour range end>'

# Inspect the fixed counts and plan before applying this exact range.
python scripts/windows/maintenance/prune_capture_event_flaps.py `
  --db '<same database>' --archive '<same archive directory>' `
  --start '<same start>' --end '<same end>' --apply
```

Use the unchanged strict allowlist: microphone device 1 silent/open and
keyboard/clipboard transient-check suppression/open, with no rule/app payload.
Archive removed original notices before exact-ID deletion; recheck the input
fingerprint in the transaction and verify all survivors unchanged. Retain the
preview, removed-row archive and receipt. Preserve stops, no-callbacks,
degradations, redactions, unlocks, unknown shapes and other meaningful or
uncertain patterns. Do not extend this to output audio's small unproven dataset,
invent replacement observations or compact/vacuum captured data.

Record the actual boundary and its evidence, ranges, removed/retained counts,
archive hashes and unchanged-survivor verification in the hand-off/checklist.
Then stop. This is a one-off historical cleanup; newly observed post-fix flaps
are evidence for investigation and the separate client, not rows to erase.
