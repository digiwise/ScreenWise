# DigiTrack evidence integration: API assessment and client guide

Reviewed 2026-10-06 against ScreenWise commit `3078fb4d8`.
Retrieval guidance updated 2026-10-06 using the two-minute export assessment
reported against `303fd20b4`; the API contract review baseline is unchanged.

The purpose of this guide is to explain ScreenWise's ability to provide evidence to DigiTrack.

ScreenWise is a fork of ScreenPipe designed for comprehensive, privacy aware, data
capture from screen, keyboard, audio and clipboard recording.  For more general information
see the [ScreenWise README](../README.md).

The API assessment found ScreenWise's records and API "sufficient for a limited, on-demand
evidence-enrichment prototype; not a complete or snapshot-consistent capture-history contract."
DigiTrack should retain its own focus, idle and microphone recording and its ownership of
time entries and Clockify sync. ScreenWise evidence can help explain those
periods, but cannot establish exact work or billable duration by itself.
This guide documents existing routes and identified limitations.
For normal summarisation, use the [staged retrieval workflow](#staged-retrieval-and-evidence-reduction)
below. The exhaustive examples are for contract inspection and drill-down, not
an instruction to load every element and full frame context for every period.

## Connection, authentication and time

- Configure the recorder's loopback base URL; the default is
  `http://127.0.0.1:3030`. Do not discover or accept arbitrary remote endpoints.
- Obtain the active recording directory's bearer token through the documented
  `screenpipe auth token --data-dir <recorder-data-directory>` command. Handle
  its output privately. DigiTrack should store it securely, never in prompts,
  exports, URLs, public logs or source control.
- Send `Authorization: Bearer <token>` on every evidence request. `/health` is
  the only exemption in the reviewed server authentication middleware. Missing
  or incorrect credentials return 403. A valid token grants more than read-only
  access: restrict the DigiTrack client to an explicit GET-route allowlist.
- Add `timezone=utc` to every JSON request. Otherwise server middleware rewrites
  timestamp-looking strings into the recorder machine's local time, including
  strings nested in captured content. Preserve UTC instants internally and
  convert only for display using the user's chosen zone and date-specific offset.
- Use URL-encoded RFC3339 timestamps with an explicit offset, preferably `Z`.
  On the client, include the start time and exclude the end time. `/search` and `/meetings` use inclusive
  upper bounds; `/capture-events` uses an exclusive upper bound. Post-filter
  and deduplicate records at adjoining period boundaries.
- `GET /health` describes current health, not historical coverage. A healthy
  response does not prove earlier capture was complete. Treat 403, 408, 503,
  other errors, cancellation and exhausted query budgets as distinct from an
  empty successful result. Never disable authentication to recover a query.

## Routes to use

| Purpose | GET route | Important fields/shape |
|---|---|---|
| Screen text and frame references | `/search?content_type=ocr` | `{data:[{type:"OCR",content:{...}}],pagination:{limit,offset,total}}`; `frame_id`, `timestamp`, `text`, `text_source`, `app_name`, `window_name`, `browser_url`, `focused`, `device_name` |
| Individual retained accessibility/OCR elements | `/elements` | `{data:[...],pagination:{...}}`; `id`, `frame_id`, `source`, `text`, `role`, `bounds`, `on_screen`, `confidence`; add visibility/source filters when needed |
| Audio transcripts | `/search?content_type=audio` | `type:"Audio"`; `chunk_id`, `offset_index`, `timestamp`, `transcription`/`text`, `device_name`, `device_type`, optional segment times, speaker/source/provider/model metadata and `meeting_id` |
| Optional retained input events | `/search?content_type=input` | `type:"Input"`; `id`, `timestamp`, `event_type`, optional app/window/context, `text_content` and `frame_id` |
| Frame timestamp for a reference | `/frames/{frame_id}/metadata` | `{frame_id,timestamp}`; returns 404 when the frame is absent |
| Full retained frame context on demand | `/frames/{frame_id}/context` | `{frame_id,text,nodes,urls,text_source}`; accessibility first, OCR fallback |
| Full element detail for a frame | `/frames/{frame_id}/elements` | `{data:[...],pagination:{...}}`; optional `source=accessibility` or `source=ocr`; returns all frame elements rather than offset pages |
| Meeting metadata | `/meetings` and `/meetings/{id}` | Array/single record; `id`, `meeting_start`, nullable `meeting_end`, `meeting_app`, `detection_source`, optional title/attendees/note |
| Meeting-specific transcript references | `/meetings/{id}/transcript` | Array with **camelCase** fields, including `id`, `meetingId`, `capturedAt`, `transcript`, `source`, `provider`, `model`, `audioChunkId`, `audioTranscriptionId`, `deviceName`, `deviceType` |
| Safe persisted notices and current loss status | `/capture-events` | `{data:[{id,timestamp,state,reason_code,message}],has_more,persistence_degraded,event_delivery,audio_delivery,audio_shutdown_degraded,audio_shutdown_issues}` |

`/search` also returns file paths and other private metadata. Do not forward
whole responses into an LLM: project only the evidence necessary for the task.
`transcription` and `text` are aliases, not two separate observations.
Do not assume audio `device_type` casing matches the meeting transcript's string
field; decode each response contract separately.

### Example request targets

These are illustrative GET targets, not commands executed during this review.
Replace the dates with the selected UTC range and encode query values with the
client HTTP library. All requests require the bearer header except `/health`.

```text
/search?content_type=ocr&start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=100&offset=0&include_frames=false&timezone=utc
/search?content_type=audio&start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=100&offset=0&timezone=utc
/search?content_type=input&start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=100&offset=0&timezone=utc
/elements?source=accessibility&on_screen=true&start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=100&offset=0&timezone=utc
/frames/123/metadata?timezone=utc
/frames/123/context?timezone=utc
/capture-events?start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=1000&timezone=utc
/meetings?end_time=2026-10-06T00:30:00Z&limit=100&offset=0&timezone=utc
/meetings/456/transcript?timezone=utc
```

### Fabricated retrieval walkthrough

Every name, ID, timestamp and captured value below is fabricated. JSON examples
show selected response fields; actual responses contain additional fields.
These are illustrative contracts, not evidence from a running recorder.

Review period: **2026-10-06 00:00:00Z to 00:30:00Z**, including the start and
excluding the end. Prefix each target with the configured loopback base URL
(for example `http://127.0.0.1:3030`, or port `31579` for the GUI trial launcher)
and send `Authorization: Bearer <token>`. Omit `q` to retrieve all matching
records, rather than only records containing a particular search phrase.

**1. Stored screen text, including accessibility-derived text.**

```http
GET /search?content_type=ocr&start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=100&offset=0&include_frames=false&timezone=utc
Authorization: Bearer <token>
```

```json
{
  "data": [
    {"type": "OCR", "content": {
      "frame_id": 123, "timestamp": "2026-10-06T00:12:00Z",
      "text": "Planner connection troubleshooting — checking permissions",
      "text_source": "accessibility", "app_name": "Example Chat",
      "window_name": "Synthetic project discussion", "browser_url": null,
      "focused": true, "device_name": "Display 1"
    }},
    {"type": "OCR", "content": {
      "frame_id": 122, "timestamp": "2026-10-06T00:04:00Z",
      "text": "ScreenWise build completed", "text_source": "ocr",
      "app_name": "Example Terminal", "window_name": "Synthetic build",
      "browser_url": null, "focused": true, "device_name": "Display 1"
    }}
  ],
  "pagination": {"limit": 100, "offset": 0, "total": 2}
}
```

**2. Element-level text and provenance.** For exhaustive inspection, fetch
`/elements` without a source or visibility filter for all retained matching
OCR/accessibility elements. This includes potentially off-screen accessibility
text. Normal summarisation should fetch selected detail only; for explicitly
visible accessibility elements, add `source=accessibility&on_screen=true`.

```http
GET /elements?start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=100&offset=0&timezone=utc
```

```json
{
  "data": [
    {"id": 9001, "frame_id": 123, "source": "accessibility",
     "role": "Text", "text": "Checking permissions", "parent_id": null,
     "depth": 1, "bounds": {"left": 40, "top": 80, "width": 200, "height": 20},
     "confidence": null, "sort_order": 0, "on_screen": true},
    {"id": 9002, "frame_id": 122, "source": "ocr",
     "role": "text", "text": "ScreenWise build completed", "parent_id": null,
     "depth": 0, "bounds": {"left": 20, "top": 60, "width": 250, "height": 20},
     "confidence": 0.96, "sort_order": 0, "on_screen": true}
  ],
  "pagination": {"limit": 100, "offset": 0, "total": 2}
}
```

Resolve element timestamps using their frame IDs. Detailed context is optional
and can be loaded when the reviewer follows a reference:

```http
GET /frames/123/metadata?timezone=utc
GET /frames/123/context?timezone=utc
GET /frames/123/elements?source=accessibility&timezone=utc
```

Example metadata and context responses, respectively:

```json
{"frame_id": 123, "timestamp": "2026-10-06T00:12:00Z"}
```

```json
{
  "frame_id": 123, "text": "Planner connection troubleshooting — checking permissions",
  "nodes": [{"role": "Text", "text": "Checking permissions", "depth": 1}],
  "urls": [], "text_source": "accessibility"
}
```

The frame-elements response uses the same `data`/`pagination` shape as
`/elements`. Frame context may include off-screen text; preserve that distinction.
Elements and frame text overlap and must not be counted as separate work.

**3. Audio/transcription evidence.**

```http
GET /search?content_type=audio&start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=100&offset=0&timezone=utc
```

```json
{
  "data": [{"type": "Audio", "content": {
    "chunk_id": 700, "offset_index": 0, "timestamp": "2026-10-06T00:16:00Z",
    "transcription": "Let us review the Planner permissions.",
    "text": "Let us review the Planner permissions.",
    "device_name": "Example USB microphone", "device_type": "Input",
    "start_time": 0.4, "end_time": 4.2, "speaker": null,
    "speaker_label": null, "speaker_provisional": true,
    "source": "local", "provider": "parakeet",
    "model": "parakeet-tdt-0.6b-v3", "meeting_id": 456
  }}],
  "pagination": {"limit": 100, "offset": 0, "total": 1}
}
```

The source/provider/model values are illustrative strings, not a required enum.
Query without app/window filters. Keep one transcript, since `text` is its alias.
Retrieve output-device rows too; do not assume microphone-only recording.

**4. Retained keyboard, clipboard and other input events.**

```http
GET /search?content_type=input&start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=100&offset=0&timezone=utc
```

```json
{
  "data": [
    {"type": "Input", "content": {
      "id": 810, "timestamp": "2026-10-06T00:03:00Z", "event_type": "text",
      "app_name": "Example Editor", "window_title": "Synthetic source file",
      "browser_url": null, "text_content": "Check the API response shape",
      "element_role": "Edit", "element_name": "Editor", "frame_id": 122
    }},
    {"type": "Input", "content": {
      "id": 811, "timestamp": "2026-10-06T00:13:00Z", "event_type": "clipboard",
      "app_name": "Example Chat", "window_title": "Synthetic project discussion",
      "browser_url": null, "text_content": "Synthetic permissions note",
      "element_role": null, "element_name": null, "frame_id": 123
    }}
  ],
  "pagination": {"limit": 100, "offset": 0, "total": 2}
}
```

Input capture must have been enabled and admitted. This route can also return
`privacy_notice` rows: discard their raw payloads from the evidence/model input
and use `/capture-events` for safe notices. Do not reconstruct suppressed input.

**5. Overlapping meetings and their transcript segments.** Omit the lower bound
when listing meetings so calls that started earlier can still be found.

```http
GET /meetings?end_time=2026-10-06T00:30:00Z&limit=100&offset=0&timezone=utc
GET /meetings/456/transcript?timezone=utc
```

Example list and transcript responses, respectively:

```json
[
  {"id": 456, "meeting_start": "2026-10-05T23:55:00Z",
   "meeting_end": "2026-10-06T00:20:00Z", "meeting_app": "Example Teams",
   "title": "Synthetic project review", "attendees": null, "note": null,
   "detection_source": "manual", "created_at": "2026-10-05T23:55:00Z"}
]
```

```json
[
  {"id": 6001, "meetingId": 456, "capturedAt": "2026-10-06T00:16:00Z",
   "createdAt": "2026-10-06T00:16:05Z", "source": "local",
   "provider": "parakeet", "model": "parakeet-tdt-0.6b-v3",
   "itemId": "synthetic-segment-1", "deviceName": "Example USB microphone",
   "deviceType": "input", "audioTranscriptionId": 701, "audioChunkId": 700,
   "audioFilePath": null, "speakerId": null, "speakerName": null,
   "transcript": "Let us review the Planner permissions."}
]
```

Client-filter meetings by overlap and transcript segments by `capturedAt`.
The segment above refers to the same audio evidence as step 3; do not duplicate
it in a summary. The meeting's pre-period portion is context, not work duration
to add to this review period.

**6. Privacy/loss notices and status.**

```http
GET /capture-events?start_time=2026-10-06T00:00:00Z&end_time=2026-10-06T00:30:00Z&limit=1000&timezone=utc
```

```json
{
  "data": [
    {"id": 820, "timestamp": "2026-10-06T00:26:00Z", "state": "unlocked",
     "reason_code": "wts_session_unlocked",
     "message": "Screen unlocked; other privacy and recording controls still apply."},
    {"id": 819, "timestamp": "2026-10-06T00:24:00Z", "state": "locked",
     "reason_code": "wts_session_locked",
     "message": "Screen locked; screen/UI capture paused."}
  ],
  "has_more": false, "persistence_degraded": false,
  "event_delivery": {"near_capacity": false, "dropped_events": 0},
  "audio_shutdown_degraded": false, "audio_shutdown_issues": []
}
```

The omitted `audio_delivery` field contains a `queues` array with per-queue
diagnostics. Top-level diagnostics describe current process state, not an audit
of this historical half hour. Query an earlier bounded notice range if initial
lock state is needed; retain unknown state if no reliable predecessor is found.

**Exhausting the range:** for each search and global-elements route, repeat with
`offset=100`, `200`, and so on until the unfiltered pagination total is consumed.
An empty search page alone is not a stopping condition. For meetings, stop at a
short page. For notices, subdivide the time range when `has_more=true`, since
there is no cursor. Filter boundary records locally to exclude exactly 00:30:00Z,
deduplicate source-scoped references and reconcile late-arriving audio. Omit
`max_content_length` to avoid deliberate search-text truncation. Apply the
pagination/concurrency limits below; reaching a budget means partial retrieval.

This retrieves the relevant API-exposed evidence, not every raw recording or
every possible activity. Images/audio files are not downloaded by these JSON
queries; memories are derived material rather than direct capture. Missing
coverage, suppressed content and unavailable transcripts remain unknown.

### Screen text: selection and provenance

Prefer explicit modality queries over `content_type=all`. The current `all`
dispatch combines screen/accessibility/audio; it does not enumerate input or
memory records. It also merges accessibility text into an OCR-labelled result
without necessarily updating that result's provenance. Separate queries avoid
that merger. Memory is not a direct observation of work and is unnecessary here.

`/search?content_type=ocr` returns stored frame text, including text acquired
through accessibility/UIA. It does not include audio transcripts or
keyboard/clipboard events; query those channels separately.
The historical `OCR` response variant is not proof that OCR produced its text:
read `text_source`. It can be `accessibility`, `ocr` or null for older records.
Frame `text` may be the stored combined/full text; source-specific elements
provide finer provenance. Do not turn `focused=null` into either true or false.
Multiple monitor records are evidence of one elapsed period, not additive time.
Identical text was observed under different monitor labels in the reviewed
export. Its acquisition/attribution cause is under investigation: do not infer
that a window was visible on every named monitor, that its pixels were copied,
or that privacy suppression caused the duplication. Retain the original labels
as reported provenance, with monitor attribution uncertain where relevant.

**Visibility caveat:** `/search?content_type=accessibility&on_screen=true`
selects frames with matching on-screen elements, but still returns the frame's
combined `full_text`. It does not strip off-screen text from that field. Use
`/elements?source=accessibility&on_screen=true` for explicitly visible element
text, grouping by `frame_id` and resolving its timestamp through frame metadata
or the screen query. Null visibility on legacy elements means unknown; the
strict filter excludes them. Track this coverage limitation rather than silently
asserting all retained accessibility text was seen by the user.

`/frames/{id}/context` likewise returns full retained context, not only visible
elements. Its fallback can return null text after a read error or for missing
data. Resolve frame existence with `/metadata`; do not treat empty context as
proof that the frame contained no text. `/text` returns positions, not a canonical
plain-text dump, and is not required for the initial client.

Leave `include_frames=false`. Do not use POST `/frames/{id}/text` (it runs OCR),
or resolve an image by reading its returned filesystem path. If later image
inspection is added, use the authenticated image route under a separate explicit
user action and review its fallback/redaction behaviour first.

### Pagination and references

Search pages use `limit`/`offset` and newest-first timestamps. The database
queries do not provide a stable ID tie-break or transaction-wide snapshot.
Concurrent inserts, transcript completion, retention and equal timestamps can
shift pages. There is no complete-export guarantee from offset pagination.

Advance offset by the requested limit, not `data.length`: the HTTP handler
filters Screenpipe app rows after database pagination, so even an empty returned
page can precede more rows. The total is a pre-display-filter count. Do not add
device/machine filters for the initial prototype: those search filters are not
also passed to the count function. Keep bounded, closed time windows, reconcile
duplicate IDs, and report limits or unsettled results. Fixing the window does not
prevent late-arriving records. Re-query when reviewing recent audio.

Search responses have a 60-second cache TTL, and their range cache key uses
whole-second timestamps. Distinct subsecond ranges can therefore share a key.
Use whole-second query boundaries for the prototype and filter exact instants
on the client; do not assume a rapid repeated query sees newly persisted data.
Stable ordering/snapshot retrieval and subsecond cache-key precision are API
follow-up items for stronger completeness claims.

Use references scoped to a locally configured **source-instance identity**, not
the base URL alone: frame ID; element ID plus frame ID; audio chunk ID plus offset
index; input-event ID; meeting ID; meeting-transcript ID; notice ID. Frame/audio
IDs can collide across recording directories. The reviewed API does not expose
an authoritative recording-store UUID or reset generation. Require explicit
reconnection/new identity when the recording directory changes; a local label
cannot detect an unannounced database replacement at the same URL.

Audio search groups by `(audio_chunk_id, offset_index)` and does not expose the
transcription row ID. Retain its timestamp, segment fields and provenance with
the reference; chunk ID alone is insufficient. Numeric `start_time`/`end_time`
are seconds within audio processing, not RFC3339 wall-clock bounds. Do not derive
exact work duration from them without validating the capture/provider anchoring.
Transcripts can arrive late, be reconciled, be absent, or change after explicit
retranscription. An unavailable transcript is not evidence of silence.

### Meetings overlapping the selected period

`/meetings?start_time=A&end_time=B` means **meeting start** lies between A and B;
it is not an overlap query. For correctness with the current API, paginate
metadata with `end_time=B` and no lower bound, then retain records satisfying
`meeting_start < B && (meeting_end == null || meeting_end > A)` on the client.
Retrieve transcripts only for retained meetings, and time-filter their segments
to the period. The list has no total: advance by limit until a short page.

A bounded lookback is cheaper but cannot guarantee finding an unusually long or
still-open older meeting. If a request/page budget prevents exhaustion, label
meeting coverage incomplete. Do not assume null meeting end proves a call is
still active or that a detection source proves the whole interval was a call.

## Privacy and historical coverage

Persisted `/capture-events` notices cover Windows lock/unlock/detection failure,
audio shutdown issues, audio queue/delivery conditions and session recovery.
Messages are reconstructed from closed reason enums; invalid stored payloads
are skipped and set `persistence_degraded`. Never parse raw input-event notice
JSON as a substitute for this allowlisted endpoint.

The range must be nonempty and at most 32 days. Limit defaults to 1000 and is
clamped to 1..1000. Results are newest first by timestamp then ID. `has_more`
signals truncation, but there is **no offset/cursor**. Subdivide time windows
when needed; if more than 1000 notices share an indivisible timestamp, report
incomplete coverage rather than claiming successful pagination.

There is no initial-state-at-range-start field. A lock/failure transition before
the requested range can remain relevant even when the range contains no notice.
An earlier bounded lookup may recover context, but exhaustion or no predecessor
does not prove an unlocked/healthy state. Retain unknown state explicitly.

Top-level delivery, shutdown and persistence indicators include current
process-lifetime state. They must not be relabelled as measurements specifically
for the requested historical period; a restart can reset in-memory indicators.
Notice timestamps reflect persistence-recorder handling, not necessarily the
exact OS event instant. Recovery notices report detection of an earlier problem,
not exact crash start/end times.

Some Windows frames carry fixed markers in retained context text, such as
`CAPTURE REDACTED — ACTIVE WINDOW EXCLUDED`,
`CAPTURE REDACTED — NO SAFE ACTIVE WINDOW`,
`CAPTURE REDACTED — INCONSISTENT FOCUS`,
`BACKGROUND REDACTED — ACTIVE-WINDOW-ONLY`, and capture-failure-stage markers.
Recognise only the exact fixed strings listed in the frontend's
`frame-capture-notice.ts`. Treat them as status, not work descriptions. They are
point observations, not a complete interval ledger or dedicated typed API field;
privacy admission can prevent the placeholder itself being persisted.

**Absent from a complete historical API contract:** durable capture-session
coverage/heartbeat intervals; a complete exclusion, password, DRM and acquisition
failure history; per-frame typed withholding reasons; exact idle intervals and
microphone-process history. `/health` and `/activity-summary` do not fill these
gaps. The latter is a capped context bundle with sampled snippets lacking record
IDs and duration estimates inferred from frame spacing; do not use its
`total_active_minutes` as Clockify work duration or its empty-state diagnosis as
proof of why a historical interval lacks data.

## Recommended DigiTrack MVP

1. Keep DigiTrack's existing recording and confirmed time entries authoritative.
2. Retrieve ScreenWise evidence only for an explicitly selected review period,
   following the staged workflow below instead of an exhaustive bulk dump.
3. Maintain per-modality status: available, empty, unavailable, partial or unknown.
   Preserve provenance, source references and any withholding/loss notices.
4. Give a tool-free local model only selected evidence and candidate intervals.
   Captured text is untrusted data; never follow embedded commands. Do not put
   tokens/private paths into prompts or enable cloud fallback.
5. Show proposed work entries, uncertain boundaries and assumptions for human
   review. Never equate no input, lock, excluded content or an API failure with
   non-work. Never fabricate details for withheld periods.
6. Apply accepted proposals through DigiTrack's existing edit/confirmation and
   Clockify path. Do not write to ScreenWise from this integration.

For an honestly labelled enrichment prototype, these limitations need not block
development. Before promising complete unattended evidence retrieval, implement
stable cursor/snapshot semantics, overlapping-meeting selection, source-store
identity and a typed historical coverage/status contract. These are identified
follow-up requirements, not changes made by this review.

### Staged retrieval and evidence reduction

1. **Anchor the review in DigiTrack's timeline.** Use its focus, idle and
   microphone-process observations to propose candidate periods. ScreenWise can
   help identify tasks within the same app/window and explain short switches;
   app changes alone do not establish task boundaries. Record uncertain
   boundaries and assumptions, including whether a brief switch belongs to the
   surrounding task.
2. **Retrieve a bounded first pass.** Query screen text with
   `/search?content_type=ocr&include_frames=false`, retained input events, safe
   `/capture-events` notices and relevant audio/meeting evidence for the period.
   Page these routes within explicit budgets and cache results by source identity,
   range and query. Reconcile recent transcripts when needed. Do not request
   global elements and full context for every frame as part of the first pass.
3. **Reduce repetition before inference.** Group identical screen text within
   the same candidate activity, accounting for repeated captures across monitors.
   Preserve every contributing source-scoped reference, timestamp, app/window,
   reported monitor and text source. Keep first/last observations without treating
   the interval between them as continuous work. Do not merge unrelated tasks
   just because they contain the same text, or discard changed passages. Screen
   text, elements and context for one frame are overlapping representations,
   not independent observations or extra elapsed time.
4. **Separate actions from existing context.** Newly typed text, focus changes
   and changed passages can corroborate an action; retained old chat messages,
   sidebar titles, menus and button labels usually supply background context.
   A capture timestamp dates the observation, not the creation or performance
   of every activity described in its text. Input fragments, edits and clicks
   need interpretation; a command visible in a chat or paste preview does not
   prove it executed. Prefer relevant content over repeated interface chrome,
   without deleting the original evidence.
5. **Resolve selected ambiguity on demand.** For representative frames or
   uncertain task transitions, use source/visibility-filtered `/elements`, or
   fetch `/frames/{id}/elements` with a source filter and select `on_screen=true`
   records on the client. Resolve timestamps from existing screen records or
   `/metadata`. Prefer explicitly visible accessibility text when available;
   null visibility and unverified monitor attribution remain uncertain. Load
   `/frames/{id}/context` only when this adds needed context or a reviewer opens
   a reference. Off-screen/older text must stay labelled as context. Missing
   visible evidence does not establish an empty screen or justify reconstructing
   withheld content.
6. **Summarise a compact evidence packet.** Give the local model selected
   changes/actions, representative excerpts, candidate boundaries, safe status,
   assumptions and references. Set explicit text/record/request budgets and
   report omitted or incomplete evidence. Retain drill-down access to originals;
   the default review should show short proposed work descriptions and supporting
   references, rather than pages of repeated JSON. Keep the existing untrusted
   content, authentication and human-confirmation requirements.

Treat privacy placeholders as point observations of unavailable/withheld screen
evidence. Neither their count nor the fraction of frames they occupy measures
lost time. Empty audio/transcript results cannot distinguish silence from absent,
suppressed or delayed capture. No notices in the requested range do not establish
complete privacy-status coverage; preserve bounded prior context and unknown
initial state as described above.

## Inspect the API evidence locally

With an existing ScreenWise instance running, use the standalone PowerShell 7
exporter to see the retained evidence a DigiTrack client could retrieve:

```powershell
pwsh -NoProfile -File .\scripts\windows\evidence-review\Export-ScreenWiseEvidence.ps1 `
  -Start '2026-10-06T10:00:00+11:00' -End '2026-10-06T10:30:00+11:00' `
  -OpenReview
```

The deployment manifest supplies the executable, data directory and API port.
For another instance, supply all three with `-ExecutablePath`, `-DataDir` and
`-BaseUrl`. The output directory contains per-modality JSON, a coverage manifest
and an expandable `index.html` for local review. It makes no model calls and
fetches no raw images/audio. Preserve the generated files privately.

The exporter includes screen/UIA/OCR text, source elements, audio transcripts,
keyboard/clipboard records, overlapping meetings and their transcripts, frame
metadata/context, safe notices and bounded prior notice context. Pagination and
request budgets can produce partial data; failures, unknown initial state and
timestamp precision are recorded explicitly. Full frame context is omitted by
default; add `-IncludeFrameContext` only for detailed inspection. Even without
it, this diagnostic exporter retrieves global elements and can be much larger
than the compact evidence packet recommended for DigiTrack. It is not a model
prompt or a proposed normal client workflow.
[Options and limits](../scripts/windows/evidence-review/README.md).

### Findings from a two-minute export (2026-10-06)

A completed assessment of an owner-reviewed export reported these aggregate
counts. No captured text, private paths or personal record identifiers are
included here. This documentation update used that assessment without reading
or re-querying the recording store.

| Result | Observed amount | Implication for DigiTrack |
|---|---:|---|
| Screen text | 56 frames; 24 distinct text values | Repeated text can be grouped while preserving references and changes. |
| Elements | 8,430 records; 3,157 explicitly off-screen | Fetch selected visible detail, rather than every retained element. |
| Full frame context | 56 responses; about 10.4 MB | Avoid duplicating screen text/elements in every review. |
| Input | 32 events | Typing and app/focus changes provide useful corroboration. |
| Retrieval | 147 sequential API requests | Approximately 85 element pages and 56 context requests dominated request count. |
| Review files | About 20.2 MB HTML; 36.2 MB total | Embedding all JSON again makes even a short period cumbersome to review. |
| Privacy placeholders | 11 frames; no in-period `/capture-events` notices | Preserve the reported withholding state; do not convert frame counts to unavailable duration. |
| Audio/transcripts | No returned records | Cause remains unknown; this is not a silence or audio-health assertion. |

The sample contained enough task/action evidence for a useful short description,
but repeated old conversation text and interface labels were poor evidence of
what work happened at capture time. Identical text appeared under multiple
monitor labels; the recorder cause remains a separate investigation. The sample
does not establish correct monitor attribution or privacy enforcement. Request
counts and file sizes explain likely overhead, but API latency and browser
rendering time were not separately benchmarked. These findings refine client
retrieval strategy; they do not change the route contracts or prove completeness.

## Verification and scope

Source inspection followed route registration and bearer/timezone middleware,
public response structs, database filters/order/count queries, notice persistence
and Windows placeholder production. Existing focused tests were run through the
canonical launcher (with `-PlanOnly` before each invocation):

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter privacy_notices::tests
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter routes::search::tests
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter routes::timezone::tests
```

Results: **7 + 7 + 8 tests passed**, zero failures. All three runs reused 685
external and 16 workspace artifacts, with zero rebuilds. Privacy tests exercise
isolated synthetic SQLite persistence, allowlisted output, malformed notices and
degraded writes. Search tests cover query parsing/cache separation/truncation;
timezone tests cover conversion. They do not prove exhaustive pagination,
real-world capture completeness or the DigiTrack integration.

During the API contract assessment, no HTTP request was made against real captured data, no capture session started,
no firewall changed, and no release binary or runtime implementation changed.
The existing endpoint integration-test target was inspected but not run: it had
no cached executable, and a new linked test variant was unnecessary for this
documentation-only contract assessment. Later deployment and exporter work is
recorded separately in the validation register. Full router-level integration remains
for the DigiTrack client smoke check on an isolated synthetic store.

### Implementation references

- [Routes and bearer/cache configuration](../crates/screenpipe-engine/src/server.rs)
- [Search contract and result conversion](../crates/screenpipe-engine/src/routes/search.rs)
- [Public evidence response types](../crates/screenpipe-engine/src/routes/content.rs)
- [Database selection, counts and pagination](../crates/screenpipe-db/src/db.rs)
- [Frame metadata/context](../crates/screenpipe-engine/src/routes/frames.rs)
- [Element provenance/visibility](../crates/screenpipe-engine/src/routes/elements.rs)
- [Meetings](../crates/screenpipe-engine/src/routes/meetings.rs)
- [Meeting transcript fields](../crates/screenpipe-db/src/types.rs)
- [Safe notice contract and tests](../crates/screenpipe-engine/src/privacy_notices.rs)
- [Timestamp middleware](../crates/screenpipe-engine/src/routes/timezone.rs)
- [Fixed frame markers](../apps/screenpipe-app-tauri/components/rewind/frame-capture-notice.ts)
- [Bounded context bundle, not an exhaustive record export](../crates/screenpipe-engine/src/routes/activity_summary.rs)
