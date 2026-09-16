# ScreenWise repository instructions

These instructions apply to this repository and all of its subdirectories.
The repository is self-contained: no parent workspace, sibling checkout,
personal account, or Codex installation is required to develop it.

## Mission and source boundary

ScreenWise is a Windows-first, local personal activity recorder derived from
Screenpipe: WGC/multi-monitor capture, Windows UIA/OCR, local audio/transcription,
SQLite search, privacy controls and a bearer-authenticated loopback API.
It is an independent fork, not the hosted Screenpipe product.

The public repository is currently for other developers only. Neither its owner
nor Digiwise supports it in any way. Keep that boundary prominent in public docs;
do not imply end-user readiness, a Digiwise product/service, or a commitment to
maintenance, releases, issue responses or assistance.
Place upstream attribution before the prominent developer warning in README.md.
Privacy, security and local-only operation are aims with incomplete validation,
not guarantees. Do not claim this system/code is secure, confidential, leak-proof,
fully offline or safe because a scoped test passed. Neither the maintainer nor
Digiwise guarantees privacy, security, correctness or safety, or commits to
security fixes. Keep known failures and untested paths visible. This limits
claims; it does not weaken the implementation requirements below.

- The approved upstream MIT baseline is
  `892199f742e46d0c5d9e8c06687b35ca7c2b6547`. Preserve that commit and ancestry.
- Never fetch or copy post-baseline/current commercial Screenpipe implementations
  to solve a problem. Keep existing copyright and third-party notices.
- Litepipe is an optional reference, not a build/runtime dependency. Consult it
  only at `8969c10723634640ad2e757b8281dca8b0272c2f`. Record exact files, revision
  and provenance for any adaptation. No Litepipe implementation has been used
  in the recorded ScreenWise changes; its pinned README was reviewed as a reference.
- Resolve ordinary compiler, linker and test failures autonomously. Ask the owner
  about material architecture, dependency upgrades, licensing, capability removal,
  privacy/security trade-offs and destructive migrations.

## Start and preserve

Inspect branch, status and recent history before changing files; do not assume a
documented historical HEAD is current. Development currently uses `screenwise`.
Read [CONTRIBUTING.md](CONTRIBUTING.md), [BUILD_NOTES.md](BUILD_NOTES.md),
[BASELINE_AUDIT.md](BASELINE_AUDIT.md) and
[VALIDATION_REGISTER.md](VALIDATION_REGISTER.md) before extending validation.

Preserve unrelated edits, the Git index and private evidence. Never read captured
contents without explicit, current authorization. Test directories can contain
passwords, recordings, screenshots, transcripts, bearer tokens and SQLite stores.
Do not publish them. `.local/`, `target/`, `smoke*/`, caches and generated media
are private/generated, not source. Ignore rules do not protect already tracked
files: inspect the exact staged paths before every commit.

Detailed pre-publication maintainer notes may exist under ignored `.local/`.
They are not needed by a fresh clone and are not current instructions or renewed
permission to inspect captured data. Public documents contain sanitized summaries.

## Build and dependency discipline

- Do not run `cargo update`; use `--locked`. Use `--offline` only after required
  locked dependencies and native tools have been explicitly provisioned.
- Do not casually change dependency versions or either Rust lockfile. Earlier
  hardening intentionally changed lockfile graphs; compare against pre-task HEAD,
  not the obsolete claim that they still equal the upstream baseline.
- Use Visual Studio Developer PowerShell and the native environment described in
  docs/WINDOWS_SETUP.md. Never hard-code one developer's checkout or runtime paths.
- Production builds use `CMAKE_GENERATOR=Ninja`. Native tests linking
  libsamplerate use `Ninja Multi-Config`. The separate desktop workspace needs
  the documented transient knf-rs-sys CRT override for debug tests.
- Put the provisioned OpenBLAS `bin` on the test/runtime PATH. OPENBLAS_PATH alone
  is a header/library location. Provision ONNX Runtime explicitly; do not use the
  incompatible Windows system DLL as a fallback.
- Keep the deliberate root `workspace.default-members` restriction. The Apple
  MLX crate remains a workspace member but is not a default Windows build target.
- No silent dependency/model/tool acquisition to make a runtime test pass.

## Authentication, privacy and safe status

Keep bearer authentication enabled, including for localhost. `/health` and a small
startup-safe surface are exempt. Retrieve the token using `screenpipe auth token
--data-dir <the-recording-directory>`; never print it in public evidence. This
token is independent of local model configuration. Bind the API to loopback.

Privacy suppression and capture failures must be visible in the activity timeline
and local logs. Report the initial state, transitions and changed failure reasons;
deduplicate an unchanged condition. Unknown privacy state must fail closed.
Use fixed, allowlisted messages and reason codes, with timestamps as needed.
Never include captured screen/audio/input content, passwords, clipboard values,
window titles, URLs, user identifiers or private paths in these notices. Never
reuse a captured image as a failure placeholder.

Persist typed notices independently of capture admission so a privacy pause
does not suppress its explanation. If persistence fails, log a safe failure and
expose degraded status to the timeline. Activity buffers must warn before capacity
is exhausted and explicitly report observed subscriber losses. Distinguish
possible activity loss from confirmed dropped deliveries; numeric counts may be
reported, affected payloads may not. Do not equate lock-check recovery with
recording resumption: other privacy gates and user preferences still apply.
Document which paths meet this rule and which remain outstanding; internal audio
queue reporting is not covered merely because general event-bus tests pass.

## Interactive validation

Read [the reusable test guide](scripts/windows/interactive-validation/README.md)
before preparing a new run. Reuse scripts rather than rebuilding preparation.
Use fresh, explicitly named directories with no valuable data. Prepare and test
in the background first. Require fresh readiness for each interactive attempt;
wait indefinitely before owner interaction, with recording stopped and owned
windows hidden/closed while waiting. Old gates/evidence never imply current consent.
Use compact synthetic fixtures, no real credentials, and minimal owner time.
Spoken fixed cues are opt-in per run and must not contaminate audio measurements.

Do not infer a firewall pass from application logs or socket sampling alone.
Inventory exact executable paths and relevant children. Never reset the firewall,
alter unrelated rules/services/profiles, or weaken authentication. Rule creation
and restoration require the agreed owner-run elevated blocks and confirmation;
do not silently remove rules after a test. Preserve loopback and verify OS state.

## Before committing or publishing

1. Run `cargo fmt --all -- --check` for Rust changes and appropriate targeted tests.
2. For meaningful runtime/build changes, run `cargo build --release --locked`
   in Developer PowerShell and an appropriately scoped smoke test.
3. Inspect the full diff and `git diff --check`; check both Rust lockfiles.
4. Update concise setup/validation documentation, including failures and limits.
5. Inspect the exact staged paths; never include private notes, captures, logs,
   databases, recordings, models, generated archives or caches accidentally.
6. Keep commits reviewable, with explicit provenance and no invented test claims.

Publication preparation is not validation completion. The current firewall/privacy
milestone is incomplete; do not commit it as completed. Do not push, create a public
repository, rewrite history, or enable inherited release automation without the
owner's authorization. The owner selected
preserving exact upstream ancestry and replacing the local maintainer email with
a GitHub noreply identity only in a separate publication copy.
The owner authorized a reviewed publication-preparation commit with the partial
validation state and prominent no-guarantees notice; that is not certification
or authorization to publish.
