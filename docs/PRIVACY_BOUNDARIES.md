# Privacy policy and recording boundaries

ScreenWise records local activity for later search and playback. Its privacy
controls decide which sources may be recorded, which frames may be saved and
which detected values should be removed. They do not identify everything a
person or organisation considers confidential.

This guide describes the current Windows recording policy, including deliberate
exceptions. ScreenWise is an unsupported developer experiment with incomplete
validation and no privacy or security guarantees. Read the
[project warning](../README.md) and the separate
[privacy and security issues](PRIVACY_SECURITY_ISSUES.md) before relying on these
boundaries. Other platforms and inherited capture paths do not have equivalent
validation. Policy snapshot: 17 October 2026, source revision `68a1be820`.

## The boundaries to understand first

- **An allowed application can contain confidential information.** Allowing VS
  Code permits ordinary source code and document text; screening is not a review
  of whether that work should be recorded.
- **Recording is not limited to the visibly exposed desktop.** Accessibility can
  supply obscured or off-screen text. Capturing an individual window can include
  portions hidden behind another window.
- **An excluded window does not pause every source.** Another monitor may still
  be recorded. Microphone, output audio and input records have separate controls.
- **Screening protects recognised patterns.** It can miss sensitive material,
  including unfamiliar secret formats and ordinary confidential prose.
- **Local access is still access.** Recording files and authenticated API clients
  can expose retained content. Exclusions do not revoke earlier recordings or
  copies already supplied to a client.

## What can be retained

The enabled capture settings determine which of these sources are collected.

| Source | Content that may be retained | Boundary to remember |
|---|---|---|
| Screen images | Monitor pixels or a selected window, sometimes with other areas blacked out | Individual-window capture can expose covered portions of that window. |
| OCR and accessibility | Recognised text, control text, roles, hierarchy and geometry | Accessibility is supplied by the application and can include text outside the visible viewport. |
| Frame context | App name, title, URL, document context and capture decisions | Titles and application identities can themselves be sensitive. |
| Input activity | Enabled keyboard, pointer and clipboard records and context | Frame screening does not screen these records. Keyboard and clipboard database capture default to off; their operations can still trigger a screen capture. |
| Audio and meetings | Enabled microphone/output audio, transcripts and meeting records | Screen exclusions are not a per-application audio isolation mechanism. |
| Derived content | Searchable records, summaries, chats, exports and caches | Removing source media does not necessarily remove every derivative. |

## Choosing which apps and windows may be recorded

Ignore rules match case-insensitive substrings. Use an app-scoped rule when you
want to distinguish an application from a word appearing in another title.

| Example ignore rule | Meaning |
|---|---|
| `excel::` | Exclude apps whose name contains `excel`, regardless of title. |
| `code::customer-project` | Exclude matching titles within an app whose name contains `code`. |
| `::confidential` | Exclude matching titles in any app. |
| `confidential` | Exclude an app name **or** title containing that word. |

These rules depend on the names the application exposes. A confidential document
with an unrelated title will not become excluded merely because its contents
are confidential. UIA can also find configured matches in nested windows and
documents; this is additional evidence, not general confidentiality detection.
UIA extraction replaces recognised password-control text with markers and treats
unknown password state conservatively for text-entry controls. That protection
depends on the provider's evidence and does not identify every password or secret
drawn in an ordinary editor or custom control.

**Include rules have two behaviours.** A bare include such as `code` restricts
otherwise eligible apps/titles to global matches. An app-scoped include such as
`code::work-project` restricts that app's matching windows, but does not by itself
exclude every other app. With only that scoped entry, other apps remain eligible.
Ignore rules still win. See the [pattern implementation](../crates/screenpipe-capture-policy/src/window_pattern.rs)
for mixed-list semantics.

Built-in protected-app checks exclude recognised password managers, credential
tools and certain system UI hosts. Private-browsing detection is configurable
and depends on supported evidence; it is not a universal detector of private
activity. The separate DRM/protected-content pause is also configurable and
does not cover every protected source.

## What happens when windows overlap

