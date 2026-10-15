import { describe, expect, it } from "vitest";
import { parseRecordingStatus, parseLiveStatus, evidenceAge, recordingDeviceScope, recordingObservationLabel, recordingPolicyExplanation, recordingReason } from "../recording-status";
const observation = { channel: "microphone", source: "audio_device", scope: { device: 3 }, condition: "silent", reasons: ["silent_input"], rules: [], since_ms: 100, observed_at_ms: 200 };
const response = (row = observation) => ({ active_intervals: [row], persistence_degraded: false });
describe("recording status", () => {
  it.each(["uia_walk_timeout", "uia_node_limit", "uia_depth_limit", "uia_property_text_limit", "uia_aggregate_text_limit", "uia_provider_failure", "uia_backoff", "uia_initialization_failed"])("reports processing despite %s when independent privacy checks passed", (reason) => {
    const row = parseRecordingStatus({ active_intervals: [{ ...observation, channel: "screen", source: "window_policy", condition: "admitted", reasons: [reason, "accessibility_processing_continues"] }], persistence_degraded: false }).rows[0];
    expect(recordingObservationLabel(row)).toContain("Screenshot storage and OCR proceeding despite accessibility retrieval failure");
    expect(recordingPolicyExplanation(row)).toContain("privacy checks passed");
    expect(recordingObservationLabel(row)).not.toContain("withheld");
    expect(row.reasons).toContain(reason.replaceAll("_", " "));
  });
  it.each([["ignored_windows", 3, "uia_subtree_exclusion", "Nested window/document exclusion"], ["ignored_urls", 2, "uia_document_url", "Accessibility document URL exclusion"], ["builtin", 0, "native_window_policy", "Current window identity/title"], ["builtin", 0, "drm_url", "Protected-content document URL"]])("identifies unresolved %s rules blocking storage and OCR", (origin, index, kind, detail) => {
    const row = parseRecordingStatus({ active_intervals: [{ ...observation, channel: "ocr", source: "window_policy", condition: "failed", reasons: ["uia_provider_failure", "accessibility_privacy_unverified"], rules: [{ origin, index, kind, text: "private pattern" }] }], persistence_degraded: false }).rows[0];
    expect(recordingObservationLabel(row)).toContain("Screenshot storage and OCR withheld");
    expect(recordingPolicyExplanation(row)).toContain(String(detail));
    expect(recordingPolicyExplanation(row)).toContain(origin + "[" + index + "]");
    expect(JSON.stringify(row)).not.toContain("private pattern");
  });
  it("describes the screen focus-policy pause without implying hardware inactivity", () => {
    const row = parseRecordingStatus({ active_intervals: [{ ...observation, channel: "screen", source: "user_preference", condition: "suppressed", reasons: ["monitor_inactive"] }], persistence_degraded: false }).rows[0];
    expect(recordingObservationLabel(row)).toBe("Paused — waiting for focus");
    expect(row.reasons).toEqual(["monitor inactive"]);
  });
  it("describes accessibility monitor ownership without asserting a screen capture pause", () => {
    const row = parseRecordingStatus({ active_intervals: [{ ...observation, channel: "accessibility", source: "window_policy", condition: "suppressed", reasons: ["monitor_inactive"] }], persistence_degraded: false }).rows[0];
    expect(recordingObservationLabel(row)).toBe("Foreground window is on another display");
    expect(row.reasons).toEqual(["monitor inactive"]);
  });
  it.each([["uia_walk_timeout","deadline exceeded"],["uia_node_limit","node limit reached"],["uia_depth_limit","depth limit reached"],["uia_property_text_limit","property text limit reached"],["uia_aggregate_text_limit","aggregate text limit reached"],["uia_provider_failure","provider request failed"],["uia_backoff","provider retry cooldown"]])("explains %s without calling it an exclusion", (reason, detail) => {
    const row = parseRecordingStatus({ active_intervals: [{ ...observation, channel: "screen", source: "window_policy", condition: "failed", reasons: [reason] }], persistence_degraded: false }).rows[0];
    expect(recordingObservationLabel(row)).toBe(`Failed — Accessibility inspection: ${detail}`);
    expect(row.reasons).not.toContain("no safe window");
  });
  it("surfaces the inspection failure before background policy context", () => {
    const row = parseRecordingStatus({ active_intervals: [{ ...observation, channel: "screen", source: "window_policy", condition: "failed", reasons: ["excluded_background", "window_metadata_unavailable", "uia_walk_timeout"] }], persistence_degraded: false }).rows[0];
    expect(recordingObservationLabel(row)).toBe("Failed — Accessibility inspection: deadline exceeded");
  });
  it("distinguishes deliberate maximised-window scoping from an exclusion", () => {
    const row = parseRecordingStatus({ ...response(), active_intervals: [{ ...observation, channel: "screen", source: "window_policy", condition: "active_window_only", reasons: ["maximised_window_only"] }] }).rows[0];
    expect(recordingObservationLabel(row)).toBe("Maximised window only");
    expect(recordingPolicyExplanation(row)).toContain("same selected maximised window");
    expect(recordingPolicyExplanation(row)).not.toMatch(/matching configured rule|Excluded|covered by a maximized/);
  });
  it("distinguishes a configured exclusion from missing-metadata fallback without sensitive match text", () => {
    const fallback = { ...observation, channel: "screen", source: "window_policy", scope: { monitor: 1 }, condition: "active_window_only", reasons: ["excluded_background", "window_metadata_unavailable"] };
    const missing = parseRecordingStatus({ ...response(), active_intervals: [fallback] }).rows[0];
    expect(recordingObservationLabel(missing)).toBe("Active window only — Window metadata could not be read");
    expect(recordingPolicyExplanation(missing)).toContain("missing app or window-title metadata");
    expect(recordingPolicyExplanation(missing)).toContain("No matching configured rule was identified");
    const configured = parseRecordingStatus({ ...response(), active_intervals: [{ ...fallback, condition: "redacted", reasons: ["excluded_background"], rules: [{ origin: "ignored_windows", index: 2, kind: "app_substring" }] }] }).rows[0];
    expect(recordingPolicyExplanation(configured)).toContain("ignored_windows[2]");
    expect(recordingPolicyExplanation(configured)).not.toContain("No matching configured rule");
  });
  it("distinguishes silent input, monitor exclusions and open microphone admission", () => {
    const result = parseRecordingStatus({ active_intervals: [observation,
      { ...observation, channel: "screen", source: "window_policy", scope: { monitor: 2 }, condition: "suppressed", reasons: ["excluded_foreground"], rules: [{ origin: "ignored_windows", index: 4, kind: "app_substring" }] },
      { ...observation, source: "privacy_admission", scope: "global", condition: "admitted", reasons: ["gate_open"] }], persistence_degraded: true });
    expect(result.rows.map((row) => row.status)).toEqual(["Silent samples received", "Suppressed", "Capture permitted"]);
    expect(result.rows[1].scope).toBe("Monitor 2");
    expect(result.rows[1].rules).toEqual(["Rule: ignored_windows[4] · app substring (configuration index)"]);
    expect(result.degraded).toBe(true);
  });
  it("never echoes arbitrary messages, excluded identities or sensitive rule content", () => {
    const value = parseRecordingStatus(response({ ...observation, message: "secret", excluded_apps: ["private"], rules: [{ origin: "ignored_windows", index: 1, kind: "app_substring", text: "sensitive title" }] } as typeof observation));
    expect(JSON.stringify(value)).not.toMatch(/secret|private|sensitive title/);
  });
  it("keeps only bounded executable basenames in separate window-policy metadata and deduplicates rule references", () => {
    const rule = { origin: "ignored_windows", index: 2, kind: "app_substring" };
    const window = { ...observation, channel: "screen", source: "window_policy", condition: "redacted", rules: [rule, rule],
      excluded_apps: ["explorer.exe", "EXCEL.exe", "explorer.exe", "C:\\private\\secret.exe", "https://private/secret.exe", "private title", "bad\nname.exe", "a".repeat(121) + ".exe"] };
    const parsed = parseRecordingStatus({ ...response(), active_intervals: [window] }).rows[0];
    expect(parsed.blockerApps).toEqual(["explorer.exe", "excel.exe"]);
    expect(parsed.rules).toHaveLength(1);
    expect(JSON.stringify(parsed)).not.toMatch(/private|secret|bad/);
    expect(parseRecordingStatus({ ...response(), active_intervals: [{ ...window, source: "privacy_admission" }] }).rows[0].blockerApps).toEqual([]);
  });
  it.each(["channel", "source", "condition", "reasons", "scope", "rules"])("rejects unrecognized %s instead of displaying unsafe text", (field) => {
    expect(() => parseRecordingStatus(response({ ...observation, [field]: "private value" }))).toThrow();
  });
  it("does not invent an active state for missing observations", () => expect(parseRecordingStatus({ active_intervals: [], persistence_degraded: false }).rows).toEqual([]));
  it.each([
    ["content_screening_blocked", "Frame withheld: content screening matched a blocked rule"],
    ["content_screening_unavailable", "Frame withheld: content screening could not complete"],
  ])("explains %s content-screening withholding", (reason, detail) => {
    const row = parseRecordingStatus({ active_intervals: [{ ...observation, channel: "screen", source: "window_policy", condition: "failed", reasons: [reason] }], persistence_degraded: false }).rows[0];
    expect(recordingObservationLabel(row)).toBe(`Failed — ${detail}`);
  });
  it("explains content-screening redaction", () => {
    const row = parseRecordingStatus({ active_intervals: [{ ...observation, channel: "screen", source: "redaction", condition: "partially_redacted", reasons: ["content_screening_redacted"] }], persistence_degraded: false }).rows[0];
    expect(recordingObservationLabel(row)).toBe("Partially redacted — Content screening blacked out detected secrets before saving");
  });
  it("explains the accessibility fail-open screened reason", () => {
    const row = parseRecordingStatus({ active_intervals: [{ ...observation, channel: "screen", source: "window_policy", condition: "admitted", reasons: ["accessibility_fail_open_screened"] }], persistence_degraded: false }).rows[0];
    expect(recordingReason(row.reasons[0])).toBe("Accessibility inspection failed; frame kept only after passing content screening");
  });
  it("accepts the fixed disconnected-monitor lifecycle reason", () => {
    const parsed = parseRecordingStatus(response({ ...observation, channel: "screen", scope: { monitor: 42 }, condition: "stopped", reasons: ["monitor_disconnected"] } as typeof observation)).rows[0];
    expect(recordingObservationLabel(parsed)).toBe("Monitor disconnected");
  });
  it("rejects impossible timestamps", () => expect(() => parseRecordingStatus(response({ ...observation, since_ms: 300 }))).toThrow());
  it("explains event loss, audio queue capacity, confirmed and possible loss, and shutdown failures", () => {
    const result = parseRecordingStatus({ ...response(), event_delivery: { near_capacity: true, dropped_events: 3 },
      diagnostic_delivery: { near_capacity: false, dropped_observations: 2 },
      audio_delivery: { queues: [{ queue: "recording", near_capacity: true, dropped_deliveries: 4, possible_lost_deliveries: 5 }] },
      audio_shutdown_degraded: true, audio_shutdown_issues: ["consumer_drain_timeout"] });
    expect(result.alerts.join("\n")).toContain("Activity subscriber deliveries dropped: 3");
    expect(result.alerts.join("\n")).toContain("Audio processing queue is nearly full");
    expect(result.alerts.join("\n")).toContain("4 deliveries dropped");
    expect(result.alerts.join("\n")).toContain("5 deliveries could not be verified");
    expect(result.alerts.join("\n")).toContain("consumer drain timeout");
    expect(result.alerts.join("\n")).toContain("Capture diagnostic observations lost: 2");
  });
  it.each([
    { event_delivery: { near_capacity: true, dropped_events: -1 } },
    { audio_delivery: { queues: [{ queue: "private path", near_capacity: true, dropped_deliveries: 0, possible_lost_deliveries: 0 }] } },
    { audio_shutdown_issues: ["private path"] }, { audio_shutdown_degraded: "yes" },
  ])("rejects invalid delivery/shutdown payloads", (fields) => expect(() => parseRecordingStatus({ ...response(), ...fields })).toThrow());
});

