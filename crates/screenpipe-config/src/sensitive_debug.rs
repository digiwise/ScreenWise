//! Opt-in sensitive decision diagnostics. Never forward to tracing, notices,
//! SQLite, HTTP responses or support-log exports. Contains private metadata.
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
const MIB: u64 = 1024 * 1024;
/// Periodic aggregate summaries: sized for a full day of heavy use.
const SUMMARY_BUDGET_BYTES: u64 = 32 * MIB;
/// The high-volume per-frame capture decisions.
const CAPTURE_DECISION_BUDGET_BYTES: u64 = 8 * MIB;
/// Every other detailed record.
const DETAIL_BUDGET_BYTES: u64 = 8 * MIB;
/// Small records reporting how many records each budget dropped.
const BUDGET_REPORT_BYTES: u64 = 256 * 1024;
const BUDGET_REPORT_INTERVAL: Duration = Duration::from_secs(60);
/// Bound on one session file: the sum of the budgets.
#[cfg(test)]
const MAX_BYTES: u64 = SUMMARY_BUDGET_BYTES
    + CAPTURE_DECISION_BUDGET_BYTES
    + DETAIL_BUDGET_BYTES
    + BUDGET_REPORT_BYTES;
const SUMMARY_CATEGORIES: [&str; 2] = ["input_keystroke_epoch", "input_probe_summary"];
const CAPTURE_DECISION_CATEGORY: &str = "capture_privacy_decision";
const BUDGET_REPORT_CATEGORY: &str = "sensitive_sink_budget";

/// Each record class has its own byte budget in the session file, so one
/// high-volume producer cannot starve another. A full budget drops (and
/// counts) only its own records; the writer keeps going.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Budget {
    Summary,
    CaptureDecision,
    Detail,
}

impl Budget {
    const ALL: [Self; 3] = [Self::Summary, Self::CaptureDecision, Self::Detail];

    fn of(category: &str) -> Self {
        if SUMMARY_CATEGORIES.contains(&category) {
            Self::Summary
        } else if category == CAPTURE_DECISION_CATEGORY {
            Self::CaptureDecision
        } else {
            Self::Detail
        }
    }

    fn bytes(self) -> u64 {
        match self {
            Self::Summary => SUMMARY_BUDGET_BYTES,
            Self::CaptureDecision => CAPTURE_DECISION_BUDGET_BYTES,
            Self::Detail => DETAIL_BUDGET_BYTES,
        }
    }

    /// Fixed label for notices and drop counts.
    pub fn code(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::CaptureDecision => "capture_privacy_decision",
            Self::Detail => "detail",
        }
    }
}
const MAX_RECORD_BYTES: usize = 64 * 1024;
const REPEAT_INTERVAL: Duration = Duration::from_secs(30);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static DISCONNECTED_REPORTED: AtomicBool = AtomicBool::new(false);
static WRITER: OnceLock<Option<mpsc::SyncSender<(&'static str, Value)>>> = OnceLock::new();

/// Fixed lifecycle/loss signals only. Private payloads cannot enter this reporter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensitiveDebugNotice {
    Enabled,
    StorageUnavailable,
    WorkerUnavailable,
    WriteFailedOrLimitReached,
    WriterStopped,
    QueueFull {
        total_dropped: u64,
    },
    /// One record class reached its byte budget; only its further records
    /// are dropped (and counted in the sink).
    BudgetReached {
        budget: &'static str,
    },
}
type NoticeReporter = fn(SensitiveDebugNotice);
#[derive(Default)]
struct NoticeState {
    reporter: Option<NoticeReporter>,
    pending: Vec<SensitiveDebugNotice>,
}
#[derive(Default)]
struct NoticeRelay {
    state: Mutex<NoticeState>,
}
static NOTICES: OnceLock<NoticeRelay> = OnceLock::new();

