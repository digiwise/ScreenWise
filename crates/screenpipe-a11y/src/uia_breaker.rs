//! Per-process UIA circuit breaker (owner policy 2026-10-16).
//!
//! A UIA request is served on the target application's UI thread, and a
//! client-side timeout does not stop that work. After a costly failure
//! (timeout, node/depth/text limit or provider failure) the target process
//! receives no UIA calls of that scope for an increasing period — 30 s, 60 s,
//! 120 s ... capped at 10 min — and three consecutive successes reset it.
//! `Walk` covers capture-time tree walks; `Text` covers only their optional
//! text phase, so a slow text phase degrades a window to OCR text without
//! withholding its frames. `Probe` covers the cheap keyboard focus probe, so a
//! large tree never disables keystroke privacy checks; a probe is instead
//! skipped while a walk to the same process has run longer than a probe's own
//! provider timeout (it would only queue behind it). Keys are process ids
//! only and the tables are bounded.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const BASE_BACKOFF: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(600);
const RESET_AFTER_SUCCESSES: u32 = 3;
const MAX_TRACKED_PROCESSES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Walk,
    Text,
    Probe,
}

/// Backoff after `failures` consecutive costly failures (>= 1).
pub(crate) fn backoff(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(5);
    BASE_BACKOFF.saturating_mul(1 << doublings).min(MAX_BACKOFF)
}

#[derive(Clone, Copy, Debug, Default)]
struct Gate {
    failures: u32,
    successes: u32,
    open_until: Option<Instant>,
}

impl Gate {
    fn allows(&self, now: Instant) -> bool {
        self.open_until.is_none_or(|until| now >= until)
    }
    fn fail(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        self.successes = 0;
        self.open_until = Some(now + backoff(self.failures));
    }
    fn succeed(&mut self) {
        self.successes = self.successes.saturating_add(1);
        if self.successes >= RESET_AFTER_SUCCESSES {
            self.failures = 0;
        }
    }
    fn idle(&self, now: Instant) -> bool {
        self.failures == 0 && self.allows(now)
    }
}

#[derive(Debug)]
struct Entry {
    walk: Gate,
    text: Gate,
    probe: Gate,
    last_seen: Instant,
}