describe("ephemeral audio device metadata and explicit reasons", () => {
  const named = (names: unknown) => ({ live_status: { ...live().live_status, device_names: names } });
  it("maps names by channel and device without inferring identities", () => {
    const names = parseLiveStatus(named([{ channel: "microphone", device: 2, name: "USB microphone" }])).deviceNames;
    expect(recordingDeviceScope("Device 2", "microphone", names)).toBe("USB microphone (device 2)");
    expect(recordingDeviceScope("Device 2", "output_audio", names)).toBe("Unknown device (ID 2)");
    expect(recordingDeviceScope("Device unknown", "microphone", names)).toBe("Device unknown");
  });
  it("isolates invalid or ambiguous names from trustworthy capture status", () => {
    const value = parseLiveStatus(named([{ channel: "microphone", device: 2, name: "first" }, { channel: "microphone", device: 2, name: "second" }, { channel: "microphone", device: 3, name: "bad\nname" }, { channel: "output_audio", device: 4, name: "Speakers" }]));
    expect(value.deviceNames).toEqual([{ channel: "output_audio", device: 4, name: "Speakers" }]);
    expect(value.evidence).toHaveLength(1);
    expect(parseLiveStatus(named("invalid")).deviceNames).toEqual([]);
  });
  it("counts Unicode code points and bounds metadata independently", () => {
    expect(parseLiveStatus(named([{ channel: "microphone", device: 2, name: "🎙".repeat(160) }])).deviceNames).toHaveLength(1);
    expect(parseLiveStatus(named([{ channel: "microphone", device: 2, name: "🎙".repeat(161) }])).deviceNames).toEqual([]);
    expect(parseLiveStatus(named(Array.from({ length: 1025 }, (_, index) => ({ channel: "microphone", device: index + 1, name: "mic" })))).deviceNames).toEqual([]);
  });
  it("explains disabled and ended states using fixed reasons", () => {
    const stopped = parseRecordingStatus(response({ ...observation, condition: "stopped", reasons: ["disabled"] })).rows[0];
    expect(recordingObservationLabel(stopped)).toBe("Disabled in recording settings");
    expect(recordingObservationLabel({ ...stopped, reasons: ["session stopped"] })).toBe("Recorder session ended");
  });
});