/// Install before producers start; early notices are replayed from a bounded,
/// content-free buffer. Callbacks run outside the relay lock and private writer.
pub fn set_notice_reporter(reporter: NoticeReporter) {
    NOTICES.get_or_init(NoticeRelay::default).install(reporter);
}
fn report_notice(notice: SensitiveDebugNotice) {
    NOTICES.get_or_init(NoticeRelay::default).report(notice);
}
impl NoticeRelay {
    fn report(&self, notice: SensitiveDebugNotice) {
        let reporter = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.reporter.is_none() {
                // One latest notice per enum variant (six slots maximum).
                state
                    .pending
                    .retain(|old| std::mem::discriminant(old) != std::mem::discriminant(&notice));
                state.pending.push(notice);
            }
            state.reporter
        };
        if let Some(reporter) = reporter {
            reporter(notice);
        }
    }
    fn install(&self, reporter: NoticeReporter) {
        let pending = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if state.reporter.is_some() {
                return;
            }
            state.reporter = Some(reporter);
            std::mem::take(&mut state.pending)
        };
        for notice in pending {
            reporter(notice);
        }
    }
}

/// Whether this process records sensitive diagnostics, so a producer can skip
/// observation work nobody will read. Starts the writer as `record` would.
/// Never a capture-admission input.
pub fn enabled() -> bool {
    WRITER.get_or_init(start_writer).is_some()
}

/// Enable with SCREENWISE_SENSITIVE_DEBUG=1 before process startup. Producers
/// must not include keystrokes, clipboard/password values, images, audio or tokens.
/// Best-effort diagnosis, never a capture-admission requirement.
pub fn record(category: &'static str, details: impl FnOnce() -> Value) {
    if let Some(sender) = WRITER.get_or_init(start_writer) {
        // Never block capture workers behind disk I/O or a full diagnostic queue.
        let details = bounded_details(details());
        match sender.try_send((category, details)) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(_)) => {
                let dropped = DROPPED.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped.is_power_of_two() {
                    report_notice(SensitiveDebugNotice::QueueFull {
                        total_dropped: dropped,
                    });
                }
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                if !DISCONNECTED_REPORTED.swap(true, Ordering::Relaxed) {
                    report_notice(SensitiveDebugNotice::WriterStopped);
                }
            }
        }
    }
}
fn start_writer() -> Option<mpsc::SyncSender<(&'static str, Value)>> {
    if std::env::var_os("SCREENWISE_SENSITIVE_DEBUG").as_deref() != Some(std::ffi::OsStr::new("1"))
    {
        return None;
    }
    let result = (|| -> io::Result<File> {
        let base = dirs::data_local_dir()
            .ok_or_else(|| io::Error::other("local data unavailable"))?
            .join("ScreenWise")
            .join("sensitive-debug");
        std::fs::create_dir_all(&base)?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        // Never reuse a directory/file/reparse point from a previous process.
        // Install the protected ACL atomically, before writing any sensitive data.
        let session = base.join(format!("session-{}-{stamp}", std::process::id()));
        create_private_directory(&session)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(session.join("decisions.jsonl"))
    })();
    let file = match result {
        Ok(file) => file,
        Err(_) => {
            report_notice(SensitiveDebugNotice::StorageUnavailable);
            return None;
        }
    };
    let (sender, receiver) = mpsc::sync_channel(128);
    if std::thread::Builder::new()
        .name("sensitive-decision-log".into())
        .spawn(move || {
            let mut sink = Sink::new(file);
            while let Ok((category, details)) = receiver.recv() {
                match sink.write(category, details) {
                    Ok(Written::BudgetReached {
                        first: true,
                        budget,
                    }) => report_notice(SensitiveDebugNotice::BudgetReached {
                        budget: budget.code(),
                    }),
                    Ok(_) => {}
                    Err(_) => {
                        report_notice(SensitiveDebugNotice::WriteFailedOrLimitReached);
                        break;
                    }
                }
            }
        })
        .is_err()
    {
        report_notice(SensitiveDebugNotice::WorkerUnavailable);
        return None;
    }
    report_notice(SensitiveDebugNotice::Enabled);
    Some(sender)
}
fn bounded_details(details: Value) -> Value {
    if serde_json::to_vec(&details).map_or(true, |bytes| bytes.len() > MAX_RECORD_BYTES / 2) {
        json!({"details_omitted":"record_size_limit"})
    } else {
        details
    }
}
struct Sink {
    file: File,
    /// Bytes written per `Budget::ALL` entry, and of budget reports.
    written: [u64; 3],
    report_written: u64,
    /// Records dropped per budget, and as of the last budget report.
    dropped: [u64; 3],
    reported: [u64; 3],
    last_report: Option<Instant>,
    recent: HashMap<&'static str, (Value, Instant)>,
}
#[derive(Debug, PartialEq, Eq)]
enum Written {
    Stored,
    Repeated,
    /// Dropped: its class reached its budget (`first` for the first drop).
    BudgetReached {
        first: bool,
        budget: Budget,
    },
}
impl Sink {
    fn new(file: File) -> Self {
        Self {
            file,
            written: [0; 3],
            report_written: 0,
            dropped: [0; 3],
            reported: [0; 3],
            last_report: None,
            recent: HashMap::new(),
        }
    }

