# UIA timing diagnostic trial (2026-10-14)

Timing diagnostics measure each fresh UIA attempt. Instrumentation
does not change rule evaluation, capture admission, deadlines, or retry policy.
The limits below govern interpretation of these measurements. Live usefulness
and overhead remain unvalidated; these diagnostics do
not establish production hardening.

Authenticated `GET /vision/uia-metrics` adds `timing`: fixed stage names with
call counts, cumulative and maximum microseconds, plus the first timeout-stage
counts. Fixed rule-type names have check counts, match counts and elapsed
microseconds. A rule-type match count means the check found a suppressing
condition, including an include-list rejection. All counters are process-lifetime and approximate during concurrent
reads. Timeout/over-budget warnings are sampled at the first diagnostic and every
64th thereafter, with exact diagnostic counts retained in metrics. Each sampled
`UIA timing diagnostic` log contains stage durations, rule-type durations, the first timeout stage,
and up to 64 applicable rule references. It contains no captured text, URL,
title, app identity, pattern, HWND, PID or file path.

`initialization` (2026-10-15) splits `uia_initialization_failed` into fixed
`failure_codes`/`failure_counts`: `com_changed_mode` (the thread was already in
a single-threaded apartment), `com_init_failed` and `uia_context_failed`
(creating the UIA client objects failed). `failure_hresult_counts` has one row
per failure code and one column per fixed `hresult_codes` entry (ten known
COM/UIA HRESULTs, then `other`); the sensitive debug record
`uia_initialization_failure` adds the exact numeric HRESULT, never the error
message. Walks run on shared blocking-pool
threads; one that arrives on a thread another component left outside the MTA
runs on a fresh thread instead and counts in `apartment_rehomed_walks`
(`apartment_rehome_failures` if that thread could not run it, which fails
closed).

The desktop GUI owns the live loopback API and links `screenpipe-a11y` directly.
Build and deploy the desktop executable as well as the recorder for a live trial;
updating `screenpipe.exe` alone leaves this endpoint on the previous schema.

The stages separate native policy, UIA initialization, root lookup, event
registration, cache configuration, hierarchy cache, document-value query,
Window/Document-name query, extension-child query, cached decoding, policy
projection, full-text recording cache/projection, focus verification,
finalization, event cleanup and failed-target recheck. Rule-type timings cover
incognito, native ignored/included windows, nested Window, Document name/value,
extension shallow checks, and ignored URL matching. The outer URL decorator runs
after the inner UIA attempt closes and updates rule counters separately; URL
checks within the walker remain part of its trace. Historical query-stage slots
remain in the vectors even when unused. DRM URL use of acquired evidence is
outside this UIA walker.

Acquisition stages are shared evidence for multiple rule IDs; their time cannot
be assigned to one rule. Rule-type matching time excludes provider acquisition
and overlaps stage timing; do not add it as an independent cost.
The stage timer identifies the first observed deadline or UIA timeout before
cleanup overwrites the active stage. A COM call that never returns cannot emit
its own completion log or finished-attempt process record; phase counters may
already have advanced. There is no external watchdog in these diagnostics and
no hard wall-clock return guarantee.
Current validation must not equate a slow stage with an unsafe admission. Only
an owner-run deployed trial can establish live timing; compilation and unit
tests do not establish provider behavior.
