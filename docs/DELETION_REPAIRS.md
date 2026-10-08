# Existing time-range deletion repairs

8 October 2026. This repair applies to existing inclusive time-range deletion,
not the proposed pending-review lifecycle or a complete permanent Discard claim.
No installed stores or captured contents were inspected.

## Scope

The two manual deletion variants and age-policy All-mode batches now remove
orphan chunk rows only if those chunks were referenced by selected frame or
transcript rows before deletion. Unrelated orphan video and untranscribed audio
keep their file pointers. Shared chunks referenced outside the selection remain.
The retention worker no longer follows batches with a global orphan-row sweep.
This preserves selection boundaries; it does not automatically select raw audio
using an inferred timestamp or expand a range to delete neighboring content.

Eligible selected video, audio and snapshot paths are queued in an additive
`media_deletion_jobs` table within the same transaction that removes records.
Failed filesystem cleanup therefore retains durable ownership after commit or
restart. Only already-requested deletions are retried; this does not select new
content by storage pressure or enable age retention.

Immediate requests process at most 100 eligible paths and report only their
own pending/failed counts. A separate startup worker processes at most 100 jobs
per minute regardless of whether optional age retention is enabled. Last-attempt
ordering rotates blocked jobs so they cannot permanently starve newer jobs.
Each unlink holds a short immediate-write transaction, rechecks video/audio/
snapshot references, and refuses referenced or nonabsolute paths. Success or
already absent files clears the job; failure retains a fixed reason code.
Private paths remain local database data and are not returned in cleanup status.
The worker deduplicates unchanged pending/failure logs.

Manual deletion invalidates hot frames, search results and frame-image LRU
metadata even if subsequent cleanup fails. The response adds snapshot counts,
`files_failed`, `files_pending_cleanup`, `cleanup_retry_scheduled`,
`cleanup_status_unknown`, `file_cleanup_complete` and
`permanent_discard_verified`. Incomplete cleanup returns HTTP 500 with
`reason: file_cleanup_incomplete` and known committed deletion counts. Unknown
cleanup status is never presented as completed cleanup. An unrelated backlog
is not attributed to the current request.

## Validation

Canonical RootFmt and whitespace/lockfile checks passed in the isolated source
worktree. The three focused DB library regressions passed in the canonical checkout.
The first target adoption reused all 399 external artifacts, rebuilding only
two workspace artifacts; the second selection reused every artifact. DesktopCheck
and both production release builds passed for the settled candidate, satisfying
the affected-workspace build milestone without redundant release-local builds.
Independent source review corrected request/backlog attribution, committed-count
preservation and cache invalidation before cleanup failures. No live API, capture
or installed-store migration was exercised.

Synthetic tests cover all three range methods with selected and retained real
file bytes, shared chunks, unrelated orphans, surviving OCR/transcripts and
selected frame-FTS removal. Additional tests exercise queued unlink failure,
store reopen, new live-reference refusal, subsequent byte deletion, missing
files, independent request counts and bounded deferred-job completion. They use
fresh synthetic files/stores and are not live capture tests.

## Remaining limits

This is not complete permanent Discard. Selected content can remain in shared
MP4/WAV chunks, mixed or nullable-origin memories, speaker embeddings, disk
extraction caches, backups and previously delivered consumer copies. Pending-
review enforcement and a full derivative lineage/access boundary are separate.

Filesystem-only writers, including compaction before its pointer transaction,
are not coordinated by the cleanup protocol. DB-reference checks cannot prove
there is no in-flight writer or alternate path to the same file. Jobs are
conservative for known live references, but full writer generation/tombstone
coordination remains required before claiming complete Discard. No overwrite,
SSD-remnant or forensic-erasure guarantee is added here. Relative or otherwise
uncertain paths require repair/owner resolution rather than guessed deletion.

Media-only eviction remains storage reclamation preserving text; its existing
file-cleanup paths are not made complete Discard by this repair. Restored/legacy
files and historical failed deletions without stored paths cannot be discovered
by the new ledger. Automatic migration of captured contents is not authorized.