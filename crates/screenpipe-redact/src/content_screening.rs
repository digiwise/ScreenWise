//! Pure, deterministic content screening for in-memory OCR/accessibility text.
//!
//! These checks are deliberately conservative substring/regex heuristics, not
//! a privacy guarantee. They can over-block (a benign document that merely
//! mentions a keyword, or a window title that contains a generic UI word like
//! "vault") and they can under-block (a secret shaped differently than any
//! regex here). Their purpose is to reduce the chance that an obviously
//! sensitive frame — a blocked domain, an open password-manager vault, a bare
//! credit-card number, an API key — gets persisted to disk; a frame that
//! passes screening is not thereby certified private or safe.
//!
//! Secret detection reuses the stored-text redactor's detectors
//! ([`crate::adapters::regex::redact_one`]) but blocks a whole frame only on
//! high-risk categories (secrets, payment cards, credential URLs) and adds
//! precision filters, plus URLs whose query or fragment carries a sensitive
//! parameter name ([`SENSITIVE_URL_PARAM_NAMES`]); see `secret_hits_in`. URL
//! tokens and their components come from
//! `screenpipe_capture_policy::url_filter`. Blocked-domain, DRM-domain and
//! password-manager brand matching use the pure matchers and lists of
//! `screenpipe-capture-policy`, shared with capture admission.
//!
//! Redaction tier (owner decision 2026-10-15): a frame whose only hits are
//! redactable secrets may be redacted in memory instead of withheld; see
//! [`screen_text_plan`], [`screening_tier`] and [`crate::content_redaction`].
//!
//! This module does no logging, reads no configuration files, and the
//! [`ContentScreeningVerdict`] it returns never carries matched text, URLs,
//! byte positions, or anything else that could itself leak the content it
//! was screening. Only [`ContentScreeningPlan`] carries hit positions, for
//! in-process redaction; its `Debug` output prints counts only. All regexes
//! are precompiled once via `once_cell::sync::Lazy` so repeated calls do not
//! pay recompilation cost, and every input is capped to a bounded prefix so
//! screening time stays linear in a fixed worst case.

use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use once_cell::sync::Lazy;
use regex::Regex;

use screenpipe_capture_policy::{drm, password_managers, url_filter};

use crate::adapters::regex::redact_one;
use crate::content_redaction::merge_spans;
use crate::span::SpanLabel;

/// Hard cap on how much of a single input text is scanned.
///
/// 512 KiB is far larger than any single OCR pass or accessibility-tree
/// snapshot produces in practice. The cap exists to bound worst-case regex
/// scan time against an adversarial or pathological input, not to silently
/// drop legitimate content. The cut point is walked back to the nearest
/// UTF-8 char boundary so truncation never panics on multi-byte text.
const MAX_INPUT_BYTES: usize = 512 * 1024;

/// Configuration for [`screen_text`].
pub struct ContentScreeningConfig {
    /// Domain/host patterns (from `ignoredUrls`), matched with
    /// `screenpipe_capture_policy::url_filter::text_mentions_blocked_url`.
    /// Empty never matches.
    pub blocked_url_patterns: Vec<String>,
    /// Apply DRM/protected-content URL matching
    /// (`screenpipe_capture_policy::drm::text_mentions_drm_url`); set only
    /// when `pauseOnDrmContent` is enabled.
    pub protected_domains: bool,
    /// Extra case-insensitive substrings (from `contentScreeningKeywords`).
    /// Blank entries are ignored.
    pub keywords: Vec<String>,
}

/// Why a frame was (or would be) blocked by [`screen_text`].
///
/// Deliberately carries no payload: the verdict must never be able to leak
/// the text, URL, or position that triggered it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContentScreeningReason {
    BlockedDomain,
    ProtectedDomain,
    PasswordManager,
    Secret,
    Keyword,
    Unavailable,
}

impl ContentScreeningReason {
    /// Fixed, allowlisted diagnostic code — safe to log or persist on its own.
    pub fn code(self) -> &'static str {
        match self {
            ContentScreeningReason::BlockedDomain => "content_blocked_domain",
            ContentScreeningReason::ProtectedDomain => "content_protected_domain",
            ContentScreeningReason::PasswordManager => "content_password_manager",
            ContentScreeningReason::Secret => "content_secret",
            ContentScreeningReason::Keyword => "content_keyword",
            ContentScreeningReason::Unavailable => "content_screening_unavailable",
        }
    }
}

/// Result of screening one or more texts. Never contains matched text, URLs,
/// or positions — only the fixed set of reasons that fired.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentScreeningVerdict {
    pub reasons: Vec<ContentScreeningReason>,
}

impl ContentScreeningVerdict {
    /// True if any reason fired; the frame must not be persisted as-is.
    pub fn blocked(&self) -> bool {
        !self.reasons.is_empty()
    }

    /// Verdict for "screening could not run" (e.g. OCR failed). Treated as
    /// blocked so a failure to screen never fails open.
    pub fn unavailable() -> Self {
        Self {
            reasons: vec![ContentScreeningReason::Unavailable],
        }
    }

    /// Verdict for a frame withheld only because of redactable secrets that
    /// could not be (or must not be) redacted in memory.
    pub fn secret() -> Self {
        Self {
            reasons: vec![ContentScreeningReason::Secret],
        }
    }

    /// Fixed, allowlisted codes for the reasons that fired, in sorted order.
    pub fn codes(&self) -> Vec<&'static str> {
        self.reasons.iter().map(|r| r.code()).collect()
    }
}

/// Screen one or more texts (OCR output, accessibility tree text, ...)
/// against the configured rules. See the module docs for what this can and
/// cannot guarantee.
pub fn screen_text(texts: &[&str], config: &ContentScreeningConfig) -> ContentScreeningVerdict {
    screen_text_plan(texts, config).verdict()
}

// --- Redaction-tier whole-frame thresholds (stage 1) ---
//
// Every threshold of the redaction tier lives here. Exceeding any of them
// withholds the whole frame with `content_secret` instead of redacting it.

/// Maximum number of distinct redactable hits a frame may carry and still be
/// redacted in memory; more than this withholds the whole frame.
pub const MAX_REDACTION_HITS: usize = 3;

/// Maximum number of plausible locations of one secret in any one source
/// (OCR text, accessibility text/tree, window metadata): exact copies plus
/// fragments of at least [`MIN_PARTIAL_MATCH_CHARS`] characters. More than
/// this withholds the whole frame.
pub const MAX_SECRET_LOCATIONS: usize = 3;

/// Maximum share of the image, in percent, that the union of the redaction
/// rectangles may cover; more than this withholds the whole frame.
pub const MAX_REDACTED_AREA_PERCENT: u64 = 50;

/// A run of at least this many consecutive characters (not bytes) of an
/// already detected secret, case-sensitive, is a plausible location of it.
/// Only detector secrets (not URL components) are matched partially; a
/// secret shorter than this is matched exactly, and only inside URL tokens.
pub const MIN_PARTIAL_MATCH_CHARS: usize = 8;

/// Upper bound on the unmerged hit texts one frame's secret matcher accepts
/// (a URL component plus the detector secrets nested in it can each be a
/// pattern). More withholds the frame; this only bounds matching work.
pub const MAX_MATCHER_SECRETS: usize = 4 * MAX_REDACTION_HITS;

/// Query/fragment parameter names that make a URL a credential carrier in
/// standard mode. Matched case-insensitively against the whole parameter
/// name, without percent-decoding (`token%3D…` is not `token=` in stage 1).
pub const SENSITIVE_URL_PARAM_NAMES: &[&str] = &[
    "token",
    "access_token",
    "refresh_token",
    "id_token",
    "private_token",
    "api_key",
    "apikey",
    "key",
    "secret",
    "client_secret",
    "password",
    "passwd",
    "pwd",
    "session",
    "sessionid",
    "sid",
    "auth",
    "authorization",
    "code",
    "signature",
    "sig",
    "x-amz-signature",
    "x-amz-credential",
    "x-amz-security-token",
];

/// Where a redactable hit came from. It decides how further copies of the
/// hit text are located in other persisted texts (see
/// [`crate::content_redaction::SecretMatcher`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SecretHitKind {
    /// A detector secret ([`redact_one`] `Secret` span): exact copies and
    /// fragments of at least [`MIN_PARTIAL_MATCH_CHARS`] characters.
    Detector,
    /// A payment card: copies are matched on the digit sequence, ignoring
    /// whitespace and `-` between digits.
    Card,
    /// A sensitive URL component (user-info, query, fragment, or the
    /// remainder of a nested credential URL): exact copies inside URL tokens
    /// only.
    UrlPart,
    /// The value of a sensitive URL parameter, or the password of URL
    /// user-info, of at least [`MIN_PARTIAL_MATCH_CHARS`] characters with at
    /// least two character classes (letters, digits, symbols): exact copies
    /// anywhere, bounded by non-token characters (never inside a longer
    /// alphanumeric/`_`/`-` run), never partial.
    UrlValue,
}

/// [`screen_text`] plus, for the redaction tier, where the redactable
/// [`ContentScreeningReason::Secret`] hits sit in each input.
///
/// The byte spans exist only so the caller can redact this frame in memory.
/// They must never be logged, persisted or copied into a verdict, notice,
/// metric or JSON; [`fmt::Debug`] therefore prints counts only.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ContentScreeningPlan {
    /// Reasons other than `Secret`, sorted and deduplicated. Any of these
    /// withholds the frame regardless of redaction.
    blocking: Vec<ContentScreeningReason>,
    /// A `Secret` hit that must never be redacted in place (a PEM
    /// private-key/secret block marker: the key body around it is not
    /// detected), so the whole frame is withheld.
    non_redactable: bool,
    /// Per input text (same order as the input slice): sorted,
    /// non-overlapping (merged) byte spans of `Secret` hits that passed the
    /// screening post-filters, including sensitive URL components.
    secret_spans: Vec<Vec<Range<usize>>>,
    /// Per input text: the unmerged hits behind `secret_spans`, sorted and
    /// deduplicated, with their provenance.
    secret_hits: Vec<Vec<(Range<usize>, SecretHitKind)>>,
    /// Number of distinct sensitive URL texts (credential user-info or
    /// sensitive query/fragment names) across every input. A count only.
    url_hits: usize,
}

