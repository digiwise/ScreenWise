-- Keep the frame-first lookup index used by accessibility element queries.
CREATE INDEX IF NOT EXISTS idx_elements_frame_source_role
    ON elements(frame_id, source, role) WHERE text IS NOT NULL;
