//! Tag endpoints: per-content tagging (moved here unchanged from
//! `routes/content.rs`, apart from `use` lines) plus the dedicated vision-tag
//! read endpoints from `.local/plans/dedicated-endpoints-contract-20261019.md`
//! (section 7).

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use oasgen::{oasgen, OaSchema};

use chrono::{DateTime, Utc};
use screenpipe_db::TagContentType;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::error;

use crate::server::AppState;

#[derive(OaSchema, Deserialize)]
pub struct AddTagsRequest {
    tags: Vec<String>,
}

#[derive(OaSchema, Serialize)]
pub struct AddTagsResponse {
    success: bool,
}

#[derive(OaSchema, Deserialize)]
pub struct RemoveTagsRequest {
    tags: Vec<String>,
}

#[derive(OaSchema, Serialize)]
pub struct RemoveTagsResponse {
    success: bool,
}

#[oasgen]
pub(crate) async fn add_tags(
    State(state): State<Arc<AppState>>,
    Path((content_type, id)): Path<(String, i64)>,
    JsonResponse(payload): JsonResponse<AddTagsRequest>,
) -> Result<Json<AddTagsResponse>, (StatusCode, JsonResponse<Value>)> {
    let content_type = match content_type.as_str() {
        "vision" => TagContentType::Vision,
        "audio" => TagContentType::Audio,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(json!({"error": "Invalid content type"})),
            ))
        }
    };

    match state.db.add_tags(id, content_type, payload.tags).await {
        Ok(_) => Ok(JsonResponse(AddTagsResponse { success: true })),
        Err(e) => {
            error!("Failed to add tags: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(json!({"error": e.to_string()})),
            ))
        }
    }
}

#[oasgen]
pub(crate) async fn remove_tags(
    State(state): State<Arc<AppState>>,
    Path((content_type, id)): Path<(String, i64)>,
    JsonResponse(payload): JsonResponse<RemoveTagsRequest>,
) -> Result<Json<RemoveTagsResponse>, (StatusCode, JsonResponse<Value>)> {
    let content_type = match content_type.as_str() {
        "vision" => TagContentType::Vision,
        "audio" => TagContentType::Audio,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(json!({"error": "Invalid content type"})),
            ))
        }
    };

    match state.db.remove_tags(id, content_type, payload.tags).await {
        Ok(_) => Ok(JsonResponse(RemoveTagsResponse { success: true })),
        Err(e) => {
            error!("Failed to remove tag: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(json!({"error": e.to_string()})),
            ))
        }
    }
}

#[derive(OaSchema, Deserialize)]
pub struct GetTagsBatchRequest {
    pub frame_ids: Vec<i64>,
}

#[derive(OaSchema, Serialize)]
pub struct GetTagsBatchResponse {
    pub tags: std::collections::HashMap<String, Vec<String>>,
}

/// Batch fetch tags for multiple vision frame IDs.
/// POST /tags/vision/batch  { "frame_ids": [1, 2, 3] }
#[oasgen]
pub(crate) async fn get_tags_batch(
    State(state): State<Arc<AppState>>,
    JsonResponse(payload): JsonResponse<GetTagsBatchRequest>,
) -> Result<Json<GetTagsBatchResponse>, (StatusCode, JsonResponse<Value>)> {
    match state.db.get_vision_tags_batch(&payload.frame_ids).await {
        Ok(tags_map) => {
            // Convert i64 keys to String for JSON compatibility
            let string_map: std::collections::HashMap<String, Vec<String>> = tags_map
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
            Ok(JsonResponse(GetTagsBatchResponse { tags: string_map }))
        }
        Err(e) => {
            error!("Failed to get tags batch: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(json!({"error": e.to_string()})),
            ))
        }
    }
}

// ── 7. GET /tags/vision and GET /tags/vision/frames ───────────────────────

const MAX_VISION_TAGS_LIMIT: u32 = 1000;
const MAX_VISION_TAG_FRAMES_LIMIT: u32 = 500;
const MAX_VISION_TAG_FRAMES_TAGS: usize = 50;
const MAX_VISION_TAG_NAME_LENGTH: usize = 100;