impl fmt::Debug for ContentScreeningPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentScreeningPlan")
            .field("blocking", &self.blocking)
            .field("non_redactable", &self.non_redactable)
            .field(
                "secret_hits",
                &self.secret_spans.iter().map(Vec::len).sum::<usize>(),
            )
            .field("url_hits", &self.url_hits)
            .finish()
    }
}

impl ContentScreeningPlan {
    /// Exactly the verdict [`screen_text`] returns for the same inputs.
    pub fn verdict(&self) -> ContentScreeningVerdict {
        let mut reasons = self.blocking.clone();
        if self.has_secret() {
            reasons.push(ContentScreeningReason::Secret);
        }
        reasons.sort();
        ContentScreeningVerdict { reasons }
    }

    /// Non-redactable reasons (everything except `Secret`).
    pub fn blocking(&self) -> &[ContentScreeningReason] {
        &self.blocking
    }

    /// True if any input carries a redactable `Secret` hit.
    pub fn has_secret(&self) -> bool {
        self.secret_spans.iter().any(|spans| !spans.is_empty())
    }

    /// True when a `Secret` hit must withhold the whole frame instead of
    /// being redacted (PEM private-key/secret block markers).
    pub fn non_redactable(&self) -> bool {
        self.non_redactable
    }

    /// Redactable hit spans in input `index` (empty when out of range).
    pub fn secret_spans(&self, index: usize) -> &[Range<usize>] {
        self.secret_spans.get(index).map_or(&[][..], Vec::as_slice)
    }

    /// The unmerged hits behind [`Self::secret_spans`] in input `index`, with
    /// their provenance (empty when out of range).
    pub fn secret_hits(&self, index: usize) -> &[(Range<usize>, SecretHitKind)] {
        self.secret_hits.get(index).map_or(&[][..], Vec::as_slice)
    }

    /// Number of distinct sensitive URL texts found across every input
    /// (diagnostic count).
    pub fn url_hits(&self) -> usize {
        self.url_hits
    }
}

/// [`screen_text`] that also records the redactable `Secret` spans per input.
/// Spans index into the original input (always inside its scanned prefix).
pub fn screen_text_plan(texts: &[&str], config: &ContentScreeningConfig) -> ContentScreeningPlan {
    let mut blocking = Vec::new();
    let mut secret_spans = Vec::with_capacity(texts.len());
    let mut secret_hits = Vec::with_capacity(texts.len());
    let mut non_redactable = false;
    let mut urls: BTreeSet<&str> = BTreeSet::new();

    for raw in texts {
        let text = bound_input(raw);
        if text.is_empty() {
            secret_spans.push(Vec::new());
            secret_hits.push(Vec::new());
            continue;
        }

        if url_filter::text_mentions_blocked_url(text, &config.blocked_url_patterns) {
            blocking.push(ContentScreeningReason::BlockedDomain);
        }

        if config.protected_domains && drm::text_mentions_drm_url(text) {
            blocking.push(ContentScreeningReason::ProtectedDomain);
        }

        // Domain checks above ran on the original hosts; URL components are
        // only ever replaced later, by the redaction tier.
        let found = secret_hits_in(text);
        non_redactable |= found.non_redactable;
        urls.extend(
            found
                .sensitive_urls
                .iter()
                .filter_map(|range| text.get(range.clone())),
        );
        secret_spans.push(merge_spans(found.hits.iter().map(|(r, _)| r.clone())));
        secret_hits.push(found.hits);

        // Case-insensitive checks share one lowercased copy of the (already
        // bounded) text.
        let lower = text.to_lowercase();

        if has_password_manager_pattern(&lower) {
            blocking.push(ContentScreeningReason::PasswordManager);
        }

        if has_keyword(&lower, &config.keywords) {
            blocking.push(ContentScreeningReason::Keyword);
        }
    }

    blocking.sort();
    blocking.dedup();
    ContentScreeningPlan {
        blocking,
        non_redactable,
        secret_spans,
        secret_hits,
        url_hits: urls.len(),
    }
}

/// Per-frame outcome of the screening tiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContentScreeningTier {
    /// No hits: persist unchanged.
    Pass,
    /// Only redactable `Secret` hits: redact in memory, then persist. The
    /// caller must still withhold the frame (as
    /// [`ContentScreeningVerdict::secret`]) when a hit cannot be located or
    /// covered, or when the post-redaction self-check finds anything.
    Redact,
    /// Withhold the whole frame with these fixed reasons.
    Drop(ContentScreeningVerdict),
}

/// Pure tier decision for one frame.
///
/// - any non-`Secret` reason (blocked/protected domain, password manager,
///   keyword, unavailable) → `Drop` with every reason that fired;
/// - no hits → `Pass`;
/// - a non-redactable hit (PEM private-key/secret block marker), a hit on a
///   fail-open frame (fail-open requires a clean pass), or more than
///   [`MAX_REDACTION_HITS`] distinct hits → `Drop(secret)`;
/// - otherwise → `Redact`.
pub fn screening_tier(
    plan: &ContentScreeningPlan,
    distinct_hits: usize,
    fail_open: bool,
) -> ContentScreeningTier {
    if !plan.blocking.is_empty() {
        return ContentScreeningTier::Drop(plan.verdict());
    }
    if !plan.has_secret() {
        return ContentScreeningTier::Pass;
    }
    if plan.non_redactable || fail_open || distinct_hits == 0 || distinct_hits > MAX_REDACTION_HITS
    {
        return ContentScreeningTier::Drop(ContentScreeningVerdict::secret());
    }
    ContentScreeningTier::Redact
}

/// Truncate to the first [`MAX_INPUT_BYTES`] bytes, walked back to a valid
/// UTF-8 char boundary.
fn bound_input(text: &str) -> &str {
    if text.len() <= MAX_INPUT_BYTES {
        return text;
    }
    let mut end = MAX_INPUT_BYTES;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

// Password-manager brand names and UI phrases. These lists are deliberately
// conservative: generic words like "fill" or "vault" are included because a
// password-manager UI reliably uses them, at the cost of over-blocking a
// frame that happens to combine an unrelated brand mention with one of these
// words within the same 12-word window. Over-blocking a frame is considered
// safer than persisting an open vault. Matching is whole-word only (see
// `phrase_regex`): "fill" never matches inside "filler"/"fulfill"/"refilled",
// and "keeper" never matches inside "bookkeeper"/"goalkeeper".
const PASSWORD_MANAGER_UI_PHRASES: &[&str] = &[
    "vault",
    "autofill",
    "auto-fill",
    "fill",
    "master password",
    "unlock",
    "generate password",
    "password generator",
    "passkey",
    "new login",
    "saved logins",
    "saved passwords",
    "add item",
    "totp",
    // Ordinary sign-in forms use it too: it blocks only near a brand name.
    "show password",
];

/// Phrases strong enough to block on their own, without needing a brand name
/// nearby.
const PASSWORD_MANAGER_STRONG_PHRASES: &[&str] = &[
    "master password",
    "unlock your vault",
    "unlock vault",
    "password generator",
    "generate password",
    "use suggested password",
    "manage passwords",
    "saved passwords",
    "copy password",
];

/// Build a whole-word regex for a brand/UI phrase: regex-escaped, with any
/// literal space turned into flexible whitespace (`\s+`) so a multi-word
/// phrase still matches across a line break or extra spacing, and wrapped in
/// `\b...\b` so e.g. "fill" never matches inside "filler"/"fulfill"/
/// "refilled" and "keeper" never matches inside "bookkeeper"/"goalkeeper".
/// Hyphenated phrases like "auto-fill" still match literally, since `-` is
/// not a word character and so does not interfere with the boundary.
fn phrase_regex(phrase: &str) -> Regex {
    let escaped = regex::escape(phrase).replace(' ', r"\s+");
    Regex::new(&format!(r"\b{}\b", escaped)).expect("valid regex")
}

static BRAND_REGEXES: Lazy<Vec<Regex>> = Lazy::new(|| {
    password_managers::screen_brand_names()
        .iter()
        .map(|p| phrase_regex(p))
        .collect()
});
static UI_PHRASE_REGEXES: Lazy<Vec<Regex>> = Lazy::new(|| {
    PASSWORD_MANAGER_UI_PHRASES
        .iter()
        .map(|p| phrase_regex(p))
        .collect()
});
static STRONG_PHRASE_REGEXES: Lazy<Vec<Regex>> = Lazy::new(|| {
    PASSWORD_MANAGER_STRONG_PHRASES
        .iter()
        .map(|p| phrase_regex(p))
        .collect()
});

fn has_password_manager_pattern(lower_text: &str) -> bool {
    if STRONG_PHRASE_REGEXES
        .iter()
        .any(|re| re.is_match(lower_text))
    {
        return true;
    }

    let starts = word_starts(lower_text);
    if starts.is_empty() {
        return false;
    }

    let mut brand_words: Vec<usize> = Vec::new();
    for re in BRAND_REGEXES.iter() {
        for m in re.find_iter(lower_text) {
            brand_words.push(word_index(&starts, m.start()));
        }
    }
    if brand_words.is_empty() {
        return false;
    }

    let mut phrase_words: Vec<usize> = Vec::new();
    for re in UI_PHRASE_REGEXES.iter() {
        for m in re.find_iter(lower_text) {
            phrase_words.push(word_index(&starts, m.start()));
        }
    }
    if phrase_words.is_empty() {
        return false;
    }

    brand_words.iter().any(|&b| {
        phrase_words
            .iter()
            .any(|&p| (b as isize - p as isize).abs() <= 12)
    })
}

/// Byte offsets of the start of every whitespace-delimited "word" in `text`.
fn word_starts(text: &str) -> Vec<usize> {
    static WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"\S+").expect("valid regex"));
    WORD.find_iter(text).map(|m| m.start()).collect()
}

