# M0–M1 architecture and realtime-safety review

Reviewed commit `217fba6` on Windows x64, including the actual rendering paths,
timebase/scheduler, routing tests, validation harness and pinned CPAL 0.15.3
WASAPI source in the Cargo registry. No production code was changed. No critical
memory-safety or normal-data-callback allocation/locking defect was demonstrated.

**Recommendation: conditional GO for M2 development.** Keep the existing mixer,
routing and control-thread device ownership. Address sample-onset semantics and
define callback-owned scheduling/clock contracts before integrating M2 launches.
This is not a go for performance deployment or distribution: physical isolation,
dropout evidence and license packaging remain open.

## Prioritized findings

### 1. P1 before sample integration: automatic fades erase valid short sources

**Confirmed behavior, not a crash or a newly observed hardware failure.**
`crates/audio-backend/src/mixer.rs:276` multiplies every source by both an onset
and an end envelope. At position zero the onset is zero; at the last frame
`remaining.saturating_sub(1)` is zero. Consequently one- and two-frame sources
are entirely silent. An M0 impulse at source frame zero is also erased when
played through the mixer. The existing M0 playback example uses the single-source
path, so this does not mean that example currently loses its first impulse.

Reproduced offline using the public API: construct two stereo prepared buffers
containing `[0.5, 0.5]` at 48000 Hz; construct `StereoMixer`; set A gain 1, B gain
0 and crossfader 0; render eight samples. Result: eight zeros, positions `[1,1]`,
finished=true. The temporary executable is under ignored `target/review-probe`.
The existing tests use longer fixtures and do not assert preservation of onsets.

Minimal recommendation: define source/envelope semantics before adding one-shots.
Keep startup/control smoothing at the bus level; make any per-source attack/release
an explicit playback policy instead of silently altering every prepared asset.
Add onset/short-percussion tests. Simply disabling all safety ramps is not advised.

### 2. P1 M2 design gate: current telemetry and controls cannot schedule launches

**Confirmed scope boundary; not an M1 regression.**
`playback.rs:77` reads independently published fields; `cpal_backend.rs:256`
reports only the maximum deck cursor. The counters are delivered/rendered frames,
not hardware presentation time. `mixer.rs:115` publishes a latest-value parameter
snapshot and `mixer.rs:258` samples it per frame: intermediate updates may be
coalesced, and their application frame depends on thread timing. This is valid
for knob targets, but cannot preserve note-on/off order or quantized launches.
The offline automation matrix (`tests/m1_integration.rs`) changes controls between
known render segments; it does not prove sample-accurate concurrent automation.

`event-engine/src/lib.rs:65` compiles a frozen tempo map, allocates and sorts on
construction; `:99` rejects blocks over 8192 frames. `examples/playback.rs:65`
pre-renders its schedule before opening the device. There is no existing live
scheduler-to-device bridge. Passing the 16384-frame integration test does not
validate calling `OfflineScheduler::render` directly with that size.

Minimal recommendation: before the first M2 event, specify one callback-owned
timeline/epoch, restart and late-event behavior, and a bounded ordered command
path with explicit admission/overflow behavior. Keep continuous parameters on
their current atomic path. Preserve beat-space intent for editable tempo and
adapt/split bounded render work. Do not use UI polling of `source_frames` as the
event clock, or clone/construct tempo maps and schedules inside the callback.

### 3. P1 distribution blocker: LICENSE is demonstrably incomplete

**Confirmed repository defect.** `LICENSE:128` ends mid-sentence in section 1.
Sections 2–17 and the remainder are absent. This contradicts the complete-license
claim in `docs/GPL_COMPATABILITY_AND_RELEASE_POLICY.md:6`. Reproduce with
`Get-Content LICENSE -Tail 9` and a search for numbered sections. The dependency
gate still passes because it checks metadata rather than license-text completeness.

Minimal recommendation: restore a verified verbatim full GPL v3 license before
distribution and correct the documentation assertion. The Windows dependency
expressions still match the reviewed seven-package Apache/MIT inventory; no
conflicting dependency was identified. This packaging defect does not require
replacing CPAL. Also resolve the empty `license_files` entry for dasp_sample in
`docs/windows-dependencies.json:18` when collecting release notices. An inventory
pass is not a completed source/notice bundle or an advisory scan. ASIO and other
platform graphs remain outside that inventory; keep their separate review gate.

### 4. P2: stop and recovery have weaker guarantees than a future transport needs

**Confirmed semantics; audible clicks/hangs were not reproduced.**
`cpal_backend.rs:210` disables rendering and immediately requests pause; it does
not ramp a currently nonzero output to zero. Source-boundary fades do not protect
a mid-source stop. Thus a nonzero-to-silence discontinuity is possible; no click-free
stop guarantee should be built on this API. A synchronous pause error leaves the
owner terminal, and a second stop returns success without retry. Runtime errors
at `:294` retain only a failed flag, discarding the cause.

