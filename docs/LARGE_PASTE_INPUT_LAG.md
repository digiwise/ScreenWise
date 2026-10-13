# Large clipboard input lag investigation (2026-10-12)

Large multiline JSON copy/paste was reported to cause multi-second cursor lag in
Firefox's developer console and the Excel Claude add-in. The source baseline was
`1875062aa`; the installed application was left running and unchanged.

The owner subsequently reported that pasting into Notepad was OK, while Firefox's
console and Claude in Excel experienced the lag. Excel's add-in was suspected to
use a WebView; that hosting detail has not been verified in this investigation.
This is reported behavior, not a controlled comparison performed by this task.
It raises the priority of destination-specific UIA provider work and browser/editor
processing as explanations. If the same clipboard payload and capture admission
were used in all cases, a purely payload-size-dependent redaction cost would be
less persuasive. Those conditions were not independently checked, and clipboard
interaction can also differ between applications. The confirmed hook-thread
defect remains valid; its repair alone is not proof that these incidents are fixed.

## Confirmed source defect

Windows installs both global low-level input hooks on `run_native_hooks`' thread.
That same thread previously read the clipboard through arboard and ran PII
redaction in its message loop. Moving those operations outside the hook callback
did not isolate input: Windows delivers hook callbacks to the installing thread,
which must keep pumping messages. Delayed clipboard rendering, clipboard retry
work, large allocations or redaction could therefore prevent prompt input
delivery. Typed-text redaction also ran on this thread.

