//! Fresh, bounded UIA bulk acquisition. All trees and subscriptions are attempt-local.

use super::timing::{self, RetryCause, Stage};
use crate::events::AccessibilityNode;
use crate::platform::windows_uia::UiaContext;
pub(super) use screenpipe_config::capture_diagnostics::AccessibilityStopReason as Stop;
use screenpipe_config::capture_diagnostics::RuleReference;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, MutexGuard,
};
use std::time::Instant;
use windows::core::{implement, Interface, VARIANT};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::*;
thread_local! { static PROVIDER_STAGE: std::cell::Cell<&'static str> = const { std::cell::Cell::new("bulk_setup") }; }
fn provider_stage(stage: &'static str) {
    PROVIDER_STAGE.with(|value| value.set(stage));
}

// Fixed diagnostic identifiers only: never retain rejected property values or IDs.
#[derive(Clone, Copy)]
#[repr(usize)]
#[allow(dead_code)] // Versioned diagnostic slots from the former cross-cache joins remain zero.
enum EvidenceRejection {
    RegistrationPoisoned,
    RequiredStringType,
    ControlTypeVariant,
    ControlTypeRange,
    PasswordVariant,
    ConflictingProjection,
    ProjectionOutsideHierarchy,
    StringVariant,
    RuntimeIdVariant,
    RuntimeIdArrayLayout,
    RuntimeIdLength,
    DuplicateQueryRuntimeId,
    DuplicateValueRuntimeId,
    DuplicateHierarchyRuntimeId,
    MissingTitleProjection,
    MissingDocumentValue,
    MissingEntryValue,
    MissingExtensionValue,
    NegativeChildCount,
    ControlElementVariant,
    RawOnlyPolicyElement,
}
const EVIDENCE_REJECTION_CODES: [&str; 21] = [
    "registration_poisoned",
    "required_string_type",
    "control_type_variant",
    "control_type_range",
    "password_variant",
    "conflicting_projection",
    "projection_outside_hierarchy",
    "string_variant",
    "runtime_id_variant",
    "runtime_id_array_layout",
    "runtime_id_length",
    "duplicate_query_runtime_id",
    "duplicate_value_runtime_id",
    "duplicate_hierarchy_runtime_id",
    "missing_title_projection",
    "missing_document_value",
    "missing_entry_value",
    "missing_extension_value",
    "negative_child_count",
    "control_element_variant",
    "raw_only_policy_element",
];
static EVIDENCE_REJECTIONS: [AtomicU64; 21] = [const { AtomicU64::new(0) }; 21];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
enum ValueEvidenceKind {
    KnownString,
    KnownUnavailable,
    UnknownVariant,
    UnknownGetter,
    UnknownSentinelCheck,
}
struct ValueRead {
    value: Option<String>,
    kind: ValueEvidenceKind,
}
struct DocumentValueObservation {
    kind: ValueEvidenceKind,
    http_url: Option<String>,
}
fn document_http_url(value: Option<&str>) -> Option<&str> {
    value
        .map(str::trim)
        .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
}
fn document_url_sources(
    observations: &[DocumentValueObservation],
    url: Option<&str>,
) -> Vec<usize> {
    let Some(url) = url else { return Vec::new() };
    observations
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (item.http_url.as_deref() == Some(url)).then_some(index))
        .collect()
}
fn document_url_loss_is_unknown(
    observations: &[DocumentValueObservation],
    previous_count: usize,
    previous_sources: &[usize],
) -> bool {
    observations.len() == previous_count
        && !previous_sources.is_empty()
        && previous_sources.iter().all(|&index| {
            observations
                .get(index)
                .is_some_and(|item| item.kind != ValueEvidenceKind::KnownString)
        })
}
const VALUE_EVIDENCE_CODES: [&str; 5] = [
    "known_string",
    "known_unavailable",
    "unknown_variant",
    "unknown_getter",
    "unknown_sentinel_check",
];
static VALUE_EVIDENCE_COUNTS: [AtomicU64; 5] = [const { AtomicU64::new(0) }; 5];
fn note_value_evidence(kind: ValueEvidenceKind, vt: Option<u16>, document: bool) {
    let index = kind as usize;
    VALUE_EVIDENCE_COUNTS[index].fetch_add(1, Ordering::Relaxed);
    if kind != ValueEvidenceKind::KnownString {
        screenpipe_config::sensitive_debug::record("uia_policy_value_evidence", || {
            serde_json::json!({"reason":VALUE_EVIDENCE_CODES[index],"reason_id":index,
                "value_vt":vt,"document":document})
        });
    }
}
fn evidence_rejected(reason: EvidenceRejection) -> Stop {
    let index = reason as usize;
    let code = EVIDENCE_REJECTION_CODES[index];
    let count = EVIDENCE_REJECTIONS[index].fetch_add(1, Ordering::Relaxed) + 1;
    let stage = timing::current_stage_name().unwrap_or("bulk_setup");
    // Exact cumulative counts remain available even when ordinary logs are sampled.
    if count == 1 || count % 64 == 0 {
        tracing::warn!(
            reason = code,
            reason_id = index,
            stage,
            count,
            "UIA bulk evidence rejected"
        );
    }
    screenpipe_config::sensitive_debug::record(
        "uia_bulk_evidence_rejection",
        || serde_json::json!({"reason":code,"reason_id":index,"stage":stage}),
    );
    Stop::Provider
}

struct Metrics {
    attempts: AtomicU64,
    completed: AtomicU64,
    failed: AtomicU64,
    denied: AtomicU64,
    retries: AtomicU64,
    calls: AtomicU64,
    root_calls: AtomicU64,
    registration_calls: AtomicU64,
    cleanup_calls: AtomicU64,
    cleanup_failures: AtomicU64,
    nodes: AtomicU64,
    raw_nodes: AtomicU64,
    duration_us: AtomicU64,
    max_us: AtomicU64,
    privacy_acquisitions: AtomicU64,
    recording_acquisitions: AtomicU64,
    histogram: [AtomicU64; 7],
    reasons: [AtomicU64; 8],
}
static METRICS: Metrics = Metrics {
    attempts: AtomicU64::new(0),
    completed: AtomicU64::new(0),
    failed: AtomicU64::new(0),
    denied: AtomicU64::new(0),
    retries: AtomicU64::new(0),
    calls: AtomicU64::new(0),
    root_calls: AtomicU64::new(0),
    registration_calls: AtomicU64::new(0),
    cleanup_calls: AtomicU64::new(0),
    cleanup_failures: AtomicU64::new(0),
    nodes: AtomicU64::new(0),
    raw_nodes: AtomicU64::new(0),
    duration_us: AtomicU64::new(0),
    max_us: AtomicU64::new(0),
    privacy_acquisitions: AtomicU64::new(0),
    recording_acquisitions: AtomicU64::new(0),
    histogram: [const { AtomicU64::new(0) }; 7],
    reasons: [const { AtomicU64::new(0) }; 8],
};
// UIA registration/removal must not run concurrently in one process. This
// mutex guards subscriptions only, never trees, evaluation or admission.
static REGISTRATION: Mutex<()> = Mutex::new(());
// Diagnostic deduplication retains only fixed rule IDs, never trees or permits.
type ScopeLogKey = (&'static str, usize, Vec<RuleReference>);
static SCOPE_LOGS: Mutex<Vec<ScopeLogKey>> = Mutex::new(Vec::new());
pub(super) fn log_scope(
    reason: &'static str,
    properties: usize,
    rules: &[RuleReference],
    nodes: usize,
) {
    let bounded: Vec<_> = rules.iter().take(64).copied().collect();
    let key = (reason, properties, bounded.clone());
    let should_log = SCOPE_LOGS
        .lock()
        .map(|mut seen| {
            if seen.contains(&key) || seen.len() >= 64 {
                return false;
            }
            seen.push(key);
            true
        })
        .unwrap_or(false);
    if should_log {
        tracing::info!(reason, property_count=properties, rule_count=rules.len(),
            rules=?bounded, rules_truncated=rules.len()>bounded.len(), nodes,
            "UIA acquisition scope exception");
    }
}
fn registration(deadline: Instant) -> Result<MutexGuard<'static, ()>, Stop> {
    timing::enter(Stage::RegistrationWait);
    loop {
        if Instant::now() >= deadline {
            timing::timeout();
            return Err(Stop::Timeout);
        }
        match REGISTRATION.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(evidence_rejected(EvidenceRejection::RegistrationPoisoned));
            }
            Err(std::sync::TryLockError::WouldBlock) => std::thread::yield_now(),
        }
    }
}
fn initial_deadline(deadline: Instant) -> Result<(), Stop> {
    if Instant::now() >= deadline {
        timing::timeout();
        Err(Stop::Timeout)
    } else {
        Ok(())
    }
}
/// UIA requires a 50 ms minimum timeout. Start a new provider call only
/// when at least 50 ms remains, and never configure past the phase deadline.
fn provider_timeout_at(now: Instant, deadline: Instant) -> Result<u32, Stop> {
    if now >= deadline {
        return Err(Stop::Timeout);
    }
    let remaining = deadline.duration_since(now);
    if remaining < std::time::Duration::from_millis(50) {
        return Err(Stop::Timeout);
    }
    Ok(remaining.as_millis().min(u32::MAX as u128) as u32)
}
fn configure_provider_timeouts(timeouts: &IUIAutomation2, deadline: Instant) -> Result<(), Stop> {
    let timeout = provider_timeout_at(Instant::now(), deadline).map_err(|reason| {
        timing::timeout();
        reason
    })?;
    unsafe {
        timeouts.SetConnectionTimeout(timeout).map_err(provider)?;
        timeouts.SetTransactionTimeout(timeout).map_err(provider)?;
    }
    initial_deadline(deadline)
}
pub(super) fn retry(cause: RetryCause) {
    METRICS.retries.fetch_add(1, Ordering::Relaxed);
    timing::retry(cause);
}
pub(super) fn fresh_until_current<T>(
    deadline: Instant,
    mut now: impl FnMut() -> Instant,
    mut acquire: impl FnMut() -> Result<Option<T>, Stop>,
) -> Result<T, Stop> {
    loop {
        if now() >= deadline {
            timing::timeout();
            return Err(Stop::Timeout);
        }
        let result = acquire()?;
        if now() >= deadline {
            timing::timeout();
            return Err(Stop::Timeout);
        }
        if let Some(result) = result {
            return Ok(result);
        }
        retry(RetryCause::PrivacyInvalidation);
    }
}
fn required_string_type(vt: u16) -> Result<(), Stop> {
    if vt == 8 {
        Ok(())
    } else {
        Err(evidence_rejected(EvidenceRejection::RequiredStringType))
    }
}
fn classify_cached_value_type(vt: u16, not_supported: Result<bool, ()>) -> ValueEvidenceKind {
    match vt {
        8 => ValueEvidenceKind::KnownString,
        // The exact UIA sentinel distinguishes a confirmed unavailable Value
        // from a provider's unrelated VT_UNKNOWN interface. Both are absent
        // evidence for matching; neither becomes an empty string.
        13 => match not_supported {
            Ok(true) => ValueEvidenceKind::KnownUnavailable,
            Ok(false) => ValueEvidenceKind::UnknownVariant,
            Err(()) => ValueEvidenceKind::UnknownSentinelCheck,
        },
        _ => ValueEvidenceKind::UnknownVariant,
    }
}
fn cached_control_type(value: &VARIANT) -> Result<i32, Stop> {
    if unsafe { value.as_raw().Anonymous.Anonymous.vt } != 3 {
        return Err(evidence_rejected(EvidenceRejection::ControlTypeVariant));
    }
    let control = i32::try_from(value).map_err(provider)?;
    if !(50000..=50040).contains(&control) {
        return Err(evidence_rejected(EvidenceRejection::ControlTypeRange));
    }
    Ok(control)
}
fn cached_password(value: &VARIANT) -> Result<Option<bool>, Stop> {
    match unsafe { value.as_raw().Anonymous.Anonymous.vt } {
        11 => bool::try_from(value).map(Some).map_err(provider),
        // Unsupported password evidence remains unknown and is redacted for
        // entry controls by the existing extraction policy.
        0 | 13 => Ok(None),
        _ => Err(evidence_rejected(EvidenceRejection::PasswordVariant)),
    }
}
fn cached_control_element(value: &VARIANT) -> Result<bool, Stop> {
    if unsafe { value.as_raw().Anonymous.Anonymous.vt } != 11 {
        return Err(evidence_rejected(EvidenceRejection::ControlElementVariant));
    }
    bool::try_from(value).map_err(provider)
}
/// A cache request has one property set for its whole subtree. The provider
/// therefore fetches Name/Value broadly when rules need them; decoding below
/// still materializes strings only for controls those rules inspect.
fn cache_properties(full_text: bool, subtree_rules: bool, need_url: bool) -> Vec<UIA_PROPERTY_ID> {
    let mut properties = vec![
        UIA_ControlTypePropertyId,
        UIA_IsPasswordPropertyId,
        UIA_IsControlElementPropertyId,
    ];
    if subtree_rules {
        properties.push(UIA_NamePropertyId);
    }
    if subtree_rules || need_url {
        properties.push(UIA_ValueValuePropertyId);
    }
    if full_text {
        if !subtree_rules {
            properties.push(UIA_NamePropertyId);
        }
        if !subtree_rules && !need_url {
            properties.push(UIA_ValueValuePropertyId);
        }
        properties.extend_from_slice(&[
            UIA_AutomationIdPropertyId,
            UIA_ClassNamePropertyId,
            UIA_BoundingRectanglePropertyId,
            UIA_IsEnabledPropertyId,
            UIA_HasKeyboardFocusPropertyId,
            UIA_IsKeyboardFocusablePropertyId,
            UIA_HelpTextPropertyId,
            UIA_AcceleratorKeyPropertyId,
            UIA_AccessKeyPropertyId,
            UIA_LocalizedControlTypePropertyId,
        ]);
    }
    properties
}
pub(super) fn cache_property_count(full_text: bool, subtree_rules: bool, need_url: bool) -> usize {
    cache_properties(full_text, subtree_rules, need_url).len()
}
fn invalidation_properties() -> [UIA_PROPERTY_ID; 5] {
    [
        UIA_NamePropertyId,
        UIA_ValueValuePropertyId,
        UIA_ControlTypePropertyId,
        UIA_IsPasswordPropertyId,
        UIA_IsControlElementPropertyId,
    ]
}

