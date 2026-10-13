# screenpipe — AI that knows everything you've seen, said, or heard
# https://screenpi.pe
"""Read-only numeric deployment baseline/comparison; never selects capture text.

Uses authenticated loopback health/vision metrics and aggregate SQL expressions
over capture decision JSON. No event messages, names, URLs, titles, or payloads
are fetched from SQLite or written as diagnostics. Missing metrics stay null.
"""
import argparse
import datetime as dt
import json
import pathlib
import sqlite3
import subprocess
import time
import urllib.request
import urllib.error

OUTCOMES = ("capture_failed", "capture_redacted", "captured")
CAUSES = ("uia_walk_timeout", "uia_provider_error", "uia_node_limit", "uia_depth_limit",
          "uia_text_limit", "uia_worker_unavailable", "uia_property_limit")
METRICS = ("capture_attempts", "frames_captured", "frames_db_written", "frames_dropped",
           "ocr_completed", "avg_ocr_latency_ms", "pipeline_stall_count", "dedup_skips",
           "last_db_write_ts", "last_capture_attempt_ts", "uptime_secs", "avg_db_latency_ms")
UIA_METRICS = ("schema_version", "attempts", "completed", "failed", "denied", "retries", "stale",
               "provider_calls", "nodes", "total_duration_us", "max_duration_us",
               "bulk_acquisition_calls", "root_acquisition_calls", "event_registration_calls",
               "event_cleanup_calls", "event_cleanup_failures",
               "hierarchy_structure_acquisitions", "accessibility_recording_acquisitions")
TIMING_STAGE_COUNT = 21
TIMING_RULE_COUNT = 8


def timing_projection(source):
    """Retain only fixed-size numeric vectors; never persist labels or content."""
    def vector(value, length):
        return ([numeric(item) for item in value] if isinstance(value, list)
                and len(value) == length else None)

    def counters(value, length):
        value = value if isinstance(value, dict) else {}
        return {key: vector(value.get(key), length)
                for key in ("calls", "total_us", "max_us")}

    source = source if isinstance(source, dict) else {}
    return {"stages": counters(source.get("stages"), TIMING_STAGE_COUNT),
            "timeout_stage_counts": vector(source.get("timeout_stage_counts"), TIMING_STAGE_COUNT),
            "rule_types": counters(source.get("rule_types"), TIMING_RULE_COUNT),
            "rule_match_counts": vector(source.get("rule_match_counts"), TIMING_RULE_COUNT)}


def uia_projection(source):
    result = {key: numeric(source.get(key)) for key in UIA_METRICS}
    # Fixed length numeric vectors; discard any free-form values/keys.
    for key in ("histogram_upper_us", "histogram_counts", "reason_counts"):
        value = source.get(key)
        result[key] = ([numeric(item) for item in value[:8]]
                       if isinstance(value, list) and len(value) <= 8 else None)
    return result


def numeric(value):
    return value if isinstance(value, (int, float, bool)) else None


def fetch(base, endpoint, token):
    headers = {"Authorization": "Bearer " + token} if token else {}
    request = urllib.request.Request(base + endpoint, headers=headers)
    start = time.monotonic()
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            # These endpoints contain status/metrics; no search/content endpoints.
            return json.load(response), 200, round((time.monotonic()-start)*1000, 3)
    except urllib.error.HTTPError as error:
        return {}, int(error.code), round((time.monotonic()-start)*1000, 3)
    except Exception:
        return {}, 0, round((time.monotonic()-start)*1000, 3)


def aggregate(connection, since):
    columns = {row[1] for row in connection.execute("PRAGMA table_info(frames)")}
    if not {"timestamp", "capture_privacy"}.issubset(columns):
        return {"available": False}
    # The timestamp index limits work to the requested interval. Valid JSON is
    # classified in SQLite; Python receives counts only, never decision payloads.
    sql = """SELECT COUNT(*),
      SUM(CASE WHEN json_valid(capture_privacy) THEN 1 ELSE 0 END),
      SUM(CASE WHEN json_valid(capture_privacy) THEN
        CASE WHEN json_extract(capture_privacy,'$.outcome')='capture_failed' THEN 1 ELSE 0 END ELSE 0 END),
      SUM(CASE WHEN json_valid(capture_privacy) THEN
        CASE WHEN json_extract(capture_privacy,'$.outcome')='capture_redacted' THEN 1 ELSE 0 END ELSE 0 END),
      SUM(CASE WHEN json_valid(capture_privacy) THEN
        CASE WHEN json_extract(capture_privacy,'$.accessibility.cause')='uia_walk_timeout' THEN 1 ELSE 0 END ELSE 0 END),
      SUM(CASE WHEN json_valid(capture_privacy) THEN
        CASE WHEN json_extract(capture_privacy,'$.accessibility.processing')='withheld' THEN 1 ELSE 0 END ELSE 0 END)
      FROM frames WHERE timestamp>=?"""
    row = connection.execute(sql, (since,)).fetchone()
    keys = ("frames", "decision_rows", "failed", "redacted", "uia_walk_timeout", "uia_withheld")
    values = dict(zip(keys, [int(n or 0) for n in row]))
    values["available"] = True
    values["other_outcomes"] = values["decision_rows"]-values["failed"]-values["redacted"]
    # Compute a numeric recovery proxy within each device in SQLite; device
    # labels and individual timestamps never leave the aggregate query.
    recovery_sql = """WITH scoped AS (
       SELECT timestamp, device_name,
        CASE WHEN json_valid(capture_privacy) THEN
         CASE WHEN json_extract(capture_privacy,'$.outcome')='capture_failed' THEN 1 ELSE 0 END ELSE 0 END failed,
        CASE WHEN json_valid(capture_privacy) THEN
         CASE WHEN json_extract(capture_privacy,'$.outcome') IN ('full_monitor','active_window_only') THEN 1 ELSE 0 END ELSE 0 END admitted
       FROM frames WHERE timestamp>=?), sequenced AS (
       SELECT timestamp, admitted, LAG(failed) OVER(PARTITION BY device_name ORDER BY timestamp) was_failed,
        LAG(timestamp) OVER(PARTITION BY device_name ORDER BY timestamp) previous_time FROM scoped),
       recoveries AS (SELECT (julianday(timestamp)-julianday(previous_time))*86400000 ms
        FROM sequenced WHERE admitted=1 AND was_failed=1)
       SELECT COUNT(*), AVG(ms), MAX(ms) FROM recoveries"""
    if "device_name" in columns:
        recovery = connection.execute(recovery_sql, (since,)).fetchone()
        values["failure_to_admitted_transitions"] = int(recovery[0])
        values["last_failure_to_admitted_avg_ms"] = numeric(recovery[1])
        values["last_failure_to_admitted_max_ms"] = numeric(recovery[2])
    # Metadata-only rows cannot prove readable text or correct admission.
    return values


