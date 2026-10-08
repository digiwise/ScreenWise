# Captured data control decisions and next steps

Discussion record updated 8 October 2026. This records the owner's agreed
direction, proposals and unresolved details. It is not a claim that the described
controls are implemented, nor authorization to deploy, start recording or change
firewall settings. Implementation readiness belongs in the validation register.

## Terminology

- **Captured data** is the umbrella, including captured content and derived data.
- **Data types** are selectable categories, such as microphone audio, transcripts,
  screen images or OCR.
- **Data source** identifies origin, such as a particular microphone or monitor.
  Use this consistently rather than alternating with capture source.
- **Computer audio output** means captured computer playback, including headphone
  or loopback playback; it does not imply audible speakers.
- **Keep** and **Discard** are actions; **retention policy** describes defaults
  and lifecycle behaviour.
- **Multiple or Unknown** is the additional monitor selection for captured data
  that cannot be attributed to one monitor. Details should distinguish multiple
  monitor attribution from unknown attribution. All monitors includes it.
- Stream and channel have not been selected as umbrella UI terminology.

The distinction between policy and rule is recommended but not yet explicitly
settled: a policy specifies behaviour, while a rule specifies a matching condition
and scope. Likewise, interface data is a proposed user-facing grouping;
accessibility describes an acquisition method, not a synonym for all interface
information. Interface text includes displayed content, labels and field values;
interface structure includes roles, relationships, bounds and states. Structural
fields can still reveal sensitive information.

## Data inventory and relationships

| Data type | Source and finer selections |
|---|---|
| Screen images and video | Individual monitor, all monitors, Multiple or Unknown |
| OCR | Derived from screen images; preserve source relationships |
| Interface text and structure | Accessibility acquisition or other verified extraction; frame/window/monitor association where known |
| Microphone audio | Individual input device or all microphones |
| Computer audio output | Individual output device or all output devices; mixed playback may prevent app-specific isolation |
| Audio transcripts | Associated audio, device, segment and conversation where verified |
| Keyboard | Typed text, individual keys and shortcuts |
| Mouse | Clicks, movement and scrolling |
| Clipboard | Captured content and copy/cut/paste operation metadata |
| App and window activity | App activation/switches, window focus, app identity/PID and associated title/URL context where available |
| Other derived records | Speaker associations, meeting/conversation records, summaries, search entries and caches where implemented |

App/window activity is an event history, not proof of continuous attention or a
complete browser history. Content-bearing titles and URLs may need separate
selection from app-switch/focus metadata.

Frame-associated accessibility records already have monitor provenance, but a
window can span monitors, a tree can include off-screen controls, focus can change,
and records can lack a reliable association. Monitor ownership, visible location
and frame acquisition origin are different relationships. Preserve uncertainty;
do not silently infer attribution. Monitor-related groupings must support one,
several or all monitors and Multiple or Unknown.

## Recording status and explanations

Any state below active recording must have the most precise practical, secure
reason. Distinguish disabled, paused, partial capture, no signal, failed capture,
failed persistence and processing/transcription delay. A running callback with
zero samples or silent input is not sufficient evidence of useful recording.

Explanations should identify onset, affected data types/sources/monitors, changed
reason, and resumption/end so the period can be reconstructed. Persist explanations
independently of capture and deduplicate unchanged conditions. Surface failures
to save explanations.

Useful detail includes the triggering policy/rule identifier, rule kind and
configuration origin; the failed operation, safe exception category and OS error
code; verified executable identity; and whether identity changed during a check.
Do not copy raw exception text containing private paths, titles, URLs, credentials
or captured content into generic logs. Resolve sensitive rule details through
protected local configuration rather than generic notices. Never reuse stale
foreground identity when the current check fails.

Silence explanations should distinguish absent callbacks, exact-zero samples,
low signal and no detected speech. Capture permission, actual acquisition,
persistence and transcription availability need separate observations.

Use the existing per-frame privacy metadata and interval diagnostics as the
authority for a future UI. Start with tray details; the optional always-on-top
panel is parked until the status model is usable. Notify on meaningful changes,
failure or required user action, rather than every unchanged poll.

