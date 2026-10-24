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

## Credential files and prefixless tokens (2026-10-24)

Three additions, each with focused unit/integration tests only (no live trial,
no `release-local` build by the implementing agent).

**Credential-file window titles** (`screenpipe_capture_policy::credential_files`).
A window whose title names a credential file is excluded exactly like a window
matching `ignoredWindows`, on every capture path and whatever
`accessibilityWalkDisabled` says: window enumeration for full-monitor and
active-window capture (the rule reads the native title of every enumerated,
non-minimised window overlapping the monitor, not only the foreground one;
`configured_exclusion` code, so a visible background window is blacked out
and forces active-window-only capture and a foreground one gives a
placeholder without title, app or URL), the accessibility walker's native
policy (before the walk-disabled check), nested Window/Document names, the
engine's final ignored-window gate, UIA-failure placeholder metadata, the
input recorder (keyboard, clipboard, click and focus rows and their capture
triggers) and the shared contact gate. A title that cannot be read during window
enumeration is now unknown metadata (fail closed); an empty title is not. The
rule is built in and always on, matches file-name shapes, never bare words,
and ignores editor decorations (`*`/`●` unsaved markers, quotes, brackets,
` - App` suffixes, directories). Built-in list, case-insensitive on each title
token's file name:

- exact names: `.env`, `.npmrc`, `.pypirc`, `.netrc`, `_netrc`,
  `.git-credentials`, `.pgpass`, `.htpasswd`, `wallet.dat`, `id_rsa`,
  `id_ed25519`, `id_ecdsa`, `id_dsa`;
- `.env.<suffix>` except the templates `.env.example`, `.env.sample`,
  `.env.template`, `.env.dist`;
- extensions `.secret`, `.pem`, `.key` (also Keynote decks), `.p12`, `.pfx`,
  `.kdbx`, `.ppk`, `.jks`, `.keystore`, after a non-empty stem (a bare `.pem`
  in "Convert .pem to .pfx" names a format, not a file); an exact name such as
  `.env` or `.npmrc` is excluded wherever it appears in a title, including a
  documentation page titled `Configuring .npmrc`;
- a data/config extension (`txt json yaml yml toml ini cfg conf config env xml
  csv properties plist dat bak old enc gpg asc age`) with a stem containing the
  whole word `token`, `secret`, `secrets`, `credential(s)`, `password(s)`,
  `passwd` or `apikey(s)` (words split at `.`, `_`, `-`, digits and camelCase;
  `api_key` counts as `apikey`; the plural `tokens` is deliberately absent);
- `credentials` only as the last component of a path (`~/.aws/credentials`) or,
  in lower case, as the whole first title segment (`credentials - Notepad`).

Not matched: `Secret Santa Planning.docx`, `Manage your credentials`,
`Credentials | Console`, `token_service.rs`, `tokenizer.json`,
`tokens.json`, `design-tokens.json`, `id_rsa.pub`, `.envrc`. A name with
spaces is judged by its last word (`Team passwords.txt` matches, `Secret
notes.txt` does not); file-name-shaped ordinary names such as
`secret-santa-list.txt` or `password-policy.txt` do match.
`credentialFilePatterns` adds owner globs (`*`, `?`, case-insensitive, whole
file name or whole title segment). Credential-file matches add no rule reference
to the capture diagnostics (the reason is the usual excluded foreground or
background); a dedicated rule kind would need a coordinated frontend change.
Nested Window/Document names are checked in every app: the accessibility walk
now always requests the subtree-rule evidence (names of nested Windows and
Documents, Document values), as it already did wherever a configured title
rule applied; a missing required name fails closed as before. Input events
are admitted against both the observer's cached title and the foreground
window's live title (read without a window message), so a credential file
opened in a tab of an already focused window is excluded at once.

**Generic tokens near credential words** (`generic_token_hits`). A run of
letters, digits and `+ / _ -` (with optional `=` padding) is a redactable
secret when it has at least 32 characters, upper case, lower case and a digit,
is not hex-only (git SHAs, digests, UUIDs with or without hyphens), has Shannon
entropy of at least 3.5 bits per character, changes character class at 30% or
more of adjacent positions and has lower-case runs averaging under 3.5
characters (camelCase identifiers and paths), **and** a credential word
(`token`, `secret`, `password`/`passwd`/`passphrase`, `credential`, `bearer`,
`authorization`/`authorisation`, `api key`, `access key`, `private key`,
`vault`, `.env`; also inside identifiers and OCR-split text) lies within 16
words of it, or the frame's window title names a credential file. Native
window titles also lend context (weaker than the title rule: they never
exclude a window): when the frame window's native title has a credential
word, its own texts (title, URL, document path, accessibility text, and the
OCR text of an active-window-only frame) are in context; on a full-monitor
frame only the OCR words whose centre lies inside a visible window whose
enumerated native title has a credential word are. Title-lent context does
not reach strings inside URL tokens (document and page IDs in address bars);
a nearby credential word still does. On a full-monitor frame the natively
enumerated titles of the other admitted windows overlapping the monitor are
screened too: any screening hit in one of them (a secret, keyword,
password-manager brand or blocked domain) withholds the frame (that title bar
is on screen and cannot be redacted as metadata). Windows are treated as
visible when enumerated, not minimised and not cloaked; occlusion is not
known, so a covered window can over-block. These titles stay in memory for screening only and are never
logged or stored. Labelled base64 integrity
digests (`sha512-…`, `sha384-…`, Go `h1:…`, `sha256:`/`sha256=` values) are
not candidates. Its hits follow the
existing tiers: redacted when the redaction tier allows, otherwise (fail-open
frames, thresholds, unlocatable copies) the frame is withheld. Existing
detectors are unchanged; generic hits never overlap them. Synthetic
measurements: 0 hits, both with proximity context and with title-lent context, on 2,418
false-positive fixtures placed beside credential
words (git SHAs, short SHAs, UUIDs, hyphen-free UUIDs, upper-case UUIDs,
22/27-character base64 IDs, sha256 digests, npm/SRI/Go base64 integrity
digests beside `jsonwebtoken`-style names, build-log lines, paths, URLs,
identifiers, prose); hex never hits even in a credential-file frame. Detection
of uniformly random tokens beside a credential word is asserted at no less than
97% (32 base62 characters), 98% (36 base62, 40 base64, 43 base64url) and 99%
(48 base62); a calibration run of the same rules measured about 99.4% for
32-character base62 tokens. Known limits: a 32+ character random-looking ID (for example a
long base64 document ID) beside a credential word is redacted or, on a
fail-open frame, withholds the frame; tokens shorter than 32 characters, split
by OCR into shorter pieces, single-case or without a digit (for example
upper-case base32 seeds) and hex-only tokens (32/40/64-hex API keys) are not
detected by this rule. Only the window title counts as credential-file
context; a credential file named elsewhere in the frame does not.

