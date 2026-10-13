# Fresh bulk UIA privacy fixture

> **Developer-only, unsupported, and not a privacy guarantee.** This synthetic
> manual trial is preparation for the selected ScreenWise candidate. The owner
> and Digiwise provide no support. Compilation and the inert self-test do not
> establish live capture, provider compatibility, or privacy-rule behavior.

The separate executable is `FreshBulkPrivacyFixture.exe`. Its source is
[`FreshBulkPrivacyFixture.cs`](../scripts/windows/interactive-validation/FreshBulkPrivacyFixture.cs).
It is locally authored, without upstream or Litepipe implementation copying.
It never controls the recorder, reads a database, uses the clipboard, injects
input, starts a listener, opens a browser, downloads dependencies, or changes
settings or firewall rules. With no arguments it exits with code 2. Only an
owner-run `--interactive` invocation opens its windows. The historical staged
kit and `source-manifest.json` are separate; this fixture is built directly from
the new source and is not silently added to those pinned live controllers.

## Build in the background

From the repository root, using the existing Windows Framework C# compiler:

```powershell
& .\scripts\windows\interactive-validation\Build-FreshBulkPrivacyFixture.ps1 -SelfTest
```

The builder uses existing `System.Windows.Forms`, `UIAutomationProvider`,
`UIAutomationTypes` and `WindowsBase` assemblies. It writes only
`target/uia-privacy-fixture/FreshBulkPrivacyFixture.exe`, refuses to replace a
running fixture, and prints the resulting SHA-256. `-Compiler <absolute csc.exe
path>` selects an already provisioned compiler. No Cargo build, restore,
package acquisition, recording, visible window, or active desktop inspection is
performed. The inert executable checks its provider roles, values, tree links,
stable IDs, bulk sizes and phase mappings; the builder also verifies that an
empty or unknown invocation rejects GUI startup. These are source/provider
logic checks, not an out-of-process UIA test.

## Prepare one fresh manual attempt

Follow the [interactive-validation readiness guide](../scripts/windows/interactive-validation/README.md)
and [fixture readiness guide](WINDOWS_FIXTURE_READINESS.md). First establish the
exact candidate profile, executable hashes and normal-user interactive desktop.
Review the settings delta and intended phases with recording stopped and owned
fixtures closed. Wait indefinitely for fresh owner readiness; previous gates
and permission to prepare this fixture do not authorize capture or deployment.
Deployment/restart remains a separately confirmed owner action.

This document gives an owner-operated trial, not an automated readiness gate or
controller. The owner must explicitly authorize the chosen capture session and
launch after readiness. Do not have an agent invoke this interactive command as
part of a build or self-test:

```powershell
& .\target\uia-privacy-fixture\FreshBulkPrivacyFixture.exe --interactive
```

The owner checks that the correct foreground window and intended text field are
actually visible and focused before typing. Use only the fixed synthetic values
below. Confirm the ordinary positive control before interpreting absent protected
markers. Classify launch, desktop visibility, focus and provider failures as
harness failures; stop a failed attempt before interpreting recorder behavior.
The fixture launches neither a topmost window nor a focus-stealing background
worker; all focus changes and opening the optional peer are owner actions.

## Reviewed settings delta

Preserve the complete original configuration outside tracked source, then append
these exact synthetic rules to the existing `ignoredWindows` array for the
rule-transition phases:

```json
{
  "ignoredWindows": [
    "FreshBulkPrivacyFixture::SWNATIVEBLOCK",
    "FreshBulkPrivacyFixture::SWDOCBLOCK",
    "FreshBulkPrivacyFixture::SWVALUEBLOCK",
    "FreshBulkPrivacyFixture::SWNESTBLOCK",
    "FreshBulkPrivacyFixture::SWEXTBLOCK"
  ],
  "ignoredUrls": ["blocked.fixture.invalid"]
}
```

This is a **settings delta**, not a replacement configuration. The established
`App::Title` syntax matches app and title case-insensitively by substring; the
app constraint deliberately omits `.exe` so it works with either returned
basename form. Native window policy uses the top-level title. The tree extractor
also applies ignored-window rules to nested `Window.Name`, `Document.Name`,
`Document.Value`, and shallow extension-popup child text. The URL decorator
checks extracted Document URLs in embedded non-browser apps as well.
`ignoredUrls` is not app scoped, so review this reserved synthetic hostname rule
as an explicit temporary global delta. The URLs are provider property strings;
the fixture never requests them. All domains end in the reserved `.invalid`.

