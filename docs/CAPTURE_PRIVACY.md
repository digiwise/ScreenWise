# Windows capture decision metadata

Capture pauses and producer conditions now have independent [all-channel interval diagnostics](CAPTURE_DIAGNOSTICS.md), including safe exclusion-rule references. Per-frame metadata below remains a separate observation surface.

New event-driven Windows frames store a JSON object in `frames.capture_privacy`.
It is written atomically with the frame and returned as `capture_privacy` on OCR
search results (`GET /search?content_type=ocr`) and `GET /frames/{id}/context`.
These routes require the local bearer token. OCR search includes accessibility,
hybrid and privacy-placeholder rows, not only text recognized from pixels.
Old rows and other capture producers have `null`; this does not mean capture
was allowed or that no excluded applications existed. There is no backfill.

For example, this **fabricated** decision says an excluded Excel background
window prevented full-monitor acquisition while a safe foreground window was
captured on the same monitor:

```json
{
  "schema_version": 1,
  "outcome": "active_window_only",
  "reason": "excluded_background",
  "blockers": [
    {
      "app": "excel.exe",
      "reasons": ["configured_exclusion"],
      "foreground": false
    }
  ],
  "blockers_truncated": false,
  "foreground_monitor": "monitor_2",
  "is_active_monitor": true,
  "foreground_monitor_changed": false
}
```

`app` is a bounded executable basename obtained from Windows process metadata,
lowercased with `.exe`. It never uses a window title or application description.
Unknown, inaccessible or unsupported names are `null`. It is application
identity metadata and can reveal which applications were running; it contains
no document names, window titles, URLs, configured patterns or executable paths.
Generic timeline notices and local logs keep their existing fixed messages.

`blockers` deduplicates by executable and foreground/background role. It preserves
restrictions observed both before and after acquisition and immediately before
UIA, even when they disappear during the operation. It is limited to 32 entries;
`blockers_truncated` explicitly reports omitted entries. A later UIA refusal has
an unknown app identity because its skip result cannot safely establish the
executable after a possible focus change. Do not infer it from earlier focus.

Foreground blockers are now observed globally, including an excluded foreground
window on another monitor when this monitor has no permitted foreground fallback.
`foreground=true` means the global foreground window at an observed policy phase;
it does not mean that window overlaps this frame's monitor. Other-monitor
background apps are omitted. A global excluded foreground app does not itself
exclude pixels on all monitors: the top-level outcome/reason explains the local
decision. Both configured matches and unavailable metadata remain explicit, without
excluded window titles or filter strings. If the foreground window cannot be
enumerated, the verified executable may be reported with
`foreground_window_unavailable`; this is uncertainty, not a configured exclusion.

## Active-monitor provenance and filtering

`foreground_monitor` identifies the Windows monitor containing the global
foreground window, using the same `monitor_<id>` spelling as frame `device_name`.
It follows Windows monitor ownership for a window spanning displays, not the
mouse position or configured primary display. IDs are local/session provenance,
not portable physical-display identities. No nearest monitor is invented for a
window that Windows cannot associate with a monitor.

`is_active_monitor` is `true` when the captured monitor owns the foreground window,
`false` when a verified different monitor does, and `null` for unavailable or
changing association. Redacted frames retain this metadata too; it does not
imply capture permission, input activity, visibility of every pixel or work time.
Policy enumeration samples focus before/after; acquisition and pre-UIA evaluations
are merged, with a final monitor check after UIA. If any sampled monitor/knownness
differs, `foreground_monitor_changed=true` and both identity/boolean become null
for that frame, even if focus returns. This is sampled provenance, not proof that
no transition occurred between checks. A changed foreground window during policy
enumeration produces an inconsistent-focus placeholder and the fixed blocker code
`foreground_changed_during_evaluation`.

