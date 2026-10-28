//! Keystroke drop alerts: an owner-approved, temporary diagnostic
//! (2026-10-28), off unless the process is started with
//! `--keystroke-drop-alerts`.
//!
//! When recordable keystrokes are lost (the keystroke accounting's own
//! classification: an unintended gate denial, or a lost-after-admission
//! cause for which `LostCause::alerts` holds) in a burst of at least the
//! threshold, a desktop notification shows the local time, the
//! application's executable basename, the number of lost keys, the cause in
//! plain words, the focus context and, only if asked for, the first lost
//! keys. Never for password fields, excluded or protected windows, privacy
//! pauses or typing that was undone.
//!
//! The lost keys exist only in memory on their way to the notification text
//! and are dropped once it is shown or suppressed; they never reach the app
//! log, the sensitive diagnostics or any file. The sensitive diagnostics get
//! one content-free record per alert (fixed codes, counts, basename).
//! Producers never wait: a full hand-off drops the loss and counts it.

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use parking_lot::RwLock;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Turns the alerts on (off by default).
pub const SWITCH_ENABLE: &str = "--keystroke-drop-alerts";
/// Minimum keys in one lost burst (default 3 once alerts are on).
pub const SWITCH_THRESHOLD: &str = "--keystroke-drop-alert-threshold";
/// How many lost keys to show (default 0: none).
pub const SWITCH_KEYS: &str = "--keystroke-drop-alert-keys";

const DEFAULT_THRESHOLD: u32 = 3;
const MAX_KEYS_SHOWN: usize = 32;
/// Gate refusals for one process this close together form one burst.
const BURST_GAP: Duration = Duration::from_secs(10);
/// At most one notification per this interval.
const RATE_LIMIT: Duration = Duration::from_secs(60);
const HANDOFF_CAPACITY: usize = 256;
const MAX_OPEN_BURSTS: usize = 16;

/// The startup switches (process lifetime; nothing is persisted).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    pub enabled: bool,
    pub threshold: u32,
    pub keys_shown: usize,
}

/// Read the switches from the process arguments (`--name=value` or
/// `--name value`); anything malformed keeps the default.
pub fn parse_args<S: AsRef<str>>(args: &[S]) -> Switches {
    let mut switches = Switches {
        enabled: false,
        threshold: DEFAULT_THRESHOLD,
        keys_shown: 0,
    };
    let value = |index: usize, name: &str| -> Option<String> {
        let arg = args[index].as_ref();
        if let Some(rest) = arg.strip_prefix(name) {
            if let Some(value) = rest.strip_prefix('=') {
                return Some(value.to_string());
            }
            if rest.is_empty() {
                return args.get(index + 1).map(|next| next.as_ref().to_string());
            }
        }
        None
    };
    for index in 0..args.len() {
        let arg = args[index].as_ref();
        if arg == SWITCH_ENABLE {
            switches.enabled = true;
        }
        if let Some(threshold) = value(index, SWITCH_THRESHOLD).and_then(|v| v.parse::<u32>().ok())
        {
            switches.threshold = threshold.max(1);
        }
        if let Some(keys) = value(index, SWITCH_KEYS).and_then(|v| v.parse::<usize>().ok()) {
            switches.keys_shown = keys.min(MAX_KEYS_SHOWN);
        }
    }
    switches
}

static SWITCHES: OnceLock<Switches> = OnceLock::new();
type Notifier = Box<dyn Fn(&str, &str) -> bool + Send + Sync>;
static NOTIFIER: OnceLock<Notifier> = OnceLock::new();
static SENDER: RwLock<Option<Sender<Loss>>> = RwLock::new(None);
static HANDOFF_DROPS: AtomicU64 = AtomicU64::new(0);

/// Set once at process start (later calls are ignored).
pub fn configure(switches: Switches) {
    let _ = SWITCHES.set(switches);
}

/// The configured switches (all off unless configured).
pub fn switches() -> Switches {
    SWITCHES.get().copied().unwrap_or_default()
}

/// Whether alerts are on for this process.
pub fn enabled() -> bool {
    switches().enabled
}