/// Approximate 0-based word index of the word containing (or starting at)
/// `byte_pos`, given the sorted word-start offsets for the same text.
fn word_index(starts: &[usize], byte_pos: usize) -> usize {
    let count = starts.partition_point(|&s| s <= byte_pos);
    count.saturating_sub(1)
}

/// Secret screening on top of the shared redactor: [`redact_one`] finds the
/// candidate spans (provider token shapes, PEM private-key markers, JWTs,
/// credential URLs, Luhn/brand-checked card numbers) and this module applies
/// stricter, frame-level precision filters, because a block withholds the
/// whole frame while a redaction only masks a span.
///
/// Hits on:
/// - [`SpanLabel::Secret`] spans that pass [`secret_span_ok`]
///   ([`SecretHitKind::Detector`]); a PEM block marker also makes the frame
///   non-redactable;
/// - `credit_card` identifier spans that pass [`card_span`]
///   ([`SecretHitKind::Card`]);
/// - credential URLs: their user-info ([`credential_userinfo`]) and any
///   secret the redactor's single span hid after it;
/// - sensitive URLs ([`sensitive_url_spans`]): credential user-info, the
///   whole query and any sensitive fragment, never protocol, host or path
///   ([`SecretHitKind::UrlPart`]).
///
/// Email, phone, date, other identifier subtypes, person and address spans
/// and plain IPv4 addresses never hit.
///
/// The redactor reports a credential URL (`scheme://user:pass@…`) as one
/// span that hides any secret after the user-info. Its user-info is always a
/// hit, and the part after the user-info is screened again so a secret in
/// its host, path or fragment is still a hit (a nested credential URL there
/// makes that whole part a hit).
fn secret_hits_in(text: &str) -> TextHits {
    let detected = detector_hits(text);
    let mut out = TextHits {
        hits: detected.spans,
        sensitive_urls: Vec::new(),
        non_redactable: detected.non_redactable,
    };
    for url in detected.credential_urls {
        out.hits
            .push((url.userinfo.clone(), SecretHitKind::UrlPart));
        out.hits.extend(userinfo_password_hit(text, &url.userinfo));
        out.sensitive_urls.push(url.span.clone());
        // Skip the `@` that ends the user-info.
        let rest = url.userinfo.end + 1..url.span.end;
        if rest.start < rest.end {
            let nested = detector_hits(&text[rest.clone()]);
            out.non_redactable |= nested.non_redactable;
            out.hits.extend(
                nested
                    .spans
                    .into_iter()
                    .map(|(r, kind)| (rest.start + r.start..rest.start + r.end, kind)),
            );
            if !nested.credential_urls.is_empty() {
                out.hits.push((rest, SecretHitKind::UrlPart));
            }
        }
    }
    let (url_spans, url_tokens) = sensitive_url_spans(text);
    out.hits.extend(url_spans);
    out.sensitive_urls.extend(url_tokens);
    out.hits.sort_by_key(|(r, kind)| (r.start, r.end, *kind));
    out.hits.dedup();
    out
}

/// Every hit of one input text (see [`secret_hits_in`]).
#[derive(Default)]
struct TextHits {
    /// Unmerged, sorted, deduplicated hit spans and their provenance.
    hits: Vec<(Range<usize>, SecretHitKind)>,
    /// Sensitive URL tokens and credential-URL spans, for the distinct count.
    sensitive_urls: Vec<Range<usize>>,
    /// A PEM private-key/secret block marker was detected.
    non_redactable: bool,
}

/// A credential URL span of [`redact_one`] and its user-info (both
/// absolute byte ranges of the scanned text).
struct CredentialUrl {
    span: Range<usize>,
    userinfo: Range<usize>,
}

/// The filtered detector hits of [`redact_one`] in one text.
#[derive(Default)]
struct DetectorHits {
    /// Secret and card spans to redact as they are.
    spans: Vec<(Range<usize>, SecretHitKind)>,
    /// Credential URL spans, to be reduced to their components.
    credential_urls: Vec<CredentialUrl>,
    /// A PEM private-key/secret block marker was detected.
    non_redactable: bool,
}

/// [`redact_one`] spans that pass the screening precision filters.
fn detector_hits(text: &str) -> DetectorHits {
    let mut out = DetectorHits::default();
    for span in redact_one(text).spans {
        let (start, end) = (span.start, span.end);
        match span.label {
            SpanLabel::Secret if secret_span_ok(text, start, end) => {
                // The redactor's PEM patterns match only the BEGIN marker,
                // never the key body after it: such a frame is never
                // redacted in place.
                out.non_redactable |= text[start..end].starts_with("-----BEGIN");
                out.spans.push((start..end, SecretHitKind::Detector));
            }
            SpanLabel::Url => {
                if let Some(userinfo) = credential_userinfo(&text[start..end]) {
                    out.credential_urls.push(CredentialUrl {
                        span: start..end,
                        userinfo: start + userinfo.start..start + userinfo.end,
                    });
                }
            }
            SpanLabel::Id if span.subtype.as_deref() == Some("credit_card") => {
                if let Some(card) = card_span(text, start, end) {
                    out.spans.push((card, SecretHitKind::Card));
                }
            }
            _ => {}
        }
    }
    // Screening-side PEM markers the redactor's exact-dash patterns miss
    // (END-only, OCR-garbled dashes). Redacting stored text is unchanged: the
    // marker is only a non-redactable hit, so the whole frame is withheld.
    for found in PEM_MARKER.find_iter(text) {
        out.non_redactable = true;
        out.spans
            .push((found.start()..found.end(), SecretHitKind::Detector));
    }
    out
}

/// PEM private-key/secret block markers in any case of OCR damage: an
/// `-----END` footer, or an uppercase `BEGIN`/`END` (up to six uppercase
/// words) `PRIVATE KEY` without requiring the exact dashes. Prose such as
/// "private key" alone never matches.
static PEM_MARKER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"-----END[A-Z\s]*PRIVATE\s+KEY|-----END[A-Z\s]*(?:SECRET|ENCRYPTED[A-Z\s]*KEY)|\b(?:BEGIN|END)\s+(?:[A-Z]+\s+){0,6}PRIVATE\s+KEY\b",
    )
    .expect("valid regex")
});

/// Letters, digits and symbols are the three character classes.
fn character_classes(value: &str) -> usize {
    let (mut letters, mut digits, mut symbols) = (false, false, false);
    for c in value.chars() {
        if c.is_alphabetic() {
            letters = true;
        } else if c.is_numeric() {
            digits = true;
        } else if !c.is_whitespace() {
            symbols = true;
        }
    }
    usize::from(letters) + usize::from(digits) + usize::from(symbols)
}

/// A URL secret value that is distinctive enough to be matched exactly
/// anywhere: at least [`MIN_PARTIAL_MATCH_CHARS`] characters and two classes.
fn url_value_hit(text: &str, range: Range<usize>) -> Option<(Range<usize>, SecretHitKind)> {
    let value = text.get(range.clone())?;
    (value.chars().count() >= MIN_PARTIAL_MATCH_CHARS && character_classes(value) >= 2)
        .then_some((range, SecretHitKind::UrlValue))
}

/// The password part (after the first `:`) of a user-info range.
fn userinfo_password_hit(
    text: &str,
    userinfo: &Range<usize>,
) -> Option<(Range<usize>, SecretHitKind)> {
    let info = text.get(userinfo.clone())?;
    let colon = info.find(':')?;
    url_value_hit(text, userinfo.start + colon + 1..userinfo.end)
}

/// Value ranges (relative to `text`) of every sensitive parameter: a piece
/// split at `name_separators` whose name before the first `=` is sensitive;
/// the value runs to the next `value_separators` character.
fn sensitive_param_values(
    text: &str,
    name_separators: &[char],
    value_separators: &[char],
) -> Vec<Range<usize>> {
    let mut values = Vec::new();
    let mut start = 0;
    let mut ends: Vec<usize> = text
        .match_indices(name_separators)
        .map(|(index, _)| index)
        .collect();
    ends.push(text.len());
    for end in ends {
        let piece = &text[start..end];
        if let Some((name, _)) = piece.split_once('=') {
            if is_sensitive_param_name(name) {
                let value_start = start + name.len() + 1;
                let value_end = text[value_start..]
                    .find(value_separators)
                    .map_or(text.len(), |index| value_start + index);
                if value_start < value_end {
                    values.push(value_start..value_end);
                }
            }
        }
        start = (end + 1).min(text.len());
    }
    values
}

fn is_sensitive_param_name(name: &str) -> bool {
    SENSITIVE_URL_PARAM_NAMES
        .iter()
        .any(|sensitive| sensitive.eq_ignore_ascii_case(name))
}

/// A query is sensitive when any `&`/`;`-separated parameter has a
/// sensitive name (the part before `=`, or the whole parameter without one).
fn query_is_sensitive(query: &str) -> bool {
    query
        .split(['&', ';'])
        .any(|param| is_sensitive_param_name(param.split('=').next().unwrap_or(param)))
}

