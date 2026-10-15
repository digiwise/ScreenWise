//! Domain-level URL blocking shared by the vision capture path
//! (`screenpipe-screen`'s `WindowFilters`), the a11y tree walker and content
//! screening: an ignored URL must produce neither frames nor accessibility
//! snapshots, and text that mentions it must not be persisted.
//!
//! - [`is_url_blocked`]: one URL (or bare host) against the blocked patterns.
//! - [`host_tokens`]: the host/URL-shaped tokens found in free text.
//! - [`text_mentions_blocked_url`]: [`is_url_blocked`] over every token of a
//!   text, for OCR / accessibility strings.

use std::sync::OnceLock;

use regex::Regex;
use url::Url;

/// Check if a URL should be filtered out for privacy.
///
/// Uses domain-level matching to avoid false positives (e.g. "chase" won't
/// match "purchase.com"). Patterns are matched case-insensitively against
/// the URL's host:
///
/// 1. Exact match: `host == pattern`
/// 2. Subdomain: host ends with `.{pattern}`
/// 3. No-TLD pattern: `chase` matches `chase.com` / `www.chase.net` / …
///
/// Returns `true` if the URL is blocked (should be skipped).
pub fn is_url_blocked(url: &str, blocked_patterns: &[String]) -> bool {
    if blocked_patterns.is_empty() {
        return false;
    }

    // Normalize so bare hosts ("wellsfargo.com") parse too.
    let url_to_parse = if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("https://{}", url)
    } else {
        url.to_string()
    };

    if let Ok(parsed) = Url::parse(&url_to_parse) {
        if let Some(host) = parsed.host_str() {
            let host_lower = host.to_lowercase();
            return blocked_patterns
                .iter()
                .any(|blocked| host_matches_pattern(&host_lower, &blocked.to_lowercase()));
        }
    }

    // Fallback to simple contains check if URL parsing fails.
    // Less precise, but ensures we don't miss obvious matches.
    let url_lower = url.to_lowercase();
    blocked_patterns
        .iter()
        .any(|blocked| url_lower.contains(&blocked.to_lowercase()))
}

/// Domain-boundary match of one lowercased host against one lowercased pattern.
fn host_matches_pattern(host_lower: &str, blocked: &str) -> bool {
    // Exact match
    if host_lower == blocked {
        return true;
    }

    // Subdomain match: host ends with ".blocked"
    if host_lower.ends_with(&format!(".{}", blocked)) {
        return true;
    }

    // For patterns without a TLD (e.g. "chase" instead of "chase.com"),
    // match the pattern against any domain label. This covers every TLD
    // (chase.com, chase.co.uk, chase.io, …) without hardcoding a list,
    // while still respecting domain boundaries: "purchase.com" splits to
    // ["purchase", "com"], so "chase" never matches it.
    if !blocked.contains('.') {
        return host_lower.split('.').any(|label| label == blocked);
    }

    false
}

/// Host/URL-like tokens: optional scheme, optional leading `www.`, one or
/// more dot-separated labels, a letters-only TLD of at least two characters,
/// and an optional path. This intentionally requires a dot and a plausible
/// TLD so a bare word like "chase" in prose is never extracted as a token —
/// only something host-shaped is ever tested against a blocklist.
fn host_token_regex() -> &'static Regex {
    static HOST_TOKEN: OnceLock<Regex> = OnceLock::new();
    HOST_TOKEN.get_or_init(|| {
        Regex::new(r"(?i)(?:https?://)?(?:www\.)?(?:[a-z0-9-]+\.)+[a-z]{2,}(?:/[^\s]*)?")
            .expect("valid regex")
    })
}

/// Host/URL-shaped tokens in `text`, in order, borrowed from `text`.
///
/// See [`host_token_regex`] for the shape; tokens may carry a scheme and a
/// path but are not otherwise normalized.
pub fn host_tokens(text: &str) -> impl Iterator<Item = &str> {
    host_token_regex().find_iter(text).map(|m| m.as_str())
}

