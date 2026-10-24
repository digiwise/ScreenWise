//! `screenpipe privacy rescreen`: owner-run re-screening of stored captures
//! with the current rules (incident SI-1, fix C).
//!
//! Never automatic. A dry run (the default) reads every frame and input
//! event up to the current highest ids, applies the current window rules and
//! content screening (`screenpipe_redact::rescreen`) in memory and prints
//! content-free counts per table, action and reason. It writes nothing to
//! the database; it saves a content-free plan (`rescreen/plan.json` in the
//! data directory: rule hash, id bounds, counts, fingerprint).
//!
//! `--apply --confirm <fingerprint>` then applies that plan: only rows up to
//! the planned ids, only while the rules are unchanged, in batches of at most
//! `--batch-size` rows, each written in one short transaction, with a pause
//! between batches. Progress (`rescreen/progress.json`, ids and counts only)
//! is saved after every batch, so an interrupted or `--max-rows`-bounded run
//! resumes where it stopped when the same command is repeated.
//!
//! Nothing here prints or logs matched text, titles, URLs, token values or
//! media paths: only counts and fixed reason codes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use screenpipe_db::rescreen::{RescreenFrameRow, RescreenUiEventRow, UiEventTextUpdate};
use screenpipe_db::DatabaseManager;
use screenpipe_redact::rescreen::{
    decide_frame, decide_ui_event, FrameDecision, RescreenRules, StoredFrame, StoredUiEvent,
    UiEventDecision,
};

/// Plan/progress file format version.
const STATE_VERSION: u32 = 1;

/// Largest accepted `--batch-size` (one write transaction per batch).
pub const MAX_BATCH_SIZE: u32 = screenpipe_db::rescreen::MAX_RESCREEN_WRITE_IDS as u32;

/// The owner's current rules as plain lists (also hashed into the plan).
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct RuleInputs {
    pub ignored_windows: Vec<String>,
    pub ignored_urls: Vec<String>,
    pub keywords: Vec<String>,
    pub protected_domains: bool,
    pub credential_file_patterns: Vec<String>,
}

impl RuleInputs {
    pub fn from_settings(settings: &screenpipe_config::RecordingSettings) -> Self {
        Self {
            ignored_windows: settings.ignored_windows.clone(),
            ignored_urls: settings.ignored_urls.clone(),
            keywords: settings.content_screening_keywords.clone(),
            protected_domains: settings.pause_on_drm_content,
            credential_file_patterns: settings.credential_file_patterns.clone(),
        }
    }

    /// Install the owner credential-file patterns and build the rules.
    pub fn rules(&self) -> RescreenRules {
        screenpipe_capture_policy::credential_files::set_owner_patterns(
            &self.credential_file_patterns,
        );
        RescreenRules::new(
            &self.ignored_windows,
            &self.ignored_urls,
            &self.keywords,
            self.protected_domains,
        )
    }

    /// Hex SHA-256 of the canonical rules (no rule value is stored).
    pub fn hash(&self) -> String {
        let canonical = serde_json::to_vec(self).unwrap_or_default();
        hex(&Sha256::digest(canonical))
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Content-free counts: `"<table>/<action>/<reason>" -> rows`, plus scans.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RescreenCounts {
    pub frames_scanned: u64,
    pub ui_events_scanned: u64,
    pub actions: BTreeMap<String, u64>,
}

impl RescreenCounts {
    fn add(&mut self, table: &str, action: &str, reason: &str) {
        *self
            .actions
            .entry(format!("{table}/{action}/{reason}"))
            .or_default() += 1;
    }

    fn merge(&mut self, other: &RescreenCounts) {
        self.frames_scanned += other.frames_scanned;
        self.ui_events_scanned += other.ui_events_scanned;
        for (key, value) in &other.actions {
            *self.actions.entry(key.clone()).or_default() += value;
        }
    }
}

/// Saved by a dry run; required by `--apply`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RescreenPlan {
    pub version: u32,
    pub rules_hash: String,
    pub frames_through: i64,
    pub ui_events_through: i64,
    pub counts: RescreenCounts,
    pub fingerprint: String,
}

impl RescreenPlan {
    fn new(rules_hash: String, through: (i64, i64), counts: RescreenCounts) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(STATE_VERSION.to_le_bytes());
        hasher.update(rules_hash.as_bytes());
        hasher.update(through.0.to_le_bytes());
        hasher.update(through.1.to_le_bytes());
        hasher.update(serde_json::to_vec(&counts).unwrap_or_default());
        hasher.update(
            chrono::Utc::now()
                .timestamp_nanos_opt()
                .unwrap_or(0)
                .to_le_bytes(),
        );
        let fingerprint = hex(&hasher.finalize())[..16].to_string();
        Self {
            version: STATE_VERSION,
            rules_hash,
            frames_through: through.0,
            ui_events_through: through.1,
            counts,
            fingerprint,
        }
    }
}

