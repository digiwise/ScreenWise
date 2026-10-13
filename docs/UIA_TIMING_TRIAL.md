# UIA timing diagnostic trial (2026-10-14)

This temporary diagnostic change measures the existing fresh UIA attempt. It
does not change rule evaluation, capture admission, deadlines, or retry policy.
The [AGENTS.md checkpoint](../AGENTS.md#temporary-uia-timing-checkpoint-2026-10-14)
requires hardening or removal before significant unrelated work.

Authenticated `GET /vision/uia-metrics` adds `timing`: fixed stage names with
call counts, cumulative and maximum microseconds, plus the first timeout-stage
counts. Fixed rule-type names have check counts, match counts and elapsed
microseconds. A rule-type match count means the check found a suppressing
condition, including an include-list rejection. All counters are process-lifetime and approximate during concurrent
reads. A timeout or over-budget attempt emits one structured `UIA timing diagnostic`
log record with stage durations, rule-type durations, the first timeout stage,
and up to 64 applicable rule references. It contains no captured text, URL,
title, app identity, pattern, HWND, PID or file path.

The stages separate native policy, UIA initialization, root lookup, event
registration, cache configuration, hierarchy cache, document-value query,
Window/Document-name query, extension-child query, cached decoding, policy
projection, full-text recording cache/projection, focus verification,
finalization, event cleanup and failed-target recheck. Rule-type timings cover
incognito, native ignored/included windows, nested Window, Document name/value,
extension shallow checks, and ignored URL matching. URL matching occurs after
the inner UIA attempt, so its rule-type counter is separate from that attempt's
stage totals. DRM URL use of acquired evidence is outside this UIA walker.

Acquisition stages are shared evidence for multiple rule IDs; their time cannot
be assigned to one rule. Rule-type matching time excludes provider acquisition.
The stage timer identifies the first observed deadline or UIA timeout before
cleanup overwrites the active stage. A COM call that never returns cannot emit
its own completion log; a future external watchdog is needed for that case.
Current validation must not equate a slow stage with an unsafe admission. Only
an owner-run deployed trial can establish live timing; compilation and unit
tests do not establish provider behavior.
