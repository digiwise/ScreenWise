//! Decisions for re-screening already stored captures (incident SI-1,
//! fix C).
//!
//! Capture-time screening only protects frames captured after a rule
//! exists. This module applies the *current* rules to rows that are already
//! stored, so an owner-run tool (`screenpipe privacy rescreen`) can remove
//! what would now be blocked. It makes no screening decision of its own: it
//! gathers a stored row's texts and calls the same implementations capture
//! uses:
//!
//! - window rules: `screenpipe_capture_policy::target::TargetFilter` (protected
//!   app, credential-file title, configured `ignoredWindows` pattern; the
//!   include list is not re-applied) and `url_filter::is_url_blocked` for
//!   `ignoredUrls`;
//! - content screening: [`crate::frame_screening::screen_frame`] and its tier
//!   (the frame-level decision shared with capture time).
//!
//! A captured frame that would now be blocked is **deleted** (text, element
//! rows, search index entries and, through the caller, its media): its
//! pixels live in a shared video chunk that cannot be redacted in place, so
//! the in-memory redaction tier does not apply to stored frames (its share is
//! reported as `content_secret_redactable`). A placeholder frame (a withheld
//! capture whose image is a fixed notice) only loses its app, title, URL and
//! document path. An input event is deleted when its window would now be
//! excluded or the tier withholds it, and otherwise has its secret spans
//! replaced with the shared `[SECRET]` label.
//!
//! Decisions carry fixed reason codes only, never matched text, titles,
//! URLs or positions. The functions do no I/O and no logging.

use std::borrow::Cow;
use std::fmt;

use screenpipe_capture_policy::target::{Exclusion, TargetFilter};
use screenpipe_capture_policy::url_filter;

use crate::content_redaction::{mask_secret_labels, replace_secret_spans, OcrWord};
use crate::content_screening::{
    ContentScreeningConfig, ContentScreeningReason, ContentScreeningTier,
};
use crate::frame_screening::{screen_frame, FrameTexts};

/// Texts are screened in windows of this many bytes, overlapping by
/// [`CHUNK_OVERLAP_BYTES`], so a stored text larger than the in-memory
/// screening bound is still screened completely.
pub const CHUNK_BYTES: usize = 256 * 1024;

/// Overlap between consecutive screening windows of one long text.
pub const CHUNK_OVERLAP_BYTES: usize = 4 * 1024;

/// The current rules, resolved once per run from the owner's settings.
pub struct RescreenRules {
    windows: TargetFilter,
    ignored_windows: usize,
    ignored_urls: Vec<String>,
    screening: ContentScreeningConfig,
}

impl fmt::Debug for RescreenRules {
    /// Counts only: rule values are configuration the owner may consider
    /// private.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RescreenRules")
            .field("ignored_windows", &self.ignored_windows)
            .field("ignored_urls", &self.ignored_urls.len())
            .field("keywords", &self.screening.keywords.len())
            .field("protected_domains", &self.screening.protected_domains)
            .finish()
    }
}

impl RescreenRules {
    /// `ignored_windows`/`ignored_urls` as in the capture settings,
    /// `keywords` from `contentScreeningKeywords`, `protected_domains` from
    /// `pauseOnDrmContent`. Owner credential-file patterns are read from the
    /// process-wide installation
    /// (`credential_files::set_owner_patterns`), like capture does.
    pub fn new(
        ignored_windows: &[String],
        ignored_urls: &[String],
        keywords: &[String],
        protected_domains: bool,
    ) -> Self {
        Self {
            windows: TargetFilter::new(ignored_windows, &[]),
            ignored_windows: ignored_windows.len(),
            ignored_urls: ignored_urls.iter().map(|url| url.to_lowercase()).collect(),
            screening: ContentScreeningConfig {
                blocked_url_patterns: ignored_urls.to_vec(),
                protected_domains,
                keywords: keywords.to_vec(),
            },
        }
    }

