//! Window titles that name a credential file (incident SI-1, fix A).
//!
//! A window whose title names a credential file is treated exactly like a
//! window matching a configured exclusion, on every capture path: nothing is
//! captured from it and its title, app and URL are not recorded. The rule is
//! built in and always on; owners may add their own file-name patterns
//! (`credentialFilePatterns`, see [`set_owner_patterns`]).
//!
//! The rule matches **file-name shapes, never bare words**, so that ordinary
//! windows about passwords or secrets ("Secret Santa planning", a password
//! settings page) stay recordable. The deliberately removed bare-word title
//! defaults (`screenpipe-a11y` `config.rs`) are not reintroduced.
//!
//! # Built-in list
//!
//! The title is split into whitespace-separated tokens. Each token loses
//! editor decorations (a leading `*`, `●` or `•` unsaved marker, surrounding
//! quotes or brackets, trailing `:`/`,`/`;`) and any directory part (after the
//! last `/` or `\`). The remaining file name matches, case-insensitively,
//! when it:
//!
//! 1. is exactly one of [`CREDENTIAL_FILE_NAMES`] (`.env`, `.npmrc`,
//!    `.pypirc`, `.netrc`, `_netrc`, `.git-credentials`, `.pgpass`,
//!    `.htpasswd`, `wallet.dat`, `id_rsa`, `id_ed25519`, `id_ecdsa`, `id_dsa`);
//! 2. starts with `.env.` (`.env.local`, `.env.production`), except the
//!    committed templates in [`ENV_TEMPLATE_SUFFIXES`] (`.env.example`, …);
//! 3. has a non-empty stem and ends in one of the [`CREDENTIAL_EXTENSIONS`]
//!    (a bare `.pem` names a format, not a file) (`.secret`, `.pem`,
//!    `.key` (also Apple Keynote decks), `.p12`, `.pfx`, `.kdbx`, `.ppk`,
//!    `.jks`, `.keystore`);
//! 4. has a data/config extension from [`DATA_EXTENSIONS`] (`.txt`, `.json`,
//!    `.yaml`, …; never a source-code or office extension) and a stem
//!    containing one of the whole words in [`CREDENTIAL_STEM_WORDS`]
//!    (`token`, `secret`, `secrets`, `credential(s)`, `password(s)`, `passwd`,
//!    `apikey(s)`; words split at `.`, `_`, `-`, digits and camelCase,
//!    adjacent pairs joined so `api_key` is `apikey`). The plural `tokens` is
//!    deliberately absent (design tokens, NLP tokenizer vocabularies):
//!    `tokens.json`, `design-tokens.json`, `tokenizer.json` and
//!    `token_service.rs` do not match. File-name-shaped ordinary names such
//!    as `secret-santa-list.txt` or `password-policy.txt` do match.
//!
//! The bare file name `credentials` (the AWS/GCP CLI file) matches only as the
//! final component of a path (`~/.aws/credentials`) or, written exactly in
//! lower case, as the whole first title segment (`credentials - Notepad`): a
//! page titled `Credentials | Console` or `Manage your credentials` does not.
//! Public keys (`id_rsa.pub`) are not matched. A file name containing spaces
//! is judged by its last whitespace-separated word only (`Team passwords.txt`
//! matches through `passwords.txt`; `Secret notes.txt` does not).
//!
//! # Owner patterns
//!
//! Each owner pattern is a case-insensitive file-name glob (`*` any run of
//! characters, `?` one character), matched against every decorated-stripped
//! token file name and every whole title segment (so names with spaces can be
//! listed). A pattern without wildcards must equal the whole name. Blank
//! entries are ignored.
//!
//! Like the rest of this crate, the helpers never log or retain the titles
//! they are given. The owner patterns are the one piece of process-wide state:
//! they are installed from settings (see [`set_owner_patterns`]); the built-in
//! list applies even when nothing was installed.

use std::sync::{OnceLock, RwLock};

use regex::Regex;

