//! Content screening of input events (`ui_events` rows), shared by the
//! recorder's write path and `screenpipe privacy rescreen`: the rules taken
//! from the owner's settings, and the mapping of an event's text columns to
//! and from the field order of the decision itself,
//! `screenpipe_redact::rescreen::decide_ui_event`.

use serde::Serialize;

use screenpipe_redact::rescreen::{
    decide_ui_event, RescreenReason, RescreenRules, StoredUiEvent, UiEventDecision,
};

/// The owner's current screening rules as plain lists (also hashed into a
/// rescreen plan).
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct RuleInputs {
    pub ignored_windows: Vec<String>,
    pub ignored_urls: Vec<String>,
    pub keywords: Vec<String>,
    pub protected_domains: bool,
    pub credential_file_patterns: Vec<String>,
}

impl RuleInputs {
    pub fn from_settings(settings: &screenpipe_config::RecordingSettings) -> Self {
        Self {
            ignored_windows: settings.ignored_windows.clone(),
            ignored_urls: settings.ignored_urls.clone(),
            keywords: settings.content_screening_keywords.clone(),
            protected_domains: settings.pause_on_drm_content,
            credential_file_patterns: settings.credential_file_patterns.clone(),
        }
    }

    /// Install the owner credential-file patterns and build the rules.
    pub fn rules(&self) -> RescreenRules {
        screenpipe_capture_policy::credential_files::set_owner_patterns(
            &self.credential_file_patterns,
        );
        RescreenRules::new(
            &self.ignored_windows,
            &self.ignored_urls,
            &self.keywords,
            self.protected_domains,
        )
    }
}

/// The screened text columns of one input event.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputEventTexts<T> {
    pub text_content: Option<T>,
    pub window_title: Option<T>,
    pub browser_url: Option<T>,
    pub element_name: Option<T>,
    pub element_value: Option<T>,
    pub element_description: Option<T>,
    pub text_before: Option<T>,
}

/// What screening does to one input event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputScreening {
    Keep,
    Delete(RescreenReason),
    /// Replace the columns that are `Some` with their redacted text.
    Redact(InputEventTexts<String>),
}

impl InputEventTexts<&str> {
    /// Screen the event's texts under `rules` (window rules use the app
    /// name, title and URL as recorded).
    pub fn screen(&self, app_name: Option<&str>, rules: &RescreenRules) -> InputScreening {
        // Pointer events without context carry nothing to screen.
        if app_name.is_none() && *self == Self::default() {
            return InputScreening::Keep;
        }
        let stored = StoredUiEvent {
            app_name,
            window_title: self.window_title,
            browser_url: self.browser_url,
            fields: [
                self.text_content,
                self.window_title,
                self.browser_url,
                self.element_name,
                self.element_value,
                self.element_description,
                self.text_before,
            ],
        };
        match decide_ui_event(&stored, rules) {
            UiEventDecision::Keep => InputScreening::Keep,
            UiEventDecision::Delete(reason) => InputScreening::Delete(reason),
            UiEventDecision::Redact(fields) => {
                let [text_content, window_title, browser_url, element_name, element_value, element_description, text_before] =
                    *fields;
                InputScreening::Redact(InputEventTexts {
                    text_content,
                    window_title,
                    browser_url,
                    element_name,
                    element_value,
                    element_description,
                    text_before,
                })
            }
        }
    }
}

/// Screen an input event about to be stored: `Err` when it must not be
/// stored, otherwise redacted in place. Called on the event exactly as
/// captured, before Basic PII removal.
pub fn screen_insert(
    event: &mut screenpipe_db::InsertUiEvent,
    rules: &RescreenRules,
) -> Result<(), RescreenReason> {
    let texts = InputEventTexts {
        text_content: event.text_content.as_deref(),
        window_title: event.window_title.as_deref(),
        browser_url: event.browser_url.as_deref(),
        element_name: event.element_name.as_deref(),
        element_value: event.element_value.as_deref(),
        element_description: event.element_description.as_deref(),
        text_before: event.text_before.as_deref(),
    };
    match texts.screen(event.app_name.as_deref(), rules) {
        InputScreening::Keep => Ok(()),
        InputScreening::Delete(reason) => Err(reason),
        InputScreening::Redact(redacted) => {
            let replace = |column: &mut Option<String>, value: Option<String>| {
                if value.is_some() {
                    *column = value;
                }
            };
            replace(&mut event.text_content, redacted.text_content);
            replace(&mut event.window_title, redacted.window_title);
            replace(&mut event.browser_url, redacted.browser_url);
            replace(&mut event.element_name, redacted.element_name);
            replace(&mut event.element_value, redacted.element_value);
            replace(&mut event.element_description, redacted.element_description);
            replace(&mut event.text_before, redacted.text_before);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> RescreenRules {
        RescreenRules::new(&[], &[], &["falcon".to_string()], false)
    }

    fn insert(text: &str, before: Option<&str>) -> screenpipe_db::InsertUiEvent {
        let mut event = screenpipe_a11y::UiEvent::text(chrono::Utc::now(), 0, text.to_string())
            .to_db_insert(None);
        event.event_type = screenpipe_db::UiEventType::TextEdit;
        event.app_name = Some("notepad.exe".to_string());
        event.window_title = Some("notes.txt - Notepad".to_string());
        event.text_before = before.map(str::to_string);
        event
    }

    #[test]
    fn screening_keeps_redacts_or_refuses_before_storage() {
        let rules = rules();
        let mut plain = insert("hello", Some("hell"));
        assert_eq!(screen_insert(&mut plain, &rules), Ok(()));
        assert_eq!(plain.text_before.as_deref(), Some("hell"));

        let mut keyword = insert("about project falcon", None);
        assert!(screen_insert(&mut keyword, &rules).is_err());

        // A synthetic high-entropy token, held only by the "before" text.
        let key = "Xq7Lm2Pz9Rt4Vb8Nc1Hd6Jf3Kg5Ws0Ya2Ue";
        let before = format!("api token {key}");
        let mut secret = insert("api token removed", Some(before.as_str()));
        assert_eq!(screen_insert(&mut secret, &rules), Ok(()));
        let stored = secret.text_before.unwrap_or_default();
        assert!(!stored.is_empty() && !stored.contains(key));
        assert_eq!(secret.text_content.as_deref(), Some("api token removed"));
    }
}
