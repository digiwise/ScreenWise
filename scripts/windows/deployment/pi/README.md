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

Run offline synthetic regressions from the repository root:

```powershell
node --test scripts/windows/deployment/pi/recording-api-core.test.mjs
```

The 15 tests use mocked fetch and fabricated JSON only. They cover route/header
scope, dates/IDs/pagination, rejection of arbitrary URLs and mutation arguments,
redirect policy, errors, timeout/abort, response/output limits, path/secret removal,
privacy notice omission, range boundaries, meeting overlap and partial reporting.
They do not validate a live recorder, model inference, OS firewall, capture privacy
or packaging. Verify registration through the explicitly provisioned SDK with
the bundled Bun executable and an absolute shared-runtime path:

```powershell
& '<absolute-path-to-bun.exe>' .\scripts\windows\deployment\pi\test-sdk-loader.mjs '<absolute-path-to-pi-agent>'
```

This loads the owned extension, asserts exactly one registered tool and executes
it against fabricated fetch responses. It starts no agent, model or recorder.
