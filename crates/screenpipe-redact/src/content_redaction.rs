//! Pure helpers for the content-screening redaction tier (owner decision
//! 2026-10-15): a frame whose only screening hits are redactable secrets is
//! redacted in memory before anything is written, instead of being withheld.
//!
//! - [`replace_secret_spans`] is the one replacement function for every
//!   persisted text representation (OCR text, OCR JSON words, accessibility
//!   text and tree node fields, window metadata). It never repairs its input:
//!   any malformed span is an explicit [`ReplacementError`], and the caller
//!   withholds the whole frame.
//! - [`SecretMatcher`] finds every plausible location of the detected
//!   secrets in a text, by provenance ([`SecretHitKind`]): exact copies and
//!   fragments of at least [`MIN_PARTIAL_MATCH_CHARS`] consecutive
//!   characters of a detector secret; card copies by digit sequence; exact
//!   copies of URL components (and of any secret shorter than
//!   [`MIN_PARTIAL_MATCH_CHARS`]) inside URL tokens only.
//! - [`mask_secret_labels`] hides inserted [`SECRET_LABEL`]s from the
//!   post-redaction self-check.
//! - [`check_secret_coverage`] and [`redacted_area_exceeds_limit`] apply the
//!   whole-frame thresholds defined in [`crate::content_screening`].
//! - [`locate_ocr_hits`] maps hit spans in the flat OCR text to the OCR
//!   words that produced them, groups the words into visual lines and returns
//!   padded pixel rectangles covering every affected line, for
//!   [`crate::image::frame_redactor::redact_regions_in_place`].
//!
//! Every helper is deterministic, does no I/O and no logging. Hit positions
//! and the original text never leave the caller's process memory; nothing
//! here formats them into a string other than the redacted output, and the
//! `Debug` output of the match types prints counts only.
//!
//! Limits: line grouping is geometric (Windows OCR JSON carries no line
//! index). Words of two lines at the same height in different columns are
//! merged into one wider region, which over-covers rather than under-covers.
//! A secret the OCR engine split across a line wrap is only covered where a
//! fragment of at least [`MIN_PARTIAL_MATCH_CHARS`] characters survives;
//! shorter fragments, OCR misreadings and encoded copies are not matched.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::ops::Range;

use screenpipe_capture_policy::url_filter;

pub use crate::content_screening::{
    SecretHitKind, MAX_MATCHER_SECRETS, MAX_REDACTED_AREA_PERCENT, MAX_REDACTION_HITS,
    MAX_SECRET_LOCATIONS, MIN_PARTIAL_MATCH_CHARS,
};
use crate::image::frame_redactor::PixelRect;

/// Fixed replacement for every redacted hit.
pub const SECRET_LABEL: &str = "[SECRET]";

/// Minimum padding, in pixels, on each side of a redacted OCR line.
pub const MIN_LINE_PADDING_PX: u32 = 4;

/// Why [`replace_secret_spans`] refused its input. Carries no text or
/// positions; any variant withholds the whole frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplacementError {
    /// A span ends past the end of the text.
    OutOfBounds,
    /// A span starts or ends inside a multi-byte character.
    NotCharBoundary,
    /// A span is empty (`start >= end`).
    EmptySpan,
    /// A span starts before the previous span's start.
    Unsorted,
    /// A span starts before the previous span's end.
    Overlapping,
}

impl fmt::Display for ReplacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ReplacementError::OutOfBounds => "span out of bounds",
            ReplacementError::NotCharBoundary => "span not on a char boundary",
            ReplacementError::EmptySpan => "empty span",
            ReplacementError::Unsorted => "spans not sorted",
            ReplacementError::Overlapping => "spans overlap",
        })
    }
}

impl std::error::Error for ReplacementError {}

/// Replace each byte span of `text` with [`SECRET_LABEL`].
///
/// The spans must be non-empty, inside `text`, on UTF-8 char boundaries,
/// sorted and non-overlapping (touching is allowed); use [`merge_spans`] to
/// build such a list. Nothing is clamped, widened or reordered: a span that
/// violates a rule returns the matching [`ReplacementError`]. No spans
/// returns an unchanged copy.
pub fn replace_secret_spans(
    text: &str,
    spans: &[Range<usize>],
) -> Result<String, ReplacementError> {
    let mut previous: Option<&Range<usize>> = None;
    for span in spans {
        if span.start >= span.end {
            return Err(ReplacementError::EmptySpan);
        }
        if span.end > text.len() {
            return Err(ReplacementError::OutOfBounds);
        }
        if !text.is_char_boundary(span.start) || !text.is_char_boundary(span.end) {
            return Err(ReplacementError::NotCharBoundary);
        }
        if let Some(previous) = previous {
            if span.start < previous.start {
                return Err(ReplacementError::Unsorted);
            }
            if span.start < previous.end {
                return Err(ReplacementError::Overlapping);
            }
        }
        previous = Some(span);
    }
    let mut out = String::with_capacity(text.len() + spans.len() * SECRET_LABEL.len());
    let mut cursor = 0;
    for span in spans {
        out.push_str(&text[cursor..span.start]);
        out.push_str(SECRET_LABEL);
        cursor = span.end;
    }
    out.push_str(&text[cursor..]);
    Ok(out)
}

/// Placeholder for one masked byte of a [`SECRET_LABEL`]: U+0001 is a single
/// byte that is neither a word character nor whitespace, like the label's
/// brackets, so masking keeps byte offsets and token boundaries.
const MASK_CHAR: char = '\u{1}';

/// `text` with every [`SECRET_LABEL`] replaced by the same number of
/// [`MASK_CHAR`]s, for the post-redaction self-check: a user keyword such as
/// `secret` must not match the labels redaction inserted. A label that was
/// already in the original text was screened there (any keyword or rule hit
/// on it withheld the frame before redaction), so masking it as well hides
/// nothing new. Borrowed when there is no label.
pub fn mask_secret_labels(text: &str) -> Cow<'_, str> {
    if !text.contains(SECRET_LABEL) {
        return Cow::Borrowed(text);
    }
    let mask: String = std::iter::repeat(MASK_CHAR)
        .take(SECRET_LABEL.len())
        .collect();
    Cow::Owned(text.replace(SECRET_LABEL, &mask))
}

/// Sort `spans` and merge every pair that overlaps or touches, dropping
/// empty spans. The result satisfies [`replace_secret_spans`]'s ordering
/// rules; bounds and char boundaries are still checked there.
pub fn merge_spans(spans: impl IntoIterator<Item = Range<usize>>) -> Vec<Range<usize>> {
    let mut spans: Vec<Range<usize>> = spans.into_iter().filter(|s| s.start < s.end).collect();
    spans.sort_by_key(|s| (s.start, s.end));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
            _ => merged.push(span),
        }
    }
    merged
}

