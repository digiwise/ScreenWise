//! UIA circuit breaker (owner policy 2026-10-16).
//!
//! A UIA request is served on the target application's UI thread, and a
//! client-side timeout does not stop that work. After a costly outcome — a
//! failure (timeout, node/depth/text limit or provider failure) or a success
//! that took longer than the walk's cost budget — the target receives no UIA
//! calls of that scope for an increasing period — 30 s, 60 s, 120 s ... capped
//! at 10 min — and three consecutive cheap successes reset it. A slow success
//! costs the target exactly what a slow failure does (owner decision
//! 2026-10-19: a window with an enormous tree answered every walk in one to
//! two seconds, so its backoff was reset by three "healthy" successes and the
//! application stalled every 30 s indefinitely).
//!
//! `Walk` (capture-time tree walks) and `Text` (only their optional text phase,
//! so a slow text phase degrades a window to OCR text without withholding its
//! frames) are keyed by window: process id plus window handle, so one large
//! window does not stop walks of the same application's small windows. The
//! window's area is recorded at each failure; when the window has shrunk by at
//! least a quarter (a list collapsed back into a bar) its backoff is released
//! but its failure count is kept, so a window that regrows and fails again
//! continues the doubling instead of starting over.
//!
//! `Probe` (the cheap keyboard focus probe) is keyed by process: a large tree
//! never disables keystroke privacy checks, and a probe is instead skipped
//! while a walk's UIA requests to the same process have run longer than a
//! probe's own provider timeout (it would only queue behind them on the
//! shared UI thread). A walk lease reserved before a screenshot does not
//! count until the walk's UIA phase starts.
//! A probe costs its target one bounded provider request, not a walk, and
//! every skipped probe withholds the user's typing, so its timeouts back off
//! for 1 s, 2 s, 4 s ... capped at 30 s (2026-10-23) instead of the walk
//! schedule; probes whose provider calls all answer count as cheap successes.
//! Keys are process ids and window handles only; the tables are bounded.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const BASE_BACKOFF: Duration = Duration::from_secs(30);
const MAX_BACKOFF: Duration = Duration::from_secs(600);
const PROBE_BASE_BACKOFF: Duration = Duration::from_secs(1);
const PROBE_MAX_BACKOFF: Duration = Duration::from_secs(30);
const RESET_AFTER_SUCCESSES: u32 = 3;
const MAX_TRACKED_TARGETS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Walk,
    Text,
    Probe,
}

/// What one UIA call cost its target.
///
/// `CostlySuccess` returned a usable tree but blocked the target's UI thread
/// for at least the walk's cost budget. It extends the backoff exactly as a
/// failure does — the point of the breaker is the cost, not the verdict — but
/// stays a distinct value so diagnostics never report it as an accessibility
/// failure: its frame was admitted with full accessibility evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// Returned within the cost budget: counts towards resetting the gate.
    Success,
    /// Returned a tree, but slowly enough to hurt the target application.
    CostlySuccess,
    /// A costly failure: no usable tree, and the target paid for the attempt.
    Failure,
}

impl Outcome {
    /// Whether this outcome extends the target's backoff.
    fn costly(self) -> bool {
        !matches!(self, Outcome::Success)
    }
}

/// Process-level targets (`Probe`) use window handle 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Target {
    pid: u32,
    hwnd: isize,
}

/// Backoff after `failures` consecutive costly outcomes (>= 1).
pub(crate) fn backoff(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(5);
    BASE_BACKOFF.saturating_mul(1 << doublings).min(MAX_BACKOFF)
}

/// Backoff of a scope after `failures` consecutive costly outcomes (>= 1).
fn scope_backoff(scope: Scope, failures: u32) -> Duration {
    match scope {
        Scope::Walk | Scope::Text => backoff(failures),
        Scope::Probe => {
            let doublings = failures.saturating_sub(1).min(5);
            PROBE_BASE_BACKOFF
                .saturating_mul(1 << doublings)
                .min(PROBE_MAX_BACKOFF)
        }
    }
}

/// Whether `area` is at most three quarters of `failed_at`.
fn shrunk_enough(area: u64, failed_at: u64) -> bool {
    area.saturating_mul(4) <= failed_at.saturating_mul(3)
}

