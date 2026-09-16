# Audio-disabled Windows lock/unlock validation

Attempt 04 passed the bounded audio-disabled contract after three incomplete harness attempts. Independent OS state supported an 11.890-second locked plateau (frame rows 7->7, UIA rows 11->11), distinct synthetic before/after controls, 403/403/200 auth and fixed safe notices. Cleanup passed and the repaired fixture recorded no exception. Included-window filtering stayed active; audio lock recovery and every-pixel proof remain outside scope.

See [VALIDATION_REGISTER.md](VALIDATION_REGISTER.md) for current issues, test
limits and artifact identifiers, and [the reusable test guide](scripts/windows/interactive-validation/README.md)
for setup. This is a sanitized summary. Detailed original authored notes are
preserved locally under ignored `.local/maintainer-notes/`; raw test evidence
is private and was not moved or read during publication preparation.
