//! The frame-level content-screening decision, shared by capture-time
//! screening (`screenpipe-engine`) and the re-screen of stored data
//! ([`crate::rescreen`]), so the two cannot drift apart.
//!
//! Callers only gather texts: the engine from the live frame (accessibility
//! chunks, OCR text and word boxes, the natively enumerated titles of the
//! windows visible on a full-monitor frame), the re-screen from stored rows
//! (stored OCR word positions where they exist). This module decides which
//! texts are in credential context for the generic token detector and runs
//! [`screen_text_plan_in_context`] once over them:
//!
//! - a frame-window title naming a credential file puts every text in
//!   context;
//! - a credential word in the frame-window title puts that window's texts
//!   in context, and the frame's image texts too when the frame shows only
//!   that window;
//! - on a full-monitor frame, OCR words whose centre lies inside a visible
//!   window whose native title has a credential word are in context;
//! - the native titles of the other visible windows are screened as well:
//!   any hit in one of them is reported separately (their title bars are on
//!   screen and cannot be redacted as frame metadata).
//!
//! The tier decision, OCR hit location and pixel redaction stay with the
//! caller ([`crate::content_screening::screening_tier`],
//! [`crate::content_redaction`]). Nothing here logs or retains a title or
//! text; `Debug` of every type prints counts only.

use std::fmt;
use std::ops::Range;

use screenpipe_capture_policy::credential_files;

use crate::content_redaction::OcrWord;
use crate::content_screening::{
    screen_text_plan, screen_text_plan_in_context, screening_tier, ContentScreeningConfig,
    ContentScreeningPlan, ContentScreeningTier, ContentScreeningVerdict, TokenContext,
};

/// A window visible in the captured area: its native title and its area,
/// normalised to 0..1 of the image (`left, top, width, height`).
#[derive(Clone)]
pub struct VisibleWindow<'a> {
    pub title: &'a str,
    pub area: [f64; 4],
}

impl fmt::Debug for VisibleWindow<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VisibleWindow")
    }
}

/// Every text of one frame, as gathered by the caller.
#[derive(Default)]
pub struct FrameTexts<'a> {
    /// Native title of the frame's window (used for decisions; include it
    /// in `window_texts` to screen it).
    pub window_title: Option<&'a str>,
    /// Texts that belong to the frame's window: its metadata and
    /// accessibility texts.
    pub window_texts: &'a [&'a str],
    /// OCR text of the frame image.
    pub ocr_text: Option<&'a str>,
    /// OCR word boxes for `ocr_text` (words joined by single spaces), when
    /// known.
    pub ocr_words: Option<&'a [OcrWord<'a>]>,
    /// Other texts derived from the whole image (OCR word JSON, stored
    /// mixed full text, OCR element text).
    pub image_texts: &'a [&'a str],
    /// Full-monitor frames: the windows visible in the captured area,
    /// including the frame's own window.
    pub visible_windows: &'a [VisibleWindow<'a>],
    /// The image shows only the frame's window (an active-window-only
    /// frame, or no window enumeration).
    pub shows_only_window: bool,
}

impl fmt::Debug for FrameTexts<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FrameTexts")
            .field("window_texts", &self.window_texts.len())
            .field("ocr", &self.ocr_text.is_some())
            .field("image_texts", &self.image_texts.len())
            .field("visible_windows", &self.visible_windows.len())
            .finish()
    }
}

/// The shared decision for one frame.
pub struct FramePlan {
    /// Plan over `window_texts`, then the OCR text (when present), then
    /// `image_texts`, in that order.
    pub plan: ContentScreeningPlan,
    /// Index of the OCR text in [`Self::plan`], when present.
    pub ocr_index: Option<usize>,
    /// Hits in the other visible windows' native titles (empty when none).
    pub visible_titles: ContentScreeningVerdict,
}

impl fmt::Debug for FramePlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FramePlan")
            .field("plan", &self.plan)
            .field("visible_titles", &self.visible_titles.reasons.len())
            .finish()
    }
}