Leave include rules unchanged for this first sequence and record any existing
rule that prevents the allowed positive control. Keep capture, authentication,
audio, monitor, privacy and model choices exactly within the reviewed run scope.
This fixture does not require audio or clipboard capture. If testing keyboard
persistence, that separate opt-in must be explicitly enabled in the reviewed
settings; otherwise absence of keyboard rows is expected and cannot prove
password suppression.

## Manual sequence and expected results

Start with **4 KiB**, **Allow / stop rapid**, and content churn off. The top-level
title is `SW ALLOWED Fresh Bulk Fixture`. The custom fragment exposes a Pane with
a Document (ValuePattern URL) and child Text, a nested Window and child Text,
and a read-only Edit whose bulk Value contains `SW_BULK_ALLOWED_<generation>`.
Native WinForms ordinary and password TextBoxes are outside that fragment.

Before claiming the Document/Window cases, verify in the candidate's normal
authorized UIA output or an explicitly authorized inspector that the custom
fragment actually appears with these roles and values. If it does not, classify
those cases as provider failures, even if OCR can read the painted labels.
No provider inspection was performed during preparation.

For each row, perform **Allow → case button → Allow**, leaving each state steady
long enough for a completed normal capture (for example three seconds, then
inspect a later completed sample). Record the generation and transition time.
The case button changes properties; it does not change ScreenWise settings.

| Button / phase | Changed property | Expected behavior with the delta above |
| --- | --- | --- |
| Allow / stop rapid | Ordinary title, document/name/URL and nested window | Admitted current accessibility content and normal Screen/OCR capture when other gates allow; ordinary positive marker present |
| native-title | Top-level title becomes `SWNATIVEBLOCK Fresh Bulk Fixture` | Native title exclusion suppresses the target frame; no blocked-generation marker |
| document-name | Document.Name becomes `SWDOCBLOCK` while native title stays ordinary | UIA subtree exclusion withholds the whole snapshot/frame; no blocked-generation marker |
| document-value | Document.Value becomes `https://fixture.invalid/SWVALUEBLOCK` | Document value matches ignored-window rule and withholds the whole snapshot/frame |
| nested-window | Nested Window.Name becomes `SWNESTBLOCK` | Nested-window exclusion withholds the whole snapshot/frame before descendants persist |
| extension-child | Document.Value becomes `chrome-extension://syntheticfixture/popup`, child Text.Name becomes `SWEXTBLOCK` | Shallow extension-child rule withholds the whole snapshot/frame |
| document-url | Document.Value becomes `https://blocked.fixture.invalid/synthetic` | URL decorator suppresses the target frame via the ignored URL rule |

Check positive suppression/reason evidence, not only absent content. Accepted
capture after the final Allow is required to demonstrate recovery. A stored
pre-transition frame may still appear in history: compare capture times and
generation, not the fact that an older frame remains visible. An in-flight
transition may require investigation of admission timing; this harness does
not declare every screenshot displayed during a transition to be a fresh sample.
Read only the explicitly authorized synthetic run results, keep any recording
evidence private, and do not expand the trial into inspection of unrelated
production history. Use the timeline's fixed rule/reason diagnostics and numeric
counts where possible; do not publish raw production output.

## Fresh content, size and rapid changes

At **4 KiB**, after an allowed positive control, click **Refresh content only** repeatedly.
The HWND, title, runtime IDs and tree shape stay stable. Only the synthetic
content generation changes. A later admitted accessibility snapshot must show
the current `SW_BULK_ALLOWED_<generation>` and child generation. This is the
specific control for accidentally reusing stale bulk property values while
fresh privacy properties are being checked.

**Toggle content churn** changes only generation/content every 250 ms until
clicked again. Churn is explicit, owner started, and uses a WinForms timer so
the normal message pump continues to dispatch. The provider raises Name and
Value property-change events for the changed fragment fields when UIA clients
are listening, exercising the bulk acquisition's revision subscription. Event
delivery and observation remain part of the live trial. Timers are requested
intervals, not hard-real-time measurements. Only the first 100 characters of
the bulk Value are painted; OCR cannot establish freshness of its complete Value.

Keep **4 KiB** selected. For each exclusion case, select its button, restore Allow, and click
**Rapid last case**. It alternates that case and Allow every requested 125 ms
for 40 ticks, then returns to Allow. Repeat for native title, Document name/value,
nested Window, extension child and Document URL. The last selected rule remains
the target even after Allow. Observe rule transitions, suppressed samples,
ordinary recovery and UI responsiveness; recording can sample fewer than all
40 phases. Missing an unsampled 125 ms phase is not a pass/fail by itself, and
the fixture does not prove continuous admission freshness or close every
in-flight race. Stop churn and repeat steady-state recovery afterward.