/// Credential context words, case-insensitive and matched anywhere (also
/// inside identifiers such as `API_TOKEN` and OCR-split text such as
/// `read- token. secret`); separators inside two-word phrases are optional.
/// Content screening uses them as context for token-shaped strings (beside
/// the string, or in a native window title); they never exclude a window.
pub const CREDENTIAL_CONTEXT_PATTERN: &str = r"(?i)token|secret|pass[\s_\-]?word|passwd|passphrase|credential|bearer|authori[sz]ation|api[\s_.\-]*key|access[\s_.\-]*key|private[\s_.\-]*key|vault|\.env\b";

/// The compiled [`CREDENTIAL_CONTEXT_PATTERN`].
pub fn credential_context_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(CREDENTIAL_CONTEXT_PATTERN).expect("valid regex"))
}

/// `true` when `text` (typically a native window title) contains a
/// credential context word or names a credential file. Weaker than
/// [`title_names_credential_file`]: it only lends credential context.
pub fn text_has_credential_word(text: &str) -> bool {
    credential_context_regex().is_match(text) || title_names_credential_file(text)
}

/// Exact credential file names (compared case-insensitively).
pub const CREDENTIAL_FILE_NAMES: &[&str] = &[
    ".env",
    ".npmrc",
    ".pypirc",
    ".netrc",
    "_netrc",
    ".git-credentials",
    ".pgpass",
    ".htpasswd",
    "wallet.dat",
    "id_rsa",
    "id_ed25519",
    "id_ecdsa",
    "id_dsa",
];

/// `.env.<suffix>` names that are committed templates, not credential files.
pub const ENV_TEMPLATE_SUFFIXES: &[&str] = &["example", "sample", "template", "dist"];

/// Extensions that make any file name a credential file.
pub const CREDENTIAL_EXTENSIONS: &[&str] = &[
    "secret", "pem", "key", "p12", "pfx", "kdbx", "ppk", "jks", "keystore",
];

/// Data/config extensions that make a file name a credential file when its
/// stem contains a [`CREDENTIAL_STEM_WORDS`] word. Source-code, document and
/// web extensions are deliberately absent (`token_service.rs`,
/// `secret_handshake.md` and `tokens.html` stay recordable), as are
/// extensions that are also top-level domains.
pub const DATA_EXTENSIONS: &[&str] = &[
    "txt",
    "json",
    "yaml",
    "yml",
    "toml",
    "ini",
    "cfg",
    "conf",
    "config",
    "env",
    "xml",
    "csv",
    "properties",
    "plist",
    "dat",
    "bak",
    "old",
    "enc",
    "gpg",
    "asc",
    "age",
];

/// Whole stem words that name a credential in a data file name.
pub const CREDENTIAL_STEM_WORDS: &[&str] = &[
    "token",
    "secret",
    "secrets",
    "credential",
    "credentials",
    "password",
    "passwords",
    "passwd",
    "apikey",
    "apikeys",
];

/// Title segment separators used by editors and browsers (`file - App`).
const SEGMENT_SEPARATORS: &[&str] = &[" - ", " \u{2014} ", " \u{2013} ", " | "];

/// Leading editor decorations: unsaved markers, quotes and brackets.
const LEADING_DECORATIONS: &[char] = &[
    '*', '\u{25CF}', '\u{2022}', '"', '\'', '`', '(', '[', '{', '<', '\u{201C}', '\u{2018}',
];

/// Trailing decorations and punctuation.
const TRAILING_DECORATIONS: &[char] = &[
    '*', '"', '\'', '`', ')', ']', '}', '>', ':', ',', ';', '!', '?', '\u{201D}', '\u{2019}',
];

/// `true` when the window title names a credential file by the built-in list
/// or an installed owner pattern. Empty titles never match.
pub fn title_names_credential_file(title: &str) -> bool {
    builtin_title_names_credential_file(title) || owner_patterns_match(title)
}

/// The built-in list alone (see the module docs).
pub fn builtin_title_names_credential_file(title: &str) -> bool {
    if title.trim().is_empty() {
        return false;
    }
    if first_segment(title) == "credentials" {
        return true;
    }
    title.split_whitespace().any(|raw| {
        let (name, from_path) = file_name_of(raw);
        !name.is_empty() && file_name_is_credential(name, from_path)
    })
}

/// A parsed owner pattern (lowercase glob characters).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerPattern(Vec<char>);