/// Saved after every applied batch.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RescreenProgress {
    pub version: u32,
    pub fingerprint: String,
    pub frames_done_through: i64,
    pub ui_events_done_through: i64,
    pub applied: RescreenCounts,
    pub media_files_queued: u64,
    pub media_files_deleted: u64,
    pub frames_with_shared_video: u64,
    /// Emptied video chunks that were their device's newest (possibly still
    /// recording) chunk; released on a later run once recording moved on.
    #[serde(default)]
    pub deferred_video_chunks: Vec<i64>,
    pub complete: bool,
}

/// Batch bounds for a run.
#[derive(Clone, Copy, Debug)]
pub struct RunLimits {
    pub batch_size: u32,
    /// Stop (resumably) after scanning this many rows in this invocation.
    pub max_rows: Option<u64>,
    pub pause: Duration,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            batch_size: 100,
            max_rows: None,
            pause: Duration::from_millis(100),
        }
    }
}

fn frame_decision(row: &RescreenFrameRow, rules: &RescreenRules) -> FrameDecision {
    use screenpipe_config::capture_diagnostics::capture_outcome;
    let outcome = row
        .capture_privacy
        .as_deref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .and_then(|value| {
            value
                .get("outcome")
                .and_then(|o| o.as_str())
                .map(str::to_string)
        });
    let mut window_texts: Vec<&str> = [
        row.accessibility_text.as_deref(),
        row.accessibility_tree_json.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    window_texts.extend(row.accessibility_element_texts.iter().map(String::as_str));
    // The first OCR row keeps its stored word positions; everything else
    // derived from the image is screened as image text.
    let mut ocr_rows = row.ocr.iter();
    let first = ocr_rows.next();
    let entries = first
        .and_then(|(_, json)| json.as_deref())
        .and_then(|json| {
            serde_json::from_str::<Vec<serde_json::Map<String, serde_json::Value>>>(json).ok()
        });
    let ocr_words = entries
        .as_deref()
        .and_then(crate::event_driven_capture::ocr_json_words);
    let mut image_texts: Vec<&str> = row.full_text.as_deref().into_iter().collect();
    image_texts.extend(first.and_then(|(_, json)| json.as_deref()));
    for (text, json) in ocr_rows {
        image_texts.push(text);
        image_texts.extend(json.as_deref());
    }
    image_texts.extend(row.ocr_element_texts.iter().map(String::as_str));
    let stored = StoredFrame {
        app_name: row.app_name.as_deref(),
        window_name: row.window_name.as_deref(),
        browser_url: row.browser_url.as_deref(),
        document_path: row.document_path.as_deref(),
        placeholder: outcome
            .as_deref()
            .is_some_and(capture_outcome::is_placeholder),
        single_window: outcome.as_deref() == Some(capture_outcome::ACTIVE_WINDOW_ONLY),
        window_texts,
        ocr_text: first.map(|(text, _)| text.as_str()),
        ocr_words,
        image_texts,
    };
    decide_frame(&stored, rules)
}

fn ui_event_decision(row: &RescreenUiEventRow, rules: &RescreenRules) -> UiEventDecision {
    let stored = StoredUiEvent {
        app_name: row.app_name.as_deref(),
        window_title: row.window_title.as_deref(),
        browser_url: row.browser_url.as_deref(),
        fields: [
            row.text_content.as_deref(),
            row.window_title.as_deref(),
            row.browser_url.as_deref(),
            row.element_name.as_deref(),
            row.element_value.as_deref(),
            row.element_description.as_deref(),
        ],
    };
    decide_ui_event(&stored, rules)
}

/// Read-only pass: counts only, nothing written to the database.
pub async fn dry_run(
    db: &DatabaseManager,
    rules: &RescreenRules,
    through: (i64, i64),
    batch_size: u32,
) -> Result<RescreenCounts> {
    let batch_size = batch_size.clamp(1, MAX_BATCH_SIZE);
    let mut counts = RescreenCounts::default();
    let mut cursor = 0;
    loop {
        let rows = db.rescreen_frames(cursor, through.0, batch_size).await?;
        let Some(last) = rows.last().map(|r| r.id) else {
            break;
        };
        for row in &rows {
            counts.frames_scanned += 1;
            match frame_decision(row, rules) {
                FrameDecision::Keep => {}
                FrameDecision::ClearMetadata(reason) => {
                    counts.add("frames", "clear_metadata", reason.code())
                }
                FrameDecision::Delete(reason) => counts.add("frames", "delete", reason.code()),
            }
        }
        cursor = last;
    }
    let mut cursor = 0;
    loop {
        let rows = db.rescreen_ui_events(cursor, through.1, batch_size).await?;
        let Some(last) = rows.last().map(|r| r.id) else {
            break;
        };
        for row in &rows {
            counts.ui_events_scanned += 1;
            match ui_event_decision(row, rules) {
                UiEventDecision::Keep => {}
                UiEventDecision::Delete(reason) => counts.add("ui_events", "delete", reason.code()),
                UiEventDecision::Redact(_) => counts.add("ui_events", "redact", "content_secret"),
            }
        }
        cursor = last;
    }
    Ok(counts)
}

/// Apply a confirmed plan from `progress` onwards; saves progress through
/// `save` after every batch. Returns whether the plan is now complete.
pub async fn apply(
    db: &DatabaseManager,
    rules: &RescreenRules,
    plan: &RescreenPlan,
    progress: &mut RescreenProgress,
    limits: RunLimits,
    mut save: impl FnMut(&RescreenProgress) -> Result<()>,
) -> Result<bool> {
    let batch_size = limits.batch_size.clamp(1, MAX_BATCH_SIZE);
    let mut scanned_now = 0u64;
    let budget_left = |scanned: u64| limits.max_rows.is_none_or(|max| scanned < max);

    release_deferred_chunks(db, progress).await?;
    while progress.frames_done_through < plan.frames_through && budget_left(scanned_now) {
        let rows = db
            .rescreen_frames(
                progress.frames_done_through,
                plan.frames_through,
                batch_size,
            )
            .await?;
        let Some(last) = rows.last().map(|r| r.id) else {
            progress.frames_done_through = plan.frames_through;
            break;
        };
        let mut batch = RescreenCounts::default();
        let (mut delete, mut clear) = (Vec::new(), Vec::new());
        for row in &rows {
            batch.frames_scanned += 1;
            match frame_decision(row, rules) {
                FrameDecision::Keep => {}
                FrameDecision::ClearMetadata(reason) => {
                    batch.add("frames", "clear_metadata", reason.code());
                    clear.push(row.id);
                }
                FrameDecision::Delete(reason) => {
                    batch.add("frames", "delete", reason.code());
                    delete.push(row.id);
                }
            }
        }
        drop(rows);
        db.clear_frame_metadata_for_rescreen(&clear).await?;
        let deletion = db.delete_frames_for_rescreen(&delete).await?;
        progress.frames_with_shared_video += deletion.counts.frames_with_shared_video;
        for chunk in deletion.deferred_chunks {
            if !progress.deferred_video_chunks.contains(&chunk) {
                progress.deferred_video_chunks.push(chunk);
            }
        }
        delete_queued_media(db, progress, &deletion.queued_media).await?;
        scanned_now += batch.frames_scanned;
        progress.applied.merge(&batch);
        progress.frames_done_through = last;
        save(progress)?;
        tokio::time::sleep(limits.pause).await;
    }

    while progress.frames_done_through >= plan.frames_through
        && progress.ui_events_done_through < plan.ui_events_through
        && budget_left(scanned_now)
    {
        let rows = db
            .rescreen_ui_events(
                progress.ui_events_done_through,
                plan.ui_events_through,
                batch_size,
            )
            .await?;
        let Some(last) = rows.last().map(|r| r.id) else {
            progress.ui_events_done_through = plan.ui_events_through;
            break;
        };
        let mut batch = RescreenCounts::default();
        let (mut delete, mut redact) = (Vec::new(), Vec::new());
        for row in &rows {
            batch.ui_events_scanned += 1;
            match ui_event_decision(row, rules) {
                UiEventDecision::Keep => {}
                UiEventDecision::Delete(reason) => {
                    batch.add("ui_events", "delete", reason.code());
                    delete.push(row.id);
                }
                UiEventDecision::Redact(fields) => {
                    batch.add("ui_events", "redact", "content_secret");
                    let [text_content, window_title, browser_url, element_name, element_value, element_description] =
                        *fields;
                    redact.push((
                        row.id,
                        UiEventTextUpdate {
                            text_content,
                            window_title,
                            browser_url,
                            element_name,
                            element_value,
                            element_description,
                        },
                    ));
                }
            }
        }
        drop(rows);
        db.delete_ui_events_for_rescreen(&delete).await?;
        db.redact_ui_events_for_rescreen(&redact).await?;
        scanned_now += batch.ui_events_scanned;
        progress.applied.merge(&batch);
        progress.ui_events_done_through = last;
        save(progress)?;
        tokio::time::sleep(limits.pause).await;
    }

    release_deferred_chunks(db, progress).await?;
    progress.complete = progress.frames_done_through >= plan.frames_through
        && progress.ui_events_done_through >= plan.ui_events_through
        && progress.deferred_video_chunks.is_empty();
    save(progress)?;
    Ok(progress.complete)
}

/// Hand now-unreferenced files to the existing media-deletion mechanism;
/// anything still pending stays queued for the recorder's periodic retry.
async fn delete_queued_media(
    db: &DatabaseManager,
    progress: &mut RescreenProgress,
    queued: &[String],
) -> Result<()> {
    if queued.is_empty() {
        return Ok(());
    }
    progress.media_files_queued += queued.len() as u64;
    let cleanup = db.retry_selected_media_deletions(queued, 100).await?;
    progress.media_files_deleted += cleanup.video_files_deleted + cleanup.snapshot_files_deleted;
    Ok(())
}

/// Release deferred chunks that are no longer their device's newest.
async fn release_deferred_chunks(
    db: &DatabaseManager,
    progress: &mut RescreenProgress,
) -> Result<()> {
    let pending = std::mem::take(&mut progress.deferred_video_chunks);
    for batch in pending.chunks(MAX_BATCH_SIZE as usize) {
        let release = db.release_rescreen_video_chunks(batch).await?;
        progress.frames_with_shared_video += release.now_shared;
        progress
            .deferred_video_chunks
            .extend(release.still_deferred);
        delete_queued_media(db, progress, &release.queued_media).await?;
    }
    Ok(())
}

fn state_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("rescreen")
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?).context("writing rescreen state")?;
    std::fs::rename(&tmp, path).context("saving rescreen state")?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(
            serde_json::from_slice(&bytes).context("reading rescreen state")?,
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("reading rescreen state"),
    }
}

