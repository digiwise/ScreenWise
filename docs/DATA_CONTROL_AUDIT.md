# Captured data lineage and control audit

Source audit, 8 October 2026, for step 2 of
[DATA_CONTROL_DISCUSSION.md](DATA_CONTROL_DISCUSSION.md). This documents
implementation foundations and gaps; it does not implement retention policy or
settle the open decisions in that discussion. ScreenWise is an unsupported
developer fork; this is not privacy/security certification.

## Scope and revision

Inspected branch `screenwise`, starting HEAD
`49d9bb3009ecfbae600a71ec036223642037b397`. References below use repository-relative
paths and one-based source line numbers in the inspected working tree. Migrations
describe the intended current schema, not verification of any installed store.

The index was empty on entry. The working tree already contained concurrent
diagnostics/microphone-policy changes in `BUILD_NOTES.md`, `VALIDATION_REGISTER.md`,
the rewind notice component/test, desktop `recording.rs`, Windows UIA, audio
manager/device/stream/transcription/meeting-streaming code, paired capture,
`audio_privacy.rs`, config exports, engine privacy/diagnostic/capture/power/route
code, monitor watcher, Windows screenshot code and `docs/CAPTURE_PRIVACY.md`.
`capture_diagnostics.rs`, `CAPTURE_DIAGNOSTICS.md` and the discussion document were
untracked. They belong to the other chat. This audit incorporates the visible
working-tree behavior and does not claim it is committed or validated.

Only source and sanitized documentation were inspected, entirely within this
fork. No captured contents, private evidence, live API/store, recording, builds,
firewall, deployment or publication were used. Two independent read-only source
reviews covered storage/deletion and exposure paths; their findings were reconciled
into this document. Only this new document is owned by this audit.

Final reconciliation kept HEAD unchanged and the same concurrent changed-path
set, but source contents continued changing during the audit. In particular the
final `AudioDevice::diagnostic_id` uses a process-local bounded ordinal registry,
not a stable hash. Final source anchors were checked against that implementation;
it does not change persisted audio lineage. Diagnostics/manual pause additions
were inspected without interpreting them as retention approval. The index was
rechecked before staging this document; neither Rust lockfile was changed.
This is a working-tree source snapshot, not a claim that concurrent work has
reached its final validated state.

## Conclusions that affect the next design

1. There is no persisted pending-review lifecycle or common content-release gate.
   Capture privacy permits and PII masking are useful foundations, but neither
   means approval to keep or expose already captured data.
2. Time-range deletion removes many database records, but is not complete Discard:
   shared chunks can retain selected bytes, memories and speaker embeddings
   survive, and filesystem failures can leave files after successful responses.
3. Media eviction deliberately preserves text and derivatives. It is storage
   reclamation and must stay visibly distinct from Discard.
4. Review enforcement must reach raw SQL, direct media/assets, live events,
   exports, derived records and persistent consumer caches as well as search.
   Bearer authentication alone cannot express approval state.
5. A monitor identifies acquisition origin for frames. It does not establish
   exclusive ownership of window/tree/input/audio content. Unknown and multiple
   attribution need explicit representation rather than inferred membership.

## Data coverage matrix

`db` below means `crates/screenpipe-db/src/db.rs`; `migrations/` means
`crates/screenpipe-db/src/migrations/`. References are source anchors for review,
not runtime evidence.

