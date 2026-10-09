# Recording-status implementation batch (2026-10-09)

Agreed scope: [handover](RECORDING_STATUS_UI.md), source baseline `1cac89c9e`.
Actual deployment and new interactive capture validation are separate.

| Task | Scope | Owner | State |
|---|---|---|---|
| 1 | Preserve stale status; output gaps remain Listening | Dashboard | In progress |
| 2 | Precise foreground/input safety failure diagnostics | Diagnostics | In progress |
| 3 | Monitor names and aliases | Dashboard | In progress |
| 4 | Dashboard visibility, geometry, level, tray recovery | Dashboard | In progress |
| 5 | Three display levels and contextual help | Dashboard | In progress |
| 6 | Wrap only main-window Status row | Dashboard | In progress |
| 7 | Retryable media eviction and retention restoration | Backend | In progress |
| 8 | Actual Windows fallback audio configuration | Backend | In progress |
| 9 | Exact CORS and minimal public readiness | Backend | In progress |
| 10 | Timestamp metadata conversion preserves captured text | Backend | In progress |
| 11 | Deployment auto-start after existing checks; prepare-only | Integration | Implemented; 73 synthetic deployment checks passed |
| 12 | Focused regressions, release-local builds, docs, candidate | Integration | Pending integration |
| 13 | Separate sensitive diagnostics; local launch opt-in enabled | Diagnostics | In progress |

Validation results and remaining limits will be recorded here after integration.
Deployment milestone: 73 synthetic checks passed, including automatic startup selection,
prepare-only, failure propagation, diagnostic opt-out and manifest type validation.
No real deployment, firewall mutation or GUI launch was performed.