const evidence = { channel: "microphone", scope: { device: 2 }, capture_supported: true, storage_supported: true,
  last_capture_at_ms: 123, last_storage_at_ms: null, capture_age_ms: 2000, storage_age_ms: null };
const live = (row: unknown = evidence) => ({ live_status: { schema_version: 1, snapshot_at_ms: 300, session_started_at_ms: 100,
  session_uptime_ms: 200, evidence_available: true, evidence: [row], interval_checks: [] } });
describe("current session recording evidence", () => {
  const topology = { available: true, checked_age_ms: 1, monitors: [{ monitor_id: 42, connected: false, last_seen_at_ms: 100, last_seen_age_ms: 200 }] };
  const withTopology = (monitor_topology: unknown) => ({ live_status: { ...live().live_status, monitor_topology } });
  it("parses bounded monitor presence independently of capture and accepts older recorders without inferring removal", () => {
    const parsed = parseLiveStatus(withTopology(topology));
    expect(parsed.monitorTopology).toEqual({ available: true, checkedAge: 1, monitors: [{ id: 42, connected: false, lastSeen: 100, lastSeenAge: 200 }] });
    expect(parsed.evidence).toHaveLength(1);
    expect(parseLiveStatus(live()).monitorTopology).toBeUndefined();
    expect(JSON.stringify(parseLiveStatus(withTopology({ ...topology, message: "private", monitors: [{ ...topology.monitors[0], name: "secret title" }] })))).not.toMatch(/private|secret/);
  });
  it.each([
    { ...topology, available: "yes" }, { ...topology, checked_age_ms: null },
    { ...topology, monitors: [topology.monitors[0], topology.monitors[0]] },
    { ...topology, monitors: [{ ...topology.monitors[0], monitor_id: -1 }] },
    { ...topology, monitors: [{ ...topology.monitors[0], connected: "false" }] },
    { ...topology, monitors: [{ ...topology.monitors[0], last_seen_age_ms: null }] },
    { ...topology, monitors: [{ ...topology.monitors[0], connected: true, last_seen_at_ms: null, last_seen_age_ms: null }] },
  ])("rejects invalid monitor topology without silently hiding a recording source", (invalid) => expect(() => parseLiveStatus(withTopology(invalid))).toThrow());
  it("keeps capture success independent of absent storage success", () => {
    const result = parseLiveStatus(live()).evidence[0];
    expect(result.captureAge).toBe(2000); expect(result.storageAge).toBeNull();
    expect(evidenceAge(result.captureAge, 1000)).toBe("3s ago");
    expect(evidenceAge(result.storageAge)).toBe("No success observed in this recorder session");
  });
  it("distinguishes unsupported instrumentation from no observed success", () => {
    const row = parseLiveStatus(live({ ...evidence, storage_supported: false })).evidence[0];
    expect(row.storageSupported).toBe(false); expect(row.captureSupported).toBe(true);
    expect(parseLiveStatus({ live_status: { ...live().live_status, evidence_available: false } }).evidence).toEqual([]);
  });
  it("uses producer check ages without treating condition onset as freshness", () => {
    const body = { ...response(), live_status: { ...live().live_status, interval_checks: [{ channel: "microphone", source: "audio_device", scope: { device: 3 }, checked_at_ms: 1000, checked_age_ms: 1 }] } };
    expect(parseRecordingStatus(body).rows[0].checkedAge).toBe(1);
    expect(parseRecordingStatus(response()).rows[0].checkedAge).toBeNull();
  });
  it("never renders unknown server text or identities from evidence", () => {
    expect(JSON.stringify(parseLiveStatus(live({ ...evidence, device_name: "sensitive", message: "private path" })))).not.toMatch(/sensitive|private path/);
  });
  it.each([
    { ...evidence, channel: "private text" }, { ...evidence, capture_age_ms: null },
    { ...evidence, storage_supported: false, last_storage_at_ms: 200, storage_age_ms: 100 },
    { ...evidence, scope: { monitor: -1 } }, { ...evidence, capture_age_ms: -1 },
    { ...evidence, last_capture_at_ms: undefined },
  ])("rejects invalid or contradictory evidence", (row) => expect(() => parseLiveStatus(live(row))).toThrow());
  it("rejects old servers and duplicate source evidence", () => {
    expect(() => parseLiveStatus(response())).toThrow();
    expect(() => parseLiveStatus({ live_status: { ...live().live_status, evidence: [evidence, evidence] } })).toThrow();
  });
});
