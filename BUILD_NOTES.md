# Windows build notes

Provenance baseline: commit `892199f742e46d0c5d9e8c06687b35ca7c2b6547` (detached HEAD).

## Environment

- Visual Studio 2026 Developer PowerShell with the x64 MSVC toolchain.
- Rust/Cargo toolchain capable of building the locked workspace.
- CMake 3.31.x and Ninja (`CMAKE_GENERATOR=Ninja`).
- OpenBLAS distribution root at `C:\Utils\OpenBLAS\win64`, with `OPENBLAS_PATH=C:\Utils\OpenBLAS\win64`. The variable must name the directory that directly contains `include\cblas.h` and `lib\libopenblas.lib`; `C:\Utils\OpenBLAS` is one level too high on this machine.

## Failures and fixes

- The initial build failed in `mlx-sys 0.2.0`: vendored MLX's `base_simd.h` uses the GNU/Clang `typeof` extension, which MSVC reports as `C3861`. Bare workspace builds included `crates/screenpipe-rfdetr-mlx` because the root member glob is `crates/*`, although that crate is explicitly Apple-Silicon-only and its consumer dependency is macOS/aarch64-gated.
- An initial attempt to add the crate to `workspace.exclude` was rejected because its manifest inherits package fields from the workspace; Cargo could no longer parse it as a path dependency. No compilation occurred in that attempt.
- Added explicit `workspace.default-members` containing every current workspace crate except `crates/screenpipe-rfdetr-mlx`. The Apple-only crate remains a workspace member (so inheritance and its target-gated macOS path dependency work) but bare `cargo build` no longer builds it directly on Windows. This is an uncommitted build-system patch; it does not change product code, dependencies, or `Cargo.lock`.
- The next workspace build failed in `antirez-asr-sys` while compiling `qwen_asr_kernels.c`: MSVC reported `fatal error C1083: Cannot open include file: 'cblas.h'`. The build script derived `C:\Utils\OpenBLAS\include` from `OPENBLAS_PATH`, but the installed header is at `C:\Utils\OpenBLAS\win64\include\cblas.h`. Corrected `OPENBLAS_PATH` to the nested distribution root; no source change was needed.

## Remaining issues

- None blocking the Windows release build. Two existing non-fatal Rust warnings remain (`unused_mut` in `screenpipe-audio` and an unused Windows `CommandExt` import in `screenpipe-engine`).

## Verification

- `cargo build --release --locked` completed successfully on 2026-09-01 with `OPENBLAS_PATH=C:\Utils\OpenBLAS\win64` (release profile, 11m 33s).
- `Cargo.lock` and Rust dependency versions were not changed. No build tools were installed or uninstalled, and no clean was run.
