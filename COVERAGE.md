# Screenpipe Coverage

Screenpipe tracks coverage at two complementary layers:

- Tauri/WebDriver E2E coverage: real product UX and local API behavior by platform.
- Core engine coverage: Rust behavioral flow coverage across capture, audio, DB, accessibility, and engine crates.

These dashboards are behavioral maps, not a replacement for line or branch coverage.
Use them to see which product risks are represented, then layer runtime job
results and `cargo llvm-cov` data on top when judging release confidence.

## Dashboards

- E2E dashboard: [apps/screenpipe-app-tauri/e2e/COVERAGE.md](apps/screenpipe-app-tauri/e2e/COVERAGE.md)
- Core engine dashboard: [coverage/CORE.md](coverage/CORE.md)

## Current Snapshot

### Tauri E2E

- Mapped specs: 41
- Declared test blocks: 142
- Weighted coverage points: 110.8

| Platform | Specs | Declared tests | Weighted points | Layers | Features | Critical score |
| --- | --- | --- | --- | --- | --- | --- |
| windows | 35 | 133 | 108.0 | 13 | 39 | 97% |
| macos | 38 | 108 | 83.4 | 12 | 40 | 90% |
| linux | 29 | 98 | 80.4 | 11 | 36 | 90% |

### Core Engine

- Mapped suites: 23
- Mapped Rust files: 178
- Active test blocks: 1584
- Ignored/manual test blocks: 102
- Weighted coverage points: 1330.8

| Platform | Suites | Active tests | Ignored tests | Weighted points | Layers | Flows | Critical score |
| --- | --- | --- | --- | --- | --- | --- | --- |
| windows | 20 | 1483 | 99 | 1283.4 | 19 | 11 | 100% |
| macos | 20 | 1534 | 79 | 1299.6 | 20 | 11 | 100% |
| linux | 18 | 1467 | 76 | 1269.3 | 18 | 11 | 100% |

## Refresh

From `apps/screenpipe-app-tauri`:

```bash
bun run coverage:all
bun run coverage:all:check
```

For core line coverage, install/use `cargo llvm-cov` and feed its JSON
summary into `coverage:core`; the core dashboard documents the exact command.