impl FramePlan {
    /// Distinct redactable hit texts across every screened text of `frame`
    /// (borrowed from the frame; never copied out of it).
    pub fn distinct_secrets<'a>(
        &self,
        frame: &FrameTexts<'a>,
    ) -> std::collections::BTreeSet<&'a str> {
        frame_inputs(frame)
            .into_iter()
            .enumerate()
            .flat_map(|(index, text)| {
                self.plan
                    .secret_spans(index)
                    .iter()
                    .filter_map(move |span| text.get(span.clone()))
            })
            .collect()
    }

    /// The screening tier of the frame: a hit in another visible window's
    /// title withholds it (that title bar cannot be redacted); otherwise
    /// [`screening_tier`] over the frame texts.
    pub fn tier(&self, frame: &FrameTexts<'_>, fail_open: bool) -> ContentScreeningTier {
        if self.visible_titles.blocked() {
            return ContentScreeningTier::Drop(self.verdict());
        }
        screening_tier(&self.plan, self.distinct_secrets(frame).len(), fail_open)
    }

    /// Every reason that fired, frame texts and visible titles together.
    pub fn verdict(&self) -> ContentScreeningVerdict {
        let mut reasons = self.plan.verdict().reasons;
        reasons.extend(self.visible_titles.reasons.iter().copied());
        reasons.sort();
        reasons.dedup();
        ContentScreeningVerdict { reasons }
    }
}

/// Screen one frame (see the module docs).
pub fn screen_frame(frame: &FrameTexts<'_>, config: &ContentScreeningConfig) -> FramePlan {
    let credential_file = frame
        .window_title
        .is_some_and(credential_files::title_names_credential_file);
    let titled = frame
        .window_title
        .is_some_and(credential_files::text_has_credential_word);
    let image_titled = titled && frame.shows_only_window;

    let texts = frame_inputs(frame);
    let mut context = TokenContext {
        titled: vec![titled; frame.window_texts.len()],
        ..Default::default()
    };
    let ocr_index = frame.ocr_text.map(|ocr| {
        let index = frame.window_texts.len();
        context.titled.push(image_titled);
        for range in visible_window_ranges(ocr, frame.ocr_words, frame.visible_windows) {
            context.ranges.push((index, range));
        }
        index
    });
    context
        .titled
        .extend(std::iter::repeat(image_titled).take(frame.image_texts.len()));
    context.credential_file = vec![credential_file; texts.len()];
    let plan = screen_text_plan_in_context(&texts, &context, config);

    let others: Vec<&str> = frame
        .visible_windows
        .iter()
        .map(|window| window.title)
        .filter(|title| !title.is_empty() && Some(*title) != frame.window_title)
        .collect();
    let visible_titles = if others.is_empty() {
        ContentScreeningVerdict::default()
    } else {
        screen_text_plan(&others, config).verdict()
    };
    FramePlan {
        plan,
        ocr_index,
        visible_titles,
    }
}

/// The screened texts of a frame, in plan order: window texts, OCR text,
/// image texts.
fn frame_inputs<'a>(frame: &FrameTexts<'a>) -> Vec<&'a str> {
    let mut texts: Vec<&'a str> = frame.window_texts.to_vec();
    texts.extend(frame.ocr_text);
    texts.extend(frame.image_texts.iter().copied());
    texts
}

