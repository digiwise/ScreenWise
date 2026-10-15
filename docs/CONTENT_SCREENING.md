# Content screening and redaction

Status: **stage 1**, implemented with focused unit/integration tests and a
`release-local` build only. No live capture trial has been run. Detection is a
set of deterministic, best-effort pattern checks over OCR, accessibility and
window text: it can miss sensitive content and can redact or withhold ordinary
frames. It is not a privacy or security guarantee. Storage protection,
authenticated local APIs and network-egress controls are separate layers with
their own validation limits.

## What stage 1 does

With `contentScreeningEnabled` (default on), after every existing privacy, UIA,
target, lock and DRM check and after deduplication, the recorder runs OCR in
memory and screens the OCR text, accessibility text and persisted tree fields,
and window metadata (title, browser URL, document path) before anything from the
frame is written. The executable name (`app_name`) is not screened. Each frame
gets one outcome:

| Outcome | When | What is persisted |
|---|---|---|
| Pass | No hits | The frame as before; the screening OCR is reused, never run twice |
| Redact | Only redactable secret hits, all protectable | The frame with each affected OCR line blacked out (solid fill, padding max(25% of line height, 4 px)); hit spans replaced with `[SECRET]` in OCR text, accessibility text/tree and metadata; every word of an affected line replaced in the OCR word JSON |
| Whole-frame | Any blocker, unprotectable hit, threshold, error or screening failure | Windows: a source-free placeholder with a fixed reason; other platforms: the capture is skipped |

**Redactable secret hits:** `screenpipe-redact`'s `redact_one` Secret spans (API
keys and tokens, JWTs) with screening precision filters; issuer/Luhn-valid card
numbers in real card layouts (copies with other spacing or Unicode dashes are
matched by digits);
credential URLs (`user:pass@host`, including database connection strings); and
URLs whose query or fragment carries a sensitive name (`token`, `access_token`,
`refresh_token`, `id_token`, `api_key`, `apikey`, `key`, `secret`,
`client_secret`, `private_token`, `password`, `pwd`, `passwd`, `session`,
`sessionid`, `sid`, `auth`, `authorization`, `code`, `signature`, `sig`,
`x-amz-signature`, `x-amz-credential`, `x-amz-security-token`; whole names,
case-insensitive, no percent-decoding). In recorded text a sensitive URL keeps
its protocol (only if present; none is invented), host, port and path; all query
parameters, a sensitive fragment and user-information (URLs with a scheme) are
redacted, as is a detected secret inside the host, path or fragment. Sensitive
parameter values and user-info passwords of at least 8 characters with mixed
character classes are also matched as exact copies anywhere else in the frame
(for example a reset token shown on the page); each such value is a distinct
secret, so a URL with several sensitive parameters can exceed the 3-secret
threshold and be withheld.

**Whole-frame triggers:**
- blockers: ignored/protected (DRM) domains in frame text, password-manager UI,
  user keywords (`contentScreeningKeywords`), private-key blocks (BEGIN or END
  `… PRIVATE KEY` markers and similar, including OCR-damaged dashes; the loose
  form is uppercase-only so prose about private keys does not match; the key body
  cannot be located reliably);
- screening unavailable: OCR error (including a missing Windows OCR language,
  which withholds **every** frame until fixed; visible as
  `content_screening_unavailable`), oversize input, processing error;
- any hit in a structural tree field, which is never redacted (role, automation
  id, class name, role description, subrole, accelerator/access keys);
- on non-Windows platforms, any OCR hit (no word boxes are available);
- unprotectable hits: an OCR hit that cannot be located in the word boxes, or,
  while pixels are persisted, more copies of a secret in accessibility text, the
  window title and the document path combined than OCR located (copies are
  counted conservatively; the browser URL counts as one visible copy unless an
  accessibility node shows the same URL; link values that are a single URL are
  text-redacted but not counted as visible);
- an exact copy of a secret in the accessibility text that no tree field shares;
- thresholds: more than 3 distinct secrets, more than 3 plausible locations of
  one secret within any one source, redaction regions over 50% of the image or
  more than 256 regions;
- a failed text replacement (the shared replacement function reports explicit
  errors) or a post-redaction re-screen that still finds a hit.

**Occurrence coverage.** Every OCR location of a detected secret is masked,
including lines containing a fragment of at least 8 consecutive characters of a
detected detector secret. URL parts are matched only as exact copies inside other
URLs.

**Fail-open windows.** `uiaFailOpenWindows` (empty by default) lets a matching
window proceed after a UIA evidence-acquisition failure only while screening is
enabled, and in stage 1 only if the frame has **no screening hits at all**; any
hit withholds it. Patterns must use the `App::` or `App::Title` form, and the app part must
equal the executable name (case-insensitive, `.exe` optional); bare and
`::Title` patterns are ignored here because page content can influence titles. Matched
exclusions, private windows, incomplete trees, monitor/focus/identity/target
checks, lock, detected DRM content and screening failure never fail open. An
unverifiable DRM URL alone does not block fail-open (DRM app detection and
DRM-domain screening still apply).

