# ScreenWise hardening progress

> [!WARNING]
> **Hardening describes effort and aims, not privacy or security guarantees.**
> Neither the maintainer nor Digiwise guarantees this system or code is private,
> secure, correct or safe. Validation is partial; see the [README notice](README.md)
> and [open issues](VALIDATION_REGISTER.md).

Publication source review identified an unresolved audio persistence race: work
admitted before a privacy transition can finish writing afterward without a
rollback. See SW-V19/SW-T12. A scoped lock test or pre-write permit check does not
establish complete suppression of queued audio across transitions.

This is the concise public development record. Repository instructions now live
in [AGENTS.md](AGENTS.md); setup is in [docs/WINDOWS_SETUP.md](docs/WINDOWS_SETUP.md).
Detailed historical authored notes are preserved privately, not published as
capture artifacts. The owner selected retaining upstream ancestry and redacting the maintainer
email only in later commits in a separate publication copy.

## Completed source boundaries

Work derives from MIT Screenpipe commit
`892199f742e46d0c5d9e8c06687b35ca7c2b6547`. Local commits repaired Windows build/
runtime consistency and local API token lookup, removed automatic update/tool/model
acquisition, added explicit artifact checks, and removed telemetry, hosted accounts,
cloud sync, external AI transports, automation, integrations and enterprise runtime
surfaces. Retained APIs require bearer auth and loopback binding. Root and desktop
lockfiles have intentional historical graph changes; no incidental upgrades are
part of publication preparation.

## September 14-16 privacy and audio work

- Windows session lock detection now verifies WTS session state and desktop access,
  fails closed on unknown/error states, and emits fixed privacy notices. Repeated
  initialization no longer creates extra polling workers.
- General activity-channel pressure/recovery and subscriber-delivery loss are
  observable. Typed notice persistence failures and shutdown degradation reach
  local logs and timeline status without captured context.
- Native accessibility exclusions and explicit audio-device selection were repaired;
  scoped native privacy and selected-output live regressions have passed.
- Audio shutdown cooperatively flushes permitted partial buffers, owns workers and
  waits for in-flight persistence. Incomplete shutdown is reported and prevents
  an unsupported clean-restart claim. A partial buffer is delivered once rather
  than split into a prefix plus tail.
- Selected-output partial-tail persistence and search after a separate process
  restart passed a bounded live test. Active meetings and same-process restart
  still need further live validation.
- A real lock/unlock passed with audio disabled: capture counts stayed unchanged
  during an independently confirmed locked interval, synthetic controls recovered,
  auth stayed enforced and safe notices persisted. This is combined lock/window-
  filter evidence, not proof about every pixel or audio recovery.

The publication checkpoint records this experimental source work. The original
complete firewall/privacy milestone has NOT been completed. The checkpoint does
not present it as complete. See [VALIDATION_REGISTER.md](VALIDATION_REGISTER.md)
for unresolved audio-queue reporting, browser/clipboard/DRM cases and network proof.

## Standalone publication preparation

The fork now owns its contributor/policy/setup documents. Public-facing historical
notes are condensed and sanitized; original authored notes remain in ignored
`.local/`. Portable test sources passed 224 offline tests (one further symlink
test skipped for Windows privileges), verification of 44 manifest entries and
parsing of ten PowerShell scripts. These are independent of old live passes;
adapted launchers and preflight were not exercised live.
No recorder, playback or interactive privacy test was started by this work.
No firewall rule, dependency version or existing development commit is changed.
Public publishing is not authorized. A separate local publication copy was
verified with the 81 historical post-baseline commit emails redacted and exact
upstream ancestry preserved. The helper now also supports new noreply commits
and passed nine synthetic tests. The development checkout and publication copy use
the owner's GitHub noreply email in repository-local configuration for future
commits; global Git configuration is unchanged.

The intended future hosting location is the owner-selected `digiwise/ScreenWise`
organisation repository. Read-only access/name checks were performed; no GitHub
repository was created and no push occurred. Hosting does not imply support by
the owner or Digiwise.

The final preparation adds a prominent no-privacy/security-guarantees notice,
upstream-first attribution, a synthetic OCR benchmark fixture and explicit
source distribution boundaries. Current changes are carried into a separate
development checkout on the redacted history; original private evidence and
excluded local artifacts remain preserved. See BUILD_NOTES.md for current
offline checks and docs/DISTRIBUTION_SCOPE.md for asset and history limits.