fn default_vision_tags_limit() -> u32 {
    200
}

fn default_vision_tag_frames_limit() -> u32 {
    50
}

#[derive(OaSchema, Deserialize)]
pub(crate) struct ListVisionTagsQuery {
    #[serde(default = "default_vision_tags_limit")]
    pub limit: u32,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct VisionTagCountItem {
    pub name: String,
    pub frame_count: i64,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct ListVisionTagsResponse {
    pub tags: Vec<VisionTagCountItem>,
}

/// Only tags that are actually applied to at least one vision frame, ordered
/// by frame_count desc. Fixes the search modal's `#tag` list, which used to
/// query without a `LIMIT` and was always rejected by `validate_raw_sql`.
#[oasgen]
pub(crate) async fn list_vision_tags_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListVisionTagsQuery>,
) -> Result<JsonResponse<ListVisionTagsResponse>, (StatusCode, JsonResponse<Value>)> {
    let limit = query.limit.clamp(1, MAX_VISION_TAGS_LIMIT) as i64;

    let tags = state.db.get_vision_tag_counts(limit).await.map_err(|e| {
        error!("failed to get vision tag counts: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            JsonResponse(json!({"error": e.to_string()})),
        )
    })?;

    Ok(JsonResponse(ListVisionTagsResponse {
        tags: tags
            .into_iter()
            .map(|t| VisionTagCountItem {
                name: t.name,
                frame_count: t.frame_count,
            })
            .collect(),
    }))
}

#[derive(OaSchema, Deserialize)]
pub(crate) struct ListFramesWithVisionTagsQuery {
    /// Comma-separated tag names.
    pub tags: String,
    #[serde(default = "default_vision_tag_frames_limit")]
    pub limit: u32,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct TaggedFrameItem {
    pub frame_id: i64,
    pub timestamp: DateTime<Utc>,
    pub app_name: Option<String>,
    pub tag_names: Vec<String>,
}

#[derive(OaSchema, Serialize)]
pub(crate) struct ListFramesWithVisionTagsResponse {
    pub frames: Vec<TaggedFrameItem>,
}

/// Frames carrying any of the given vision tags, ordered by timestamp desc.
/// Tag names are bound parameters, never interpolated into SQL text.
#[oasgen]
pub(crate) async fn list_frames_with_vision_tags_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListFramesWithVisionTagsQuery>,
) -> Result<JsonResponse<ListFramesWithVisionTagsResponse>, (StatusCode, JsonResponse<Value>)> {
    let tags: Vec<String> = query
        .tags
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect();

    if tags.is_empty() || tags.len() > MAX_VISION_TAG_FRAMES_TAGS {
        return Err((
            StatusCode::BAD_REQUEST,
            JsonResponse(json!({
                "error": format!(
                    "tags must have between 1 and {} non-empty entries",
                    MAX_VISION_TAG_FRAMES_TAGS
                )
            })),
        ));
    }
    // Content-free: do not echo caller-controlled tag text back in the error
    // (matches the other new handlers in this batch). Compare by char count,
    // not byte length, since the limit and message are both in characters.
    if tags
        .iter()
        .any(|t| t.chars().count() > MAX_VISION_TAG_NAME_LENGTH)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            JsonResponse(json!({
                "error": format!(
                    "tag too long (max {} chars)",
                    MAX_VISION_TAG_NAME_LENGTH
                )
            })),
        ));
    }

    let limit = query.limit.clamp(1, MAX_VISION_TAG_FRAMES_LIMIT) as i64;

    let frames = state
        .db
        .get_frames_with_vision_tags(&tags, limit)
        .await
        .map_err(|e| {
            error!("failed to get frames with vision tags: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(json!({"error": e.to_string()})),
            )
        })?;

    Ok(JsonResponse(ListFramesWithVisionTagsResponse {
        frames: frames
            .into_iter()
            .map(|f| TaggedFrameItem {
                frame_id: f.frame_id,
                timestamp: f.timestamp,
                app_name: f.app_name,
                tag_names: f.tag_names,
            })
            .collect(),
    }))
}
