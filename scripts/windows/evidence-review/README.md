# Private local API evidence review

This PowerShell 7 exporter retrieves the retained JSON evidence DigiTrack would
use for an explicitly selected period. DigiTrack should call the API directly;
these files support the owner's local inspection. Nothing is sent to a model,
cloud service or browser server. The script never starts recording or the capture GUI,
changes authentication/firewall settings, downloads media or opens API-returned
file paths. It uses `screenpipe auth token --data-dir` privately and only GETs
constructed allowlisted routes on literal loopback HTTP.

From the repository root, with an existing instance already running:

```powershell
pwsh -NoProfile -File scripts/windows/evidence-review/Export-ScreenWiseEvidence.ps1 `
  -Start '2026-10-06T09:00:00+11:00' -End '2026-10-06T10:00:00+11:00' -OpenReview
```

By default `.local/deployment/deployment.json` supplies `data_directory`,
`cli_executable` and `port`. Use `-ManifestPath` for a different manifest, or
`-DataDir`, `-ExecutablePath` and `-BaseUrl http://127.0.0.1:<port>` for another
existing instance. IPv6 `http://[::1]:<port>` is also allowed. Set `-SourceLabel`
when reconnecting to another store. Paths and labels scope source references;
there is no authoritative store UUID/reset generation in this API.

Dates require explicit RFC3339 offsets and are converted to UTC. The period is
nonempty, at most 32 days, includes its start and excludes its end. The default
output is a fresh `.local/evidence-exports/<timestamp>-<unique-id>/`; existing
output directories are refused. `-OutputDirectory` can select a fresh private
location. Treat every output as captured private data: never stage, publish or
upload it. Terminal output contains only counts/statuses and the output path.

The transport parser uses `System.Text.Json` to preserve original API timestamp
strings, including offsets and fractions of 1–9 digits, on PowerShell 7. It avoids
`ConvertFrom-Json`'s implicit conversion of timestamps to locale-formatted dates.
Comparisons truncate fractions beyond seven digits to .NET's 100 ns tick precision;
records and boundaries within the same tick cannot be distinguished. Original
source strings and the CLI's original requested dates remain in the export, and
the manifest records this precision limit.

Open `index.html` locally to review expandable modality records and limitations.
Use `-OpenReview` to open that generated file in the default browser/application
after the export is saved, including when retrieval was partial. If opening
fails, the export is retained and the script prints a warning. Without the option,
no review window is opened.
It contains escaped text, inline styling and no scripts or external resources.
Per-modality JSON retains records, source references, counts, status and notes;
`manifest.json` summarizes coverage. Exact bearer secrets and `file_path`/
`audioFilePath` metadata are removed recursively. Input `privacy_notice` payloads
are omitted; safe notices come from `/capture-events`.

The exporter retrieves OCR/UIA screen text, all retained source elements with
visibility metadata, audio transcripts, input events, meetings overlapping the
period, their time-filtered retained transcripts, and safe notices. Elements
without OCR frame timestamps use bounded metadata lookups and receive a resolved
timestamp when known. Full frame context is normally fetched on demand by
DigiTrack; opt into it with `-IncludeFrameContext` under the same request budget.
No raw image/audio is fetched. Missing timestamps remain uncertain.

Audio references prefer the background `transcription_id`; live audio uses the
negative chunk ID. For older API responses, references also include chunk,
offset and segment start/end. Multiple transcript rows can share a chunk and
offset, so deduplicating by those two fields alone loses retained text. The
legacy fallback remains unable to distinguish identical segment metadata.

`-PageSize` defaults to 100 (maximum 100), `-MaxPages` to 100 per paginated
modality, and `-MaxRequests` to 500 globally. Search offsets advance by requested
limit even when a page is empty; original totals are used. Meeting listing omits
the lower bound to discover older overlapping meetings and stops at a short page.
Notice ranges subdivide when `has_more`; request/page/depth limits or too many
equal-timestamp notices yield partial coverage. `-InitialLookbackDays` defaults
to 7 (maximum 32) for earlier notice context; initial state stays explicitly
unknown because no authoritative state-at-start field exists.

Statuses distinguish available records, empty success, partial retrieval, errors
and unrequested/unknown frame context. Partial data remains reviewable after a
failure. Errors contain fixed safe text; auth failure is not reported as an empty
success. Current process loss/degradation diagnostics are labelled separately
from historical notices. Concurrent inserts, late transcripts and the 60-second
search cache prevent snapshot-completeness claims. Whole-second API search
bounds are widened then exact dates are filtered locally. These observations do
not establish work or billable duration.

Run synthetic checks without a live API or recorder:

```powershell
pwsh -NoProfile -File scripts/windows/evidence-review/Test-EvidenceReview.ps1
```

Tests inject mock responses only, and leave a small fabricated review fixture in
the OS temporary directory. They cover pagination, auth error versus empty,
offset dates, overlap, notice subdivision/saturation, source references, metadata,
HTML escaping, token omission and overwrite refusal. They do not establish live
API compatibility, capture completeness, OS isolation or privacy guarantees.
Serialized JSON regressions also cover transport timestamp types, singleton
arrays, microsecond/nanosecond notices, original precision and exclusive bounds.
