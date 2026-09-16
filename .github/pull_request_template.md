---
name: pull request
about: submit changes to the project
title: "[pr] "
labels: ''
assignees: ''

---

## description

brief description of the changes in this pr.

This is an unsupported developer-only project. Neither the repository owner nor
Digiwise promises review, support, maintenance or a release.

related issue: #

## before

Describe the previous behavior using a synthetic example. Do not upload private
recordings, transcripts, databases, bearer tokens or unreviewed logs.

## after

Describe the resulting behavior and relevant verification. Synthetic, non-private
illustrations are optional; a screen recording is not required.

## how to test

add a few steps to test the pr in the most time efficient way.

1. 
2. 
3. 

## desktop app checklist (if applicable)

If this PR adds or changes `#[tauri::command]` handlers or Rust types exported to the frontend, from `apps/screenpipe-app-tauri/`:

- [ ] `bun run bindings:generate` (if bindings changed)
- [ ] `bun run bindings:check`
- [ ] `bun run typecheck`

Commands are auto-collected via the vendored `tauri-helper` crate — no manual handler list edits in `main.rs`.

