---
name: screenpipe-cli
description: Use the local Screenpipe CLI to check recorder status, search locally captured history, manage audio and vision devices, and maintain the local database.
---

# Screenpipe CLI

Run the installed `screenpipe` executable. The recorder is local-first: use
`screenpipe status`, `screenpipe search`, `screenpipe audio`,
`screenpipe vision`, `screenpipe db`, `screenpipe backup`, and
`screenpipe export` for recorder operations. Use `screenpipe auth token` to
retrieve the local API bearer token for the same recorder data directory.

Do not create or run scheduled automation files. Screenpipe does not execute
pipe workflows.
