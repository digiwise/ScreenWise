# ScreenWise Pi recording retrieval

`recording-api.ts` registers the single `screenwise_recordings` tool using the
installed MIT Pi 0.75.4 SDK contract. `recording-api-core.mjs` contains its local
GET-only transport, range validation, filtering and output bounds. Both files
must remain together in deployed `assets/pi`. The desktop supplies
`SCREENPIPE_LOCAL_API_URL` (canonical `http://127.0.0.1:<port>` or
`http://[::1]:<port>`) and `SCREENPIPE_LOCAL_API_KEY`; the tool never accepts these
from model arguments. No package implementation was copied.

Load the explicit extension with `--no-extensions --extension <absolute-path>`
and select `--no-builtin-tools --tools screenwise_recordings`. Pi's `--no-tools`
also disables extension tools and is unsuitable for this retrieval mode.

Each request requires a valid timezone-bearing `start_time` and `end_time`,
ordered with an exclusive end and no more than 32 days apart. Kinds are
`screen_text`, `visible_elements`, `elements`, `audio`, `input`, `meetings`,
`meeting_transcript`, `frame_metadata`, `frame_context`, and `capture_events`.
The three detail kinds require a positive safe integer `id` and omit pagination.
Other kinds accept `limit` (1–100, default 50) and `offset` (default 0), except
capture notices use range subdivision instead of offset. Frame context first
checks metadata to avoid fetching out-of-range or timestamp-unknown context.

`audio`, `screen_text` and `input` also accept optional `q` (1–256 characters).
For model compatibility, the extension accepts a list of up to four bounded
strings and searches only the first; additional terms require separate calls.
Audio keyword searches return matching transcription rows, not a representative
row from a matching chunk. If an exact audio query has zero matches, the tool
scans at most ten pages/1,000 unfiltered rows in the same requested range under
the existing ten-second deadline. It looks for a one-character spelling variant
of a distinctive term of at least six characters, or word-spacing variants, and
includes up to five neighboring rows on each side. For multiword input it uses
the longest term. This deliberately approximate recovery is labelled
`keyword_fallback` and `keyword_match_mode=possible_variant`; it does not prove
an exact keyword match. Without a variant, only bounded unfiltered candidates
are returned and labelled accordingly. Remaining pages have `next_query`.

Background audio has `audio-transcription:<transcription_id>` references; live
audio uses its negative chunk ID as `live-audio:<id>`. Compact model output keeps
one copy of the transcription and the original `capture_timestamp`.
`local_segment_timestamp` adds the retained segment start offset to that
timestamp in the query's timezone, in code rather than in the model. This is a
display convention using stored offsets, not independently validated timing of
the words. References still require a separately configured store identity.

Results preserve original pagination, unfiltered candidate counts, safe notice
diagnostics and explicit partial/truncation notes. Continue pagination even when
local filtering makes a page empty: advance offset by the requested limit.
Search bounds widen to whole seconds to avoid subsecond cache ambiguity, then
filter locally. Original micro/nanosecond timestamps remain in results; JavaScript
comparisons use milliseconds, so finer boundaries remain uncertain.
Meetings are overlap candidates, transcripts
are filtered by `capturedAt`, and missing timestamps remain uncertain. Raw input
`privacy_notice` rows are omitted; request `capture_events` for safe notices.
File path metadata and the exact bearer secret are removed recursively. Responses
are capped at 512 KiB of JSON and roughly 32 KiB of model text; a truncated page
requires a smaller query. Frame context can include off-screen retained text.
These observations do not establish work/billing duration or complete coverage.

The extension distinguishes invalid arguments from unavailable retrieval and
returns structured errors with `data:null`. Its completed-message guard replaces
unsupported empty/error answers with factual uncertainty. For requested audio
transcripts, it renders stored wording, references and times directly; possible
variant recovery retains the nearby returned context. It removes tool-use
preambles. Recording-mode desktop streaming holds draft `message_update` events
until the guarded final answer; tool progress and completion still pass through.
Completed guarded text is delivered once through the existing delta path before
`message_end`, so both foreground and background chats retain the answer.
The desktop's automatic title request uses a separate title-only system prompt
with no active tools. Recording tools are restored for ordinary prompts. Title
labels are bounded to 50 characters and unsupported absence labels get a neutral
review title, rather than claims that audio is missing.
Explicit English `between ... and ...` ranges with a named/ISO date (or
today/yesterday) and AEDT/AEST/UTC offset constrain tool requests. Other natural
language date forms still depend on the model. These guards do not certify
relevance, model summaries, transcription accuracy or complete coverage.

Run offline synthetic regressions from the repository root:

```powershell
node --test scripts/windows/deployment/pi/recording-api-core.test.mjs
```

The 25 tests use mocked fetch and fabricated JSON only. They cover route/header
scope, dates/IDs/pagination, rejection of arbitrary URLs and mutation arguments,
redirect policy, errors, timeout/abort, response/output limits, path/secret removal,
privacy notice omission, range boundaries, meeting overlap, bounded spelling
recovery, explicit requested-range enforcement and completed transcript guards.
They do not validate a live recorder, model inference, OS firewall, capture privacy
or packaging. Verify registration through the explicitly provisioned SDK with
the bundled Bun executable and an absolute shared-runtime path:

```powershell
& '<absolute-path-to-bun.exe>' .\scripts\windows\deployment\pi\test-sdk-loader.mjs '<absolute-path-to-pi-agent>'
```

This loads the owned extension, asserts exactly one registered tool and executes
it and its message hooks against fabricated fetch responses. It starts no agent,
model or recorder.

For opt-in local inference, reuse an already provisioned Bun/Pi/Ollama runtime:

```powershell
python scripts/windows/deployment/pi/check-recording-agent.py --bun '<absolute-bun.exe>' --runtime '<absolute-pi-agent>' --output-directory '<fresh-private-output-directory>'
```

This exercises matching context, empty success and API failure with an
authenticated synthetic loopback server. It refuses an existing output directory
and keeps separate configuration/chats per case while sharing immutable packages
and existing model weights. It verifies the pinned package, entrypoint,
dependencies and link-free runtime path, and rejects output inside that runtime.
Five inert identity/output-isolation checks run with
`python scripts/windows/deployment/pi/check-recording-agent.test.py`.
No capture, firewall changes or downloads occur.
Add `--cases title context empty error` to include the automatic title regression.
Use `--prompt-file <path>` for prompt trials without rebuilding the app.
The desktop embeds the settled `recording-prompt.txt`; its current local clock
and offset are appended at Pi startup. Deploy the matching extension assets
alongside the rebuilt GUI; exact-source checks reject stale assets.
