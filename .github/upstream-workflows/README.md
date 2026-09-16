# Inactive upstream workflows

These inherited workflow files are preserved for reference outside
`.github/workflows`, with `.disabled` suffixes. They are not ScreenWise CI or
release definitions. They contain upstream package/service identities, external
credentials, scheduled/write-capable automation and self-hosted runner assumptions.

Do not move them back or trigger equivalent workflows without reviewing the
destination, permissions, actions, dependency acquisition and release scope.
ScreenWise's source build/test commands are documented in CONTRIBUTING.md and
docs/WINDOWS_SETUP.md. No new hosted CI pass is claimed by this archive.

Before enabling future CI, use fork-owned namespaces, least privileges, reviewed
action revisions and synthetic tests. Recording/private data and inherited
upstream credentials do not belong in hosted test artifacts.
