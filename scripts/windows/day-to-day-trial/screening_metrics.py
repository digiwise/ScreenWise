"""Read-only, aggregate-only content-screening and UIA failure metrics.

Covers the owner's standing permissions in AGENTS.md: aggregates over frame
metadata and capture_privacy fixed codes, flags, counts and executable basenames;
captured text (full_text) only as present vs null/empty, never its value.
Every query groups and counts; no row is printed. Code columns are checked
against a fixed-code pattern and app columns against a basename pattern, and
anything else prints as `other`, so an unexpected value cannot leak text.
"""
import argparse
import pathlib
import re
import sqlite3
import sys

CODE = re.compile(r"^[a-z0-9_]{1,48}$")
APP = re.compile(r"^[A-Za-z0-9_.\- ]{1,64}$")
CODE_COLUMNS = {"text_source", "outcome", "reason", "processing", "cause", "phase", "blocker_reason", "path", "foreground_skip", "window_choice"}
APP_COLUMNS = {"app_name", "sampled_app", "app", "window_app"}
GENERATED_COLUMNS = {"utc_bucket"}

CS = "json_extract(capture_privacy, '$.content_screening"
A = "json_extract(capture_privacy, '$.accessibility"
SINCE = ("julianday(timestamp) >= julianday(:since) AND julianday(timestamp) < julianday(:until) "
         "AND json_valid(capture_privacy)")
SCREENED = f"{SINCE} AND json_type(capture_privacy, '$.content_screening') = 'object'"
WITH_A11Y = f"{SINCE} AND json_type(capture_privacy, '$.accessibility') = 'object'"


def percentiles(field):
    return f"""
WITH v AS (SELECT {CS}.{field}') AS ms FROM frames WHERE {SCREENED} AND {CS}.{field}') IS NOT NULL),
r AS (SELECT ms, ROW_NUMBER() OVER (ORDER BY ms) AS rn, COUNT(*) OVER () AS n FROM v)
SELECT MAX(n) AS n, MIN(CASE WHEN rn >= 0.50 * n THEN ms END) AS p50,
       MIN(CASE WHEN rn >= 0.90 * n THEN ms END) AS p90,
       MIN(CASE WHEN rn >= 0.99 * n THEN ms END) AS p99, MAX(ms) AS max, SUM(ms) AS total FROM r"""


