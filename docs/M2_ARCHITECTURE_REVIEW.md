# M2.8 engineering review

Reviewed `e7a4e26` on Windows x64, 2026-10-29. Read the product/technical
specification, prototype plan, M0 architecture and M1/M2 milestone contracts;
inspected the timebase, both schedulers, Part reducer, source preparation,
renderers, DSP, routing, CPAL adapter, tests and performance examples.
Line references below refer to that commit.

**Recommendation: conditional GO for M3 development; NO-GO for declaring the
full performance interface ready.** Preserve the current core. Resolve findings
1–3 before treating UI state and routine performance transitions as dependable.
There is no demonstrated need for a framework rewrite. Hardware acceptance and
release packaging remain separate gates. No P0 memory-safety, unbounded-loop or
normal-render allocation/locking defect was demonstrated. Production code was
not changed; the issues below warrant targeted follow-up, not an unsolicited
redesign during this review.

## Findings, in priority order

### 1. P1: filtered EOF and manual interruption bypass output smoothing

**Demonstrated signal discontinuity.** `crates/audio-backend/src/live.rs:525`
removes a source immediately at EOF, while `part_dsp.rs:131` can still contain a
nonzero low-pass state. Source preparation fades (`samples.rs:177`) occur before
the filter, so a zero final input does not imply zero final output. Similarly,
`live.rs:498` removes a manually stopped voice without a release, and `:493–497`
resets a retrigger/replacement cursor immediately. Gain-target smoothing does not
smooth these ownership/state transitions. Existing documentation acknowledges
manual interruptions; filtered natural EOF is an additional case.

Reproduction: prepare a mono float WAV of 4096 samples all equal to 1 at 48 kHz,
OneShot, `BoundaryPolicy::Fade { frames: 64 }`; set the slot to a 20 Hz low-pass
before launch; render 4097 frames through the default mixer. Measured master L
at frame 4095 is `0.0050794967`, followed by exactly zero at 4096. A separate
sixteen-loop probe stopping one settled slot produced a one-sample change of
`-0.005524285`. These are measured discontinuities, not a claim about audible
severity on headphones; no listening test was performed.

Minimal correction: define a short bounded **post-filter** release for EOF and
stop, and a bounded transition for retrigger/replacement. Keep the requested
musical action at its exact frame and specify the audible-tail semantics. Do not
delay the scheduler boundary to hide the discontinuity. Add discontinuity tests
with DC/low-frequency fixtures, filter enabled, both buses, and callback splits
at the action frame. This does not require an effects rack.

### 2. P1: accepted scheduler state can say Playing while rendering silence

**Demonstrated public-API correctness defect; not exercised by the shipped demo.**
`crates/audio-backend/src/live.rs:271` accepts any `MusicalScheduler`, but `:326`
initializes every audio cursor to None. It neither rejects already-playing
scheduler state nor initializes its audio cursors. At `:494–497`, an event whose
Part is absent from the bank silently maps to None; the reducer has already
transitioned to Playing and the engine does not fail.

Two reproductions using public APIs:

1. Queue an Immediate launch on a scheduler for Part 1; pass it to
   `LiveEngine::new` with a bank containing only Part 2; render 1024 frames.
2. Queue Part 1, call `scheduler.due(|_| {})` before construction, then pass that
   scheduler with a matching Part 1 bank; render 1024 frames.

Both constructors succeed. Both produce `Playing`, `source_position=None`,
all-zero output and `failed=false`. Rules installed before construction can
likewise reference a nonresident Part. Existing bank checks validate duplicates,
rate and size, not the scheduler/bank relationship.

Minimal correction: for this milestone, explicitly require a pristine scheduler
(no active/pending parts or installed rules) and reject violations at construction;
alternatively validate and initialize every supported preconfigured state.
Missing bank resolution during dispatch must be an explicit failure rather than
a successful silent launch. Add constructor tests for active, queued and rule
states. The rejection approach is the smaller change.

