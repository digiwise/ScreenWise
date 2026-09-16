---
name: release
description: "Review ScreenWise publication readiness; publication requires explicit owner authorization."
---

# ScreenWise release preparation

Read repository AGENTS.md, README.md and VALIDATION_REGISTER.md. The inherited release
workflows are inactive reference files under .github/upstream-workflows.

A source change is not authorization to push, tag, publish packages, upload
recordings, contact upstream services or use upstream signing/publishing identities.
Local maintainer checklists, if present, stay in ignored notes and are not required
by a fresh clone. This repository has no enabled automated release pipeline.

Prepare a concrete review covering source/history/privacy scope, license notices,
artifact provenance, target repository/package identity, checks and untested cases.
Use only explicit owner-authorized destinations and operations. Do not infer a
release from commit-message text or enable archived automation automatically.