SECTIONS = {
    "screening": [
        ("Frames since start", f"""
SELECT COUNT(*) AS frames,
       SUM(json_type(capture_privacy, '$.content_screening') = 'object') AS screened,
       SUM(text_source = 'privacy_placeholder') AS withheld_placeholders,
       ROUND((julianday(MAX(timestamp)) - julianday(MIN(timestamp))) * 1440, 1) AS span_min
FROM frames WHERE {SINCE}"""),
        ("Frames per 10-minute UTC bucket", f"""
SELECT strftime('%Y-%m-%d %H:', timestamp) || printf('%02d', (CAST(strftime('%M', timestamp) AS INT) / 10) * 10) AS utc_bucket,
       COUNT(*) AS frames, SUM(text_source = 'privacy_placeholder') AS withheld,
       SUM({CS}.outcome') = 'passed') AS passed, SUM({CS}.outcome') = 'redacted') AS redacted,
       SUM({CS}.outcome') = 'blocked') AS blocked, SUM({CS}.outcome') = 'unavailable') AS unavailable
FROM frames WHERE {SINCE} GROUP BY utc_bucket ORDER BY utc_bucket"""),
        ("Screening outcome x text_source (ms)", f"""
SELECT text_source, {CS}.outcome') AS outcome, COUNT(*) AS frames,
       SUM({CS}.ocr_forced')) AS forced, SUM({CS}.fail_open')) AS fail_open,
       ROUND(AVG({CS}.ocr_ms')), 1) AS avg_ocr, MAX({CS}.ocr_ms')) AS max_ocr,
       ROUND(AVG({CS}.screen_ms')), 1) AS avg_screen, MAX({CS}.screen_ms')) AS max_screen,
       ROUND(AVG({CS}.redact_ms')), 1) AS avg_redact, SUM({CS}.redacted_regions')) AS regions
FROM frames WHERE {SCREENED} GROUP BY 1, 2 ORDER BY frames DESC"""),
        ("OCR ms percentiles", percentiles("ocr_ms")),
        ("Screen ms percentiles", percentiles("screen_ms")),
        ("Screening reason codes", f"""
SELECT j.value AS reason, COUNT(*) AS frames
FROM frames, json_each(capture_privacy, '$.content_screening.reasons') AS j
WHERE {SCREENED} GROUP BY 1 ORDER BY frames DESC"""),
        ("Hit locations and forced-OCR yield", f"""
SELECT SUM({CS}.hits_ocr_only')) AS ocr_only, SUM({CS}.hits_a11y_only')) AS a11y_only,
       SUM({CS}.hits_both')) AS both_sources,
       SUM(CASE WHEN {CS}.ocr_forced') = 1 THEN {CS}.ocr_ms') END) AS forced_ocr_ms,
       SUM(CASE WHEN {CS}.ocr_forced') = 1 AND {CS}.hits_ocr_only') > 0 THEN 1 ELSE 0 END) AS forced_with_ocr_only_hit
FROM frames WHERE {SCREENED}"""),
        ("Per app (top 15)", f"""
SELECT app_name, COUNT(*) AS frames, SUM({CS}.ocr_forced')) AS forced,
       ROUND(AVG({CS}.ocr_ms')), 0) AS avg_ocr, SUM({CS}.outcome') <> 'passed') AS not_passed
FROM frames WHERE {SCREENED} GROUP BY 1 ORDER BY frames DESC LIMIT 15"""),
    ],
    "uia": [
        ("Capture reason x text_source", f"""
SELECT json_extract(capture_privacy, '$.reason') AS reason, text_source, COUNT(*) AS frames
FROM frames WHERE {SINCE} GROUP BY 1, 2 ORDER BY frames DESC"""),
        ("Accessibility processing x cause x phase", f"""
SELECT json_extract(capture_privacy, '$.reason') AS reason, {A}.processing') AS processing,
       {A}.cause') AS cause, {A}.phase') AS phase, COUNT(*) AS frames,
       SUM(text_source = 'privacy_placeholder') AS withheld
FROM frames WHERE {WITH_A11Y} GROUP BY 1, 2, 3, 4 ORDER BY frames DESC"""),
        ("Sampled app x cause (top 20)", f"""
SELECT {A}.sampled_target_app') AS sampled_app, {A}.cause') AS cause, COUNT(*) AS frames,
       SUM(text_source = 'privacy_placeholder') AS withheld
FROM frames WHERE {WITH_A11Y} GROUP BY 1, 2 ORDER BY frames DESC LIMIT 20"""),
    ],
    "privacy": [
        ("Capture outcome x reason x text_source", f"""
SELECT json_extract(capture_privacy, '$.outcome') AS outcome, json_extract(capture_privacy, '$.reason') AS reason,
       json_extract(capture_privacy, '$.active_window_choice') AS window_choice, text_source, COUNT(*) AS frames
FROM frames WHERE {SINCE} GROUP BY 1, 2, 3, 4 ORDER BY frames DESC"""),
        ("Blocker reason x role (frames)", f"""
SELECT r.value AS blocker_reason, json_extract(b.value, '$.foreground') AS foreground,
       COUNT(DISTINCT frames.rowid) AS frames
FROM frames, json_each(capture_privacy, '$.blockers') AS b, json_each(b.value, '$.reasons') AS r
WHERE {SINCE} GROUP BY 1, 2 ORDER BY frames DESC"""),
        ("Capture reason x foreground blocker reason", f"""
SELECT json_extract(capture_privacy, '$.reason') AS reason, r.value AS blocker_reason,
       COUNT(DISTINCT frames.rowid) AS frames
FROM frames, json_each(capture_privacy, '$.blockers') AS b, json_each(b.value, '$.reasons') AS r
WHERE {SINCE} AND json_extract(b.value, '$.foreground') = 1 GROUP BY 1, 2 ORDER BY frames DESC"""),
        ("Withheld reason x blocker app x reason x role (top 30)", f"""
SELECT json_extract(capture_privacy, '$.reason') AS reason, json_extract(b.value, '$.app') AS app,
       r.value AS blocker_reason, json_extract(b.value, '$.foreground') AS foreground,
       COUNT(DISTINCT frames.rowid) AS frames
FROM frames, json_each(capture_privacy, '$.blockers') AS b, json_each(b.value, '$.reasons') AS r
WHERE {SINCE} AND text_source = 'privacy_placeholder' GROUP BY 1, 2, 3, 4 ORDER BY frames DESC LIMIT 30"""),
        ("No-safe-window detail", f"""
SELECT json_extract(capture_privacy, '$.no_safe_window.phase') AS phase,
       json_extract(capture_privacy, '$.no_safe_window.path') AS path,
       json_extract(capture_privacy, '$.no_safe_window.cause') AS cause,
       json_extract(capture_privacy, '$.no_safe_window.foreground_skip') AS foreground_skip,
       json_extract(capture_privacy, '$.no_safe_window.window_app') AS window_app,
       json_extract(capture_privacy, '$.no_safe_window.foreground_on_monitor') AS foreground_on_monitor,
       COUNT(*) AS frames,
       ROUND(AVG(json_extract(capture_privacy, '$.no_safe_window.visible_windows')), 1) AS avg_visible,
       ROUND(AVG(json_extract(capture_privacy, '$.no_safe_window.excluded_visible_windows')), 1) AS avg_excluded
FROM frames WHERE {SINCE} AND json_extract(capture_privacy, '$.reason') = 'no_safe_active_window'
GROUP BY 1, 2, 3, 4, 5, 6 ORDER BY frames DESC"""),
        ("Withheld frames without blockers", f"""
SELECT json_extract(capture_privacy, '$.reason') AS reason, COUNT(*) AS frames
FROM frames WHERE {SINCE} AND text_source = 'privacy_placeholder'
  AND COALESCE(json_array_length(capture_privacy, '$.blockers'), 0) = 0
GROUP BY 1 ORDER BY frames DESC"""),
        ("Unlisted foreground decisions", f"""
SELECT json_extract(capture_privacy, '$.unlisted_foreground.decision') AS decision,
       json_extract(capture_privacy, '$.outcome') AS outcome,
       json_extract(capture_privacy, '$.unlisted_foreground.visible') AS visible,
       json_extract(capture_privacy, '$.unlisted_foreground.cloaked') AS cloaked,
       json_extract(capture_privacy, '$.unlisted_foreground.tool_window') AS tool,
       json_extract(capture_privacy, '$.unlisted_foreground.popup') AS popup,
       json_extract(capture_privacy, '$.unlisted_foreground.has_owner') AS has_owner,
       json_extract(capture_privacy, '$.unlisted_foreground.owner_listed') AS owner_listed,
       json_extract(capture_privacy, '$.unlisted_foreground.owner_shown') AS owner_shown,
       COUNT(*) AS frames
FROM frames WHERE {SINCE} AND json_extract(capture_privacy, '$.unlisted_foreground') IS NOT NULL
GROUP BY 1, 2, 3, 4, 5, 6, 7, 8, 9 ORDER BY frames DESC"""),
        ("Native metadata on UIA-failure placeholders", f"""
SELECT json_extract(capture_privacy, '$.native_metadata') AS native_metadata,
       json_extract(capture_privacy, '$.accessibility.cause') AS cause, app_name, COUNT(*) AS frames,
       SUM(window_name IS NOT NULL AND window_name <> '') AS with_title
FROM frames WHERE {SINCE} AND text_source = 'privacy_placeholder'
  AND json_extract(capture_privacy, '$.reason') = 'accessibility_inspection'
GROUP BY 1, 2, 3 ORDER BY frames DESC"""),
        ("Blocker app x reason (top 20)", f"""
SELECT json_extract(b.value, '$.app') AS app, r.value AS blocker_reason,
       json_extract(b.value, '$.foreground') AS foreground, COUNT(DISTINCT frames.rowid) AS frames
FROM frames, json_each(capture_privacy, '$.blockers') AS b, json_each(b.value, '$.reasons') AS r
WHERE {SINCE} GROUP BY 1, 2, 3 ORDER BY frames DESC LIMIT 20"""),
        ("Monitor provenance", f"""
SELECT json_extract(capture_privacy, '$.reason') AS reason,
       json_extract(capture_privacy, '$.is_active_monitor') AS is_active_monitor,
       json_extract(capture_privacy, '$.foreground_monitor_changed') AS monitor_changed,
       json_extract(capture_privacy, '$.blockers_truncated') AS truncated, COUNT(*) AS frames
FROM frames WHERE {SINCE} GROUP BY 1, 2, 3, 4 ORDER BY frames DESC"""),
        ("Stored text presence x text_source", f"""
SELECT text_source, COUNT(*) AS frames,
       SUM(full_text IS NOT NULL AND full_text <> '') AS with_text,
       SUM(full_text IS NULL OR full_text = '') AS without_text
FROM frames WHERE {SINCE} GROUP BY 1 ORDER BY frames DESC"""),
    ],
}