#[derive(Clone, Copy, Debug, Default)]
struct Gate {
    /// Consecutive costly outcomes: failures and slow successes alike.
    failures: u32,
    successes: u32,
    open_until: Option<Instant>,
}

impl Gate {
    fn allows(&self, now: Instant) -> bool {
        self.open_until.is_none_or(|until| now >= until)
    }
    /// Extend the backoff after a costly outcome: a failure or a slow success.
    fn back_off(&mut self, scope: Scope, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        self.successes = 0;
        self.open_until = Some(now + scope_backoff(scope, self.failures));
    }
    /// Count a cheap success; only these reset the failure streak.
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

#[derive(Debug, Default)]
struct Entry {
    walk: Gate,
    text: Gate,
    probe: Gate,
    /// Window area at its last costly outcome (window targets only).
    failure_area: Option<u64>,
    last_seen: Option<Instant>,
}

impl Entry {
    fn gate(&self, scope: Scope) -> &Gate {
        match scope {
            Scope::Walk => &self.walk,
            Scope::Text => &self.text,
            Scope::Probe => &self.probe,
        }
    }
    fn gate_mut(&mut self, scope: Scope) -> &mut Gate {
        match scope {
            Scope::Walk => &mut self.walk,
            Scope::Text => &mut self.text,
            Scope::Probe => &mut self.probe,
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
    entries: HashMap<Target, Entry>,
}

impl Breaker {
    /// Whether `scope` calls to the target may be made now. A window that has
    /// shrunk enough since its last failure has its backoff released first.
    fn allows(&mut self, target: Target, scope: Scope, area: Option<u64>, now: Instant) -> bool {
        let Some(entry) = self.entries.get_mut(&target) else {
            return true;
        };
        if let (Some(area), Some(failed_at)) = (area, entry.failure_area) {
            if shrunk_enough(area, failed_at) {
                entry.walk.open_until = None;
                entry.text.open_until = None;
                entry.failure_area = None;
                SHRINK_RELEASES.fetch_add(1, Ordering::Relaxed);
            }
        }
        entry.gate(scope).allows(now)
    }

    fn record(
        &mut self,
        target: Target,
        scope: Scope,
        outcome: Outcome,
        area: Option<u64>,
        now: Instant,
    ) {
        if !outcome.costly() && !self.entries.contains_key(&target) {
            return;
        }
        self.evict(now);
        let entry = self.entries.entry(target).or_default();
        entry.last_seen = Some(now);
        if outcome.costly() {
            entry.gate_mut(scope).back_off(scope, now);
            if area.is_some() {
                entry.failure_area = area;
            }
            OPENINGS.fetch_add(1, Ordering::Relaxed);
            if outcome == Outcome::CostlySuccess {
                COSTLY_SUCCESSES.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            entry.gate_mut(scope).succeed();
        }
        if entry.idle(now) {
            self.entries.remove(&target);
        }
    }

    fn evict(&mut self, now: Instant) {
        if self.entries.len() < MAX_TRACKED_TARGETS {
            return;
        }
        self.entries.retain(|_, entry| !entry.all_allow(now));
        while self.entries.len() >= MAX_TRACKED_TARGETS {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_seen)
                .map(|(target, _)| *target)
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
        if self.walks.len() < MAX_TRACKED_TARGETS || self.walks.contains_key(&pid) {
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
    /// Start a walk only when none to `pid` is running (an exclusive lease).
    fn try_start(&mut self, pid: u32, now: Instant) -> bool {
        if self
            .walks
            .get(&pid)
            .is_some_and(|starts| !starts.is_empty())
        {
            return false;
        }
        self.start(pid, now);
        true
    }
    fn busy(&self, pid: u32) -> bool {
        self.walks
            .get(&pid)
            .is_some_and(|starts| !starts.is_empty())
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

/// One walk per target process: returns the lease, or `None` while another
/// walk of `pid` is running. Concurrent requests to one provider made it
/// slower and coincided with a UI Automation heap failure (SW-R20). An
/// unknown process (`pid == 0`) is never leased exclusively.
pub(crate) fn try_walk_lease(pid: u32) -> Option<WalkInFlight> {
    let started = Instant::now();
    if pid == 0 {
        return Some(walk_in_flight(pid));
    }
    with_in_flight(|in_flight| in_flight.try_start(pid, started))
        .then_some(WalkInFlight { pid, started })
}

/// A walk lease reserved before the screenshot and held through the walk, so
/// a capture never takes pixels it must then discard because its walk target
/// is busy. The walk itself uses it via `TreeWalkerConfig::windows_walk_lease_pid`.
pub struct WalkLease {
    _in_flight: WalkInFlight,
    pid: u32,
}

impl WalkLease {
    pub fn pid(&self) -> u32 {
        self.pid
    }
}

/// Reserve the one walk of `pid`; `None` while another walk of it runs.
pub fn reserve_walk_target(pid: u32) -> Option<WalkLease> {
    try_walk_lease(pid).map(|in_flight| WalkLease {
        _in_flight: in_flight,
        pid,
    })
}

/// Whether a walk of `pid` is running now.
pub fn walk_target_busy(pid: u32) -> bool {
    pid != 0 && with_in_flight(|in_flight| in_flight.busy(pid))
}

/// UIA requests of walks actually running (not leases held across a
/// screenshot): only these occupy the target's UI thread ahead of a probe.
static ACTIVE: OnceLock<Mutex<InFlight>> = OnceLock::new();

fn with_active<T>(f: impl FnOnce(&mut InFlight) -> T) -> T {
    let mut guard = ACTIVE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

/// Marks a walk's UIA phase to `pid` active until dropped.
pub(crate) struct ActiveWalk {
    pid: u32,
    started: Instant,
}

impl Drop for ActiveWalk {
    fn drop(&mut self) {
        with_active(|active| active.finish(self.pid, self.started));
    }
}

/// Start of a walk's UIA requests to `pid` (after any screenshot taken under
/// a lease reserved earlier).
pub(crate) fn walk_uia_active(pid: u32) -> ActiveWalk {
    let started = Instant::now();
    with_active(|active| active.start(pid, started));
    ActiveWalk { pid, started }
}

/// Whether a walk's UIA requests to `pid` have run for longer than `age`; a
/// lease held while a screenshot is taken does not count.
pub(crate) fn long_walk_in_flight(pid: u32, age: Duration) -> bool {
    pid != 0 && with_active(|active| active.longer_than(pid, age, Instant::now()))
}

static BREAKER: OnceLock<Mutex<Breaker>> = OnceLock::new();
static OPENINGS: AtomicU64 = AtomicU64::new(0);
/// Openings caused by a slow success rather than a failure.
static COSTLY_SUCCESSES: AtomicU64 = AtomicU64::new(0);
static SHRINK_RELEASES: AtomicU64 = AtomicU64::new(0);
static WALK_SKIPS: AtomicU64 = AtomicU64::new(0);
static TEXT_SKIPS: AtomicU64 = AtomicU64::new(0);
static PROBE_SKIPS: AtomicU64 = AtomicU64::new(0);
static EXCLUSION_SKIPS: AtomicU64 = AtomicU64::new(0);

fn with_breaker<T>(f: impl FnOnce(&mut Breaker) -> T) -> T {
    let mut guard = BREAKER
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

/// Whether `Walk`/`Text` calls may be made to this window now. `area` is its
/// current native window area (pixels), when known. Pid 0 is never tracked
/// (callers already fail unknown targets closed).
pub(crate) fn allows_window(pid: u32, hwnd: isize, area: Option<u64>, scope: Scope) -> bool {
    pid == 0
        || with_breaker(|breaker| breaker.allows(Target { pid, hwnd }, scope, area, Instant::now()))
}

pub(crate) fn record_window(
    pid: u32,
    hwnd: isize,
    area: Option<u64>,
    scope: Scope,
    outcome: Outcome,
) {
    if pid != 0 {
        with_breaker(|breaker| {
            breaker.record(Target { pid, hwnd }, scope, outcome, area, Instant::now())
        });
    }
}

/// Whether keyboard probes may call the provider of process `pid` now.
pub(crate) fn allows_probe(pid: u32) -> bool {
    pid == 0
        || with_breaker(|breaker| {
            breaker.allows(Target { pid, hwnd: 0 }, Scope::Probe, None, Instant::now())
        })
}

/// Probes either answer cheaply or fail; they have no slow-success case.
pub(crate) fn record_probe(pid: u32, costly_failure: bool) {
    if pid != 0 {
        let outcome = if costly_failure {
            Outcome::Failure
        } else {
            Outcome::Success
        };
        with_breaker(|breaker| {
            breaker.record(
                Target { pid, hwnd: 0 },
                Scope::Probe,
                outcome,
                None,
                Instant::now(),
            )
        });
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
/// A UIA call was not made because its target is excluded from capture.
pub(crate) fn note_exclusion_skip() {
    EXCLUSION_SKIPS.fetch_add(1, Ordering::Relaxed);
}

/// Numeric counters only (no process ids or handles).
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
        "breaker_costly_successes": COSTLY_SUCCESSES.load(Ordering::Relaxed),
        "breaker_shrink_releases": SHRINK_RELEASES.load(Ordering::Relaxed),
        "breaker_walk_skips": WALK_SKIPS.load(Ordering::Relaxed),
        "breaker_text_skips": TEXT_SKIPS.load(Ordering::Relaxed),
        "breaker_probe_skips": PROBE_SKIPS.load(Ordering::Relaxed),
        "uia_exclusion_skips": EXCLUSION_SKIPS.load(Ordering::Relaxed),
        "breaker_open_walk_windows": open_walk,
        "breaker_open_text_windows": open_text,
        "breaker_open_probe_processes": open_probe,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(pid: u32, hwnd: isize) -> Target {
        Target { pid, hwnd }
    }

    #[test]
    fn backoff_doubles_from_thirty_seconds_and_caps_at_ten_minutes() {
        let secs: Vec<u64> = (1..=8).map(|n| backoff(n).as_secs()).collect();
        assert_eq!(secs, vec![30, 60, 120, 240, 480, 600, 600, 600]);
        let walk: Vec<u64> = (1..=8)
            .map(|n| scope_backoff(Scope::Walk, n).as_secs())
            .collect();
        assert_eq!(walk, secs);
    }

    #[test]
    fn probe_timeouts_back_off_briefly_and_answered_probes_reset() {
        let secs: Vec<u64> = (1..=8)
            .map(|n| scope_backoff(Scope::Probe, n).as_secs())
            .collect();
        assert_eq!(secs, vec![1, 2, 4, 8, 16, 30, 30, 30]);
        let mut breaker = Breaker::default();
        let process = window(40, 0);
        let t0 = Instant::now();
        breaker.record(process, Scope::Probe, Outcome::Failure, None, t0);
        assert!(!breaker.allows(process, Scope::Probe, None, t0 + Duration::from_millis(999)));
        assert!(breaker.allows(process, Scope::Probe, None, t0 + Duration::from_secs(1)));
        let t1 = t0 + Duration::from_secs(1);
        breaker.record(process, Scope::Probe, Outcome::Failure, None, t1);
        assert!(!breaker.allows(
            process,
            Scope::Probe,
            None,
            t1 + Duration::from_millis(1_999)
        ));
        let t2 = t1 + Duration::from_secs(2);
        for _ in 0..RESET_AFTER_SUCCESSES {
            breaker.record(process, Scope::Probe, Outcome::Success, None, t2);
        }
        assert!(
            breaker.entries.is_empty(),
            "answered probes forget the streak"
        );
        breaker.record(process, Scope::Probe, Outcome::Failure, None, t2);
        assert!(breaker.allows(process, Scope::Probe, None, t2 + Duration::from_secs(1)));
    }

    #[test]
    fn costly_failures_back_off_one_window_and_three_successes_reset() {
        let mut breaker = Breaker::default();
        let big = window(10, 0x100);
        let t0 = Instant::now();
        breaker.record(big, Scope::Walk, Outcome::Failure, Some(1_000), t0);
        assert!(!breaker.allows(big, Scope::Walk, None, t0 + Duration::from_secs(29)));
        assert!(
            breaker.allows(big, Scope::Text, None, t0),
            "scopes are independent"
        );
        assert!(
            breaker.allows(window(10, 0x200), Scope::Walk, None, t0),
            "the same application's other windows are unaffected"
        );
        assert!(
            breaker.allows(window(10, 0), Scope::Probe, None, t0),
            "walk failures never block probes"
        );
        let t1 = t0 + Duration::from_secs(30);
        assert!(breaker.allows(big, Scope::Walk, None, t1));
        breaker.record(big, Scope::Walk, Outcome::Failure, Some(1_000), t1);
        assert!(!breaker.allows(big, Scope::Walk, None, t1 + Duration::from_secs(59)));
        let t2 = t1 + Duration::from_secs(60);
        // Two successes do not forget the failure streak...
        breaker.record(big, Scope::Walk, Outcome::Success, None, t2);
        breaker.record(big, Scope::Walk, Outcome::Success, None, t2);
        breaker.record(big, Scope::Walk, Outcome::Failure, Some(1_000), t2);
        assert!(!breaker.allows(big, Scope::Walk, None, t2 + Duration::from_secs(119)));
        // ...three in a row do.
        let t3 = t2 + Duration::from_secs(120);
        for _ in 0..3 {
            breaker.record(big, Scope::Walk, Outcome::Success, None, t3);
        }
        breaker.record(big, Scope::Walk, Outcome::Failure, Some(1_000), t3);
        assert!(breaker.allows(big, Scope::Walk, None, t3 + Duration::from_secs(30)));
    }

    /// The bug this fixes: a window whose tree is so large that every walk
    /// takes seconds answered successfully, so three of those slow walks reset
    /// the gate and the target was walked again every 30 s forever.
    #[test]
    fn slow_successes_keep_doubling_the_backoff_instead_of_resetting_it() {
        let mut breaker = Breaker::default();
        let heavy = window(30, 0x400);
        let t0 = Instant::now();
        let costly_before = COSTLY_SUCCESSES.load(Ordering::Relaxed);
        breaker.record(heavy, Scope::Walk, Outcome::CostlySuccess, Some(1_000), t0);
        // A slow success opens the gate exactly as a failure does.
        assert!(!breaker.allows(heavy, Scope::Walk, None, t0 + Duration::from_secs(29)));
        assert!(breaker.allows(heavy, Scope::Walk, None, t0 + Duration::from_secs(30)));
        // Three more never reset it: each doubles the period.
        let mut at = t0;
        for expected in [60u64, 120, 240] {
            at += Duration::from_secs(expected / 2);
            breaker.record(heavy, Scope::Walk, Outcome::CostlySuccess, Some(1_000), at);
            assert!(!breaker.allows(
                heavy,
                Scope::Walk,
                None,
                at + Duration::from_secs(expected - 1)
            ));
            assert!(breaker.allows(heavy, Scope::Walk, None, at + Duration::from_secs(expected)));
        }
        // Only cheap successes recover: three of them reset the streak, so the
        // next costly outcome starts again at the base period.
        for _ in 0..RESET_AFTER_SUCCESSES {
            breaker.record(heavy, Scope::Walk, Outcome::Success, None, at);
        }
        breaker.record(heavy, Scope::Walk, Outcome::CostlySuccess, Some(1_000), at);
        assert!(!breaker.allows(heavy, Scope::Walk, None, at + Duration::from_secs(29)));
        assert!(breaker.allows(heavy, Scope::Walk, None, at + Duration::from_secs(30)));
        // A slow success is still a success for the frame, so it is counted
        // separately from a hard failure in the diagnostics.
        assert!(
            COSTLY_SUCCESSES.load(Ordering::Relaxed) >= costly_before + 5,
            "each slow success is counted as one"
        );
    }

    #[test]
    fn a_slow_success_mixed_into_a_streak_does_not_reset_it() {
        let mut breaker = Breaker::default();
        let heavy = window(31, 0x500);
        let t0 = Instant::now();
        breaker.record(heavy, Scope::Walk, Outcome::Failure, Some(1_000), t0);
        // Two cheap successes and one slow one: the slow walk breaks the run
        // of successes and extends the backoff to the second step.
        breaker.record(heavy, Scope::Walk, Outcome::Success, None, t0);
        breaker.record(heavy, Scope::Walk, Outcome::Success, None, t0);
        breaker.record(heavy, Scope::Walk, Outcome::CostlySuccess, Some(1_000), t0);
        assert!(!breaker.allows(heavy, Scope::Walk, None, t0 + Duration::from_secs(59)));
        assert!(breaker.allows(heavy, Scope::Walk, None, t0 + Duration::from_secs(60)));
        // A third cheap success after it does not inherit the earlier two.
        let t1 = t0 + Duration::from_secs(60);
        breaker.record(heavy, Scope::Walk, Outcome::Success, None, t1);
        breaker.record(heavy, Scope::Walk, Outcome::Failure, Some(1_000), t1);
        assert!(!breaker.allows(heavy, Scope::Walk, None, t1 + Duration::from_secs(119)));
    }

    #[test]
    fn shrinking_releases_the_backoff_but_regrowth_keeps_doubling() {
        let mut breaker = Breaker::default();
        let list = window(20, 0x300);
        let t0 = Instant::now();
        breaker.record(list, Scope::Walk, Outcome::Failure, Some(1_000), t0);
        breaker.record(list, Scope::Text, Outcome::Failure, Some(1_000), t0);
        // Shrinking by less than a quarter keeps it backed off.
        assert!(!breaker.allows(list, Scope::Walk, Some(751), t0));
        // Collapsed to the bar: released at once, both scopes.
        assert!(breaker.allows(list, Scope::Walk, Some(750), t0));
        assert!(breaker.allows(list, Scope::Text, Some(750), t0));
        // Expanded again and failing again: the second failure's 60 s applies.
        breaker.record(list, Scope::Walk, Outcome::Failure, Some(1_000), t0);
        assert!(!breaker.allows(list, Scope::Walk, None, t0 + Duration::from_secs(59)));
        assert!(breaker.allows(list, Scope::Walk, None, t0 + Duration::from_secs(60)));
        // Unknown size never releases.
        breaker.record(list, Scope::Walk, Outcome::Failure, Some(1_000), t0);
        assert!(!breaker.allows(list, Scope::Walk, None, t0));
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
        assert!(in_flight.try_start(9, t0));
        assert!(
            !in_flight.try_start(9, t0),
            "a second walk of the same process waits"
        );
        assert!(
            in_flight.try_start(10, t0),
            "other processes are unaffected"
        );
        assert!(in_flight.busy(9));
        in_flight.finish(9, t0);
        assert!(!in_flight.busy(9));
        assert!(in_flight.try_start(9, t0));
        let lease = reserve_walk_target(4343).expect("free target");
        assert_eq!(lease.pid(), 4343);
        assert!(walk_target_busy(4343));
        assert!(
            reserve_walk_target(4343).is_none(),
            "reserved until dropped"
        );
        drop(lease);
        assert!(!walk_target_busy(4343));
        // A lease alone (screenshot phase) never makes probes wait.
        let guard = walk_in_flight(4242);
        std::thread::sleep(Duration::from_millis(5));
        assert!(!long_walk_in_flight(4242, Duration::from_millis(1)));
        let active = walk_uia_active(4242);
        std::thread::sleep(Duration::from_millis(5));
        assert!(long_walk_in_flight(4242, Duration::from_millis(1)));
        drop(active);
        assert!(!long_walk_in_flight(4242, Duration::ZERO));
        drop(guard);
    }

    #[test]
    fn healthy_targets_are_not_tracked_and_the_table_is_bounded() {
        let mut breaker = Breaker::default();
        let now = Instant::now();
        breaker.record(window(1, 1), Scope::Walk, Outcome::Success, None, now);
        assert!(breaker.entries.is_empty());
        // A slow success of an untracked window is tracked at once.
        breaker.record(window(2, 1), Scope::Walk, Outcome::CostlySuccess, None, now);
        assert_eq!(breaker.entries.len(), 1);
        breaker.entries.clear();
        for pid in 1..=(MAX_TRACKED_TARGETS as u32 * 2) {
            breaker.record(window(pid, 1), Scope::Walk, Outcome::Failure, Some(10), now);
        }
        assert!(breaker.entries.len() <= MAX_TRACKED_TARGETS);
    }
}