## Capture policy boundaries

Existing visual exclusions must not suppress microphone recording. Explicit
pause all recording must suppress everything. Preserve lock policy, schedule,
microphone disablement and manual pause protections. Foreground uncertainty is
not proof of protected content and needs truthful reasons and recovery.

An excluded foreground window on one monitor does not automatically prohibit
safe capture on another. Protect excluded windows wherever visible, including
background and spanning windows; preserve fail-closed handling for uncertain
affected areas. Actual protected-content handling remains a separate policy.

Computer audio output requires independent consideration: if app playback is
mixed, reliable exclusion of one application's sound may be impossible without
pausing that output source. Keyboard/clipboard policy should follow verified
event context; mouse policy should account for interaction with excluded windows.
These broader rule scopes require explicit decisions rather than silent migration.

## Keep and Discard selections

Default Discard includes all related captured data from the selected data source
or grouping, including derivatives such as OCR with screenshots. Fine-grained
selection can override the default down to individual data types and detailed
subtypes. The UI must disclose what remains when an override keeps related data.

Common selections are everything, all audio, microphone only, computer audio
output only and custom selection. Accepted groupings for exploration include:

- Visual and interface data, expandable into images, OCR, interface text/structure
  and their derivatives, with monitor selection.
- Broad input data, expandable into keyboard, mouse and clipboard subtypes.
- Activity metadata, with content-bearing context independently selectable.
- Conversation audio, grouping microphone and computer audio output when they
  belong together, but allowing separate device/type selection.

Actions include Discard so far (future capture continues), Discard this conversation
including its remainder, and Discard a previous conversation or interval, including
selection of one from a specified number of minutes ago. Estimated boundaries
must be reviewable and adjustable. Capturing temporarily is distinct from consent
to keep; a Keep action does not establish consent from other participants.

Audit the existing deletion relationships before extending them. Discard must
handle source records, transcripts, OCR, indexes and caches consistently. Mixed
source summaries require removal or regeneration from kept material. Exact
fine-grained exceptions and any content-free discard audit entry remain to be
specified. Discard means immediate permanent deletion after careful confirmation,
with no undo holding period. Confirmation must show the interval, data sources,
data types and related derivatives affected, including anything an override keeps.
Discard applies to accepted storage and pending-review storage alike. Overwrite
content before deletion where practical as a best-effort measure; do not promise
removal of every remnant from SSDs, SQLite journals, backups or other copies.
Deletion failures and incomplete cleanup must remain visible rather than reporting
successful permanent deletion prematurely.

## Retention policy and pending review

Review before keeping and Keep by default are separate policy settings. In review
before keeping, hold captured data temporarily for configurable days (for example,
seven days). Warn as the discard deadline approaches, with proposed reminders at
three days, one day and two hours. Snooze extends the discard deadline, not merely
the reminder. Accepted Snooze options are two hours, one day, three days, seven
days and a custom date/time, with no limit on repeated snoozing. Show the new
deadline and recalculate reminders against it, skipping thresholds already passed.

Pending-review captured data must be redacted from APIs with an appropriate reason.
This requirement includes derived content and must not be bypassed by search,
frames/media access, meeting transcripts, caches or consumers such as DigiTrack.
The live system must report **pending review** when a request overlaps withheld
captured data, including an audio transcript request for such a period. It must
not report no captured data merely because content is withheld. For mixed intervals,
return accepted results and identify the withheld portions using safe time/source
metadata without content. For example: "Audio was captured during this period but
is pending retention review. Its transcript is withheld. Open review to Keep,
Discard or Snooze." Exact API response schema and permitted metadata fields remain
to be specified; this behaviour applies regardless of storage architecture.

The accepted preferred architecture is a separate review store with a dedicated
authorized local review interface. Ordinary APIs/consumers cannot retrieve pending
content; the review interface can show/play selected captured data and derivatives.
Keep transfers the selected data and derivatives into accepted storage before
ordinary API exposure. Discard removes selected data from either store. Snooze
extends its review deadline without opening consumer access. Ordinary DigiTrack
and Codex retrieval remain blocked; assistant access to particular pending items
requires separate explicit review authorization.