Windows capture checks application identity, exclusions, focus and monitor
association around acquisition. Depending on the result it saves a monitor,
captures one eligible window with the surrounding monitor blacked out, or saves
a source-free placeholder explaining why capture was withheld. Unknown or
changing required evidence normally withholds capture, subject to the explicit
exceptions below. These are sampled checks, not continuous observation.

| Scenario | Current policy |
|---|---|
| You work in VS Code with no excluded overlapping windows | Monitor pixels may be captured; accessibility text may extend beyond the visible editor viewport. |
| VS Code has focus and excluded Excel overlaps the same monitor | ScreenWise can capture VS Code alone. Its own surface can include the portion covered by Excel; Excel pixels are omitted. |
| Excluded Excel has focus on that monitor | That monitor's normal foreground path withholds capture; it does not switch to another background window there. |
| You work on monitor A; restored Excel partly covers an eligible VS Code window on monitor B | Monitor B can capture the first eligible window beneath Excel in stacking order. VS Code need not be maximised. Covered portions of VS Code can appear. |
| Excluded Excel is maximised or covers monitor B above the eligible window | It blocks selection of windows beneath it; monitor B is withheld. |
| Excluded Excel has focus on monitor A, with eligible windows on B | Monitor B can continue independently. Exclusion is not a global recording pause. |

The background-window fallback requires verified foreground ownership on another
monitor. It does not borrow that foreground window's accessibility tree or
labels. Post-acquisition checks and enabled content screening still apply.
Native handling of unlisted foreground popups also requires the popup and its
owner to be located off the captured monitor before proceeding.

Explorer taskbars are blacked out in admitted Windows images. Start menus,
previews, notifications and other flyouts are separate windows subject to normal
rules. A maximised-window selection has its own rechecks; it is not a general
permission to capture behind a maximised excluded window.

**Practical interpretation:** ScreenWise records eligible application content,
not just pixels a person could see at that instant. UIA text and window images
can both cross the visible-desktop boundary. Exclude an application or window
if its contents should not be retained, rather than relying on another window
covering it. See [capture decisions](CAPTURE_PRIVACY.md) for the detailed paths.

## Screening before a frame is saved

`contentScreeningEnabled` defaults to `true`. In the event-driven frame path,
ScreenWise runs OCR in memory and screens the image's recognised text,
accessibility text/tree fields and frame context before persisting the frame.

- A frame with no detected hits can be saved.
- Protectable secret hits can have their OCR regions blacked out and matching
  text replaced before saving.
- Blockers, unprotectable hits, excessive redaction or screening errors withhold
  the frame. Windows uses a source-free placeholder.

Detectors cover supported token/key formats, card patterns, credential URLs,
sensitive URL parameters, password-manager indicators and applicable domain
rules. `contentScreeningKeywords` adds case-insensitive substrings that withhold
a frame. For example, a synthetic project marker in this list can block a frame
when OCR or accessibility exposes it. A window ignore pattern is not
automatically copied into this keyword list.

A salary table, medical discussion or unreleased design may contain none of
these patterns. OCR can also miss text. A successful screening result therefore
means no implemented detector fired, not that the content is non-sensitive.

Frame screening does not screen microphone/output recordings, transcripts or
input rows. Optional background text/image PII workers run after storage and
are separate controls: they cannot promise that original content was never
stored or read. See [screening behaviour and limits](CONTENT_SCREENING.md).

## Explicit exceptions to missing evidence

### UIA fail open

`uiaFailOpenWindows` is empty by default. Adding `code::` or `msaccess::` permits
that executable's eligible frames to proceed after a UIA evidence-acquisition
failure, but only with frame screening enabled and **no screening hits at all**.
In the current stage, a fail-open frame with a hit is withheld rather than
redacted. Unlike ignore rules, the app part must match the executable exactly,
case-insensitively, with `.exe` optional. A bare `code` entry does not enable it.

For example, a large Access form may repeatedly exceed UIA limits. Without an
exception, unresolved required UIA checks withhold its frames. With
`msaccess::`, ordinary form contents can be recorded when native checks and
screening pass. This deliberately trades missing accessibility evidence for
useful capture. It does not make confidential form contents safe to store.

