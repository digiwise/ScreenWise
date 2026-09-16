# Partial-tail stop/restart acceptance preparation

Preparation only. Nothing here launches Screenpipe, devices, models, UI,
playback, network activity, or firewall operations. `tail_acceptance.py` is a pure
evaluator over a caller-supplied mapping. It does not inspect old run data.

## Preferred next live plan: fresh 60-second single flush

Time ordering permits equal timestamps at adjacent handoffs because this Python
runtime uses GetTickCount64 with a 15.625 ms tick. Startup, playback and shutdown
work must still have positive duration, and reversed order fails. Preserve any
original result if a checker correction requires offline reevaluation; record
the original result hash and new evaluator hash, without modifying evidence.

If Windows refuses initial programmatic foreground activation, the collector
waits passively and indefinitely for the owner to click the compact fixture.
Recording and playback have not started at that point. Locking Windows or closing
the fixture aborts safely. No repeated focus-stealing calls run while waiting.

`tail_acceptance_60s.py` is the preferred, less intrusive acceptance contract for
the revised producer. Configure a fresh recorder process for a 60-second chunk;
with the fixed two-second overlap, its first normal emission threshold is 62
seconds. Target the stop request before 30 seconds when capture becomes ready
quickly. Enforce an absolute 45-second safety watchdog from verified process
creation. If real capture readiness leaves insufficient time for the 10.783-second baseline,
2.884-second tail, zero-row checkpoint, and stop request, stop cleanly and mark
the attempt incomplete for retry. Never extend the watchdog.

After real capture readiness, play `baseline.wav` and then `tail.wav` once. After
playback completes but before requesting stop, the fresh data store must still
contain zero audio chunks and zero baseline/tail transcription hits. The process
must request graceful stop strictly before 45 seconds from creation. This leaves
12 seconds of shared shutdown headroom before 57 seconds and remains below both
the configured 60 seconds and the 62-second normal threshold.

The revised producer bypasses the overlap-preserving normal flush once stop is
requested. Its final zero-overlap flush sends the complete partial buffer once.
After clean exit, require exactly one new selected-device `audio_chunks` row and
one owned media file. Obtain its duration from `ffprobe` metadata after exit; the
database does not store media duration. It must be under 60 seconds. Both exact
markers must be persisted in transcription rows joined to that same chunk ID.
The canonical evaluator markers remain the word forms. SQLite and authenticated
search additionally accept only the fixed synthetic equivalents `seven`/`7`,
`nine`/`9`, and `five`/`5`; matching rows and returned chunk IDs are deduplicated.
The collector journals the exact synthetic query forms used without transcript
bodies. Transcription device evidence follows the product schema: `device` is
the bare selected name and `is_input_device` must be false. Only after both
columns match does the collector reconstruct and report the selected
`<name> (output)` identity.
Any force, nonzero exit, device recovery, privacy transition, extra chunk,
`queued_work_discarded`, unresolved worker, or other shutdown issue prevents a
pass.

Start a distinct recorder process on the same opaque data-store identity using a
five-second chunk. Recheck the 403/403/200 bearer matrix, then require authenticated
search results for both baseline and tail to identify the original shutdown chunk.
Play the 10.773-second restart control, require its exact marker in a new chunk in
SQLite and authenticated search, and stop the restarted process cleanly.

This proves a process-level partial-buffer stop and restart. It does not exercise
same-process `AudioManager::restart`. A separately established routing/model run
is useful context, but the baseline marker inside this partial buffer remains the
structural positive control for the tested run.

### Prepared collector wiring against the existing adapter

Keep the pinned controller assets unchanged. `tail_collector_60s.py` is a bounded,
mock-tested adapter that reuses these existing operations. Import and preview do
not perform OS actions; the execution path has not been live verified:

1. Use `coordinator.prepare` and `consume_confirmation` for the indefinite fresh
   readiness gate, then `WindowsBackend.quiescent` and `preflight` before launch.
2. Extend `start_recorder` arguments for the 60-second first process and replace
   its 240-second watchdog with a run-local 45-second watchdog measured from the
   returned `OwnedProcess` creation identity. Record monotonic creation time next
   to PID and creation ticks.