Use the audit to assess separate database/file storage feasibility before settling
the physical layout. Investigate partial selection, shared media files, cross-store
references and recovery after interrupted Keep. If complexity is excessive, park
physical separation and consider a single store with explicit review states only
if a central access boundary can reliably cover all content paths. This fallback
does not weaken pending-review redaction or the dedicated review authorization.

Deadline expiry is expected to discard unapproved captured data; exact expiry,
offline/startup handling and deletion failure behaviour need specification.
Storage pressure must prompt Discard and never force it. If recording stops because
storage is exhausted, alert the user and persist the reason where possible. This
does not cancel a configured review deadline or an explicitly chosen age-based
cleanup policy; precedence and safeguards need explicit design.

## Existing foundations and accepted deferrals

| Existing implementation or proposal | Agreed treatment |
|---|---|
| Time-range deletion and timeline selection | Extend for prior conversations and data-type/source selection |
| Age-based retention worker/settings | Preserve; add pending-review lifecycle alongside it |
| Media-only eviction preserving searchable text | Treat as storage reclamation, not Discard |
| Disk usage, storage preview and cleanup confirmation | Reuse in a related but separate storage workstream |
| Frame privacy outcomes and interval diagnostics | Reuse for status UI, rather than adding independent detectors |
| Monitor IDs and association metadata | Reuse, with Multiple or Unknown and honest attribution limits |
| Structure-only interface retention | Park pending audit of text/structure dependencies |
| Metadata-only exclusions | Park until explicit scopes and exclusion migration are settled |
| Final field text capture | Park as a separate acquisition expansion |
| Accessibility cleanup/deduplication | Separate interpretation work; prioritise keeping synthetic placeholders out of content results |
| Meeting IDs on audio search rows | Later association improvement; do not label time overlap as verified membership |
| Always-on-top status panel | Park; explore tray detail surface first |

Existing retention configuration defaults are disabled, fourteen days and media
mode; this is source-code precedent, not a claim about the live configuration.
The existing deletion request selects a time range, not monitors or data types.
Current paths must be audited for coverage, file chunk boundaries and derivatives
before promising granular deletion. Review queues, deadline warnings and snoozing
are new lifecycle requirements.

Existing privacy settings include window/app patterns, URL exclusions, input
enablement, protected-content pause and PII masking. Migrate them by asking the
owner to **Use Codex**: inventory existing rules, show explicit old/new scope and
behaviour, resolve every decision and apply the reviewed mapping. Do not silently
weaken protections. No migration is authorized by this discussion record alone.

## Completed audit findings

The source-only [data control audit](DATA_CONTROL_AUDIT.md) completed and was
committed as `0a4c54f72`. No recordings or live stores were inspected. It examined
concurrently changing source; reconcile its conclusions with the final implementation.

- No persisted pending-review lifecycle or common content-release gate exists.
  Capture privacy and PII masking do not enforce retention approval.
- Time-range deletion can leave selected content inside shared media chunks.
  Memories and speaker embeddings can survive; untranscribed audio and failed
  filesystem deletion also have cleanup gaps. Existing deletion is not sufficient
  to promise complete permanent Discard.
- Pending-review enforcement must cover raw SQL, streaming/live events, direct
  desktop media/assets, exports/backups, derivatives and caches, including consumer
  caches. Search filtering and bearer authentication alone are insufficient.
- Monitor attribution is incomplete across data types. Frame acquisition origin
  does not prove every associated element belongs exclusively to that monitor.
- Media-only eviction preserves text and derivatives by design and remains
  storage reclamation, distinct from Discard.

Before retention implementation, specify shared-chunk handling, derivative lineage,
recoverable deletion jobs with visible failure status, and a complete access boundary.
The preferred separate review store must be evaluated against those findings; it
does not by itself solve accepted-data deletion or previously exposed consumer copies.

## Completed design contract and builder work