**Stored-text redaction also changed.** Independently of this setting, the shared
`redact_one` detector used by the background PII redactor now recognises AWS
`ASIA…` keys, longer Google/GitLab/GitHub fine-grained tokens and more card
layouts (including cards followed by an expiry or year), narrows Maestro to real
issuer ranges and fixes Diners 300–305 / Discover 644–649 prefixes that never
matched. `REGEX_REDACTOR_VERSION` is 3 (it keys only the in-memory cache).

## Settings

| Key | Default | Meaning |
|---|---|---|
| `contentScreeningEnabled` | `true` | Screen every frame in memory before persistence |
| `contentScreeningKeywords` | `[]` | Extra case-insensitive substrings that withhold the frame |
| `uiaFailOpenWindows` | `[]` | `App::`/`App::Title` patterns allowed to fail open as above |

With screening disabled, capture decisions and decision metadata are unchanged
from before screening existed.

## Diagnostics and metrics

Windows decision metadata (`frames.capture_privacy`, see
[capture decision metadata](CAPTURE_PRIVACY.md)) gains `content_screening`:

| Field | Meaning |
|---|---|
| `outcome` | `passed`, `redacted`, `blocked` or `unavailable` |
| `reasons` | Fixed codes (`content_secret`, `content_keyword`, `content_password_manager`, `content_blocked_domain`, `content_protected_domain`, `content_screening_unavailable`) |
| `fail_open` | Whether the frame was admitted under `uiaFailOpenWindows` |
| `redacted_regions` | Number of blacked-out regions (redacted frames) |
| `ocr_forced` | OCR ran only because of screening (paired capture would otherwise have used accessibility text); absent only when screening was unavailable before OCR |
| `ocr_ms`, `screen_ms`, `redact_ms` | Whole milliseconds (saturating) for screening OCR (set even if OCR failed), text screening and tier decision, and redaction planning plus pixel fill (redaction only) |
| `hits_ocr_only`, `hits_a11y_only`, `hits_both` | Distinct detector-found secrets by where they were detected (window metadata, tree text and node fields count as accessibility; extra copies found by matching are not counted) |

`accessibility.processing` may be `screened_fail_open`; the internal
`screening_required` state is never persisted. Capture reasons:
`content_screening_blocked`, `content_screening_unavailable`,
`content_screening_redacted`, `accessibility_fail_open_screened`. OCR latency is
also recorded in the pipeline metrics behind `/health` (`avg_ocr_latency_ms`).
Diagnostics carry fixed codes, counts and durations only: never secrets,
fragments, hashes, titles, URLs or paths. Other platforms record no per-frame
screening metadata.

## Evaluating always-OCR

The question is whether forcing OCR on frames that accessibility text would have
covered (`ocr_forced`) is worth its cost. AGENTS.md permits agents to run only
read-only, aggregate queries over the fields below; open the recorder database
with a read-only SQLite URI (`file:…?mode=ro`).

```sql
-- Cost and yield of forced OCR per app per day.
SELECT date(timestamp) AS day,
       app_name,
       COUNT(*) AS screened_frames,
       SUM(json_extract(capture_privacy, '$.content_screening.ocr_forced')) AS forced_frames,
       SUM(CASE WHEN json_extract(capture_privacy, '$.content_screening.ocr_forced') = 1
                THEN json_extract(capture_privacy, '$.content_screening.ocr_ms') END) AS forced_ocr_ms,
       SUM(CASE WHEN json_extract(capture_privacy, '$.content_screening.ocr_forced') = 1
                 AND json_extract(capture_privacy, '$.content_screening.hits_ocr_only') > 0
                THEN 1 ELSE 0 END) AS forced_frames_with_ocr_only_hit
FROM frames
WHERE json_valid(capture_privacy)
  AND json_type(capture_privacy, '$.content_screening') = 'object'
GROUP BY day, app_name
ORDER BY day, forced_ocr_ms DESC;

-- Outcome distribution and average costs by text source.
SELECT text_source,
       json_extract(capture_privacy, '$.content_screening.outcome') AS outcome,
       COUNT(*) AS frames,
       AVG(json_extract(capture_privacy, '$.content_screening.ocr_ms')) AS avg_ocr_ms,
       MAX(json_extract(capture_privacy, '$.content_screening.ocr_ms')) AS max_ocr_ms,
       AVG(json_extract(capture_privacy, '$.content_screening.screen_ms')) AS avg_screen_ms
FROM frames
WHERE json_valid(capture_privacy)
  AND json_type(capture_privacy, '$.content_screening') = 'object'
GROUP BY text_source, outcome;
```

`forced_frames_with_ocr_only_hit / forced_frames` is the yield of always-OCR;
`forced_ocr_ms` is its cost. Frames skipped as duplicates are never screened, so
they add no OCR cost. Results contain app names (local activity metadata): keep
them out of public documents except as sanitized numeric summaries.