/// A fragment is sensitive when it contains `name=` with a sensitive name at
/// the start or after `&`, `;`, `?`, `/` or `#` (e.g. `#access_token=…`,
/// `#/callback?code=…`).
fn fragment_is_sensitive(fragment: &str) -> bool {
    fragment.split(['&', ';', '?', '/', '#']).any(|piece| {
        piece
            .split_once('=')
            .is_some_and(|(name, _)| is_sensitive_param_name(name))
    })
}

/// Redaction spans of every sensitive URL token in `text`, and the byte
/// ranges of those tokens.
///
/// A URL token (with or without a scheme, see
/// [`url_filter::url_token_spans`]) is sensitive when it has credential
/// user-info (`scheme://user:pass@`, non-empty password), a sensitive query
/// parameter name or a sensitive fragment. For a sensitive URL the spans are
/// its user-info (any, when a scheme is present), its entire query (after
/// `?`) and, when sensitive, its fragment (after `#`). Protocol, host and
/// path are kept; nothing is decoded, so an encoded `%3D` is not `=`.
///
/// Each sensitive parameter value and the user-info password also yield a
/// [`SecretHitKind::UrlValue`] hit when distinctive (see [`url_value_hit`]).
fn sensitive_url_spans(text: &str) -> (Vec<(Range<usize>, SecretHitKind)>, Vec<Range<usize>>) {
    let mut spans = Vec::new();
    let mut tokens = Vec::new();
    for token_range in url_filter::url_token_spans(text) {
        let token = &text[token_range.clone()];
        let parts = url_filter::split_url_token(token);
        let credential = parts.userinfo.as_ref().is_some_and(|r| {
            matches!(token[r.clone()].split_once(':'), Some((_, password)) if !password.is_empty())
        });
        let sensitive_fragment = parts
            .fragment
            .as_ref()
            .is_some_and(|r| fragment_is_sensitive(&token[r.clone()]));
        let sensitive_query = parts
            .query
            .as_ref()
            .is_some_and(|r| query_is_sensitive(&token[r.clone()]));
        if !(credential || sensitive_query || sensitive_fragment) {
            continue;
        }
        let fragment = parts.fragment.filter(|_| sensitive_fragment);
        let absolute =
            |range: &Range<usize>| token_range.start + range.start..token_range.start + range.end;
        if let Some(userinfo) = parts.userinfo.as_ref().filter(|r| r.start < r.end) {
            spans.extend(userinfo_password_hit(text, &absolute(userinfo)));
        }
        for range in [&parts.userinfo, &parts.query, &fragment]
            .into_iter()
            .flatten()
        {
            if range.start < range.end {
                spans.push((absolute(range), SecretHitKind::UrlPart));
            }
        }
        if let Some(query) = parts.query.as_ref().filter(|_| sensitive_query) {
            let base = token_range.start + query.start;
            for value in sensitive_param_values(&token[query.clone()], &['&', ';'], &['&', ';']) {
                spans.extend(url_value_hit(text, base + value.start..base + value.end));
            }
        }
        if let Some(fragment) = fragment.as_ref() {
            let base = token_range.start + fragment.start;
            for value in sensitive_param_values(
                &token[fragment.clone()],
                &['&', ';', '?', '/', '#'],
                &['&', ';', '?', '#'],
            ) {
                spans.extend(url_value_hit(text, base + value.start..base + value.end));
            }
        }
        tokens.push(token_range);
    }
    (spans, tokens)
}

/// True if `c` may be part of an identifier-like token, so a prefix that
/// directly follows it is mid-identifier rather than the start of a token.
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// Precision filters for a [`SpanLabel::Secret`] span at `text[start..end]`.
///
/// - A token starting with an alphanumeric must not directly follow an
///   identifier character (`a-glpat-...`, `my-ghp_...`): the redactor's `\b`
///   treats `-` as a boundary, a frame-level block should not. Block markers
///   such as `-----BEGIN ... PRIVATE KEY-----` are exempt.
/// - Anthropic `sk-ant-` keys need the `sk-ant-api<NN>-` / `sk-ant-admin<NN>-`
///   shape followed by at least 40 characters; otherwise the generic `sk-`
///   rule below applies.
/// - OpenAI `sk-` / `sk-proj-` keys need a body of at least 32 characters
///   containing a digit and a letter, and a hyphenated body needs an
///   uppercase letter (lowercase words joined by hyphens, such as branch
///   names, are not keys).
/// - AWS access key ids (`AKIA` / `ASIA`) are exactly 20 characters with no
///   alphanumeric continuation.
fn secret_span_ok(text: &str, start: usize, end: usize) -> bool {
    let matched = &text[start..end];
    if matched.starts_with(|c: char| c.is_ascii_alphanumeric())
        && text[..start].chars().next_back().is_some_and(is_token_char)
    {
        return false;
    }
    if let Some(rest) = matched.strip_prefix("sk-ant-") {
        return looks_like_anthropic_key(rest) || looks_like_openai_key(matched);
    }
    if matched.starts_with("sk-") {
        return looks_like_openai_key(matched);
    }
    if matched.starts_with("AKIA") || matched.starts_with("ASIA") {
        return matched.len() == 20
            && !text[end..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric());
    }
    true
}

/// Anthropic keys: `api<NN>-` or `admin<NN>-` (after `sk-ant-`) followed by
/// at least 40 characters.
fn looks_like_anthropic_key(rest: &str) -> bool {
    let Some(after) = rest
        .strip_prefix("api")
        .or_else(|| rest.strip_prefix("admin"))
    else {
        return false;
    };
    let b = after.as_bytes();
    b.len() >= 3 + 40 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'-'
}

/// Real OpenAI keys are long random strings: require a body of at least 32
/// characters (after `sk-` / `sk-proj-`) containing both a digit and a
/// letter, and an uppercase letter when the body contains a hyphen, so
/// hyphenated prose, branch names and identifiers do not match.
fn looks_like_openai_key(matched: &str) -> bool {
    let body = matched
        .strip_prefix("sk-proj-")
        .or_else(|| matched.strip_prefix("sk-"))
        .unwrap_or(matched);
    body.len() >= 32
        && body.bytes().any(|b| b.is_ascii_digit())
        && body.bytes().any(|b| b.is_ascii_alphabetic())
        && (!body.contains('-') || body.bytes().any(|b| b.is_ascii_uppercase()))
}

/// Database URL schemes of the redactor's connection-string pattern.
const DB_URL_SCHEMES: &[&str] = &[
    "postgres",
    "postgresql",
    "mysql",
    "mariadb",
    "mongodb",
    "mongodb+srv",
    "redis",
    "rediss",
    "amqp",
    "amqps",
];

/// The user-info (relative byte range) of a credential URL span
/// `scheme://user:password@host…` with a non-empty password, or `None`.
///
/// For a generic scheme the user-info must sit in the authority (before any
/// `/`, `?` or `#`), so a URL with a port and a later `@` in its path is not
/// a credential URL. For a database scheme the redactor's span (which ends
/// at whitespace) is trusted up to its last `@`, because database passwords
/// may contain `/` or `?`; anything up to that `@` is treated as user-info.
fn credential_userinfo(span: &str) -> Option<Range<usize>> {
    let (scheme, rest) = span.split_once("://")?;
    let start = scheme.len() + 3;
    let len = if DB_URL_SCHEMES
        .iter()
        .any(|db| db.eq_ignore_ascii_case(scheme))
    {
        rest.rfind('@')?
    } else {
        let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        rest[..authority_end].rfind('@')?
    };
    matches!(rest[..len].split_once(':'), Some((_, password)) if !password.is_empty())
        .then_some(start..start + len)
}

/// Digit-group layouts of real card numbers: 4-4-4-4 (16), 4-4-4-4-3 (19),
/// Amex 4-6-5 (15) and Diners 4-6-4 (14).
const CARD_LAYOUTS: &[&[usize]] = &[&[4, 4, 4, 4], &[4, 4, 4, 4, 3], &[4, 6, 5], &[4, 6, 4]];

/// True if `sep` followed by a further 4-digit group (exactly four digits)
/// follows `pos`, i.e. the candidate is really part of a longer run of
/// 4-digit groups. An expiry (`12/27`) or a shorter/longer number after the
/// separator does not extend the grouping.
fn continues_after(b: &[u8], pos: usize, sep: u8) -> bool {
    let start = pos + 1;
    pos < b.len()
        && b[pos] == sep
        && start + 4 <= b.len()
        && b[start..start + 4].iter().all(u8::is_ascii_digit)
        && !b.get(start + 4).is_some_and(u8::is_ascii_digit)
}

/// The span of a `credit_card` detector hit at `text[start..end]` when it
/// passes [`card_layout_ok`], trimmed of any trailing separator the
/// redactor's catch-all card pattern swallowed.
fn card_span(text: &str, start: usize, end: usize) -> Option<Range<usize>> {
    let b = text.as_bytes();
    let mut end = end;
    while end > start && matches!(b[end - 1], b' ' | b'-') {
        end -= 1;
    }
    card_layout_ok(b, start, end).then_some(start..end)
}

/// Layout and boundary rules for a trimmed `credit_card` span `b[start..end]`
/// (brand, length and Luhn were already verified by the redactor): 13-19
/// unbroken digits, or a known grouping joined by one consistent single
/// space or dash. A number with an adjoining digit (or digit plus separator)
/// on either side, or followed by a further same-length group, is part of a
/// longer run and is ignored. An expiry, year or lone digit afterwards does
/// not matter.
fn card_layout_ok(b: &[u8], start: usize, end: usize) -> bool {
    let span = &b[start..end];
    if span.is_empty() || !span[0].is_ascii_digit() || !span[span.len() - 1].is_ascii_digit() {
        return false;
    }
    if start >= 1 && b[start - 1].is_ascii_digit() {
        return false;
    }
    if start >= 2 && matches!(b[start - 1], b' ' | b'-') && b[start - 2].is_ascii_digit() {
        return false;
    }
    if b.get(end).is_some_and(u8::is_ascii_digit) {
        return false;
    }

    let separators: Vec<u8> = span
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_digit())
        .collect();
    let Some(&sep) = separators.first() else {
        return (13..=19).contains(&span.len());
    };
    if !matches!(sep, b' ' | b'-') || separators.iter().any(|&c| c != sep) {
        return false;
    }
    let groups: Vec<usize> = span.split(|&c| c == sep).map(<[u8]>::len).collect();
    CARD_LAYOUTS.contains(&groups.as_slice()) && !continues_after(b, end, sep)
}