def sample(deployment, token, connection, since):
    base = "http://127.0.0.1:"+str(int(deployment["port"]))
    health, health_status, health_ms = fetch(base, "/health", token)
    vision, vision_status, vision_ms = fetch(base, "/vision/metrics", token)
    uia, uia_status, uia_ms = fetch(base, "/vision/uia-metrics", token)
    result = {"sample_unix_ms": int(time.time()*1000), "health_http_status": health_status,
              "vision_http_status": vision_status, "health_http_ms": health_ms,
              "vision_http_ms": vision_ms,
              "uia_http_status": uia_status, "uia_http_ms": uia_ms,
              "uia": uia_projection(uia), "timing": timing_projection(uia.get("timing")),
              "vision": {key: numeric(vision.get(key)) for key in METRICS},
              "health": {key: numeric(health.get(key)) for key in
                         ("vision_db_write_stalled", "drm_content_paused", "schedule_paused")},
              "admission_latency_ms": None}
    if uia_status == 200 and numeric(uia.get("attempts")) and numeric(uia.get("total_duration_us")) is not None:
        result["admission_latency_ms"] = uia["total_duration_us"]/uia["attempts"]/1000
    try:
        result["frames"] = aggregate(connection, since)
    except sqlite3.Error:
        result["frames"] = {"available": False}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--deployment", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--seconds", type=int, default=120)
    parser.add_argument("--interval", type=int, default=10)
    parser.add_argument("--lookback-minutes", type=int, default=20)
    parser.add_argument("--since-utc", help="Explicit current-session startup; overrides lookback.")
    args = parser.parse_args()
    if args.seconds < 0 or args.interval < 1 or args.lookback_minutes < 0:
        parser.error("invalid interval")
    deployment = json.loads(pathlib.Path(args.deployment).read_text(encoding="utf-8-sig"))
    # Auth token remains in memory; no subprocess output or errors are printed.
    token_result = subprocess.run([deployment["cli_executable"], "auth", "token", "--data-dir",
                                  deployment["data_directory"]], capture_output=True, text=True, timeout=15)
    token = token_result.stdout.strip().splitlines()[-1] if token_result.returncode == 0 and token_result.stdout.strip() else ""
    database = pathlib.Path(deployment["data_directory"])/"db.sqlite"
    connection = sqlite3.connect(database.as_uri()+"?mode=ro", uri=True, timeout=2)
    connection.execute("PRAGMA query_only=ON")
    since_dt = (dt.datetime.fromisoformat(args.since_utc.replace("Z", "+00:00"))
                if args.since_utc else dt.datetime.now(dt.timezone.utc)-dt.timedelta(minutes=args.lookback_minutes))
    if since_dt.tzinfo is None or since_dt > dt.datetime.now(dt.timezone.utc):
        raise ValueError("Invalid current-session bound")
    since_dt = since_dt.astimezone(dt.timezone.utc)
    since = since_dt.isoformat(timespec="milliseconds").replace("+00:00", "Z")
    output = pathlib.Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    # Exclusive create prevents overwriting earlier evidence.
    with output.open("x", encoding="utf-8") as stream:
        stream.write(json.dumps({"schema": 1, "baseline_start_unix_ms": int(since_dt.timestamp()*1000),
                                 "seconds": args.seconds, "interval_seconds": args.interval,
                                 "lookback_minutes": args.lookback_minutes,
                                 "explicit_session_bound": bool(args.since_utc),
                                 "admission_latency_available": False})+"\n")
        deadline = time.monotonic()+args.seconds
        while True:
            stream.write(json.dumps(sample(deployment, token, connection, since))+"\n")
            stream.flush()
            remaining = deadline-time.monotonic()
            if remaining <= 0:
                break
            time.sleep(min(args.interval, remaining))
    connection.close()
    print(json.dumps({"collector_complete": 1}))


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # Never expose a private path, token, database error, or server response.
        print(json.dumps({"collector_failed": 1}))
        raise SystemExit(1)