| Data type | Acquisition, storage and verified relationships | Existing removal/reclamation | Gaps and required contract |
|---|---|---|---|
| Screen snapshots | Paired capture writes a monitor-origin JPEG, then inserts frame metadata and OCR atomically in SQLite; filesystem write is separate. `crates/screenpipe-capture/src/paired_capture.rs:139,362`; frames retain snapshot path, device, app/window/URL/document context, text/tree/hash/privacy metadata (`db:2874`). | Range deletion selects frame timestamps and collects snapshot paths (`db:6314` onward); media eviction clears media pointers but retains frame/text rows (`db:6745` onward). | Bind lifecycle to durable frame/source identity and file ownership. Handle failed insertion/orphan snapshots and in-flight writes. Snapshot path access must check release state. |
| Video, including compacted snapshots | Frames reference `video_chunks` and offsets; compaction groups old snapshots by `device_name`, writes shared MP4 and updates frame pointers (`crates/screenpipe-engine/src/snapshot_compaction.rs:132,181,337`). | A chunk is eligible for range unlink only when no outside-range frame survives (`db:6326`). Media eviction selects chunks through frame timestamps (`db:6763`). | Partial interval/type discard cannot remove selected bytes from retained MP4. Specify split/rewrite, coarser reviewed boundary, or another explicit strategy; do not promise byte deletion from row removal. Coordinate compaction and discard. |
| OCR text and positions/elements | OCR derives from screenshot. Paired capture can use OCR fallback or hybrid OCR/UIA; `text_source` distinguishes them (`paired_capture.rs:283`). `ocr_text`, frame `full_text` and structured `elements(source='ocr')` are related copies (`db:2408`; `migrations/20260301000000_create_elements_table.sql:2`). | Range deletion explicitly removes OCR and selected frame elements (`db:6367,6431`). Media mode keeps them. | Fine-grained Keep OCR / Discard image must disclose remaining content. Gate full text, JSON positions, element search and FTS; avoid treating OCR-only as independent acquisition. |
| Accessibility text, tree and elements | Frame text/tree plus `elements(source='accessibility')` with roles, text, bounds, hierarchy, properties/on-screen fields (`db:2604,2786,4612`). Deduplicated frames can reference another frame's elements (`migrations/20260318000000_add_elements_ref_frame_id.sql:4`). | Range deletion moves shared elements to a surviving anchor and rewrites references before deleting source frame (`db:6375-6425`); media mode preserves text/tree/elements. | Shared content has multiple lineage dependents. Structure can contain names, values, identifiers and other sensitive properties; structure-only retention needs field-level design. A kept anchor must not release pending/discarded content through a reference. |
| Microphone audio | Audio device has input/output kind. Raw chunks are persisted even without successful transcription; audio manager passes privacy permit at persistence (`crates/screenpipe-audio/src/audio_manager/manager.rs:1413`). Transcription persistence links path, device name/kind, speaker and sample segment times (`crates/screenpipe-audio/src/transcription/transcription_result.rs:156`). | Range/file selection currently derives from transcription timestamps, not all raw chunk acquisition times (`db:6350,6778`). | No-transcript chunks can escape selection; orphan cleanup can erase DB path without unlinking file. Need raw-acquisition identity, extent and ownership independent of transcript success. Device names/session diagnostic IDs are not a verified durable hardware identity contract. |
| Computer audio output | Same audio storage, distinguished by output device kind; playback can be mixed. The current concurrent policy separates microphone permits from visual/output protection (`crates/screenpipe-config/src/audio_privacy.rs`; `crates/screenpipe-audio/src/core/device.rs:91`). | Same chunk/transcript deletion and media eviction as microphone. | No verified per-app sample lineage or monitor ownership. Do not implement app-specific or monitor-specific output discard by inferring foreground context. Preserve independent source selection. |
| Ordinary audio transcripts | `audio_transcriptions` references chunk; device/kind, capture timestamp, segment start/end, speaker and diarization provenance are available (`transcription_result.rs:137-175`; `crates/screenpipe-db/src/types.rs:155,335`). | Range deletes transcript timestamp rows, cleans orphan chunks; media eviction keeps transcripts. FTS derives from transcript rows. | Timestamp membership differs from sample overlap. Several segment rows can share one file. Need explicit clipping/boundary semantics, complete derivative closure and delayed-transcription rejection after discard. Time overlap with a meeting is not verified membership. |
| Live meeting transcripts | Separate `meeting_transcript_segments`: meeting FK, provider/item, device name/type, captured_at, transcript; no audio chunk FK (`migrations/20260514000000_create_meeting_transcript_segments.sql:9`; `crates/screenpipe-audio/src/meeting_streaming/controller.rs:856`). | Selected captured_at rows are deleted; meeting deletion cascades segments (`db:6483`). Existing privacy compensation removes an invalidated newly inserted segment (`controller.rs:870`). | Cannot assume ordinary transcript filtering/deletion covers these. Need verified linkage to audio sample extents or explicitly unknown lineage; partial/final event gate and discard-future generation protection. |
| Keyboard text, keys, shortcuts | UI events persist event type, key/modifier, typed text/length, session/time and context (`crates/screenpipe-db/src/types.rs:641`; `crates/screenpipe-a11y/src/events.rs:680`). | Time-range UI event deletion (`db:6476`); media mode preserves input. | No persisted monitor field. Frame ID is optional, and conversion preserves it rather than proving attribution (`events.rs:896`). Keys, text, shortcuts and context need explicit fine-grained field/type scopes and uncertainty. |
| Mouse clicks, motion, scroll | UI events contain coordinates/deltas/button, optional element context and app/window context (`types.rs:641`). | Same UI-event range deletion. | Coordinates can support a sampled hit-test relationship, not continuous window/monitor ownership. No durable multi-monitor source relation in schema. Discard matching must use verified event context. |
| Clipboard content and operations | Clipboard operation/content capture and persistence gates are distinct; an operation may trigger screen capture without a stored clipboard row (`crates/screenpipe-engine/src/ui_recorder.rs:119,145,208,989`). | Stored clipboard rows deleted with UI events. Trigger-generated frames remain separately selected. | Default source/group Discard must traverse triggered captures where intended, not only clipboard rows. Operation metadata, text and related element context need explicit scope. No implemented generic causal FK from trigger to all resulting frames. |
| App/window activity and context | App switch/window focus events contain identity/PID, titles/URLs and element context; frames also carry app/window/URL/document metadata (`types.rs:641`; `paired_capture.rs:368`). | UI-event/frame removal by independent timestamp selections. | Content-bearing titles/URLs/property values need separate selection. Event history does not establish continuous attention/browser history. No automatic monitor inference from stale focus. |
| Meetings/conversations | `meetings` stores start/end/app/title/attendees/detection source, later note/end-reason fields (`migrations/20260225000000_create_meetings.sql:1`; `20260320000000_add_note_to_meetings.sql`; `20260528090000_add_end_reason_to_meetings.sql`). | Meeting removal uses end timestamp in selected range (`db:6490`), not interval overlap. Open meetings persist; cascade can remove segments outside selected interval. | Conversation boundary estimates and future remainder discard require reviewed membership and a live exclusion marker. Preserve partial-meeting metadata only by explicit policy; title/attendees/note are content. |
| Speakers, embeddings, diarization and identity evidence | Durable speaker/embedding records; chunk-owned runs/segments, speaker associations and audio identity evidence (`migrations/20241108202826_create_speaker_table.sql:2`; `20260515000000_audio_diarization_tables.sql:11`). | Chunk deletion cascades runs/segments and chunk-linked evidence; segment evidence may SET NULL. Range deletion does not erase durable speakers/embeddings. Media mode preserves them. | Embeddings lack complete per-source contribution lineage. Default Discard must remove/regenerate derived identities where appropriate; durable user labels versus derived voice data requires a decision. |
| Memories and summaries | Memories persist content/source_context/tags with nullable single frame origin (`db:10081`; `migrations/20260315000000_add_frame_id_to_memories.sql:2`). Activity summaries are computed from captured records; exported/chat summaries may be separate artifacts. | Frame deletion sets memory frame_id NULL; memories survive until separate delete (`db:10161`). | Single nullable origin/free JSON cannot enforce mixed-source closure. Need many-source provenance and removal/regeneration rules. Do not count all user-authored memories as captured derivatives without evidence. |
| Search indexes/tags | Current frames/audio/elements/UI events/memories FTS; delete/update triggers synchronize supported records. Tags are global vocabulary plus frame/audio associations. `migrations/20260415000000_frames_fts_external_content.sql`; `20260301000000_create_elements_table.sql:27`; `20260310000000_create_memories.sql:18`. | Logical deletion triggers maintain indexes; memory index survives with memory. Legacy chunked-text indexes and OCR embeddings were dropped (`20260311000000_drop_unused_tables.sql:7-15`); old accessibility table dropped (`20260312000000_consolidate_search_to_frames_full_text.sql:113-117`). | Gate snippets, suggestions, counts, tag/context lists and indexes as content-derived observations; test migration/rebuild consistency. Old schemas are migration cases, not current duplicate stores. |
| Caches and generated artifacts | Hot frame cache, disk frame/video cache, browser timeline/search caches, exported media/backups and configured local model consumers hold copies. See exposure matrix below. | Manual range handlers invalidate hot frame cache; retention worker does not (`crates/screenpipe-engine/src/routes/data.rs:96,218`; `retention.rs:342-470`). | Need generation/tombstone invalidation across every copy and in-flight request. Previously delivered external copies cannot be revoked solely by changing server reads. |