fn has_keyword(lower_text: &str, keywords: &[String]) -> bool {
    keywords.iter().any(|k| {
        let k = k.trim().to_lowercase();
        !k.is_empty() && lower_text.contains(&k)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::national_id::luhn as luhn_valid;

    fn config(blocked: &[&str], protected: bool, keywords: &[&str]) -> ContentScreeningConfig {
        ContentScreeningConfig {
            blocked_url_patterns: blocked.iter().map(|s| s.to_string()).collect(),
            protected_domains: protected,
            keywords: keywords.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn empty_config() -> ContentScreeningConfig {
        config(&[], false, &[])
    }

    // --- BlockedDomain ---

    #[test]
    fn blocked_domain_positive() {
        let cfg = config(&["chase"], false, &[]);
        let verdict = screen_text(&["please visit chase.com to log in"], &cfg);
        assert!(verdict
            .reasons
            .contains(&ContentScreeningReason::BlockedDomain));
    }

    #[test]
    fn blocked_domain_negative_substring_and_prose() {
        let cfg = config(&["chase"], false, &[]);
        // "purchase.com" must not match the "chase" pattern (domain boundary).
        let verdict = screen_text(&["please visit purchase.com to check out"], &cfg);
        assert!(!verdict.blocked());

        // A bare word "chase" in prose (no dot, no TLD) must never be
        // extracted as a host token in the first place.
        let verdict = screen_text(&["I will chase my dream today"], &cfg);
        assert!(!verdict.blocked());
    }

    // --- ProtectedDomain ---

    #[test]
    fn protected_domain_positive() {
        let cfg = config(&[], true, &[]);
        let verdict = screen_text(&["now streaming at netflix.com/watch"], &cfg);
        assert!(verdict
            .reasons
            .contains(&ContentScreeningReason::ProtectedDomain));
    }

    #[test]
    fn protected_domain_negative() {
        let cfg = config(&[], true, &[]);
        let verdict = screen_text(&["just some unrelated.example text"], &cfg);
        assert!(!verdict.blocked());
    }

    // --- PasswordManager ---

    #[test]
    fn password_manager_strong_phrase_alone_blocks() {
        let cfg = empty_config();
        let verdict = screen_text(&["please enter your master password to continue"], &cfg);
        assert!(verdict
            .reasons
            .contains(&ContentScreeningReason::PasswordManager));
    }

    #[test]
    fn password_manager_brand_and_phrase_within_window_blocks() {
        let cfg = empty_config();
        let verdict = screen_text(&["bitwarden vault unlocked for this device"], &cfg);
        assert!(verdict
            .reasons
            .contains(&ContentScreeningReason::PasswordManager));
    }

    #[test]
    fn password_manager_brand_far_from_phrase_does_not_block() {
        let cfg = empty_config();
        let filler: String = std::iter::repeat("lorem")
            .take(13)
            .collect::<Vec<_>>()
            .join(" ");
        let text = format!("bitwarden {} vault", filler);
        let verdict = screen_text(&[text.as_str()], &cfg);
        assert!(!verdict.blocked());
    }

    #[test]
    fn password_manager_word_boundary_rejects_filler_variants() {
        let cfg = empty_config();
        // "fill" must not match inside "filler"/"fulfill"/"refilled"; with no
        // genuine whole-word brand or phrase present this must not block.
        let verdict = screen_text(
            &["bitwarden team will fulfill and refilled the filler form later"],
            &cfg,
        );
        assert!(!verdict.blocked());
    }

    #[test]
    fn password_manager_word_boundary_rejects_bookkeeper_goalkeeper() {
        let cfg = empty_config();
        // "keeper" must not match inside "bookkeeper"/"goalkeeper", even
        // though "vault" is present nearby as a genuine UI phrase.
        let verdict = screen_text(&["the bookkeeper and goalkeeper opened the vault"], &cfg);
        assert!(!verdict.blocked());
    }

    #[test]
    fn password_manager_whole_word_brand_and_phrase_still_blocks() {
        let cfg = empty_config();
        // Genuine whole-word brand ("keeper") and UI phrase ("vault") within
        // the 12-word window must still block after the boundary fix.
        let verdict = screen_text(&["keeper vault access granted"], &cfg);
        assert!(verdict
            .reasons
            .contains(&ContentScreeningReason::PasswordManager));
    }

    // --- Secret ---

    #[test]
    fn secret_valid_luhn_card_blocks() {
        let cfg = empty_config();
        let verdict = screen_text(&["card number 4111 1111 1111 1111 on file"], &cfg);
        assert!(verdict.reasons.contains(&ContentScreeningReason::Secret));
    }

    #[test]
    fn secret_invalid_luhn_card_does_not_block() {
        let cfg = empty_config();
        let verdict = screen_text(&["card number 4111 1111 1111 1112 on file"], &cfg);
        assert!(!verdict.blocked());
    }

    #[test]
    fn secret_aws_key_blocks() {
        let cfg = empty_config();
        let verdict = screen_text(&["AKIA1234567890ABCDEF leaked in logs"], &cfg);
        assert!(verdict.reasons.contains(&ContentScreeningReason::Secret));
    }

    /// Append the Luhn check digit to `body`.
    fn luhn_complete(body: &str) -> String {
        (0..=9)
            .map(|d| format!("{}{}", body, d))
            .find(|s| luhn_valid(s))
            .expect("some check digit is valid")
    }

    fn secret(text: &str) -> bool {
        screen_text(&[text], &empty_config())
            .reasons
            .contains(&ContentScreeningReason::Secret)
    }

    /// Split `digits` into groups of the given sizes joined by `sep`.
    fn grouped(digits: &str, sizes: &[usize], sep: &str) -> String {
        assert_eq!(digits.len(), sizes.iter().sum::<usize>());
        let mut out = Vec::new();
        let mut at = 0;
        for &s in sizes {
            out.push(&digits[at..at + s]);
            at += s;
        }
        out.join(sep)
    }

    #[test]
    fn secret_card_real_layouts_block() {
        let visa16 = "4111111111111111";
        for sep in [" ", "-", ""] {
            assert!(secret(&grouped(visa16, &[4, 4, 4, 4], sep)), "sep {sep:?}");
        }
        assert!(secret("378282246310005"));
        assert!(secret("3782 822463 10005"));
        assert!(secret("3782-822463-10005"));
        let diners = luhn_complete("3056930902590");
        assert_eq!(diners.len(), 14);
        assert!(secret(&grouped(&diners, &[4, 6, 4], " ")));
        let visa19 = luhn_complete("411111111111111111");
        assert_eq!(visa19.len(), 19);
        assert!(secret(&grouped(&visa19, &[4, 4, 4, 4, 3], " ")));
        assert!(secret(&grouped(&visa19, &[4, 4, 4, 4, 3], "-")));
        assert!(secret(&visa19));
        // Other issuers, unbroken.
        for body in [
            "555555555555444", // Mastercard 51-55
            "222100000000000", // Mastercard 2221-2720
            "601111111111111", // Discover
            "353011133330000", // JCB
            "620000000000000", // UnionPay
            "501800000000000", // Maestro
            "411111111111",    // Visa 13
        ] {
            let number = luhn_complete(body);
            assert!(secret(&number), "{number}");
        }
    }

    #[test]
    fn secret_card_surrounding_punctuation_still_blocks() {
        // Expiry or year after the number does not extend the grouping.
        assert!(secret("4111 1111 1111 1111 12/27"));
        assert!(secret("4111111111111111 2026"));
        assert!(secret("4111-1111-1111-1111 05/29"));
        assert!(secret("378282246310005 1225"));
        assert!(secret("4111 1111 1111 1111 5"));
        assert!(secret("4111 1111 1111 1111"));
        assert!(secret("Card:4111 1111 1111 1111."));
        assert!(secret("(4111-1111-1111-1111), exp 12/30"));
        assert!(secret("n=4111111111111111\nnext 5"));
    }

    #[test]
    fn secret_card_luhn_valid_wrong_layout_does_not_block() {
        // Luhn-valid Visa-prefixed 16-digit numbers in layouts no card uses.
        let digits = luhn_complete("411111111111111");
        assert!(luhn_valid(&digits));
        for layout in [
            &[8usize, 8][..],
            &[3, 3, 3, 3, 4],
            &[2, 4, 4, 4, 2],
            &[5, 5, 6],
        ] {
            for sep in [" ", "-"] {
                let text = grouped(&digits, layout, sep);
                assert!(!secret(&text), "{text}");
            }
        }
        // Ordinary number runs.
        assert!(!secret("101 102 103 104 105"));
        assert!(!secret("2026-10-14 2026-10-15"));
        assert!(!secret("2026-10-14 2026-10-15 2026-10-16"));
        // Multiple spaces, tabs and mixed separators are not a card layout.
        let visa16 = "4111111111111111";
        assert!(luhn_valid(visa16));
        assert!(!secret("4111  1111 1111 1111"));
        assert!(!secret("4111\t1111\t1111\t1111"));
        assert!(!secret("4111 1111-1111 1111"));
        assert!(!secret("4111-1111 1111-1111"));
        assert!(!secret("4111 1111 1111-1111"));
        assert!(!secret("3782 822463-10005"));
    }

    #[test]
    fn secret_card_embedded_in_longer_run_does_not_block() {
        assert!(luhn_valid("4111111111111111"));
        assert!(!secret("1234 4111 1111 1111 1111"));
        assert!(!secret("4111 1111 1111 1111 1111"));
        assert!(!secret("4111-1111-1111-1111-1111"));
        assert!(!secret("5 4111 1111 1111 1111"));
        // Unbroken numbers inside longer digit runs.
        assert!(!secret("94111111111111111"));
        assert!(!secret("41111111111111119"));
        assert!(!secret("41111111111111111111"));
    }

    #[test]
    fn secret_card_unknown_issuer_does_not_block() {
        // Luhn-valid, correct length and layout, but no issuer uses the prefix.
        let digits = luhn_complete("123456781234567");
        assert_eq!(digits.len(), 16);
        assert!(luhn_valid(&digits));
        assert!(!secret(&digits));
        assert!(!secret(&grouped(&digits, &[4, 4, 4, 4], " ")));
        // Visa prefix with a length Visa does not use.
        let visa15 = luhn_complete("41111111111111");
        assert_eq!(visa15.len(), 15);
        assert!(!secret(&visa15));
    }

    const OPENAI_BODY: &str = "Ab3dE6gH9jK2mN5pQ8sT1vW4yZ7bC0eF3hJ6kL9nP2rS5uV8";

    #[test]
    fn secret_token_prefix_inside_identifier_does_not_block() {
        assert!(!secret("task-queue-worker-configuration"));
        assert!(!secret("disk-usage-monitoring-dashboard"));
        assert!(!secret("risk-assessment-framework-document-2026-q3"));
        assert!(!secret("mask-1234567890abcdefghijklmnopqrstuvwxyz"));
        assert!(!secret("sk-learn and sk-ant are just names"));
        assert!(!secret("sk-ant"));
        assert!(!secret(&format!("my-ghp_{}", "a1".repeat(20))));
        assert!(!secret(&format!("xghp_{}", "a1".repeat(20))));
        assert!(!secret("NOTAKIA1234567890ABCDEF"));
        assert!(!secret("my_AIza0123456789abcdefghijklmnopqrstuvwxy"));
        assert!(!secret("a-glpat-abcdefghij0123456789"));
        assert!(!secret("notxoxb-0123456789-abcdef"));
        // Not a 20-character AWS key id: it continues into more letters.
        assert!(!secret("AKIA1234567890ABCDEFG"));
        assert!(!secret("ASIAPACIFICREGIONCODE"));
    }

    #[test]
    fn secret_openai_body_shape_required() {
        // Too short, no digit, or no letter.
        assert!(!secret("sk-abc123def456ghi789jkl012"));
        assert!(!secret("sk-abcdefghijklmnopqrstuvwxyzabcdefghij"));
        assert!(!secret("sk-proj-abcdefghijklmnopqrstuvwxyzabcdefghij"));
        assert!(!secret("sk-1234567890123456789012345678901234567890"));
    }

    #[test]
    fn secret_tokens_block_at_boundaries() {
        let openai = format!("sk-proj-{OPENAI_BODY}");
        let plain = format!("sk-{OPENAI_BODY}");
        let anthropic = format!("sk-ant-api03-{OPENAI_BODY}");
        let github = format!("ghp_{}", "aB3".repeat(12));
        let gitlab = "glpat-abcdefghij0123456789";
        let slack = "xoxb-0123456789-abcdefghij";
        let google = format!("AIza{}", "aB3_".repeat(9) + "xyz");
        for token in [
            openai.as_str(),
            plain.as_str(),
            anthropic.as_str(),
            github.as_str(),
            gitlab,
            slack,
            google.as_str(),
            "AKIA1234567890ABCDEF",
            "ASIA1234567890ABCDEF",
        ] {
            for text in [
                token.to_string(),
                format!("key {token} end"),
                format!("\"{token}\""),
                format!("'{token}'"),
                format!("KEY={token}"),
                format!("token:{token}"),
                format!("({token})"),
                format!("line one\n{token}\n"),
            ] {
                assert!(secret(&text), "{text}");
            }
        }
        // A rejected mid-identifier candidate does not hide a later real key.
        assert!(secret(&format!(
            "task-queue-worker-configuration sk-{OPENAI_BODY}"
        )));
    }

    #[test]
    fn blocked_domain_needs_patterns() {
        let cfg = config(&[], false, &[]);
        assert!(!screen_text(&["visit chase.com"], &cfg).blocked());
        assert!(screen_text(&["visit chase.com"], &config(&["chase"], false, &[])).blocked());
    }

    #[test]
    fn protected_domain_needs_flag() {
        let off = config(&[], false, &[]);
        assert!(!screen_text(&["now streaming at netflix.com/watch"], &off).blocked());
    }

    #[test]
    fn secret_credential_url_blocks() {
        assert!(secret("psql postgres://aiden:S3cret@db.acme.com:5432/prod"));
        assert!(secret("mongodb+srv://user:pw0rd@cluster0.example.net/db"));
        assert!(secret("see https://user:hunter2@example.com/path"));
        // Userinfo without a password, or `@` only in the path: not credentials.
        assert!(!secret("https://example.com:8080/@handle"));
        assert!(!secret("ssh://git@example.com/repo.git"));
    }

    #[test]
    fn secret_jwt_and_pem_block() {
        assert!(secret(
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJ1IjoxfQ.x123abc"
        ));
        assert!(secret("-----BEGIN RSA PRIVATE KEY-----\nMIIEow"));
        assert!(secret("-----BEGIN PRIVATE KEY-----"));
        assert!(secret("-----BEGIN OPENSSH PRIVATE KEY-----"));
    }

    #[test]
    fn non_secret_pii_categories_do_not_block() {
        for text in [
            "plain ipv4 192.168.1.1 and 10.0.0.255",
            "contact marcus.chen@helios-ai.io for details",
            "call (415) 555-0142 or +1 415 555 0142",
            "born 1990-04-12 on 12/04/1990",
            "Invoice IBAN GB82WEST12345698765432 please pay",
            "SSN 123-45-6789 on file",
            "DNI 12345678Z verified",
            "MAC 00:1A:2B:3C:4D:5E",
            "Order ID 1234567890123456",
        ] {
            assert!(!secret(text), "{text}");
        }
    }

    #[test]
    fn keyword_positive_and_negative() {
        let cfg = config(&[], false, &["secretproject"]);
        let verdict = screen_text(&["notes about SecretProject launch plan"], &cfg);
        assert!(verdict.reasons.contains(&ContentScreeningReason::Keyword));

        let verdict = screen_text(&["notes about an unrelated launch plan"], &cfg);
        assert!(!verdict.blocked());
    }

    // --- General behaviour ---

    #[test]
    fn empty_config_passes_ordinary_text() {
        let cfg = empty_config();
        let verdict = screen_text(
            &["just ordinary benign text, nothing suspicious here."],
            &cfg,
        );
        assert!(!verdict.blocked());
        assert!(verdict.reasons.is_empty());
    }

    #[test]
    fn reasons_are_deduplicated_and_sorted() {
        let cfg = config(&["chase"], false, &["secretproject"]);
        let text = "visit chase.com about the SecretProject plan";
        // Same text passed twice across the slice should not duplicate reasons.
        let verdict = screen_text(&[text, text], &cfg);
        assert_eq!(
            verdict.reasons,
            vec![
                ContentScreeningReason::BlockedDomain,
                ContentScreeningReason::Keyword
            ]
        );
    }

    #[test]
    fn unavailable_is_blocked() {
        let verdict = ContentScreeningVerdict::unavailable();
        assert!(verdict.blocked());
        assert_eq!(verdict.codes(), vec!["content_screening_unavailable"]);
    }

    #[test]
    fn input_bound_does_not_panic_on_multibyte_boundary() {
        // Build a string where the naive MAX_INPUT_BYTES cut point would land
        // in the middle of a multi-byte character.
        let mut s = String::new();
        while s.len() < MAX_INPUT_BYTES - 1 {
            s.push('a');
        }
        // 'é' is 2 bytes in UTF-8; the byte at MAX_INPUT_BYTES now falls
        // inside this character rather than on a boundary.
        s.push('é');
        s.push_str("more text after the boundary");

        let bounded = bound_input(&s);
        assert!(bounded.len() <= MAX_INPUT_BYTES);
        assert!(s.is_char_boundary(bounded.len()));

        // Also exercise the public entry point end-to-end with an oversized,
        // multi-byte input to make sure nothing panics.
        let cfg = empty_config();
        let verdict = screen_text(&[s.as_str()], &cfg);
        assert!(!verdict.blocked());
    }

    // --- Redaction-tier plan ---

    #[test]
    fn plan_verdict_is_identical_to_screen_text() {
        let cfg = config(&["chase"], false, &["secretproject"]);
        let card = "card 4111 1111 1111 1111 exp";
        for texts in [
            vec![],
            vec!["just ordinary text"],
            vec![card],
            vec!["visit chase.com", card],
            vec!["", "SecretProject", "master password", card],
            vec!["AKIA1234567890ABCDEF", "AKIA1234567890ABCDEF"],
        ] {
            let plan = screen_text_plan(&texts, &cfg);
            assert_eq!(plan.verdict(), screen_text(&texts, &cfg), "{texts:?}");
            assert!(!plan.blocking().contains(&ContentScreeningReason::Secret));
        }
    }

    #[test]
    fn plan_spans_are_per_input_and_trimmed() {
        let key = format!("sk-proj-{OPENAI_BODY}");
        let texts = [
            "".to_string(),
            "card 4111 1111 1111 1111 12/27".to_string(),
            format!("token:{key} and AKIA1234567890ABCDEF"),
            "nothing here, ssn 123-45-6789".to_string(),
        ];
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let plan = screen_text_plan(&refs, &empty_config());
        assert!(plan.blocking().is_empty());
        assert!(plan.has_secret());
        assert!(plan.secret_spans(0).is_empty());
        let card: Vec<&str> = plan
            .secret_spans(1)
            .iter()
            .map(|r| &texts[1][r.clone()])
            .collect();
        assert_eq!(card, ["4111 1111 1111 1111"]);
        let tokens: Vec<&str> = plan
            .secret_spans(2)
            .iter()
            .map(|r| &texts[2][r.clone()])
            .collect();
        assert_eq!(tokens, [key.as_str(), "AKIA1234567890ABCDEF"]);
        // Non-blocking PII categories never produce redaction spans.
        assert!(plan.secret_spans(3).is_empty());
        assert!(plan.secret_spans(99).is_empty());
    }

    #[test]
    fn plan_debug_prints_counts_not_positions_or_text() {
        let text = "x AKIA1234567890ABCDEF";
        let plan = screen_text_plan(&[text], &empty_config());
        let debug = format!("{plan:?}");
        assert_eq!(
            debug,
            "ContentScreeningPlan { blocking: [], non_redactable: false, secret_hits: 1, url_hits: 0 }"
        );
        assert!(!debug.contains("AKIA"));
    }

    // --- Standard-mode URL credentials ---

    /// The single input redacted with its plan spans, and the URL count.
    fn redact_urls(text: &str) -> (String, usize) {
        let plan = screen_text_plan(&[text], &empty_config());
        let out = crate::content_redaction::replace_secret_spans(text, plan.secret_spans(0))
            .expect("plan spans are valid");
        (out, plan.url_hits())
    }

    #[test]
    fn sensitive_url_keeps_protocol_host_and_path_and_redacts_the_whole_query() {
        for (text, expected) in [
            (
                "open https://example.com/cb?code=abc123&state=xyz now",
                "open https://example.com/cb?[SECRET] now",
            ),
            // No protocol in the text: none is invented.
            ("example.com/cb?token=abc123", "example.com/cb?[SECRET]"),
            ("app.example.io?api_key=k1&x=2", "app.example.io?[SECRET]"),
            // Every parameter of a sensitive URL is redacted.
            (
                "https://e.com/p?a=1&TOKEN=2&b=3",
                "https://e.com/p?[SECRET]",
            ),
            (
                "http://localhost:8080/oauth/callback?Code=4/0Ab&scope=email",
                "http://localhost:8080/oauth/callback?[SECRET]",
            ),
            // Name without a value, and `;` / HTML-escaped separators.
            ("https://e.com/?x=1;sid", "https://e.com/?[SECRET]"),
            ("https://e.com/?x=1&amp;secret=2", "https://e.com/?[SECRET]"),
            // A JWT inside the query merges into one replacement.
            (
                "https://e.com/cb?token=eyJhbGciOiJIUzI1NiJ9.eyJ1IjoxfQ.x123abc&x=1",
                "https://e.com/cb?[SECRET]",
            ),
            // Unicode host, path and value.
            (
                "見て https://例え.jp/パス?password=秘密 。",
                "見て https://例え.jp/パス?[SECRET] 。",
            ),
        ] {
            let (out, urls) = redact_urls(text);
            assert_eq!(out, expected, "{text}");
            assert_eq!(urls, 1, "{text}");
            assert!(secret(text), "{text}");
        }
    }

    #[test]
    fn sensitive_url_fragments_are_redacted_when_they_carry_a_sensitive_name() {
        for (text, expected) in [
            // OAuth implicit flow.
            (
                "https://app.example.com/callback#access_token=ya&expires_in=3600",
                "https://app.example.com/callback#[SECRET]",
            ),
            (
                "https://e.com/#/cb?refresh_token=1",
                "https://e.com/#[SECRET]",
            ),
            // Query and fragment both redacted.
            (
                "https://e.com/?x=1#sig=abc",
                "https://e.com/?[SECRET]#[SECRET]",
            ),
            // A non-sensitive fragment of a sensitive URL is kept.
            (
                "https://e.com/p?sessionid=1#section-2",
                "https://e.com/p?[SECRET]#section-2",
            ),
        ] {
            assert_eq!(redact_urls(text), (expected.to_string(), 1), "{text}");
        }
    }

    #[test]
    fn non_sensitive_urls_are_untouched() {
        for text in [
            "https://example.com/search?q=hello&page=2",
            "example.com/docs?lang=en#code",
            "https://e.com/token/abc/password",
            "https://e.com/?tokens=1&keys=2&codes=3",
            "https://e.com/p#section?x=1",
            "https://e.com/p#code",
            "see netflix.com/watch and b.org",
            "ssh://git@example.com/repo.git",
            // Encoded `=` is not decoded in stage 1.
            "example.com/?token%3Dabc",
            "https://e.com/#access_token%3Dabc",
            // Not a URL: no dot/TLD and no scheme.
            "callback?code=abc",
        ] {
            assert_eq!(redact_urls(text), (text.to_string(), 0), "{text}");
            assert!(!secret(text), "{text}");
        }
    }

    #[test]
    fn credential_urls_redact_userinfo_query_and_secrets_the_detector_hid() {
        for (text, expected) in [
            (
                "psql postgres://aiden:S3cret@db.acme.com:5432/prod",
                "psql postgres://[SECRET]@db.acme.com:5432/prod",
            ),
            (
                "see https://user:hunter2@example.com/path?x=1",
                "see https://[SECRET]@example.com/path?[SECRET]",
            ),
            // A secret in the path of a credential URL is still a hit.
            (
                "https://u:p@example.com/AKIA1234567890ABCDEF/x",
                "https://[SECRET]@example.com/[SECRET]/x",
            ),
            // ... and of an ordinary sensitive URL.
            (
                "https://e.com/AKIA1234567890ABCDEF?code=1",
                "https://e.com/[SECRET]?[SECRET]",
            ),
            // A nested credential URL makes the whole remainder a hit.
            (
                "https://u:p@e.com/r/https://a:b@c.example/",
                "https://[SECRET]@[SECRET]",
            ),
        ] {
            let (out, _) = redact_urls(text);
            assert_eq!(out, expected, "{text}");
        }
        // Counted as sensitive URLs; two URLs in one text count twice.
        let (_, urls) = redact_urls("a https://u:p@e.com/ b example.com?sig=1");
        assert_eq!(urls, 2);
        // Redacted output no longer screens as a secret.
        let (out, _) = redact_urls("see https://user:hunter2@example.com/path?code=1#sig=2");
        assert!(!secret(&out), "{out}");
    }

    #[test]
    fn domain_checks_run_on_the_original_host_of_a_sensitive_url() {
        let cfg = config(&["chase"], true, &[]);
        let plan = screen_text_plan(
            &["https://www.chase.com/cb?code=1 and netflix.com/watch?sid=2"],
            &cfg,
        );
        assert_eq!(
            plan.verdict().reasons,
            vec![
                ContentScreeningReason::BlockedDomain,
                ContentScreeningReason::ProtectedDomain,
                ContentScreeningReason::Secret
            ]
        );
        assert_eq!(plan.url_hits(), 2);
    }

    #[test]
    fn tier_drops_blocking_fail_open_and_too_many_hits() {
        let cfg = config(&["chase"], false, &[]);
        let one = "AKIA1234567890ABCDEF";
        // No hits: pass.
        let plan = screen_text_plan(&["ordinary"], &cfg);
        assert_eq!(screening_tier(&plan, 0, false), ContentScreeningTier::Pass);
        assert_eq!(screening_tier(&plan, 0, true), ContentScreeningTier::Pass);
        // Secrets only: redact unless fail-open or over the hit limit.
        let plan = screen_text_plan(&[one], &cfg);
        assert_eq!(
            screening_tier(&plan, 1, false),
            ContentScreeningTier::Redact
        );
        assert_eq!(
            screening_tier(&plan, MAX_REDACTION_HITS, false),
            ContentScreeningTier::Redact
        );
        let secret_drop = ContentScreeningTier::Drop(ContentScreeningVerdict::secret());
        assert_eq!(screening_tier(&plan, 1, true), secret_drop);
        assert_eq!(
            screening_tier(&plan, MAX_REDACTION_HITS + 1, false),
            secret_drop
        );
        assert_eq!(screening_tier(&plan, 0, false), secret_drop);
        assert_eq!(
            ContentScreeningVerdict::secret().codes(),
            vec!["content_secret"]
        );
        // Any non-secret reason drops with every reason that fired.
        let plan = screen_text_plan(&["visit chase.com", one], &cfg);
        assert_eq!(
            screening_tier(&plan, 1, false),
            ContentScreeningTier::Drop(ContentScreeningVerdict {
                reasons: vec![
                    ContentScreeningReason::BlockedDomain,
                    ContentScreeningReason::Secret
                ],
            })
        );
        let plan = screen_text_plan(&["master password"], &cfg);
        assert_eq!(
            screening_tier(&plan, 0, false),
            ContentScreeningTier::Drop(ContentScreeningVerdict {
                reasons: vec![ContentScreeningReason::PasswordManager],
            })
        );
    }

    // --- Fix round (2026-10-15 review) ---

    const PEM_BODY: &str = "MIIEowIBAAKCAQEAu1SU1LfVLPHCozMxH2Mo4lgOEePzNm0tRgeLezV6ffAt0gun";

    #[test]
    fn pem_private_key_blocks_are_never_redacted() {
        let cfg = empty_config();
        for marker in ["RSA PRIVATE KEY", "PRIVATE KEY", "ENCRYPTED PRIVATE KEY"] {
            // Multi-line (accessibility) and space-joined (Windows OCR) layouts.
            let multi_line =
                format!("-----BEGIN {marker}-----\n{PEM_BODY}\n-----END {marker}-----");
            let ocr = multi_line.replace('\n', " ");
            for text in [multi_line.as_str(), ocr.as_str()] {
                let plan = screen_text_plan(&["ordinary", text], &cfg);
                assert!(plan.non_redactable(), "{text}");
                assert!(plan.has_secret());
                assert_eq!(plan.verdict(), ContentScreeningVerdict::secret());
                for distinct in [1, MAX_REDACTION_HITS] {
                    assert_eq!(
                        screening_tier(&plan, distinct, false),
                        ContentScreeningTier::Drop(ContentScreeningVerdict::secret())
                    );
                }
            }
        }
        let plan = screen_text_plan(&["-----BEGIN PGP SECRET KEY BLOCK-----"], &cfg);
        assert!(plan.non_redactable());
        // Other secrets stay redactable.
        let plan = screen_text_plan(&["key AKIA1234567890ABCDEF"], &cfg);
        assert!(!plan.non_redactable());
        assert_eq!(
            screening_tier(&plan, 1, false),
            ContentScreeningTier::Redact
        );
    }

    #[test]
    fn pem_end_only_and_ocr_damaged_markers_are_non_redactable() {
        let cfg = empty_config();
        for text in [
            "-----END RSA PRIVATE KEY-----",
            "-----END PRIVATE KEY-----",
            "-----END ENCRYPTED PRIVATE KEY-----",
            "-----END PGP SECRET KEY BLOCK-----",
            "-----END OPENSSH PRIVATE KEY",
            "\u{2014}BEGIN RSA PRIVATE KEY\u{2014}",
            "----BEGIN OPENSSH PRIVATE KEY----",
            "BEGIN PRIVATE KEY",
            "some words END RSA PRIVATE KEY more words",
            "BEGIN\nEC\nPRIVATE\nKEY",
        ] {
            let plan = screen_text_plan(&["ordinary", text], &cfg);
            assert!(plan.non_redactable(), "{text}");
            assert_eq!(plan.verdict(), ContentScreeningVerdict::secret(), "{text}");
            assert_eq!(
                screening_tier(&plan, 1, false),
                ContentScreeningTier::Drop(ContentScreeningVerdict::secret()),
                "{text}"
            );
        }
        for text in [
            "private key",
            "Generate a new private key for the server",
            "Keep your private key safe; begin by exporting it",
            "the end of the private key rotation",
        ] {
            let plan = screen_text_plan(&[text], &cfg);
            assert!(!plan.non_redactable(), "{text}");
            assert!(!plan.has_secret(), "{text}");
        }
    }

    #[test]
    fn distinctive_url_values_and_passwords_are_extra_hits() {
        let cfg = empty_config();
        let hits = |text: &str| -> Vec<(String, SecretHitKind)> {
            let plan = screen_text_plan(&[text], &cfg);
            plan.secret_hits(0)
                .iter()
                .map(|(r, kind)| (text[r.clone()].to_string(), *kind))
                .collect()
        };
        let url = "https://e.com/reset?token=Zq81xLmP0wT6AbCd&x=1";
        let found = hits(url);
        assert!(found.contains(&("token=Zq81xLmP0wT6AbCd&x=1".into(), SecretHitKind::UrlPart)));
        assert!(found.contains(&("Zq81xLmP0wT6AbCd".into(), SecretHitKind::UrlValue)));
        // Fragment values, including slashes inside the value.
        let found = hits("https://e.com/cb#access_token=Ab1/Cd2-Ef3x&state=q");
        assert!(found.contains(&("Ab1/Cd2-Ef3x".into(), SecretHitKind::UrlValue)));
        // User-info password.
        let found = hits("https://alice:Pa55w0rd!xyz@e.com/");
        assert!(found.contains(&("alice:Pa55w0rd!xyz".into(), SecretHitKind::UrlPart)));
        assert!(found.contains(&("Pa55w0rd!xyz".into(), SecretHitKind::UrlValue)));
        // Short or single-class values stay URL-only.
        for text in [
            "https://e.com/cb?code=abc",
            "https://e.com/cb?code=abc123",
            "https://e.com/cb?token=abcdefghijkl",
            "https://e.com/cb?token=1234567890",
            "https://u:p@e.com/",
        ] {
            assert!(
                hits(text)
                    .iter()
                    .all(|(_, kind)| *kind != SecretHitKind::UrlValue),
                "{text}"
            );
        }
        // Non-sensitive parameters yield no value hit.
        assert!(hits("https://e.com/?page=Zq81xLmP0wT6AbCd")
            .iter()
            .all(|(_, kind)| *kind != SecretHitKind::UrlValue));
    }

    #[test]
    fn show_password_needs_a_password_manager_brand_nearby() {
        assert!(!screen_text(&["Password  Show password  Sign in"], &empty_config()).blocked());
        assert!(
            screen_text(&["Bitwarden login item: show password"], &empty_config())
                .reasons
                .contains(&ContentScreeningReason::PasswordManager)
        );
    }

    #[test]
    fn sk_prefixed_branch_names_and_short_anthropic_bodies_do_not_hit() {
        for text in [
            "sk-add-user-authentication-flow-for-mobile-2fa-support-v2",
            "git checkout sk-fix-login-redirect-loop-in-oauth-callback-handler-2026",
            "sk-ant-fix-login-redirect-loop-in-oauth-callback-handler-2026",
            "sk-proj-update-the-billing-dashboard-copy-for-q4-2026-launch",
            // Anthropic shape with a body shorter than 40 characters (and
            // too short for the generic rule).
            "sk-ant-api03-Ab3dE6gH9jK2mN5p",
        ] {
            assert!(!secret(text), "{text}");
        }
        // Realistic keys still hit, including hyphenated mixed-case bodies.
        assert!(secret(&format!("sk-ant-api03-{OPENAI_BODY}")));
        assert!(secret(&format!("sk-ant-admin01-{OPENAI_BODY}")));
        assert!(secret(&format!(
            "sk-proj-{}-{}",
            &OPENAI_BODY[..20],
            &OPENAI_BODY[20..]
        )));
    }

    #[test]
    fn database_url_passwords_with_slash_or_question_mark_are_redacted() {
        for (text, expected) in [
            (
                "psql postgres://admin:Ab3/x+Y=@db.example.com/prod now",
                "psql postgres://[SECRET]@db.example.com/prod now",
            ),
            (
                "mysql://root:p?ss/w0rd@10.0.0.5:3306/app",
                "mysql://[SECRET]@10.0.0.5:3306/app",
            ),
            (
                "MongoDB+SRV://svc:a/b@cluster0.example.net/db",
                "MongoDB+SRV://[SECRET]@cluster0.example.net/db",
            ),
        ] {
            let (out, urls) = redact_urls(text);
            assert_eq!(out, expected, "{text}");
            assert_eq!(urls, 1, "{text}");
            assert!(!secret(&out), "{out}");
        }
        // A generic scheme still needs the user-info inside the authority.
        assert!(!secret("https://example.com/a:b@c"));
    }

    #[test]
    fn extended_sensitive_parameter_names_are_whole_and_case_insensitive() {
        for name in [
            "client_secret",
            "ID_TOKEN",
            "private_token",
            "apikey",
            "pwd",
            "passwd",
            "X-Amz-Signature",
            "X-Amz-Credential",
            "x-amz-security-token",
        ] {
            let text = format!("https://bucket.example.com/o?a=1&{name}=v4lue");
            assert_eq!(
                redact_urls(&text),
                ("https://bucket.example.com/o?[SECRET]".to_string(), 1),
                "{name}"
            );
        }
        for text in [
            "https://e.com/?pwds=1",
            "https://e.com/?my_apikey=1",
            "https://e.com/?x-amz-date=20261015",
        ] {
            assert_eq!(redact_urls(text), (text.to_string(), 0), "{text}");
        }
    }

    #[test]
    fn url_hits_count_distinct_url_texts_and_hits_keep_their_provenance() {
        let url = "https://e.com/cb?code=abc123";
        let plan = screen_text_plan(
            &[url, url, &format!("see {url} and {url}")],
            &empty_config(),
        );
        assert_eq!(plan.url_hits(), 1);
        let text = "https://u:p@e.com/AKIA1234567890ABCDEF?token=eyJhbGciOiJIUzI1NiJ9.eyJ1IjoxfQ.x123abc card 4111 1111 1111 1111";
        let plan = screen_text_plan(&[text], &empty_config());
        let kinds: Vec<(&str, SecretHitKind)> = plan
            .secret_hits(0)
            .iter()
            .map(|(r, kind)| (&text[r.clone()], *kind))
            .collect();
        assert!(kinds.contains(&("u:p", SecretHitKind::UrlPart)));
        assert!(kinds.contains(&("AKIA1234567890ABCDEF", SecretHitKind::Detector)));
        assert!(kinds.contains(&(
            "token=eyJhbGciOiJIUzI1NiJ9.eyJ1IjoxfQ.x123abc",
            SecretHitKind::UrlPart
        )));
        assert!(kinds.contains(&(
            "eyJhbGciOiJIUzI1NiJ9.eyJ1IjoxfQ.x123abc",
            SecretHitKind::Detector
        )));
        assert!(kinds.contains(&("4111 1111 1111 1111", SecretHitKind::Card)));
        assert!(plan.secret_hits(1).is_empty());
        // Merged spans are unchanged by provenance tracking.
        let merged: Vec<&str> = plan
            .secret_spans(0)
            .iter()
            .map(|r| &text[r.clone()])
            .collect();
        assert_eq!(
            merged,
            [
                "u:p",
                "AKIA1234567890ABCDEF",
                "token=eyJhbGciOiJIUzI1NiJ9.eyJ1IjoxfQ.x123abc",
                "4111 1111 1111 1111"
            ]
        );
    }
}