impl Entry {
    fn gate(&self, scope: Scope) -> &Gate {
        match scope {
            Scope::Walk => &self.walk,
            Scope::Text => &self.text,
            Scope::Probe => &self.probe,
        }
    }
    fn all_allow(&self, now: Instant) -> bool {
        self.walk.allows(now) && self.text.allows(now) && self.probe.allows(now)
    }
    fn idle(&self, now: Instant) -> bool {
        self.walk.idle(now) && self.text.idle(now) && self.probe.idle(now)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Breaker {
    entries: HashMap<u32, Entry>,
}

impl Breaker {
    pub(crate) fn allows(&self, pid: u32, scope: Scope, now: Instant) -> bool {
        self.entries
            .get(&pid)
            .is_none_or(|entry| entry.gate(scope).allows(now))
    }

    pub(crate) fn record(&mut self, pid: u32, scope: Scope, costly_failure: bool, now: Instant) {
        if !costly_failure && !self.entries.contains_key(&pid) {
            return;
        }
        self.evict(now);
        let entry = self.entries.entry(pid).or_insert(Entry {
            walk: Gate::default(),
            text: Gate::default(),
            probe: Gate::default(),
            last_seen: now,
        });
        entry.last_seen = now;
        let gate = match scope {
            Scope::Walk => &mut entry.walk,
            Scope::Text => &mut entry.text,
            Scope::Probe => &mut entry.probe,
        };
        if costly_failure {
            gate.fail(now);
            OPENINGS.fetch_add(1, Ordering::Relaxed);
        } else {
            gate.succeed();
        }
        if entry.idle(now) {
            self.entries.remove(&pid);
        }
    }

    fn evict(&mut self, now: Instant) {
        if self.entries.len() < MAX_TRACKED_PROCESSES {
            return;
        }
        self.entries.retain(|_, entry| !entry.all_allow(now));
        while self.entries.len() >= MAX_TRACKED_PROCESSES {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_seen)
                .map(|(pid, _)| *pid)
            else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }

    fn open_count(&self, scope: Scope, now: Instant) -> u64 {
        self.entries
            .values()
            .filter(|entry| !entry.gate(scope).allows(now))
            .count() as u64
    }
}

/// Start times of walks currently in flight, per target process.
#[derive(Debug, Default)]
pub(crate) struct InFlight {
    walks: HashMap<u32, Vec<Instant>>,
}

impl InFlight {
    fn start(&mut self, pid: u32, now: Instant) {
        if self.walks.len() < MAX_TRACKED_PROCESSES || self.walks.contains_key(&pid) {
            self.walks.entry(pid).or_default().push(now);
        }
    }
    fn finish(&mut self, pid: u32, started: Instant) {
        if let Some(starts) = self.walks.get_mut(&pid) {
            if let Some(index) = starts.iter().position(|start| *start == started) {
                starts.swap_remove(index);
            }
            if starts.is_empty() {
                self.walks.remove(&pid);
            }
        }
    }
    fn longer_than(&self, pid: u32, age: Duration, now: Instant) -> bool {
        self.walks.get(&pid).is_some_and(|starts| {
            starts
                .iter()
                .any(|start| now.saturating_duration_since(*start) > age)
        })
    }
}

static IN_FLIGHT: OnceLock<Mutex<InFlight>> = OnceLock::new();

fn with_in_flight<T>(f: impl FnOnce(&mut InFlight) -> T) -> T {
    let mut guard = IN_FLIGHT
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

/// Marks a walk to `pid` in flight until dropped.
pub(crate) struct WalkInFlight {
    pid: u32,
    started: Instant,
}

impl Drop for WalkInFlight {
    fn drop(&mut self) {
        with_in_flight(|in_flight| in_flight.finish(self.pid, self.started));
    }
}

pub(crate) fn walk_in_flight(pid: u32) -> WalkInFlight {
    let started = Instant::now();
    with_in_flight(|in_flight| in_flight.start(pid, started));
    WalkInFlight { pid, started }
}

/// Whether a walk to `pid` has been running for longer than `age`.
pub(crate) fn long_walk_in_flight(pid: u32, age: Duration) -> bool {
    pid != 0 && with_in_flight(|in_flight| in_flight.longer_than(pid, age, Instant::now()))
}

static BREAKER: OnceLock<Mutex<Breaker>> = OnceLock::new();
static OPENINGS: AtomicU64 = AtomicU64::new(0);
static WALK_SKIPS: AtomicU64 = AtomicU64::new(0);
static TEXT_SKIPS: AtomicU64 = AtomicU64::new(0);
static PROBE_SKIPS: AtomicU64 = AtomicU64::new(0);

fn with_breaker<T>(f: impl FnOnce(&mut Breaker) -> T) -> T {
    let mut guard = BREAKER
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

/// Whether UIA calls of `scope` may be made to `pid` now. Pid 0 is unknown
/// and never tracked (callers already fail such targets closed).
pub(crate) fn allows(pid: u32, scope: Scope) -> bool {
    pid == 0 || with_breaker(|breaker| breaker.allows(pid, scope, Instant::now()))
}

pub(crate) fn record(pid: u32, scope: Scope, costly_failure: bool) {
    if pid != 0 {
        with_breaker(|breaker| breaker.record(pid, scope, costly_failure, Instant::now()));
    }
}

pub(crate) fn note_walk_skipped() {
    WALK_SKIPS.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn note_text_skipped() {
    TEXT_SKIPS.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn note_probe_skipped() {
    PROBE_SKIPS.fetch_add(1, Ordering::Relaxed);
}

/// Numeric counters only (no process ids): openings, skips and open gates.
pub(crate) fn metrics_snapshot() -> serde_json::Value {
    let now = Instant::now();
    let (open_walk, open_text, open_probe) = with_breaker(|breaker| {
        (
            breaker.open_count(Scope::Walk, now),
            breaker.open_count(Scope::Text, now),
            breaker.open_count(Scope::Probe, now),
        )
    });
    serde_json::json!({
        "breaker_openings": OPENINGS.load(Ordering::Relaxed),
        "breaker_walk_skips": WALK_SKIPS.load(Ordering::Relaxed),
        "breaker_text_skips": TEXT_SKIPS.load(Ordering::Relaxed),
        "breaker_probe_skips": PROBE_SKIPS.load(Ordering::Relaxed),
        "breaker_open_walk_processes": open_walk,
        "breaker_open_text_processes": open_text,
        "breaker_open_probe_processes": open_probe,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_from_thirty_seconds_and_caps_at_ten_minutes() {
        let secs: Vec<u64> = (1..=8).map(|n| backoff(n).as_secs()).collect();
        assert_eq!(secs, vec![30, 60, 120, 240, 480, 600, 600, 600]);
    }

    #[test]
    fn costly_failures_open_per_process_and_three_successes_reset() {
        let mut breaker = Breaker::default();
        let t0 = Instant::now();
        breaker.record(10, Scope::Walk, true, t0);
        assert!(!breaker.allows(10, Scope::Walk, t0 + Duration::from_secs(29)));
        assert!(
            breaker.allows(10, Scope::Text, t0),
            "scopes are independent"
        );
        assert!(
            breaker.allows(10, Scope::Probe, t0),
            "walk failures never block probes"
        );
        assert!(
            breaker.allows(11, Scope::Walk, t0),
            "other processes unaffected"
        );
        let t1 = t0 + Duration::from_secs(30);
        assert!(breaker.allows(10, Scope::Walk, t1));
        breaker.record(10, Scope::Walk, true, t1);
        assert!(!breaker.allows(10, Scope::Walk, t1 + Duration::from_secs(59)));
        let t2 = t1 + Duration::from_secs(60);
        assert!(breaker.allows(10, Scope::Walk, t2));
        // Two successes do not forget the failure streak...
        breaker.record(10, Scope::Walk, false, t2);
        breaker.record(10, Scope::Walk, false, t2);
        breaker.record(10, Scope::Walk, true, t2);
        assert!(!breaker.allows(10, Scope::Walk, t2 + Duration::from_secs(119)));
        // ...three in a row do.
        let t3 = t2 + Duration::from_secs(120);
        for _ in 0..3 {
            breaker.record(10, Scope::Walk, false, t3);
        }
        breaker.record(10, Scope::Walk, true, t3);
        assert!(breaker.allows(10, Scope::Walk, t3 + Duration::from_secs(30)));
    }

    #[test]
    fn probes_wait_only_behind_a_long_walk_to_the_same_process() {
        let mut in_flight = InFlight::default();
        let t0 = Instant::now();
        in_flight.start(7, t0);
        let limit = Duration::from_millis(50);
        assert!(!in_flight.longer_than(7, limit, t0 + Duration::from_millis(50)));
        assert!(in_flight.longer_than(7, limit, t0 + Duration::from_millis(51)));
        assert!(!in_flight.longer_than(8, limit, t0 + Duration::from_secs(1)));
        in_flight.finish(7, t0);
        assert!(!in_flight.longer_than(7, limit, t0 + Duration::from_secs(1)));
        assert!(in_flight.walks.is_empty());
        let guard = walk_in_flight(4242);
        std::thread::sleep(Duration::from_millis(5));
        assert!(long_walk_in_flight(4242, Duration::from_millis(1)));
        drop(guard);
        assert!(!long_walk_in_flight(4242, Duration::ZERO));
    }

    #[test]
    fn healthy_processes_are_not_tracked_and_the_table_is_bounded() {
        let mut breaker = Breaker::default();
        let now = Instant::now();
        breaker.record(1, Scope::Walk, false, now);
        assert!(breaker.entries.is_empty());
        for pid in 1..=(MAX_TRACKED_PROCESSES as u32 * 2) {
            breaker.record(pid, Scope::Walk, true, now);
        }
        assert!(breaker.entries.len() <= MAX_TRACKED_PROCESSES);
    }
}
