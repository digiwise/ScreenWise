//! Dedicated read endpoints for the app's own activity/timeline queries,
//! replacing fixed `/raw_sql` queries the frontend used to build by hand.
//! See `.local/plans/dedicated-endpoints-contract-20261019.md` (sections 1-6).

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use oasgen::{oasgen, OaSchema};

use chrono::{DateTime, Duration, Utc};
use screenpipe_db::TextSnippetSource;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::error;

use crate::server::AppState;

/// `since_minutes` windows are clamped to this ceiling (7 days) everywhere
/// in this module, per the dedicated-endpoints contract's conventions.
const MAX_SINCE_MINUTES: u32 = 10_080;

fn clamp_since_minutes(minutes: u32) -> u32 {
    minutes.clamp(1, MAX_SINCE_MINUTES)
}

fn since_from_minutes(minutes: u32) -> DateTime<Utc> {
    Utc::now() - Duration::minutes(clamp_since_minutes(minutes) as i64)
}

fn internal_error(e: impl std::fmt::Display) -> (StatusCode, JsonResponse<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        JsonResponse(json!({"error": e.to_string()})),
    )
}

// ── 1. GET /activity/coverage ────────────────────────────────────────────

#[derive(OaSchema, Serialize)]
pub(crate) struct ActivityCoverageResponse {
    pub first_timestamp: Option<DateTime<Utc>>,
    pub last_timestamp: Option<DateTime<Utc>>,
    pub days: Vec<String>,
}

#[oasgen]
pub(crate) async fn activity_coverage_handler(
    State(state): State<Arc<AppState>>,
) -> Result<JsonResponse<ActivityCoverageResponse>, (StatusCode, JsonResponse<Value>)> {
    let coverage = state.db.get_activity_coverage().await.map_err(|e| {
        error!("failed to get activity coverage: {}", e);
        internal_error(e)
    })?;

    Ok(JsonResponse(ActivityCoverageResponse {
        first_timestamp: coverage.first_timestamp,
        last_timestamp: coverage.last_timestamp,
        days: coverage.days,
    }))
}

// ── 2. GET /activity/apps ─────────────────────────────────────────────────

const MAX_ACTIVITY_APPS_LIMIT: u32 = 500;

fn default_since_minutes() -> u32 {
    MAX_SINCE_MINUTES
}

fn default_activity_apps_limit() -> u32 {
    100
}

