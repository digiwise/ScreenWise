//! Storage operations for the owner-run re-screen of stored captures
//! (incident SI-1, fix C; decisions live in `screenpipe-redact::rescreen`,
//! the tool in `screenpipe privacy rescreen`).
//!
//! Reads return bounded batches by id cursor; every write is one short
//! `IMMEDIATE` transaction over a caller-bounded id list, so a live recorder
//! is never blocked for long. Writes run with `secure_delete` on, so freed
//! pages are zeroed. Rows returned here hold captured content: callers keep
//! them in memory and never log, print or persist them.

use sqlx::Row;

use crate::db::{
    delete_frame_set, delete_selected_orphan_chunks, enqueue_media_deletions, newest_video_chunks,
    select_frame_set_by_ids, LiveVideoChunks,
};
use crate::{DatabaseManager, ImmediateTx};

/// One stored frame and every text derived from it.
pub struct RescreenFrameRow {
    pub id: i64,
    pub app_name: Option<String>,
    pub window_name: Option<String>,
    pub browser_url: Option<String>,
    pub document_path: Option<String>,
    pub capture_privacy: Option<String>,
    pub full_text: Option<String>,
    pub accessibility_text: Option<String>,
    pub accessibility_tree_json: Option<String>,
    /// `(ocr_text.text, ocr_text.text_json)` rows of the frame.
    pub ocr: Vec<(String, Option<String>)>,
    /// `elements.text` and `elements.properties` of the frame's element
    /// anchor (the frame itself or its `elements_ref_frame_id`), by source.
    pub accessibility_element_texts: Vec<String>,
    pub ocr_element_texts: Vec<String>,
}

/// One stored input event's identity and text columns.
pub struct RescreenUiEventRow {
    pub id: i64,
    pub app_name: Option<String>,
    pub window_title: Option<String>,
    pub browser_url: Option<String>,
    pub text_content: Option<String>,
    pub element_name: Option<String>,
    pub element_value: Option<String>,
    pub element_description: Option<String>,
}

/// Replacement text columns for one input event (`None` keeps the column).
#[derive(Default)]
pub struct UiEventTextUpdate {
    pub text_content: Option<String>,
    pub window_title: Option<String>,
    pub browser_url: Option<String>,
    pub element_name: Option<String>,
    pub element_value: Option<String>,
    pub element_description: Option<String>,
}

/// Content-free counts of [`DatabaseManager::delete_frames_for_rescreen`].
pub type RescreenFrameDeletion = crate::db::FrameSetCounts;

/// Paths queued for the existing media-deletion mechanism, plus counts.
pub struct RescreenFrameDeletionResult {
    pub counts: RescreenFrameDeletion,
    /// Now-unreferenced video and snapshot files queued in
    /// `media_deletion_jobs`. Local paths: never print or log them.
    pub queued_media: Vec<String>,
    /// Video chunk ids left without frames that were their device's newest
    /// chunk (possibly still recording); release them later with
    /// [`DatabaseManager::release_rescreen_video_chunks`].
    pub deferred_chunks: Vec<i64>,
}

/// Outcome of [`DatabaseManager::release_rescreen_video_chunks`].
#[derive(Debug, Default)]
pub struct RescreenChunkRelease {
    /// Files queued in `media_deletion_jobs` (local paths: never print them).
    pub queued_media: Vec<String>,
    /// Chunks that are still their device's newest.
    pub still_deferred: Vec<i64>,
    /// Chunks that gained frames meanwhile (pixels stay in the file).
    pub now_shared: u64,
}

/// Placeholder that binds an id list into `IN (...)`.
fn in_list(query: &mut sqlx::QueryBuilder<'_, sqlx::Sqlite>, ids: &[i64]) {
    let mut values = query.separated(", ");
    for id in ids {
        values.push_bind(*id);
    }
    values.push_unseparated(")");
}

