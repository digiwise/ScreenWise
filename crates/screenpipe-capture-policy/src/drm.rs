//! Pure DRM / content-protection matchers.
//!
//! Streaming services and remote-desktop clients that blank their windows
//! while a screen recorder is active. These matchers only classify an app name
//! or URL; pause state, notices and logging live in `screenpipe-engine`'s
//! `drm_detector`, which calls this module.
//!
//! - [`is_drm_app`] / [`is_drm_url`] / [`is_drm_content`]: classify the
//!   foreground app and/or browser URL.
//! - [`text_mentions_drm_url`]: the same URL rule over host-shaped tokens in
//!   free text (OCR / accessibility strings), for content screening.

use crate::url_filter::host_tokens;

/// Apps that trigger macOS content protection while ScreenCaptureKit is active
/// (DRM streaming services plus remote-desktop clients that defensively blank
/// their windows when any app holds an SCK session). Lowercased for comparison.
pub const DRM_APPS: &[&str] = &[
    "netflix",
    "disney+",
    "hulu",
    "prime video",
    "apple tv",
    "peacock",
    "paramount+",
    "hbo max",
    "max",
    "crunchyroll",
    "dazn",
    // Remote-desktop clients using macOS content protection.
    // Matches Omnissa Horizon Client, Omnissa Horizon Client Next,
    // and the legacy VMware Horizon Client branding.
    "horizon client",
];

/// Check whether `app_name` matches a known DRM streaming app.
pub fn is_drm_app(app_name: &str) -> bool {
    let lower = app_name.to_lowercase();
    for &drm in DRM_APPS {
        if drm == "max" {
            if lower == "max" {
                return true;
            }
        } else if lower.contains(drm) {
            return true;
        }
    }
    false
}

/// Known DRM streaming domains.
pub const DRM_DOMAINS: &[&str] = &[
    "netflix.com",
    "disneyplus.com",
    "hulu.com",
    "primevideo.com",
    "tv.apple.com",
    "peacocktv.com",
    "paramountplus.com",
    "play.max.com",
    "crunchyroll.com",
    "dazn.com",
];

/// URL path patterns for streaming services on generic domains.
pub const DRM_URL_PATHS: &[(&str, &str)] = &[("amazon.com", "/gp/video/")];

/// Check whether `url` points to a known DRM streaming site.
pub fn is_drm_url(url: &str) -> bool {
    let lower = url.to_lowercase();
    let host_and_path = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let normalized = host_and_path.strip_prefix("www.").unwrap_or(host_and_path);
    // Extract just the host portion (before any path slash)
    let host = normalized.split('/').next().unwrap_or(normalized);

    for &domain in DRM_DOMAINS {
        // Match exact host or any subdomain (e.g. "apps.disneyplus.com" matches "disneyplus.com")
        if host == domain || host.strip_suffix(domain).is_some_and(|s| s.ends_with('.')) {
            return true;
        }
    }
    for &(domain, path) in DRM_URL_PATHS {
        if normalized.starts_with(domain) {
            if let Some(url_path) = normalized.strip_prefix(domain) {
                if url_path.starts_with(path) {
                    return true;
                }
            }
        }
    }
    false
}

/// Combined check: is the foreground content DRM-protected?
pub fn is_drm_content(app_name: &str, url: Option<&str>) -> bool {
    if is_drm_app(app_name) {
        return true;
    }
    if let Some(u) = url {
        if is_drm_url(u) {
            return true;
        }
    }
    false
}