def safe(column, value):
    if value is None or isinstance(value, (int, float)):
        return value
    text = str(value)
    if column in CODE_COLUMNS:
        return text if CODE.match(text) else "other"
    if column in APP_COLUMNS:
        return text if APP.match(text) else "other"
    if column in GENERATED_COLUMNS:
        return text
    return "other"


def run(con, since, sections, until="9999-12-31T00:00:00Z"):
    """Yield (title, columns, rows) with every value passed through `safe`."""
    for name in sections:
        for title, sql in SECTIONS[name]:
            cur = con.execute(sql, {"since": since, "until": until})
            columns = [d[0] for d in cur.description]
            yield title, columns, [[safe(c, v) for c, v in zip(columns, row)] for row in cur.fetchall()]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("database", type=pathlib.Path)
    parser.add_argument("--since", required=True, help="UTC ISO-8601 start, e.g. 2026-10-15T07:07:57Z")
    parser.add_argument("--until", default="9999-12-31T00:00:00Z", help="UTC ISO-8601 end (exclusive)")
    parser.add_argument("--section", choices=sorted(SECTIONS), action="append")
    args = parser.parse_args(argv)
    con = sqlite3.connect(f"file:{args.database.resolve().as_posix()}?mode=ro", uri=True)
    try:
        for title, columns, rows in run(con, args.since, args.section or list(SECTIONS), args.until):
            print(f"\n## {title}\n" + " | ".join(columns))
            for row in rows:
                print(" | ".join("" if v is None else (f"{v:.1f}" if isinstance(v, float) else str(v)) for v in row))
    finally:
        con.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