Both **64 KiB** and **320 KiB (over budget)** put a single Value above the
retained **16 KiB per-property** limit. With a responsive provider and this
fixture's small tree, recording acquisition should fail with
`uia_property_text_limit` (`capture_uia_property_text_limit` in capture reason
metadata). They deliberately test incomplete retrieval rather than admitted
bulk content. The 320 KiB Value also exceeds the 256 KiB aggregate ceiling, but
the per-property limit is reached first: this fixture does **not** establish
aggregate-limit behavior. A timeout/provider failure must be classified by its
actual reason rather than counted as an observed property-limit result.
With the applicable subtree/URL delta in force, unresolved UIA policy must
withhold the frame and expose its failure/reason; do not accept a persisted
partial bulk tree as proof that privacy passed. Test the oversized values with
Allow selected, then return to **4 KiB** and Allow to verify recovery after any
normal bounded retry cooldown. A separately reviewed
run with no applicable tree-dependent rules may admit pixels despite failed
accessibility retrieval; that is a different policy case, not an assertion that
this fixture's UIA content was captured.

## Focus, password and separate rule profiles

Use **Open peer for manual focus**, then click/Alt+Tab between the main form and
`SW PEER Fresh Bulk Fixture`. The peer is a distinct unowned top-level form in
the same executable with ordinary marker `SW_PEER_POSITIVE`. Confirm both forms'
foreground identity and a positive ordinary capture after each switch. Repeat
while main-form content churn is active, then with a selected blocked phase.
The synthetic provider's nested Window case is distinct from this real peer;
the provider does not attach the peer to its custom tree. Existing monitor and
background-window policies may suppress the visible blocked main form even
when the peer is foreground; assess the reported capture scope rather than
assuming any foreground switch permits pixels.

With Allow steady, click the ordinary field, type the fixed marker
`SW_ORDINARY_TYPED`, then click the password field and type
`SW_PASSWORD_TYPED`, then return to the ordinary field and type
`SW_ORDINARY_RECOVERED`. Repeat the cycle. The native password control uses
`UseSystemPasswordChar`; its fake prefilled value is `SW_FAKE_PASSWORD`.
Accessibility text/tree must contain the password redaction marker and none of
the fake password Value. Its painted bullets do not require blocking the whole
ordinary screen. If keyboard persistence was enabled for this approved run,
both ordinary controls must persist while the password marker stays absent,
with the expected positive password-suppression diagnostic delta. No input is
injected by the fixture. Keyboard capture disabled, wrong focus, or no ordinary
positive control makes that keyboard subcase inconclusive.

Two optional, separately reviewed profiles exercise native configuration:

1. Append `FreshBulkPrivacyFixture::` to `ignoredWindows`: both forms of this
   executable must be suppressed regardless of title. Remove exactly that
   temporary entry after recording is stopped and verify ordinary recovery in
   a fresh authorized attempt. App-only exclusion cannot produce an Allow phase
   merely by clicking the fixture's Allow button.
2. In a separate profile append `FreshBulkPrivacyFixture::SW ALLOWED` to
   `includedWindows`, preserving the original array. The main allowed title
   passes this scoped whitelist; the peer title and native blocked title do
   not. Other ignored rules still take precedence. A configuration change/restart
   is governed by the approved run and does not happen inside the fixture.

Stop capture before restoring exactly the temporary settings delta or closing
the trial. Closing the main form stops its timers and closes its peer. No
firewall exception is needed because this executable has no listener or network
client; do not accept a firewall-access prompt or weaken any rule for the trial.

## Limits and preparation evidence

The explicit raw UIA provider supplies deterministic Document, nested Window
and ValuePattern properties. It does not implement Chromium, Firefox, real
extension providers, browser address-bar inspection, private/incognito window
detection, browser timing, real DRM/HDCP, remote desktop, or protected video.
The extension URI exercises the extractor's synthetic shallow-child branch;
it is not real browser-extension evidence. URL exclusion checks are Document
decorator evidence only. Do not rename this fixture to a known DRM executable
or call a synthetic hostname test real protected-content coverage.

Preparation compiled the x64 Framework executable and passed inert provider
and startup self-tests. No visible fixture launch, focus change, input,
out-of-process UIA inspection, recorder capture, production deployment or
firewall change is included. Record live observations and exact candidate/
fixture hashes separately after a fresh approved attempt. Even successful
synthetic observations apply only to the exercised candidate, rules and sampled
transitions; they provide no general privacy or security guarantee.