    /// The window rule that excludes this recorded identity, if any.
    fn window_reason(
        &self,
        app: Option<&str>,
        title: Option<&str>,
        url: Option<&str>,
    ) -> Option<RescreenReason> {
        match self.windows.recorded_exclusion(app, title) {
            Some(Exclusion::CredentialFile) => return Some(RescreenReason::CredentialFileTitle),
            Some(Exclusion::ProtectedApp) => return Some(RescreenReason::ProtectedApp),
            Some(Exclusion::Ignored) => return Some(RescreenReason::IgnoredWindow),
            Some(Exclusion::OutsideIncludes | Exclusion::Unidentified) | None => {}
        }
        url.is_some_and(|url| url_filter::is_url_blocked(url, &self.ignored_urls))
            .then_some(RescreenReason::IgnoredUrl)
    }
}

/// Why a stored row would now be blocked. Fixed codes only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RescreenReason {
    /// The window title names a credential file.
    CredentialFileTitle,
    /// A password manager, credential viewer or system UI host.
    ProtectedApp,
    /// A configured `ignoredWindows` pattern matches.
    IgnoredWindow,
    /// The recorded URL is on a configured `ignoredUrls` domain.
    IgnoredUrl,
    /// The screening tier withholds the row for this reason.
    Content(ContentScreeningReason),
    /// Only secrets that the capture-time redaction tier could have
    /// redacted in place.
    RedactableSecret,
}

impl RescreenReason {
    /// Fixed, allowlisted code, safe to print and persist.
    pub fn code(self) -> &'static str {
        match self {
            Self::CredentialFileTitle => "credential_file_title",
            Self::ProtectedApp => "protected_app",
            Self::IgnoredWindow => "ignored_window",
            Self::IgnoredUrl => "ignored_url",
            Self::Content(reason) => reason.code(),
            Self::RedactableSecret => "content_secret_redactable",
        }
    }
}

/// The re-screen reason for a screening tier, `None` for a pass.
fn tier_reason(tier: &ContentScreeningTier) -> Option<RescreenReason> {
    match tier {
        ContentScreeningTier::Pass => None,
        ContentScreeningTier::Redact => Some(RescreenReason::RedactableSecret),
        ContentScreeningTier::Drop(verdict) => Some(RescreenReason::Content(
            verdict
                .reasons
                .first()
                .copied()
                .unwrap_or(ContentScreeningReason::Unavailable),
        )),
    }
}

/// One stored frame, borrowed from the caller's in-memory row.
#[derive(Default)]
pub struct StoredFrame<'a> {
    pub app_name: Option<&'a str>,
    pub window_name: Option<&'a str>,
    pub browser_url: Option<&'a str>,
    pub document_path: Option<&'a str>,
    /// A withheld capture whose image is a fixed notice (its
    /// `capture_privacy.outcome` is `redacted` or `capture_failed`).
    pub placeholder: bool,
    /// The image showed only the frame's window (`capture_privacy.outcome`
    /// is `active_window_only`), as capture's `shows_only_window`.
    pub single_window: bool,
    /// Texts of the frame's window: accessibility text, tree JSON and
    /// accessibility element texts/properties.
    pub window_texts: Vec<&'a str>,
    /// The stored OCR text and its stored word positions, when parsed.
    pub ocr_text: Option<&'a str>,
    pub ocr_words: Option<Vec<OcrWord<'a>>>,
    /// Other image-derived texts: stored full text, OCR word JSON and OCR
    /// element texts.
    pub image_texts: Vec<&'a str>,
}

/// What to do with a stored frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameDecision {
    Keep,
    /// Placeholder frame: clear app, title, URL and document path.
    ClearMetadata(RescreenReason),
    /// Captured frame: delete it with its texts, elements and media.
    Delete(RescreenReason),
}

/// Mask earlier `[SECRET]` labels (as the capture-time self-check does) and
/// split texts longer than [`CHUNK_BYTES`] into overlapping windows.
fn masked_windows<'a>(texts: impl IntoIterator<Item = &'a str>) -> Vec<Cow<'a, str>> {
    let mut out = Vec::new();
    for text in texts.into_iter().filter(|text| !text.is_empty()) {
        match mask_secret_labels(text) {
            Cow::Borrowed(text) => {
                out.extend(screening_windows(text).into_iter().map(Cow::Borrowed))
            }
            Cow::Owned(text) => out.extend(
                screening_windows(&text)
                    .into_iter()
                    .map(|window| Cow::Owned(window.to_string())),
            ),
        }
    }
    out
}