impl OwnerPattern {
    /// Parse one owner pattern; `None` for blank input.
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| Self(trimmed.to_lowercase().chars().collect()))
    }

    /// Parse a list, dropping blank entries.
    pub fn parse_list(raw: &[String]) -> Vec<Self> {
        raw.iter().filter_map(|s| Self::parse(s)).collect()
    }

    /// Whether this glob matches the whole of `name` (case-insensitive).
    pub fn matches(&self, name: &str) -> bool {
        let name: Vec<char> = name.to_lowercase().chars().collect();
        glob_matches(&self.0, &name)
    }
}

/// `true` when any of `patterns` matches a token file name or a whole title
/// segment of `title`.
pub fn title_matches_owner_patterns(title: &str, patterns: &[OwnerPattern]) -> bool {
    if patterns.is_empty() || title.trim().is_empty() {
        return false;
    }
    let mut candidates: Vec<&str> = title
        .split_whitespace()
        .map(|raw| file_name_of(raw).0)
        .filter(|name| !name.is_empty())
        .collect();
    candidates.extend(
        segments(title)
            .map(strip_decorations)
            .filter(|segment| !segment.is_empty()),
    );
    candidates
        .iter()
        .any(|candidate| patterns.iter().any(|pattern| pattern.matches(candidate)))
}

fn owner_store() -> &'static RwLock<Vec<OwnerPattern>> {
    static PATTERNS: OnceLock<RwLock<Vec<OwnerPattern>>> = OnceLock::new();
    PATTERNS.get_or_init(|| RwLock::new(Vec::new()))
}

/// Install the owner's extra credential-file patterns
/// (`credentialFilePatterns`), replacing any earlier set. Called when
/// recording settings are resolved; the built-in list needs no installation.
pub fn set_owner_patterns(raw: &[String]) {
    let parsed = OwnerPattern::parse_list(raw);
    match owner_store().write() {
        Ok(mut guard) => *guard = parsed,
        Err(poisoned) => *poisoned.into_inner() = parsed,
    }
}

/// Number of installed owner patterns (a count only, for diagnostics/tests).
pub fn owner_pattern_count() -> usize {
    match owner_store().read() {
        Ok(guard) => guard.len(),
        Err(poisoned) => poisoned.into_inner().len(),
    }
}

fn owner_patterns_match(title: &str) -> bool {
    match owner_store().read() {
        Ok(guard) => title_matches_owner_patterns(title, &guard),
        Err(poisoned) => title_matches_owner_patterns(title, &poisoned.into_inner()),
    }
}

/// Title segments split at editor/browser separators.
fn segments(title: &str) -> impl Iterator<Item = &str> {
    let mut parts = vec![title];
    for separator in SEGMENT_SEPARATORS {
        parts = parts
            .into_iter()
            .flat_map(|part| part.split(separator))
            .collect();
    }
    parts.into_iter()
}

/// The decoration-stripped first title segment (original case).
fn first_segment(title: &str) -> &str {
    segments(title).next().map(strip_decorations).unwrap_or("")
}

fn strip_decorations(raw: &str) -> &str {
    raw.trim()
        .trim_start_matches(|c: char| LEADING_DECORATIONS.contains(&c) || c.is_whitespace())
        .trim_end_matches(|c: char| TRAILING_DECORATIONS.contains(&c) || c.is_whitespace())
}

/// The file name of one whitespace token (decorations and directories
/// removed) and whether it carried a directory part.
fn file_name_of(raw: &str) -> (&str, bool) {
    let token = strip_decorations(raw);
    match token.rfind(['/', '\\']) {
        Some(index) => (strip_decorations(&token[index + 1..]), true),
        None => (token, false),
    }
}

fn file_name_is_credential(name: &str, from_path: bool) -> bool {
    let lower = name.to_lowercase();
    if CREDENTIAL_FILE_NAMES.contains(&lower.as_str()) {
        return true;
    }
    if from_path && lower == "credentials" {
        return true;
    }
    if let Some(suffix) = lower.strip_prefix(".env.") {
        return !suffix.is_empty() && !ENV_TEMPLATE_SUFFIXES.contains(&suffix);
    }
    let Some(dot) = lower.rfind('.') else {
        return false;
    };
    let extension = &lower[dot + 1..];
    if extension.is_empty() || !extension.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    // A bare extension (`.pem` in "Convert .pem to .pfx") names a format,
    // not a file: the extension rules need a stem.
    if dot == 0 {
        return false;
    }
    if CREDENTIAL_EXTENSIONS.contains(&extension) {
        return true;
    }
    DATA_EXTENSIONS.contains(&extension) && stem_has_credential_word(&name[..dot])
}