/// Whether any host-shaped token in `text` matches one of `blocked_patterns`
/// by [`is_url_blocked`] rules. Empty `blocked_patterns` never match.
pub fn text_mentions_blocked_url(text: &str, blocked_patterns: &[String]) -> bool {
    !blocked_patterns.is_empty()
        && host_tokens(text).any(|tok| is_url_blocked(tok, blocked_patterns))
}

/// URL-shaped tokens for component inspection (see [`url_token_spans`]):
///
/// - `scheme://` followed by everything up to whitespace (any scheme, any
///   authority: user-info, port, IP literal or `localhost`); or
/// - a scheme-less host of the [`host_token_regex`] shape (at least one dot
///   and a letters-only TLD), an optional numeric port and, optionally, a
///   path, query or fragment up to whitespace.
///
/// Trailing punctuation is kept: callers redact components to the end of the
/// token, which over-covers rather than under-covers.
fn url_token_regex() -> &'static Regex {
    static URL_TOKEN: OnceLock<Regex> = OnceLock::new();
    URL_TOKEN.get_or_init(|| {
        Regex::new(
            r"(?i)[a-z][a-z0-9+.-]*://[^\s]*|(?:[a-z0-9-]+\.)+[a-z]{2,}(?::[0-9]{1,5})?(?:[/?#][^\s]*)?",
        )
        .expect("valid regex")
    })
}

/// Byte ranges of URL-shaped tokens in `text`, in order and non-overlapping,
/// with or without a scheme. Unlike [`host_tokens`], a token keeps any
/// user-info, port, query and fragment, and a query or fragment may follow
/// the host directly (`example.com?x=1`). Nothing is decoded or normalized.
pub fn url_token_spans(text: &str) -> impl Iterator<Item = std::ops::Range<usize>> + '_ {
    url_token_regex().find_iter(text).map(|m| m.range())
}

/// Byte ranges of the components of one URL-shaped token (relative to the
/// token), split by the generic RFC 3986 delimiters without decoding:
/// the authority ends at the first `/`, `?` or `#` after `scheme://`; the
/// query starts after the first `?` before any `#`; the fragment after the
/// first `#`. Delimiters themselves are excluded from every range. `Debug`
/// prints which components are present, never their positions.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct UrlTokenParts {
    /// The scheme, without `://`. `None` for a scheme-less token.
    pub scheme: Option<std::ops::Range<usize>>,
    /// Everything before the last `@` of the authority. Only recognised
    /// when a scheme is present, so a bare `user@example.com` stays an
    /// address rather than user-info.
    pub userinfo: Option<std::ops::Range<usize>>,
    /// Host and optional port.
    pub host: std::ops::Range<usize>,
    /// The path (possibly empty), from its leading `/`.
    pub path: std::ops::Range<usize>,
    /// The query after `?` (possibly empty). `None` without a `?`.
    pub query: Option<std::ops::Range<usize>>,
    /// The fragment after `#` (possibly empty). `None` without a `#`.
    pub fragment: Option<std::ops::Range<usize>>,
}

impl std::fmt::Debug for UrlTokenParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UrlTokenParts")
            .field("scheme", &self.scheme.is_some())
            .field("userinfo", &self.userinfo.is_some())
            .field("query", &self.query.is_some())
            .field("fragment", &self.fragment.is_some())
            .finish()
    }
}