Use authenticated `GET /search?content_type=ocr&active_monitor=true` for a first
pass containing known active-monitor screen/UIA/OCR **and placeholder** records.
`active_monitor=false` selects known other-monitor records. Filtering occurs
before pagination and applies to `pagination.total`; omit the filter for all
records. Null, missing, malformed and explicitly changing associations match
neither boolean filter. The option requires `content_type=ocr`; incompatible
types are rejected. Other modalities and historical coverage require separate
queries. A filtered empty result does not establish an empty or inactive period.
Old records have no active-monitor provenance and need an unfiltered fallback.

| Outcome | Meaning |
|---|---|
| `full_monitor` | The Windows window-exclusion policy admitted monitor pixels. |
| `active_window_only` | Allowed foreground-window pixels on this monitor; the rest is blacked out with a disclosure banner. |
| `redacted` | Source-free disclosure frame; real pixels and text extraction withheld. |
| `capture_failed` | Source-free failure frame; acquisition or policy evaluation failed. |

Top-level reasons distinguish `excluded_background`, `active_window_excluded`,
`no_safe_active_window`, `inconsistent_focus`, and acquisition stages
`initial_privacy_evaluation`, `monitor_acquisition`, `active_window_acquisition`,
`post_capture_privacy_evaluation`. `full_monitor` has no top-level reason.
These describe window privacy admission, not subsequent image/text PII redaction,
lock notices or the complete recording lifecycle. Query recording-status events
separately to understand pauses, failures and gaps.

Blocker codes identify configured exclusions (`configured_exclusion`), include
filter rejection (`outside_include_filter`), built-in exclusions
(`builtin_exclusion`, `recorder_ui`), unverified window/URL metadata
(`window_metadata_unavailable`, `browser_url_unavailable`), URL exclusions
(`configured_url_exclusion`), and unsuitable foreground fallback surfaces
(`shell_surface`, `builtin_application_skip`). Later UIA gates use
`private_browsing`, `ignored_window`, `not_in_include_list`, `blocked_url`,
`builtin_application_skip` or `monitor_unverified`. No excluded pattern or URL
is included to explain a match.

## Excluded background windows

When the foreground excluded app belongs to monitor A, B/C can retain admitted
monitor pixels if their own enumerated overlapping windows pass policy. A stable,
verified other-monitor association skips the global foreground UIA tree and its
labels; B/C use local bitmap OCR without borrowing A's app/title/text. This does
not make inactive monitors automatically safe or enable workers disabled by
existing capture preferences. Missing foreground enumeration, unknown native
monitor association or inconsistent sampling produces a placeholder. Monitor
association changes across acquisition/pre-UIA phases also withhold the bitmap.
Configured app exclusion remains separate from global protected-content, lock
and schedule gates. The subsequent [microphone-policy change](CAPTURE_DIAGNOSTICS.md)
separates microphone admission from visual checks; retention policy is unchanged.

Non-minimized windows overlapping a monitor conservatively affect its policy,
even if another window appears to cover them completely. This implementation
does not prove occlusion or capture arbitrary nonforeground windows. An excluded
background window permits only a safe foreground window on that same monitor;
if global focus belongs to another monitor, a placeholder remains appropriate.
Minimized and nonoverlapping windows do not force this fallback.

Ordinary File Explorer folder windows (`CabinetWClass`, `ExploreWClass`) can now
serve as the safe foreground fallback. Explorer's desktop/taskbar/unknown shell
surfaces cannot. Explicit user exclusions still take precedence. The remaining
monitor pixels stay black; no background-window bitmap is acquired. A new
background exclusion discovered immediately before UIA invalidates an already
acquired full-monitor bitmap and produces a placeholder.

This is per-persisted-frame evidence, not an exhaustive application-presence
timeline: throttling, identical-image/content deduplication and recording pauses
can omit intervals. It does not identify historical blockers. Focus changes can
still pair an earlier allowed-window bitmap with a later allowed-window UIA tree;
exact screenshot/tree window identity across those transitions needs separate
work. Broader provider failures, protected playback and privacy accuracy are not
guaranteed by these diagnostics or the deterministic tests.
