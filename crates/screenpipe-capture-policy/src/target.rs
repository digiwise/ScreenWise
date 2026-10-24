//! One answer to "may ScreenWise contact this app or window at all?"
//!
//! Every component that would call into another application (accessibility
//! tree walks, keyboard and click probes, browser URL lookups, meeting
//! detection scans) asks this first, from native identity alone: the
//! executable name and window title, read without calling into the target.
//! An exclusion means no call into that process for that window. Capture
//! admission then applies its further, evidence-based rules (URLs, private
//! windows, DRM, nested windows); those are not part of this gate.
//!
//! The gate covers:
//! - protected apps: the canonical password managers, the Windows credential
//!   viewer and system UI hosts, always excluded;
//! - windows whose title names a credential file
//!   ([`crate::credential_files`]), always excluded like an ignore match;
//! - the configured ignore list (`App::Title` patterns, see
//!   [`crate::window_pattern`]);
//! - the configured include list (an app or window outside it is not
//!   captured, so it is not contacted either).
//!
//! An unidentified app (no executable name) is excluded: the gate cannot
//! show that it is not excluded.

use crate::credential_files;
use crate::password_managers;
use crate::window_pattern::{self, WindowPattern};
use std::sync::OnceLock;

/// System UI hosts (logon/lock screen, search, notifications, pickers,
/// snipping). Not password managers; never contacted.
pub const SYSTEM_UI_APPS: &[&str] = &[
    "loginwindow",
    "logonui",
    "lockapp",
    "searchhost",
    "shellexperiencehost",
    "pickerhost",
    "snippingtool",
];

/// Terminal and console hosts: their window title is often the running
/// command line, so it is not stored as unverified metadata.
pub const TERMINAL_APPS: &[&str] = &[
    "windowsterminal",
    "openconsole",
    "conhost",
    "cmd.exe",
    "powershell",
    "pwsh",
    "wezterm",
    "alacritty",
    "mintty",
    "putty",
    "kitty",
    "terminal",
    "iterm",
];

/// `true` when the app's window title may be a command line. `app_lc` must be
/// lowercase.
pub fn title_may_carry_commands(app_lc: &str) -> bool {
    TERMINAL_APPS.iter().any(|name| app_lc.contains(name))
}

/// `true` when a window title names a password manager or credential tool.
/// Unlike on-screen text, the brand alone suffices: a title is short and names
/// what is open (a web vault in a browser has no protected process name).
/// `title_lc` must be lowercase.
pub fn title_names_password_manager(title_lc: &str) -> bool {
    password_managers::screen_brand_names()
        .iter()
        .any(|brand| title_lc.contains(brand.as_str()))
}

/// Why a target may not be contacted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exclusion {
    /// No executable name was available.
    Unidentified,
    /// A password manager, credential tool or system UI host.
    ProtectedApp,
    /// The window title names a credential file (built-in list or owner
    /// pattern). Treated like [`Exclusion::Ignored`] everywhere.
    CredentialFile,
    /// Matched a configured ignore pattern.
    Ignored,
    /// Outside a configured include list.
    OutsideIncludes,
}

/// Lowercased protected app names, matched as substrings of the lowercased
/// app name: the canonical password managers, the Windows credential viewer,
/// the macOS keychain and Linux keyrings, then [`SYSTEM_UI_APPS`].
pub fn protected_app_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        password_managers::desktop_process_names()
            .iter()
            .cloned()
            .chain([
                password_managers::CREDENTIAL_MANAGER.to_lowercase(),
                password_managers::KEYCHAIN_ACCESS.to_lowercase(),
            ])
            .chain(
                password_managers::LINUX_KEYRING_APPS
                    .iter()
                    .map(|app| app.to_string()),
            )
            .chain(SYSTEM_UI_APPS.iter().map(|app| app.to_string()))
            .collect()
    })
}

/// `true` for a protected app. `app_lc` must be lowercase.
pub fn is_protected_app(app_lc: &str) -> bool {
    protected_app_names()
        .iter()
        .any(|name| app_lc.contains(name.as_str()))
}

/// The ignore decision for one window, shared by every capture path and by
/// the re-screen of stored data: a title naming a credential file
/// ([`credential_files`]), then a configured ignore pattern. `app_lc` is the
/// lowercase app name (may be empty when unknown); `title` keeps its
/// original case (`None` when unknown).
pub fn ignore_exclusion(
    ignore: &[WindowPattern],
    app_lc: &str,
    title: Option<&str>,
) -> Option<Exclusion> {
    if title.is_some_and(credential_files::title_names_credential_file) {
        return Some(Exclusion::CredentialFile);
    }
    let title_lc = title.unwrap_or_default().to_lowercase();
    window_pattern::matches_any(ignore, app_lc, &title_lc).then_some(Exclusion::Ignored)
}