## Attribution limits

Frame `device_name` and acquisition monitor are usable source foundations; they
are not a complete source registry. The Windows tree path verifies the sampled
window monitor and returns DifferentMonitor/MonitorUnverified rather than guessing
(`crates/screenpipe-a11y/src/tree/windows.rs:414-419`). Concurrent capture code
rechecks sampled foreground association and withholds inconsistent results
(`crates/screenpipe-engine/src/event_driven_capture.rs:2345,2413`). These checks
must not be represented as continuous attribution or exclusive monitor ownership.

Trees can include off-screen controls; bounds/on_screen describe a different
relationship from capture origin. A spanning window, legacy frame, optional input
frame correlation, mixed audio or multi-source memory needs unknown/multiple
provenance. Audio device diagnostic IDs are process-local ordinals assigned to device identities
(`crates/screenpipe-audio/src/core/device.rs:91`); they do not establish persistence
across restarts or hardware changes. No implemented common relation expresses
one/several monitors across all captured types. The next contract should retain
acquisition origin, sampled foreground monitor, visible bounds and derivative
origins separately, with uncertainty and sampling time.

## Deletion and media boundary behavior

The REST delete request is a time range with `local_only`, not data-type/device/
monitor selection (`crates/screenpipe-engine/src/routes/data.rs:20`). Both database
variants collect removable chunk paths; the comment implying non-cloud paths are
preserved by one mode is not an effective distinction in the inspected code
(`db:6326,6541`). Do not build new policy on that comment.

