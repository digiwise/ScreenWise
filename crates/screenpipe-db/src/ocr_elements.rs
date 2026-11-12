//! OCR elements are built when read, not stored. A frame's OCR words are kept
//! once, with their positions, in `ocr_text.text_json`; the elements API
//! derives element rows from them here. Redaction deletes `text_json` when it
//! changes a frame's OCR text, so such a frame falls back to its redacted
//! `ocr_text.text`, one element per line, without positions.

use std::collections::BTreeMap;

use crate::{Element, ElementBounds, ElementSource, OcrTextBlock};

/// Bits of an OCR element id that hold the element's index within its frame.
const INDEX_BITS: u32 = 20;
/// Elements past this index in one frame are left out: their ids would clash.
const MAX_INDEX: usize = (1 << INDEX_BITS) - 1;

/// The roles OCR elements can have.
const OCR_ROLES: [&str; 5] = ["page", "block", "paragraph", "line", "word"];

/// Whether an elements search with these filters can return OCR elements,
/// which it builds by reading every OCR frame in its range. OCR elements have
/// no on-screen flag and only the OCR roles.
pub fn search_reads_ocr(
    source: Option<&ElementSource>,
    role: Option<&str>,
    on_screen: Option<bool>,
) -> bool {
    source != Some(&ElementSource::Accessibility)
        && on_screen.is_none()
        && role.is_none_or(|r| OCR_ROLES.contains(&r))
}

/// Id of a frame's `index`-th OCR element: the same on every read, and
/// negative so it never equals a stored element row's id.
pub fn ocr_element_id(frame_id: i64, index: usize) -> i64 {
    -((frame_id << INDEX_BITS) | index as i64)
}

/// The OCR elements of `frame_id`, from its `ocr_text` rows
/// `(text, text_json)` in storage order.
pub fn ocr_elements(frame_id: i64, rows: &[(Option<String>, Option<String>)]) -> Vec<Element> {
    let mut out = Vec::new();
    for (text, json) in rows {
        match json.as_deref().filter(|j| !j.trim().is_empty()) {
            Some(json) => push_json_elements(frame_id, json, &mut out),
            None => push_text_lines(frame_id, text.as_deref().unwrap_or(""), &mut out),
        }
    }
    out.truncate(MAX_INDEX + 1);
    out
}

/// Words from the OCR engine's JSON. Flat engines (Windows, Apple) give
/// level-0 blocks; Tesseract gives a page → block → paragraph → line → word
/// hierarchy, linked through `parent_id`. JSON that doesn't parse gives none.
fn push_json_elements(frame_id: i64, json: &str, out: &mut Vec<Element>) {
    let Ok(blocks) = serde_json::from_str::<Vec<OcrTextBlock>>(json) else {
        return;
    };
    let mut pages: BTreeMap<i64, usize> = BTreeMap::new();
    let mut blocks_at: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    let mut paragraphs: BTreeMap<(i64, i64, i64), usize> = BTreeMap::new();
    let mut lines: BTreeMap<(i64, i64, i64, i64), usize> = BTreeMap::new();

    for block in &blocks {
        let level: i64 = block.level.parse().unwrap_or(0);
        let page: i64 = block.page_num.parse().unwrap_or(0);
        let block_num: i64 = block.block_num.parse().unwrap_or(0);
        let par: i64 = block.par_num.parse().unwrap_or(0);
        let line: i64 = block.line_num.parse().unwrap_or(0);
        let text = Some(block.text.as_str()).filter(|t| !t.trim().is_empty());
        let conf: Option<f64> = block.conf.parse().ok();

        let index = out.len();
        let (role, text, parent, depth, confidence) = match level {
            0 => match text {
                Some(t) => ("block", Some(t), None, 0, conf),
                None => continue,
            },
            1 => {
                if pages.contains_key(&page) {
                    continue;
                }
                pages.insert(page, index);
                ("page", None, None, 0, None)
            }
            2 => {
                if blocks_at.contains_key(&(page, block_num)) {
                    continue;
                }
                blocks_at.insert((page, block_num), index);
                ("block", None, pages.get(&page).copied(), 1, None)
            }
            3 => {
                if paragraphs.contains_key(&(page, block_num, par)) {
                    continue;
                }
                paragraphs.insert((page, block_num, par), index);
                let parent = blocks_at.get(&(page, block_num)).copied();
                ("paragraph", None, parent, 2, None)
            }
            4 => {
                if lines.contains_key(&(page, block_num, par, line)) {
                    continue;
                }
                lines.insert((page, block_num, par, line), index);
                let parent = paragraphs.get(&(page, block_num, par)).copied();
                ("line", None, parent, 3, None)
            }
            5 => match text {
                Some(t) => {
                    let parent = lines.get(&(page, block_num, par, line)).copied();
                    ("word", Some(t), parent, 4, conf)
                }
                None => continue,
            },
            _ => continue,
        };
        let bounds = match (
            block.left.parse().ok(),
            block.top.parse().ok(),
            block.width.parse().ok(),
            block.height.parse().ok(),
        ) {
            (Some(left), Some(top), Some(width), Some(height)) => Some(ElementBounds {
                left,
                top,
                width,
                height,
            }),
            _ => None,
        };
        out.push(Element {
            id: ocr_element_id(frame_id, index),
            frame_id,
            source: ElementSource::Ocr,
            role: role.to_string(),
            text: text.map(str::to_string),
            parent_id: parent.map(|p| ocr_element_id(frame_id, p)),
            depth,
            bounds,
            confidence,
            sort_order: index as i32,
            on_screen: None,
        });
    }
}

