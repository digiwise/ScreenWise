//! Pure, side-effect-free matchers that decide what must not be captured.
//!
//! This leaf crate has no logging, I/O or platform code, so capture
//! admission (accessibility tree walker, window filters, engine) and in-memory
//! content screening (`screenpipe-redact`) share one implementation of each
//! rule instead of injecting predicates or duplicating lists.
//!
//! - [`url_filter`]: domain-boundary URL blocking and a text scan for
//!   host-shaped tokens that mention a blocked domain.
//! - [`window_pattern`]: `App::Title` ignore/include pattern parsing and
//!   matching.
//! - [`drm`]: DRM streaming / protected-content app and URL matchers, plus the
//!   DRM variant of the text URL scan. The stateful pause logic stays in
//!   `screenpipe-engine`'s `drm_detector`.
//! - [`password_managers`]: the canonical password-manager and credential-tool
//!   identities used for process exclusion and on-screen brand screening.
//! - [`credential_files`]: window titles that name a credential file (built-in
//!   file-name shapes plus owner globs), excluded like a configured ignore rule.
//! - [`target`]: the shared gate deciding, from native identity alone, whether
//!   an app or window may be contacted at all (protected apps plus the
//!   configured ignore/include lists).
//!
//! Helpers return plain booleans or borrowed data and never retain or log the
//! text, URLs or titles they are given. The only process-wide state is the
//! owner's extra credential-file patterns, installed from settings
//! ([`credential_files::set_owner_patterns`]).

pub mod credential_files;
pub mod drm;
pub mod password_managers;
pub mod target;
pub mod url_filter;
pub mod window_pattern;