The database uses inclusive BETWEEN intervals for these deletes, while exports
describe [start,end) windows. Define one action boundary contract before composing
intervals. Transcript timestamps are not full sample intervals. Selected rows in
shared chunks are removed while the file remains for surviving references. Media
eviction likewise operates on whole chunks, preserving searchable rows and
derivatives. It does not perform selected-byte surgery.

Global orphan row cleanup can affect chunks outside the requested interval
(`db:6448,6465,7133`). Collected file paths originate from selected frames or
transcripts, so preexisting orphan files and no-transcript audio may remain with
no database path. `get_oldest_timestamp` omits raw audio chunks (`db:7168-7182`),
which also limits retention coverage.

Database commit precedes file unlink. Unlink failures are warnings and the handler
can still return success; snapshot failures are not represented like video/audio
counts (`routes/data.rs:72-115`). REST deletion counts omit meeting/segment counts
even though DB removal covers them. Require durable per-object cleanup results,
retryable ownership records and an honest partially-completed action state.

Compaction reads/encodes snapshots before pointer mutation. A concurrent discard
can therefore race a new MP4 containing selected images. Pointer-update submission
failure is logged before source JPEG cleanup (`snapshot_compaction.rs:337-364`).
Specify coordination and recovery for discard, compaction, capture, retranscription
and cache writers. Existing WAL and incremental vacuum (`db:332`; `retention.rs:465`)
are not proof of forensic erasure; permanent deletion versus undo remains open.

## Content exposure matrix

All paths below are implemented source paths unless expressly marked otherwise.
`engine/` abbreviates `crates/screenpipe-engine/src/`; `desktop/` abbreviates
`apps/screenpipe-app-tauri/`. No route or consumer was exercised. These are bypass
risks for a proposed gate applied only to search, not claims of a deployed review
feature failing.

