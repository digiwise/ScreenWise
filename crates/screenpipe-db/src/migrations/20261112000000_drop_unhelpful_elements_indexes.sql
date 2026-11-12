-- Drop elements indexes that no query needs or that mislead the planner.
--
-- Measured on a 44 GB local store (EXPLAIN QUERY PLAN for every elements query
-- in the code, no planner statistics):
--   idx_elements_frame_id           prefix of idx_elements_frame_source, which
--                                   serves the same lookups and FK checks.
--   idx_elements_frame_source_role  never chosen.
--   idx_elements_source             `source` has two values; time-ranged
--                                   searches walked every row of one source
--                                   through it instead of the frame range.
--   idx_elements_source_role_text   the activity summary walked every
--                                   accessibility row ever stored through it
--                                   instead of that day's frames.
-- Without them those queries start from idx_frames_timestamp and fetch each
-- frame's elements through idx_elements_frame_source (or idx_elements_on_screen
-- for on-screen filters). Together they held about
-- 9.5 GB; the pages return to the database free list for reuse.
-- idx_elements_parent_id stays: it backs the parent_id foreign-key check on
-- deletes.
DROP INDEX IF EXISTS idx_elements_frame_id;
DROP INDEX IF EXISTS idx_elements_frame_source_role;
DROP INDEX IF EXISTS idx_elements_source;
DROP INDEX IF EXISTS idx_elements_source_role_text;
