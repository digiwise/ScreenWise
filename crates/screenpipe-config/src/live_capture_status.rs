//! Content-free, process-session evidence. Success is not a continuous recording guarantee.
use crate::capture_diagnostics::{now_ms, CaptureChannel, CaptureScope, DiagnosticSource};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};

#[derive(Clone, Copy)]
struct Observation {
    at_ms: u64,
    clock: Instant,
}
#[derive(Default)]
struct Evidence {
    captured: Option<Observation>,
    stored: Option<Observation>,
}
#[derive(Default)]
struct MonitorPresence {
    connected: Option<bool>,
    last_seen: Option<Observation>,
}
struct Store {
    started_at_ms: u64,
    started: Instant,
    evidence: HashMap<(CaptureChannel, CaptureScope), Evidence>,
    checks: HashMap<(CaptureChannel, DiagnosticSource, CaptureScope), Observation>,
    device_names: HashMap<(CaptureChannel, u64), String>,
    monitors: HashMap<u32, MonitorPresence>,
    topology_check: Option<Observation>,
    truncated: bool,
}
impl Default for Store {
    fn default() -> Self {
        Self {
            started_at_ms: now_ms(),
            started: Instant::now(),
            evidence: HashMap::new(),
            checks: HashMap::new(),
            device_names: HashMap::new(),
            monitors: HashMap::new(),
            topology_check: None,
            truncated: false,
        }
    }
}
fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

#[derive(Serialize)]
pub struct LiveEvidence {
    pub channel: CaptureChannel,
    pub scope: CaptureScope,
    pub capture_supported: bool,
    pub storage_supported: bool,
    pub last_capture_at_ms: Option<u64>,
    pub last_storage_at_ms: Option<u64>,
    pub capture_age_ms: Option<u64>,
    pub storage_age_ms: Option<u64>,
}
#[derive(Serialize)]
pub struct IntervalCheck {
    pub channel: CaptureChannel,
    pub source: DiagnosticSource,
    pub scope: CaptureScope,
    pub checked_at_ms: u64,
    pub checked_age_ms: u64,
}
#[derive(Serialize)]
pub struct LiveStatus {
    pub monitor_topology: MonitorTopology,
    /// Ephemeral hardware metadata for the authenticated current-session UI.
    /// Never included in diagnostic notices or persisted history.
    pub device_names: Vec<DeviceName>,
    pub audio_activity: crate::audio_activity::AudioActivity,
    pub schema_version: u8,
    pub snapshot_at_ms: u64,
    pub session_started_at_ms: u64,
    pub session_uptime_ms: u64,
    pub evidence: Vec<LiveEvidence>,
    pub evidence_available: bool,
    pub interval_checks: Vec<IntervalCheck>,
}
#[derive(Serialize, Default)]
pub struct MonitorTopology {
    pub available: bool,
    pub checked_age_ms: Option<u64>,
    pub monitors: Vec<MonitorStatus>,
}
#[derive(Serialize)]
pub struct MonitorStatus {
    pub monitor_id: u32,
    pub connected: Option<bool>,
    pub last_seen_at_ms: Option<u64>,
    pub last_seen_age_ms: Option<u64>,
}