[Microsoft's low-level mouse hook guidance](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc)
recommends handing work to a separate worker and returning immediately.
[Delayed clipboard rendering](https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-operations#delayed-rendering)
can require synchronous work by the clipboard owner. No post-baseline upstream
or Litepipe implementation was consulted or adapted.

## Repair

Clipboard acquisition and typed/clipboard PII redaction now run on one separate
content worker at the configured extraction priority. The hook thread hands off
requests with `try_send` into a 32-entry queue. Saturation/disconnection never
waits for the reader: clipboard operation metadata is retained without content,
and observed request losses have a numeric, content-free warning. No per-request
threads, dependency changes, content-size truncation or acquisition are added.

Clipboard content requires the original privacy generation and a fresh current
focus/password permission before reading, after reading and after redaction.
Requests older than one second retain operation metadata without reading content;
in-flight clipboard content is discarded on stop or loss of privacy admission.
Already-admitted typed text keeps its existing source-field semantics, including
the final shutdown flush, and is redacted before delivery. A privacy pause still
withholds delivery. Processing taking at least 100 ms emits only a fixed message
and numeric duration, without clipboard values, titles, URLs or paths.

## Production investigation and limits

Owner-authorized investigation used streaming scans of the production rolling
logs and read-only SQLite queries with bounded row selections, existing indexes,
query-only mode and execution deadlines. No recording, clipboard mutation,
desktop reproduction, screenshot/video/transcript read, deletion, checkpoint or
database migration was performed. Private evidence and the original checkout
were preserved.

The selected 256 clipboard records included two paste operations with valid JSON
payloads of 239,606 characters and 10,266 newline characters, attributed to
ChatGPT and VS Code. None of these selected clipboard records was attributed to
Firefox or Excel; they do not identify the reported incidents. Only lengths,
JSON validity and other diagnostic metadata were returned; payloads were not
printed or published. A bounded 20-minute frame-metadata query returned 898 rows,
including 392 accessibility frames; the largest stored tree was approximately
657 KB. These stored sizes do not measure provider execution time. The rolling
log scan found privacy-unavailability and database-lock messages, but no direct
clipboard initialization/read failures, hook timeout or allocation-failure record.
Current stored settings enabled clipboard capture and PII removal and disabled
the optional input-priority setting. The new worker handoff applies independently
of that setting. Existing logs contain no clipboard-processing timing, so they
cannot establish whether clipboard rendering, redaction, UIA or resource pressure
caused the reported stalls. The source defect is confirmed; attribution of the
specific incidents remains an inference.

The initial UIA review found a separate limitation: the Windows tree path called
`ElementFromHandleBuildCache` with `TreeScope_Subtree` before applying the local
node limit, and checked its walk timeout before the provider call rather than
bounding that call. The fallback walker also lacked a traversal deadline. These
calls ran off the input-hook thread, but provider work could burden the foreground
application. The clipboard-worker repair did not bound this path; the follow-up
below now does so. Neither change empirically rules out application/editor work.
An OS clipboard read can still delay the content worker and shutdown while it is
in flight; the fix isolates input delivery, not provider cancellation. A live
Firefox/Excel regression claim requires a separately gated synthetic trial.

## Bounded UIA follow-up (2026-10-13)

The follow-up starts from `14b6bcb8b`. Both tree capture and click/focus context
lookups now use `TreeScope_Element`; the unlimited subtree cache and the second
fallback traversal are removed. Every navigation step requests one element with
cached properties, and checks the shared budget before further provider work.
The existing adaptive timeout/node overrides still tighten these limits.

- Walk deadline: at most 250 ms, using the time remaining after window admission.
- Provider connection and transaction waits: each at most 50 ms, reduced to the
  remaining walk/probe budget before each request, with a one-millisecond minimum.
- Tree storage: at most 5,000 nodes and depth 35, or tighter configured limits.
- String decoding: at most 16 KiB per property and 256 KiB across the tree;
  the capture configuration additionally caps that aggregate at four times its
  maximum extracted-text length (200,000 bytes with the default configuration).
- Password-state probe: its own 50 ms overall budget, with existing generation,
  native-focus, ownership and unsupported-property checks preserved.
- Retry cooldown: one second after incomplete/failed tree work, context lookup
  failure, oversized context text or an unavailable password-state probe.

UTF-16 properties are decoded from a borrowed BSTR with bounded UTF-8 allocation,
avoiding the previous complete BSTR/PROPVARIANT copies and full string conversion.
COM still obtains/marshals a property's original value; these limits cannot cap
the provider's own allocation or prevent work already running in Firefox/WebView.
The timeout-capable Windows automation class is required; initialization does
not fall back to the older class and its long default provider waits.
[Microsoft documents these timeouts as client waits](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nn-uiautomationclient-iuiautomation2);
[the default transaction wait is 20 seconds](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomation2-put_transactiontimeout).
They do not promise cancellation of provider work or a hard wall-clock bound
including COM overhead and OS scheduling. No abandoned timeout threads are added.

Incomplete trees are discarded, including a tree at a node/depth boundary where
another request would be needed to prove completion. Actual navigation failures
are distinguished from a successful null child/sibling result. The tree walker
returns `AccessibilityIncomplete`; the engine persists a failure placeholder and
returns before screenshot persistence/OCR fallback. This prevents a partial tree
from authorizing a frame after missing an ignored subtree or blocked URL. Large,
deep or slow windows may therefore have capture withheld. Context enrichment is
also withheld on its limits; password-probe failure continues to deny content.
The original candidate kept tree-limit causes only in a debug message and
collapsed persisted causes. The [reporting follow-up](CAPTURE_PRIVACY.md#uia-failure-reporting-and-retry-transitions-2026-10-13)
preserves specific causes in frames/notices and adds bounded private diagnostics.

The canonical serial accessibility tests passed 213 tests, with 23 live tests
ignored and all 264 external artifacts reused. Fourteen new regressions exercise
complete small trees, empty budgets, deep/cyclic and wide trees, deadline expiry,
early/late provider failure, synthetic large multiline JSON, Unicode and aggregate
text limits, short remaining timeouts, cooldown expiry, successful-null versus
failed navigation, and BSTR ownership. They install no hooks, request no real UIA
providers, and read no clipboard. RootFmt passed; the existing unused `press_key`
helper warning remains. The 45 existing scoped engine capture tests passed,
reusing 685 external artifacts and rebuilding four workspace artifacts. These
exercise capture policy, placeholder/late-exclusion handling, monitor ownership,
trigger reduction and timeout behavior; they are not a live end-to-end test of
the new UIA failure path. Its placeholder return before OCR was also source-reviewed.

Canonical commands used unchanged offline caches, pinned dependencies and one job:

```powershell
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootFmt
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-a11y -Lib -Jobs 1 -TestArguments @('--test-threads=1')
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootTest -Package screenpipe-engine -Lib -TestFilter event_driven_capture -Jobs 1 -TestArguments @('--test-threads=1')
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task RootBuild -Jobs 1
.\scripts\windows\build\Invoke-ScreenWiseBuild.ps1 -Task DesktopBuild -Jobs 1
```

Both release-local builds passed sequentially at reduced process priority:

| Build | Cargo duration | External reused/rebuilt | Workspace reused/rebuilt |
|---|---|---|---|
| RootBuild | 5m10s | 785 / 0 | 14 / 5 |
| DesktopBuild | 10m16s | 1,080 / 0 | 13 / 5 |

There were no new dependency variants or unexpected external rebuilds. Available
memory was monitored; no other application was stopped or reprioritized. The
recorder's `--version`, `--help` and `record --help` all exited zero without capture;
versions remain recorder 0.4.15 and desktop 2.5.28. The desktop GUI was not launched.

Private canonical evidence identifiers:
`20261012T130827859Z-Root-Fmt-afe70120`,
`20261012T130628667Z-Root-Test-8e43bf62` (accessibility),
`20261012T130834793Z-Root-Test-77a94bc9` (engine),
`20261012T131142170Z-Root-Build-c968fc88`,
`20261012T131753008Z-Desktop-Build-3b6e67d2`.

The UIA follow-up artifacts supersede the clipboard-only hashes below:

| Artifact (release-local) | SHA256 |
|---|---|
| `target/release-local/screenpipe.exe` | `381970B0C5F9EC9D1E8991BB26B8E9EE534FB75F6E6BC08EBEB6CFA0176D521E` |
| `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe` | `81C711AB4747E28284CDB5730DC880A3CF008E430E8A097DC4BFDE0999B54DDA` |

Both Rust lockfiles are unchanged. No dependency acquisition, cache clearing or
relocation, full-release build, frontend change, external-client MCP work, desktop
capture, deployment, restart or firewall change occurred. Installed behavior and
a live Firefox/Excel paste comparison remain unverified.

## Clipboard repair validation (2026-10-12)

The canonical `RootTest -Package screenpipe-a11y -Lib -Jobs 1` with serial test
execution passed 199 tests; 23 interactive tests remained ignored. All 264
external artifacts were reused. Eight new tests cover stalled-reader isolation,
bounded overload with metadata fallback, a synthetic multiline JSON larger than
239,000 bytes with PII, permission changes before/after read and redaction,
disabled/expired/denied reads, pause during read, fixed read failures and shutdown
draining. Existing text/privacy tests now exercise the worker handoff. No new test
installs hooks or accesses the real clipboard. RootFmt passed. The existing
unused `press_key` test-helper warning remains.

Both canonical release-local builds passed sequentially with `-Jobs 1`:

| Build | Cargo duration | External reused/rebuilt | Workspace reused/rebuilt |
|---|---|---|---|
| RootBuild | 3m54s | 785 / 0 | 14 / 5 |
| DesktopBuild | 8m53s | 1,080 / 0 | 13 / 5 |

There were no new dependency variants or unexpected external rebuilds. The first
unit-test build also refreshed the local config crate for the preceding committed
monitor-status change; the settled second test build rebuilt only accessibility.
The desktop compiler's priority was lowered during its final compile when memory
headroom narrowed; no other application was stopped or changed.

Private canonical evidence identifiers:
`20261012T115712261Z-Root-Fmt-7a0c4d7d`,
`20261012T115713993Z-Root-Test-b60d4f37`,
`20261012T115819019Z-Root-Build-ba01428f`,
`20261012T120253967Z-Desktop-Build-1aee23b6`.

The built recorder's `--version`, `--help` and `record --help` all exited zero;
version remains 0.4.15. The desktop version remains 2.5.28 and its GUI was not
launched. Artifact SHA256 values:

| Artifact (release-local) | SHA256 |
|---|---|
| `target/release-local/screenpipe.exe` | `3134A8F60D390D2F79917493B0C694325B6C134A3AA8FAB7ED134000012721C0` |
| `apps/screenpipe-app-tauri/src-tauri/target/release-local/screenpipe-app.exe` | `D37AAE51CF540D4584EAB422903B73011F32E5C3C59B45E3F0D89EEB84E0A5BB` |

Both Rust lockfiles are unchanged; no dependency acquisition, cache relocation or
clearing, full-release build, frontend change, external-client MCP work, deployment,
restart or firewall change occurred. Full release performance, installed behavior
and a new live capture session are outside this validation.