The [retention and access contract](DATA_CONTROL_CONTRACT.md) is committed as
`7549a2203`. Sol 6.1 High design work and independent Astra High review resolved
six substantive findings. It proposes separate review storage plus a common
access boundary and complete sealed capture bundles as the first enforceable
selection scope. Fine conversation/monitor/data-type selections are refused until
byte isolation and lineage can enforce them. These are reviewable proposals, not
owner approval of reduced initial scope or application implementation.

Owner decisions remain on initial scope/fallback UX, review permissions and SQL/
asset mediation, backup/legacy migration, expiry/recovery semantics, content-free
audit/tombstone retention, consumer-copy completion, restore freshness and reliable
conversation boundaries. Review the contract's explicit decision list before
authorizing its proposed synthetic ledger/media-isolation prototype.

The separate builder work is committed as `16aee39a9` on
`codex/windows-build-concurrency` in an isolated worktree. Its opt-in `-Concurrent`
supports root/desktop overlap while preserving same-cache and desktop-staging
exclusion and default serialization. Synthetic checks passed: 12 lock, 97 launcher,
18 cache and 84 trial-profile assertions. It has not been integrated into this
checkout or used for the production builds above. Real-build speedup and PowerShell
5.1 execution remain unvalidated. Integration review is separate from deployment.

## Suggested next steps

1. Capture-policy implementation is committed as `3ea64c8c7`, including diagnostics,
   unaffected-monitor capture and microphone independence. Focused checks,
   independent Astra High review, frontend checks and both `release-local` builds
   completed. Both production release builds and 54 synthetic deployment-script
   checks have now passed; see [validation](../VALIDATION_REGISTER.md). Deployment,
   hardware microphone signal and live transition validation remain separate.
2. Review the completed audit's coverage matrix and reconcile it with settled
   capture-policy source. Resolve its shared-media, derivative, cleanup and monitor
   attribution gaps in the design; assess separate review storage and granular
   deletion feasibility before planning the retention backend.
3. Review the completed retention/access contract and its unresolved owner
   decisions. It specifies pending-review reasons, mixed results, local review,
   Keep/Discard transitions, copy handling and recovery. Agree the initial scope
   and storage/access architecture before authorizing a synthetic prototype.
4. Work through a conversation example to settle default groupings, fine-grained
   overrides and boundaries. Apply the accepted unlimited Snooze options and
   permanent Discard confirmation; settle expiry/startup behaviour, reminder
   persistence, shared-file deletion and best-effort overwrite limitations.
5. Define explicit policy/rule scopes and the Codex-assisted migration procedure.
6. Design the tray status/review interaction against these contracts; implement
   the retention backend before presenting controls that promise enforcement.
7. Assess storage pressure separately using existing mechanisms, including alerts,
   stopped-recording persistence, voluntary cleanup and interaction with deadlines.

Steps beyond currently authorized implementation are recommendations, not new
instructions to the other chat.

## Related implementation references

- [Capture privacy and monitor provenance](CAPTURE_PRIVACY.md)
- [Capture interval diagnostics](CAPTURE_DIAGNOSTICS.md)
- [Local retention worker](../crates/screenpipe-engine/src/retention.rs)
- [Data deletion and storage preview API](../crates/screenpipe-engine/src/routes/data.rs)
- [Retention settings](../apps/screenpipe-app-tauri/components/settings/retention-settings.tsx)
- [Privacy settings](../apps/screenpipe-app-tauri/components/settings/privacy-section.tsx)
- [Input event model](../crates/screenpipe-a11y/src/events.rs)

## Newly authorized parallel work (2026-10-08)

The owner requested a recording-status GUI ahead of optional review/retention
controls, concrete repairs to major existing deletion gaps, and a concurrent
investigation/fix of unnecessary desktop rebuilds. Status must expose precise
safe reasons and avoid claiming unimplemented retention enforcement. Deletion
repairs preserve existing selection scope; material storage/access architecture
changes remain decisions in the contract. Desktop build inputs must remain
unchanged until the running build finishes.