/// Decide what the current rules do to a stored frame.
pub fn decide_frame(frame: &StoredFrame<'_>, rules: &RescreenRules) -> FrameDecision {
    // The frame metadata capture screens: title, URL, document path (the
    // executable name is not screened).
    let metadata = [frame.window_name, frame.browser_url, frame.document_path]
        .into_iter()
        .flatten()
        .filter(|text| !text.trim().is_empty());
    let window = rules.window_reason(frame.app_name, frame.window_name, frame.browser_url);
    if frame.placeholder {
        // The notice image and marker text hold no captured content; only
        // the recorded identity can still disclose anything.
        if let Some(reason) = window {
            return FrameDecision::ClearMetadata(reason);
        }
        let texts = masked_windows(metadata);
        if texts.is_empty() {
            return FrameDecision::Keep;
        }
        let refs: Vec<&str> = texts.iter().map(|text| text.as_ref()).collect();
        let screened = FrameTexts {
            window_title: frame.window_name,
            window_texts: &refs,
            shows_only_window: true,
            ..Default::default()
        };
        return match tier_reason(&screen_frame(&screened, &rules.screening).tier(&screened, false))
        {
            Some(reason) => FrameDecision::ClearMetadata(reason),
            None => FrameDecision::Keep,
        };
    }
    if let Some(reason) = window {
        return FrameDecision::Delete(reason);
    }
    let window_texts = masked_windows(metadata.chain(frame.window_texts.iter().copied()));
    // Stored OCR keeps its word positions only while it fits one window.
    let ocr = frame.ocr_text.filter(|text| !text.is_empty());
    let ocr_fits = ocr.is_some_and(|text| text.len() <= CHUNK_BYTES);
    let ocr_masked = ocr.filter(|_| ocr_fits).map(mask_secret_labels);
    let positions = ocr_masked
        .as_ref()
        .is_some_and(|masked| matches!(masked, Cow::Borrowed(_)));
    let image_texts = masked_windows(
        ocr.filter(|_| !ocr_fits)
            .into_iter()
            .chain(frame.image_texts.iter().copied()),
    );
    let window_refs: Vec<&str> = window_texts.iter().map(|text| text.as_ref()).collect();
    let image_refs: Vec<&str> = image_texts.iter().map(|text| text.as_ref()).collect();
    let screened = FrameTexts {
        window_title: frame.window_name,
        window_texts: &window_refs,
        ocr_text: ocr_masked.as_deref(),
        // Masking changes nothing in length, but positions are only used for
        // unmasked text, where the words still equal the stored OCR words.
        ocr_words: frame.ocr_words.as_deref().filter(|_| positions),
        image_texts: &image_refs,
        visible_windows: &[],
        shows_only_window: frame.single_window,
    };
    match tier_reason(&screen_frame(&screened, &rules.screening).tier(&screened, false)) {
        Some(reason) => FrameDecision::Delete(reason),
        None => FrameDecision::Keep,
    }
}

/// Overlapping windows of at most [`CHUNK_BYTES`] covering `text`, cut at
/// char boundaries.
fn screening_windows(text: &str) -> Vec<&str> {
    if text.len() <= CHUNK_BYTES {
        return vec![text];
    }
    let mut windows = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + CHUNK_BYTES).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        windows.push(&text[start..end]);
        if end == text.len() {
            break;
        }
        let mut next = end.saturating_sub(CHUNK_OVERLAP_BYTES).max(start + 1);
        while !text.is_char_boundary(next) {
            next += 1;
        }
        start = next;
    }
    windows
}

/// One stored input event, borrowed from the caller's in-memory row.
#[derive(Default)]
pub struct StoredUiEvent<'a> {
    pub app_name: Option<&'a str>,
    pub window_title: Option<&'a str>,
    pub browser_url: Option<&'a str>,
    /// Fields screened and, on a redactable hit, rewritten, in
    /// [`UI_EVENT_TEXT_FIELDS`] order.
    pub fields: [Option<&'a str>; UI_EVENT_TEXT_FIELDS.len()],
}

/// The `ui_events` text columns screened and rewritten by
/// [`decide_ui_event`], in the order of [`StoredUiEvent::fields`].
pub const UI_EVENT_TEXT_FIELDS: [&str; 6] = [
    "text_content",
    "window_title",
    "browser_url",
    "element_name",
    "element_value",
    "element_description",
];