/// Attempt-local RawView-only rule evidence and retained Document URL candidates.
/// Raw-only strings never enter the recorded extraction tree or snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RawPolicyEvidence {
    NestedWindow {
        name: Option<String>,
    },
    Document {
        name: Option<String>,
        value: Option<String>,
    },
    DocumentUrl {
        value: Option<String>,
    },
    ExtensionShallow {
        name: Option<String>,
        value: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawNodeAction {
    Keep,
    Flatten,
}
fn raw_node_action(root: bool, is_control: bool) -> RawNodeAction {
    if root || is_control {
        RawNodeAction::Keep
    } else {
        RawNodeAction::Flatten
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RawPolicyScope {
    nested_window: bool,
    document: bool,
    extension_shallow: bool,
    name: bool,
    required_name: bool,
    value: bool,
}

fn raw_policy_scope(
    root: bool,
    is_control: bool,
    control: i32,
    subtree_rules: bool,
    need_url: bool,
    raw_extension_levels: usize,
    policy_visible: bool,
    shallow_visible: bool,
    raw_extension_origin: bool,
) -> RawPolicyScope {
    let raw_only = !root && !is_control;
    let nested_window =
        policy_visible && raw_only && subtree_rules && control == UIA_WindowControlTypeId.0;
    let document = policy_visible
        && raw_only
        && (subtree_rules || need_url)
        && control == UIA_DocumentControlTypeId.0;
    // A retained Document is handled by the ControlView walk. Its raw-only
    // children need extra evidence; retained children need it only when the
    // extension Document itself was raw-only and disappeared from that walk.
    let extension_shallow = shallow_visible
        && subtree_rules
        && raw_extension_levels > 0
        && (raw_only || raw_extension_origin);
    RawPolicyScope {
        nested_window,
        document,
        extension_shallow,
        name: nested_window || (document && subtree_rules) || extension_shallow,
        required_name: nested_window || (document && subtree_rules) || extension_shallow,
        value: document || extension_shallow,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RulePropertyScope {
    name: bool,
    required_name: bool,
    value: bool,
}
fn rule_property_scope(
    control: i32,
    subtree_rules: bool,
    need_url: bool,
    extension_levels: usize,
) -> RulePropertyScope {
    let document = control == UIA_DocumentControlTypeId.0;
    let entry = control == UIA_EditControlTypeId.0 || control == UIA_ComboBoxControlTypeId.0;
    let title =
        subtree_rules && (control == UIA_WindowControlTypeId.0 || document || extension_levels > 0);
    RulePropertyScope {
        name: title,
        required_name: title,
        value: ((subtree_rules || need_url) && (document || entry))
            || (subtree_rules && extension_levels > 0),
    }
}
fn projected_rule_scope(
    action: RawNodeAction,
    control: i32,
    subtree_rules: bool,
    need_url: bool,
    extension_levels: usize,
) -> Option<RulePropertyScope> {
    (action == RawNodeAction::Keep)
        .then(|| rule_property_scope(control, subtree_rules, need_url, extension_levels))
}
fn document_url_scope(control: i32, subtree_rules: bool, need_url: bool) -> bool {
    control == UIA_DocumentControlTypeId.0 && (subtree_rules || need_url)
}
fn additional_document_url_evidence(
    action: RawNodeAction,
    raw_scope: RawPolicyScope,
    document_url: bool,
) -> bool {
    document_url && (action == RawNodeAction::Keep || !raw_scope.document)
}
fn next_extension_levels(
    control: i32,
    subtree_rules: bool,
    value: Option<&str>,
    extension_levels: usize,
) -> usize {
    if subtree_rules
        && control == UIA_DocumentControlTypeId.0
        && value.is_some_and(super::is_extension_url)
    {
        2
    } else {
        extension_levels.saturating_sub(1)
    }
}
fn child_extension_state(
    control: i32,
    subtree_rules: bool,
    name: Option<&str>,
    value: Option<&str>,
    control_levels: usize,
    raw_levels: usize,
    raw_origin: bool,
    action: RawNodeAction,
    policy_visible: bool,
    shallow_visible: bool,
    scope: RawPolicyScope,
) -> (usize, usize, bool, bool) {
    let extension_document = policy_visible
        && subtree_rules
        && control == UIA_DocumentControlTypeId.0
        && value.is_some_and(super::is_extension_url);
    // Empty non-control layout wrappers do not consume either of the two
    // shallow policy levels. A named/value-bearing raw node does consume one.
    let meaningful_raw = action == RawNodeAction::Keep
        || scope.nested_window
        || scope.document
        || (subtree_rules && control == UIA_DocumentControlTypeId.0)
        || name.is_some_and(|name| !name.trim().is_empty())
        || value.is_some_and(|value| !value.trim().is_empty());
    let next_raw = if extension_document {
        2
    } else if meaningful_raw {
        raw_levels.saturating_sub(1)
    } else {
        raw_levels
    };
    let next_control = if action == RawNodeAction::Keep {
        next_extension_levels(control, subtree_rules, value, control_levels)
    } else {
        control_levels
    };
    let next_origin = if extension_document {
        action == RawNodeAction::Flatten
    } else {
        raw_origin
    };
    let next_shallow_visible = extension_document || (shallow_visible && next_raw > 0);
    (next_control, next_raw, next_origin, next_shallow_visible)
}
fn retained_control_prunes_policy(
    control: i32,
    password: Option<bool>,
    value: Option<&str>,
) -> bool {
    let skipped = [
        UIA_ScrollBarControlTypeId.0,
        UIA_ImageControlTypeId.0,
        UIA_SeparatorControlTypeId.0,
        UIA_ThumbControlTypeId.0,
        UIA_ToolTipControlTypeId.0,
        UIA_ProgressBarControlTypeId.0,
    ]
    .contains(&control);
    let entry = control == UIA_EditControlTypeId.0 || control == UIA_ComboBoxControlTypeId.0;
    skipped
        || password == Some(true)
        || (entry && password.is_none())
        || (entry && value.is_some_and(|value| !value.trim().is_empty()))
}
fn counts_policy_depth(
    action: RawNodeAction,
    scope: RawPolicyScope,
    document_url: bool,
    name: Option<&str>,
    value: Option<&str>,
) -> bool {
    action == RawNodeAction::Keep
        || scope.nested_window
        || scope.document
        || document_url
        || (scope.extension_shallow
            && (name.is_some_and(|name| !name.trim().is_empty())
                || value.is_some_and(|value| !value.trim().is_empty())))
}
fn node_budget(
    now: Instant,
    deadline: Instant,
    nodes: usize,
    max_nodes: usize,
    depth: usize,
    max_raw_depth: usize,
) -> Result<(), Stop> {
    if now >= deadline {
        timing::timeout();
        return Err(Stop::Timeout);
    }
    if nodes >= max_nodes {
        return Err(Stop::Nodes);
    }
    if depth > max_raw_depth {
        return Err(Stop::Depth);
    }
    Ok(())
}
fn meaningful_depth_budget(
    control_depth: usize,
    policy_depth: usize,
    max_depth: usize,
) -> Result<(), Stop> {
    if control_depth > max_depth || policy_depth > max_depth {
        Err(Stop::Depth)
    } else {
        Ok(())
    }
}
const MAX_RAW_DEPTH: usize = 128;
fn bounded_utf16(utf16: &[u16], remaining: usize) -> Result<String, Stop> {
    let limit = remaining.min(16 * 1024);
    let mut result = String::new();
    for character in char::decode_utf16(utf16.iter().copied()) {
        let character = character.unwrap_or(char::REPLACEMENT_CHARACTER);
        if result.len() + character.len_utf8() > limit {
            return Err(if remaining < 16 * 1024 {
                Stop::AggregateText
            } else {
                Stop::PropertyText
            });
        }
        result.push(character);
    }
    Ok(result)
}
pub(super) fn record_attempt(
    duration: std::time::Duration,
    result: &anyhow::Result<super::TreeWalkResult>,
) {
    METRICS.attempts.fetch_add(1, Ordering::Relaxed);
    match result {
        Ok(super::TreeWalkResult::Found(_)) => {
            METRICS.completed.fetch_add(1, Ordering::Relaxed);
        }
        Err(_)
        | Ok(super::TreeWalkResult::Skipped(super::SkipReason::AccessibilityRetrievalFailed(_)))
        | Ok(super::TreeWalkResult::Skipped(super::SkipReason::AccessibilityIncomplete(_)))
        | Ok(super::TreeWalkResult::Skipped(super::SkipReason::MonitorUnverified)) => {
            METRICS.failed.fetch_add(1, Ordering::Relaxed);
        }
        _ => {
            METRICS.denied.fetch_add(1, Ordering::Relaxed);
        }
    }
    let us = duration.as_micros().min(u64::MAX as u128) as u64;
    METRICS.duration_us.fetch_add(us, Ordering::Relaxed);
    METRICS.max_us.fetch_max(us, Ordering::Relaxed);
    let bucket = [25_000, 50_000, 100_000, 150_000, 250_000, 500_000]
        .iter()
        .position(|bound| us <= *bound)
        .unwrap_or(6);
    METRICS.histogram[bucket].fetch_add(1, Ordering::Relaxed);
    let cause = match result {
        Ok(super::TreeWalkResult::Skipped(super::SkipReason::AccessibilityRetrievalFailed(
            failure,
        ))) => Some(failure.cause),
        Ok(super::TreeWalkResult::Skipped(super::SkipReason::AccessibilityIncomplete(reason))) => {
            Some(*reason)
        }
        _ => None,
    };
    if let Some(cause) = cause {
        let index = match cause {
            Stop::Timeout => 0,
            Stop::Nodes => 1,
            Stop::Depth => 2,
            Stop::PropertyText => 3,
            Stop::AggregateText => 4,
            Stop::Provider => 5,
            Stop::Backoff => 6,
            Stop::Initialization => 7,
        };
        METRICS.reasons[index].fetch_add(1, Ordering::Relaxed);
    }
}
pub(super) fn metrics_snapshot() -> serde_json::Value {
    let load = |value: &AtomicU64| value.load(Ordering::Relaxed);
    serde_json::json!({"schema_version":3,"attempts":load(&METRICS.attempts),
        "completed":load(&METRICS.completed),"failed":load(&METRICS.failed),"denied":load(&METRICS.denied),
        "retries":load(&METRICS.retries),"stale":load(&METRICS.retries),
        "provider_calls":load(&METRICS.calls),"bulk_acquisition_calls":load(&METRICS.calls),
        "root_acquisition_calls":load(&METRICS.root_calls),
        "event_registration_calls":load(&METRICS.registration_calls),
        "event_cleanup_calls":load(&METRICS.cleanup_calls),
        "event_cleanup_failures":load(&METRICS.cleanup_failures),"nodes":load(&METRICS.nodes),
        "raw_nodes":load(&METRICS.raw_nodes),"projected_nodes":load(&METRICS.nodes),
        "total_duration_us":load(&METRICS.duration_us),"max_duration_us":load(&METRICS.max_us),
        "histogram_upper_us":[25000,50000,100000,150000,250000,500000,null],
        "histogram_counts":METRICS.histogram.iter().map(load).collect::<Vec<_>>(),
        "reason_codes":["uia_walk_timeout","uia_node_limit","uia_depth_limit","uia_property_text_limit",
            "uia_aggregate_text_limit","uia_provider_failure","uia_backoff","uia_initialization_failed"],
        "reason_counts":METRICS.reasons.iter().map(load).collect::<Vec<_>>(),
        "evidence_rejection_codes":EVIDENCE_REJECTION_CODES,
        "evidence_rejection_counts":EVIDENCE_REJECTIONS.iter().map(load).collect::<Vec<_>>(),
        "value_evidence_codes":VALUE_EVIDENCE_CODES,
        "value_evidence_counts":VALUE_EVIDENCE_COUNTS.iter().map(load).collect::<Vec<_>>(),
        "hierarchy_structure_acquisitions":load(&METRICS.privacy_acquisitions),
        "accessibility_recording_acquisitions":load(&METRICS.recording_acquisitions),
        "acquisition_mode":"single_raw_cache_control_projection",
        "single_cache_acquisitions":load(&METRICS.privacy_acquisitions)+load(&METRICS.recording_acquisitions),
        "single_cache_privacy_acquisitions":load(&METRICS.privacy_acquisitions),
        "single_cache_recording_acquisitions":load(&METRICS.recording_acquisitions)})
}

#[implement(IUIAutomationStructureChangedEventHandler)]
struct Structure(Arc<AtomicU64>);
impl IUIAutomationStructureChangedEventHandler_Impl for Structure_Impl {
    fn HandleStructureChangedEvent(
        &self,
        _: Option<&IUIAutomationElement>,
        _: StructureChangeType,
        _: *const windows::Win32::System::Com::SAFEARRAY,
    ) -> windows::core::Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[implement(IUIAutomationPropertyChangedEventHandler)]
struct Property(Arc<AtomicU64>);
impl IUIAutomationPropertyChangedEventHandler_Impl for Property_Impl {
    fn HandlePropertyChangedEvent(
        &self,
        _: Option<&IUIAutomationElement>,
        _: UIA_PROPERTY_ID,
        _: &VARIANT,
    ) -> windows::core::Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

pub(super) struct Attempt<'a> {
    automation: &'a IUIAutomation,
    timeouts: &'a IUIAutomation2,
    root: IUIAutomationElement,
    structure: IUIAutomationStructureChangedEventHandler,
    property: IUIAutomationPropertyChangedEventHandler,
    revision: Arc<AtomicU64>,
    deadline: Instant,
    max_nodes: usize,
    max_depth: usize,
    text_limit: usize,
    text_bytes: usize,
    pub calls: usize,
    pub nodes: usize,
    projected_nodes: usize,
    registered: bool,
    scope_rules: Vec<RuleReference>,
    raw_policy_evidence: Vec<RawPolicyEvidence>,
    document_values: Vec<DocumentValueObservation>,
}
fn provider(error: windows::core::Error) -> Stop {
    timing::native_hresult(error.code().0 as u32);
    screenpipe_config::sensitive_debug::record(
        "uia_bulk_provider_failure",
        || serde_json::json!({"stage":PROVIDER_STAGE.with(|value| value.get()), "hresult":error.code().0}),
    );
    if error.code().0 as u32 == UIA_E_TIMEOUT {
        timing::timeout();
        Stop::Timeout
    } else {
        Stop::Provider
    }
}
fn cleanup_result(result: windows::core::Result<()>) -> Result<(), Stop> {
    METRICS.cleanup_calls.fetch_add(1, Ordering::Relaxed);
    result.map_err(|error| {
        METRICS.cleanup_failures.fetch_add(1, Ordering::Relaxed);
        provider(error)
    })
}
impl<'a> Attempt<'a> {
    pub fn new(
        uia: &'a UiaContext,
        hwnd: HWND,
        deadline: Instant,
        nodes: usize,
        depth: usize,
        text_limit: usize,
    ) -> Result<Self, Stop> {
        provider_stage("bulk_setup");
        initial_deadline(deadline)?;
        if nodes == 0 {
            return Err(Stop::Nodes);
        }
        let automation = uia.bulk_automation();
        let timeouts = uia.bulk_timeouts();
        unsafe {
            configure_provider_timeouts(timeouts, deadline)?;
            provider_stage("bulk_element_from_handle");
            timing::enter(Stage::RootLookup);
            METRICS.root_calls.fetch_add(1, Ordering::Relaxed);
            let root = automation.ElementFromHandle(hwnd).map_err(provider)?;
            let revision = Arc::new(AtomicU64::new(0));
            let structure: IUIAutomationStructureChangedEventHandler =
                Structure(revision.clone()).into();
            let property: IUIAutomationPropertyChangedEventHandler =
                Property(revision.clone()).into();
            let _registration = registration(deadline)?;
            provider_stage("bulk_structure_subscribe");
            timing::enter(Stage::StructureSubscribe);
            METRICS.registration_calls.fetch_add(1, Ordering::Relaxed);
            configure_provider_timeouts(timeouts, deadline)?;
            automation
                .AddStructureChangedEventHandler(
                    &root,
                    TreeScope_Subtree,
                    None::<&IUIAutomationCacheRequest>,
                    &structure,
                )
                .map_err(provider)?;
            let properties = invalidation_properties();
            provider_stage("bulk_property_subscribe");
            timing::enter(Stage::PropertySubscribe);
            METRICS.registration_calls.fetch_add(1, Ordering::Relaxed);
            let property_registration = (|| -> Result<(), Stop> {
                configure_provider_timeouts(timeouts, deadline)?;
                automation
                    .AddPropertyChangedEventHandlerNativeArray(
                        &root,
                        TreeScope_Subtree,
                        None::<&IUIAutomationCacheRequest>,
                        &property,
                        &properties,
                    )
                    .map_err(provider)
            })();
            if let Err(reason) = property_registration {
                // Preserve the provider failure's remaining budget before
                // subscription removal can block in the failed setup path.
                timing::failure_budget(Instant::now());
                provider_stage("bulk_failed_setup_structure_remove");
                timing::enter(Stage::EventCleanup);
                let _ = cleanup_result(
                    automation.RemoveStructureChangedEventHandler(&root, &structure),
                );
                return Err(reason);
            }
            drop(_registration);
            let result = Self {
                automation,
                timeouts,
                root,
                structure,
                property,
                revision,
                deadline,
                max_nodes: nodes.min(5000),
                max_depth: depth.min(35),
                text_limit: text_limit.min(256 * 1024),
                text_bytes: 0,
                calls: 3,
                nodes: 0,
                projected_nodes: 0,
                registered: true,
                scope_rules: Vec::new(),
                raw_policy_evidence: Vec::new(),
                document_values: Vec::new(),
            };
            if let Err(reason) = result.check() {
                timing::failure_budget(Instant::now());
                return Err(reason);
            }
            Ok(result)
        }
    }
    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }
    pub fn set_deadline(&mut self, deadline: Instant) {
        self.deadline = deadline;
    }
    pub fn set_scope_rules(&mut self, rules: Vec<RuleReference>) {
        self.scope_rules = rules;
    }
    /// Read immediately after a successful acquire, under the caller's
    /// revision and focus checks. A later acquire replaces this evidence.
    pub fn raw_policy_evidence(&self) -> &[RawPolicyEvidence] {
        &self.raw_policy_evidence
    }
    /// Document ordinals refer only to the most recent successful cache
    /// acquisition and are meaningful under the caller's revision guard.
    pub fn document_value_count(&self) -> usize {
        self.document_values.len()
    }
    pub fn document_url_sources(&self, url: Option<&str>) -> Vec<usize> {
        document_url_sources(&self.document_values, url)
    }
    pub fn missing_url_is_unknown(
        &self,
        previous_count: usize,
        previous_sources: &[usize],
    ) -> bool {
        document_url_loss_is_unknown(&self.document_values, previous_count, previous_sources)
    }
    pub fn current(&self, revision: u64) -> bool {
        self.revision() == revision
    }
    /// Finish subscriptions before a result can be published. Removal errors,
    /// late cleanup and observed invalidation never become permission. If a
    /// revision changed while removing, restart subscriptions for a fresh retry
    /// using the same original deadline and no previous tree/result.
    pub fn finish(&mut self, revision: u64) -> Result<bool, Stop> {
        self.check()?;
        {
            let _registration = registration(self.deadline)?;
            unsafe {
                provider_stage("bulk_property_remove");
                timing::enter(Stage::EventCleanup);
                self.set_request_timeout()?;
                cleanup_result(
                    self.automation
                        .RemovePropertyChangedEventHandler(&self.root, &self.property),
                )?;
                provider_stage("bulk_structure_remove");
                self.set_request_timeout()?;
                cleanup_result(
                    self.automation
                        .RemoveStructureChangedEventHandler(&self.root, &self.structure),
                )?;
            }
            self.registered = false;
        }
        self.check()?;
        if self.current(revision) {
            return Ok(true);
        }
        {
            let _registration = registration(self.deadline)?;
            unsafe {
                provider_stage("bulk_structure_subscribe");
                timing::enter(Stage::StructureSubscribe);
                METRICS.registration_calls.fetch_add(1, Ordering::Relaxed);
                self.set_request_timeout()?;
                self.automation
                    .AddStructureChangedEventHandler(
                        &self.root,
                        TreeScope_Subtree,
                        None::<&IUIAutomationCacheRequest>,
                        &self.structure,
                    )
                    .map_err(provider)?;
                self.registered = true;
                provider_stage("bulk_property_subscribe");
                timing::enter(Stage::PropertySubscribe);
                METRICS.registration_calls.fetch_add(1, Ordering::Relaxed);
                self.set_request_timeout()?;
                self.automation
                    .AddPropertyChangedEventHandlerNativeArray(
                        &self.root,
                        TreeScope_Subtree,
                        None::<&IUIAutomationCacheRequest>,
                        &self.property,
                        &invalidation_properties(),
                    )
                    .map_err(provider)?;
            }
        }
        self.check()?;
        Ok(false)
    }
    pub fn check(&self) -> Result<(), Stop> {
        if Instant::now() >= self.deadline {
            timing::timeout();
            Err(Stop::Timeout)
        } else {
            Ok(())
        }
    }
    fn request(&mut self) -> Result<(), Stop> {
        self.check()?;
        self.set_request_timeout()?;
        self.calls += 1;
        Ok(())
    }
    fn set_request_timeout(&self) -> Result<(), Stop> {
        configure_provider_timeouts(self.timeouts, self.deadline)
    }
    unsafe fn cache(
        &self,
        properties: &[UIA_PROPERTY_ID],
        scope: TreeScope,
    ) -> Result<IUIAutomationCacheRequest, Stop> {
        provider_stage("bulk_cache_configuration");
        timing::enter(Stage::CacheConfiguration);
        let cache = self.automation.CreateCacheRequest().map_err(provider)?;
        cache.SetTreeScope(scope).map_err(provider)?;
        cache
            .SetTreeFilter(&self.automation.RawViewCondition().map_err(provider)?)
            .map_err(provider)?;
        cache
            .SetAutomationElementMode(AutomationElementMode_Full)
            .map_err(provider)?;
        for property in properties {
            cache.AddProperty(*property).map_err(provider)?;
        }
        Ok(cache)
    }
    fn string(
        &mut self,
        element: &IUIAutomationElement,
        property: UIA_PROPERTY_ID,
    ) -> Result<Option<String>, Stop> {
        self.check()?;
        let variant =
            unsafe { element.GetCachedPropertyValueEx(property, true) }.map_err(provider)?;
        unsafe {
            let raw = variant.as_raw();
            if raw.Anonymous.Anonymous.vt != 8 {
                return if matches!(raw.Anonymous.Anonymous.vt, 0 | 13) {
                    Ok(None)
                } else {
                    Err(evidence_rejected(EvidenceRejection::StringVariant))
                };
            }
            let bstr = std::mem::ManuallyDrop::new(windows::core::BSTR::from_raw(
                raw.Anonymous.Anonymous.Anonymous.bstrVal,
            ));
            let remaining = self.text_limit.saturating_sub(self.text_bytes);
            let result = bounded_utf16(bstr.as_wide(), remaining)?;
            self.text_bytes += result.len();
            self.check()?;
            Ok((!result.is_empty()).then_some(result))
        }
    }
    fn value(&mut self, element: &IUIAutomationElement, document: bool) -> Result<ValueRead, Stop> {
        self.check()?;
        let variant =
            match unsafe { element.GetCachedPropertyValueEx(UIA_ValueValuePropertyId, true) } {
                Ok(variant) => variant,
                Err(_) => {
                    self.check()?;
                    note_value_evidence(ValueEvidenceKind::UnknownGetter, None, document);
                    return Ok(ValueRead {
                        value: None,
                        kind: ValueEvidenceKind::UnknownGetter,
                    });
                }
            };
        self.check()?;
        let vt = unsafe { variant.as_raw().Anonymous.Anonymous.vt };
        let not_supported = if vt == 13 {
            self.check()?;
            let result = unsafe { self.automation.CheckNotSupported(&variant) }
                .map(|result| result.as_bool())
                .map_err(|_| ());
            self.check()?;
            result
        } else {
            Err(())
        };
        let kind = classify_cached_value_type(vt, not_supported);
        note_value_evidence(kind, Some(vt), document);
        if kind != ValueEvidenceKind::KnownString {
            return Ok(ValueRead { value: None, kind });
        }
        unsafe {
            let raw = variant.as_raw();
            let bstr = std::mem::ManuallyDrop::new(windows::core::BSTR::from_raw(
                raw.Anonymous.Anonymous.Anonymous.bstrVal,
            ));
            let remaining = self.text_limit.saturating_sub(self.text_bytes);
            let result = bounded_utf16(bstr.as_wide(), remaining)?;
            self.text_bytes += result.len();
            self.check()?;
            Ok(ValueRead {
                value: (!result.is_empty()).then_some(result),
                kind,
            })
        }
    }
    /// One hierarchy and its cached properties provide all mandatory evidence.
    /// The broad provider acquisition is logged without logging property values.
    pub fn acquire(
        &mut self,
        full_text: bool,
        subtree_rules: bool,
        need_url: bool,
    ) -> Result<AccessibilityNode, Stop> {
        let calls_before = self.calls;
        let result = self.acquire_inner(full_text, subtree_rules, need_url);
        if result.is_err() {
            self.raw_policy_evidence.clear();
            self.document_values.clear();
        }
        METRICS
            .calls
            .fetch_add((self.calls - calls_before) as u64, Ordering::Relaxed);
        METRICS
            .nodes
            .fetch_add(self.projected_nodes as u64, Ordering::Relaxed);
        METRICS
            .raw_nodes
            .fetch_add(self.nodes as u64, Ordering::Relaxed);
        result
    }
    fn acquire_inner(
        &mut self,
        full_text: bool,
        subtree_rules: bool,
        need_url: bool,
    ) -> Result<AccessibilityNode, Stop> {
        self.nodes = 0;
        self.projected_nodes = 0;
        self.text_bytes = 0;
        self.raw_policy_evidence.clear();
        self.document_values.clear();
        if !full_text && !subtree_rules && !need_url {
            self.check()?;
            return Ok(AccessibilityNode::default());
        }
        let properties = cache_properties(full_text, subtree_rules, need_url);
        unsafe {
            let cache = self.cache(&properties, TreeScope_Subtree)?;
            self.request()?;
            provider_stage("bulk_build_updated_cache");
            timing::enter(if full_text {
                Stage::RecordingCache
            } else {
                Stage::HierarchyCache
            });
            if full_text {
                METRICS
                    .recording_acquisitions
                    .fetch_add(1, Ordering::Relaxed);
            } else {
                METRICS.privacy_acquisitions.fetch_add(1, Ordering::Relaxed);
            }
            let root = self.root.BuildUpdatedCache(&cache).map_err(provider)?;
            self.check()?;
            timing::enter(Stage::CacheDecode);
            let result = self.decode(&root, full_text, subtree_rules, need_url);
            if !full_text {
                // A single subtree request must ask the provider for these
                // strings across every control, though decode scopes them.
                let broad = if subtree_rules { 2 } else { 1 };
                log_scope(
                    if subtree_rules {
                        "single_cache_broad_name_value"
                    } else {
                        "single_cache_broad_value"
                    },
                    broad,
                    &self.scope_rules,
                    self.nodes,
                );
            }
            let result = result?;
            self.check()?;
            Ok(result)
        }
    }
    unsafe fn require_string(
        &self,
        element: &IUIAutomationElement,
        property: UIA_PROPERTY_ID,
    ) -> Result<(), Stop> {
        let variant = element
            .GetCachedPropertyValueEx(property, true)
            .map_err(provider)?;
        required_string_type(variant.as_raw().Anonymous.Anonymous.vt)
    }
    unsafe fn decode(
        &mut self,
        element: &IUIAutomationElement,
        full: bool,
        subtree_rules: bool,
        need_url: bool,
    ) -> Result<AccessibilityNode, Stop> {
        let mut projected = Vec::with_capacity(1);
        // The old decoder retained the requested root in ControlView. Keep
        // that behavior even if its IsControlElement value is false.
        self.decode_into(
            element,
            0,
            full,
            subtree_rules,
            need_url,
            0,
            0,
            0,
            0,
            true,
            false,
            false,
            true,
            &mut projected,
        )?;
        if projected.len() != 1 {
            return Err(Stop::Provider);
        }
        Ok(projected.remove(0))
    }
    unsafe fn decode_into(
        &mut self,
        element: &IUIAutomationElement,
        raw_depth: usize,
        full: bool,
        subtree_rules: bool,
        need_url: bool,
        control_extension_levels: usize,
        raw_extension_levels: usize,
        control_depth: usize,
        policy_depth: usize,
        policy_visible: bool,
        shallow_visible: bool,
        raw_extension_origin: bool,
        root: bool,
        out: &mut Vec<AccessibilityNode>,
    ) -> Result<(), Stop> {
        // RawView can have more nodes and deeper paths than ControlView. Apply
        // both bounds to raw traversal, conservatively refusing incomplete work.
        node_budget(
            Instant::now(),
            self.deadline,
            self.nodes,
            self.max_nodes,
            raw_depth,
            MAX_RAW_DEPTH,
        )?;
        self.nodes += 1;
        let control = cached_control_type(
            &element
                .GetCachedPropertyValueEx(UIA_ControlTypePropertyId, true)
                .map_err(provider)?,
        )?;
        // FALSE applies UIA's default TRUE if the provider omits the property,
        // matching ControlViewCondition's effective membership.
        let is_control = cached_control_element(
            &element
                .GetCachedPropertyValueEx(UIA_IsControlElementPropertyId, false)
                .map_err(provider)?,
        )?;
        let password = cached_password(
            &element
                .GetCachedPropertyValueEx(UIA_IsPasswordPropertyId, true)
                .map_err(provider)?,
        )?;
        let action = raw_node_action(root, is_control);
        let policy_scope = raw_policy_scope(
            root,
            is_control,
            control,
            subtree_rules,
            need_url,
            raw_extension_levels,
            policy_visible,
            shallow_visible,
            raw_extension_origin,
        );
        let document_url = document_url_scope(control, subtree_rules, need_url);
        meaningful_depth_budget(control_depth, policy_depth, self.max_depth)?;
        let child_control_depth = control_depth + usize::from(action == RawNodeAction::Keep);
        let control_scope = projected_rule_scope(
            action,
            control,
            subtree_rules,
            need_url,
            control_extension_levels,
        );
        if control_scope.is_some_and(|scope| scope.required_name) || policy_scope.required_name {
            self.require_string(element, UIA_NamePropertyId)?;
        }
        let projected_name = action == RawNodeAction::Keep
            && (full || control_scope.is_some_and(|scope| scope.name));
        let projected_value = action == RawNodeAction::Keep
            && (full || control_scope.is_some_and(|scope| scope.value));
        let name = if projected_name || policy_scope.name {
            self.string(element, UIA_NamePropertyId)?
        } else {
            None
        };
        let value_read = if projected_value || policy_scope.value || document_url {
            Some(self.value(element, control == UIA_DocumentControlTypeId.0)?)
        } else {
            None
        };
        let value_kind = value_read.as_ref().map(|read| read.kind);
        let value = value_read.and_then(|read| read.value);
        if document_url {
            let kind = value_kind.ok_or(Stop::Provider)?;
            self.document_values.push(DocumentValueObservation {
                kind,
                http_url: document_http_url(value.as_deref()).map(str::to_owned),
            });
        }
        let retained_pruned = action == RawNodeAction::Keep
            && retained_control_prunes_policy(control, password, value.as_deref());
        let child_policy_visible = policy_visible && !retained_pruned;
        let meaningful_policy_node = counts_policy_depth(
            action,
            policy_scope,
            document_url,
            name.as_deref(),
            value.as_deref(),
        );
        let child_policy_depth = policy_depth + usize::from(meaningful_policy_node);
        if policy_scope.nested_window {
            self.raw_policy_evidence
                .push(RawPolicyEvidence::NestedWindow { name: name.clone() });
        }
        if policy_scope.document {
            self.raw_policy_evidence.push(RawPolicyEvidence::Document {
                name: name.clone(),
                value: value.clone(),
            });
        }
        if additional_document_url_evidence(action, policy_scope, document_url) {
            self.raw_policy_evidence
                .push(RawPolicyEvidence::DocumentUrl {
                    value: value.clone(),
                });
        }
        if policy_scope.extension_shallow {
            self.raw_policy_evidence
                .push(RawPolicyEvidence::ExtensionShallow {
                    name: name.clone(),
                    value: value.clone(),
                });
        }
        let (
            child_control_extension_levels,
            child_raw_extension_levels,
            child_raw_extension_origin,
            child_shallow_visible,
        ) = child_extension_state(
            control,
            subtree_rules,
            name.as_deref(),
            value.as_deref(),
            control_extension_levels,
            raw_extension_levels,
            raw_extension_origin,
            action,
            child_policy_visible,
            shallow_visible,
            policy_scope,
        );
        if action == RawNodeAction::Flatten {
            // A non-control ancestor is transparent in ControlView. Its Edit,
            // password, or decorative role cannot prune retained descendants.
            self.decode_children(
                element,
                raw_depth,
                full,
                subtree_rules,
                need_url,
                control_extension_levels,
                child_raw_extension_levels,
                child_control_depth,
                child_policy_depth,
                policy_visible,
                child_shallow_visible,
                child_raw_extension_origin,
                out,
            )?;
            return Ok(());
        }
        self.projected_nodes += 1;
        let mut node = AccessibilityNode {
            control_type: crate::platform::windows_uia::control_type_id_to_name(control)
                .to_string(),
            name: if projected_name { name } else { None },
            value: if projected_value { value } else { None },
            is_password: password,
            ..Default::default()
        };
        if full {
            node.automation_id = self.string(element, UIA_AutomationIdPropertyId)?;
            node.class_name = self.string(element, UIA_ClassNamePropertyId)?;
            node.help_text = self.string(element, UIA_HelpTextPropertyId)?;
            node.accelerator_key = self.string(element, UIA_AcceleratorKeyPropertyId)?;
            node.access_key = self.string(element, UIA_AccessKeyPropertyId)?;
            node.localized_control_type =
                self.string(element, UIA_LocalizedControlTypePropertyId)?;
            node.is_enabled = bool::try_from(
                &element
                    .GetCachedPropertyValueEx(UIA_IsEnabledPropertyId, true)
                    .map_err(provider)?,
            )
            .unwrap_or(false);
            node.is_focused = bool::try_from(
                &element
                    .GetCachedPropertyValueEx(UIA_HasKeyboardFocusPropertyId, true)
                    .map_err(provider)?,
            )
            .ok();
            node.is_keyboard_focusable = bool::try_from(
                &element
                    .GetCachedPropertyValueEx(UIA_IsKeyboardFocusablePropertyId, true)
                    .map_err(provider)?,
            )
            .ok();
            node.bounds = bulk_cached_bounds(element);
        }
        self.decode_children(
            element,
            raw_depth,
            full,
            subtree_rules,
            need_url,
            child_control_extension_levels,
            child_raw_extension_levels,
            child_control_depth,
            child_policy_depth,
            child_policy_visible,
            child_shallow_visible,
            child_raw_extension_origin,
            &mut node.children,
        )?;
        out.push(node);
        Ok(())
    }
    unsafe fn decode_children(
        &mut self,
        element: &IUIAutomationElement,
        raw_depth: usize,
        full: bool,
        subtree_rules: bool,
        need_url: bool,
        control_extension_levels: usize,
        raw_extension_levels: usize,
        control_depth: usize,
        policy_depth: usize,
        policy_visible: bool,
        shallow_visible: bool,
        raw_extension_origin: bool,
        out: &mut Vec<AccessibilityNode>,
    ) -> Result<(), Stop> {
        // GetCachedChildren is a local cache read. Null means a complete leaf;
        // HRESULT failure remains unknown, never a partial successful tree.
        let mut raw = std::ptr::null_mut();
        (Interface::vtable(element).GetCachedChildren)(Interface::as_raw(element), &mut raw)
            .ok()
            .map_err(provider)?;
        if !raw.is_null() {
            let children = IUIAutomationElementArray::from_raw(raw);
            let length = children.Length().map_err(provider)?;
            if length < 0 {
                return Err(evidence_rejected(EvidenceRejection::NegativeChildCount));
            }
            for index in 0..length {
                self.check()?;
                let child = children.GetElement(index).map_err(provider)?;
                self.decode_into(
                    &child,
                    raw_depth + 1,
                    full,
                    subtree_rules,
                    need_url,
                    control_extension_levels,
                    raw_extension_levels,
                    control_depth,
                    policy_depth,
                    policy_visible,
                    shallow_visible,
                    raw_extension_origin,
                    false,
                    out,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::HashSet;
    use std::time::Duration;

    #[test]
    fn evidence_rejection_is_counted_and_remains_fail_closed() {
        let index = EvidenceRejection::RequiredStringType as usize;
        let before = EVIDENCE_REJECTIONS[index].load(Ordering::Relaxed);
        assert_eq!(
            evidence_rejected(EvidenceRejection::RequiredStringType),
            Stop::Provider
        );
        let snapshot = metrics_snapshot();
        assert_eq!(
            snapshot["evidence_rejection_codes"][index],
            "required_string_type"
        );
        assert!(
            snapshot["evidence_rejection_counts"][index]
                .as_u64()
                .unwrap()
                > before
        );
        assert_eq!(EVIDENCE_REJECTION_CODES.len(), EVIDENCE_REJECTIONS.len());
        assert_eq!(
            EVIDENCE_REJECTION_CODES
                .iter()
                .collect::<HashSet<_>>()
                .len(),
            EVIDENCE_REJECTION_CODES.len()
        );
        for obsolete in [
            EvidenceRejection::ConflictingProjection,
            EvidenceRejection::ProjectionOutsideHierarchy,
            EvidenceRejection::RuntimeIdVariant,
            EvidenceRejection::RuntimeIdArrayLayout,
            EvidenceRejection::RuntimeIdLength,
            EvidenceRejection::DuplicateQueryRuntimeId,
            EvidenceRejection::DuplicateValueRuntimeId,
            EvidenceRejection::DuplicateHierarchyRuntimeId,
            EvidenceRejection::MissingTitleProjection,
            EvidenceRejection::MissingDocumentValue,
            EvidenceRejection::MissingEntryValue,
            EvidenceRejection::MissingExtensionValue,
        ] {
            assert_eq!(snapshot["evidence_rejection_counts"][obsolete as usize], 0);
        }
        assert_eq!(
            snapshot["evidence_rejection_codes"][19],
            "control_element_variant"
        );
        assert_eq!(
            snapshot["evidence_rejection_codes"][20],
            "raw_only_policy_element"
        );
        assert_eq!(
            snapshot["acquisition_mode"],
            "single_raw_cache_control_projection"
        );
        assert_eq!(snapshot["schema_version"], 3);
        assert!(snapshot["raw_nodes"].is_u64());
        assert!(snapshot["projected_nodes"].is_u64());
    }

    #[test]
    fn provider_timeout_uses_remaining_phase_time_without_overrun() {
        let now = Instant::now();
        assert_eq!(
            provider_timeout_at(now, now + Duration::from_millis(750)),
            Ok(750)
        );
        assert_eq!(
            provider_timeout_at(now, now + Duration::from_millis(125)),
            Ok(125)
        );
        assert_eq!(
            provider_timeout_at(now, now + Duration::from_millis(50)),
            Ok(50)
        );
        assert_eq!(
            provider_timeout_at(
                now,
                now + Duration::from_millis(50) - Duration::from_micros(1)
            ),
            Err(Stop::Timeout)
        );
        assert_eq!(
            provider_timeout_at(
                now,
                now + Duration::from_millis(750) - Duration::from_micros(100)
            ),
            Ok(749)
        );
        assert_eq!(provider_timeout_at(now, now), Err(Stop::Timeout));
    }

    #[test]
    fn fresh_bulk_zero_or_expired_budget_never_acquires() {
        let now = Instant::now();
        let calls = Cell::new(0);
        let result = fresh_until_current(
            now,
            || now,
            || {
                calls.set(calls.get() + 1);
                Ok(Some(()))
            },
        );
        assert_eq!(result, Err(Stop::Timeout));
        assert_eq!(calls.get(), 0);
        assert_eq!(
            node_budget(now, now + Duration::from_secs(1), 0, 0, 0, 35),
            Err(Stop::Nodes)
        );
    }
    #[test]
    fn expired_initial_deadline_marks_the_active_stage() {
        let now = Instant::now();
        timing::begin(now, now, now);
        timing::enter(Stage::BulkSetup);
        assert_eq!(initial_deadline(now), Err(Stop::Timeout));
        assert_eq!(
            timing::current_timeout_stage_for_test(),
            Some(Stage::BulkSetup)
        );
        timing::finish(now, Duration::ZERO, Duration::ZERO, None, &[]);
    }
    #[test]
    fn fresh_bulk_late_success_and_decoding_are_never_published() {
        let start = Instant::now();
        let deadline = start + Duration::from_millis(100);
        for elapsed in [100, 101] {
            let now = Cell::new(start);
            let result = fresh_until_current(
                deadline,
                || now.get(),
                || {
                    now.set(start + Duration::from_millis(elapsed));
                    Ok(Some("candidate"))
                },
            );
            assert_eq!(result, Err(Stop::Timeout));
        }
    }
    #[test]
    fn fresh_bulk_invalidated_attempt_reacquires_inside_original_deadline() {
        let start = Instant::now();
        let now = Cell::new(start);
        let calls = Cell::new(0);
        let result = fresh_until_current(
            start + Duration::from_millis(100),
            || now.get(),
            || {
                calls.set(calls.get() + 1);
                now.set(now.get() + Duration::from_millis(40));
                Ok(if calls.get() == 1 { None } else { Some(2) })
            },
        );
        assert_eq!(result, Ok(2));
        assert_eq!(calls.get(), 2);
        assert_eq!(now.get(), start + Duration::from_millis(80));
    }
    #[test]
    fn fresh_bulk_invalidations_never_restart_deadline() {
        let start = Instant::now();
        let now = Cell::new(start);
        let calls = Cell::new(0);
        let result: Result<(), Stop> = fresh_until_current(
            start + Duration::from_millis(100),
            || now.get(),
            || {
                calls.set(calls.get() + 1);
                now.set(now.get() + Duration::from_millis(40));
                Ok(None)
            },
        );
        assert_eq!(result, Err(Stop::Timeout));
        assert_eq!(calls.get(), 3);
    }
    #[test]
    fn fresh_bulk_provider_errors_never_become_partial_success() {
        let now = Instant::now();
        let result: Result<(), Stop> =
            fresh_until_current(now + Duration::from_secs(1), || now, || Err(Stop::Provider));
        assert_eq!(result, Err(Stop::Provider));
    }
    #[test]
    fn fresh_bulk_required_name_and_partial_value_are_distinct() {
        assert_eq!(required_string_type(8), Ok(())); // includes empty BSTR
        for vt in [0, 13, 3, 11, 0x4008] {
            assert_eq!(required_string_type(vt), Err(Stop::Provider));
        }
        assert_eq!(
            classify_cached_value_type(8, Err(())),
            ValueEvidenceKind::KnownString
        );
        assert_eq!(
            classify_cached_value_type(13, Ok(true)),
            ValueEvidenceKind::KnownUnavailable
        );
        assert_eq!(
            classify_cached_value_type(13, Ok(false)),
            ValueEvidenceKind::UnknownVariant
        );
        assert_eq!(
            classify_cached_value_type(13, Err(())),
            ValueEvidenceKind::UnknownSentinelCheck
        );
        for vt in [0, 3, 11, 0x4008] {
            assert_eq!(
                classify_cached_value_type(vt, Ok(true)),
                ValueEvidenceKind::UnknownVariant
            );
        }
        assert_eq!(VALUE_EVIDENCE_CODES.len(), VALUE_EVIDENCE_COUNTS.len());
    }
    #[test]
    fn fresh_bulk_document_url_loss_requires_same_unknown_sources() {
        let observed = |kind, url: Option<&str>| DocumentValueObservation {
            kind,
            http_url: url.map(str::to_owned),
        };
        assert_eq!(
            document_http_url(Some(" https://private.example/page ")),
            Some("https://private.example/page")
        );
        assert_eq!(document_http_url(Some("about:blank")), None);
        let mandatory = [
            observed(
                ValueEvidenceKind::KnownString,
                Some("https://private.example/page"),
            ),
            observed(ValueEvidenceKind::KnownUnavailable, None),
            observed(
                ValueEvidenceKind::KnownString,
                Some("https://private.example/page"),
            ),
        ];
        let sources = document_url_sources(&mandatory, Some("https://private.example/page"));
        assert_eq!(sources, [0, 2]);
        assert!(document_url_sources(&mandatory, None).is_empty());
        let optional_unknown = [
            observed(ValueEvidenceKind::UnknownGetter, None),
            observed(
                ValueEvidenceKind::KnownString,
                Some("https://other.example"),
            ),
            observed(ValueEvidenceKind::UnknownVariant, None),
        ];
        assert!(document_url_loss_is_unknown(
            &optional_unknown,
            mandatory.len(),
            &sources,
        ));
        let optional_unavailable = [
            observed(ValueEvidenceKind::KnownUnavailable, None),
            observed(ValueEvidenceKind::KnownString, None),
            observed(ValueEvidenceKind::UnknownSentinelCheck, None),
        ];
        assert!(document_url_loss_is_unknown(
            &optional_unavailable,
            mandatory.len(),
            &sources,
        ));
        assert!(!document_url_loss_is_unknown(
            &optional_unknown[..2],
            mandatory.len(),
            &sources,
        ));
        assert!(!document_url_loss_is_unknown(
            &optional_unknown,
            mandatory.len(),
            &[],
        ));
        assert!(!document_url_loss_is_unknown(
            &optional_unknown,
            mandatory.len(),
            &[3],
        ));
        for current in [
            [
                observed(ValueEvidenceKind::KnownString, None), // known empty
                observed(ValueEvidenceKind::KnownUnavailable, None),
                observed(ValueEvidenceKind::UnknownVariant, None),
            ],
            [
                observed(ValueEvidenceKind::KnownString, None), // known non-HTTP
                observed(ValueEvidenceKind::KnownUnavailable, None),
                observed(ValueEvidenceKind::UnknownVariant, None),
            ],
            [
                observed(ValueEvidenceKind::UnknownGetter, None),
                observed(ValueEvidenceKind::KnownUnavailable, None),
                observed(
                    ValueEvidenceKind::KnownString,
                    Some("https://changed.example"),
                ),
            ],
        ] {
            assert!(!document_url_loss_is_unknown(
                &current,
                mandatory.len(),
                &sources,
            ));
        }
    }
    #[test]
    fn fresh_bulk_structural_properties_reject_coercible_wrong_tags() {
        assert_eq!(
            cached_control_type(&VARIANT::from(UIA_DocumentControlTypeId.0)),
            Ok(UIA_DocumentControlTypeId.0)
        );
        for value in [
            VARIANT::from("50030"),
            VARIANT::from(true),
            VARIANT::from(49999i32),
        ] {
            assert_eq!(cached_control_type(&value), Err(Stop::Provider));
        }
        assert_eq!(cached_password(&VARIANT::from(false)), Ok(Some(false)));
        assert_eq!(cached_password(&VARIANT::from(true)), Ok(Some(true)));
        assert_eq!(cached_password(&VARIANT::default()), Ok(None));
        for value in [
            VARIANT::from("false"),
            VARIANT::from(0i32),
            VARIANT::from(1i32),
        ] {
            assert_eq!(cached_password(&value), Err(Stop::Provider));
        }
        assert_eq!(cached_control_element(&VARIANT::from(true)), Ok(true));
        assert_eq!(cached_control_element(&VARIANT::from(false)), Ok(false));
        for value in [
            VARIANT::default(),
            VARIANT::from("true"),
            VARIANT::from(1i32),
        ] {
            assert_eq!(cached_control_element(&value), Err(Stop::Provider));
        }
    }
    #[test]
    fn fresh_bulk_node_depth_boundaries_are_complete_or_failed() {
        let now = Instant::now();
        let deadline = now + Duration::from_secs(1);
        assert_eq!(
            node_budget(now, deadline, 4999, 5000, 128, MAX_RAW_DEPTH),
            Ok(())
        );
        assert_eq!(
            node_budget(now, deadline, 5000, 5000, 128, MAX_RAW_DEPTH),
            Err(Stop::Nodes)
        );
        assert_eq!(
            node_budget(now, deadline, 0, 5000, 129, MAX_RAW_DEPTH),
            Err(Stop::Depth)
        );
        assert_eq!(
            node_budget(now, deadline, 0, 5000, 80, MAX_RAW_DEPTH),
            Ok(())
        );
        assert_eq!(meaningful_depth_budget(17, 12, 35), Ok(()));
        assert_eq!(meaningful_depth_budget(36, 12, 35), Err(Stop::Depth));
        assert_eq!(meaningful_depth_budget(12, 36, 35), Err(Stop::Depth));
        let mut policy_depth = 0;
        for raw_depth in 1..=80 {
            assert_eq!(
                node_budget(now, deadline, raw_depth, 5000, raw_depth, MAX_RAW_DEPTH),
                Ok(())
            );
            assert!(!counts_policy_depth(
                RawNodeAction::Flatten,
                RawPolicyScope::default(),
                false,
                None,
                None,
            ));
            assert_eq!(meaningful_depth_budget(0, policy_depth, 35), Ok(()));
        }
        let nested_window = raw_policy_scope(
            false,
            false,
            UIA_WindowControlTypeId.0,
            true,
            false,
            0,
            true,
            false,
            false,
        );
        for _ in 0..=35 {
            assert_eq!(meaningful_depth_budget(0, policy_depth, 35), Ok(()));
            policy_depth += usize::from(counts_policy_depth(
                RawNodeAction::Flatten,
                nested_window,
                false,
                Some("nested window"),
                None,
            ));
        }
        assert_eq!(
            meaningful_depth_budget(0, policy_depth, 35),
            Err(Stop::Depth)
        );
    }
    #[test]
    fn fresh_bulk_single_cache_property_union_never_requires_runtime_id() {
        let ids = |full, subtree, url| {
            cache_properties(full, subtree, url)
                .iter()
                .map(|property| property.0)
                .collect::<Vec<_>>()
        };
        let control = UIA_ControlTypePropertyId.0;
        let password = UIA_IsPasswordPropertyId.0;
        let membership = UIA_IsControlElementPropertyId.0;
        let name = UIA_NamePropertyId.0;
        let value = UIA_ValueValuePropertyId.0;
        assert_eq!(ids(false, false, false), [control, password, membership]);
        assert_eq!(
            ids(false, false, true),
            [control, password, membership, value]
        );
        assert_eq!(
            ids(false, true, false),
            [control, password, membership, name, value]
        );
        assert_eq!(
            ids(false, true, true),
            [control, password, membership, name, value]
        );
        for (full, subtree, url) in [
            (false, false, true),
            (false, true, false),
            (true, false, false),
        ] {
            let requested = ids(full, subtree, url);
            assert!(!requested.contains(&UIA_RuntimeIdPropertyId.0));
            assert_eq!(
                requested.iter().copied().collect::<HashSet<_>>().len(),
                requested.len()
            );
        }
        let full = ids(true, false, false);
        assert_eq!(full.len(), 15);
        assert_eq!(cache_property_count(true, false, false), 15);
        assert_eq!(cache_property_count(true, true, false), 15);
        assert_eq!(cache_property_count(true, false, true), 15);
        for property in [
            name,
            value,
            UIA_AutomationIdPropertyId.0,
            UIA_ClassNamePropertyId.0,
            UIA_BoundingRectanglePropertyId.0,
            UIA_IsEnabledPropertyId.0,
            UIA_HasKeyboardFocusPropertyId.0,
            UIA_IsKeyboardFocusablePropertyId.0,
            UIA_HelpTextPropertyId.0,
            UIA_AcceleratorKeyPropertyId.0,
            UIA_AccessKeyPropertyId.0,
            UIA_LocalizedControlTypePropertyId.0,
        ] {
            assert!(full.contains(&property));
        }
        assert!(!full.contains(&UIA_IsValuePatternAvailablePropertyId.0));
        assert!(!ids(true, true, false).contains(&UIA_IsValuePatternAvailablePropertyId.0));
    }
    #[test]
    fn fresh_bulk_mandatory_cache_properties_all_invalidate_revision() {
        let subscribed: HashSet<_> = invalidation_properties()
            .iter()
            .map(|property| property.0)
            .collect();
        assert_eq!(subscribed.len(), 5);
        for (subtree_rules, need_url) in [(true, false), (false, true), (true, true)] {
            for property in cache_properties(false, subtree_rules, need_url) {
                assert!(
                    subscribed.contains(&property.0),
                    "mandatory cache property {} lacks invalidation",
                    property.0
                );
            }
        }
        let revision = Arc::new(AtomicU64::new(0));
        let handler: IUIAutomationPropertyChangedEventHandler = Property(revision.clone()).into();
        unsafe {
            handler
                .HandlePropertyChangedEvent(
                    None::<&IUIAutomationElement>,
                    UIA_IsControlElementPropertyId,
                    &VARIANT::from(false),
                )
                .unwrap();
            handler
                .HandlePropertyChangedEvent(
                    None::<&IUIAutomationElement>,
                    UIA_ValueValuePropertyId,
                    &VARIANT::from("fixture"),
                )
                .unwrap();
        }
        assert_eq!(revision.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn fresh_bulk_raw_only_rule_scope_preserves_control_projection() {
        let window = UIA_WindowControlTypeId.0;
        let document = UIA_DocumentControlTypeId.0;
        let edit = UIA_EditControlTypeId.0;
        let combo = UIA_ComboBoxControlTypeId.0;
        let pane = UIA_PaneControlTypeId.0;
        let image = UIA_ImageControlTypeId.0;
        let scope = |root, control_member, kind, subtree, url, extension| {
            raw_policy_scope(
                root,
                control_member,
                kind,
                subtree,
                url,
                extension,
                true,
                true,
                false,
            )
        };
        // The requested root remains a projected control even if its cached
        // membership is false; raw-only rule evidence starts below it.
        assert_eq!(raw_node_action(true, false), RawNodeAction::Keep);
        assert_eq!(
            scope(true, false, window, true, false, 0),
            RawPolicyScope::default()
        );
        assert!(rule_property_scope(window, true, false, 0).required_name);
        assert_eq!(
            scope(true, false, document, false, true, 0),
            RawPolicyScope::default()
        );
        assert!(rule_property_scope(document, false, true, 0).value);
        assert_eq!(raw_node_action(false, true), RawNodeAction::Keep);
        assert_eq!(raw_node_action(false, false), RawNodeAction::Flatten);
        assert!(document_url_scope(document, false, true));
        assert!(document_url_scope(document, true, false));
        assert!(!document_url_scope(document, false, false));
        assert!(!document_url_scope(pane, true, true));
        // URL candidates also count as meaningful policy nodes when the
        // Document is raw-only and hidden from subtree Name matching.
        let hidden = RawPolicyScope::default();
        assert!(counts_policy_depth(
            RawNodeAction::Flatten,
            hidden,
            document_url_scope(document, true, true),
            None,
            None,
        ));
        assert!(!counts_policy_depth(
            RawNodeAction::Flatten,
            hidden,
            document_url_scope(pane, true, true),
            None,
            None,
        ));
        let raw_window = scope(false, false, window, true, false, 0);
        assert!(raw_window.nested_window && raw_window.required_name);
        assert!(!raw_window.value);
        let raw_document = scope(false, false, document, true, false, 0);
        assert!(raw_document.document && raw_document.required_name && raw_document.value);
        assert!(!additional_document_url_evidence(
            RawNodeAction::Flatten,
            raw_document,
            document_url_scope(document, true, false),
        ));
        assert!(additional_document_url_evidence(
            RawNodeAction::Flatten,
            hidden,
            document_url_scope(document, true, false),
        ));
        assert!(additional_document_url_evidence(
            RawNodeAction::Keep,
            hidden,
            document_url_scope(document, true, false),
        ));
        assert_eq!(
            classify_cached_value_type(13, Ok(true)),
            ValueEvidenceKind::KnownUnavailable
        );
        let url_document = scope(false, false, document, false, true, 0);
        assert!(url_document.document && url_document.value);
        assert!(!url_document.name);
        assert_eq!(
            scope(false, false, window, false, true, 0),
            RawPolicyScope::default()
        );
        // Entry values are not direct window-pattern or URL candidates. They
        // are in scope only as extension children; irrelevant raw-only roles
        // do not force a broad string probe or prune retained descendants.
        for kind in [edit, combo, pane, image] {
            assert_eq!(
                scope(false, false, kind, true, false, 0),
                RawPolicyScope::default()
            );
        }
        for level in [1, 2] {
            let raw_child = scope(false, false, pane, true, false, level);
            assert!(raw_child.extension_shallow && raw_child.required_name && raw_child.value);
            let control_child = scope(false, true, edit, true, false, level);
            assert!(!control_child.extension_shallow);
            let raw_origin_control =
                raw_policy_scope(false, true, edit, true, false, level, true, true, true);
            assert!(raw_origin_control.extension_shallow && raw_origin_control.required_name);
        }
        assert_eq!(
            scope(false, false, pane, true, false, 0),
            RawPolicyScope::default()
        );
        assert_eq!(
            scope(false, false, pane, false, true, 2),
            RawPolicyScope::default()
        );
        assert_eq!(
            raw_policy_scope(false, false, document, true, false, 0, false, false, false),
            RawPolicyScope::default()
        );
        assert!(
            raw_policy_scope(false, false, pane, true, false, 1, false, true, false)
                .extension_shallow
        );
        assert!(retained_control_prunes_policy(image, Some(false), None));
        assert!(retained_control_prunes_policy(pane, Some(true), None));
        assert!(retained_control_prunes_policy(edit, None, None));
        assert!(retained_control_prunes_policy(
            edit,
            Some(false),
            Some("entry text")
        ));
        assert!(!retained_control_prunes_policy(
            edit,
            Some(false),
            Some("  ")
        ));
        assert!(!retained_control_prunes_policy(
            pane,
            Some(false),
            Some("content")
        ));
    }
    #[test]
    fn fresh_bulk_extension_depth_tracks_raw_and_control_hierarchies_separately() {
        let document = UIA_DocumentControlTypeId.0;
        let pane = UIA_PaneControlTypeId.0;
        let extension_url = Some("chrome-extension://fixed-test-id/page");
        let scope = RawPolicyScope::default();
        let (control, raw, origin, shallow) = child_extension_state(
            document,
            true,
            None,
            extension_url,
            0,
            0,
            false,
            RawNodeAction::Keep,
            true,
            false,
            scope,
        );
        assert_eq!((control, raw), (2, 2));
        assert!(!origin && shallow);
        assert!(retained_control_prunes_policy(
            document,
            Some(true),
            extension_url
        ));
        let hidden = child_extension_state(
            document,
            true,
            None,
            extension_url,
            0,
            0,
            false,
            RawNodeAction::Keep,
            false,
            false,
            scope,
        );
        assert_eq!(hidden, (2, 0, false, false));
        // Empty RawView layout wrappers do not consume the two meaningful
        // shallow levels or the ControlView level.
        let empty_wrapper =
            raw_policy_scope(false, false, pane, true, false, raw, true, shallow, origin);
        let (control, raw, origin, shallow) = child_extension_state(
            pane,
            true,
            None,
            None,
            control,
            raw,
            origin,
            RawNodeAction::Flatten,
            true,
            shallow,
            empty_wrapper,
        );
        assert_eq!((control, raw), (2, 2));
        assert!(shallow && !origin);
        let (control, raw, origin, shallow) = child_extension_state(
            pane,
            true,
            Some("visible shallow text"),
            None,
            control,
            raw,
            origin,
            RawNodeAction::Flatten,
            true,
            shallow,
            empty_wrapper,
        );
        assert_eq!((control, raw), (2, 1));
        let (control, raw, origin, shallow) = child_extension_state(
            pane,
            true,
            Some("second meaningful level"),
            None,
            control,
            raw,
            origin,
            RawNodeAction::Flatten,
            true,
            shallow,
            empty_wrapper,
        );
        assert_eq!((control, raw), (2, 0));
        assert!(!shallow);
        assert!(
            !raw_policy_scope(false, false, pane, true, false, raw, true, shallow, origin)
                .extension_shallow
        );
        // A raw-only Document starts its own shallow scan while remaining
        // absent from the projected hierarchy.
        let (control, raw, origin, shallow) = child_extension_state(
            document,
            true,
            None,
            extension_url,
            0,
            0,
            false,
            RawNodeAction::Flatten,
            true,
            false,
            scope,
        );
        assert_eq!((control, raw), (0, 2));
        assert!(origin && shallow);
        let descendant =
            raw_policy_scope(false, true, pane, true, false, raw, true, shallow, origin);
        assert!(descendant.extension_shallow);
    }
    #[test]
    fn fresh_bulk_projected_extension_grandchild_scope_survives_pruned_parent() {
        let document = UIA_DocumentControlTypeId.0;
        let text = UIA_TextControlTypeId.0;
        let extension_url = Some("chrome-extension://fixed-test-id/page");
        let (document_control_levels, document_raw_levels, raw_origin, shallow_visible) =
            child_extension_state(
                document,
                true,
                None,
                extension_url,
                0,
                0,
                false,
                RawNodeAction::Keep,
                true,
                false,
                RawPolicyScope::default(),
            );
        assert_eq!(document_control_levels, 2);
        let shallow_scope = RulePropertyScope {
            name: true,
            required_name: true,
            value: true,
        };
        // The ControlView extension fallback reads child and grandchild
        // properties even if normal text extraction prunes the child.
        for (pruned_kind, password, value) in [
            (UIA_ImageControlTypeId.0, Some(false), None),
            (UIA_PaneControlTypeId.0, Some(true), None),
            (UIA_EditControlTypeId.0, Some(false), Some("entry text")),
        ] {
            assert!(retained_control_prunes_policy(pruned_kind, password, value));
            assert_eq!(
                projected_rule_scope(
                    RawNodeAction::Keep,
                    pruned_kind,
                    true,
                    false,
                    document_control_levels,
                ),
                Some(shallow_scope)
            );
            let (grandchild_control_levels, _, _, _) = child_extension_state(
                pruned_kind,
                true,
                None,
                value,
                document_control_levels,
                document_raw_levels,
                raw_origin,
                RawNodeAction::Keep,
                false,
                shallow_visible,
                RawPolicyScope::default(),
            );
            assert_eq!(grandchild_control_levels, 1);
            assert_eq!(
                projected_rule_scope(
                    RawNodeAction::Keep,
                    text,
                    true,
                    false,
                    grandchild_control_levels,
                ),
                Some(shallow_scope)
            );
            assert_eq!(
                projected_rule_scope(
                    RawNodeAction::Flatten,
                    text,
                    true,
                    false,
                    grandchild_control_levels,
                ),
                None
            );
            assert_eq!(
                next_extension_levels(text, true, None, grandchild_control_levels),
                0
            );
            assert!(
                !raw_policy_scope(
                    false,
                    true,
                    document,
                    true,
                    false,
                    grandchild_control_levels,
                    false,
                    false,
                    false,
                )
                .document
            );
        }
    }
    #[test]
    fn fresh_bulk_rule_scope_covers_nested_titles_urls_and_extension_depth() {
        let window = UIA_WindowControlTypeId.0;
        let document = UIA_DocumentControlTypeId.0;
        let edit = UIA_EditControlTypeId.0;
        let combo = UIA_ComboBoxControlTypeId.0;
        let text = UIA_TextControlTypeId.0;
        let button = UIA_ButtonControlTypeId.0;
        let scope =
            |control, subtree, url, level| rule_property_scope(control, subtree, url, level);
        let title = RulePropertyScope {
            name: true,
            required_name: true,
            value: false,
        };
        let document_scope = RulePropertyScope {
            value: true,
            ..title
        };
        let entry_scope = RulePropertyScope {
            name: false,
            required_name: false,
            value: true,
        };
        let blank = RulePropertyScope {
            name: false,
            required_name: false,
            value: false,
        };
        assert_eq!(scope(window, true, false, 0), title);
        assert_eq!(scope(document, true, false, 0), document_scope);
        for control in [edit, combo] {
            assert_eq!(scope(control, true, false, 0), entry_scope);
            assert_eq!(scope(control, false, true, 0), entry_scope);
        }
        assert_eq!(
            scope(document, false, true, 0),
            RulePropertyScope {
                name: false,
                required_name: false,
                value: true,
            }
        );
        assert_eq!(scope(window, false, true, 0), blank);
        assert_eq!(scope(text, true, false, 0), blank);
        assert_eq!(scope(button, true, false, 0), blank);
        for level in [1, 2] {
            assert_eq!(
                scope(text, true, false, level),
                RulePropertyScope {
                    name: true,
                    required_name: true,
                    value: true,
                }
            );
            assert_eq!(
                scope(button, true, false, level),
                RulePropertyScope {
                    name: true,
                    required_name: true,
                    value: true,
                }
            );
        }
        assert_eq!(scope(text, false, true, 2), blank);
        let extension = Some("chrome-extension://fixed-test-id/page");
        let child_level = next_extension_levels(document, true, extension, 0);
        assert_eq!(child_level, 2);
        let grandchild_level = next_extension_levels(text, true, None, child_level);
        assert_eq!(grandchild_level, 1);
        assert_eq!(next_extension_levels(text, true, None, grandchild_level), 0);
        assert_eq!(scope(text, true, false, 0), blank);
        assert_eq!(
            next_extension_levels(document, true, extension, grandchild_level),
            2
        );
        assert_eq!(next_extension_levels(document, false, extension, 0), 0);
    }
    #[test]
    fn fresh_bulk_unicode_limits_never_publish_truncated_rule_strings() {
        let unicode: Vec<u16> = "é🙂".encode_utf16().collect();
        assert_eq!(bounded_utf16(&unicode, 6), Ok("é🙂".to_string()));
        assert_eq!(bounded_utf16(&unicode, 5), Err(Stop::AggregateText));
        assert_eq!(
            bounded_utf16(&vec![b'x' as u16; 16385], 20000),
            Err(Stop::PropertyText)
        );
        assert_eq!(bounded_utf16(&[], 0), Ok(String::new()));
    }
}

unsafe fn bulk_cached_bounds(
    element: &IUIAutomationElement,
) -> Option<crate::events::ElementBounds> {
    let variant = element
        .GetCachedPropertyValueEx(UIA_BoundingRectanglePropertyId, true)
        .ok()?;
    let raw = variant.as_raw();
    if raw.Anonymous.Anonymous.vt != 0x2005 {
        return None;
    }
    let array = raw.Anonymous.Anonymous.Anonymous.parray;
    if array.is_null()
        || (*array).cDims != 1
        || (*array).cbElements != 8
        || (*array).rgsabound[0].cElements != 4
        || (*array).pvData.is_null()
    {
        return None;
    }
    let values = std::slice::from_raw_parts((*array).pvData.cast::<f64>(), 4);
    if !values.iter().all(|value| value.is_finite()) {
        return None;
    }
    Some(crate::events::ElementBounds {
        x: values[0],
        y: values[1],
        width: values[2],
        height: values[3],
    })
}
impl Drop for Attempt<'_> {
    fn drop(&mut self) {
        if !self.registered {
            return;
        }
        unsafe {
            timing::enter(Stage::RegistrationWait);
            let _registration = REGISTRATION
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            provider_stage("bulk_fallback_property_remove");
            timing::enter(Stage::EventCleanup);
            let _ = cleanup_result(
                self.automation
                    .RemovePropertyChangedEventHandler(&self.root, &self.property),
            );
            provider_stage("bulk_fallback_structure_remove");
            let _ = cleanup_result(
                self.automation
                    .RemoveStructureChangedEventHandler(&self.root, &self.structure),
            );
        }
    }
}
