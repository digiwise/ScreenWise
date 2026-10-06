# Windows capture decision metadata

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
  "blockers_truncated": false
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