### 3. P1 M3 integration gate: diagnostics cannot serve as authoritative UI state

**Confirmed API limitation; mixed-generation reads are a concurrency risk, not
an observed failing race in this review.** `crates/audio-backend/src/live.rs:70`
exposes a state code, one PartId and consumed-frame position. `:551–565` publishes
the active Part for QueuedReplace, omitting the pending Part, command ID and
musical deadline. `:137–169` reads separate atomics without a consistent snapshot.
The code correctly labels this diagnostic-only. Ticket rows expire after reuse
(`:108–113`); they are not durable applied-event history. Global/deck/part beat
queries are on the callback-owned scheduler, not the control-side snapshot.

Reproduction: launch Part 1, queue replacement by Part 2 at a future phrase, then
inspect `LiveControl::slot`: it supplies Part 1 and QueuedReplace but cannot tell
the consumer which Part is pending or its due beat. After enough later commands,
the replacement's ticket also becomes Expired even while still pending. Inferring
state only from local intent fails when automation, rejection or EOF intervenes.

Minimal correction before wiring contextual controls/LEDs: publish a bounded
coherent actual-state record including active/pending identity, deadline and
pending ID plus the relevant clock/phase. Keep writer work bounded; readers may
retry outside realtime. Retain diagnostic meters separately. Do not put UI
access behind an audio-thread mutex. Test snapshot consistency and rejected,
superseded, expired and rule-origin actions. This is an interface contract gap,
not a request to implement the GUI now.

### 4. P2: tempo history imposes a lifetime editing ceiling

**Confirmed, documented limitation.** `crates/timebase/src/lib.rs:101–112` rejects
insertion at index 64 and never retires historical segments. Starting with one
segment, 63 edits at successively later distinct beats exhaust storage; replacing
an existing/future segment can still succeed. A long performance tempo control
will eventually reject all further forward edits even though old segments are
no longer needed for future rendering. Repeating the same BPM still consumes a
segment at a new beat.

Reproduce with existing test
`tempo_edits_reject_past_invalid_and_exhausted_storage_transactionally`
(`crates/audio-backend/tests/quantized.rs:336`). Rejection is transactional and
does not stop playback; this is not corruption or drift.

Minimal correction before continuous tempo controls: define bounded retention of
past tempo history while preserving fractional anchors and all required phase
queries; keep archival history off-thread if needed. Until then expose the limit
and rejection clearly. Increasing the constant merely postpones exhaustion.

### 5. P2: stream failure/terminal stop does not close live command admission

**Confirmed by control-flow inspection; no device disconnect injected here.**
The error callback (`crates/audio-backend/src/cpal_backend.rs:335`) sets the stream
Shared failure flag. The render gate (`:260–269`) then bypasses LiveEngine.
`stop` (`:223`) also disables rendering. Neither closes the separate LiveControl
queue. It closes only on an engine scheduling failure or engine destruction
(`live.rs:602–610`). Retaining a stopped/failed stream owner therefore allows
successful submissions that will never be consumed; tickets remain Pending until
row reuse/destruction instead of promptly reporting Closed.

Minimal correction: propagate terminal device lifecycle to command admission,
or require the control owner to destroy the failed stream immediately and gate
all submissions using PlaybackMonitor. Test terminal stop and injected adapter
failure with pending tickets. The existing demo exits on monitor failure, so it
avoids prolonged exposure; do not assume the same of a persistent GUI.

### 6. P1 for distribution only: incomplete GPL text remains unresolved

**Confirmed existing packaging defect.** `LICENSE:128` ends mid-sentence in
section 1. `docs/GPL_COMPATABILITY_AND_RELEASE_POLICY.md:6` incorrectly says the
complete GPL text is present. Restore the complete canonical GPLv3 text and
verify release notices/source packaging before distribution. This does not block
local M3 engineering. The current Windows inventory gate passes; no new dependency
or asset licensing issue was introduced by M2 or this review.