/// The configured decision alone (no protected-app check): the ignore
/// decision ([`ignore_exclusion`]), then the include list. `app_lc`
/// lowercase; `title` keeps its original case, and an unknown title
/// (`None`) matches only rules that need no title.
pub fn configured_exclusion(
    ignore: &[WindowPattern],
    include: &[WindowPattern],
    app_lc: &str,
    title: Option<&str>,
) -> Option<Exclusion> {
    if let Some(exclusion) = ignore_exclusion(ignore, app_lc, title) {
        return Some(exclusion);
    }
    let title_lc = title.unwrap_or_default().to_lowercase();
    (!window_pattern::passes_includes(include, app_lc, &title_lc))
        .then_some(Exclusion::OutsideIncludes)
}

/// Parsed ignore/include lists, for components that hold their own copy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetFilter {
    ignore: Vec<WindowPattern>,
    include: Vec<WindowPattern>,
}

impl TargetFilter {
    pub fn new(ignored: &[String], included: &[String]) -> Self {
        Self::from_patterns(
            WindowPattern::parse_list(ignored),
            WindowPattern::parse_list(included),
        )
    }

    pub fn from_patterns(ignore: Vec<WindowPattern>, include: Vec<WindowPattern>) -> Self {
        Self { ignore, include }
    }

    /// Why a window may not be contacted, or `None` when it may. `app_names`
    /// are every name the window's process is known by (for example the raw
    /// executable and a normalised host name); any excluded name excludes it.
    pub fn window_exclusion(&self, app_names: &[&str], title: Option<&str>) -> Option<Exclusion> {
        let mut identified = false;
        for app in app_names {
            let app_lc = app.trim().to_lowercase();
            if app_lc.is_empty() {
                continue;
            }
            identified = true;
            if let Some(exclusion) = self.identity_exclusion(&app_lc, title) {
                return Some(exclusion);
            }
        }
        (!identified).then_some(Exclusion::Unidentified)
    }

    /// The exclusion of one known identity, in the fixed order: protected
    /// app, credential-file title, ignore pattern, include list. `app_lc`
    /// lowercase.
    fn identity_exclusion(&self, app_lc: &str, title: Option<&str>) -> Option<Exclusion> {
        if is_protected_app(app_lc) {
            return Some(Exclusion::ProtectedApp);
        }
        configured_exclusion(&self.ignore, &self.include, app_lc, title)
    }

    /// [`Self::window_exclusion`] for a stored record whose app name may be
    /// missing: an unknown app is not itself a reason (the record exists),
    /// but every title rule still applies.
    pub fn recorded_exclusion(&self, app: Option<&str>, title: Option<&str>) -> Option<Exclusion> {
        let app_lc = app.unwrap_or_default().trim().to_lowercase();
        self.identity_exclusion(&app_lc, title)
    }

    /// `true` when a window may be contacted.
    pub fn allows_window(&self, app_names: &[&str], title: Option<&str>) -> bool {
        self.window_exclusion(app_names, title).is_none()
    }

