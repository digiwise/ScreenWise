//! Windows toast identity for the keystroke drop alerts.
//!
//! The notification plugin raises toasts under the app identifier as their
//! AppUserModelID. An unpackaged executable (such as the deployed
//! `release-local` build) has no installer shortcut registering that ID, and
//! Windows then discards its toasts without an error. Registering the ID for
//! the current user, with a display name, lets Windows show them.

use std::io;
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

const CLASSES: &str = r"Software\Classes";
/// Name in the toast header and in Windows notification settings.
const DISPLAY_NAME: &str = "ScreenWise";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastIdentity {
    /// Registered by this call.
    Registered,
    /// Already registered (an installer or an earlier run); left unchanged.
    AlreadyRegistered,
}

/// Ensure `app_id` is registered as an AppUserModelID for the current user.
/// An existing display name is never changed.
pub fn ensure_toast_identity(app_id: &str) -> io::Result<ToastIdentity> {
    ensure_under(&RegKey::predef(HKEY_CURRENT_USER), CLASSES, app_id)
}

fn ensure_under(root: &RegKey, classes: &str, app_id: &str) -> io::Result<ToastIdentity> {
    // One plain key name: no separators that would address another key.
    if app_id.is_empty() || app_id.contains(['\\', '/']) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid app identifier",
        ));
    }
    let path = format!(r"{classes}\AppUserModelId\{app_id}");
    if let Ok(existing) = root.open_subkey(&path) {
        if existing.get_value::<String, _>("DisplayName").is_ok() {
            return Ok(ToastIdentity::AlreadyRegistered);
        }
    }
    let (key, _) = root.create_subkey(&path)?;
    key.set_value("DisplayName", &DISPLAY_NAME)?;
    Ok(ToastIdentity::Registered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// A private per-run key directly under the current user's `Software`,
    /// removed with everything below it on drop.
    struct Scratch {
        root: RegKey,
        path: String,
    }

    impl Scratch {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            Self {
                root: RegKey::predef(HKEY_CURRENT_USER),
                path: format!(
                    r"Software\ScreenWiseTest-toast-identity-{}-{stamp}",
                    std::process::id()
                ),
            }
        }

        fn display_name(&self, app_id: &str) -> String {
            self.root
                .open_subkey(format!(r"{}\AppUserModelId\{app_id}", self.path))
                .unwrap()
                .get_value("DisplayName")
                .unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = self.root.delete_subkey_all(&self.path);
        }
    }

    #[test]
    fn registers_once_and_never_changes_an_existing_registration() {
        let scratch = Scratch::new();
        let ensure = |app_id| ensure_under(&scratch.root, &scratch.path, app_id).unwrap();
        assert_eq!(ensure("screenpi.pe.test"), ToastIdentity::Registered);
        assert_eq!(scratch.display_name("screenpi.pe.test"), DISPLAY_NAME);
        assert_eq!(ensure("screenpi.pe.test"), ToastIdentity::AlreadyRegistered);
        // An installer's (or anyone else's) display name is kept.
        let (other, _) = scratch
            .root
            .create_subkey(format!(r"{}\AppUserModelId\other.app", scratch.path))
            .unwrap();
        other.set_value("DisplayName", &"Other").unwrap();
        assert_eq!(ensure("other.app"), ToastIdentity::AlreadyRegistered);
        assert_eq!(scratch.display_name("other.app"), "Other");
    }

    #[test]
    fn rejects_identifiers_that_would_address_another_key() {
        let scratch = Scratch::new();
        for app_id in ["", r"a\b", "a/b", r"..\x"] {
            let error = ensure_under(&scratch.root, &scratch.path, app_id).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{app_id:?}");
        }
        assert!(scratch.root.open_subkey(&scratch.path).is_err());
    }
}
