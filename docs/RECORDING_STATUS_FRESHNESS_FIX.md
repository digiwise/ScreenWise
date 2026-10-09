# Window-policy status freshness repair (2026-10-09)

An available status response could attach a stale producer's age to an earlier
redaction summary from a different observer. In particular, content-protection
notices report transitions rather than every poll. Their old positive state was
treated as stale for visual channels even when current aggregate privacy
admission checked the same protection flag. An active-window-only outcome also
fell through the summary classifier without its own explanation.

The frontend now recognizes positive protection transitions as covered only by
fresh admission for channels whose aggregate gate checks that flag. Negative
protection observations remain independent; fresh capture cannot clear them.
When the API is available, retained blocker labels must belong to the same
observation key as the stale check. During an API outage the previous summary
and its stale warning remain. Stale details identify the observer and explain
that check age is not proof of the duration of an ongoing problem.

Active-window-only capture now has an explicit summary and reason. It reports
recent capture only with independent fresh capture evidence, otherwise capture
remains unconfirmed; permitted fallback alone is not called a complete pause.
Window-policy details distinguish missing metadata from matching configured rules. They show
safe list/index/kind references when present, explain when no rule was identified,
and describe the conservative overlapping-window policy. Unknown metadata can
prevent full-display capture even when a maximized window covers the affected
background window. This repair does not change capture admission, exclusion
settings, occlusion handling or the requirement to withhold uncertain pixels.
The authenticated drill-down also displays the separately supplied window-policy
blocker executable basenames. It does not infer identities from old focus or map
aggregate applications to particular rules. Paths, URLs, title text, invalid
names and oversized lists are omitted; generic labels and logs remain content-free.

## Validation

82 focused frontend regressions passed across the status parser, dashboard,
alerts and expanded dialog. New cases cover cross-observer stale-age attribution,
same-observer retention, positive versus negative protection state, and missing
metadata versus configured exclusions. A further 21 existing polling, floating
dashboard and status-log tests passed. The settled fallback classification passed
its 38 dashboard/dialog checks and TypeScript no-emit validation. Rust sources
and both lockfiles are unchanged by this repair; unrelated native/deployment work
belongs to a separate task.

Diagnosis read only authenticated current status fields and monitor metadata,
plus exclusion configuration shape. It did not inspect recorded screen, input
or audio contents. No new capture session, focus interaction, firewall change,
deployment or restart was performed. The frontend exported all 16 pages
successfully. The final candidate also includes the separately tested diagnostic
flap and optional-background-accessibility reporting repairs described in
[the master checklist](CAPTURE_FIX_MASTER_CHECKLIST.md). The separately authorised
[maximised-window implementation](VISIBLE_WINDOW_CAPTURE_PLAN.md) changes capture
admission and adds an explicit Maximised window only label. Three further parser,
dashboard and dialog regressions passed for this label without invented blockers
or successful-capture claims. The Screen recording settings description now makes
its accessibility and OCR scope explicit. Final release-local RootBuild and
DesktopBuild passed; the candidate and interactive-test status are recorded in
[the hand-off](CAPTURE_FIX_HANDOFF.md).
