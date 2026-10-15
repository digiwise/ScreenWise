//! Canonical password-manager and credential-tool identities.
//!
//! One table feeds every caller that needs to recognise these tools:
//!
//! - app/process exclusion in the accessibility layer
//!   ([`desktop_app_display_names`], [`desktop_process_names`]);
//! - on-screen brand screening in `screenpipe-redact` ([`screen_brand_names`]).
//!
//! Each caller keeps its own platform extras (system UI processes, Linux
//! keyrings, window managers) next to its own code and composes them with
//! these lists. Matching is always case-insensitive substring/whole-word on
//! the lowercased name; the display form only exists because the accessibility
//! config exposes a user-visible default list in its original casing.

use std::sync::OnceLock;

struct Identity {
    /// Original-case name, e.g. `1Password`.
    display: &'static str,
    /// Whether the desktop app is excluded outright by process/app name.
    desktop_excluded: bool,
}

/// Canonical identities. Callers only any-match, so the overall order is not
/// significant; the desktop-excluded subset keeps its historical order
/// `1Password, Bitwarden, LastPass, Dashlane, KeePassXC`.
const IDENTITIES: &[Identity] = &[
    Identity {
        display: "1Password",
        desktop_excluded: true,
    },
    Identity {
        display: "Bitwarden",
        desktop_excluded: true,
    },
    Identity {
        display: "LastPass",
        desktop_excluded: true,
    },
    Identity {
        display: "Dashlane",
        desktop_excluded: true,
    },
    Identity {
        display: "KeePassXC",
        desktop_excluded: true,
    },
    Identity {
        display: "Keeper",
        desktop_excluded: false,
    },
    Identity {
        display: "NordPass",
        desktop_excluded: false,
    },
    Identity {
        display: "Proton Pass",
        desktop_excluded: false,
    },
    Identity {
        display: "KeePass",
        desktop_excluded: false,
    },
    Identity {
        display: "RoboForm",
        desktop_excluded: false,
    },
    Identity {
        display: "Enpass",
        desktop_excluded: false,
    },
    Identity {
        display: "Google Password Manager",
        desktop_excluded: false,
    },
    Identity {
        display: "Microsoft Password Manager",
        desktop_excluded: false,
    },
    Identity {
        display: "Edge Password Manager",
        desktop_excluded: false,
    },
];

/// macOS credential viewer excluded alongside the password managers.
pub const KEYCHAIN_ACCESS: &str = "Keychain Access";

/// Windows credential viewer excluded alongside the password managers.
pub const CREDENTIAL_MANAGER: &str = "Credential Manager";

/// Linux keyring front-ends (lowercase process/app names).
pub const LINUX_KEYRING_APPS: &[&str] = &["gnome-keyring", "seahorse", "kwalletmanager"];

/// Original-case names of the password managers excluded by app name, in
/// historical order.
pub fn desktop_app_display_names() -> impl Iterator<Item = &'static str> {
    IDENTITIES
        .iter()
        .filter(|i| i.desktop_excluded)
        .map(|i| i.display)
}

/// Lowercased names of the password managers excluded by app/process name, in
/// historical order, ready for `app_lower.contains(name)`.
pub fn desktop_process_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| desktop_app_display_names().map(str::to_lowercase).collect())
}

/// Lowercased brand names recognised in on-screen text (a superset of the
/// desktop-excluded set: it also lists managers that are only screened, not
/// excluded by process name).
pub fn screen_brand_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        IDENTITIES
            .iter()
            .map(|i| i.display.to_lowercase())
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_display_names_keep_historical_order_and_case() {
        assert_eq!(
            desktop_app_display_names().collect::<Vec<_>>(),
            [
                "1Password",
                "Bitwarden",
                "LastPass",
                "Dashlane",
                "KeePassXC"
            ]
        );
    }

    #[test]
    fn desktop_process_names_are_lowercase_display_names() {
        assert_eq!(
            desktop_process_names(),
            [
                "1password",
                "bitwarden",
                "lastpass",
                "dashlane",
                "keepassxc"
            ]
        );
    }

    #[test]
    fn screen_brands_are_the_historical_screening_set() {
        let mut actual: Vec<&str> = screen_brand_names().iter().map(String::as_str).collect();
        actual.sort_unstable();
        let mut expected = vec![
            "bitwarden",
            "1password",
            "lastpass",
            "dashlane",
            "keeper",
            "nordpass",
            "proton pass",
            "keepass",
            "keepassxc",
            "roboform",
            "enpass",
            "google password manager",
            "microsoft password manager",
            "edge password manager",
        ];
        expected.sort_unstable();
        assert_eq!(actual, expected);
    }

    #[test]
    fn every_desktop_excluded_manager_is_also_a_screening_brand() {
        for name in desktop_process_names() {
            assert!(screen_brand_names().contains(name), "{name}");
        }
    }
}