## Verified strengths and architectural limits

- **Realtime ownership:** one consumer owns scheduler, cursors, filters and mixer.
  The SPSC publication order uses release/acquire indices; slots cannot be reused
  until consumption is published. Ingress snapshots the producer once and admits
  at most 32 commands. Receipt updates use one CAS attempt, not a retry loop.
  The bank is retained for the engine lifetime; ordinary stop/retrigger does not
  destroy PCM. Decode, preparation, stdin, formatting and WAV writes are outside
  render. Rust unsafe code is forbidden in workspace crates.
- **Bounded work:** `event-engine/src/live.rs:380` dispatches at most 16 manual
  actions per frame, scanning 16 slots for each selection; `:202` checks 16 rules.
  The bank search is bounded by 64 voices, tempo lookups by 64 segments, trace by
  128 records. Work scales with supplied frames; a finite bound is not a deadline
  guarantee. Idle slots skip sample/filter DSP, though scheduler scans remain.
  Equal-power trigonometry is recomputed per frame (`mixer.rs:290`); profiling
  supports caching unchanged coefficients later, not a speculative rewrite now.
- **Scheduling/state:** current tests cover exact half-open boundaries, Immediate
  eligibility, same-frame cancellation, conflicts, replacement, EOF, tempo edits,
  recurring/manual precedence and independent decks. Beat targets are retained
  across tempo edits; source cursors are not the musical clock. Inverse tick
  rounding is explicitly handled. No additional timing defect was demonstrated.
  The M0 frozen impulse scheduler remains independent and deterministic.
- **Phase:** global frame time advances while all parts are stopped. Deck epochs
  define phrase position; part start ticks define nominal musical phase. Sample
  loop length is a separate domain. Loops run at native prepared rate, so tempo
  changes can desynchronize audio cycles from musical phase by design. No stretch
  or resampling is implied. Input arriving after a callback snapshot waits until
  the next callback; no OS-input timestamp accuracy is promised.
- **Assets/DSP:** WAV input is bounded to 64 MiB, decoded PCM to 128 MiB per asset,
  and resident bank PCM to 256 MiB. The latter is not a total process/temporary
  preparation memory cap. Mono/stereo decoding, finite amplitude checks, half-open
  loop ranges and rate-mismatch rejection are explicit. Gain/mute and filter
  coefficients ramp over 20 ms; the one-pole recurrence is stable and nonresonant,
  retaining state on edits. Denormal-sized filter state is flushed. Boundary
  releases are the separate defect above.
- **Mix/routing:** fixed 1/8 summing avoids normalization jumps when slots change.
  Both buses retain conservative 0.125 sample ceilings, independently limited;
  cue is post deck gain/pre crossfader. Mapping validates disjoint pairs and
  stereo fallback never folds cue into master. Sample-peak limiting is not a
  true-peak, acoustic-volume or hearing-safety guarantee.
- **Future MIDI/patterns:** reuse BeatTicks/TempoMap and explicit deck/slot addresses.
  The current one-pending-action-per-slot reducer is an arrangement lifecycle,
  not a polyphonic note scheduler. A future note path needs its own explicit
  bounded event/voice budget, ordered note-offs and input timestamp policy. The
  32-entry command queue rejects saturation explicitly; dense knob traffic can
  crowd stop/cancel requests. A non-RT coordinator/coalescing policy is needed
  before attaching high-rate controllers. No new abstraction is justified yet.
- **Dependencies:** all workspace packages declare GPL-3.0-or-later. The reviewed
  Windows graph remains seven packages: CPAL 0.15.3 and its existing permissive
  dependencies, with Apache-2.0 selected in the inventory. No ASIO SDK, third-party
  demo audio or new crate was added. The metadata gate is not a full release
  notice audit or current vulnerability scan; cargo-deny was not run.

## Validation performed