#[derive(OaSchema, Deserialize)]
pub(crate) struct ActivityAppsQuery {
    #[serde(default = "default_since_minutes")]
    pub since_minutes: u32,
    #[serde(default = "default_activity_apps_limit")]
    pub limit: u32,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct AppFrameCountItem {
    pub app_name: String,
    pub frame_count: i64,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct ActivityAppsResponse {
    pub apps: Vec<AppFrameCountItem>,
}

#[oasgen]
pub(crate) async fn activity_apps_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ActivityAppsQuery>,
) -> Result<JsonResponse<ActivityAppsResponse>, (StatusCode, JsonResponse<Value>)> {
    let since = since_from_minutes(query.since_minutes);
    let limit = query.limit.clamp(1, MAX_ACTIVITY_APPS_LIMIT) as i64;

    let apps = state
        .db
        .get_app_frame_counts(since, limit)
        .await
        .map_err(|e| {
            error!("failed to get app activity counts: {}", e);
            internal_error(e)
        })?;

    Ok(JsonResponse(ActivityAppsResponse {
        apps: apps
            .into_iter()
            .map(|a| AppFrameCountItem {
                app_name: a.app_name,
                frame_count: a.frame_count,
            })
            .collect(),
    }))
}

// ── 3. GET /activity/app-windows ──────────────────────────────────────────

const MAX_APP_WINDOWS_APP_LIMIT: u32 = 500;
const MAX_APP_WINDOWS_PER_APP: u32 = 50;

fn default_app_windows_limit() -> u32 {
    200
}

fn default_windows_per_app() -> u32 {
    20
}

#[derive(OaSchema, Deserialize)]
pub(crate) struct ActivityAppWindowsQuery {
    #[serde(default = "default_since_minutes")]
    pub since_minutes: u32,
    #[serde(default = "default_app_windows_limit")]
    pub limit: u32,
    #[serde(default = "default_windows_per_app")]
    pub windows_per_app: u32,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct AppWindowItem {
    pub window_name: Option<String>,
    pub frame_count: i64,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct AppWindowTreeItem {
    pub app_name: String,
    pub frame_count: i64,
    pub window_count: i64,
    pub windows: Vec<AppWindowItem>,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct ActivityAppWindowsResponse {
    pub apps: Vec<AppWindowTreeItem>,
}

#[oasgen]
pub(crate) async fn activity_app_windows_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ActivityAppWindowsQuery>,
) -> Result<JsonResponse<ActivityAppWindowsResponse>, (StatusCode, JsonResponse<Value>)> {
    let since = since_from_minutes(query.since_minutes);
    let app_limit = query.limit.clamp(1, MAX_APP_WINDOWS_APP_LIMIT) as i64;
    let windows_per_app = query.windows_per_app.clamp(1, MAX_APP_WINDOWS_PER_APP) as i64;

    let rows = state
        .db
        .get_app_window_tree(since, app_limit, windows_per_app)
        .await
        .map_err(|e| {
            error!("failed to get app/window activity tree: {}", e);
            internal_error(e)
        })?;

    // The DB returns one row per (app, window) pair, ordered by the unique
    // `app_rn`/`rn` ranks so all of one app's rows are contiguous (see
    // `get_app_window_tree`'s ORDER BY comment). Assemble the nested shape
    // here rather than in SQL.
    //
    // Defence in depth: key by a `HashMap<app_name, index>` rather than
    // relying solely on `apps.last_mut()`/row contiguity, so a future
    // change to the SQL ordering degrades to "duplicate app_name entries
    // merged" instead of silently splitting one app across two response
    // entries.
    let mut apps: Vec<AppWindowTreeItem> = Vec::new();
    let mut index_by_app: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for row in rows {
        let window = AppWindowItem {
            window_name: row.window_name,
            frame_count: row.window_frame_count,
        };
        if let Some(&idx) = index_by_app.get(&row.app_name) {
            apps[idx].windows.push(window);
        } else {
            index_by_app.insert(row.app_name.clone(), apps.len());
            apps.push(AppWindowTreeItem {
                app_name: row.app_name,
                frame_count: row.app_frame_count,
                window_count: row.window_count,
                windows: vec![window],
            });
        }
    }

    Ok(JsonResponse(ActivityAppWindowsResponse { apps }))
}

// ── 4. GET /activity/domains ──────────────────────────────────────────────

const MAX_DOMAINS_LIMIT: u32 = 500;

fn default_domains_limit() -> u32 {
    100
}

#[derive(OaSchema, Deserialize)]
pub(crate) struct ActivityDomainsQuery {
    #[serde(default = "default_since_minutes")]
    pub since_minutes: u32,
    #[serde(default = "default_domains_limit")]
    pub limit: u32,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct DomainCountItem {
    pub domain: String,
    pub frame_count: i64,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct ActivityDomainsResponse {
    pub domains: Vec<DomainCountItem>,
}

#[oasgen]
pub(crate) async fn activity_domains_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ActivityDomainsQuery>,
) -> Result<JsonResponse<ActivityDomainsResponse>, (StatusCode, JsonResponse<Value>)> {
    let since = since_from_minutes(query.since_minutes);
    let limit = query.limit.clamp(1, MAX_DOMAINS_LIMIT) as i64;

    let domains = state
        .db
        .get_browser_domain_counts(since, limit)
        .await
        .map_err(|e| {
            error!("failed to get browser domain activity: {}", e);
            internal_error(e)
        })?;

    Ok(JsonResponse(ActivityDomainsResponse {
        domains: domains
            .into_iter()
            .map(|d| DomainCountItem {
                domain: d.domain,
                frame_count: d.frame_count,
            })
            .collect(),
    }))
}

// ── 5. GET /activity/text-snippets ────────────────────────────────────────

const MAX_TEXT_SNIPPETS_LIMIT: u32 = 50;
const MAX_TEXT_SNIPPETS_MAX_CHARS: u32 = 2000;

fn default_text_snippets_since_minutes() -> u32 {
    15
}

fn default_text_snippets_limit() -> u32 {
    8
}

fn default_max_chars() -> u32 {
    200
}

#[derive(OaSchema, Deserialize)]
pub(crate) struct ActivityTextSnippetsQuery {
    pub source: String,
    #[serde(default = "default_text_snippets_since_minutes")]
    pub since_minutes: u32,
    #[serde(default = "default_text_snippets_limit")]
    pub limit: u32,
    #[serde(default = "default_max_chars")]
    pub max_chars: u32,
    #[serde(default)]
    pub min_chars: u32,
    /// `"recent"` (default) or `"random"`.
    pub order: Option<String>,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct TextSnippetItem {
    pub app_name: Option<String>,
    pub window_name: Option<String>,
    pub text: String,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct ActivityTextSnippetsResponse {
    pub total: i64,
    pub snippets: Vec<TextSnippetItem>,
}

#[oasgen]
pub(crate) async fn activity_text_snippets_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ActivityTextSnippetsQuery>,
) -> Result<JsonResponse<ActivityTextSnippetsResponse>, (StatusCode, JsonResponse<Value>)> {
    let source = match query.source.as_str() {
        "accessibility" => TextSnippetSource::Accessibility,
        "ocr" => TextSnippetSource::Ocr,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(json!({"error": "source must be 'accessibility' or 'ocr'"})),
            ))
        }
    };

    let random_order = match query.order.as_deref() {
        None | Some("recent") => false,
        Some("random") => true,
        Some(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(json!({"error": "order must be 'recent' or 'random'"})),
            ))
        }
    };