/// Only a successful hardware enumeration may establish presence/removal.
/// Returns newly disconnected scopes, including producers whose tasks already exited.
pub fn observe_monitor_topology(ids: &[u32]) -> Vec<u32> {
    // Desktop session restarts can retain current diagnostic rows from the
    // previous session even though live evidence was reset. Reconcile them too.
    let old_scopes: Vec<_> = crate::capture_diagnostics::current()
        .into_iter()
        .map(|row| row.scope)
        .collect();
    let Ok(mut state) = store().lock() else {
        return Vec::new();
    };
    for scope in old_scopes {
        track_monitor(&mut state, scope);
    }
    let observation = Observation {
        at_ms: now_ms(),
        clock: Instant::now(),
    };
    let mut reconnected = Vec::new();
    for &id in ids {
        if state.monitors.len() >= 1024 && !state.monitors.contains_key(&id) {
            state.truncated = true;
            continue;
        }
        let monitor = state.monitors.entry(id).or_default();
        if monitor.connected == Some(false) {
            reconnected.push(id);
        }
        monitor.connected = Some(true);
        monitor.last_seen = Some(observation);
    }
    let mut disconnected = Vec::new();
    for (&id, monitor) in &mut state.monitors {
        if !ids.contains(&id) && monitor.connected != Some(false) {
            monitor.connected = Some(false);
            disconnected.push(id);
        }
    }
    // A reconnected native ID must earn fresh producer evidence. Historical
    // diagnostic deliveries stay queued/persisted; only current state is reset.
    for &id in &reconnected {
        state
            .evidence
            .retain(|(_, scope), _| *scope != CaptureScope::Monitor(id));
        state
            .checks
            .retain(|(_, _, scope), _| *scope != CaptureScope::Monitor(id));
    }
    state.topology_check = Some(observation);
    drop(state);
    for id in reconnected {
        crate::capture_diagnostics::reset_monitor(id);
    }
    for &id in &disconnected {
        crate::capture_diagnostics::disconnect_monitor(id);
    }
    disconnected
}
pub fn monitor_topology_unavailable() {
    if let Ok(mut state) = store().lock() {
        state.topology_check = None;
    }
}
pub(crate) fn monitor_disconnected(scope: CaptureScope) -> bool {
    let CaptureScope::Monitor(id) = scope else {
        return false;
    };
    store().lock().is_ok_and(|state| {
        state
            .monitors
            .get(&id)
            .is_some_and(|monitor| monitor.connected == Some(false))
    })
}
fn track_monitor(state: &mut Store, scope: CaptureScope) {
    if let CaptureScope::Monitor(id) = scope {
        if state.monitors.len() < 1024 || state.monitors.contains_key(&id) {
            state.monitors.entry(id).or_default();
        } else {
            state.truncated = true;
        }
    }
}
#[derive(Serialize)]
pub struct DeviceName {
    pub channel: CaptureChannel,
    pub device: u64,
    pub name: String,
}
pub fn register_device_name(channel: CaptureChannel, device: u64, name: &str) {
    if device == 0
        || !matches!(
            channel,
            CaptureChannel::Microphone | CaptureChannel::OutputAudio
        )
    {
        return;
    }
    let name: String = name
        .chars()
        .filter(|ch| {
            !ch.is_control() && !matches!(*ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(160)
        .collect();
    if name.trim().is_empty() {
        return;
    }
    if let Ok(mut state) = store().lock() {
        if state.device_names.len() < 1024 || state.device_names.contains_key(&(channel, device)) {
            state.device_names.insert((channel, device), name);
        }
    }
}
/// Called once before recording producers start. Never reads historical database rows.
pub fn start_session() {
    crate::audio_activity::start_session();
    if let Ok(mut state) = store().lock() {
        *state = Store::default();
    }
}
pub(crate) fn checked(channel: CaptureChannel, source: DiagnosticSource, scope: CaptureScope) {
    if let Ok(mut state) = store().lock() {
        track_monitor(&mut state, scope);
        let key = (channel, source, scope);
        if state.checks.len() < 4096 || state.checks.contains_key(&key) {
            state.checks.insert(
                key,
                Observation {
                    at_ms: now_ms(),
                    clock: Instant::now(),
                },
            );
        } else {
            state.truncated = true;
        }
    }
}
fn record(channel: CaptureChannel, scope: CaptureScope, stored: bool) {
    if monitor_disconnected(scope) {
        return;
    }
    if let Ok(mut state) = store().lock() {
        track_monitor(&mut state, scope);
        let key = (channel, scope);
        if state.evidence.len() >= 4086 && !state.evidence.contains_key(&key) {
            state.truncated = true;
            return;
        }
        let evidence = state.evidence.entry(key).or_default();
        let observation = Some(Observation {
            at_ms: now_ms(),
            clock: Instant::now(),
        });
        if stored {
            evidence.stored = observation;
        } else {
            evidence.captured = observation;
        }
    }
}
pub fn captured(channel: CaptureChannel, scope: CaptureScope) {
    record(channel, scope, false);
}
/// Producer must call only after the relevant file write/database transaction succeeds.
pub fn stored(channel: CaptureChannel, scope: CaptureScope) {
    record(channel, scope, true);
}
pub fn snapshot() -> LiveStatus {
    let mut output = LiveStatus {
        monitor_topology: MonitorTopology::default(),
        device_names: Vec::new(),
        audio_activity: crate::audio_activity::snapshot(),
        schema_version: 1,
        snapshot_at_ms: now_ms(),
        session_started_at_ms: 0,
        session_uptime_ms: 0,
        evidence: Vec::new(),
        evidence_available: false,
        interval_checks: Vec::new(),
    };
    let Ok(state) = store().lock() else {
        return output;
    };
    let now = Instant::now();
    output.snapshot_at_ms = now_ms();
    output.evidence_available = !state.truncated;
    output.session_started_at_ms = state.started_at_ms;
    output.session_uptime_ms = now.saturating_duration_since(state.started).as_millis() as u64;
    output.monitor_topology = MonitorTopology {
        available: state.topology_check.is_some() && !state.truncated,
        checked_age_ms: state
            .topology_check
            .map(|value| now.saturating_duration_since(value.clock).as_millis() as u64),
        monitors: state
            .monitors
            .iter()
            .map(|(&monitor_id, monitor)| MonitorStatus {
                monitor_id,
                connected: monitor.connected,
                last_seen_at_ms: monitor.last_seen.map(|value| value.at_ms),
                last_seen_age_ms: monitor
                    .last_seen
                    .map(|value| now.saturating_duration_since(value.clock).as_millis() as u64),
            })
            .collect(),
    };
    output
        .monitor_topology
        .monitors
        .sort_by_key(|monitor| monitor.monitor_id);
    output.device_names = state
        .device_names
        .iter()
        .map(|(&(channel, device), name)| DeviceName {
            channel,
            device,
            name: name.clone(),
        })
        .collect();
    output.device_names.sort_by_key(|row| row.device);
    output.interval_checks = state
        .checks
        .iter()
        .map(|(&(channel, source, scope), value)| IntervalCheck {
            channel,
            source,
            scope,
            checked_at_ms: value.at_ms,
            checked_age_ms: now.saturating_duration_since(value.clock).as_millis() as u64,
        })
        .collect();
    // Global empty rows advertise supported stages without inventing monitor/device identities.
    let channels = [
        CaptureChannel::Screen,
        CaptureChannel::ScreenStorage,
        CaptureChannel::Ocr,
        CaptureChannel::Accessibility,
        CaptureChannel::Keyboard,
        CaptureChannel::Clipboard,
        CaptureChannel::Pointer,
        CaptureChannel::Microphone,
        CaptureChannel::OutputAudio,
        CaptureChannel::Transcription,
    ];
    let keys = channels
        .into_iter()
        .map(|c| (c, CaptureScope::Global))
        .chain(state.evidence.keys().copied());
    let mut seen = std::collections::HashSet::new();
    for (channel, scope) in keys {
        if !seen.insert((channel, scope)) {
            continue;
        }
        let evidence = state.evidence.get(&(channel, scope));
        let captured = evidence.and_then(|e| e.captured);
        let stored = evidence.and_then(|e| e.stored);
        output.evidence.push(LiveEvidence {
            channel,
            scope,
            capture_supported: channel != CaptureChannel::ScreenStorage,
            storage_supported: channel != CaptureChannel::Screen,
            last_capture_at_ms: captured.map(|v| v.at_ms),
            last_storage_at_ms: stored.map(|v| v.at_ms),
            capture_age_ms: captured
                .map(|v| now.saturating_duration_since(v.clock).as_millis() as u64),
            storage_age_ms: stored
                .map(|v| now.saturating_duration_since(v.clock).as_millis() as u64),
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture_diagnostics::{self as diagnostics, CaptureCondition, CaptureReason};
    #[test]
    fn monitor_topology_retires_lock_state_and_preserves_history_on_reconnect() {
        let _lock = crate::audio_activity::tests::GLOBAL_TEST_LOCK
            .lock()
            .unwrap();
        start_session();
        let scope = CaptureScope::Monitor(900_001);
        let source = DiagnosticSource::MonitorCapture;
        observe_monitor_topology(&[900_001, 900_002]);
        diagnostics::observe(
            CaptureChannel::Screen,
            source,
            scope,
            CaptureCondition::Suppressed,
            vec![CaptureReason::ScreenLocked],
            Vec::new(),
        );
        captured(CaptureChannel::Screen, scope);
        let seen = snapshot()
            .monitor_topology
            .monitors
            .iter()
            .find(|row| row.monitor_id == 900_001)
            .unwrap()
            .last_seen_at_ms;
        assert_eq!(observe_monitor_topology(&[900_002]), vec![900_001]);
        let retired = diagnostics::current()
            .into_iter()
            .find(|row| row.scope == scope)
            .unwrap();
        assert_eq!(retired.condition, CaptureCondition::Stopped);
        assert_eq!(retired.reasons, vec![CaptureReason::MonitorDisconnected]);
        assert!(observe_monitor_topology(&[900_002]).is_empty());
        let disconnected = snapshot();
        assert_eq!(
            disconnected
                .monitor_topology
                .monitors
                .iter()
                .find(|row| row.monitor_id == 900_001)
                .unwrap()
                .connected,
            Some(false)
        );
        assert_eq!(
            disconnected
                .monitor_topology
                .monitors
                .iter()
                .find(|row| row.monitor_id == 900_001)
                .unwrap()
                .last_seen_at_ms,
            seen
        );
        // An aborted/blocking producer completing late cannot resurrect its old scope.
        diagnostics::observe(
            CaptureChannel::Screen,
            source,
            scope,
            CaptureCondition::Admitted,
            vec![CaptureReason::GateOpen],
            Vec::new(),
        );
        assert!(diagnostics::current()
            .iter()
            .any(|row| row.scope == scope && row.condition == CaptureCondition::Stopped));
        observe_monitor_topology(&[900_001, 900_002]);
        assert!(!diagnostics::current().iter().any(|row| row.scope == scope));
        assert!(!snapshot().evidence.iter().any(|row| row.scope == scope));
        assert!(!snapshot()
            .interval_checks
            .iter()
            .any(|row| row.scope == scope));
        let history = diagnostics::drain();
        assert!(history
            .iter()
            .any(|row| row.scope == scope && row.reasons == [CaptureReason::ScreenLocked]));
        assert!(history
            .iter()
            .any(|row| row.scope == scope && row.reasons == [CaptureReason::MonitorDisconnected]));
        diagnostics::observe(
            CaptureChannel::Screen,
            source,
            scope,
            CaptureCondition::Admitted,
            vec![CaptureReason::GateOpen],
            Vec::new(),
        );
        assert!(snapshot()
            .interval_checks
            .iter()
            .any(|row| row.scope == scope));
        diagnostics::reset_monitor(900_001);
        start_session();
    }
    #[test]
    fn monitor_topology_distinguishes_failure_empty_enumeration_and_dead_producers() {
        let _lock = crate::audio_activity::tests::GLOBAL_TEST_LOCK
            .lock()
            .unwrap();
        start_session();
        observe_monitor_topology(&[900_003]);
        monitor_topology_unavailable();
        let failed = snapshot().monitor_topology;
        assert!(!failed.available);
        assert_eq!(
            failed
                .monitors
                .iter()
                .find(|row| row.monitor_id == 900_003)
                .unwrap()
                .connected,
            Some(true)
        );
        assert_eq!(failed.checked_age_ms, None);
        assert_eq!(observe_monitor_topology(&[]), vec![900_003]);
        assert!(snapshot().monitor_topology.available);
        assert_eq!(
            snapshot()
                .monitor_topology
                .monitors
                .iter()
                .find(|row| row.monitor_id == 900_003)
                .unwrap()
                .connected,
            Some(false)
        );
        // A producer may have exited before the watcher saw its old native ID.
        checked(
            CaptureChannel::Screen,
            DiagnosticSource::CaptureOperation,
            CaptureScope::Monitor(900_004),
        );
        assert!(observe_monitor_topology(&[900_005]).contains(&900_004));
        assert_eq!(
            snapshot()
                .monitor_topology
                .monitors
                .iter()
                .find(|row| row.monitor_id == 900_004)
                .unwrap()
                .last_seen_at_ms,
            None
        );
        start_session();
        assert!(snapshot().monitor_topology.monitors.is_empty());
        assert!(!snapshot().monitor_topology.available);
    }
    #[test]
    fn monitor_topology_reconciles_retained_diagnostics_after_session_restart() {
        let _lock = crate::audio_activity::tests::GLOBAL_TEST_LOCK
            .lock()
            .unwrap();
        start_session();
        let scope = CaptureScope::Monitor(900_006);
        diagnostics::observe(
            CaptureChannel::Screen,
            DiagnosticSource::MonitorCapture,
            scope,
            CaptureCondition::Suppressed,
            vec![CaptureReason::ScreenLocked],
            Vec::new(),
        );
        start_session();
        assert!(observe_monitor_topology(&[900_007]).contains(&900_006));
        assert_eq!(
            snapshot()
                .monitor_topology
                .monitors
                .iter()
                .find(|row| row.monitor_id == 900_006)
                .unwrap()
                .connected,
            Some(false)
        );
        diagnostics::reset_monitor(900_006);
        start_session();
    }
    #[test]
    fn live_evidence_separates_stages_and_resets_session() {
        let _lock = crate::audio_activity::tests::GLOBAL_TEST_LOCK
            .lock()
            .unwrap();
        start_session();
        captured(CaptureChannel::Microphone, CaptureScope::Device(42));
        let snapshot_before = snapshot();
        let row = snapshot_before
            .evidence
            .iter()
            .find(|e| e.scope == CaptureScope::Device(42))
            .unwrap();
        assert!(row.capture_age_ms.is_some());
        assert_eq!(row.last_storage_at_ms, None);
        stored(CaptureChannel::Microphone, CaptureScope::Device(42));
        assert!(snapshot()
            .evidence
            .iter()
            .find(|e| e.scope == CaptureScope::Device(42))
            .unwrap()
            .storage_age_ms
            .is_some());
        start_session();
        assert!(!snapshot()
            .evidence
            .iter()
            .any(|e| e.scope == CaptureScope::Device(42)));
        assert!(snapshot()
            .evidence
            .iter()
            .all(|e| e.last_capture_at_ms.is_none() && e.last_storage_at_ms.is_none()));
    }
}
