# Windows build and runtime baseline

Upstream provenance baseline: commit `892199f742e46d0c5d9e8c06687b35ca7c2b6547` (MIT-licensed upstream). The Windows compatibility change described below is commit `a08843c80` on top of that baseline. Do not update dependencies or `Cargo.lock` while reproducing this baseline.

## Historical upstream guidance

- Historical Screenpipe Windows documentation specified Visual Studio 2022. That likely describes a genuinely compatible historical toolchain, not just stale documentation: the native dependencies and their build scripts date from that toolchain generation.
- The old documentation added a GnuWin32 directory to `PATH` unconditionally. Future documentation should make that conditional on the directory actually existing. The real requirement is simply to verify that `unzip` is available on `PATH`.

## Environment-specific findings on this machine

These findings were verified on 2026-09-01 and are not source requirements for every Windows machine:

- Builds must run from Visual Studio Developer PowerShell, not ordinary PowerShell. The developer shell supplies the MSVC compiler, Windows SDK headers, and variables such as `VCINSTALLDIR`, `VCToolsInstallDir`, `WindowsSdkDir`, `INCLUDE`, `LIB`, and `LIBPATH`.
- The installed shell is Visual Studio 2026 Developer PowerShell 18.9.2, using MSVC tools 14.51.36231 and Windows SDK 10.0.26100.0. Historical upstream guidance remains Visual Studio 2022 as noted above.
- CMake 4.4 was incompatible with the older OpenBLAS/MLX native build scripts. CMake 3.31.x works; the verified version is 3.31.8.
- CMake 3.31 does not recognise the Visual Studio 18 / 2026 generator. In a Visual Studio 2026 Developer PowerShell session, use Ninja explicitly:

  ```powershell
  $env:CMAKE_GENERATOR = "Ninja"
  ```

  The verified Ninja version is 1.13.2.
- The verified Rust toolchain is `rustc 1.93.1` / `cargo 1.93.1`. On this machine, launching Developer PowerShell from another shell can omit `C:\Users\clanc\.cargo\bin` from `PATH`; add the existing Rust toolchain path to that session if `cargo` is not found.
- OpenBLAS is required by the default Qwen/ASR native build path. On this machine its distribution root is:

  ```powershell
  $env:OPENBLAS_PATH = "C:\Utils\OpenBLAS\win64"
  ```

  That exact directory directly contains `include\cblas.h` and `lib\libopenblas.lib`. Setting `OPENBLAS_PATH` to `C:\Utils\OpenBLAS` is one level too high for this installation.

## Source and build-system change in this repository

- The bare workspace build originally included `crates/screenpipe-rfdetr-mlx` because the root member glob is `crates/*`. That explicitly Apple-Silicon-only crate then failed under MSVC because vendored MLX uses the GNU/Clang `typeof` extension.
- Commit `a08843c80` (`fix Windows workspace build`) adds explicit `workspace.default-members` containing every current workspace crate except `crates/screenpipe-rfdetr-mlx`. The crate remains a workspace member so its inherited manifest fields and target-gated macOS path dependency continue to work. This is the only source/build-system compatibility change relative to upstream commit `892199f`.
- An earlier attempt to put the crate in `workspace.exclude` did not work: Cargo could no longer parse its inherited package fields as a path dependency. That attempt is not present in the repository.
- The compatibility commit does not alter product functionality, dependency versions, or `Cargo.lock`.

## Reproducible release build

From Visual Studio Developer PowerShell, after setting `OPENBLAS_PATH` and (for Visual Studio 2026) `CMAKE_GENERATOR`, use the locked dependency graph:

```powershell
cargo build --release --locked
```

Do not substitute an unlocked build when validating this baseline.

## OpenBLAS runtime DLL requirement

The build-time import library is not sufficient at runtime. The Qwen/ASR native path links dynamically to the exact DLL filename `libopenblas.dll`. Before the runtime fix, `screenpipe.exe --help` and `--version` exited before Rust `main` with process exit code `-1073741515` (`0xC0000135`, `STATUS_DLL_NOT_FOUND`). Because the Windows loader fails before Clap or logging initialises, the console can appear to exit silently.

For this installation, copy:

- Source: `C:\Utils\OpenBLAS\win64\bin\libopenblas.dll`
- Destination: `D:\Data\NoSync\Repos\ScreenWise\screenpipe\target\release\libopenblas.dll` (beside `target\release\screenpipe.exe`)

Equivalent command from the repository root:

```powershell
Copy-Item C:\Utils\OpenBLAS\win64\bin\libopenblas.dll `
  target\release\libopenblas.dll
```

Putting `C:\Utils\OpenBLAS\win64\bin` on the launching process's `PATH` also allows the Windows loader to find the DLL, but application-local deployment is more deterministic. Future Windows packaging/distribution must bundle `libopenblas.dll` beside every distributed `screenpipe.exe` (or deliberately install and expose an ABI-compatible DLL directory on `PATH`). Merely setting `OPENBLAS_PATH` does not solve runtime loading because its `bin` child is not searched automatically.

The source and deployed DLLs verified on this machine are both 51,117,073 bytes and have SHA-256 `B554C45AF7B39154C561FB4879FD784D4928462E9A70335AADD9B1DE3C75E9E2`.

## Verification on 2026-09-01

- Visual Studio Developer PowerShell reported `VSCMD_VER=18.9.2`, `VisualStudioVersion=18.0`, MSVC tools 14.51.36231, and Windows SDK 10.0.26100.0. `INCLUDE`, `LIB`, and `LIBPATH` contained the corresponding MSVC and SDK directories.
- `cmake --version`: 3.31.8.
- `ninja --version`: 1.13.2.
- `OPENBLAS_PATH=C:\Utils\OpenBLAS\win64`; both `include\cblas.h` and `lib\libopenblas.lib` exist.
- The required `target\release\libopenblas.dll` exists beside `screenpipe.exe`, and its SHA-256 matches the installed source DLL.
- `cargo build --release --locked` completed successfully (release profile, 1m 16s for this verification run). The only Rust warnings were the existing `unused_mut` in `screenpipe-audio` and unused Windows `CommandExt` import in `screenpipe-engine`.
- `target\release\screenpipe.exe --help` printed the CLI usage and exited 0.
- `target\release\screenpipe.exe --version` printed `screenpipe 0.4.15` and exited 0.
- A bare `target\release\screenpipe.exe` printed usage and exited 2 because a subcommand is required. Normal recording is started with `record`.
- A bounded normal-launch smoke test ran `record` with audio, vision, and telemetry disabled on port 31337. The process remained running after 12 seconds and `http://127.0.0.1:31337/health` returned HTTP 200, version 0.4.15, and status `healthy`; the verification process was then stopped.
- `Cargo.lock` has no diff relative to upstream commit `892199f` or the current `HEAD`.

No product functionality, dependencies, or `CONTRIBUTING.md` were modified as part of this documentation pass.