The exception covers only a retrieval failure during the privacy check itself:
a timeout, node or depth limit, provider error or backoff, as in the Access
example. A walk whose privacy check passed but whose final verification ran out
of budget (`accessibility_incomplete`) is always withheld. Known exclusions,
detected private windows and the independent identity, focus, monitor, lock and
detected DRM gates still block. With screenshots disabled, a fail-open frame is
withheld, because its admission depends on screening the stored image.
UIA backoff reduces repeated requests to expensive providers and can prolong
the period in which a configured app relies on this exception.

Individual unavailable UIA Value properties are also allowed as unknown
evidence; they are not assumed empty. Available evidence is still evaluated,
but an unavailable value can conceal a match. This exception is distinct from
allowing an entire failed acquisition to proceed.

### Apps with untitled windows

`untitledSafeApps` is empty by default and matches executable names exactly,
case-insensitively, with `.exe` optional. An entry such as `code.exe` lets that
app's untitled windows count as verified despite rules that could depend on
titles. App-name exclusions still apply. This is a trust decision about the
listed app's unnamed windows; it does not establish their contents are safe.

Without the override, an empty title causes uncertainty where configured rules
could read it. If only title-independent app rules apply, the missing title
alone need not block capture.

### Metadata when images are withheld

A frame withheld solely for an eligible UIA retrieval failure may retain the
verified native app name and a screened, PII-redacted title. This does not require
enabling UIA fail-open. For example, a title can reveal which project you opened
even though the image and accessibility text were withheld.

Titles are dropped for terminal hosts, unchecked URL exclusions, password-manager
names and screening hits, among other checks. Ordinary confidential wording can
survive. This title check runs even when frame screening is disabled. Other
withheld outcomes omit frame title/app/URL context, but structured
blocker metadata can still identify verified executable names. Generic notices
use fixed messages. Details are in [placeholder metadata](CAPTURE_PRIVACY.md#native-metadata-on-uia-failure-placeholders-2026-10-16).

## Audio input and stopping recording

Excluding a browser or editor does not mute the microphone. Microphone admission
is independent of visual content protection; output audio has a separate
protected-content gate. Neither provides verified per-app attribution of all
sound. For example, excluding a browser window is insufficient to exclude a
conversation audible to the microphone or guarantee removal of that browser's
sound from a mixed output stream.

Use the explicit all-recording stop/pause for a private interval, or disable the
specific source. Audio normally pauses while locked, with an explicit
`recordWhileLocked` exception. Schedule and source controls also apply, but
schedule modality and monitor-selection gaps are listed in the
[separate issues](PRIVACY_SECURITY_ISSUES.md).

Disabling keyboard/clipboard database capture does not prevent their operation
from triggering a screened screen capture. Disabling image storage is likewise
not proof that text acquisition stopped. A recording-status explanation is an
observation of a producer's gate; one gate recovering does not mean every
source resumed. The all-recording pause flag is session-scoped and is not a
persistent promise across process restarts. See [channel diagnostics](CAPTURE_DIAGNOSTICS.md).

## Local storage API access and deletion

The recorder is intended to process recordings locally and exposes its API on
loopback with bearer authentication. Unauthenticated `/health` returns minimal
readiness; detailed health and recording routes require authentication. Supported
model/tool acquisition is explicit; builds and provisioning can use the network.
Optional local AI can consume recording evidence and retain derived chat content.

The API token grants access to sensitive recorder capabilities. Loopback binding
does not protect against a local program that obtains it, and API authentication
does not protect files read directly through the operating system. Do not infer
encryption of all recording files from a token, settings encryption or a UI lock.
Keep the recording directory, tokens, exports and backups within your chosen OS
account and storage protections. Network restrictions are a separate deployment
control, not proof of confidentiality.

Generic status notices use fixed messages, but opt-in sensitive debug logging is
a separate diagnostic surface and can contain titles and other private context.
Treat those logs as private recording-related data rather than public bug-report
attachments.

An exclusion change affects future admission; it does not erase history. Media
eviction can preserve searchable text and other derivatives. Time-range deletion
is not a secure-erasure guarantee: shared media, caches, derived records,
filesystem failures and external copies need separate consideration. An API
client, export or chat may retain content already delivered to it. See the
[data control audit](DATA_CONTROL_AUDIT.md) and
[unresolved issues](PRIVACY_SECURITY_ISSUES.md).
