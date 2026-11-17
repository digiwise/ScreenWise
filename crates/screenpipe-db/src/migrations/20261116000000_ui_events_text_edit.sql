-- Entered-text capture (`text_edit` input events): the text at the caret
-- before an edit (`text_content` holds the text after it), and fixed-code
-- details of the edit as a small JSON object (how it ended, the text source,
-- truncation). Both are null for every other event type.
ALTER TABLE ui_events ADD COLUMN text_before TEXT;
ALTER TABLE ui_events ADD COLUMN edit_detail TEXT;
-- When the background redaction worker last rewrote `text_before`.
ALTER TABLE ui_events ADD COLUMN text_before_redacted_at INTEGER;