`python scripts/validate_m1.py` succeeded: fmt check, workspace/all-target check,
Clippy with warnings denied, **80 tests**, doc-tests and exact Windows dependency
inventory. Log: ignored `target/m2_8-validation.log`. No production edits required
a second validation run. Tests include sixteen-part summing, independent routing,
44.1/48/96 kHz partition matrices, tempo/rule retiming, simultaneous producer and
consumer activity, and the M2.7 arrangement script. Passing tests did not cover
the constructor and filtered-EOF probes above.

A separate ignored diagnostic crate at `target/m2-review` exercised the public
APIs and wrapped the system allocator with counters enabled only around render.
Run `cargo run --manifest-path target/m2-review/Cargo.toml --release --offline`.
It is review tooling, not part of the workspace or final harness. Probe results
are in `target/m2_8-probe-final.log`; reproduction recipes above do not require
keeping those build artifacts.

The final stress probe used 16 active looping slots, 16 installed every-beat rules,
16 pending phrase retriggers, and a full batch of 32 commands per callback
(16 filter/gain updates and 16 retrigger requests). Four-channel master/cue output,
48 kHz, one resident 4096-frame mono-derived source, default equal-power mix,
1000 callbacks per size. Release build, Rust 1.85.1, Windows x64; reported CPU
identifier `AMD64 Family 26 Model 68 Stepping 0, AuthenticAMD`.

| Frames/callback | Mean render us | Maximum render us | Nominal budget us |
| --- | ---: | ---: | ---: |
| 32 | 10 | 216 | 666 |
| 128 | 33 | 175 | 2666 |
| 512 | 130 | 257 | 10666 |
| 2048 | 518 | 1323 | 42666 |

Integer microseconds are truncated. Zero watched allocations and deallocations
were observed across these 4000 render calls; engine failure remained false.
This is an offline microbenchmark on the control process, not a CPAL callback
measurement, exhaustive allocator proof, maximum-bank/cache-pressure benchmark,
15-minute soak or dropout measurement. Rules can be skipped by the deliberately
pending manual actions; existing tests separately exercise rule dispatch/overflow.

## Hardware gates before M3 acceptance

No audible playback or new hardware stream/soak was run during this review.
M2.7's prior paused builds established HyperX stereo and DualSense four-channel
48 kHz f32/default-buffer construction only. The pinned CPAL WASAPI source was
inspected locally: `src/host/wasapi/device.rs:372–400` derives capability probes
from the endpoint mix format; `:687` uses shared mode. Endpoint channel counts
do not establish physical jack mapping. Whether the intended DJ interface needs
ASIO remains hardware-dependent; do not choose that backend from the DualSense
report alone. Enabling it requires a separate SDK/dependency review.

Required physical checks:

1. Sustain a 15-minute M2 run with all sixteen slots, active filters, rule/manual
   collisions, rapid controls and tempo edits under realistic UI/CPU load. Record
   actual callback-size distribution, device/configuration, timing maxima and
   budget exceedances. The old M1 silent soak does not establish M2 load safety.
2. Measure output continuity with external capture or suitable loopback/driver
   telemetry. Correlate glitches with callback timing; zero render-budget
   exceedances alone cannot support a zero-dropout claim.
3. Verify physical master/cue isolation and channel order on the intended
   multichannel interface, with crossfader endpoints and both cue selections.
   Repeat supported rate/buffer configurations rather than assuming exposure.
4. Test unplug, default-device changes, sleep/resume, failure visibility and
   destruction/recreation. Confirm command closure and muted recovery behavior;
   restart currently creates a new transport, not seamless session recovery.
5. At low monitored level, audition starts/stops/retriggers, filtered EOF,
   loop joins and rapid parameter moves after the targeted corrections. Measure
   input-to-output latency and assess the five-minute M3 performance walkthrough.

M3 may proceed incrementally with these gates visible. Do not build authoritative
UI/LED behavior on diagnostic-only fields or call the current engine
performance-ready merely because offline tests pass.