/// Plausible locations of one secret in one text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SecretCount {
    /// Non-overlapping exact copies.
    pub exact: usize,
    /// Fragments of at least [`MIN_PARTIAL_MATCH_CHARS`] consecutive
    /// characters of the secret that do not overlap an exact copy.
    pub partial: usize,
}

impl SecretCount {
    /// Every plausible location (exact copies plus fragments).
    pub fn total(&self) -> usize {
        self.exact + self.partial
    }
}

impl std::ops::AddAssign for SecretCount {
    fn add_assign(&mut self, other: Self) {
        self.exact += other.exact;
        self.partial += other.partial;
    }
}

/// Where the secrets of a [`SecretMatcher`] occur in one text. The spans are
/// in-memory only; `Debug` prints counts.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct TextMatches {
    counts: Vec<SecretCount>,
    spans: Vec<Range<usize>>,
}

impl fmt::Debug for TextMatches {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextMatches")
            .field("counts", &self.counts)
            .field("spans", &self.spans.len())
            .finish()
    }
}

impl TextMatches {
    /// Merged replacement spans: exact copies as matched, fragments widened
    /// to the whole whitespace-delimited token that contains them.
    pub fn spans(&self) -> &[Range<usize>] {
        &self.spans
    }

    /// Locations of secret `index` (zero when out of range).
    pub fn count(&self, index: usize) -> SecretCount {
        self.counts.get(index).copied().unwrap_or_default()
    }

    /// True when no secret occurs in the text, even partially.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Fragment (partial-match) locations across every secret.
    pub fn partial_locations(&self) -> usize {
        self.counts.iter().map(|c| c.partial).sum()
    }
}

struct SecretPattern<'s> {
    text: &'s str,
    /// Card secrets: the digit sequence, matched across whitespace and `-`.
    digits: Option<String>,
    /// Exact copies count only inside URL tokens of the scanned text.
    url_only: bool,
    /// Exact copies must not touch a token character (alphanumeric, `_`, `-`)
    /// on either side ([`SecretHitKind::UrlValue`]).
    bounded: bool,
    /// Every window of [`MIN_PARTIAL_MATCH_CHARS`] consecutive characters
    /// (empty for `url_only` patterns: they are never matched partially).
    windows: HashSet<&'s str>,
    /// First bytes of `windows`, a cheap prefilter for the text scan.
    first_bytes: [bool; 256],
}

/// Finds every plausible location of up to [`MAX_MATCHER_SECRETS`] detected
/// hit texts in a text, according to their provenance:
///
/// - [`SecretHitKind::Detector`]: an exact, case-sensitive copy, or a
///   fragment: a maximal run of overlapping
///   [`MIN_PARTIAL_MATCH_CHARS`]-character windows of the text that are each
///   a substring of the secret and do not overlap an exact copy;
/// - [`SecretHitKind::Card`]: the card's digit sequence, with any Unicode
///   whitespace or `-` between digits (`4111 1111…`, `4111-1111…`, NBSP or
///   line-break separated within one text), plus fragments as above; the
///   location spans the original separators;
/// - [`SecretHitKind::UrlValue`]: an exact copy anywhere, not touching an
///   alphanumeric/`_`/`-` character on either side, never partial;
/// - [`SecretHitKind::UrlPart`], and any secret shorter than
///   [`MIN_PARTIAL_MATCH_CHARS`] characters: exact copies lying inside a URL
///   token ([`url_filter::url_token_spans`]) of the scanned text only, never
///   partial and never in ordinary words.
///
/// Work is bounded: per secret one exact scan and one window lookup per text
/// character (hash set of at most one window per secret character).
pub struct SecretMatcher<'s> {
    secrets: Vec<SecretPattern<'s>>,
}

impl fmt::Debug for SecretMatcher<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretMatcher")
            .field("secrets", &self.secrets.len())
            .finish()
    }
}

/// Byte offset of every char start in `text`, followed by `text.len()`.
fn char_boundaries(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .collect()
}

