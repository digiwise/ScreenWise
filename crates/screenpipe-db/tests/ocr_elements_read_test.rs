//! OCR elements are built from `ocr_text` when read
//! (`screenpipe_db::ocr_elements`), not stored as `elements` rows. These tests
//! pin what the elements search and frame lookups return for them.

use chrono::{DateTime, Duration, Utc};
use screenpipe_db::{
    ocr_elements::ocr_element_id, DatabaseManager, Element, ElementSource, FrameWindowData,
    OcrEngine,
};
use std::sync::Arc;

async fn setup() -> DatabaseManager {
    DatabaseManager::new("sqlite::memory:", Default::default())
        .await
        .unwrap()
}

/// The word list the Windows OCR engine stores in `text_json`.
fn windows_json(words: &[&str]) -> String {
    let words: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            serde_json::json!({
                "text": w, "left": format!("{}", i as f64 / 10.0), "top": "0.1",
                "width": "0.05", "height": "0.02", "conf": "1.0"
            })
        })
        .collect();
    serde_json::to_string(&words).unwrap()
}

/// A snapshot frame with on-screen accessibility nodes and OCR words.
async fn frame(
    db: &DatabaseManager,
    at: DateTime<Utc>,
    a11y: &[&str],
    ocr: &[&str],
    shares: Option<i64>,
) -> i64 {
    let nodes: Vec<_> = a11y
        .iter()
        .map(|t| serde_json::json!({"role": "AXStaticText", "text": t, "depth": 0, "on_screen": true}))
        .collect();
    let tree = serde_json::to_string(&nodes).unwrap();
    let a11y_text = a11y.join("\n");
    let (text, json) = (ocr.join(" "), windows_json(ocr));
    db.insert_snapshot_frame_with_ocr(
        "test_device",
        at,
        "/tmp/test.jpg",
        Some("TestApp"),
        Some("test window"),
        None,
        None,
        true,
        Some("event"),
        (!a11y.is_empty()).then_some(a11y_text.as_str()),
        Some("ocr"),
        (!a11y.is_empty()).then_some(tree.as_str()),
        None,
        None,
        (!ocr.is_empty()).then_some((text.as_str(), json.as_str(), "WindowsNative")),
        shares,
    )
    .await
    .unwrap()
}

/// Element inserts are queued after the frame's commit.
async fn settle() {
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}

fn labels(elements: &[Element]) -> Vec<(String, String)> {
    elements
        .iter()
        .map(|e| (e.source.to_string(), e.text.clone().unwrap_or_default()))
        .collect()
}

fn label(source: &str, text: &str) -> (String, String) {
    (source.to_string(), text.to_string())
}

async fn stored_ocr_rows(db: &DatabaseManager) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM elements WHERE source = 'ocr'")
        .fetch_one(&db.pool)
        .await
        .unwrap()
}

