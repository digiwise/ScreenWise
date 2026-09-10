-- Preserve local machine identity for record provenance and filtering.

ALTER TABLE frames ADD COLUMN machine_id TEXT;
ALTER TABLE audio_chunks ADD COLUMN machine_id TEXT;
ALTER TABLE video_chunks ADD COLUMN machine_id TEXT;

-- Index for local machine filtering.
CREATE INDEX IF NOT EXISTS idx_frames_machine_id ON frames(machine_id);