3. Use `await_api`, `api_checks`, and the existing health polling to establish the
   exact single selected device and actual capture-handle readiness. If readiness
   consumes the budget, call `stop_recorder` and report incomplete.
4. Add finite, hash-checked one-shot playback for the three `../speech-retry`
   assets; the existing `play_stimulus` only understands older phase files and
   loops them, so it cannot be reused unchanged.
5. Use `aggregate` immediately after both first-process WAVs complete and before
   stop. Require zero audio chunks and marker hits. Capture recovery/privacy
   transition counters from allowlisted status/notice evidence.
6. Use `stop_recorder`, `clean_shutdown`, `processes_stopped`, and the owned
   PID/creation-time guard. After exit, extend `final_evidence` with the one new
   chunk ID, same-chunk marker joins, selected device, owned media existence, and
   `ffprobe` duration metadata. Do not read or return non-synthetic transcript or
   media content.
7. Create a second backend/process on the same fresh data directory with a
   five-second chunk. Reuse `await_api`, `api_checks`, `audio_marker_count`, and
   authenticated `request`, but preserve returned audio chunk IDs so search hits
   can be linked to the shutdown chunk rather than accepted as global matches.
8. Persist/search the restart control, then run the same identity-guarded clean
   shutdown checks. Feed only the bounded evidence mapping to
   `tail_acceptance_60s.evaluate`.

The collector adds acknowledged synchronous one-shot playback with foreground,
WTS, process-identity, and budget checks; a phase-local 45-second watchdog; exact
post-exit marker/device joins; `ffprobe` duration collection; real
`Audio.content.chunk_id` search parsing; and strict capture-status/SQLite checks.
API evidence requests recheck the owned PID, creation time, executable, exact
`127.0.0.1:31479` listener, and authentication. Execution also requires a
separately reviewed `../collector-pins.json` with schema
`screenwise.tail-collector-pins.v1`, pinning all new collector/preparation/speech
assets relative to the background-run directory. Root creates that manifest only
after these files freeze. Regular-emission timestamp and callback instrumentation
remain unnecessary for this preferred plan.

Safe structured checkpoints for the bearer status matrix, pre-stop aggregate
counts, and both shutdown results are written to the owned run journal as they
are collected. They contain numeric statuses/counts and process/shutdown state,
without bearer tokens, transcript bodies, or private paths, so a later evidence
exception does not discard already established facts.

For a future reviewed live run, preview first with
`.\launch.ps1 -RunId <fresh-gate-run-id>`. After the owner explicitly replies
Ready with that gate's nonce, use `.\launch.ps1 -RunId <fresh-gate-run-id>
-OwnerReady <nonce> -ExecuteInteractive`. Do not execute until the separate pin
manifest has been reviewed and generated. The wrapper starts nothing in preview.

## Alternative: five-second boundary-instrumented plan

The live collector should use three new, exact synthetic marker strings: a normal
pre-tail control, a final-tail marker, and a post-restart control. They must be
mutually distinct. Immediately before playing the tail, record the maximum audio
transcription row ID and prove the tail marker has zero SQLite hits and zero
authenticated `/search` hits. This checkpoint prevents an earlier chunk from
being relabeled as the final tail.

The generated mono 16-bit 22050 Hz SAPI assets are in `../speech-retry`, with the
source manifest at `../speech-retry/manifest.json` and independent measurements
at `../speech-validation.json`. No playback occurred during their generation:

- `baseline.wav`: 10.783 seconds; marker `cobalt meadow seven`, spoken three times
  inside the labeled baseline passage.
- `tail.wav`: 2.884 seconds; passage `Final marker violet harbor nine.`
- `restart.wav`: 10.773 seconds; marker `silver orchard five`, spoken three times
  inside the labeled restart passage.

The current regular segment window is five seconds plus two seconds of retained
overlap: a seven-second emitted window advances by a five-second stride. The tail
duration therefore leaves at most 2.116 seconds from a regular emission boundary
to request stop, and less if playback starts after that boundary. The collector
must record the last regular-emission timestamp, start the tail immediately after
it, observe a real capture callback after tail onset, and prove the stop request
preceded the next five-second emission boundary. A `normal_chunk_emitted` flag by
itself is insufficient unless instrumentation supplies that boundary evidence.

After the tail stimulus finishes, request graceful stop immediately. Wait for the
owned recorder identity to exit. A force kill, nonzero exit, shutdown issue, or
`worker_completion_unconfirmed` is a failure. Only after exit, inspect the fresh
test SQLite database; the stopped recorder cannot serve authenticated search. The
matching row ID must be newer than the pre-tail maximum. Start a new recorder
process against the same test data directory, wait for real API/capture readiness,
then prove the tail remains persisted and authenticated-searchable. Emit and
persist the distinct restart control before stopping gracefully again. This is a
recorder-process restart test, not evidence for a same-process AudioManager
stop/restart path.

The evidence mapping consumed by `evaluate()` has these sections. A stable opaque
`data_store_id` must link the pre-tail, stopped-process SQLite, and restarted API
observations without exposing a private filesystem path:

- `readiness`: gate and acceptance monotonic times, exact nonce match, explicit
  owner Ready, stopped/hidden waiting state, `expires_at: null`, and
  `deadline_enforced: false`.
- `markers`, `before_control`, `pre_tail_snapshot`, and `tail_stimulus`: exact
  synthetic marker identities, positive counts, zero pre-tail tail counts, row
  checkpoint, and monotonic event order.
- `partial_boundary`: seven-second window, two-second overlap, five-second
  emission stride, the measured tail WAV duration, last regular-emission time, a
  capture callback within the tail/stop window, and no later regular emission.
- `api_auth`: exact `http://127.0.0.1:31479`, 403 for missing/wrong bearer, 200
  for valid bearer, with redirects disabled. The same status matrix is repeated
  after process restart so a restart that silently disabled auth cannot pass.