Pinned upstream `src/host/wasapi/stream.rs:169` enqueues play/pause commands;
success is not device acknowledgement. Its destructor at `:159` sends termination
and joins the worker thread. DAWJ currently calls these on the control thread,
which is correct. Moving stream destruction into an M2 callback would introduce
blocking/deadlock risk; a driver hang during teardown is a risk, not an observed
failure. Existing soaks primarily test EOF stop/recreation, not nonzero mid-play
stop, unplug or driver failure.

Minimal recommendation: retain control-thread stream destruction; document
terminal-stop semantics explicitly in transport callers. Before user-facing
transport, add a bounded fade/acknowledgement path and retain a compact error
category without formatting strings in the callback. Test mid-source stop and
failure paths separately. No automatic recovery framework is justified yet.

### 5. P2 hardware gate: routing validates stream slots, not endpoint identity

**Confirmed limitation; misrouting after device changes was not reproduced.**
`cpal_backend.rs:104` resolves an ordinal against a fresh enumeration. A stale
ordinal can select a different endpoint if devices change; channel validation
cannot detect a different device with the same format. `routing.rs` correctly
rejects overlap and labels separate channels unverified. Stereo fallback never
folds cue into master. The isolation tests support logical separation only.

Minimal recommendation: before persistent hardware configuration, bind routing
to a retained device selection or stable endpoint identity and revalidate after
changes. Do not reuse ordinal-based routing across enumeration snapshots.
Verify physical channel order and absence of vendor mirroring on the selected
interface. Current WASAPI observations expose stereo HyperX and four-channel
DualSense; neither establishes private headphone monitoring. Pinned CPAL WASAPI
`device.rs:372` enumerates the shared-mode mix-format channel count, not every
hardware output arrangement. ASIO may be needed if the eventual interface only
exposes required outputs through its ASIO driver; this is still hardware-dependent.

### 6. P2 evidence limitation: the soak is an execution baseline, not a realtime proof

**Confirmed measurement boundary, not a discovered dropout.**
`examples/soak.rs:28` fixes both amplitudes to zero. It exercises control, render
and stream lifetime code but not audible signal fidelity, large sample-buffer
working sets, or sustained limiter reduction. `cpal_backend.rs:285` measures the
renderer call only, excluding the metrics publication and driver work outside
the callback; it discards CPAL timestamps. A delayed/missing callback can escape
the nominal render-duration test. Monotonic delivered counters cannot prove DAC
continuity. The existing report appropriately avoids zero-dropout claims.

Minimal recommendation: retain the current counters with their precise label;
before performance sign-off, add nonzero offline/loopback evidence, representative
sample/synth load and callback-arrival/presentation diagnostics. Allocation
instrumentation would strengthen confidence. Do not repeat the same silent soak
as a substitute for these different measurements.

## Realtime and architecture observations without a demonstrated defect

- Normal DAWJ data rendering uses owned source storage, stack frames, bounded
  slice writes and native x64 atomic loads/stores. No normal-path allocation,
  reference-count cloning, logging, mutex acquisition or explicit waiting was
  found. `Instant`, trig and driver calls are not worst-case execution-time
  proofs. Upstream CPAL performs waits/command processing outside the data
  callback and can format/drop allocated error descriptions on exceptional paths;
  do not describe the entire driver worker as allocation-free.
- Immutable source ownership is sound. For M2 asset replacement, prepare data
  off-thread and reclaim old allocations off-thread; dropping the last owned
  buffer in a callback would free memory. This is a future integration constraint,
  not a currently observed callback deallocation bug.
- Timebase bounds, signed preroll and absolute tick conversion are coherent;
  the scheduler maintains half-open blocks and bounded event storage. No sample
  clock arithmetic error was reproduced. Telemetry is deliberately nontransactional;
  an Acquire load of `finished` does not retroactively refresh earlier field loads.
- The two bus limiters have independent gain state; mapping validation prevents
  overlap and unmapped outputs are silent. The fixed 0.125 sample ceiling bounds
  samples but not intersample peaks or acoustic loudness. No current finite-input
  path to an unbounded output was found. Cue is post-deck-gain/pre-crossfader as
  documented, so deck gain zero also silences cue.
- The mixer is usable offline without a device, although it lives in the
  CPAL-dependent crate. `Renderer::next_frame` (`playback.rs:111`) and `render`
  (`:177`) duplicate source advancement; each mixer deck also contains unused
  playback telemetry. Consolidate the cursor implementation when introducing a
  third source type, rather than growing two separate synth/sample implementations.
  No generic graph/plugin abstraction or immediate crate split is warranted.

## Verification and decision

Reran `python scripts/validate_m1.py`: fmt/check/Clippy, all 35 tests, doc-tests and
the Windows inventory passed. Ran the short-source reproducer offline. Reviewed
the recorded M1.5 soak transcripts; did not rerun long or audible playback.
Cargo-deny remains unavailable. All findings distinguish code inspection or
offline reproduction from untested hardware scenarios.

GO to implement M2 incrementally after making the onset policy and event/clock
contract explicit. NO-GO to treat the current latest-value controls as a sequencer
command channel, to claim verified private cue/dropout-free performance, or to
distribute the current incomplete license bundle. No critical source patch was
made; this review adds documentation only.