impl<'s> SecretMatcher<'s> {
    /// `None` (the caller withholds the frame) when there are more than
    /// [`MAX_MATCHER_SECRETS`] secrets, any secret is empty, or a card
    /// secret has no digit.
    pub fn new(secrets: &[(&'s str, SecretHitKind)]) -> Option<Self> {
        if secrets.len() > MAX_MATCHER_SECRETS || secrets.iter().any(|(s, _)| s.is_empty()) {
            return None;
        }
        let secrets = secrets
            .iter()
            .map(|&(text, kind)| {
                let bounds = char_boundaries(text);
                let chars = bounds.len() - 1;
                let digits = match kind {
                    SecretHitKind::Card => {
                        let digits: String = text.chars().filter(char::is_ascii_digit).collect();
                        if digits.is_empty() {
                            return None;
                        }
                        Some(digits)
                    }
                    _ => None,
                };
                let value = kind == SecretHitKind::UrlValue;
                let url_only =
                    kind == SecretHitKind::UrlPart || (!value && chars < MIN_PARTIAL_MATCH_CHARS);
                let mut windows = HashSet::new();
                let mut first_bytes = [false; 256];
                if !url_only && !value {
                    for start in 0..bounds.len().saturating_sub(MIN_PARTIAL_MATCH_CHARS) {
                        let window = &text[bounds[start]..bounds[start + MIN_PARTIAL_MATCH_CHARS]];
                        first_bytes[usize::from(window.as_bytes()[0])] = true;
                        windows.insert(window);
                    }
                }
                Some(SecretPattern {
                    text,
                    digits,
                    url_only,
                    bounded: value,
                    windows,
                    first_bytes,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self { secrets })
    }

    /// Number of secrets.
    pub fn len(&self) -> usize {
        self.secrets.len()
    }

    /// True without secrets.
    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }

    /// Every plausible location of every secret in `text`.
    pub fn locate(&self, text: &str) -> TextMatches {
        let mut counts = Vec::with_capacity(self.secrets.len());
        let mut spans = Vec::new();
        let mut bounds: Option<Vec<usize>> = None;
        let mut url_tokens: Option<Vec<Range<usize>>> = None;
        for secret in &self.secrets {
            let mut exact: Vec<Range<usize>> = match &secret.digits {
                Some(digits) => card_copies(text, digits),
                None => text
                    .match_indices(secret.text)
                    .map(|(start, hit)| start..start + hit.len())
                    .collect(),
            };
            if secret.bounded {
                exact.retain(|copy| {
                    !text[..copy.start]
                        .chars()
                        .next_back()
                        .is_some_and(is_token_char)
                        && !text[copy.end..].chars().next().is_some_and(is_token_char)
                });
            }
            if secret.url_only {
                let tokens =
                    url_tokens.get_or_insert_with(|| url_filter::url_token_spans(text).collect());
                exact.retain(|copy| {
                    tokens
                        .iter()
                        .any(|token| token.start <= copy.start && copy.end <= token.end)
                });
            }
            let mut fragments: Vec<Range<usize>> = Vec::new();
            if !secret.windows.is_empty() && text.len() >= MIN_PARTIAL_MATCH_CHARS {
                let bounds = bounds.get_or_insert_with(|| char_boundaries(text));
                let mut next_exact = 0;
                for start in 0..bounds.len().saturating_sub(MIN_PARTIAL_MATCH_CHARS) {
                    let window = bounds[start]..bounds[start + MIN_PARTIAL_MATCH_CHARS];
                    while exact.get(next_exact).is_some_and(|e| e.end <= window.start) {
                        next_exact += 1;
                    }
                    if exact.get(next_exact).is_some_and(|e| e.start < window.end) {
                        continue;
                    }
                    if !secret.first_bytes[usize::from(text.as_bytes()[window.start])]
                        || !secret.windows.contains(&text[window.clone()])
                    {
                        continue;
                    }
                    match fragments.last_mut() {
                        // Overlapping windows extend one fragment; merely
                        // touching windows are separate locations.
                        Some(last) if window.start < last.end => last.end = window.end,
                        _ => fragments.push(window),
                    }
                }
            }
            counts.push(SecretCount {
                exact: exact.len(),
                partial: fragments.len(),
            });
            spans.extend(exact);
            spans.extend(fragments.into_iter().map(|f| widen_to_token(text, f)));
        }
        TextMatches {
            counts,
            spans: merge_spans(spans),
        }
    }
}

/// A dash between card digits: ASCII `-`, the Unicode dashes U+2010..U+2015
/// (hyphen to horizontal bar) and the minus sign U+2212.
fn is_card_dash(c: char) -> bool {
    matches!(c, '-' | '\u{2010}'..='\u{2015}' | '\u{2212}')
}

/// True if `c` may be part of an identifier-like token (alphanumeric, `_`, `-`).
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Non-overlapping copies of a card's `digits` in `text`: runs of ASCII
/// digits separated only by Unicode whitespace or dashes are compared on their
/// digit sequence, and a copy spans from its first to its last digit (any
/// separators in between included).
fn card_copies(text: &str, digits: &str) -> Vec<Range<usize>> {
    fn flush(
        run: &mut Vec<usize>,
        run_digits: &mut String,
        digits: &str,
        out: &mut Vec<Range<usize>>,
    ) {
        let mut from = 0;
        while let Some(found) = run_digits[from..].find(digits) {
            let at = from + found;
            out.push(run[at]..run[at + digits.len() - 1] + 1);
            from = at + digits.len();
        }
        run.clear();
        run_digits.clear();
    }
    let mut copies = Vec::new();
    // Byte offset of each digit of the current run, and the digits.
    let mut run = Vec::new();
    let mut run_digits = String::new();
    for (index, c) in text.char_indices() {
        if c.is_ascii_digit() {
            run.push(index);
            run_digits.push(c);
        } else if !(c.is_whitespace() || is_card_dash(c)) {
            flush(&mut run, &mut run_digits, digits, &mut copies);
        }
    }
    flush(&mut run, &mut run_digits, digits, &mut copies);
    copies
}

/// Widen `span` to the whitespace-delimited token that contains it.
fn widen_to_token(text: &str, span: Range<usize>) -> Range<usize> {
    let start = text[..span.start]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(index, c)| index + c.len_utf8());
    let end = text[span.end..]
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map_or(text.len(), |(index, _)| span.end + index);
    start..end
}

/// Plausible locations of one secret per source of persisted text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SecretCopies {
    /// Located in the OCR text (and so mapped to OCR lines).
    pub ocr: usize,
    /// In the accessibility text and tree node fields, counted without
    /// double-counting representations of the same content.
    pub accessibility: usize,
    /// In window metadata (title, browser URL, document path), summed.
    pub metadata: usize,
    /// Accessibility nodes (the address bar) whose value or url equals the
    /// browser URL and hold this secret: they duplicate the browser-URL
    /// metadata copy, so one metadata copy per such node is not counted again
    /// as visible. Never more than the browser-URL copies of `metadata`.
    pub address_bar_duplicates: usize,
}

/// Why [`check_secret_coverage`] withholds a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverageError {
    /// More than [`MAX_SECRET_LOCATIONS`] locations of one secret in a source.
    TooManyLocations,
    /// Pixels are persisted and a source shows more copies of a secret than
    /// OCR located (an accessibility/metadata hit without any located OCR
    /// copy included): some copy may be visible but not masked.
    UncoveredCopies,
}

/// Whole-frame thresholds on per-secret copies: any source with more than
/// [`MAX_SECRET_LOCATIONS`] locations of one secret, or — when pixels are
/// persisted — an accessibility or metadata count above the OCR count.
pub fn check_secret_coverage(
    copies: &[SecretCopies],
    pixels_persisted: bool,
) -> Result<(), CoverageError> {
    for c in copies {
        if c.ocr.max(c.accessibility).max(c.metadata) > MAX_SECRET_LOCATIONS {
            return Err(CoverageError::TooManyLocations);
        }
    }
    // Visible copies are accessibility + window-title + document-path +
    // browser-URL copies, minus the URL copies the address-bar accessibility
    // node duplicates.
    if pixels_persisted
        && copies
            .iter()
            .any(|c| c.accessibility + c.metadata.saturating_sub(c.address_bar_duplicates) > c.ocr)
    {
        return Err(CoverageError::UncoveredCopies);
    }
    Ok(())
}

/// Pixels covered by the union of `rects`, clamped to the image.
pub fn union_area(rects: &[PixelRect], image_width: u32, image_height: u32) -> u64 {
    let clamped: Vec<(u32, u32, u32, u32)> = rects
        .iter()
        .filter_map(|r| {
            let x1 = r.x.saturating_add(r.width).min(image_width);
            let y1 = r.y.saturating_add(r.height).min(image_height);
            (r.x < x1 && r.y < y1).then_some((r.x, r.y, x1, y1))
        })
        .collect();
    let mut xs: Vec<u32> = clamped.iter().flat_map(|r| [r.0, r.2]).collect();
    let mut ys: Vec<u32> = clamped.iter().flat_map(|r| [r.1, r.3]).collect();
    xs.sort_unstable();
    xs.dedup();
    ys.sort_unstable();
    ys.dedup();
    let mut area = 0u64;
    for x in xs.windows(2) {
        for y in ys.windows(2) {
            if clamped
                .iter()
                .any(|r| r.0 <= x[0] && x[1] <= r.2 && r.1 <= y[0] && y[1] <= r.3)
            {
                area += u64::from(x[1] - x[0]) * u64::from(y[1] - y[0]);
            }
        }
    }
    area
}

/// Upper bound on rectangles for the union-area check; more always exceeds.
const MAX_AREA_RECTS: usize = 256;

/// True when the union of `rects` covers more than
/// [`MAX_REDACTED_AREA_PERCENT`] of the image (or the image is empty, or
/// there are implausibly many rectangles).
pub fn redacted_area_exceeds_limit(
    rects: &[PixelRect],
    image_width: u32,
    image_height: u32,
) -> bool {
    if rects.is_empty() {
        return false;
    }
    let total = u64::from(image_width) * u64::from(image_height);
    if total == 0 || rects.len() > MAX_AREA_RECTS {
        return true;
    }
    union_area(rects, image_width, image_height) * 100 > MAX_REDACTED_AREA_PERCENT * total
}

/// One recognised OCR word and its bounding box, normalised to 0..1 of the
/// recognised image (top-left origin), as the platform OCR JSON stores it.
#[derive(Clone, Copy)]
pub struct OcrWord<'a> {
    pub text: &'a str,
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl OcrWord<'_> {
    fn valid(&self) -> bool {
        [self.left, self.top, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.width > 0.0
            && self.height > 0.0
    }

    fn bottom(&self) -> f64 {
        self.top + self.height
    }
}

/// Where the hits of one OCR result sit. Contains indices and pixel geometry
/// only, never text; `Debug` prints counts only.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct OcrHitLocation {
    /// One padded, image-clamped rectangle per affected OCR line.
    pub rects: Vec<PixelRect>,
    /// Indices (into the word list) of every word on an affected line, sorted.
    /// The caller replaces these words' texts in the OCR JSON.
    pub words: Vec<usize>,
}