/// Whether any host-shaped token in `text` is a DRM URL per [`is_drm_url`].
///
/// Tokens without a scheme are tested as `https://<token>`.
pub fn text_mentions_drm_url(text: &str) -> bool {
    host_tokens(text).any(|tok| {
        let normalized = if tok.starts_with("http://") || tok.starts_with("https://") {
            tok.to_string()
        } else {
            format!("https://{}", tok)
        };
        is_drm_url(&normalized)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_drm_app_positive() {
        assert!(is_drm_app("Netflix"));
        assert!(is_drm_app("netflix"));
        assert!(is_drm_app("Disney+"));
        assert!(is_drm_app("Hulu"));
        assert!(is_drm_app("Prime Video"));
        assert!(is_drm_app("Apple TV"));
        assert!(is_drm_app("Peacock"));
        assert!(is_drm_app("Paramount+"));
        assert!(is_drm_app("HBO Max"));
        assert!(is_drm_app("Crunchyroll"));
        assert!(is_drm_app("Max"));
        assert!(is_drm_app("max"));
    }

    #[test]
    fn test_is_drm_app_negative() {
        assert!(!is_drm_app("Finder"));
        assert!(!is_drm_app("Safari"));
        assert!(!is_drm_app("Chrome"));
        assert!(!is_drm_app("Maximize"));
        assert!(!is_drm_app("Max Mustermann"));
        assert!(!is_drm_app("3ds Max"));
        assert!(!is_drm_app("Terminal"));
    }

    #[test]
    fn test_is_drm_app_horizon_variants() {
        assert!(is_drm_app("Omnissa Horizon Client"));
        assert!(is_drm_app("Omnissa Horizon Client Next"));
        assert!(is_drm_app("VMware Horizon Client"));
        assert!(is_drm_app("horizon client"));
        // Guard against overly broad matches.
        assert!(!is_drm_app("Horizon Zero Dawn"));
        assert!(!is_drm_app("Blue Horizon"));
    }

    #[test]
    fn test_is_drm_url_positive() {
        assert!(is_drm_url("https://netflix.com/watch/12345"));
        assert!(is_drm_url("https://www.netflix.com/browse"));
        assert!(is_drm_url("https://disneyplus.com/video/abc"));
        assert!(is_drm_url("https://hulu.com/watch"));
        assert!(is_drm_url("https://primevideo.com/detail"));
        assert!(is_drm_url("https://tv.apple.com/show/123"));
        assert!(is_drm_url("https://peacocktv.com/watch"));
        assert!(is_drm_url("https://paramountplus.com/shows"));
        assert!(is_drm_url("https://play.max.com/movie/abc"));
        assert!(is_drm_url("https://crunchyroll.com/watch"));
        assert!(is_drm_url("http://netflix.com/watch/12345"));
        assert!(is_drm_url(
            "https://www.amazon.com/gp/video/detail/B0CXGTK4HY/ref=atv_hm"
        ));
        assert!(is_drm_url("https://amazon.com/gp/video/detail/something"));
        // subdomain matching
        assert!(is_drm_url(
            "https://apps.disneyplus.com/il/shows/scrubs/123/watch"
        ));
        assert!(is_drm_url("https://watch.hulu.com/show/abc"));
    }

    #[test]
    fn test_is_drm_url_negative() {
        assert!(!is_drm_url("https://google.com"));
        assert!(!is_drm_url("https://github.com"));
        assert!(!is_drm_url("https://max.com"));
        assert!(!is_drm_url("https://example.com/netflix.com"));
        assert!(!is_drm_url("https://amazon.com/dp/B09V3KXJPB"));
        assert!(!is_drm_url("https://amazon.com/s?k=headphones"));
        // subdomain anchor — must not match lookalike domains
        assert!(!is_drm_url("https://netflix.com.evil.com/phish"));
        assert!(!is_drm_url("https://notdisneyplus.com/page"));
    }

    #[test]
    fn test_is_drm_content_combined() {
        assert!(is_drm_content("Netflix", None));
        assert!(is_drm_content(
            "Google Chrome",
            Some("https://netflix.com/watch/123")
        ));
        assert!(is_drm_content(
            "Arc",
            Some("https://www.amazon.com/gp/video/detail/B0CXGTK4HY/ref=atv_hm")
        ));
        assert!(!is_drm_content("Finder", Some("https://google.com")));
        assert!(!is_drm_content("Finder", None));
    }

    #[test]
    fn text_scan_positive() {
        assert!(text_mentions_drm_url("now streaming at netflix.com/watch"));
        assert!(text_mentions_drm_url(
            "see https://play.max.com/movie/abc today"
        ));
        assert!(text_mentions_drm_url("amazon.com/gp/video/detail/x"));
    }

    #[test]
    fn text_scan_negative() {
        assert!(!text_mentions_drm_url("just some unrelated.example text"));
        assert!(!text_mentions_drm_url("watch netflix tonight"));
        assert!(!text_mentions_drm_url("https://netflix.com.evil.com/phish"));
    }
}