/// The plain OCR text, one element per non-empty line, without positions.
fn push_text_lines(frame_id: i64, text: &str, out: &mut Vec<Element>) {
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let index = out.len();
        out.push(Element {
            id: ocr_element_id(frame_id, index),
            frame_id,
            source: ElementSource::Ocr,
            role: "line".to_string(),
            text: Some(line.to_string()),
            parent_id: None,
            depth: 0,
            bounds: None,
            confidence: None,
            sort_order: index as i32,
            on_screen: None,
        });
    }
}

/// Whether `text` matches an elements search as the stored rows' FTS index
/// matched it: each whitespace-separated query term (a quoted phrase in
/// `sanitize_fts5_query`) must occur as consecutive tokens. A token is a run
/// of letters or digits, compared case-insensitively, as FTS5's `unicode61`
/// tokenizer splits text (its diacritic folding is not reproduced).
pub(crate) fn text_matches(text: &str, query: &str) -> bool {
    let phrases: Vec<Vec<String>> = query
        .split_whitespace()
        .map(tokens)
        .filter(|p| !p.is_empty())
        .collect();
    if phrases.is_empty() {
        return false;
    }
    let words = tokens(text);
    phrases
        .iter()
        .all(|p| words.windows(p.len()).any(|w| w == p.as_slice()))
}

/// The tokens of a query, for a cheap SQL pre-check against an `ocr_text`
/// row's `text` and `text_json`: an element whose text matches contains every
/// token, and an element's text appears in the column it is built from
/// (JSON escapes never touch letters or digits). ASCII only: SQLite's
/// `lower()` folds no other letters.
pub(crate) fn prefilter_tokens(query: &str) -> Vec<String> {
    let mut out: Vec<String> = query
        .split_whitespace()
        .flat_map(tokens)
        .filter(|t| t.is_ascii())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn tokens(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(text: &str, json: Option<&str>) -> Vec<(Option<String>, Option<String>)> {
        vec![(Some(text.to_string()), json.map(str::to_string))]
    }

    #[test]
    fn windows_words_become_flat_blocks_with_stable_ids() {
        // The shape the Windows OCR engine writes: no level, conf "1.0".
        let json = r#"[{"text":"Hello","left":"0.1","top":"0.2","width":"0.3","height":"0.04","conf":"1.0"},
                       {"text":" ","left":"0","top":"0","width":"0","height":"0","conf":"1.0"},
                       {"text":"world","left":"0.5","top":"0.2","width":"0.2","height":"0.04","conf":"1.0"}]"#;
        let elements = ocr_elements(7, &rows("Hello world", Some(json)));
        assert_eq!(elements.len(), 2, "blank words are skipped");
        assert_eq!(elements[1].text.as_deref(), Some("world"));
        assert_eq!(elements[1].role, "block");
        assert_eq!(elements[1].sort_order, 1);
        assert_eq!(elements[1].confidence, Some(1.0));
        assert_eq!(elements[1].bounds.as_ref().map(|b| b.left), Some(0.5));
        assert_eq!(elements[1].id, ocr_element_id(7, 1));
        assert!(elements.iter().all(|e| e.id < 0 && e.frame_id == 7));
        let again: Vec<i64> = ocr_elements(7, &rows("Hello world", Some(json)))
            .iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(again, elements.iter().map(|e| e.id).collect::<Vec<_>>());
        assert_ne!(ocr_element_id(7, 1), ocr_element_id(8, 1));
    }

    #[test]
    fn tesseract_hierarchy_links_parents() {
        let block = |level: &str, line: &str, text: &str| {
            format!(
                r#"{{"level":"{level}","page_num":"1","block_num":"1","par_num":"1","line_num":"{line}","word_num":"1","text":"{text}","left":"1","top":"2","width":"3","height":"4","conf":"90"}}"#
            )
        };
        let json = format!(
            "[{},{},{},{},{},{}]",
            block("1", "0", ""),
            block("2", "0", ""),
            block("3", "0", ""),
            block("4", "1", ""),
            block("5", "1", "alpha"),
            block("5", "1", "beta"),
        );
        let elements = ocr_elements(3, &rows("alpha beta", Some(&json)));
        let roles: Vec<&str> = elements.iter().map(|e| e.role.as_str()).collect();
        assert_eq!(
            roles,
            ["page", "block", "paragraph", "line", "word", "word"]
        );
        for pair in elements.windows(2).take(4) {
            assert_eq!(pair[1].parent_id, Some(pair[0].id));
        }
        assert_eq!(elements[5].parent_id, Some(elements[3].id));
        assert_eq!(elements[5].depth, 4);
        assert_eq!(elements[0].confidence, None);
        assert_eq!(elements[5].confidence, Some(90.0));
    }

    #[test]
    fn redacted_frame_falls_back_to_text_lines() {
        let elements = ocr_elements(5, &rows("first [EMAIL]\n\n  second  ", None));
        let texts: Vec<_> = elements.iter().map(|e| e.text.as_deref()).collect();
        assert_eq!(texts, [Some("first [EMAIL]"), Some("second")]);
        assert!(elements
            .iter()
            .all(|e| e.role == "line" && e.bounds.is_none() && e.confidence.is_none()));
        assert!(ocr_elements(5, &rows("text", Some("not json"))).is_empty());
        assert!(ocr_elements(5, &rows("", None)).is_empty());
    }

    #[test]
    fn matching_follows_fts_phrases() {
        assert!(text_matches("Invoice #42 sent", "invoice"));
        assert!(text_matches("100.100.0.42", "100.100.0.42"));
        assert!(text_matches("Invoice #42 sent", "\"42\" SENT"));
        assert!(!text_matches("Invoice #42 sent", "voice"));
        assert!(!text_matches("Invoice #42 sent", "sent 42x"));
        assert!(!text_matches("anything", "..."));
        assert_eq!(prefilter_tokens("Café 100.100"), ["100"]);
    }
}
