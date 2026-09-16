# Distribution scope

ScreenWise is prepared as a source-only developer repository. The publication
tip does not distribute recorder output, validation evidence, model weights,
recorded test media, compiled helper programs, or screenshots whose contents
were not reviewed for public redistribution. This is a distribution boundary,
not a privacy or security certification.

The approved Screenpipe baseline is
`892199f742e46d0c5d9e8c06687b35ca7c2b6547`. Its exact ancestry remains in the
repository. Removing a path from the ScreenWise tip does not erase that path or
its Git blob/LFS pointer from inherited history. Publication therefore does not
claim history erasure. The source-only publication does not upload the
corresponding Git LFS object payloads.

No model-backed runtime capability is removed by this boundary. Production
speaker segmentation and embedding already load explicitly provisioned,
checksum-verified files from the operating-system cache rather than from the
source checkout. On the validated Windows setup that is
`%LOCALAPPDATA%\screenpipe\models`. The inherited repository model files are
used only by ignored/manual tests, examples, and audio-evaluation tools.

The repository does not establish a separately documented acquisition revision
or redistribution licence for the two inherited speaker-model artifacts.
Their accepted filenames and hashes remain documented in source, but the model
payloads are not distributed. Operators must review and provision their own
artifacts. The same rule applies to omitted audio fixtures: ignored/manual tests,
examples, benches, and eval tools require operator-supplied local fixtures at
their historical paths, or a later explicit fixture-path option. Ordinary source
compilation does not embed any omitted path.

The inherited URL-detection benchmark corpus is replaced at the publication tip
by a small, locally authored synthetic fixture. Other captured-looking upstream
screenshots and OCR fixtures are omitted without inspecting their contents.
The legacy OCR tests and benches that open `testing_OCR.png` or
`testing_OCR_chinese.png` are unavailable unless an operator supplies reviewed
local fixtures at those historical paths. The three fixture-dependent integration
tests are explicitly ignored by default with that reason; benches still require
manual fixtures. These expected skips are not test passes, and no universal OCR
validation is claimed.

Normal build assets such as application icons, installer artwork, and retained
UI artwork stay in the source tree where the build refers to them. Their
presence relies on the inherited repository notices within their actual scope;
this document does not make an unconditional licence or provenance claim for
every individual asset.

## Paths omitted from the publication tip

Git LFS pointer paths whose payloads are not published:

- `.github/scripts/audio_test.wav`
- `crates/screenpipe-audio/models/pyannote/segmentation-3.0.onnx`
- `crates/screenpipe-audio/models/pyannote/wespeaker_en_voxceleb_CAM++.onnx`
- `crates/screenpipe-audio/test_data/Arifi.wav`
- `crates/screenpipe-audio/test_data/accuracy1.wav`
- `crates/screenpipe-audio/test_data/accuracy2.wav`
- `crates/screenpipe-audio/test_data/accuracy3.wav`
- `crates/screenpipe-audio/test_data/accuracy4.mp4`
- `crates/screenpipe-audio/test_data/accuracy4.wav`
- `crates/screenpipe-audio/test_data/accuracy5.mp4`
- `crates/screenpipe-audio/test_data/accuracy5.wav`
- `crates/screenpipe-audio/test_data/poetic_kapil_gupta.wav`
- `crates/screenpipe-audio/test_data/selah.mp4`
- `crates/screenpipe-audio/test_data/speaker_identification/6_speakers.wav`
- `crates/screenpipe-audio/test_data/speaker_identification/obama.wav`
- `docs/mintlify/docs-mintlify-mig-tmp/public/5ire-setup.gif`
- `docs/mintlify/docs-mintlify-mig-tmp/public/claude-setup.gif`
- `docs/mintlify/docs-mintlify-mig-tmp/public/cursor-setup.gif`

Other inherited data, binaries, and captured-looking media omitted from the tip:

- `crates/screenpipe-audio/test_data/selah.mp3`
- `apps/screenpipe-app-tauri/components/__tests__/url-detection-benchmark-data.json`
- `apps/screenpipe-app-tauri/src-tauri/ui_monitor-aarch64-apple-darwin`
- `apps/screenpipe-app-tauri/src-tauri/ui_monitor-x86_64-apple-darwin`
- `.github/pr-media/mdns_optin_flow.png`
- `.github/pr-media/owned-browser-eval-owner.png`
- `.github/pr-media/owned-browser-no-flash.png`
- `apps/screenpipe-app-tauri/docs/ask-sources-timeline.png`
- `apps/screenpipe-app-tauri/docs/media/privacy-installed-apps.png`
- `apps/screenpipe-app-tauri/public/images/google-oauth-unverified-app-walkthrough.png`
- `apps/screenpipe-app-tauri/public/pipe-store-preview.png`
- `crates/screenpipe-redact/docs/pii-categories.png`
- `crates/screenpipe-screen/tests/Claude_prompt.png`
- `crates/screenpipe-screen/tests/testing_OCR.png`
- `crates/screenpipe-screen/tests/testing_OCR_chinese.png`
- `docs/assets/audio-transcription-backlog-status.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/app-screenshots/home-home.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/app-screenshots/pipes-discover-tab.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/app-screenshots/pipes-section-loaded.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/app-screenshots/settings-privacy.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/app-screenshots/settings-recording.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/app-screenshots/settings-storage.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/cursor-mcp-result.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/lmstudio1.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/lmstudio2.png`
- `docs/mintlify/docs-mintlify-mig-tmp/public/lmstudio3.png`
- `docs/mockups/note-drag-drop-images.png`

## Licence metadata boundary

The retained root `LICENSE.md` contains the MIT licence text and an inherited
exception for an `ee/` directory that is not present in this tree. Inherited
Cargo workspace metadata says `MIT OR Apache-2.0`, but no Apache licence text is
tracked. This publication inventory records that mismatch; it does not grant an
Apache licence, alter copyright notices, or resolve the legal scope of inherited
assets. Model, recording, screenshot, logo, and other third-party asset rights
must be assessed from artifact-specific evidence rather than inferred from a
workspace package field.

No binary, model, audio, video, image, database, or archive file was added by
the ScreenWise commits between the approved baseline and audited head
`b81025fd1ddaed11f28fd672ad40bf54bcbda8e1`. Publication preparation must repeat
the path inventory against the final committed tip. Working-tree and index
changes are deliberately excluded by the isolated publication builder.