## Stage 1 limits

- Every persisted frame with a screenshot runs OCR (an estimated 50–300 ms of
  Windows OCR CPU, not yet measured) plus text screening, including frames that
  previously skipped OCR.
- Only the screenshot path is screened. `ui_events`, clipboard, keyboard rows and
  the macOS HD recorder are not.
- Non-Windows code paths are not compiled or tested on the development machine;
  macOS/Tesseract OCR errors may look like empty text and pass as blank frames.
- The in-memory frame comparer still sees original pixels (internal only).
- Building screening input and copying tree text still run on the async task;
  only OCR, rule screening, redaction planning and pixel fill run on blocking
  threads.
- The link-target exemption trusts the role names `Hyperlink`, `AXLink` and
  `Link`; links under other role names are counted and so can withhold frames.
- Editors whose accessibility tree contains off-screen text (e.g. a scrolled
  `.env` file), or controls whose text appears in both parent and child nodes,
  can cause whole-frame redaction because UIA bounds are not used yet.
- Common sensitive query names (`code`, `key`, `session`, `sid`, `auth`) also
  occur in ordinary URLs; such frames are redacted, or withheld when the
  address-bar copy cannot be located (e.g. a truncated address bar). URL tokens
  extend to the next whitespace, so adjacent punctuation can be redacted, and
  dotted names such as `notes.txt#code=1` are treated as URLs.
- A secret wrapped across lines is protected only where it was detected; format
  variants of a known secret other than card spacing are not matched; four
  stacked 4-digit OCR lines can be read as a card number.
- Partial matching can find fragments of a secret in unrelated text, which can
  push a frame over the location threshold.

## Learnings

- `paired_capture` writes the JPEG before running OCR, so any pre-persistence
  text check must run in `do_capture`, before paired capture.
- Windows OCR JSON has no line field; lines are inferred geometrically, so
  side-by-side columns can merge and be over-masked.
- UIA node bounds do not record their coordinate space (monitor or window
  relative, DPI, scroll clipping), which is why stage 1 cannot use them.
- Treating URL parts like random secrets made short values (`?auth`) match inside
  ordinary words; URL parts need provenance-aware exact matching.
- Private-key detection finds only the marker line, so key blocks must withhold.
- Two similar secrets (e.g. test AWS keys sharing 16 characters) link through
  partial matching; this is intended but trips the location threshold.
- `PipelineMetrics::record_ocr` existed but was not called on the event-driven
  path, so `/health` OCR latency was blank for normal recording before this change.

## Future considerations

- Screen the other text channels (`ui_events`, clipboard, keyboard, macOS HD).
- Replace `screenpipe-core::pii_removal` (inline PII removal) with `redact_one`
  (behaviour change; owner decision).
- Use the always-OCR metrics to decide a per-app forced-OCR policy.
- Persist periodic metric snapshots (UIA, audio, pipeline) for trials; including
  process names there would need an AGENTS.md exception.
- Compile and test the non-Windows paths.
- Add a launcher preset for offline lockfile refresh with diff verification, and
  fix the parallel-run flakiness of `screenpipe-config` global-state tests.

## Planned phases (agreed 2026-10-15, not implemented)

1. **Matching.** Fuzzy matching (secret length ≥12, ≤1 edit per 12 characters,
   capped at 2); drop the redundant re-screen in favour of the shared replacement
   function's explicit errors.
2. **URL credentials, full.** Opaque path/fragment identifiers, bounded decoding
   of encoded components, OCR-damage tolerance.
3. **Label-associated secrets.** Configurable labels (`Password`, `CVV`, `API key`,
   `Recovery code`, …) associate values by UIA relationships or OCR geometry,
   including unmasked passwords; label text is kept. The looser OCR card rule
   (no checksum/issuer) applies only adjacent to a card label, with date/time,
   IP, version and phone shapes excluded, card-like grouping required, and a
   regression corpus of ordinary developer/office screens.
4. **Profiles.** Four independent profiles (UIA standard/aggressive, OCR
   standard/aggressive) in one `contentScreeningProfiles` store object with
   defaults for missing values; no new UI. Aggressive modes add prefix-only
   credential detection, adjacent-line masking and 50%/8 px padding. Eligible
   fail-open frames may then be redacted under OCR aggressive instead of withheld.
5. **UIA geometry and blocker scope.** Record UIA bounds provenance (coordinate
   space, DPI, visible-area clipping) first. Then: UIA-only hits without an OCR
   copy get text redaction plus OCR aggressive, and whole-frame only for a
   visible focused control; per-monitor/window blocker hold (≥1 s after the latest
   observation) and clearance by two observed blocker-free UIA results ≥250 ms
   apart and ≤1 s old, using captures that happen anyway (no extra probes by
   default; at most one bounded probe per second per scope if enabled).

Each phase updates this document with implemented behaviour and evidence.