fn print_counts(title: &str, counts: &RescreenCounts) {
    println!("{title}");
    println!("  frames scanned:    {}", counts.frames_scanned);
    println!("  ui_events scanned: {}", counts.ui_events_scanned);
    if counts.actions.is_empty() {
        println!("  no stored row would now be blocked");
    }
    for (key, value) in &counts.actions {
        println!("  {key}: {value}");
    }
}

/// Command-line options of `screenpipe privacy rescreen`.
pub struct RescreenCommand<'a> {
    pub data_dir: &'a Path,
    pub apply: bool,
    pub confirm: Option<&'a str>,
    /// Run with default rules when the data directory has no settings.
    pub allow_default_rules: bool,
    pub limits: RunLimits,
}

/// Entry point for the CLI subcommand.
pub async fn handle_rescreen(command: RescreenCommand<'_>) -> Result<()> {
    let data_dir = command.data_dir;
    let db_path = data_dir.join("db.sqlite");
    if !db_path.is_file() {
        bail!("no database at {}", db_path.display());
    }
    if command.limits.batch_size == 0 || command.limits.batch_size > MAX_BATCH_SIZE {
        bail!("--batch-size must be between 1 and {MAX_BATCH_SIZE}");
    }
    let settings = match super::load_recording_settings_from_store(data_dir)? {
        Some(settings) => {
            println!("rules: from the settings in this data directory");
            settings
        }
        None if command.allow_default_rules => {
            println!("rules: built-in defaults (no settings found in this data directory)");
            screenpipe_config::RecordingSettings::default()
        }
        None => bail!(
            "no settings found in this data directory; pass --allow-default-rules to re-screen with the built-in defaults only"
        ),
    };
    let inputs = RuleInputs::from_settings(&settings);
    println!(
        "  ignored windows: {}, ignored URLs: {}, keywords: {}, credential-file patterns: {}, DRM domains: {}",
        inputs.ignored_windows.len(),
        inputs.ignored_urls.len(),
        inputs.keywords.len(),
        inputs.credential_file_patterns.len(),
        if inputs.protected_domains { "on" } else { "off" }
    );
    let rules = inputs.rules();
    let db = DatabaseManager::new(&db_path.to_string_lossy(), Default::default())
        .await
        .context("opening the database")?;
    let state = state_dir(data_dir);
    std::fs::create_dir_all(&state).context("creating the rescreen state directory")?;
    let plan_path = state.join("plan.json");
    let progress_path = state.join("progress.json");

    if !command.apply {
        if command.confirm.is_some() {
            bail!("--confirm is only used with --apply");
        }
        let through = db.rescreen_max_ids().await?;
        let counts = dry_run(&db, &rules, through, command.limits.batch_size).await?;
        let plan = RescreenPlan::new(inputs.hash(), through, counts);
        write_json(&plan_path, &plan)?;
        print_counts("rescreen dry run (no data changed):", &plan.counts);
        println!();
        println!("plan fingerprint: {}", plan.fingerprint);
        println!(
            "to apply exactly this plan: screenpipe privacy rescreen --data-dir <same directory> --apply --confirm {}",
            plan.fingerprint
        );
        return Ok(());
    }

    let Some(confirm) = command.confirm else {
        bail!("--apply requires --confirm <fingerprint> from a dry run");
    };
    let Some(plan) = read_json::<RescreenPlan>(&plan_path)? else {
        bail!("no dry-run plan found; run the dry run first");
    };
    if plan.version != STATE_VERSION || plan.fingerprint != confirm.trim() {
        bail!("--confirm does not match the latest dry-run plan; run the dry run again");
    }
    if plan.rules_hash != inputs.hash() {
        bail!("the screening rules changed since the dry run; run the dry run again");
    }
    let mut progress = match read_json::<RescreenProgress>(&progress_path)? {
        Some(progress) if progress.fingerprint == plan.fingerprint => progress,
        _ => RescreenProgress {
            version: STATE_VERSION,
            fingerprint: plan.fingerprint.clone(),
            ..Default::default()
        },
    };
    if progress.complete {
        print_counts("this plan was already applied:", &progress.applied);
        return Ok(());
    }
    let complete = apply(&db, &rules, &plan, &mut progress, command.limits, |p| {
        write_json(&progress_path, p)
    })
    .await?;
    print_counts("rescreen applied so far:", &progress.applied);
    println!(
        "  media files queued for deletion: {}",
        progress.media_files_queued
    );
    println!(
        "  media files deleted:             {}",
        progress.media_files_deleted
    );
    println!(
        "  deleted frames whose video chunk is shared with kept frames (pixels remain in that file): {}",
        progress.frames_with_shared_video
    );
    println!(
        "  emptied video chunks still being recorded (removed by a later run): {}",
        progress.deferred_video_chunks.len()
    );
    if complete {
        println!("plan complete.");
    } else {
        println!("not finished; repeat the same command to resume (later, if chunks are still being recorded).");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic synthetic token (never a real credential).
    fn token() -> String {
        let mut state = 0x5101_FACE_u64;
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        loop {
            let candidate: String = (0..36)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    alphabet[(state % alphabet.len() as u64) as usize] as char
                })
                .collect();
            if !screenpipe_redact::content_screening::generic_token_hits(
                &format!("token {candidate}"),
                false,
            )
            .is_empty()
            {
                return candidate;
            }
        }
    }

    /// A synthetic database in a temporary directory (never a real one).
    async fn synthetic_db(dir: &Path) -> DatabaseManager {
        let db = DatabaseManager::new(&dir.join("db.sqlite").to_string_lossy(), Default::default())
            .await
            .unwrap();
        let token = token();
        let only_chunk = dir.join("synthetic-only.mp4");
        std::fs::write(&only_chunk, b"synthetic").unwrap();
        let only: i64 =
            sqlx::query_scalar("INSERT INTO video_chunks (file_path) VALUES (?1) RETURNING id")
                .bind(only_chunk.to_string_lossy().into_owned())
                .fetch_one(&db.pool)
                .await
                .unwrap();
        let shared: i64 = sqlx::query_scalar(
            "INSERT INTO video_chunks (file_path) VALUES ('synthetic-shared.mp4') RETURNING id",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        let frames = [
            // Credential-file title (walk disabled, full monitor).
            (
                Some(only),
                "notepad.exe",
                "*deploy-token.secret - Notepad",
                "harmless",
                r#"{"outcome":"full_monitor"}"#,
            ),
            // Prefixless token beside OCR-garbled context.
            (
                Some(shared),
                "notepad.exe",
                "notes.txt - Notepad",
                "read- token. secret\nFile\n§",
                r#"{"outcome":"full_monitor"}"#,
            ),
            // Ordinary frame sharing the chunk: kept.
            (
                Some(shared),
                "notepad.exe",
                "notes.txt - Notepad",
                "ordinary words",
                r#"{"outcome":"full_monitor"}"#,
            ),
            // Placeholder that kept a credential-file title.
            (
                None,
                "notepad.exe",
                "id_rsa - Notepad",
                "",
                r#"{"outcome":"redacted"}"#,
            ),
        ];
        for (chunk, app, title, text, privacy) in frames {
            let text = text.replace('§', &token);
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO frames (video_chunk_id, timestamp, app_name, window_name, full_text, capture_privacy)
                 VALUES (?1, '2026-01-01T00:00:00Z', ?2, ?3, ?4, ?5) RETURNING id",
            )
            .bind(chunk)
            .bind(app)
            .bind(title)
            .bind(&text)
            .bind(privacy)
            .fetch_one(&db.pool)
            .await
            .unwrap();
            sqlx::query("INSERT INTO ocr_text (frame_id, text) VALUES (?1, ?2)")
                .bind(id)
                .bind(&text)
                .execute(&db.pool)
                .await
                .unwrap();
        }
        let typed = format!("api token {token}");
        for (title, text, event_type) in [
            ("*deploy-token.secret - Notepad", "", "key"),
            ("notes.txt - Notepad", typed.as_str(), "clipboard"),
            ("notes.txt - Notepad", "hello", "text"),
        ] {
            sqlx::query(
                "INSERT INTO ui_events (event_type, app_name, window_title, text_content)
                 VALUES (?1, 'notepad.exe', ?2, ?3)",
            )
            .bind(event_type)
            .bind(title)
            .bind(text)
            .execute(&db.pool)
            .await
            .unwrap();
        }
        db
    }

    fn count_of(counts: &RescreenCounts, key: &str) -> u64 {
        counts.actions.get(key).copied().unwrap_or(0)
    }

    #[tokio::test]
    async fn dry_run_counts_then_bounded_resumable_apply_removes_blocked_content() {
        let dir = tempfile::tempdir().unwrap();
        let db = synthetic_db(dir.path()).await;
        let inputs = RuleInputs::default();
        let rules = inputs.rules();
        let through = db.rescreen_max_ids().await.unwrap();

        let counts = dry_run(&db, &rules, through, 2).await.unwrap();
        assert_eq!(counts.frames_scanned, 4);
        assert_eq!(counts.ui_events_scanned, 3);
        assert_eq!(count_of(&counts, "frames/delete/credential_file_title"), 1);
        assert_eq!(
            count_of(&counts, "frames/delete/content_secret_redactable"),
            1
        );
        assert_eq!(
            count_of(&counts, "frames/clear_metadata/credential_file_title"),
            1
        );
        assert_eq!(
            count_of(&counts, "ui_events/delete/credential_file_title"),
            1
        );
        assert_eq!(count_of(&counts, "ui_events/redact/content_secret"), 1);
        // The dry run changed nothing.
        assert_eq!(db.rescreen_frames(0, i64::MAX, 10).await.unwrap().len(), 4);
        let serialized = serde_json::to_string(&counts).unwrap();
        assert!(!serialized.contains("deploy") && !serialized.contains(&token()));

        let plan = RescreenPlan::new(inputs.hash(), through, counts);
        let mut progress = RescreenProgress {
            version: STATE_VERSION,
            fingerprint: plan.fingerprint.clone(),
            ..Default::default()
        };
        let mut saves = 0;
        let limits = RunLimits {
            batch_size: 1,
            max_rows: Some(2),
            pause: Duration::ZERO,
        };
        // Bounded: stops after two rows and can resume.
        assert!(!apply(&db, &rules, &plan, &mut progress, limits, |_| {
            saves += 1;
            Ok(())
        })
        .await
        .unwrap());
        assert!(saves >= 2);
        assert_eq!(progress.applied.frames_scanned, 2);
        let resumed = RunLimits {
            max_rows: None,
            ..limits
        };
        assert!(
            apply(&db, &rules, &plan, &mut progress, resumed, |_| Ok(()))
                .await
                .unwrap()
        );
        assert_eq!(progress.applied.actions, plan.counts.actions);

        let frames = db.rescreen_frames(0, i64::MAX, 10).await.unwrap();
        assert_eq!(frames.len(), 2);
        assert!(frames
            .iter()
            .all(|f| f.full_text.as_deref() != Some("harmless")));
        let placeholder = frames
            .iter()
            .find(|f| f.full_text.as_deref() == Some(""))
            .unwrap();
        assert!(placeholder.window_name.is_none() && placeholder.app_name.is_none());
        let events = db.rescreen_ui_events(0, i64::MAX, 10).await.unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| !e
            .text_content
            .as_deref()
            .unwrap_or_default()
            .contains(&token())));
        assert!(events
            .iter()
            .any(|e| e.text_content.as_deref() == Some("api token [SECRET]")));
        // The only-referenced video chunk was handed to media deletion.
        assert_eq!(progress.media_files_queued, 1);
        assert_eq!(progress.media_files_deleted, 1);
        assert!(!dir.path().join("synthetic-only.mp4").exists());
        assert_eq!(progress.frames_with_shared_video, 1);
        // Re-screening the cleaned database finds nothing more.
        let again = dry_run(&db, &rules, db.rescreen_max_ids().await.unwrap(), 50)
            .await
            .unwrap();
        assert!(again.actions.is_empty(), "{:?}", again.actions);
    }

    #[tokio::test]
    async fn apply_requires_the_dry_run_fingerprint_and_unchanged_rules() {
        let dir = tempfile::tempdir().unwrap();
        drop(synthetic_db(dir.path()).await);
        // No settings in the directory: refused unless defaults are allowed.
        assert!(handle_rescreen(RescreenCommand {
            data_dir: dir.path(),
            apply: false,
            confirm: None,
            allow_default_rules: false,
            limits: RunLimits::default(),
        })
        .await
        .is_err());
        let command = |apply: bool, confirm: Option<&'static str>| RescreenCommand {
            data_dir: dir.path(),
            apply,
            confirm,
            allow_default_rules: true,
            limits: RunLimits {
                pause: Duration::ZERO,
                ..Default::default()
            },
        };
        assert!(handle_rescreen(command(true, Some("0000"))).await.is_err());
        handle_rescreen(command(false, None)).await.unwrap();
        let plan: RescreenPlan = read_json(&dir.path().join("rescreen/plan.json"))
            .unwrap()
            .unwrap();
        assert!(handle_rescreen(command(true, None)).await.is_err());
        assert!(handle_rescreen(command(true, Some("ffffffffffffffff")))
            .await
            .is_err());
        let state = std::fs::read_to_string(dir.path().join("rescreen/plan.json")).unwrap();
        assert!(!state.contains("Notepad") && !state.contains("secret -"));
        let confirm: &'static str = Box::leak(plan.fingerprint.into_boxed_str());
        handle_rescreen(command(true, Some(confirm))).await.unwrap();
        let progress: RescreenProgress = read_json(&dir.path().join("rescreen/progress.json"))
            .unwrap()
            .unwrap();
        assert!(progress.complete);
        // A missing database is refused rather than created.
        let empty = tempfile::tempdir().unwrap();
        assert!(handle_rescreen(RescreenCommand {
            data_dir: empty.path(),
            apply: false,
            confirm: None,
            allow_default_rules: true,
            limits: RunLimits::default(),
        })
        .await
        .is_err());
        assert!(!empty.path().join("db.sqlite").exists());
    }
}