/// Split a URL-shaped token (for example one of [`url_token_spans`]) into
/// [`UrlTokenParts`]. Never fails: an unusual token yields an empty host or
/// path rather than an error.
pub fn split_url_token(token: &str) -> UrlTokenParts {
    let mut parts = UrlTokenParts::default();
    let authority_start = match token.find("://") {
        Some(index)
            if index > 0
                && token[..index]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-')) =>
        {
            parts.scheme = Some(0..index);
            index + 3
        }
        _ => 0,
    };
    let rest = &token[authority_start..];
    let authority_end = authority_start
        + rest
            .find(|c: char| matches!(c, '/' | '?' | '#'))
            .unwrap_or(rest.len());
    let mut host_start = authority_start;
    if parts.scheme.is_some() {
        if let Some(at) = token[authority_start..authority_end].rfind('@') {
            parts.userinfo = Some(authority_start..authority_start + at);
            host_start = authority_start + at + 1;
        }
    }
    parts.host = host_start..authority_end;
    let fragment_at = token[authority_end..]
        .find('#')
        .map(|index| authority_end + index);
    let before_fragment = fragment_at.unwrap_or(token.len());
    let query_at = token[authority_end..before_fragment]
        .find('?')
        .map(|index| authority_end + index);
    parts.path = authority_end..query_at.unwrap_or(before_fragment);
    parts.query = query_at.map(|index| index + 1..before_fragment);
    parts.fragment = fragment_at.map(|index| index + 1..token.len());
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocked(patterns: &[&str]) -> Vec<String> {
        patterns.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_empty_list_blocks_nothing() {
        assert!(!is_url_blocked("https://wellsfargo.com", &[]));
    }

    #[test]
    fn test_exact_domain_match() {
        let b = blocked(&["wellsfargo.com"]);
        assert!(is_url_blocked("https://wellsfargo.com", &b));
        assert!(is_url_blocked("https://wellsfargo.com/login", &b));
        assert!(is_url_blocked("https://www.wellsfargo.com", &b));
        assert!(is_url_blocked("https://online.wellsfargo.com/account", &b));
    }

    #[test]
    fn test_case_insensitive() {
        let b = blocked(&["WellsFargo.com"]);
        assert!(is_url_blocked("https://WELLSFARGO.COM", &b));
        assert!(is_url_blocked("https://wellsfargo.com/Login", &b));
    }

    #[test]
    fn test_no_false_positive_on_substring() {
        let b = blocked(&["chase.com"]);
        assert!(is_url_blocked("https://chase.com", &b));
        assert!(is_url_blocked("https://www.chase.com/login", &b));
        assert!(!is_url_blocked("https://purchase.com", &b));
        assert!(!is_url_blocked("https://showcase.example.com", &b));
    }

    #[test]
    fn test_partial_domain_pattern_matches_any_tld() {
        let b = blocked(&["chase"]);
        assert!(is_url_blocked("https://chase.com", &b));
        assert!(is_url_blocked("https://www.chase.com", &b));
        assert!(is_url_blocked("https://chase.bank", &b));
        // TLDs not in the old hardcoded list still match (no TLD allowlist).
        assert!(is_url_blocked("https://chase.co.uk", &b));
        assert!(is_url_blocked("https://chase.io", &b));
        assert!(is_url_blocked("https://online.chase.de/account", &b));
        // Domain boundaries are still respected — no substring false positives.
        assert!(!is_url_blocked("https://purchase.com", &b));
        assert!(!is_url_blocked("https://purchase.co.uk", &b));
        assert!(!is_url_blocked("https://showcase.example.com", &b));
    }

    #[test]
    fn test_without_protocol() {
        let b = blocked(&["wellsfargo.com"]);
        assert!(is_url_blocked("wellsfargo.com", &b));
        assert!(is_url_blocked("www.wellsfargo.com/account", &b));
    }

    #[test]
    fn test_multiple_patterns() {
        let b = blocked(&["wellsfargo.com", "chase.com", "bankofamerica.com"]);
        assert!(is_url_blocked("https://chase.com/login", &b));
        assert!(is_url_blocked("https://www.bankofamerica.com", &b));
        assert!(!is_url_blocked("https://google.com", &b));
    }

    #[test]
    fn text_scan_positive() {
        let b = blocked(&["chase"]);
        assert!(text_mentions_blocked_url(
            "please visit chase.com to log in",
            &b
        ));
        assert!(text_mentions_blocked_url(
            "see https://www.chase.co.uk/login now",
            &b
        ));
    }

    #[test]
    fn text_scan_respects_domain_boundaries_and_prose() {
        let b = blocked(&["chase"]);
        assert!(!text_mentions_blocked_url(
            "please visit purchase.com to check out",
            &b
        ));
        // A bare word with no dot/TLD is never a host token.
        assert!(!text_mentions_blocked_url(
            "I will chase my dream today",
            &b
        ));
    }

    #[test]
    fn text_scan_needs_patterns() {
        assert!(!text_mentions_blocked_url("visit chase.com", &[]));
    }

    #[test]
    fn host_tokens_extracts_host_shaped_tokens_only() {
        let tokens: Vec<&str> =
            host_tokens("go to https://a.example.com/x?y=1 or b.org, not chase").collect();
        assert_eq!(tokens, ["https://a.example.com/x?y=1", "b.org"]);
    }

    fn url_tokens(text: &str) -> Vec<&str> {
        url_token_spans(text).map(|r| &text[r]).collect()
    }

    #[test]
    fn url_token_spans_cover_schemes_userinfo_ports_and_bare_queries() {
        assert_eq!(
            url_tokens("open http://localhost:8080/cb?code=x now"),
            ["http://localhost:8080/cb?code=x"]
        );
        assert_eq!(
            url_tokens("db postgres://u:p@10.0.0.1:5432/x and example.com?token=1"),
            ["postgres://u:p@10.0.0.1:5432/x", "example.com?token=1"]
        );
        assert_eq!(
            url_tokens("app.example.co.uk:8443#access_token=abc, done."),
            ["app.example.co.uk:8443#access_token=abc,"]
        );
        // Prose, version numbers and a bare word are not URL tokens.
        assert!(url_tokens("version 1.2.3 and chase my dream").is_empty());
        // Multi-byte text around a token keeps valid byte ranges.
        let text = "é https://ex.com/ü?k=é é";
        assert_eq!(url_tokens(text), ["https://ex.com/ü?k=é"]);
    }

    fn part<'a>(token: &'a str, range: &Option<std::ops::Range<usize>>) -> Option<&'a str> {
        range.clone().map(|r| &token[r])
    }

    #[test]
    fn split_url_token_finds_components_without_decoding() {
        let token = "https://user:pa%3Ass@host.example:8443/a/b?x=1&token%3Dy#frag?z=2";
        let parts = split_url_token(token);
        assert_eq!(part(token, &parts.scheme), Some("https"));
        assert_eq!(part(token, &parts.userinfo), Some("user:pa%3Ass"));
        assert_eq!(&token[parts.host.clone()], "host.example:8443");
        assert_eq!(&token[parts.path.clone()], "/a/b");
        assert_eq!(part(token, &parts.query), Some("x=1&token%3Dy"));
        assert_eq!(part(token, &parts.fragment), Some("frag?z=2"));

        // Scheme-less: no user-info, query directly after the host.
        let token = "example.com?code=abc";
        let parts = split_url_token(token);
        assert!(parts.scheme.is_none() && parts.userinfo.is_none());
        assert_eq!(&token[parts.host.clone()], "example.com");
        assert_eq!(&token[parts.path.clone()], "");
        assert_eq!(part(token, &parts.query), Some("code=abc"));
        assert!(parts.fragment.is_none());

        // `@` only in the path is not user-info; `?` inside the fragment is
        // not a query.
        let token = "https://example.com:8080/@handle#a?b";
        let parts = split_url_token(token);
        assert!(parts.userinfo.is_none());
        assert_eq!(&token[parts.path.clone()], "/@handle");
        assert!(parts.query.is_none());
        assert_eq!(part(token, &parts.fragment), Some("a?b"));

        // Last `@` of the authority separates user-info.
        let token = "postgres://u:p@ss@db/x";
        assert_eq!(
            part(token, &split_url_token(token).userinfo),
            Some("u:p@ss")
        );
        // Empty query and fragment are present but empty.
        let token = "https://e.com/?#";
        let parts = split_url_token(token);
        assert_eq!(part(token, &parts.query), Some(""));
        assert_eq!(part(token, &parts.fragment), Some(""));
    }

    #[test]
    fn url_token_parts_debug_prints_presence_not_positions() {
        assert_eq!(
            format!("{:?}", split_url_token("https://u:p@host.example/x?q=1#f")),
            "UrlTokenParts { scheme: true, userinfo: true, query: true, fragment: true }"
        );
        assert_eq!(
            format!("{:?}", split_url_token("host.example/x")),
            "UrlTokenParts { scheme: false, userinfo: false, query: false, fragment: false }"
        );
    }
}