async fn search(
    db: &DatabaseManager,
    query: &str,
    source: Option<ElementSource>,
    role: Option<&str>,
    on_screen: Option<bool>,
    limit: u32,
    offset: u32,
) -> (Vec<Element>, i64) {
    db.search_elements(
        query,
        None,
        source.as_ref(),
        role,
        None,
        None,
        None,
        on_screen,
        limit,
        offset,
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn frame_elements_are_built_from_ocr_text_not_stored() {
    let db = setup().await;
    let id = frame(&db, Utc::now(), &["alpha"], &["Invoice", "42"], None).await;
    settle().await;
    assert_eq!(stored_ocr_rows(&db).await, 0);

    let all = db.get_frame_elements(id, None).await.unwrap();
    assert_eq!(
        labels(&all),
        [
            label("accessibility", "alpha"),
            label("ocr", "Invoice"),
            label("ocr", "42")
        ]
    );
    let ocr = &all[1..];
    assert_eq!(ocr[0].id, ocr_element_id(id, 0));
    assert_eq!(ocr[1].id, ocr_element_id(id, 1));
    assert_eq!(ocr[1].frame_id, id);
    assert_eq!(ocr[1].bounds.as_ref().map(|b| b.left), Some(0.1));
    assert_eq!(ocr[1].confidence, Some(1.0));
    assert!(all[0].id > 0, "accessibility elements keep their row ids");

    let only_ocr = db
        .get_frame_elements(id, Some(&ElementSource::Ocr))
        .await
        .unwrap();
    assert_eq!(only_ocr.len(), 2);
    let only_a11y = db
        .get_frame_elements(id, Some(&ElementSource::Accessibility))
        .await
        .unwrap();
    assert_eq!(labels(&only_a11y), [label("accessibility", "alpha")]);
}

#[tokio::test]
async fn batch_inserted_frames_store_no_ocr_elements() {
    let db = setup().await;
    db.insert_video_chunk_with_fps("/tmp/v.mp4", "dev", 1.0)
        .await
        .unwrap();
    let windows = vec![FrameWindowData {
        app_name: Some("App".into()),
        window_name: Some("W".into()),
        browser_url: None,
        focused: false,
        text: "one two".into(),
        text_json: windows_json(&["one", " ", "two"]),
    }];
    let inserted = db
        .insert_frames_with_ocr_batch(
            "dev",
            Some(Utc::now()),
            0,
            &windows,
            Arc::new(OcrEngine::WindowsNative),
        )
        .await
        .unwrap();
    assert_eq!(stored_ocr_rows(&db).await, 0);
    let elements = db.get_frame_elements(inserted[0].0, None).await.unwrap();
    assert_eq!(
        labels(&elements),
        [label("ocr", "one"), label("ocr", "two")],
        "blank words are skipped"
    );
}

#[tokio::test]
async fn search_runs_newest_frame_first_and_pages_across_sources() {
    let db = setup().await;
    let now = Utc::now();
    frame(
        &db,
        now - Duration::minutes(2),
        &["a1", "a2"],
        &["o1", "o2", "o3"],
        None,
    )
    .await;
    frame(
        &db,
        now - Duration::minutes(1),
        &["b1"],
        &["p1", "p2"],
        None,
    )
    .await;
    settle().await;

    let (all, total) = search(&db, "", None, None, None, 100, 0).await;
    assert_eq!(total, 8);
    assert_eq!(
        labels(&all),
        [
            label("accessibility", "b1"),
            label("ocr", "p1"),
            label("ocr", "p2"),
            label("accessibility", "a1"),
            label("accessibility", "a2"),
            label("ocr", "o1"),
            label("ocr", "o2"),
            label("ocr", "o3"),
        ]
    );
    let mut paged = Vec::new();
    for offset in [0, 3, 6] {
        let (page, page_total) = search(&db, "", None, None, None, 3, offset).await;
        assert_eq!(page_total, 8);
        paged.extend(page);
    }
    assert_eq!(labels(&paged), labels(&all));
    let ids: std::collections::HashSet<i64> = all.iter().map(|e| e.id).collect();
    assert_eq!(ids.len(), 8, "ids are unique across sources");

    assert_eq!(
        search(&db, "", Some(ElementSource::Ocr), None, None, 100, 0)
            .await
            .1,
        5
    );
    assert_eq!(
        search(
            &db,
            "",
            Some(ElementSource::Accessibility),
            None,
            None,
            100,
            0
        )
        .await
        .1,
        3
    );
    let (visible, visible_total) = search(&db, "", None, None, Some(true), 100, 0).await;
    assert_eq!(visible_total, 3, "OCR elements carry no on-screen flag");
    assert!(visible
        .iter()
        .all(|e| e.source == ElementSource::Accessibility));
    let (none, none_total) =
        search(&db, "", Some(ElementSource::Ocr), None, Some(true), 100, 0).await;
    assert!(
        none.is_empty() && none_total == 0,
        "OCR with an on-screen filter matches nothing"
    );
    let (by_role, role_total) = search(&db, "", None, Some("AXStaticText"), None, 100, 0).await;
    assert_eq!(role_total, 3, "a non-OCR role reads no OCR frames");
    assert!(by_role
        .iter()
        .all(|e| e.source == ElementSource::Accessibility));
}

#[tokio::test]
async fn text_and_role_filters_apply_to_ocr_words() {
    let db = setup().await;
    frame(
        &db,
        Utc::now(),
        &["invoice draft"],
        &["Invoice", "#42", "sent"],
        None,
    )
    .await;
    settle().await;

    let (hits, total) = search(&db, "invoice", None, None, None, 100, 0).await;
    assert_eq!(total, 2);
    assert_eq!(
        labels(&hits),
        [
            label("accessibility", "invoice draft"),
            label("ocr", "Invoice")
        ]
    );
    let (hits, _) = search(&db, "42", None, None, None, 100, 0).await;
    assert_eq!(labels(&hits), [label("ocr", "#42")]);
    assert_eq!(
        search(&db, "voice", Some(ElementSource::Ocr), None, None, 100, 0)
            .await
            .1,
        0,
        "whole tokens only, as the search index matched"
    );
    let (blocks, _) = search(&db, "", None, Some("block"), None, 100, 0).await;
    assert_eq!(blocks.len(), 3);
    assert!(blocks.iter().all(|e| e.source == ElementSource::Ocr));
}

#[tokio::test]
async fn redacted_frame_falls_back_to_redacted_text_lines() {
    let db = setup().await;
    let id = frame(&db, Utc::now(), &[], &["synthetic", "words"], None).await;
    // What redaction leaves when it changes a frame's OCR text.
    sqlx::query(
        "UPDATE ocr_text SET text = 'first [REDACTED]' || char(10) || 'second', text_json = NULL \
         WHERE frame_id = ?1",
    )
    .bind(id)
    .execute(&db.pool)
    .await
    .unwrap();

    let elements = db
        .get_frame_elements(id, Some(&ElementSource::Ocr))
        .await
        .unwrap();
    assert_eq!(
        labels(&elements),
        [label("ocr", "first [REDACTED]"), label("ocr", "second")]
    );
    assert!(elements
        .iter()
        .all(|e| e.bounds.is_none() && e.role == "line"));
}

#[tokio::test]
async fn shared_frames_read_their_anchor_and_are_not_counted_twice() {
    let db = setup().await;
    let now = Utc::now();
    let anchor = frame(
        &db,
        now - Duration::minutes(1),
        &[],
        &["same", "words"],
        None,
    )
    .await;
    let shared = frame(&db, now, &[], &["same", "words"], Some(anchor)).await;
    settle().await;

    let elements = db
        .get_frame_elements(shared, Some(&ElementSource::Ocr))
        .await
        .unwrap();
    assert_eq!(elements.len(), 2);
    assert!(elements.iter().all(|e| e.frame_id == anchor));
    assert_eq!(search(&db, "", None, None, None, 100, 0).await.1, 2);
}

#[tokio::test]
async fn ocr_rows_stored_by_earlier_versions_are_not_returned() {
    let db = setup().await;
    let id = frame(&db, Utc::now(), &["alpha"], &["word"], None).await;
    settle().await;
    sqlx::query(
        "INSERT INTO elements (frame_id, source, role, text) VALUES (?1, 'ocr', 'block', 'legacy')",
    )
    .bind(id)
    .execute(&db.pool)
    .await
    .unwrap();

    let (all, total) = search(&db, "", None, None, None, 100, 0).await;
    assert_eq!(total, 2);
    assert!(all.iter().all(|e| e.text.as_deref() != Some("legacy")));
    let frame_elements = db.get_frame_elements(id, None).await.unwrap();
    assert_eq!(
        labels(&frame_elements),
        [label("accessibility", "alpha"), label("ocr", "word")]
    );
}
