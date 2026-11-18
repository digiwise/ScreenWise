-- Entered-text capture (`text_edit` input events): when the edit's first key
-- was seen. `timestamp` is when its last key, or the Enter that ended it, was
-- seen. Null for every other event type, and for edits stored before it.
ALTER TABLE ui_events ADD COLUMN started_at DATETIME;