| Surface | Verified source foundation | Required pending-review coverage |
|---|---|---|
| Authentication and vault | `engine/server.rs:434-510,638-686` registers content routes and checks bearer/header, cookie or query token; `engine/routes/vault.rs:95-121` returns 423 for a locked vault. | Caller authentication and whole-vault lock are distinct from approval of individual records. Already upgraded sockets and direct local files require separate enforcement. |
| REST search and keyword search | `engine/routes/search.rs:209,374-379,525,563-572`; server search TTL is 60 seconds (`engine/server.rs:365`). | Cover text, paths, snippets, metadata, extracted images and cache hits. Removing rows from results alone incorrectly reports absence. |
| Elements, summaries, suggestions and context | `engine/routes/elements.rs:127-181`; activity summary executes seven direct queries (`engine/routes/activity_summary.rs:495-501`). Frame/search endpoints expose derived context. | Apply shared eligibility to every direct query, aggregation and snippet. Counts, names, edited paths and structural properties can disclose pending material. Define allowed safe metadata explicitly. |
| Frame image/text/metadata/context and on-demand OCR | `engine/routes/frames.rs:45,58-115,265-276,310-344,400,458,528,747,858`. | Gate before image cache, snapshot read, FFmpeg extraction, nearby-frame fallback and derivative OCR creation. Known pending ID must return a reason, not missing/404 or an unexplained replacement frame. |
| HTTP frame browser cache | PII response uses no-store (`engine/routes/frames.rs:1105`); ordinary file response is public/max-age 604800 (`:1118-1126`). | Seven-day caching does not match review/revocation lifecycle. Define no-store/version behavior and limitations on already delivered images. |
| Timeline stream/WebSocket | `/stream/frames` registration (`engine/server.rs:590`); response contains text/images/paths/audio (`engine/routes/streaming.rs:74-124`), hot cache (`:309`), DB backfill (`:359`), live frame/nearby audio (`:501-548`), live audio (`:581-597`). | Gate initial, historical, backfill and live delivery. Cancel queued content on transition and send explicit redaction/revocation status. Nearby audio is temporal association, not provenance. |
| General event WebSocket | `/ws/events` (`engine/server.rs:591`); all-event subscription/serialization (`engine/routes/websocket.rs:109-141`); images=false only strips image from two event names (`:132-135`); client messages rebroadcast events (`:113-118`). | Event content can precede DB persistence. Need release checks at publication/delivery, unknown-event policy, image=true coverage and queue invalidation. Text/arbitrary payloads are not covered by image stripping. |
| Meeting transcript, memory and speaker REST | `engine/routes/meetings.rs:219,252,267`; memories `:200-235,270`; speakers `:106,180,284`. | Cover titles/attendees/notes, transcripts, memories/tags, speaker labels/embeddings/sample paths and mixed-source derivatives. Existing meeting PII redaction is not pending approval. |
| Raw SQL | `engine/routes/content.rs:543-608` allows SELECT/WITH/EXPLAIN with LIMIT and executes caller SQL. | Table/row visibility is unrestricted by those query-shape checks. A normal route gate is bypassable through joins/FTS/direct tables. Specify approved views/surface or denied classes; field-name response scrubbing is insufficient. This choice is still open. |
| HTTP media export | `engine/server.rs:460-462`; `engine/routes/meetings.rs:784`; common export directly queries frames/audio (`engine/meeting_export.rs:105-147`). | Enforce in common core before reading any source, including mixed device/monitor files. Export interval is not verified meeting membership. Independent MP4 has no implemented recall mechanism. |
| Desktop and offline CLI export | `desktop/src-tauri/src/meeting_export.rs:44-82` calls engine core in process; `engine/cli/export.rs:55-95` opens DB without daemon and calls same core. | HTTP middleware cannot protect these. Decide ordinary export versus trusted review capability and enforce lifecycle in DB/core. |
| Database backup | `engine/routes/data.rs:357-386`; `engine/cli/backup.rs:10,66` call SQLite backup_to. | Raw database copies include pending material. Specify archival backup/recovery scope separately from consumer content access; do not silently choose whether backup may contain pending data. Copies are outside ordinary row deletion. |
| Offline CLI search | `engine/cli/search.rs:28,47-56,96` opens DB directly and prints ordinary content results. | Daemon middleware is bypassed. Apply the same reason-bearing visibility result to shared DB/search conversion; do not render pending as the existing no-results output. |
| Recovery archives | `engine/cli/db.rs:438-514` creates recovery/snapshot/pre-recovery artifacts; cleanup is a separate command (`:649-701`). | Copies can retain removed content. Define recovery/backup obligations and exclusions explicitly; source-row Discard does not erase these artifacts. |
| Content-producing mutations | `/add`, frame merge, media validation and retranscription are registered (`engine/server.rs:445,468,476-477`); retranscription selects chunk IDs/ranges directly (`engine/routes/retranscribe.rs:263-282,506`), media validation takes a file path (`engine/routes/content.rs:639-643`). | Gate reads/derived outputs and carry lifecycle into replacement writes. User-supplied content/imports need explicit provenance; unknown must not silently become kept. |
| Browser bridge | `/browser/ws`, `/browser/eval`, `/browser/status` (`engine/server.rs:600-628`); eval returns extension result (`engine/routes/browser.rs:124-143`). | Independent browser acquisition is outside recorded-data lineage. If it can access a local review/content page, that page must enforce its capability; bridge existence is not verified captured-data membership. The cookies handler exists in source but is not registered here. |
| Desktop raw media IPC | `desktop/src-tauri/src/main.rs:245-279` reads supplied path with fs::read; `desktop/lib/actions/video-actions.ts:3-9` exposes wrapper. | Requires file-to-record/chunk authority and handling mixed approved/pending bytes. REST redaction is bypassed; local review exception is not yet authorized by policy. |
| Audio playback blobs | `desktop/lib/hooks/use-audio-playback.tsx:132,184-226` loads whole file, caches segments and makes blob URL; `:178,511` revokes URLs. | Whole file can expose adjacent segments. Stop playback, revoke blobs and purge segment/path caches on lifecycle transition. |
| Direct frame/video asset URLs | `desktop/components/rewind/hooks/use-frame-loading.ts:145-149` uses convertFileSrc; `desktop/src-tauri/tauri.conf.json:85-90` asset protocol permits app-data and default recording-root paths. | Bypasses HTTP entirely, including shared files. Needs provenance-aware media access or an explicitly scoped review capability. |
| Generic local viewer/attachments | `desktop/src-tauri/src/viewer.rs:116-169` reads paths/base64 images; `desktop/components/markdown.tsx:256-273` uses assets; `desktop/components/meeting-notes/note-view.tsx:220` reads files. | Cached paths can recover retained bytes. Generic viewer is not automatically trusted review. Markdown references `/experimental/frames/from-file`, but it is not registered in engine server and is not counted as an active endpoint. |
| Persistent timeline cache | `desktop/lib/hooks/use-timeline-cache.tsx:10-18,33-80,100` stores/loads last 200 response objects in localforage; `use-timeline-store.tsx:142-152,234,279` displays cached entries before refresh and saves them. | Store namespace is not lifecycle version. Purge/version persistent copies on review/discard/expiry and notify displayed records. Startup cache can otherwise reveal deleted text/transcripts. |
| Frontend stale cache | `desktop/lib/cache.ts:24-38` TTL/getStale; `:48-63` manual invalidation. | TTL is not retention. Purge by source/derivative identity including expired objects, not only currently fresh entries. |
| Server extraction/search/video caches | Manual delete only evicts hot frame entries (`engine/routes/data.rs:96-104,218`); search and frame-image caches are separate (`engine/routes/search.rs:374`; `engine/routes/frames.rs:58,265`). Video extraction cache implementation is in `engine/video_cache.rs`. | Retention worker and manual deletion need one invalidation contract for text, paths, extracted temp media and concurrent insertions. Recheck lifecycle before serving each cache hit. |
| Configured Pi/local model consumer | `desktop/src-tauri/src/pi.rs:1314-1328` configures authenticated bash shim with local API key. | Broad authenticated tools must receive redaction reasons. Tool results/model output/chat history can be derivative copies; local model execution is not approval to release pending content. |
| DigiTrack evidence packets | `scripts/windows/evidence-review/EvidenceReview.psm1:72,161-209` GET allowlist collects search/elements/meetings/transcripts/metadata/context; `:257` notes concurrent insert and search cache limits. | Collector must preserve pending reason rather than claim no evidence. `docs/DIGITRACK_API_INTEGRATION.md` is guidance; there is no downstream DigiTrack implementation/purge proof in this fork. No evidence output was read. |

