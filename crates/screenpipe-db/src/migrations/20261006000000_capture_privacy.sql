-- Capture decision and blocker disclosure metadata. Legacy rows remain NULL.
ALTER TABLE frames ADD COLUMN capture_privacy TEXT DEFAULT NULL;