/// What to do with a stored input event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiEventDecision {
    Keep,
    Delete(RescreenReason),
    /// Replace the listed fields (`Some`) with their redacted text.
    Redact(Box<[Option<String>; UI_EVENT_TEXT_FIELDS.len()]>),
}

/// Fields longer than this are not rewritten in place: the event is deleted.
pub const MAX_REDACTED_FIELD_BYTES: usize = 64 * 1024;

/// The tier of an input event's fields: the event is a frame of its own
/// window (the shared frame decision, `shows_only_window`).
fn ui_event_tier(
    title: Option<&str>,
    fields: &[&str],
    config: &ContentScreeningConfig,
) -> (ContentScreeningTier, crate::frame_screening::FramePlan) {
    let screened = FrameTexts {
        window_title: title,
        window_texts: fields,
        shows_only_window: true,
        ..Default::default()
    };
    let plan = screen_frame(&screened, config);
    (plan.tier(&screened, false), plan)
}

/// Decide what the current rules do to a stored input event.
pub fn decide_ui_event(event: &StoredUiEvent<'_>, rules: &RescreenRules) -> UiEventDecision {
    if let Some(reason) = rules.window_reason(event.app_name, event.window_title, event.browser_url)
    {
        return UiEventDecision::Delete(reason);
    }
    let fields: Vec<&str> = event.fields.iter().map(|f| f.unwrap_or_default()).collect();
    let masked: Vec<Cow<'_, str>> = fields.iter().map(|text| mask_secret_labels(text)).collect();
    let refs: Vec<&str> = masked.iter().map(|text| text.as_ref()).collect();
    let (tier, plan) = ui_event_tier(event.window_title, &refs, &rules.screening);
    match &tier {
        ContentScreeningTier::Pass => return UiEventDecision::Keep,
        ContentScreeningTier::Drop(_) => {
            return UiEventDecision::Delete(tier_reason(&tier).expect("drop has a reason"))
        }
        ContentScreeningTier::Redact => {}
    }
    let secret = RescreenReason::Content(ContentScreeningReason::Secret);
    if fields
        .iter()
        .any(|text| text.len() > MAX_REDACTED_FIELD_BYTES)
    {
        return UiEventDecision::Delete(secret);
    }
    let mut out: [Option<String>; UI_EVENT_TEXT_FIELDS.len()] = Default::default();
    for (index, field) in event.fields.iter().enumerate() {
        let spans = plan.plan.secret_spans(index);
        if spans.is_empty() {
            continue;
        }
        let Some(field) = field else {
            return UiEventDecision::Delete(secret);
        };
        match replace_secret_spans(field, spans) {
            Ok(redacted) => out[index] = Some(redacted),
            Err(_) => return UiEventDecision::Delete(secret),
        }
    }
    // Self-check: the rewritten event must pass every rule, including the
    // window rules on a rewritten title or URL.
    let rewritten: Vec<&str> = (0..UI_EVENT_TEXT_FIELDS.len())
        .map(|i| out[i].as_deref().or(event.fields[i]).unwrap_or_default())
        .collect();
    let rewritten_masked: Vec<Cow<'_, str>> = rewritten
        .iter()
        .map(|text| mask_secret_labels(text))
        .collect();
    let rewritten_refs: Vec<&str> = rewritten_masked.iter().map(|text| text.as_ref()).collect();
    if ui_event_tier(event.window_title, &rewritten_refs, &rules.screening).0
        != ContentScreeningTier::Pass
        || rules
            .window_reason(event.app_name, Some(rewritten[1]), Some(rewritten[2]))
            .is_some()
    {
        return UiEventDecision::Delete(secret);
    }
    UiEventDecision::Redact(Box::new(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_redaction::SECRET_LABEL;

    fn rules() -> RescreenRules {
        RescreenRules::new(
            &["::confidential".to_string()],
            &["bank.example".to_string()],
            &["project falcon".to_string()],
            false,
        )
    }

    /// A deterministic synthetic token (never a real credential).
    fn token() -> String {
        let mut state = 0x5101_C0DE_u64;
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
            if !crate::content_screening::generic_token_hits(&format!("token {candidate}"), false)
                .is_empty()
            {
                return candidate;
            }
        }
    }

    /// A stored frame whose texts are image-derived (stored full text).
    fn frame<'a>(title: &'a str, texts: Vec<&'a str>) -> StoredFrame<'a> {
        StoredFrame {
            app_name: Some("notepad.exe"),
            window_name: Some(title),
            image_texts: texts,
            ..Default::default()
        }
    }

    #[test]
    fn credential_file_frames_are_deleted_and_placeholders_lose_metadata() {
        let rules = rules();
        let captured = frame("*deploy-token.secret - Notepad", vec!["harmless text"]);
        assert_eq!(
            decide_frame(&captured, &rules),
            FrameDecision::Delete(RescreenReason::CredentialFileTitle)
        );
        let placeholder = StoredFrame {
            placeholder: true,
            ..frame("id_rsa - Notepad", vec![])
        };
        assert_eq!(
            decide_frame(&placeholder, &rules),
            FrameDecision::ClearMetadata(RescreenReason::CredentialFileTitle)
        );
        let bare = StoredFrame {
            placeholder: true,
            ..Default::default()
        };
        assert_eq!(decide_frame(&bare, &rules), FrameDecision::Keep);
    }

    #[test]
    fn window_rules_and_content_rules_apply_to_frames() {
        let rules = rules();
        let token = token();
        let body = format!("File Edit View\nread token\n{token}");
        let cases = [
            (frame("notes.txt - Notepad", vec!["ordinary notes"]), None),
            (
                frame("Confidential plan - Notepad", vec![]),
                Some(RescreenReason::IgnoredWindow),
            ),
            (
                StoredFrame {
                    app_name: Some("1Password.exe"),
                    ..Default::default()
                },
                Some(RescreenReason::ProtectedApp),
            ),
            (
                StoredFrame {
                    app_name: Some("msedge.exe"),
                    browser_url: Some("https://login.bank.example/home"),
                    ..Default::default()
                },
                Some(RescreenReason::IgnoredUrl),
            ),
            (
                frame("notes.txt - Notepad", vec![body.as_str()]),
                Some(RescreenReason::RedactableSecret),
            ),
            (
                frame(
                    "notes.txt - Notepad",
                    vec!["-----BEGIN RSA PRIVATE KEY-----"],
                ),
                Some(RescreenReason::Content(ContentScreeningReason::Secret)),
            ),
            (
                frame("notes.txt - Notepad", vec!["Project Falcon budget"]),
                Some(RescreenReason::Content(ContentScreeningReason::Keyword)),
            ),
        ];
        for (stored, expected) in cases {
            let decision = decide_frame(&stored, &rules);
            match expected {
                None => assert_eq!(decision, FrameDecision::Keep),
                Some(reason) => assert_eq!(decision, FrameDecision::Delete(reason)),
            }
        }
    }

    #[test]
    fn a_credential_word_title_lends_context_as_at_capture_time() {
        let rules = rules();
        let token = token();
        let body = format!("first line\n{token}\nlast line");
        // Accessibility (window) texts are in context whatever the outcome.
        let window_text = StoredFrame {
            window_texts: vec![body.as_str()],
            ..frame("API token rotation - Docs", vec![])
        };
        assert_eq!(
            decide_frame(&window_text, &rules),
            FrameDecision::Delete(RescreenReason::RedactableSecret)
        );
        // Image texts only when the image showed just that window.
        let image = |single_window| StoredFrame {
            single_window,
            ..frame("API token rotation - Docs", vec![body.as_str()])
        };
        assert_eq!(
            decide_frame(&image(true), &rules),
            FrameDecision::Delete(RescreenReason::RedactableSecret)
        );
        assert_eq!(decide_frame(&image(false), &rules), FrameDecision::Keep);
        assert_eq!(
            decide_frame(
                &StoredFrame {
                    single_window: true,
                    ..frame("Quarterly report - Word", vec![body.as_str()])
                },
                &rules
            ),
            FrameDecision::Keep
        );
        // Typed into a titled window: redacted.
        let UiEventDecision::Redact(fields) = decide_ui_event(
            &event("Vault settings - Browser", Some(token.as_str())),
            &rules,
        ) else {
            panic!("expected a redaction");
        };
        assert_eq!(fields[0].as_deref(), Some(SECRET_LABEL));
        assert_eq!(
            decide_ui_event(
                &event("Quarterly report - Word", Some(token.as_str())),
                &rules
            ),
            UiEventDecision::Keep
        );
    }

    #[test]
    fn stored_ocr_positions_are_not_needed_without_visible_windows() {
        let rules = rules();
        let token = token();
        let ocr = format!("{token} {token}");
        let words = vec![
            OcrWord {
                text: &token,
                left: 0.1,
                top: 0.5,
                width: 0.2,
                height: 0.02,
            };
            2
        ];
        let stored = StoredFrame {
            ocr_text: Some(ocr.as_str()),
            ocr_words: Some(words),
            single_window: true,
            ..frame("API token rotation - Docs", vec![])
        };
        assert_eq!(
            decide_frame(&stored, &rules),
            FrameDecision::Delete(RescreenReason::RedactableSecret)
        );
    }

    #[test]
    fn earlier_redaction_labels_are_not_hits() {
        let mut rules = rules();
        rules.screening.keywords = vec!["secret".to_string()];
        let text = format!("api token {SECRET_LABEL} rotated");
        assert_eq!(
            decide_frame(&frame("notes.txt - Notepad", vec![text.as_str()]), &rules),
            FrameDecision::Keep
        );
    }

    #[test]
    fn long_texts_are_screened_beyond_the_in_memory_bound() {
        let rules = rules();
        let token = token();
        let filler = "lorem ipsum ".repeat(30_000);
        let text = format!("{filler}\napi token {token}\n{filler}");
        assert!(text.len() > 2 * CHUNK_BYTES);
        assert_eq!(
            decide_frame(&frame("notes.txt - Notepad", vec![text.as_str()]), &rules),
            FrameDecision::Delete(RescreenReason::RedactableSecret)
        );
        let windows = screening_windows(&text);
        assert!(windows.iter().all(|w| w.len() <= CHUNK_BYTES));
        assert_eq!(windows.first().map(|w| w.as_ptr()), Some(text.as_ptr()));
        assert!(windows.last().is_some_and(|w| text.ends_with(w)));
    }

    fn event<'a>(title: &'a str, text: Option<&'a str>) -> StoredUiEvent<'a> {
        StoredUiEvent {
            app_name: Some("notepad.exe"),
            window_title: Some(title),
            browser_url: None,
            fields: [text, Some(title), None, None, None, None],
        }
    }

    #[test]
    fn ui_events_are_deleted_or_redacted() {
        let rules = rules();
        assert_eq!(
            decide_ui_event(&event("*deploy-token.secret - Notepad", None), &rules),
            UiEventDecision::Delete(RescreenReason::CredentialFileTitle)
        );
        assert_eq!(
            decide_ui_event(&event("notes.txt - Notepad", Some("hello")), &rules),
            UiEventDecision::Keep
        );
        assert_eq!(
            decide_ui_event(
                &event("notes.txt - Notepad", Some("about project falcon")),
                &rules
            ),
            UiEventDecision::Delete(RescreenReason::Content(ContentScreeningReason::Keyword))
        );
        let token = token();
        let typed = format!("api token {token}");
        let UiEventDecision::Redact(fields) =
            decide_ui_event(&event("notes.txt - Notepad", Some(typed.as_str())), &rules)
        else {
            panic!("expected a redaction");
        };
        assert_eq!(
            fields[0].as_deref(),
            Some(format!("api token {SECRET_LABEL}").as_str())
        );
        assert!(fields[1..].iter().all(Option::is_none));
    }

    #[test]
    fn decisions_and_rules_never_print_content() {
        let rules = rules();
        let debug = format!("{rules:?}");
        assert!(!debug.contains("confidential") && !debug.contains("bank.example"));
        for reason in [
            RescreenReason::CredentialFileTitle,
            RescreenReason::ProtectedApp,
            RescreenReason::IgnoredWindow,
            RescreenReason::IgnoredUrl,
            RescreenReason::Content(ContentScreeningReason::Secret),
            RescreenReason::RedactableSecret,
        ] {
            assert!(reason
                .code()
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'));
        }
    }
}