/// The desktop's notification path: `show(title, body)` returns whether it
/// was shown. Set once.
pub fn set_notifier(show: impl Fn(&str, &str) -> bool + Send + Sync + 'static) {
    let _ = NOTIFIER.set(Box::new(show));
}

fn notify(title: &str, body: &str) -> Option<bool> {
    NOTIFIER.get().map(|show| show(title, body))
}

/// A lost key as shown: a character, or a special key by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyLabel {
    Char(char),
    Named(&'static str),
}

impl KeyLabel {
    /// From a buffered or translated character (special characters by name).
    pub(crate) fn from_char(c: char) -> Self {
        match c {
            '\n' | '\r' => Self::Named("Enter"),
            '\t' => Self::Named("Tab"),
            '\x08' => Self::Named("Backspace"),
            ' ' => Self::Named("Space"),
            c if c.is_control() => Self::Named("Control key"),
            c => Self::Char(c),
        }
    }

    fn text(self) -> String {
        match self {
            Self::Char(c) => c.to_string(),
            Self::Named(name) => format!("[{name}]"),
        }
    }
}

/// Where the loss happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LossKind {
    /// Refused at the privacy gate.
    Refusal,
    /// Buffered text discarded by a recheck.
    Discard,
    /// Lost after admission (queue, worker, database ...).
    AfterAdmission,
}

impl LossKind {
    fn code(self) -> &'static str {
        match self {
            Self::Refusal => "refused_at_gate",
            Self::Discard => "buffer_discarded",
            Self::AfterAdmission => "lost_after_admission",
        }
    }
}

/// Fixed-code context of a loss (no content).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LossContext {
    /// Refusals: time since the foreground changed (fixed bucket code).
    pub(crate) since_switch: Option<&'static str>,
    /// Refusals: whether probing was paused after a timeout.
    pub(crate) breaker_open: Option<bool>,
    /// Discards: where focus moved (fixed relation code, control type).
    pub(crate) focus_relation: Option<&'static str>,
    pub(crate) control_type: Option<&'static str>,
}

/// One loss handed from a producer to the alert worker.
#[derive(Clone, Debug)]
pub(crate) struct Loss {
    pub(crate) at: Instant,
    pub(crate) pid: u32,
    pub(crate) keys: u64,
    /// The accounting's fixed cause code.
    pub(crate) cause: &'static str,
    pub(crate) kind: LossKind,
    /// The native foreground window when known, for the exclusion recheck.
    pub(crate) foreground: Option<isize>,
    pub(crate) context: LossContext,
    /// At most `keys_shown` labels (empty unless asked for).
    pub(crate) labels: Vec<KeyLabel>,
}

