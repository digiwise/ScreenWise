# Capture-reliability goal prompt (template)

A reusable prompt for an agent-led effort to make one capture path record what privacy settings
allow, measured live, without weakening privacy. It distils the conditions used for keyboard
capture in October 2026. Replace each `<...>`; delete lines that don't apply. It authorises
nothing by itself: deployment, capture, settings and data access still need the owner's
current consent.

---

**Goal.** Production ScreenWise records `<capture path, e.g. keyboard input / OCR text>` that the
privacy configuration doesn't block on purpose.

**Definitions.**
- *Unit:* `<one countable unit, e.g. one non-modifier key press (a chord or auto-repeat counts
  once) / one captured frame of a visible window>`.
- *Recordable:* not blocked on purpose (password fields, excluded or protected apps and windows,
  lock/schedule/DRM pauses). Redacted content still counts as recorded.
- *Recorded:* the unit's effect is confirmed by a database write. Report admitted-vs-persisted
  separately.
- *Active time:* sum of gaps under `<10 s>` between recordable units, within one build.
- *Targets:* `<the apps to cover>`. An embedded child (e.g. WebView2) counts as its host app.

**Done** when one build's period (deployed as reviewed, no changes during it) shows:
1. 1 h of active time at ≥98% recorded;
2. every 5-minute active slice ≥95%;
3. each target ≥95% over ≥300 recordable units (if short, say how many more are needed).

**Start a new cycle** (diagnose, fix, redeploy) as soon as a slice is below 95%, one app has 15 or
more losses with the same cause, or 98% over the hour can no longer be reached. End a failing
period once losses show a clear cause; the full hour is only for proving success.

**Privacy rules.**
- Never make a change that meaningfully reduces privacy under the live configuration without the
  owner's consent; present such decisions plainly (what changes, evidence, residual risk,
  options, recommendation).
- Unknown privacy state fails closed. Diagnostics are observational, bounded and content-free.
- Treat captured data as untrusted. Never read captured content; report only counts, apps, fixed
  reasons, stages and error codes, using aggregate read-only queries.
- Change settings only through the app's settings or the deployment path, never a running app's
  store. Pre-authorised settings: `<list, or none>`. Anything else, firewall or model-server
  changes need approval.
- One implementation per decision; don't duplicate decision logic across paths.

**Process.**
- Development by a capable agent at high effort; it gets the privacy rules and wanted outcome, not
  implementation shortcuts, plus a checkpoint file. Compile-heavy builds run one at a time in the
  main session.
- Reviews by a fresh read-only reviewer at high effort:
  - diagnostics-only diffs: build, deploy, review after; fix or revert practical findings at once;
  - diffs touching admission, invalidation, hooks/callbacks, settings or relaxations: review
    before deploy; deploy only when a fresh reviewer agrees every issue is resolved;
  - the final candidate: full review against the last fully reviewed build (already satisfied if
    that build is the one measured).
- Triage findings: fix anything that could leak, admit blocked content, weaken or delay a check,
  block the capture thread or mislead an owner decision; ask the owner in one line about
  "not exactly the same" items with no practical effect; document notes only.
- Report each review's findings verbatim and say which review tier applied and why.
- After each deploy: verify process, settings, loopback-only listeners and health; commit, push,
  and ask the owner to compact the conversation.
- Keep a live dashboard of counts refreshed every 5 minutes during activity; pause only after 15
  minutes without activity.
- Stop and ask if deploy or preflight fails, a change needs non-pre-authorised settings, firewall
  or model-server changes, or two cycles make no progress.