The ordinary desktop rewind/viewer and consumers are existing content readers,
not an agreed trusted local review boundary. Step 3 needs a capability/response
contract for local review and a shared release result used by DB queries, core
exports, media access and event delivery. OS-level user access to files, exported
copies and separately configured consumers cannot be recalled by a REST predicate;
state that limitation while defining application-controlled purge obligations.
Local logs and panic files are also separate retained artifacts
(`engine/crash_log.rs:18-40`). The audited media path logs filenames
(`desktop/src-tauri/src/main.rs:251`); no log/archive content was inspected.
Do not assume range deletion covers diagnostic artifacts or claim they contain no
sensitive context. A broader logging-content review remains separate from this
lineage inventory; new review/discard notices must use safe reason codes.

## Next contracts and synthetic tests

The following are recommendations for step 3, not implementation authorization.

| Contract to specify | Minimum regression evidence |
|---|---|
| Durable lifecycle and release gate | Pending content returns explicit safe reason across every route/stream; absence, privacy suppression, failed capture and pending review remain distinct. Kept data releases only after committed transition. Decide allowed safe metadata and trusted local review separately. |
| Lineage and fine-grained selection | Synthetic monitor/device/type selection includes source and derivatives by default; kept exceptions disclose all remaining content. Unknown/multiple attribution is included by All monitors; sampled foreground is never substituted for acquisition origin. |
| Shared files and boundaries | Frames/transcripts on both sides of selected boundary in one MP4; sample-overlap versus row timestamps; no-transcript chunks; spanning/multiple monitor content. Verify retained bytes and honest reports, not only row counts. |
| Derivative closure | Shared accessibility anchors, OCR/full_text/elements/FTS, live meeting segments, mixed-source memories and speaker embeddings. Verify removal/regeneration policy and kept exceptions, including a meeting ending inside range whose earlier segments lie outside. |
| Race-free transitions | Pause/Keep/Discard with queued transcription, live partial/final events, cache insertion, compaction and in-flight export. Stale work cannot recreate or release discarded/pending content; capture permits alone are insufficient. |
| Durable deletion completion | Inject database failure, unlink failure, crash between DB commit/unlink and missing/full storage. Retry survives restart and reports incomplete removal without copying sensitive errors into notices. |
| Cache and consumer revocation | Warm disk/browser/server caches before transition; reconnect events/streams; consumer polling and stored packets obey lifecycle version. Define external-copy limitations and consumer acknowledgement instead of promising retroactive erasure. |
| Review deadlines and snooze | Configurable temporary days, approaching-deadline reminders, snooze extends deadline, restart/offline expiry, clock changes, repeated extensions and failed deletion. Exact durations/reset/expiry semantics remain owner decisions. |
| Storage pressure and precedence | Voluntary discard prompt; pressure alone never forces discard. Exhaustion stops affected recording and alerts/persists reason where possible. Separately test configured age cleanup/review deadline precedence and failures to persist the alert. |