/// Producers: hand a loss over without waiting; a full or absent hand-off
/// drops it (and counts the drop when alerts are on).
pub(crate) fn hand_off(loss: Loss) {
    let Some(sender) = SENDER.try_read() else {
        HANDOFF_DROPS.fetch_add(1, Ordering::Relaxed);
        return;
    };
    match sender.as_ref().map(|sender| sender.try_send(loss)) {
        Some(Ok(())) => {}
        Some(Err(TrySendError::Full(_) | TrySendError::Disconnected(_))) | None => {
            HANDOFF_DROPS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Open the hand-off for one alert worker (replacing an earlier one).
pub(crate) fn open_hand_off() -> Receiver<Loss> {
    let (sender, receiver) = bounded(HANDOFF_CAPACITY);
    *SENDER.write() = Some(sender);
    receiver
}

/// Close the hand-off when the alert worker stops.
pub(crate) fn close_hand_off() {
    *SENDER.write() = None;
}

/// After-admission losses from the keystroke accounting (any thread).
/// Buffer discards are handed over by the hook with their focus context.
pub(crate) fn note_lost_after_admission(
    pid: u32,
    keys: u64,
    cause: crate::input_accounting::LostCause,
) {
    if !enabled() || keys == 0 || !cause.alerts() {
        return;
    }
    if cause == crate::input_accounting::LostCause::BufferDiscarded {
        return;
    }
    hand_off(Loss {
        at: Instant::now(),
        pid,
        keys,
        cause: cause.code(),
        kind: LossKind::AfterAdmission,
        foreground: None,
        context: LossContext::default(),
        labels: Vec::new(),
    });
}

/// A burst that reached the threshold, ready for the notification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Alert {
    pub(crate) pid: u32,
    pub(crate) keys: u64,
    pub(crate) cause: &'static str,
    pub(crate) kind: LossKind,
    pub(crate) foreground: Option<isize>,
    pub(crate) context: LossContext,
    pub(crate) labels: Vec<KeyLabel>,
    /// Shown, or suppressed by the rate limit.
    pub(crate) rate_limited: bool,
}

struct Burst {
    pid: u32,
    kind: LossKind,
    last_at: Instant,
    keys: u64,
    cause: &'static str,
    foreground: Option<isize>,
    context: LossContext,
    labels: Vec<KeyLabel>,
    alerted: bool,
}

/// Burst grouping, threshold and rate limit (pure; bounded state).
pub(crate) struct AlertCore {
    switches: Switches,
    bursts: Vec<Burst>,
    last_shown: Option<Instant>,
}

impl AlertCore {
    pub(crate) fn new(switches: Switches) -> Self {
        Self {
            switches,
            bursts: Vec::new(),
            last_shown: None,
        }
    }

    /// Forget refusal bursts older than the burst gap (and their keys).
    pub(crate) fn expire(&mut self, now: Instant) {
        self.bursts
            .retain(|burst| now.saturating_duration_since(burst.last_at) <= BURST_GAP);
    }

    /// Add one loss; returns the alert when its burst reaches the threshold.
    pub(crate) fn observe(&mut self, loss: Loss) -> Option<Alert> {
        if !self.switches.enabled {
            return None;
        }
        self.expire(loss.at);
        let keys_shown = self.switches.keys_shown;
        let index = (loss.kind == LossKind::Refusal)
            .then(|| {
                self.bursts.iter().position(|burst| {
                    burst.kind == LossKind::Refusal
                        && burst.pid == loss.pid
                        && loss.at.saturating_duration_since(burst.last_at) <= BURST_GAP
                })
            })
            .flatten();
        let index = match index {
            Some(index) => {
                let burst = &mut self.bursts[index];
                burst.last_at = loss.at;
                burst.keys += loss.keys;
                burst.cause = loss.cause;
                burst.foreground = loss.foreground.or(burst.foreground);
                burst.context = loss.context;
                if !burst.alerted {
                    let room = keys_shown.saturating_sub(burst.labels.len());
                    burst.labels.extend(loss.labels.into_iter().take(room));
                }
                index
            }
            None => {
                if self.bursts.len() >= MAX_OPEN_BURSTS {
                    self.bursts.remove(0);
                }
                let mut labels = loss.labels;
                labels.truncate(keys_shown);
                self.bursts.push(Burst {
                    pid: loss.pid,
                    kind: loss.kind,
                    last_at: loss.at,
                    keys: loss.keys,
                    cause: loss.cause,
                    foreground: loss.foreground,
                    context: loss.context,
                    labels,
                    alerted: false,
                });
                self.bursts.len() - 1
            }
        };
        let burst = &mut self.bursts[index];
        if burst.alerted || burst.keys < u64::from(self.switches.threshold) {
            if burst.kind != LossKind::Refusal {
                // A discard or after-admission loss is one burst on its own.
                self.bursts.remove(index);
            }
            return None;
        }
        burst.alerted = true;
        let labels = std::mem::take(&mut burst.labels);
        let alert = Alert {
            pid: burst.pid,
            keys: burst.keys,
            cause: burst.cause,
            kind: burst.kind,
            foreground: burst.foreground,
            context: burst.context,
            labels,
            rate_limited: self
                .last_shown
                .is_some_and(|at| loss.at.saturating_duration_since(at) < RATE_LIMIT),
        };
        if burst.kind != LossKind::Refusal {
            self.bursts.remove(index);
        }
        if !alert.rate_limited {
            self.last_shown = Some(loss.at);
        }
        Some(alert)
    }
}

/// The cause in plain words (fixed labels).
pub(crate) fn cause_label(alert: &Alert) -> String {
    match alert.cause {
        "decision_unavailable" if alert.context.breaker_open == Some(true) => {
            "No privacy decision yet (checks paused after a timeout)".to_string()
        }
        "decision_unavailable" => "No privacy decision yet".to_string(),
        "generation_changed" => "Privacy decision changed while typing".to_string(),
        "decision_stale" => "Privacy decision too old".to_string(),
        "focus_mismatch" => "Privacy decision was for another field".to_string(),
        "worker_lock_contended" => "Privacy check busy".to_string(),
        "buffer_discarded" => match alert.context.focus_relation {
            Some("other_process") | Some("webview2_child") => {
                "Typed text discarded: focus moved to another app".to_string()
            }
            Some("foreground") => {
                "Typed text discarded: focus moved within the same app".to_string()
            }
            _ => "Typed text discarded: focus target not known".to_string(),
        },
        "queue_full" => "Input queue was full".to_string(),
        "content_worker" => "Input worker refused the text".to_string(),
        "database_failure" => "Database write failed".to_string(),
        "recorder_filtered" => "Not recorded by the recorder's configuration".to_string(),
        "no_effect" => "Key had no recordable effect".to_string(),
        other => format!("Lost ({other})"),
    }
}

fn switch_label(code: &str) -> &'static str {
    match code {
        "lt1s" => "under 1 s",
        "lt10s" => "under 10 s",
        "ge10s" => "10 s or more",
        "unsampled" => "just now (not yet checked)",
        _ => "unknown",
    }
}

/// Notification title and body. `local_time` and `exe` are supplied by the
/// caller; the keys appear only here.
pub(crate) fn alert_text(alert: &Alert, local_time: &str, exe: &str) -> (String, String) {
    let mut lines = vec![
        format!("{local_time} · {exe} · {} key(s) lost", alert.keys),
        cause_label(alert),
    ];
    match alert.kind {
        LossKind::Refusal => {
            if let Some(code) = alert.context.since_switch {
                lines.push(format!("Last app switch: {}", switch_label(code)));
            }
        }
        LossKind::Discard => {
            if let Some(control) = alert.context.control_type {
                lines.push(format!("Focus moved to: {control}"));
            }
        }
        LossKind::AfterAdmission => {}
    }
    if !alert.labels.is_empty() {
        let keys: Vec<String> = alert.labels.iter().map(|label| label.text()).collect();
        lines.push(format!("First keys: {}", keys.join(" ")));
    }
    ("ScreenWise: keystrokes lost".to_string(), lines.join("\n"))
}

/// The content-free sensitive-diagnostics record of one alert: no keys.
pub(crate) fn sink_record(alert: &Alert, exe: Option<&str>, outcome: &'static str) -> Value {
    json!({
        "outcome": outcome,
        "kind": alert.kind.code(),
        "cause": alert.cause,
        "keys": alert.keys,
        "exe": exe,
        "since_foreground_change": alert.context.since_switch,
        "breaker_open": alert.context.breaker_open,
        "focus_relation": alert.context.focus_relation,
        "control_type": alert.context.control_type,
        "keys_shown": alert.labels.len(),
        "handoff_dropped_total": HANDOFF_DROPS.load(Ordering::Relaxed),
    })
}

/// Show (or record as rate limited) one alert, then drop its keys.
pub(crate) fn deliver(alert: Alert, exe: Option<String>) {
    let exe_text = exe.as_deref().unwrap_or("unknown app");
    let outcome = if alert.rate_limited {
        "rate_limited"
    } else {
        let local_time = chrono::Local::now().format("%H:%M:%S").to_string();
        let (title, body) = alert_text(&alert, &local_time, exe_text);
        match notify(&title, &body) {
            Some(true) => "shown",
            Some(false) => "notification_failed",
            None => "notifier_unavailable",
        }
    };
    let record = sink_record(&alert, exe.as_deref(), outcome);
    drop(alert);
    screenpipe_config::sensitive_debug::record("keystroke_drop_alert", || record);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loss(at: Instant, pid: u32, keys: u64, kind: LossKind, labels: &[char]) -> Loss {
        Loss {
            at,
            pid,
            keys,
            cause: match kind {
                LossKind::Refusal => "decision_unavailable",
                LossKind::Discard => "buffer_discarded",
                LossKind::AfterAdmission => "queue_full",
            },
            kind,
            foreground: Some(1),
            context: LossContext::default(),
            labels: labels.iter().copied().map(KeyLabel::from_char).collect(),
        }
    }

    fn on(threshold: u32, keys_shown: usize) -> Switches {
        Switches {
            enabled: true,
            threshold,
            keys_shown,
        }
    }

    #[test]
    fn switches_are_off_by_default_and_parsed_only_from_the_three_names() {
        assert_eq!(
            parse_args::<&str>(&[]),
            Switches {
                enabled: false,
                threshold: 3,
                keys_shown: 0
            }
        );
        assert!(!parse_args(&["app.exe", "--keystroke-drop-alert-keys=1"]).enabled);
        let s = parse_args(&[
            "app.exe",
            SWITCH_ENABLE,
            "--keystroke-drop-alert-threshold",
            "5",
            "--keystroke-drop-alert-keys=1",
        ]);
        assert_eq!(
            s,
            Switches {
                enabled: true,
                threshold: 5,
                keys_shown: 1
            }
        );
        let s = parse_args(&[
            SWITCH_ENABLE,
            "--keystroke-drop-alert-threshold=x",
            "--keystroke-drop-alert-keys=999",
        ]);
        assert_eq!((s.threshold, s.keys_shown), (3, MAX_KEYS_SHOWN));
        assert_eq!(
            parse_args(&[SWITCH_ENABLE, "--keystroke-drop-alert-threshold=0"]).threshold,
            1
        );
    }

    #[test]
    fn switched_off_nothing_is_kept_or_alerted() {
        let mut core = AlertCore::new(Switches::default());
        let now = Instant::now();
        assert!(core
            .observe(loss(now, 1, 9, LossKind::Discard, &['a']))
            .is_none());
        assert!(core.bursts.is_empty());
        // Unconfigured process: off, and producers return before any work.
        assert!(!enabled());
        note_lost_after_admission(1, 5, crate::input_accounting::LostCause::QueueFull);
        assert!(SENDER.read().is_none());
    }

    #[test]
    fn refusals_group_per_process_within_ten_seconds_and_alert_at_the_threshold() {
        let mut core = AlertCore::new(on(3, 2));
        let t = Instant::now();
        assert!(core
            .observe(loss(t, 7, 1, LossKind::Refusal, &['h']))
            .is_none());
        // Another process does not join the burst.
        assert!(core
            .observe(loss(t, 8, 1, LossKind::Refusal, &['x']))
            .is_none());
        assert!(core
            .observe(loss(
                t + Duration::from_secs(4),
                7,
                1,
                LossKind::Refusal,
                &['i']
            ))
            .is_none());
        let alert = core
            .observe(loss(
                t + Duration::from_secs(9),
                7,
                1,
                LossKind::Refusal,
                &['\n'],
            ))
            .unwrap();
        assert_eq!((alert.pid, alert.keys, alert.rate_limited), (7, 3, false));
        assert_eq!(alert.labels, vec![KeyLabel::Char('h'), KeyLabel::Char('i')]);
        // The same burst never alerts twice.
        assert!(core
            .observe(loss(
                t + Duration::from_secs(10),
                7,
                5,
                LossKind::Refusal,
                &[]
            ))
            .is_none());
        // More than ten seconds after the last refusal: a new burst.
        assert!(core
            .observe(loss(
                t + Duration::from_secs(21),
                7,
                2,
                LossKind::Refusal,
                &[]
            ))
            .is_none());
    }

    #[test]
    fn a_discard_is_one_burst_and_alerts_are_rate_limited_to_one_a_minute() {
        let mut core = AlertCore::new(on(3, 0));
        let t = Instant::now();
        // Below the threshold: dropped, keys and all.
        assert!(core
            .observe(loss(t, 1, 2, LossKind::Discard, &['a', 'b']))
            .is_none());
        assert!(core.bursts.is_empty());
        let first = core
            .observe(loss(t, 1, 3, LossKind::Discard, &['a']))
            .unwrap();
        assert!(
            !first.rate_limited && first.labels.is_empty(),
            "keys_shown 0"
        );
        let second = core
            .observe(loss(
                t + Duration::from_secs(30),
                2,
                4,
                LossKind::AfterAdmission,
                &[],
            ))
            .unwrap();
        assert!(second.rate_limited);
        let third = core
            .observe(loss(
                t + Duration::from_secs(61),
                2,
                4,
                LossKind::Discard,
                &[],
            ))
            .unwrap();
        assert!(!third.rate_limited);
        assert!(core.bursts.is_empty(), "no state kept after discards");
        // Bounded open refusal bursts.
        for pid in 0..40 {
            core.observe(loss(
                t + Duration::from_secs(62),
                pid,
                1,
                LossKind::Refusal,
                &[],
            ));
        }
        assert!(core.bursts.len() <= MAX_OPEN_BURSTS);
        core.expire(t + Duration::from_secs(100));
        assert!(core.bursts.is_empty());
    }

    #[test]
    fn alert_text_uses_fixed_labels_and_names_special_keys() {
        let alert = |cause, kind, context, labels: &[char]| Alert {
            pid: 1,
            keys: 4,
            cause,
            kind,
            foreground: None,
            context,
            labels: labels.iter().copied().map(KeyLabel::from_char).collect(),
            rate_limited: false,
        };
        let refused = alert(
            "decision_unavailable",
            LossKind::Refusal,
            LossContext {
                since_switch: Some("lt10s"),
                breaker_open: Some(true),
                ..Default::default()
            },
            &['a', '\n', '\x08', '\t', ' '],
        );
        let (title, body) = alert_text(&refused, "14:05:23", "app.exe");
        assert_eq!(title, "ScreenWise: keystrokes lost");
        assert_eq!(
            body,
            "14:05:23 · app.exe · 4 key(s) lost\n\
             No privacy decision yet (checks paused after a timeout)\n\
             Last app switch: under 10 s\n\
             First keys: a [Enter] [Backspace] [Tab] [Space]"
        );
        let discarded = alert(
            "buffer_discarded",
            LossKind::Discard,
            LossContext {
                focus_relation: Some("other_process"),
                control_type: Some("Window"),
                ..Default::default()
            },
            &[],
        );
        let (_, body) = alert_text(&discarded, "09:00:00", "app.exe");
        assert!(
            body.contains("focus moved to another app") && body.contains("Focus moved to: Window")
        );
        assert!(!body.contains("First keys"));
        for (cause, label) in [
            (
                "generation_changed",
                "Privacy decision changed while typing",
            ),
            ("decision_stale", "Privacy decision too old"),
        ] {
            assert_eq!(
                cause_label(&alert(
                    cause,
                    LossKind::Refusal,
                    LossContext::default(),
                    &[]
                )),
                label
            );
        }
        let same_app = alert(
            "buffer_discarded",
            LossKind::Discard,
            LossContext {
                focus_relation: Some("foreground"),
                ..Default::default()
            },
            &[],
        );
        assert!(cause_label(&same_app).contains("within the same app"));
        // The sink record never carries the keys.
        let record = sink_record(&refused, Some("app.exe"), "shown");
        let text = record.to_string();
        assert!(!text.contains("[Enter]") && !text.contains("\"a\""));
        assert_eq!(record["keys"], 4);
        assert_eq!(record["keys_shown"], 5);
        assert_eq!(record["cause"], "decision_unavailable");
    }

    #[test]
    fn only_unintended_losses_other_than_undone_typing_and_pauses_alert() {
        use crate::input_accounting::LostCause;
        for cause in LostCause::ALL {
            let expected = !matches!(
                cause,
                LostCause::RecorderExcluded
                    | LostCause::NetZeroEdit
                    | LostCause::RecorderPrivacyPause
            );
            assert_eq!(cause.alerts(), expected, "{}", cause.code());
        }
    }
}