**Re-screening stored data** (`screenpipe privacy rescreen`, owner-run only).
Applies the current rules (credential-file titles, protected apps,
`ignoredWindows`, `ignoredUrls`, content screening with
`contentScreeningKeywords` and, with `pauseOnDrmContent`, DRM domains) to every
stored frame (app for the window rules; title, URL, document path,
accessibility text, tree JSON and accessibility element texts as the frame
window's texts; OCR text with its stored word positions, OCR word JSON, full
text and OCR element texts as image texts) and input event (title, URL, text,
element name/value/description, screened as a frame of its own window).
`includedWindows` is not re-applied. Screening always runs here, even if
`contentScreeningEnabled` is off. Capture time and the re-screen make no
decision of their own: both call the shared frame decision
(`screenpipe_redact::frame_screening::screen_frame` and its tier) and the
shared window rules (`screenpipe_capture_policy::target`: `TargetFilter`,
`ignore_exclusion`); a test runs the same fixtures through both callers.
Stored rows keep no window enumeration, so the visible-window context of a
full-monitor frame cannot be re-applied: such frames' image texts only get
proximity context on re-screen. A stored text larger than the in-memory
screening bound is screened in overlapping windows (capture time withholds
such a frame instead). Frames are deleted through the same frame-set
deletion as range deletion (`db::delete_frame_set`); only the re-screen keeps
a device's newest, possibly still recording, video chunk.

```text
screenpipe privacy rescreen --data-dir <recording data directory>
screenpipe privacy rescreen --data-dir <same directory> --apply --confirm <fingerprint>
```

The first command is a dry run: it changes no data, prints scanned counts and
`<table>/<action>/<reason>` counts (`content_secret_redactable` marks frames
whose only hits capture-time screening could have redacted in place), and
saves a content-free plan
(`<data-dir>/rescreen/plan.json`) with a fingerprint. `--apply --confirm`
applies only that plan: rows up to the planned highest ids, only while the
rules are unchanged (otherwise run the dry run again), in batches of
`--batch-size` (default 100, at most 200) rows, each written in one short
`IMMEDIATE` transaction with `secure_delete` on, pausing `--pause-ms` (default
100) between batches. Progress is saved after every batch
(`rescreen/progress.json`); `--max-rows` bounds one invocation and repeating the
command resumes. `--data-dir` has no default and a missing database is refused,
not created; a data directory without settings (`store.bin`) is refused unless
`--allow-default-rules` is given, and both runs print where the rules came
from and their counts. Actions: a captured frame that would now be blocked is deleted with
its OCR rows, elements (a shared element anchor moves to the first kept frame,
as in range deletion) and search-index entries, and its video chunk or snapshot
is queued in `media_deletion_jobs` and removed when no frame still uses it.
A device's newest video chunk (possibly still recording) is never removed: an
emptied one is listed in the progress file and released by a later run once
recording has moved to a new chunk (the plan completes only then); a
placeholder frame only loses app, title, URL and document path; an input event
is deleted, or its secret spans are replaced with `[SECRET]` and the row
re-screened. Output and state files hold counts, fixed reason codes, ids and
hashes only.

Limits: stored pixels cannot be redacted in place, so a deleted frame whose
video chunk is shared with kept frames leaves its pixels in that file (counted
as `deleted frames whose video chunk is shared`); deleting a time range is the
existing way to remove them. Old text can survive in SQLite free pages, FTS
segments not yet merged and the WAL until those are rewritten (a later
checkpoint/`VACUUM` reduces this). Audio transcripts and memories are not
re-screened. It opens the database with the normal migrations, so run it with
the same build as the recorder. Tested only on synthetic databases.

## Settings

| Key | Default | Meaning |
|---|---|---|
| `contentScreeningEnabled` | `true` | Screen every frame in memory before persistence |
| `contentScreeningKeywords` | `[]` | Extra case-insensitive substrings that withhold the frame |
| `uiaFailOpenWindows` | `[]` | `App::`/`App::Title` patterns allowed to fail open as above |
| `untitledSafeApps` | `[]` | Executables whose untitled windows count as verified for window exclusion (see [capture decision metadata](CAPTURE_PRIVACY.md)) |
| `credentialFilePatterns` | `[]` | Owner file-name globs added to the built-in credential-file title rule (no settings UI yet) |

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
  the macOS HD recorder are not (input rows in credential-file windows are not
  recorded at all; stored rows can be re-screened with `screenpipe privacy
  rescreen`).
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