    fn write(&mut self, category: &'static str, details: Value) -> io::Result<Written> {
        let written = self.write_record(category, details)?;
        self.report_drops()?;
        Ok(written)
    }

    /// Every `BUDGET_REPORT_INTERVAL` at most, a small record with the drop
    /// counts per budget (fixed codes and counts only), within its own budget.
    fn report_drops(&mut self) -> io::Result<()> {
        let now = Instant::now();
        if self.dropped == self.reported
            || self
                .last_report
                .is_some_and(|at| now.duration_since(at) < BUDGET_REPORT_INTERVAL)
        {
            return Ok(());
        }
        let dropped: serde_json::Map<String, Value> = Budget::ALL
            .iter()
            .zip(self.dropped)
            .map(|(budget, count)| (budget.code().to_string(), json!(count)))
            .collect();
        let mut record = serde_json::to_vec(&json!({
            "sensitive": true,
            "timestamp_unix_ms": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),
            "category": BUDGET_REPORT_CATEGORY,
            "details": {"dropped": dropped},
        }))?;
        record.push(b'\n');
        if self.report_written + record.len() as u64 > BUDGET_REPORT_BYTES {
            return Ok(());
        }
        self.file.write_all(&record)?;
        self.file.flush()?;
        self.report_written += record.len() as u64;
        self.reported = self.dropped;
        self.last_report = Some(now);
        Ok(())
    }

    fn write_record(&mut self, category: &'static str, details: Value) -> io::Result<Written> {
        let details = bounded_details(details);
        let now = Instant::now();
        if self.recent.get(category).is_some_and(|(previous, at)| {
            previous == &details && now.duration_since(*at) < REPEAT_INTERVAL
        }) {
            return Ok(Written::Repeated);
        }
        let mut record = serde_json::to_vec(
            &json!({"sensitive":true,"timestamp_unix_ms":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),"category":category,"details":details}),
        )?;
        if record.len() > MAX_RECORD_BYTES {
            record = serde_json::to_vec(
                &json!({"sensitive":true,"category":category,"details_omitted":"record_size_limit"}),
            )?;
        }
        record.push(b'\n');
        let budget = Budget::of(category);
        let index = budget as usize;
        if self.written[index].saturating_add(record.len() as u64) > budget.bytes() {
            self.dropped[index] += 1;
            return Ok(Written::BudgetReached {
                first: self.dropped[index] == 1,
                budget,
            });
        }
        self.file.write_all(&record)?;
        self.file.flush()?;
        self.written[index] += record.len() as u64;
        // Fixed producer categories, never titles/IDs as keys.
        self.recent.insert(category, (details, now));
        Ok(Written::Stored)
    }
}
#[cfg(windows)]
fn create_private_directory(path: &Path) -> io::Result<()> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    #[repr(C)]
    struct SecurityAttributes {
        length: u32,
        descriptor: *mut c_void,
        inherit: i32,
    }
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text: *const u16,
            revision: u32,
            descriptor: *mut *mut c_void,
            size: *mut u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateDirectoryW(path: *const u16, security: *const SecurityAttributes) -> i32;
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }
    // Protected DACL inherited by children: object owner and LocalSystem only.
    // OWNER RIGHTS follows the creator without username/path shell interpolation.
    // Administrators with takeover privileges are outside this boundary.
    let descriptor: Vec<u16> = "D:P(A;OICI;FA;;;OW)(A;OICI;FA;;;SY)\0"
        .encode_utf16()
        .collect();
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut security = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            descriptor.as_ptr(),
            1,
            &mut security,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let attributes = SecurityAttributes {
            length: std::mem::size_of::<SecurityAttributes>() as u32,
            descriptor: security,
            inherit: 0,
        };
        let created = CreateDirectoryW(path.as_ptr(), &attributes);
        let error = (created == 0).then(io::Error::last_os_error);
        LocalFree(security);
        error.map_or(Ok(()), Err)
    }
}
#[cfg(unix)]
fn create_private_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(path)
}
#[cfg(not(any(windows, unix)))]
fn create_private_directory(_: &Path) -> io::Result<()> {
    Err(io::Error::other("protected storage unsupported"))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensitive_reporter_replays_early_failures_and_latest_loss_count() {
        static RECEIVED: Mutex<Vec<SensitiveDebugNotice>> = Mutex::new(Vec::new());
        fn reporter(notice: SensitiveDebugNotice) {
            RECEIVED.lock().unwrap().push(notice);
        }
        let relay = NoticeRelay::default();
        relay.report(SensitiveDebugNotice::StorageUnavailable);
        relay.report(SensitiveDebugNotice::QueueFull { total_dropped: 1 });
        relay.report(SensitiveDebugNotice::QueueFull { total_dropped: 2 });
        assert_eq!(relay.state.lock().unwrap().pending.len(), 2);
        relay.install(reporter);
        relay.report(SensitiveDebugNotice::WriterStopped);
        assert_eq!(
            *RECEIVED.lock().unwrap(),
            vec![
                SensitiveDebugNotice::StorageUnavailable,
                SensitiveDebugNotice::QueueFull { total_dropped: 2 },
                SensitiveDebugNotice::WriterStopped
            ]
        );
        assert!(relay.state.lock().unwrap().pending.is_empty());
        relay.install(reporter);
        assert_eq!(RECEIVED.lock().unwrap().len(), 3);
    }

    #[test]
    fn sensitive_directory_never_reuses_an_existing_destination() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("session");
        create_private_directory(&path).unwrap();
        assert!(create_private_directory(&path).is_err());
        File::create(path.join("test.jsonl")).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn sensitive_directory_acl_is_protected_owner_and_system_only() {
        use std::ffi::c_void;
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "advapi32")]
        unsafe extern "system" {
            fn GetFileSecurityW(
                path: *const u16,
                information: u32,
                descriptor: *mut c_void,
                length: u32,
                needed: *mut u32,
            ) -> i32;
            fn ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor: *const c_void,
                revision: u32,
                information: u32,
                text: *mut *mut u16,
                length: *mut u32,
            ) -> i32;
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn LocalFree(memory: *mut c_void) -> *mut c_void;
        }
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("session");
        create_private_directory(&path).unwrap();
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            let mut needed = 0;
            GetFileSecurityW(wide.as_ptr(), 4, std::ptr::null_mut(), 0, &mut needed);
            assert!(needed > 0);
            // u64 alignment is sufficient for a self-relative descriptor.
            let mut descriptor = vec![0u64; (needed as usize).div_ceil(8)];
            assert_ne!(
                GetFileSecurityW(
                    wide.as_ptr(),
                    4,
                    descriptor.as_mut_ptr().cast(),
                    needed,
                    &mut needed
                ),
                0
            );
            let mut text = std::ptr::null_mut();
            let mut length = 0;
            assert_ne!(
                ConvertSecurityDescriptorToStringSecurityDescriptorW(
                    descriptor.as_ptr().cast(),
                    1,
                    4,
                    &mut text,
                    &mut length
                ),
                0
            );
            let sddl = String::from_utf16_lossy(std::slice::from_raw_parts(
                text,
                length.saturating_sub(1) as usize,
            ));
            LocalFree(text.cast());
            assert!(
                sddl.starts_with("D:P"),
                "DACL must prohibit inherited grants"
            );
            assert_eq!(sddl.matches("(A;").count(), 2);
            assert!(sddl.contains("(A;OICI;FA;;;OW)"));
            assert!(sddl.contains("(A;OICI;FA;;;SY)"));
        }
    }
    #[test]
    fn sensitive_sink_bounds_deduplicates_and_preserves_error_details() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut sink = Sink::new(file.reopen().unwrap());
        let details = json!({"stage":"password_property","error":"synthetic provider error\n0x80004005","candidate":{"pid":42}});
        sink.write("probe", details.clone()).unwrap();
        let size = sink.written;
        assert_eq!(
            sink.write("probe", details.clone()).unwrap(),
            Written::Repeated
        );
        assert_eq!(sink.written, size);
        let row: Value =
            serde_json::from_str(std::fs::read_to_string(file.path()).unwrap().trim()).unwrap();
        assert_eq!(row["details"], details);
        assert_eq!(row["sensitive"], true);
        sink.write("probe", json!({"stage":"different"})).unwrap();
        assert!(sink.written[Budget::Detail as usize] > size[Budget::Detail as usize]);
    }

    #[test]
    fn sensitive_sink_budgets_keep_one_record_class_from_starving_another() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut sink = Sink::new(file.reopen().unwrap());
        assert_eq!(Budget::of("input_keystroke_epoch"), Budget::Summary);
        assert_eq!(Budget::of("input_probe_summary"), Budget::Summary);
        assert_eq!(
            Budget::of(CAPTURE_DECISION_CATEGORY),
            Budget::CaptureDecision
        );
        assert_eq!(Budget::of("input_privacy_probe"), Budget::Detail);
        // The capture decisions' budget is full: only they are dropped.
        sink.written[Budget::CaptureDecision as usize] = CAPTURE_DECISION_BUDGET_BYTES;
        assert_eq!(
            sink.write(CAPTURE_DECISION_CATEGORY, json!({"n":1}))
                .unwrap(),
            Written::BudgetReached {
                first: true,
                budget: Budget::CaptureDecision
            }
        );
        assert_eq!(
            sink.write(CAPTURE_DECISION_CATEGORY, json!({"n":2}))
                .unwrap(),
            Written::BudgetReached {
                first: false,
                budget: Budget::CaptureDecision
            }
        );
        assert_eq!(
            sink.write("input_keystroke_epoch", json!({"admitted":1}))
                .unwrap(),
            Written::Stored
        );
        assert_eq!(
            sink.write("input_privacy_probe", json!({"stage":"x"}))
                .unwrap(),
            Written::Stored
        );
        // Summaries have their own budget too, and a full one never stops
        // the writer for the others.
        sink.written[Budget::Summary as usize] = SUMMARY_BUDGET_BYTES;
        assert!(matches!(
            sink.write("input_probe_summary", json!({"probes":1}))
                .unwrap(),
            Written::BudgetReached {
                first: true,
                budget: Budget::Summary
            }
        ));
        assert_eq!(
            sink.write("input_privacy_probe", json!({"stage":"y"}))
                .unwrap(),
            Written::Stored
        );
        assert_eq!(sink.dropped, [1, 2, 0]);
        // The drops were reported once (rate limited), with fixed codes.
        let text = std::fs::read_to_string(file.path()).unwrap();
        let reports: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|row| row["category"] == BUDGET_REPORT_CATEGORY)
            .collect();
        assert_eq!(reports.len(), 1);
        assert_eq!(
            reports[0]["details"]["dropped"],
            json!({"summary": 0, "capture_privacy_decision": 1, "detail": 0})
        );
        // Every budget holds: disk use stays bounded by their sum.
        for (index, budget) in Budget::ALL.into_iter().enumerate() {
            assert!(sink.written[index] <= budget.bytes(), "{budget:?}");
        }
        assert!(sink.report_written <= BUDGET_REPORT_BYTES);
        assert!(sink.written.iter().sum::<u64>() + sink.report_written <= MAX_BYTES);
        assert_eq!(Budget::Summary.code(), "summary");
    }

    #[test]
    fn sensitive_oversized_records_remain_valid_json() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut sink = Sink::new(file.reopen().unwrap());
        sink.write("probe", json!({"error":"x".repeat(MAX_RECORD_BYTES)}))
            .unwrap();
        let row: Value =
            serde_json::from_str(std::fs::read_to_string(file.path()).unwrap().trim()).unwrap();
        assert_eq!(row["details"]["details_omitted"], "record_size_limit");
        assert!(sink.recent.get("probe").unwrap().0.to_string().len() < 128);
    }
}
