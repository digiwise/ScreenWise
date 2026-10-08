# Contributing to ScreenWise

> [!WARNING]
> **No privacy or security guarantees.** Hardening is a development aim with
> incomplete validation. The maintainer and Digiwise make no guarantee about
> the privacy, security, correctness or safety of this system or its code.
> Use synthetic or non-sensitive test data; see the prominent [README notice](README.md)
> and [known limitations](VALIDATION_REGISTER.md).

This repository is currently intended only for other developers to inspect,
build and modify. It is not supported in any way by the repository owner or
Digiwise. Contributions or issues do not create an expectation of a response,
review, fix, release or assistance. Do not contact Digiwise for project support.

ScreenWise is an independent Windows-first fork of Screenpipe, based on upstream
MIT commit `892199f742e46d0c5d9e8c06687b35ca7c2b6547`. Read [AGENTS.md](AGENTS.md)
for repository-wide provenance, privacy and verification requirements.

## Set up a checkout

Clone this fork into any directory and work from its root. The checkout does not
need a parent workspace or a sibling Litepipe repository. See
[Windows setup](docs/WINDOWS_SETUP.md) for the pinned toolchain, native artifacts,
Developer PowerShell environment, release build, native test matrix and packaging.
The existing executable/package names remain `screenpipe` for compatibility;
this is not a repository-wide identifier rename.

Before committing, configure your own GitHub noreply address locally in each
clone if you do not want a personal/work email in public commit metadata:

```powershell
git config --local user.email '<your GitHub noreply address>'
git var GIT_AUTHOR_IDENT
git var GIT_COMMITTER_IDENT
```

Use the address shown in your GitHub email settings. Repository-local Git
configuration is not cloned. Environment variables, explicit author options and
preserved authors on cherry-picked commits can override the default; inspect
commit metadata before a public push. This setting does not redact old commits.

Dependency and model acquisition must be explicit. Do not run `cargo update` or
change locked package versions as incidental cleanup. Both Rust workspaces have
lockfiles. Use `--locked`; add `--offline` after required caches are provisioned.

## Changes and checks

Keep changes focused and preserve unrelated work. For Rust source changes:

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootFmt
# Run targeted tests through RootTest or DesktopTest; see the launcher guide.
# Functional builds default to release-local. Adopt the new cache once with
# -AllowColdCache after reviewing -PlanOnly; keep subsequent invocations stable.
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootBuild

git diff --check
git diff -- Cargo.lock apps/screenpipe-app-tauri/src-tauri/Cargo.lock
```

Check the separate desktop workspace and frontend when a change affects them.
Use the [canonical launcher](scripts/windows/build/README.md) for supported Windows
Cargo tasks. Use `release-local` by default for local development, validation and
deployment. Run `-BuildProfile release` only when explicitly requested by the owner
or when investigating inadequate `release-local` performance. Before pushing, a
full release build may be recommended with a concrete reason; the owner chooses.
Adequate local-profile performance is sufficient to proceed, assuming release
will perform at least as well. Label measurements with the profile actually tested.
Required tests, formatting, frontend export and affected native checks/builds remain.
Do not substitute a unit test for a live capture, audio or OS-firewall claim.
Record exact tested scope and outstanding checks in
[VALIDATION_REGISTER.md](VALIDATION_REGISTER.md). Build/signing/publishing actions
are separate from routine source validation; inherited upstream automation must
not publish under Screenpipe's identities or services.

## Safe tests

Prefer deterministic synthetic tests and fresh test stores. Interactive capture,
clipboard, audio playback and Windows lock tests require current owner readiness.
Wait indefinitely with recording stopped before interaction. Reuse
[the interactive harness](scripts/windows/interactive-validation/README.md).
Never disable API authentication, change global firewall defaults, replay old
consent, inspect private captures without authorization, or use real credentials
as test stimuli. Keep private run output and detailed machine notes out of Git.

## Reviews and provenance

Explain the concrete behavior change, tests performed and limits. Preserve
upstream copyright/license notices. Do not fetch or copy current/post-MIT
Screenpipe source. If an approved external reference is adapted, document its
exact revision and files. Preserve the approved baseline commit. Inspect staged
paths before every commit; do not use blanket staging on a recording workstation.

Public pushes and releases require explicit owner authorization and a review of
the exact tree, history, asset provenance and redistribution scope. Keep local
publication checklists and audit details in ignored maintainer notes.