impl fmt::Debug for OcrHitLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OcrHitLocation")
            .field("rects", &self.rects.len())
            .field("words", &self.words.len())
            .finish()
    }
}

/// Two consecutive OCR words belong to one visual line when the second does
/// not move left and they overlap vertically by at least half the shorter
/// word's height.
fn same_line(previous: &OcrWord<'_>, next: &OcrWord<'_>) -> bool {
    let overlap = previous.bottom().min(next.bottom()) - previous.top.max(next.top);
    next.left >= previous.left && overlap >= 0.5 * previous.height.min(next.height)
}

/// Locate redactable hits (byte spans of `ocr_text`) in the OCR `words` and
/// return the padded pixel rectangles of every OCR line containing any of a
/// hit's words.
///
/// Returns `None` — the caller must withhold the frame — when the hits cannot
/// be mapped reliably:
/// - `ocr_text` is not exactly the words joined by single spaces (the shape
///   Windows OCR produces), so byte offsets cannot be mapped to words;
/// - any word is empty or has a non-finite / non-positive box;
/// - a span is out of range or touches no word;
/// - an affected line's rectangle covers no pixel of the image, or the image
///   is empty.
///
/// Each rectangle is the union of the line's word boxes, padded on every side
/// by the larger of 25% of the line height and [`MIN_LINE_PADDING_PX`], then
/// clamped to the image.
pub fn locate_ocr_hits(
    ocr_text: &str,
    words: &[OcrWord<'_>],
    spans: &[Range<usize>],
    image_width: u32,
    image_height: u32,
) -> Option<OcrHitLocation> {
    if spans.is_empty() {
        return Some(OcrHitLocation::default());
    }
    if image_width == 0 || image_height == 0 || words.is_empty() {
        return None;
    }

    // Byte range of each word inside `ocr_text`.
    let bytes = ocr_text.as_bytes();
    let mut offsets = Vec::with_capacity(words.len());
    let mut pos = 0usize;
    for (index, word) in words.iter().enumerate() {
        if word.text.is_empty() || !word.valid() {
            return None;
        }
        if index > 0 {
            if bytes.get(pos) != Some(&b' ') {
                return None;
            }
            pos += 1;
        }
        let end = pos.checked_add(word.text.len())?;
        if bytes.get(pos..end) != Some(word.text.as_bytes()) {
            return None;
        }
        offsets.push(pos..end);
        pos = end;
    }
    if pos != bytes.len() {
        return None;
    }

    // Visual line of each word.
    let mut lines = Vec::with_capacity(words.len());
    let mut line = 0usize;
    for (index, word) in words.iter().enumerate() {
        if index > 0 && !same_line(&words[index - 1], word) {
            line += 1;
        }
        lines.push(line);
    }

    let mut affected = BTreeSet::new();
    for span in spans {
        if span.start >= span.end || span.end > bytes.len() {
            return None;
        }
        let mut touched = false;
        for (index, range) in offsets.iter().enumerate() {
            if range.start < span.end && span.start < range.end {
                affected.insert(lines[index]);
                touched = true;
            }
        }
        if !touched {
            return None;
        }
    }

    let (w, h) = (f64::from(image_width), f64::from(image_height));
    let mut location = OcrHitLocation::default();
    for &line in &affected {
        let members: Vec<usize> = (0..words.len()).filter(|&i| lines[i] == line).collect();
        let left = members
            .iter()
            .map(|&i| words[i].left)
            .fold(f64::MAX, f64::min);
        let top = members
            .iter()
            .map(|&i| words[i].top)
            .fold(f64::MAX, f64::min);
        let right = members
            .iter()
            .map(|&i| words[i].left + words[i].width)
            .fold(f64::MIN, f64::max);
        let bottom = members
            .iter()
            .map(|&i| words[i].bottom())
            .fold(f64::MIN, f64::max);
        let x0 = (left * w).floor();
        let y0 = (top * h).floor();
        let x1 = (right * w).ceil();
        let y1 = (bottom * h).ceil();
        let pad = (0.25 * (y1 - y0))
            .ceil()
            .max(f64::from(MIN_LINE_PADDING_PX));
        let rx0 = (x0 - pad).clamp(0.0, w);
        let ry0 = (y0 - pad).clamp(0.0, h);
        let rx1 = (x1 + pad).clamp(0.0, w);
        let ry1 = (y1 + pad).clamp(0.0, h);
        if rx1 <= rx0 || ry1 <= ry0 {
            return None;
        }
        location.rects.push(PixelRect {
            x: rx0 as u32,
            y: ry0 as u32,
            width: (rx1 - rx0) as u32,
            height: (ry1 - ry0) as u32,
        });
        location.words.extend(members);
    }
    location.words.sort_unstable();
    Some(location)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, left: f64, top: f64, width: f64, height: f64) -> OcrWord<'_> {
        OcrWord {
            text,
            left,
            top,
            width,
            height,
        }
    }

    fn span_of(text: &str, needle: &str) -> Range<usize> {
        let start = text.find(needle).expect("needle present");
        start..start + needle.len()
    }

    // --- Shared replacement function ---

    #[test]
    fn replacement_replaces_sorted_spans_and_keeps_the_rest() {
        let text = "key AKIA1234567890ABCDEF and more";
        let span = span_of(text, "AKIA1234567890ABCDEF");
        assert_eq!(
            replace_secret_spans(text, &[span]).unwrap(),
            "key [SECRET] and more"
        );
        // Touching spans are allowed; no spans copy the text unchanged.
        assert_eq!(
            replace_secret_spans("abcdefgh", &[1..3, 3..4, 6..8]).unwrap(),
            "a[SECRET][SECRET]ef[SECRET]"
        );
        assert_eq!(replace_secret_spans("plain", &[]).unwrap(), "plain");
        assert_eq!(replace_secret_spans("", &[]).unwrap(), "");
        assert_eq!(replace_secret_spans("abc", &[0..3]).unwrap(), SECRET_LABEL);
    }

    #[test]
    fn replacement_reports_explicit_errors_instead_of_repairing() {
        use ReplacementError::*;
        assert_eq!(replace_secret_spans("abc", &[1..4]), Err(OutOfBounds));
        assert_eq!(replace_secret_spans("abc", &[5..9]), Err(OutOfBounds));
        assert_eq!(replace_secret_spans("abc", &[1..1]), Err(EmptySpan));
        #[allow(clippy::reversed_empty_ranges)]
        let reversed = 2..1;
        assert_eq!(replace_secret_spans("abc", &[reversed]), Err(EmptySpan));
        assert_eq!(replace_secret_spans("abcdef", &[3..4, 0..1]), Err(Unsorted));
        assert_eq!(
            replace_secret_spans("abcdef", &[0..3, 2..4]),
            Err(Overlapping)
        );
        // Multi-byte: 'é' is 2 bytes, '秘' 3 bytes.
        let multi = "é秘密é";
        assert_eq!(replace_secret_spans(multi, &[1..2]), Err(NotCharBoundary));
        assert_eq!(replace_secret_spans(multi, &[2..3]), Err(NotCharBoundary));
        assert_eq!(
            replace_secret_spans(multi, &[2..5]).unwrap(),
            "é[SECRET]密é"
        );
        // Display is fixed and content-free.
        assert_eq!(Overlapping.to_string(), "spans overlap");
    }

    #[test]
    fn merge_spans_sorts_merges_and_drops_empty() {
        assert_eq!(
            merge_spans([4..6, 1..3, 2..4, 6..6, 7..9, 9..10]),
            vec![1..6, 7..10]
        );
        assert!(merge_spans(Vec::new()).is_empty());
        let merged = merge_spans([0..2, 5..7]);
        assert_eq!(
            replace_secret_spans("abcdefgh", &merged).unwrap(),
            "[SECRET]cde[SECRET]h"
        );
    }

    // --- Occurrence coverage and partial matches ---

    const KEY: &str = "AKIA1234567890ABCDEF";

    #[test]
    fn matcher_counts_every_exact_copy() {
        let m = SecretMatcher::new(&[(KEY, SecretHitKind::Detector)]).unwrap();
        let text = format!("{KEY} then {KEY}, and x{KEY}y");
        let found = m.locate(&text);
        assert_eq!(
            found.count(0),
            SecretCount {
                exact: 3,
                partial: 0
            }
        );
        let redacted = replace_secret_spans(&text, found.spans()).unwrap();
        assert_eq!(redacted, "[SECRET] then [SECRET], and x[SECRET]y");
        assert!(m.locate(&redacted).is_empty());
        assert!(m.locate("nothing to see").is_empty());
        assert_eq!(m.locate("").count(0), SecretCount::default());
    }

    #[test]
    fn matcher_finds_fragments_of_at_least_eight_chars_and_widens_to_token() {
        let m = SecretMatcher::new(&[(KEY, SecretHitKind::Detector)]).unwrap();
        // 8 consecutive characters of the key inside a token: a location.
        let found = m.locate("prefix x1234567890y suffix");
        assert_eq!(
            found.count(0),
            SecretCount {
                exact: 0,
                partial: 1
            }
        );
        assert_eq!(
            replace_secret_spans("prefix x1234567890y suffix", found.spans()).unwrap(),
            "prefix [SECRET] suffix"
        );
        // Exactly 8 characters matches, 7 does not.
        assert_eq!(m.locate("AKIA1234").count(0).partial, 1);
        assert!(m.locate("AKIA123 KIA1234 ").is_empty());
        // Case-sensitive.
        assert!(m.locate("akia1234 890abcdef").is_empty());
        // Two separated fragments are two locations; a fragment next to an
        // exact copy is counted separately.
        let found = m.locate("AKIA12345 and 7890ABCDEF");
        assert_eq!(
            found.count(0),
            SecretCount {
                exact: 0,
                partial: 2
            }
        );
        let text = format!("{KEY} AKIA1234");
        assert_eq!(
            m.locate(&text).count(0),
            SecretCount {
                exact: 1,
                partial: 1
            }
        );
        // Non-contiguous pieces of the secret that merely touch are separate
        // locations; a piece shorter than the window is not one.
        assert_eq!(m.locate("AKIA1234890ABCDE").count(0).partial, 2);
        assert_eq!(m.locate("AKIA1234ABCDEF00").count(0).partial, 1);
        assert_eq!(found.partial_locations(), 2);
    }

    #[test]
    fn matcher_is_char_based_for_multibyte_secrets_and_text() {
        let secret = "pässwörd-秘密-tökén";
        let m = SecretMatcher::new(&[(secret, SecretHitKind::Detector)]).unwrap();
        // 8 characters (not bytes) of the secret, in multi-byte text.
        let text = "é wörd-秘密-tö é";
        let found = m.locate(text);
        assert_eq!(found.count(0).partial, 1);
        assert_eq!(
            replace_secret_spans(text, found.spans()).unwrap(),
            "é [SECRET] é"
        );
        // Seven characters is not enough.
        assert!(m.locate("örd-秘密-").is_empty());
        let exact = format!("「{secret}」");
        let found = m.locate(&exact);
        assert_eq!(
            found.count(0),
            SecretCount {
                exact: 1,
                partial: 0
            }
        );
        assert_eq!(
            replace_secret_spans(&exact, found.spans()).unwrap(),
            "「[SECRET]」"
        );
    }

    #[test]
    fn matcher_short_secrets_match_exactly_and_limits_apply() {
        use SecretHitKind::*;
        // A secret shorter than the window matches exactly, inside URL
        // tokens only, whatever its provenance.
        for kind in [Detector, UrlPart] {
            let m = SecretMatcher::new(&[("u:pw", kind)]).unwrap();
            assert!(m.locate("a u:pw b u:pw").is_empty());
            let urls = "https://u:pw@h.example and ftp://u:pw@x.example";
            assert_eq!(m.locate(urls).count(0).exact, 2);
            assert!(m.locate("https://u:p@h.example").is_empty());
        }
        // More than MAX_MATCHER_SECRETS secrets, an empty secret or a card
        // without digits: refused.
        let many: Vec<String> = (0..=MAX_MATCHER_SECRETS)
            .map(|i| format!("secret-{i:08}"))
            .collect();
        let patterns: Vec<(&str, SecretHitKind)> =
            many.iter().map(|s| (s.as_str(), Detector)).collect();
        assert!(SecretMatcher::new(&patterns[..MAX_MATCHER_SECRETS]).is_some());
        assert!(SecretMatcher::new(&patterns).is_none());
        assert!(SecretMatcher::new(&[("", Detector)]).is_none());
        assert!(SecretMatcher::new(&[("no digits", Card)]).is_none());
        // Several secrets: counts per secret, spans merged.
        let m = SecretMatcher::new(&[(KEY, Detector), ("4111 1111 1111 1111", Card)]).unwrap();
        let text = format!("{KEY} card 4111 1111 1111 1111 x 1111 1111 9");
        let found = m.locate(&text);
        assert_eq!(found.count(0).exact, 1);
        assert_eq!(
            found.count(1),
            SecretCount {
                exact: 1,
                partial: 1
            }
        );
        assert_eq!(found.count(7), SecretCount::default());
        let debug = format!("{found:?} {m:?}");
        assert!(!debug.contains("AKIA") && !debug.contains("1111"));
    }

    #[test]
    fn card_copies_match_on_digits_across_any_whitespace_or_dash() {
        let m = SecretMatcher::new(&[("4111 1111 1111 1111", SecretHitKind::Card)]).unwrap();
        for (text, expected) in [
            (
                "card 4111\u{a0}1111\u{a0}1111\u{a0}1111 ok",
                "card [SECRET] ok",
            ),
            ("card 4111-1111-1111-1111 ok", "card [SECRET] ok"),
            ("card 4111111111111111 ok", "card [SECRET] ok"),
            ("4111 1111\n1111  1111.", "[SECRET]."),
            ("4111\u{2009}1111 - 1111\t1111", "[SECRET]"),
            // Unicode hyphens/dashes and the minus sign between digits.
            (
                "card 4111\u{2010}1111\u{2011}1111\u{2012}1111 ok",
                "card [SECRET] ok",
            ),
            ("4111\u{2013}1111\u{2014}1111\u{2015}1111", "[SECRET]"),
            ("4111\u{2212}1111 \u{2212} 1111\u{2212}1111", "[SECRET]"),
        ] {
            let found = m.locate(text);
            assert_eq!(found.count(0).exact, 1, "{text:?}");
            assert_eq!(
                replace_secret_spans(text, found.spans()).unwrap(),
                expected,
                "{text:?}"
            );
        }
        // Other punctuation ends a digit run; other digits do not match.
        assert_eq!(m.locate("4111.1111.1111.1111").count(0).exact, 0);
        assert_eq!(
            m.locate("4111\u{2016}1111\u{2016}1111\u{2016}1111")
                .count(0)
                .exact,
            0
        );
        assert_eq!(m.locate("4111 1111 1111 1112").count(0).exact, 0);
        // Two copies in one run are two locations.
        assert_eq!(
            m.locate("4111111111111111 4111-1111-1111-1111")
                .count(0)
                .exact,
            2
        );
    }

    #[test]
    fn url_components_match_exactly_inside_url_tokens_only() {
        use SecretHitKind::*;
        for query in [
            "auth=Zq81xLmP0wT6",
            "sid=abc",
            "code=4/0AbCdEfGh&state=xyz123",
        ] {
            let m = SecretMatcher::new(&[(query, UrlPart)]).unwrap();
            // Exact copies in another URL token (with or without a scheme).
            let urls = format!("https://e.com/cb?{query} and e.com/x?{query}");
            assert_eq!(m.locate(&urls).count(0).exact, 2, "{query}");
            assert_eq!(m.locate(&urls).count(0).partial, 0, "{query}");
            // Prose copies and fragments in ordinary words are no location.
            let prose = format!("the value {query} was in a note; state=xyz123 too");
            assert!(m.locate(&prose).is_empty(), "{query}");
            // The distinctive parameter value alone (a UrlValue hit) is the
            // one URL part that is also located in prose.
            if let Some(value) = query.strip_prefix("auth=") {
                let by_value = SecretMatcher::new(&[(value, UrlValue)]).unwrap();
                assert_eq!(
                    by_value
                        .locate(&format!("the value {value} was"))
                        .count(0)
                        .exact,
                    1
                );
            }
            // A fragment inside another URL is not a location either.
            assert!(
                m.locate("https://e.com/?state=xyz123").is_empty(),
                "{query}"
            );
        }
        // An OAuth authorisation URL shares parameter text with a sensitive
        // callback query but is no copy of it.
        let callback = "code=4/0AbCdEfGh&state=xyz123";
        let m = SecretMatcher::new(&[(callback, UrlPart)]).unwrap();
        let authorize = "https://login.example.com/authorize?client_id=app&redirect_uri=https://app.example.com/cb&response_type=code&state=xyz123";
        assert!(m.locate(authorize).is_empty());
        // A detector secret still matches partially anywhere.
        let m = SecretMatcher::new(&[(KEY, Detector)]).unwrap();
        assert_eq!(m.locate("prose AKIA12345678 here").count(0).partial, 1);
    }

    #[test]
    fn url_values_match_exactly_anywhere_bounded_by_non_token_characters() {
        let value = "Zq81xLmP0wT6AbCd";
        let m = SecretMatcher::new(&[(value, SecretHitKind::UrlValue)]).unwrap();
        for text in [
            "Your code: Zq81xLmP0wT6AbCd",
            "Zq81xLmP0wT6AbCd",
            "(Zq81xLmP0wT6AbCd).",
            "https://e.com/reset?token=Zq81xLmP0wT6AbCd&x=1",
        ] {
            let found = m.locate(text);
            assert_eq!(found.count(0).exact, 1, "{text}");
            assert_eq!(found.count(0).partial, 0, "{text}");
            assert!(replace_secret_spans(text, found.spans())
                .unwrap()
                .contains("[SECRET]"));
        }
        // Inside a longer alphanumeric/`_`/`-` run, or as a fragment: no copy.
        for text in [
            "xZq81xLmP0wT6AbCd",
            "Zq81xLmP0wT6AbCdx",
            "a_Zq81xLmP0wT6AbCd",
            "Zq81xLmP0wT6AbCd-1",
            "Zq81xLmP0wT6Ab and 0wT6AbCd",
        ] {
            assert!(m.locate(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn secret_labels_are_masked_for_the_self_check_only() {
        let redacted = "key [SECRET] and https://e.com/?[SECRET]";
        let masked = mask_secret_labels(redacted);
        let mask = "\u{1}".repeat(SECRET_LABEL.len());
        assert_eq!(masked, format!("key {mask} and https://e.com/?{mask}"));
        assert_eq!(masked.len(), redacted.len());
        assert!(!masked.to_lowercase().contains("secret"));
        assert!(matches!(
            mask_secret_labels("plain"),
            Cow::Borrowed("plain")
        ));
        let location = OcrHitLocation {
            rects: vec![PixelRect {
                x: 11,
                y: 22,
                width: 33,
                height: 44,
            }],
            words: vec![5, 6],
        };
        assert_eq!(
            format!("{location:?}"),
            "OcrHitLocation { rects: 1, words: 2 }"
        );
    }

    #[test]
    fn coverage_thresholds_at_three_and_four_locations() {
        let three = SecretCopies {
            ocr: 3,
            accessibility: 3,
            metadata: 3,
            address_bar_duplicates: 0,
        };
        assert_eq!(check_secret_coverage(&[three], false), Ok(()));
        for four in [
            SecretCopies { ocr: 4, ..three },
            SecretCopies {
                accessibility: 4,
                ..three
            },
            SecretCopies {
                metadata: 4,
                ..three
            },
        ] {
            assert_eq!(
                check_secret_coverage(&[three, four], false),
                Err(CoverageError::TooManyLocations)
            );
        }
        assert_eq!(MAX_SECRET_LOCATIONS, 3);
    }

    #[test]
    fn coverage_requires_ocr_copies_only_when_pixels_persist() {
        let uncovered = [
            SecretCopies {
                ocr: 1,
                accessibility: 2,
                ..SecretCopies::default()
            },
            SecretCopies {
                metadata: 1,
                ..SecretCopies::default()
            },
            SecretCopies {
                accessibility: 1,
                ..SecretCopies::default()
            },
            // Body + window-title/document-path copies are summed against
            // the OCR copies.
            SecretCopies {
                ocr: 2,
                accessibility: 2,
                metadata: 1,
                address_bar_duplicates: 0,
            },
            // OCR 1 (the address bar), accessibility 1 (a body copy), browser
            // URL 1: the body copy has no OCR location of its own.
            SecretCopies {
                ocr: 1,
                accessibility: 1,
                metadata: 1,
                address_bar_duplicates: 0,
            },
        ];
        for copies in uncovered {
            assert_eq!(
                check_secret_coverage(&[copies], true),
                Err(CoverageError::UncoveredCopies)
            );
            assert_eq!(check_secret_coverage(&[copies], false), Ok(()));
        }
        let covered = SecretCopies {
            ocr: 3,
            accessibility: 2,
            metadata: 1,
            address_bar_duplicates: 0,
        };
        assert_eq!(check_secret_coverage(&[covered], true), Ok(()));
        // The address-bar node duplicates the browser URL copy: one OCR copy
        // covers both; a body copy needs its own.
        let address_bar = SecretCopies {
            ocr: 1,
            accessibility: 1,
            metadata: 1,
            address_bar_duplicates: 1,
        };
        assert_eq!(check_secret_coverage(&[address_bar], true), Ok(()));
        let address_bar_and_body = SecretCopies {
            ocr: 2,
            accessibility: 2,
            metadata: 1,
            address_bar_duplicates: 1,
        };
        assert_eq!(check_secret_coverage(&[address_bar_and_body], true), Ok(()));
        // A URL-only secret that OCR did not locate stays uncovered.
        let url_unlocated = SecretCopies {
            metadata: 1,
            ..SecretCopies::default()
        };
        assert_eq!(
            check_secret_coverage(&[url_unlocated], true),
            Err(CoverageError::UncoveredCopies)
        );
        assert_eq!(check_secret_coverage(&[], true), Ok(()));
    }

    #[test]
    fn area_threshold_uses_the_union_at_fifty_percent() {
        let rect = |x, y, width, height| PixelRect {
            x,
            y,
            width,
            height,
        };
        // Exactly 50%: allowed; one more pixel row: withheld.
        assert!(!redacted_area_exceeds_limit(
            &[rect(0, 0, 100, 50)],
            100,
            100
        ));
        assert!(redacted_area_exceeds_limit(
            &[rect(0, 0, 100, 51)],
            100,
            100
        ));
        // Overlapping rectangles count once (union 100x50).
        let overlapping = [rect(0, 0, 100, 30), rect(0, 20, 100, 30)];
        assert_eq!(union_area(&overlapping, 100, 100), 5000);
        assert!(!redacted_area_exceeds_limit(&overlapping, 100, 100));
        // Disjoint rectangles add up.
        let disjoint = [rect(0, 0, 100, 30), rect(0, 60, 100, 21)];
        assert_eq!(union_area(&disjoint, 100, 100), 5100);
        assert!(redacted_area_exceeds_limit(&disjoint, 100, 100));
        // Clamped to the image; none, empty image, too many rectangles.
        assert_eq!(union_area(&[rect(50, 50, 500, 500)], 100, 100), 2500);
        assert!(!redacted_area_exceeds_limit(&[], 100, 100));
        assert!(redacted_area_exceeds_limit(&[rect(0, 0, 1, 1)], 0, 100));
        let many = vec![rect(0, 0, 1, 1); MAX_AREA_RECTS + 1];
        assert!(redacted_area_exceeds_limit(&many, 100, 100));
        assert_eq!(MAX_REDACTED_AREA_PERCENT, 50);
    }

    // --- OCR line location ---

    /// Two lines: "visible <key> here" / "next line". Binary fractions keep
    /// the pixel arithmetic exact on a 1024 px image.
    fn two_lines() -> (String, Vec<OcrWord<'static>>) {
        let words = vec![
            word("visible", 0.125, 0.125, 0.0625, 0.0625),
            word("AKIA1234567890ABCDEF", 0.25, 0.125, 0.25, 0.0625),
            // Slightly shorter word on the same baseline.
            word("here", 0.5625, 0.140625, 0.0625, 0.046875),
            word("next", 0.125, 0.25, 0.0625, 0.0625),
            word("line", 0.25, 0.25, 0.0625, 0.0625),
        ];
        let text = words.iter().map(|w| w.text).collect::<Vec<_>>().join(" ");
        (text, words)
    }

    #[test]
    fn hit_maps_to_padded_rect_of_its_whole_line() {
        let (text, words) = two_lines();
        let span = span_of(&text, "AKIA1234567890ABCDEF");
        let located = locate_ocr_hits(&text, &words, &[span], 1024, 1024).unwrap();
        // The whole first line (3 words), not the second.
        assert_eq!(located.words, vec![0, 1, 2]);
        // Union x 128..640, y 128..192 (64 px tall) padded by 16 px.
        assert_eq!(
            located.rects,
            vec![PixelRect {
                x: 112,
                y: 112,
                width: 544,
                height: 96,
            }]
        );
    }

    #[test]
    fn partial_word_and_multi_line_hits_cover_every_touched_line() {
        let (text, words) = two_lines();
        // Hit spanning the end of line one and the start of line two.
        let start = text.find("here").unwrap() + 2;
        let end = text.find("next").unwrap() + 2;
        let located = locate_ocr_hits(&text, &words, &[start..end], 1000, 1000).unwrap();
        assert_eq!(located.words, vec![0, 1, 2, 3, 4]);
        assert_eq!(located.rects.len(), 2);
        // Two hits on the same line produce one region.
        let a = span_of(&text, "visible");
        let b = span_of(&text, "AKIA1234567890ABCDEF");
        let located = locate_ocr_hits(&text, &words, &[a, b], 1000, 1000).unwrap();
        assert_eq!(located.rects.len(), 1);
    }

    #[test]
    fn padding_has_a_four_pixel_minimum_and_is_clamped_to_the_image() {
        let words = [word("AKIA1234567890ABCDEF", 0.0, 0.0, 0.5, 0.0078125)];
        let text = words[0].text;
        let located = locate_ocr_hits(text, &words, &[0..text.len()], 200, 128).unwrap();
        // 1 px tall line: padding is the 4 px minimum, clamped at 0.
        assert_eq!(
            located.rects,
            vec![PixelRect {
                x: 0,
                y: 0,
                width: 104,
                height: 5,
            }]
        );
        // A box running past the right/bottom edge is clamped.
        let words = [word("AKIA1234567890ABCDEF", 0.9, 0.9, 0.5, 0.5)];
        let located = locate_ocr_hits(text, &words, &[0..text.len()], 100, 100).unwrap();
        let rect = located.rects[0];
        assert_eq!(rect.x + rect.width, 100);
        assert_eq!(rect.y + rect.height, 100);
    }

    #[test]
    fn unmappable_hits_return_none() {
        let (text, words) = two_lines();
        let span = span_of(&text, "AKIA1234567890ABCDEF");
        // Text that is not exactly the space-joined words.
        let other = text.replace(' ', "\n");
        assert!(locate_ocr_hits(&other, &words, &[span.clone()], 1000, 1000).is_none());
        let extra = format!("{text} trailing");
        assert!(locate_ocr_hits(&extra, &words, &[span.clone()], 1000, 1000).is_none());
        // No words, empty image, out-of-range or empty span.
        assert!(locate_ocr_hits(&text, &[], &[span.clone()], 1000, 1000).is_none());
        assert!(locate_ocr_hits(&text, &words, &[span.clone()], 0, 1000).is_none());
        assert!(locate_ocr_hits(&text, &words, &[0..text.len() + 1], 1000, 1000).is_none());
        assert!(locate_ocr_hits(&text, &words, &[3..3], 1000, 1000).is_none());
        // Invalid boxes.
        for bad in [f64::NAN, f64::INFINITY] {
            let mut broken = words.clone();
            broken[4].left = bad;
            assert!(locate_ocr_hits(&text, &broken, &[span.clone()], 1000, 1000).is_none());
        }
        let mut zero = words.clone();
        zero[0].height = 0.0;
        assert!(locate_ocr_hits(&text, &zero, &[span.clone()], 1000, 1000).is_none());
        // A line lying entirely outside the image.
        let outside = [word("AKIA1234567890ABCDEF", 1.5, 1.5, 0.1, 0.1)];
        let t = outside[0].text;
        assert!(locate_ocr_hits(t, &outside, &[0..t.len()], 100, 100).is_none());
        // No spans: nothing to locate, nothing to redact.
        assert_eq!(
            locate_ocr_hits("anything", &[], &[], 0, 0),
            Some(OcrHitLocation::default())
        );
    }

    #[test]
    fn same_height_columns_merge_into_one_wider_region() {
        // Column one ends at x=0.40; column two starts at x=0.60 at the same
        // height. Without a line index they form one region (over-covers).
        let words = vec![
            word("left", 0.10, 0.10, 0.10, 0.05),
            word("AKIA1234567890ABCDEF", 0.22, 0.10, 0.18, 0.05),
            word("right", 0.60, 0.10, 0.10, 0.05),
            // Next line starts back at the left margin.
            word("below", 0.10, 0.20, 0.10, 0.05),
        ];
        let text = words.iter().map(|w| w.text).collect::<Vec<_>>().join(" ");
        let span = span_of(&text, "AKIA1234567890ABCDEF");
        let located = locate_ocr_hits(&text, &words, &[span], 1000, 1000).unwrap();
        assert_eq!(located.words, vec![0, 1, 2]);
        assert_eq!(located.rects.len(), 1);
        assert!(located.rects[0].x + located.rects[0].width >= 700);
    }
}