    /// `true` when every window of the app is excluded whatever its title:
    /// a protected app, or an ignore rule that needs no title. A `false`
    /// answer still requires [`Self::allows_window`] per window.
    pub fn excludes_whole_app(&self, app_name: &str) -> bool {
        let app_lc = app_name.trim().to_lowercase();
        app_lc.is_empty()
            || is_protected_app(&app_lc)
            || window_pattern::matches_any(&self.ignore, &app_lc, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(ignored: &[&str], included: &[&str]) -> TargetFilter {
        let owned = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        TargetFilter::new(&owned(ignored), &owned(included))
    }

    #[test]
    fn protected_apps_are_never_contacted() {
        let open = filter(&[], &[]);
        for app in [
            "1Password.exe",
            "LogonUI.exe",
            "SnippingTool.exe",
            "KeePassXC",
        ] {
            assert_eq!(
                open.window_exclusion(&[app], Some("anything")),
                Some(Exclusion::ProtectedApp),
                "{app}"
            );
            assert!(open.excludes_whole_app(app), "{app}");
        }
        assert!(open.allows_window(&["notepad.exe"], Some("notes")));
    }

    #[test]
    fn app_scoped_ignore_excludes_every_window_of_the_app() {
        let f = filter(&["excel::", "firefox::"], &[]);
        assert_eq!(
            f.window_exclusion(&["EXCEL.EXE"], Some("Budget")),
            Some(Exclusion::Ignored)
        );
        assert_eq!(
            f.window_exclusion(&["firefox.exe"], None),
            Some(Exclusion::Ignored)
        );
        assert!(f.excludes_whole_app("firefox.exe"));
        assert!(f.allows_window(&["msedge.exe"], Some("Budget")));
    }

    #[test]
    fn title_scoped_ignore_excludes_only_matching_windows() {
        let f = filter(&["Slack::#hr", "::confidential"], &[]);
        assert!(!f.excludes_whole_app("slack.exe"));
        assert!(f.allows_window(&["slack.exe"], Some("#general")));
        assert!(!f.allows_window(&["slack.exe"], Some("#hr - Slack")));
        assert!(!f.allows_window(&["code.exe"], Some("Confidential plan")));
    }

    #[test]
    fn include_list_excludes_other_apps() {
        let f = filter(&[], &["Code"]);
        assert!(f.allows_window(&["Code.exe"], Some("main.rs")));
        assert_eq!(
            f.window_exclusion(&["teams.exe"], Some("Meeting")),
            Some(Exclusion::OutsideIncludes)
        );
        // An include list alone never excludes a whole app by name: titles decide.
        assert!(!f.excludes_whole_app("teams.exe"));
    }

    #[test]
    fn any_known_name_can_exclude_and_unknown_identity_is_excluded() {
        let f = filter(&["msedge::"], &[]);
        assert_eq!(
            f.window_exclusion(&["explorer.exe", "msedge.exe"], Some("Widgets")),
            Some(Exclusion::Ignored)
        );
        assert_eq!(
            f.window_exclusion(&[], Some("x")),
            Some(Exclusion::Unidentified)
        );
        assert_eq!(
            f.window_exclusion(&["  "], Some("x")),
            Some(Exclusion::Unidentified)
        );
        assert!(f.excludes_whole_app(""));
    }

    #[test]
    fn credential_file_titles_are_excluded_without_any_configured_rule() {
        let open = filter(&[], &[]);
        assert_eq!(
            open.window_exclusion(&["notepad.exe"], Some("*deploy-token.secret - Notepad")),
            Some(Exclusion::CredentialFile)
        );
        assert_eq!(
            open.window_exclusion(&["code.exe"], Some(".env - repo - Visual Studio Code")),
            Some(Exclusion::CredentialFile)
        );
        // Protected apps keep their own reason; ordinary titles pass.
        assert_eq!(
            open.window_exclusion(&["1password.exe"], Some("id_rsa")),
            Some(Exclusion::ProtectedApp)
        );
        assert!(open.allows_window(&["notepad.exe"], Some("Secret Santa list - Notepad")));
        assert!(open.allows_window(&["notepad.exe"], None));
    }

    #[test]
    fn recorded_identities_use_the_same_order_without_requiring_an_app() {
        let f = filter(&["::confidential"], &[]);
        assert_eq!(
            f.recorded_exclusion(None, Some("id_rsa - Notepad")),
            Some(Exclusion::CredentialFile)
        );
        assert_eq!(
            f.recorded_exclusion(Some("1Password.exe"), Some("id_rsa")),
            Some(Exclusion::ProtectedApp)
        );
        assert_eq!(
            f.recorded_exclusion(None, Some("Confidential plan")),
            Some(Exclusion::Ignored)
        );
        assert_eq!(f.recorded_exclusion(None, None), None);
        assert_eq!(
            ignore_exclusion(&[], "notepad.exe", Some("*deploy-token.secret - Notepad")),
            Some(Exclusion::CredentialFile)
        );
        assert_eq!(
            ignore_exclusion(&[], "notepad.exe", Some("notes.txt")),
            None
        );
    }

    #[test]
    fn terminal_titles_are_recognised() {
        for app in [
            "WindowsTerminal.exe",
            "pwsh.exe",
            "cmd.exe",
            "OpenConsole.exe",
        ] {
            assert!(title_may_carry_commands(&app.to_lowercase()), "{app}");
        }
        for app in ["msedge.exe", "excel.exe", "code.exe", "digitrack.app.exe"] {
            assert!(!title_may_carry_commands(app), "{app}");
        }
    }

    #[test]
    fn password_manager_brands_in_titles_are_recognised() {
        assert!(title_names_password_manager("1password - sign in"));
        assert!(title_names_password_manager(
            "my vault | bitwarden web vault"
        ));
        assert!(!title_names_password_manager("inbox - gmail"));
    }

    #[test]
    fn protected_names_cover_the_former_walker_list() {
        for name in password_managers::desktop_process_names() {
            assert!(protected_app_names().contains(name));
        }
        assert!(protected_app_names()
            .iter()
            .any(|n| n == &password_managers::CREDENTIAL_MANAGER.to_lowercase()));
    }
}
