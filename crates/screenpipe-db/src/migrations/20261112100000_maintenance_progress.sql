-- Progress of one-off background maintenance tasks, so a finished task is not
-- repeated on every launch and an interrupted one resumes where it stopped.
CREATE TABLE IF NOT EXISTS maintenance_progress (
    task TEXT PRIMARY KEY,
    last_id INTEGER NOT NULL DEFAULT 0,
    finished_at TEXT
);