async fn secure_delete(tx: &mut ImmediateTx) -> Result<(), sqlx::Error> {
    sqlx::query("PRAGMA secure_delete = ON")
        .execute(&mut **tx.conn())
        .await?;
    Ok(())
}

/// Most ids per write transaction.
pub const MAX_RESCREEN_WRITE_IDS: usize = 200;

impl DatabaseManager {
    /// Highest frame and input-event ids now stored (0 when empty).
    pub async fn rescreen_max_ids(&self) -> Result<(i64, i64), sqlx::Error> {
        let frames: Option<i64> = sqlx::query_scalar("SELECT MAX(id) FROM frames")
            .fetch_one(&self.pool)
            .await?;
        let events: Option<i64> = sqlx::query_scalar("SELECT MAX(id) FROM ui_events")
            .fetch_one(&self.pool)
            .await?;
        Ok((frames.unwrap_or(0), events.unwrap_or(0)))
    }

    /// Up to `limit` frames with `after < id <= through`, ascending, with
    /// their OCR and element texts.
    pub async fn rescreen_frames(
        &self,
        after: i64,
        through: i64,
        limit: u32,
    ) -> Result<Vec<RescreenFrameRow>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, app_name, window_name, browser_url, document_path, capture_privacy,
                    full_text, accessibility_text, accessibility_tree_json,
                    COALESCE(elements_ref_frame_id, id) AS anchor
             FROM frames WHERE id > ?1 AND id <= ?2 ORDER BY id LIMIT ?3",
        )
        .bind(after)
        .bind(through)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        let mut frames = Vec::with_capacity(rows.len());
        let mut anchors = Vec::with_capacity(rows.len());
        for row in rows {
            anchors.push(row.get::<i64, _>("anchor"));
            frames.push(RescreenFrameRow {
                id: row.get("id"),
                app_name: row.get("app_name"),
                window_name: row.get("window_name"),
                browser_url: row.get("browser_url"),
                document_path: row.get("document_path"),
                capture_privacy: row.get("capture_privacy"),
                full_text: row.get("full_text"),
                accessibility_text: row.get("accessibility_text"),
                accessibility_tree_json: row.get("accessibility_tree_json"),
                ocr: Vec::new(),
                accessibility_element_texts: Vec::new(),
                ocr_element_texts: Vec::new(),
            });
        }
        if frames.is_empty() {
            return Ok(frames);
        }
        let ids: Vec<i64> = frames.iter().map(|f| f.id).collect();
        let index = |id: i64, list: &[i64]| list.iter().position(|v| *v == id);

        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT frame_id, text, text_json FROM ocr_text WHERE frame_id IN (",
        );
        in_list(&mut query, &ids);
        for row in query.build().fetch_all(&self.pool).await? {
            if let Some(i) = index(row.get("frame_id"), &ids) {
                let text: Option<String> = row.get("text");
                let json: Option<String> = row.get("text_json");
                frames[i].ocr.push((text.unwrap_or_default(), json));
            }
        }

        let mut unique = anchors.clone();
        unique.sort_unstable();
        unique.dedup();
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT frame_id, source, text, properties FROM elements WHERE frame_id IN (",
        );
        in_list(&mut query, &unique);
        for row in query.build().fetch_all(&self.pool).await? {
            let anchor: i64 = row.get("frame_id");
            let source: String = row.get("source");
            let text: Option<String> = row.get("text");
            let properties: Option<String> = row.get("properties");
            for (i, frame_anchor) in anchors.iter().enumerate() {
                if *frame_anchor == anchor {
                    let target = if source == "ocr" {
                        &mut frames[i].ocr_element_texts
                    } else {
                        &mut frames[i].accessibility_element_texts
                    };
                    target.extend(
                        text.iter()
                            .chain(properties.iter())
                            .filter(|s| !s.is_empty())
                            .cloned(),
                    );
                }
            }
        }
        Ok(frames)
    }

    /// Up to `limit` input events with `after < id <= through`, ascending.
    pub async fn rescreen_ui_events(
        &self,
        after: i64,
        through: i64,
        limit: u32,
    ) -> Result<Vec<RescreenUiEventRow>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, app_name, window_title, browser_url, text_content, element_name,
                    element_value, element_description
             FROM ui_events WHERE id > ?1 AND id <= ?2 ORDER BY id LIMIT ?3",
        )
        .bind(after)
        .bind(through)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| RescreenUiEventRow {
                id: row.get("id"),
                app_name: row.get("app_name"),
                window_title: row.get("window_title"),
                browser_url: row.get("browser_url"),
                text_content: row.get("text_content"),
                element_name: row.get("element_name"),
                element_value: row.get("element_value"),
                element_description: row.get("element_description"),
            })
            .collect())
    }

    /// Delete these captured frames through the shared frame-set deletion
    /// (`db::delete_frame_set`, also used by range deletion), keeping a
    /// device's newest video chunk (possibly still recording) as deferred.
    /// At most [`MAX_RESCREEN_WRITE_IDS`] ids; one short transaction.
    pub async fn delete_frames_for_rescreen(
        &self,
        ids: &[i64],
    ) -> Result<RescreenFrameDeletionResult, sqlx::Error> {
        if ids.is_empty() {
            return Ok(RescreenFrameDeletionResult {
                counts: RescreenFrameDeletion::default(),
                queued_media: Vec::new(),
                deferred_chunks: Vec::new(),
            });
        }
        if ids.len() > MAX_RESCREEN_WRITE_IDS {
            return Err(sqlx::Error::Protocol(
                "too many ids in one rescreen write".into(),
            ));
        }
        let mut tx = self.begin_immediate_with_retry().await?;
        secure_delete(&mut tx).await?;
        select_frame_set_by_ids(&mut tx, ids).await?;
        let deleted = delete_frame_set(&mut tx, LiveVideoChunks::Defer).await?;
        tx.commit().await?;
        let mut queued_media = deleted.video_files;
        queued_media.extend(deleted.snapshot_files);
        Ok(RescreenFrameDeletionResult {
            counts: deleted.counts,
            queued_media,
            deferred_chunks: deleted.deferred_chunks,
        })
    }

    /// Release video chunks deferred by [`Self::delete_frames_for_rescreen`]
    /// because they were their device's newest (possibly still recording)
    /// chunk: a chunk that is no longer the newest and has no frame is
    /// removed and its file queued for media deletion; a chunk that is still
    /// the newest stays deferred; a chunk that gained frames is shared and
    /// dropped from the list (its earlier pixels remain in the file).
    pub async fn release_rescreen_video_chunks(
        &self,
        chunks: &[i64],
    ) -> Result<RescreenChunkRelease, sqlx::Error> {
        let mut release = RescreenChunkRelease::default();
        if chunks.is_empty() {
            return Ok(release);
        }
        if chunks.len() > MAX_RESCREEN_WRITE_IDS {
            return Err(sqlx::Error::Protocol(
                "too many ids in one rescreen write".into(),
            ));
        }
        let mut tx = self.begin_immediate_with_retry().await?;
        secure_delete(&mut tx).await?;
        let live = newest_video_chunks(&mut tx).await?;
        let mut released = Vec::new();
        for &chunk in chunks {
            let row = sqlx::query(
                "SELECT file_path, EXISTS(SELECT 1 FROM frames WHERE video_chunk_id = ?1) AS used
                 FROM video_chunks WHERE id = ?1",
            )
            .bind(chunk)
            .fetch_optional(&mut **tx.conn())
            .await?;
            let Some(row) = row else { continue };
            if row.get::<bool, _>("used") {
                release.now_shared += 1;
            } else if live.contains(&chunk) {
                release.still_deferred.push(chunk);
            } else {
                released.push(chunk);
                release.queued_media.push(row.get("file_path"));
            }
        }
        enqueue_media_deletions(&mut tx, &release.queued_media, &[], &[]).await?;
        delete_selected_orphan_chunks(&mut tx, &released, true).await?;
        tx.commit().await?;
        Ok(release)
    }

    /// Clear the recorded identity (app, title, URL, document path) of these
    /// placeholder frames. Returns the number of frames changed.
    pub async fn clear_frame_metadata_for_rescreen(&self, ids: &[i64]) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        if ids.len() > MAX_RESCREEN_WRITE_IDS {
            return Err(sqlx::Error::Protocol(
                "too many ids in one rescreen write".into(),
            ));
        }
        let mut tx = self.begin_immediate_with_retry().await?;
        secure_delete(&mut tx).await?;
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "UPDATE frames SET app_name = NULL, window_name = NULL, browser_url = NULL, document_path = NULL WHERE id IN (",
        );
        in_list(&mut query, ids);
        let changed = query
            .build()
            .execute(&mut **tx.conn())
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok(changed)
    }

    /// Delete these input events (their FTS entries go with them).
    pub async fn delete_ui_events_for_rescreen(&self, ids: &[i64]) -> Result<u64, sqlx::Error> {
        if ids.is_empty() {
            return Ok(0);
        }
        if ids.len() > MAX_RESCREEN_WRITE_IDS {
            return Err(sqlx::Error::Protocol(
                "too many ids in one rescreen write".into(),
            ));
        }
        let mut tx = self.begin_immediate_with_retry().await?;
        secure_delete(&mut tx).await?;
        let mut query =
            sqlx::QueryBuilder::<sqlx::Sqlite>::new("DELETE FROM ui_events WHERE id IN (");
        in_list(&mut query, ids);
        let deleted = query
            .build()
            .execute(&mut **tx.conn())
            .await?
            .rows_affected();
        tx.commit().await?;
        Ok(deleted)
    }

    /// Replace redacted text columns of these input events in one
    /// transaction (the FTS update trigger reindexes them).
    pub async fn redact_ui_events_for_rescreen(
        &self,
        updates: &[(i64, UiEventTextUpdate)],
    ) -> Result<u64, sqlx::Error> {
        if updates.is_empty() {
            return Ok(0);
        }
        if updates.len() > MAX_RESCREEN_WRITE_IDS {
            return Err(sqlx::Error::Protocol(
                "too many ids in one rescreen write".into(),
            ));
        }
        let mut tx = self.begin_immediate_with_retry().await?;
        secure_delete(&mut tx).await?;
        let mut changed = 0;
        for (id, update) in updates {
            changed += sqlx::query(
                "UPDATE ui_events SET
                    text_content = COALESCE(?1, text_content),
                    window_title = COALESCE(?2, window_title),
                    browser_url = COALESCE(?3, browser_url),
                    element_name = COALESCE(?4, element_name),
                    element_value = COALESCE(?5, element_value),
                    element_description = COALESCE(?6, element_description)
                 WHERE id = ?7",
            )
            .bind(&update.text_content)
            .bind(&update.window_title)
            .bind(&update.browser_url)
            .bind(&update.element_name)
            .bind(&update.element_value)
            .bind(&update.element_description)
            .bind(id)
            .execute(&mut **tx.conn())
            .await?
            .rows_affected();
        }
        tx.commit().await?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn db() -> DatabaseManager {
        DatabaseManager::new("sqlite::memory:", Default::default())
            .await
            .unwrap()
    }

    async fn chunk(db: &DatabaseManager, path: &str) -> i64 {
        sqlx::query_scalar("INSERT INTO video_chunks (file_path) VALUES (?1) RETURNING id")
            .bind(path)
            .fetch_one(&db.pool)
            .await
            .unwrap()
    }

    async fn frame(
        db: &DatabaseManager,
        chunk: Option<i64>,
        text: &str,
        anchor: Option<i64>,
    ) -> i64 {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO frames (video_chunk_id, timestamp, app_name, window_name, full_text, elements_ref_frame_id)
             VALUES (?1, '2026-01-01T00:00:00Z', 'synthetic.exe', 'Synthetic title', ?2, ?3) RETURNING id",
        )
        .bind(chunk)
        .bind(text)
        .bind(anchor)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO ocr_text (frame_id, text, text_json) VALUES (?1, ?2, '[]')")
            .bind(id)
            .bind(text)
            .execute(&db.pool)
            .await
            .unwrap();
        id
    }

    async fn element(db: &DatabaseManager, frame_id: i64, text: &str) {
        sqlx::query(
            "INSERT INTO elements (frame_id, source, role, text) VALUES (?1, 'ocr', 'line', ?2)",
        )
        .bind(frame_id)
        .bind(text)
        .execute(&db.pool)
        .await
        .unwrap();
    }

    async fn count(db: &DatabaseManager, sql: &str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(&db.pool).await.unwrap()
    }

    /// One fixture, deleted once by time range and once by ids through the
    /// re-screen: both callers of the shared frame-set deletion must leave
    /// the same rows, elements and media jobs (an emptied chunk and an
    /// unshared snapshot are queued; a chunk or snapshot still used by a
    /// kept frame is not).
    #[tokio::test]
    async fn range_and_rescreen_deletion_share_one_implementation() {
        use chrono::{TimeZone, Utc};
        let at = |second: u32| Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, second).unwrap();
        async fn build(
            db: &DatabaseManager,
            at: &dyn Fn(u32) -> chrono::DateTime<Utc>,
        ) -> Vec<i64> {
            let dir = std::env::temp_dir();
            let mut chunks = Vec::new();
            for name in ["si1-shared-a.mp4", "si1-shared-b.mp4", "si1-shared-c.mp4"] {
                chunks.push(chunk(db, &dir.join(name).to_string_lossy()).await);
            }
            let own_snapshot = dir.join("si1-own.jpg").to_string_lossy().into_owned();
            let kept_snapshot = dir.join("si1-kept.jpg").to_string_lossy().into_owned();
            let mut ids = Vec::new();
            for (second, chunk_index, anchor, snapshot) in [
                (1, 0, None, Some(&own_snapshot)),
                (2, 0, None, Some(&kept_snapshot)),
                (3, 1, None, None),
                (9, 1, Some(0usize), None),
                (9, 2, None, Some(&kept_snapshot)),
            ] {
                let anchor = anchor.map(|index: usize| ids[index]);
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO frames (video_chunk_id, timestamp, full_text, elements_ref_frame_id, snapshot_path)
                     VALUES (?1, ?2, 'shared fixture', ?3, ?4) RETURNING id",
                )
                .bind(chunks[chunk_index])
                .bind(at(second).to_rfc3339())
                .bind(anchor)
                .bind(snapshot)
                .fetch_one(&db.pool)
                .await
                .unwrap();
                ids.push(id);
            }
            element(db, ids[0], "shared element").await;
            ids
        }
        let state = |db: &DatabaseManager| {
            let pool = db.pool.clone();
            async move {
                let frames: Vec<(i64, Option<i64>)> =
                    sqlx::query_as("SELECT id, elements_ref_frame_id FROM frames ORDER BY id")
                        .fetch_all(&pool)
                        .await
                        .unwrap();
                let elements: Vec<i64> = sqlx::query_scalar("SELECT frame_id FROM elements")
                    .fetch_all(&pool)
                    .await
                    .unwrap();
                let jobs: Vec<String> = sqlx::query_scalar(
                    "SELECT file_path FROM media_deletion_jobs ORDER BY file_path",
                )
                .fetch_all(&pool)
                .await
                .unwrap();
                let chunks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM video_chunks")
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                (frames, elements, jobs, chunks)
            }
        };
        let by_range = db().await;
        let ids = build(&by_range, &at).await;
        by_range.delete_time_range(at(0), at(5)).await.unwrap();
        let by_ids = db().await;
        let same = build(&by_ids, &at).await;
        assert_eq!(ids, same);
        by_ids.delete_frames_for_rescreen(&ids[..3]).await.unwrap();
        let (range_state, id_state) = (state(&by_range).await, state(&by_ids).await);
        assert_eq!(range_state, id_state);
        // The kept referencing frame owns the shared element now.
        assert_eq!(range_state.0, [(ids[3], None), (ids[4], None)]);
        assert_eq!(range_state.1, [ids[3]]);
        // Queued: the emptied chunk and the unshared snapshot only.
        assert_eq!(range_state.2.len(), 2);
        assert!(range_state
            .2
            .iter()
            .any(|path| path.ends_with("si1-shared-a.mp4")));
        assert!(range_state
            .2
            .iter()
            .any(|path| path.ends_with("si1-own.jpg")));
        assert_eq!(range_state.3, 2);
    }

    #[tokio::test]
    async fn frames_are_read_with_ocr_and_anchor_elements() {
        let db = db().await;
        let anchor = frame(&db, None, "alpha synthetic", None).await;
        element(&db, anchor, "alpha element").await;
        let referencing = frame(&db, None, "beta synthetic", Some(anchor)).await;
        let rows = db.rescreen_frames(0, i64::MAX, 10).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].id, referencing);
        assert_eq!(rows[1].ocr_element_texts, ["alpha element"]);
        assert!(rows[1].accessibility_element_texts.is_empty());
        assert_eq!(
            rows[1].ocr,
            [("beta synthetic".to_string(), Some("[]".to_string()))]
        );
        assert_eq!(
            db.rescreen_frames(anchor, i64::MAX, 10)
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(db.rescreen_frames(0, anchor, 10).await.unwrap().len(), 1);
        assert_eq!(db.rescreen_max_ids().await.unwrap().0, referencing);
    }

    #[tokio::test]
    async fn deleting_frames_removes_text_index_and_orphaned_media_only() {
        let db = db().await;
        let dir = std::env::temp_dir();
        let only = dir
            .join("si1-rescreen-only.mp4")
            .to_string_lossy()
            .into_owned();
        let shared = dir
            .join("si1-rescreen-shared.mp4")
            .to_string_lossy()
            .into_owned();
        let only_chunk = chunk(&db, &only).await;
        let shared_chunk = chunk(&db, &shared).await;
        // The recorder's current chunk (newest for the device) is never removed.
        let live = dir
            .join("si1-rescreen-live.mp4")
            .to_string_lossy()
            .into_owned();
        let live_chunk = chunk(&db, &live).await;
        let in_live = frame(&db, Some(live_chunk), "zebracode live", None).await;
        let a = frame(&db, Some(only_chunk), "zebracode one", None).await;
        let b = frame(&db, Some(shared_chunk), "zebracode two", None).await;
        let kept = frame(&db, Some(shared_chunk), "ordinary kept", None).await;
        element(&db, a, "zebracode element").await;

        let result = db
            .delete_frames_for_rescreen(&[a, b, in_live])
            .await
            .unwrap();
        assert_eq!(result.deferred_chunks, [live_chunk]);
        assert_eq!(result.counts.frames_deleted, 3);
        assert_eq!(result.counts.ocr_rows_deleted, 3);
        assert_eq!(result.counts.elements_deleted, 1);
        assert_eq!(result.counts.video_chunks_deleted, 1);
        assert_eq!(result.counts.frames_with_shared_video, 1);
        assert_eq!(result.queued_media, [only.clone()]);
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM media_deletion_jobs").await,
            1
        );
        assert_eq!(count(&db, "SELECT COUNT(*) FROM frames").await, 1);
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM frames_fts WHERE frames_fts MATCH 'zebracode'"
            )
            .await,
            0
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM elements_fts WHERE elements_fts MATCH 'zebracode'"
            )
            .await,
            0
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM frames_fts WHERE frames_fts MATCH 'ordinary'"
            )
            .await,
            1
        );
        assert!(db.rescreen_frames(0, i64::MAX, 10).await.unwrap()[0].id == kept);

        // Still the newest chunk: stays deferred, nothing queued.
        let release = db
            .release_rescreen_video_chunks(&[live_chunk])
            .await
            .unwrap();
        assert_eq!(release.still_deferred, [live_chunk]);
        assert!(release.queued_media.is_empty());
        // Recording moved on to a new chunk: the empty one is released.
        chunk(&db, "si1-rescreen-next.mp4").await;
        let release = db
            .release_rescreen_video_chunks(&[live_chunk])
            .await
            .unwrap();
        assert!(release.still_deferred.is_empty());
        assert_eq!(release.queued_media, [live]);
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM media_deletion_jobs").await,
            2
        );
    }

    #[tokio::test]
    async fn a_deleted_element_anchor_moves_to_the_first_kept_reference() {
        let db = db().await;
        let anchor = frame(&db, None, "anchor", None).await;
        element(&db, anchor, "shared element").await;
        let first = frame(&db, None, "first", Some(anchor)).await;
        let second = frame(&db, None, "second", Some(anchor)).await;
        let result = db.delete_frames_for_rescreen(&[anchor]).await.unwrap();
        assert_eq!(result.counts.elements_deleted, 0);
        let rows = db.rescreen_frames(0, i64::MAX, 10).await.unwrap();
        assert_eq!(
            rows.iter().map(|r| r.id).collect::<Vec<_>>(),
            [first, second]
        );
        assert!(rows
            .iter()
            .all(|r| r.ocr_element_texts == ["shared element"]));
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM frames WHERE elements_ref_frame_id IS NULL"
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn metadata_and_input_events_are_cleared_deleted_and_redacted() {
        let db = db().await;
        let placeholder = frame(&db, None, "", None).await;
        assert_eq!(
            db.clear_frame_metadata_for_rescreen(&[placeholder])
                .await
                .unwrap(),
            1
        );
        let row = &db.rescreen_frames(0, i64::MAX, 10).await.unwrap()[0];
        assert!(row.app_name.is_none() && row.window_name.is_none());

        let insert = |title: &'static str, text: &'static str| {
            let pool = db.pool.clone();
            async move {
                sqlx::query_scalar::<_, i64>(
                    "INSERT INTO ui_events (event_type, app_name, window_title, text_content)
                     VALUES ('text', 'synthetic.exe', ?1, ?2) RETURNING id",
                )
                .bind(title)
                .bind(text)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        let gone = insert("Synthetic window", "quokkaword typed").await;
        let redacted = insert("Synthetic window", "wombatword typed").await;
        assert_eq!(db.delete_ui_events_for_rescreen(&[gone]).await.unwrap(), 1);
        let update = UiEventTextUpdate {
            text_content: Some("[SECRET] typed".into()),
            ..Default::default()
        };
        assert_eq!(
            db.redact_ui_events_for_rescreen(&[(redacted, update)])
                .await
                .unwrap(),
            1
        );
        let rows = db.rescreen_ui_events(0, i64::MAX, 10).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].text_content.as_deref(), Some("[SECRET] typed"));
        assert_eq!(rows[0].window_title.as_deref(), Some("Synthetic window"));
        for word in ["quokkaword", "wombatword"] {
            let sql =
                format!("SELECT COUNT(*) FROM ui_events_fts WHERE ui_events_fts MATCH '{word}'");
            assert_eq!(count(&db, &sql).await, 0, "{word}");
        }
        assert_eq!(db.rescreen_max_ids().await.unwrap().1, redacted);
    }
}