    let since = since_from_minutes(query.since_minutes);
    let limit = query.limit.clamp(1, MAX_TEXT_SNIPPETS_LIMIT) as i64;
    let max_chars = query.max_chars.clamp(1, MAX_TEXT_SNIPPETS_MAX_CHARS) as i64;
    let min_chars = query.min_chars as i64;

    let page = state
        .db
        .get_text_snippets(source, since, limit, max_chars, min_chars, random_order)
        .await
        .map_err(|e| {
            error!("failed to get text snippets: {}", e);
            internal_error(e)
        })?;

    // `page.snippets` carries captured text — never log it, only the error path above.
    Ok(JsonResponse(ActivityTextSnippetsResponse {
        total: page.total,
        snippets: page
            .snippets
            .into_iter()
            .map(|s| TextSnippetItem {
                app_name: s.app_name,
                window_name: s.window_name,
                text: s.text,
            })
            .collect(),
    }))
}

// ── 6. GET /activity/ui-events ────────────────────────────────────────────

const MAX_UI_EVENTS_LIMIT: u32 = 200;

fn default_ui_events_limit() -> u32 {
    30
}

#[derive(OaSchema, Deserialize)]
pub(crate) struct ActivityUiEventsQuery {
    #[serde(deserialize_with = "super::time::deserialize_flexible_datetime")]
    pub start: DateTime<Utc>,
    #[serde(deserialize_with = "super::time::deserialize_flexible_datetime")]
    pub end: DateTime<Utc>,
    #[serde(default = "default_ui_events_limit")]
    pub limit: u32,
    /// `"asc"` (default) or `"desc"`.
    pub order: Option<String>,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct UiEventItem {
    pub event_type: String,
    pub text_content: Option<String>,
    pub app_name: Option<String>,
    pub window_title: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct ActivityUiEventsResponse {
    pub events: Vec<UiEventItem>,
}

#[oasgen]
pub(crate) async fn activity_ui_events_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ActivityUiEventsQuery>,
) -> Result<JsonResponse<ActivityUiEventsResponse>, (StatusCode, JsonResponse<Value>)> {
    if query.end <= query.start {
        return Err((
            StatusCode::BAD_REQUEST,
            JsonResponse(json!({"error": "end must be after start"})),
        ));
    }

    let descending = match query.order.as_deref() {
        None | Some("asc") => false,
        Some("desc") => true,
        Some(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(json!({"error": "order must be 'asc' or 'desc'"})),
            ))
        }
    };

    let limit = query.limit.clamp(1, MAX_UI_EVENTS_LIMIT) as i64;

    let events = state
        .db
        .get_ui_events_in_range(query.start, query.end, limit, descending)
        .await
        .map_err(|e| {
            error!("failed to get ui events in range: {}", e);
            internal_error(e)
        })?;

    Ok(JsonResponse(ActivityUiEventsResponse {
        events: events
            .into_iter()
            .map(|e| UiEventItem {
                event_type: e.event_type,
                text_content: e.text_content,
                app_name: e.app_name,
                window_title: e.window_title,
                timestamp: e.timestamp,
            })
            .collect(),
    }))
}
