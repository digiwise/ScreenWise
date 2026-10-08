# Desktop context generation and cache reuse

On 2026-10-08 an otherwise unchanged production desktop build relinked because
one generated Tauri HTML asset was newer than Cargo's application dependency
reference. The previous compilation reference preceded asset creation by about
22 seconds. External dependencies were fresh. This was one follow-up application
rebuild after first generating an asset, not evidence of repeated asset rewriting:
the pinned Tauri generator already reuses existing content-addressed output.

The desktop now uses Tauri's supported build-script context generation. Pinned
`tauri-build` 2.6.2 enables `codegen`; `build.rs` passes default `CodegenContext`
to `try_build`, and the application includes its default `tauri-build-context.rs`
using `tauri_build_context!`. Assets therefore exist before application rustc starts,
instead of being first created by `generate_context!` during that compilation.
Configuration loading, frontend assets, icons and capability defaults use the same
pinned Tauri context generator. Development/release selection comes from Tauri's
`DEP_TAURI_DEV` instruction, as required by its build-script API. Configuration,
frontend directory and icons remain build-script inputs. No asset content,
permissions, runtime sidecars or configuration values are intentionally changed.

The desktop lockfile adds only the existing `quote` and `tauri-codegen` dependency
edges to `tauri-build`. No package, version or checksum changes; the root lockfile
is unchanged. This feature change legitimately requires new build-time artifacts
once. Do not clean caches, normalize generated timestamps or suppress Cargo's
invalidation checks to hide the original cause.

Preparation checks in the isolated worktree:

- Canonical `DesktopMetadata`, `DesktopTree` (locked/offline) and `DesktopFmt` passed.
- `scripts/windows/build/Test-DesktopContextGeneration.ps1` passed five checks
  using a dependency-free Cargo fixture: initial compilation, generated asset
  predating compiler dependency reference, immediate unchanged reuse, changed
  frontend invalidation, and immediate unchanged reuse after that change. The
  fixture uses a separate generated cache and no desktop build or live capture.

The fixture proves Cargo ordering rather than complete Tauri runtime parity.
Canonical DesktopCheck passed in 1m31s, and the production desktop release
build passed in 11m01s. Each rebuilt 15 expected Tauri variants, with zero
unexpected external rebuilds. The real release output contained 494 generated
asset files, none newer than the application dependency reference; the generated
context also preceded that reference. This validates the intended ordering in
the actual Tauri build without an extra production cache-warming run. Observe asset ordering
and normal reuse on the next already necessary equivalent build, without adding
an expensive production build solely for benchmarking. Live desktop UI behavior,
non-Windows builds and Windows PowerShell 5.1 execution remain unvalidated.
