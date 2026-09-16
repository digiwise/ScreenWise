# Selected-output tail and process-restart validation

A bounded live test preserved both synthetic markers in one 13.798-second partial audio chunk. Search found that original chunk after a separate process restart and found a new marker in a new chunk. Both stops were clean and both auth matrices were 403/403/200. Original checker failure was preserved and separately reevaluated after a clock-resolution correction; earlier attempts remain incomplete.

See [VALIDATION_REGISTER.md](VALIDATION_REGISTER.md) for current issues, test
limits and artifact identifiers, and [the reusable test guide](scripts/windows/interactive-validation/README.md)
for setup. This is a sanitized summary. Detailed original authored notes are
preserved locally under ignored `.local/maintainer-notes/`; raw test evidence
is private and was not moved or read during publication preparation.