/// Whole words of a file stem: split at non-alphanumerics, letter/digit
/// changes and camelCase humps (`MyAPIToken` is `my`, `api`, `token`).
fn stem_words(stem: &str) -> Vec<String> {
    let chars: Vec<char> = stem.chars().collect();
    let mut words = Vec::new();
    let mut current = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some(&prev) = i.checked_sub(1).and_then(|p| chars.get(p)) {
            let next = chars.get(i + 1).copied();
            let boundary = (prev.is_alphabetic() != c.is_alphabetic() && prev.is_alphanumeric())
                || (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_uppercase()
                    && c.is_uppercase()
                    && next.is_some_and(|n| n.is_lowercase()));
            if boundary && !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
        }
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn stem_has_credential_word(stem: &str) -> bool {
    let words = stem_words(stem);
    words
        .iter()
        .any(|word| CREDENTIAL_STEM_WORDS.contains(&word.as_str()))
        || words
            .windows(2)
            .any(|pair| CREDENTIAL_STEM_WORDS.contains(&format!("{}{}", pair[0], pair[1]).as_str()))
}

/// Iterative glob match (`*`, `?`) over chars; linear backtracking.
fn glob_matches(pattern: &[char], name: &[char]) -> bool {
    let (mut p, mut n) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut mark = 0usize;
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            mark = n;
            p += 1;
        } else if let Some(s) = star {
            p = s + 1;
            mark += 1;
            n = mark;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

#[cfg(test)]
pub(crate) fn owner_patterns_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic titles that must be treated as credential files.
    const MATCHES: &[&str] = &[
        ".example-dns-read-token.secret - Notepad",
        "*.example-dns-read-token.secret - Notepad",
        "\u{25CF} .env - project - Visual Studio Code",
        ".env.local - Notepad++",
        "*C:\\Users\\someone\\repo\\.env.production - Notepad++",
        "server.pem - Notepad",
        "tls.KEY - Editor",
        "client.p12",
        "bundle.pfx - Viewer",
        "Personal.kdbx - Keeper of things",
        "putty-session.ppk - Notepad",
        "id_rsa - Notepad",
        "C:\\Users\\someone\\.ssh\\id_ed25519 - Notepad",
        "id_ecdsa",
        "credentials - Notepad",
        "~/.aws/credentials - Text Editor",
        "C:\\Users\\someone\\.aws\\credentials - Notepad",
        ".npmrc - Notepad",
        ".pypirc - Notepad",
        "_netrc - Notepad",
        ".netrc",
        ".git-credentials - Notepad",
        "\"api-token.txt\" - Notepad",
        "client_secret.json - Visual Studio Code",
        "accessToken.json - Editor",
        "MyAPIToken.yaml - Editor",
        "service_api_key.ini - Notepad",
        "secrets.yaml - project - Visual Studio Code",
        "token2.json - Editor",
        "passwords.txt - Notepad",
        "db-credentials.conf - Notepad",
        "(github-token.txt) - Editor",
        "deploy.secret [Read Only] - Editor",
        ".htpasswd - Notepad",
        "wallet.dat - Viewer",
        "release.keystore - Android Studio",
        "trust.jks",
        "api-keys.json - Editor",
        "Team passwords.txt - Notepad",
    ];

    /// Synthetic titles that must stay recordable.
    const NON_MATCHES: &[&str] = &[
        "",
        "Secret Santa Planning - Word",
        "Secret Santa Planning.docx - Word",
        "Password settings - Settings",
        "Manage your credentials - Settings",
        "Credentials | Example Console - Browser",
        "Credentials - Notepad",
        "AWS Secrets Manager - Browser",
        "token_service.rs - project - Visual Studio Code",
        "secret_handshake.md - Typora",
        "tokenizer.json - Editor",
        "design-tokens.json - Editor",
        "special_tokens_map.json - Editor",
        "id_rsa.pub - Notepad",
        ".env.example - Notepad",
        ".env.sample - Notepad",
        ".envrc - Notepad",
        "environment.yml - Editor",
        "secretlab.example - Browser",
        "Token economics explained - Browser",
        "How to store a secret in config.json - Blog - Browser",
        "keyboard.txt - Notepad",
        "monkey.json - Editor",
        "notes.txt - Notepad",
        "README.md - Notepad",
        "Inbox - Mail",
        "Secret notes.txt - Notepad",
        "tokens.json - Editor",
        "keystore-guide.md - Editor",
        "htpasswd manual - Browser",
        "Convert .pem to .pfx - Q&A - Browser",
        "Using the .key format (.key, .p12) - Help",
        "*.kdbx files explained - Browser",
    ];

    #[test]
    fn built_in_list_matches_credential_file_titles() {
        for title in MATCHES {
            assert!(builtin_title_names_credential_file(title), "{title}");
        }
    }

    #[test]
    fn built_in_list_ignores_bare_words_and_ordinary_files() {
        for title in NON_MATCHES {
            assert!(!builtin_title_names_credential_file(title), "{title}");
        }
    }

    #[test]
    fn stem_words_split_case_digits_and_separators() {
        assert_eq!(stem_words("MyAPIToken"), ["my", "api", "token"]);
        assert_eq!(stem_words("token2"), ["token", "2"]);
        assert_eq!(stem_words("db-credentials"), ["db", "credentials"]);
        assert_eq!(stem_words("tokenizer"), ["tokenizer"]);
    }

    #[test]
    fn owner_patterns_are_whole_name_globs() {
        let patterns = OwnerPattern::parse_list(&[
            "*.gpg".to_string(),
            "vault-??.txt".to_string(),
            "  ".to_string(),
            "My Recovery Codes.txt".to_string(),
        ]);
        assert_eq!(patterns.len(), 3);
        assert!(title_matches_owner_patterns(
            "backup.GPG - Viewer",
            &patterns
        ));
        assert!(title_matches_owner_patterns(
            "vault-01.txt - Notepad",
            &patterns
        ));
        assert!(!title_matches_owner_patterns(
            "vault-001.txt - Notepad",
            &patterns
        ));
        assert!(title_matches_owner_patterns(
            "*My Recovery Codes.txt - Notepad",
            &patterns
        ));
        assert!(!title_matches_owner_patterns(
            "gpg guide - Browser",
            &patterns
        ));
        assert!(!title_matches_owner_patterns("anything", &[]));
    }

    #[test]
    fn installed_owner_patterns_extend_the_built_in_list() {
        let _guard = owner_patterns_test_lock();
        set_owner_patterns(&["*.vault".to_string()]);
        assert_eq!(owner_pattern_count(), 1);
        assert!(title_names_credential_file("family.vault - Notepad"));
        assert!(title_names_credential_file("id_rsa - Notepad"));
        set_owner_patterns(&[]);
        assert!(!title_names_credential_file("family.vault - Notepad"));
        assert!(title_names_credential_file("id_rsa - Notepad"));
    }

    #[test]
    fn credential_words_lend_context_without_excluding() {
        for title in [
            "API token rotation - Docs",
            "Client secret settings - Console",
            "Vault overview - Browser",
            "cat .env.local - Terminal",
            "id_rsa - Notepad",
        ] {
            assert!(text_has_credential_word(title), "{title}");
        }
        for title in [
            "Quarterly report - Word",
            "Inbox - Mail",
            ".envrc - Notepad",
        ] {
            assert!(!text_has_credential_word(title), "{title}");
        }
        // Context words alone never exclude a window.
        assert!(!title_names_credential_file("API token rotation - Docs"));
    }

    #[test]
    fn glob_handles_stars_anywhere() {
        let p = |s: &str| OwnerPattern::parse(s).unwrap();
        assert!(p("*").matches("anything"));
        assert!(p("a*b*c").matches("aXXbYYc"));
        assert!(!p("a*b*c").matches("aXXbYY"));
        assert!(p("*secret*").matches("my-secret-file"));
        assert!(!p("abc").matches("abcd"));
    }
}