Existing retention settings default to disabled, fourteen days and media mode
(`crates/screenpipe-engine/src/retention.rs:78`). The worker is age-based and uses
one-hour range batches on a five-minute poll, not a review queue. Disk usage and
cleanup confirmation are reuse
foundations (`apps/screenpipe-app-tauri/src-tauri/src/disk_usage.rs:333`;
`apps/screenpipe-app-tauri/components/settings/retention-settings.tsx:448,478`).
They do not establish storage-pressure stop/alert coverage. Review state, reminders,
snooze, expiry/startup semantics, API shape, local review trust, undo, fine-grained
exceptions, conversation membership and mixed-source derivative handling remain
unresolved. No existing age worker should silently be converted into these policies.

## Post-audit scoped repairs (2026-10-08)

The owner subsequently authorized status UI and concrete existing-deletion repairs.
See [deletion repairs](DELETION_REPAIRS.md): selected-only orphan-row cleanup,
transactional media cleanup ownership, bounded fair restart retries, live-reference
checks, safe incomplete/unknown responses and cache invalidation now have three
passing synthetic byte/store regressions. The historical audit above remains the
inspection record; its line anchors are not current source positions. Shared media
bytes, uncertain derivative lineage, raw untranscribed range selection, historical
orphans, consumer copies and filesystem-only writer coordination remain gaps.
The cleanup table is not a retention approval ledger or an anti-resurrection fence.
Production build validation is recorded separately in the validation register.