/// Byte ranges of the OCR words whose centre lies inside a visible window
/// whose native title has a credential word. Empty when no word boxes are
/// known or the OCR text is not the words joined by single spaces.
fn visible_window_ranges(
    ocr_text: &str,
    words: Option<&[OcrWord<'_>]>,
    windows: &[VisibleWindow<'_>],
) -> Vec<Range<usize>> {
    let areas: Vec<[f64; 4]> = windows
        .iter()
        .filter(|window| credential_files::text_has_credential_word(window.title))
        .map(|window| window.area)
        .collect();
    let Some(words) = words else {
        return Vec::new();
    };
    if areas.is_empty() {
        return Vec::new();
    }
    let bytes = ocr_text.as_bytes();
    let mut ranges = Vec::new();
    let mut pos = 0usize;
    for (index, word) in words.iter().enumerate() {
        if index > 0 {
            if bytes.get(pos) != Some(&b' ') {
                return Vec::new();
            }
            pos += 1;
        }
        let end = pos + word.text.len();
        if ocr_text.get(pos..end) != Some(word.text) {
            return Vec::new();
        }
        let (cx, cy) = (word.left + word.width / 2.0, word.top + word.height / 2.0);
        if areas.iter().any(|[left, top, width, height]| {
            cx >= *left && cx <= left + width && cy >= *top && cy <= top + height
        }) {
            ranges.push(pos..end);
        }
        pos = end;
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_screening::ContentScreeningReason;

    fn config() -> ContentScreeningConfig {
        ContentScreeningConfig {
            blocked_url_patterns: Vec::new(),
            protected_domains: false,
            keywords: vec!["synthetic-keyword".into()],
        }
    }

    /// A generated, never-real token shape (36 mixed-class characters).
    fn token() -> String {
        (0..36u32)
            .map(|i| b"Ab3Xq9Lm2Zt7Pk4Rw8Ns5Vy1Hc6Jd0Ge"[((i * 7 + 3) % 32) as usize] as char)
            .collect()
    }

    fn word(text: &str, left: f64) -> OcrWord<'_> {
        OcrWord {
            text,
            left,
            top: 0.5,
            width: 0.2,
            height: 0.02,
        }
    }

    #[test]
    fn title_context_follows_window_and_frame_shape() {
        let token = token();
        let body = format!("first line\n{token}\nlast line");
        let config = config();
        let window_texts = ["API token rotation - Docs", body.as_str()];
        // Active-window-only: the window's texts and the image are in context.
        let frame = FrameTexts {
            window_title: Some("API token rotation - Docs"),
            window_texts: &window_texts,
            ocr_text: Some(body.as_str()),
            shows_only_window: true,
            ..Default::default()
        };
        let plan = screen_frame(&frame, &config);
        assert_eq!(plan.plan.secret_spans(1).len(), 1);
        assert_eq!(plan.plan.secret_spans(2).len(), 1);
        assert_eq!(plan.ocr_index, Some(2));
        // Full monitor: the image is not, unless a titled window covers it.
        let full = FrameTexts {
            shows_only_window: false,
            ..frame
        };
        assert!(screen_frame(&full, &config).plan.secret_spans(2).is_empty());
        // An innocent title lends nothing.
        let plain = ["Quarterly report - Word", body.as_str()];
        let innocent = FrameTexts {
            window_title: Some("Quarterly report - Word"),
            window_texts: &plain,
            ocr_text: Some(body.as_str()),
            shows_only_window: true,
            ..Default::default()
        };
        assert!(!screen_frame(&innocent, &config).verdict().blocked());
    }

    #[test]
    fn visible_windows_lend_context_to_their_region_and_are_screened() {
        let token = token();
        let ocr = format!("{token} {token}");
        let words = [word(&token, 0.1), word(&token, 0.6)];
        let windows = [
            VisibleWindow {
                title: "API token rotation - Docs",
                area: [0.0, 0.0, 0.5, 1.0],
            },
            VisibleWindow {
                title: "Quarterly report - Word",
                area: [0.5, 0.0, 0.5, 1.0],
            },
        ];
        let titles = ["Quarterly report - Word"];
        let frame = FrameTexts {
            window_title: Some("Quarterly report - Word"),
            window_texts: &titles,
            ocr_text: Some(ocr.as_str()),
            ocr_words: Some(&words),
            visible_windows: &windows,
            ..Default::default()
        };
        let plan = screen_frame(&frame, &config());
        assert_eq!(plan.plan.secret_spans(1), [0..token.len()]);
        assert!(plan.visible_titles.reasons.is_empty());
        // A hit in another visible title is reported; the frame's own title
        // is screened as a frame text instead.
        let leaking_title = format!("api key {token} - Browser");
        let leaking = [
            VisibleWindow {
                title: leaking_title.as_str(),
                area: [0.5, 0.0, 0.5, 1.0],
            },
            VisibleWindow {
                title: "synthetic-keyword notes - Editor",
                area: [0.0, 0.0, 0.5, 1.0],
            },
        ];
        let frame = FrameTexts {
            visible_windows: &leaking,
            ..frame
        };
        let plan = screen_frame(&frame, &config());
        assert_eq!(
            plan.visible_titles.reasons,
            [
                ContentScreeningReason::Secret,
                ContentScreeningReason::Keyword
            ]
        );
        assert!(plan.verdict().blocked());
        let own = FrameTexts {
            window_title: Some(leaking_title.as_str()),
            visible_windows: &leaking[..1],
            ..Default::default()
        };
        assert!(screen_frame(&own, &config())
            .visible_titles
            .reasons
            .is_empty());
    }

    #[test]
    fn a_credential_file_title_puts_every_text_in_context() {
        let token = token();
        let body = format!("first line\n{token}\nlast line");
        let texts = ["*deploy.secret - Editor"];
        let images = [body.as_str()];
        let frame = FrameTexts {
            window_title: Some("*deploy.secret - Editor"),
            window_texts: &texts,
            image_texts: &images,
            ..Default::default()
        };
        assert_eq!(
            screen_frame(&frame, &config()).plan.secret_spans(1).len(),
            1
        );
    }
}