- `first_shutdown` and `final_shutdown`: graceful request, process exit, zero
  exit code, no force, no unresolved workers, and no shutdown issues.
- `post_stop_tail`: exact tail marker, a positive SQLite count after exit, and a
  matching row newer than the checkpoint. It deliberately has no API claim.
- `restart`: a new recorder-process identity on the same data store, real
  readiness, exact-marker authenticated search for the durable tail, and positive
  SQLite/authenticated-search counts for the distinct restart control. The final
  shutdown must identify that process and occur after the restart control.

Proposed short live sequence after a fresh indefinite owner-readiness reply:

1. Start a fresh-directory recorder with only the selected audio path enabled;
   wait for actual capture/API readiness and verify bearer enforcement.
2. Play the normal control long enough to cross the configured segmentation
   threshold; wait until it is persisted and searchable.
3. Take the zero-hit tail checkpoint, play the distinct short tail, then request
   graceful stop as soon as playback completes.
4. After verified process exit, collect post-stop SQLite evidence. Start a new
   recorder process on the same directory; after API/capture readiness, prove the
   tail is authenticated-searchable, play/persist the restart control, and stop
   gracefully once more.

This preparation cannot prove real device routing, model accuracy, actual partial
buffer timing, database/API observations, process identity, graceful worker joins,
or restart behavior. One delayed readiness reply demonstrates that a particular
gate remained usable; the non-expiring policy also needs source/mock coverage and
cannot be universally proven by a finite wait. Row ordering plus a zero-hit
checkpoint strongly identifies a fresh tail, but only the future live collector
can prove the checkpoint was taken at the stated phase and against the same owned
database/API instance. The active-meeting child-task ownership repair is now in
the built source but has not yet been live qualified; any
`worker_completion_unconfirmed` result must fail. The five-second alternative's
live harness still needs trustworthy instrumentation for regular-emission time,
capture callback time, process PID plus creation time, graceful exit, worker
completion, and a stable opaque identity for the SQLite/API data store. It must
also bind each authenticated response to the owned restarted listener and record
the exact synthetic query marker. Logs, a bare PID, caller assertions, or inferred
wall-clock delays alone do not satisfy those fields. The current pinned live
controller does not expose all of this instrumentation; this preparation does not
claim it can execute the contract without a separately reviewed collector change.

Run the mock suite from this directory with:

```powershell
python -W error -m unittest -v test_tail_acceptance_60s.py
python -W error -m unittest -v test_tail_acceptance.py
python -W error -m unittest -v test_tail_collector_60s.py
```
