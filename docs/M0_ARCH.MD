# M0 architecture review and handoff

Implemented backlog issues 1–3: workspace/quality gates, timebase and offline
impulse scheduler. Three crates are sufficient for these responsibilities; empty
UI, asset, audio, MIDI and recording crates are deliberately deferred until used.

`timebase` owns signed 960-PPQ `BeatTicks`, signed timeline `SampleFrame`, immutable
piecewise-constant tempo maps and integer quantization. Beat zero maps to frame
zero; the first tempo extends into negative preroll. Subsequent tempo points must
be strictly increasing and nonnegative. BPM accepts finite 1–1000 values, rates
8000–384000 Hz. Tick conversion is bounded to ±2^40 and frames strictly inside
±2^52 to protect double-precision conversion. Samples round once, ties away from
zero; each periodic event derives from an absolute beat, never a rounded period.
Segment anchors retain fractional frames. Inverse conversion returns the nearest
storage tick, not sample-resolution microtiming. Tempo ramps are not implemented.

Quantization uses an explicit epoch and strict/inclusive boundary policy. Launch
callers can choose the next beat/bar/phrase, including a deck-specific epoch,
without floating epsilon logic. Overflow is an error. Musical time and device
callback counters are separate concepts; stop/resume needs an explicit mapping
before connecting a live audio host.

`event-engine` compiles at most 4096 impulses from beat targets and a **frozen**
tempo map. Construction/sort allocate outside rendering. `render` fills supplied
mono buffers of at most 8192 frames, scans a bounded immutable event vector, and
advances a cursor; it contains no allocation, filesystem access or locks. Blocks
are half-open; equal-frame impulses retain source order, sum in f64 and clamp to
[-1,1]. This is fixture protection, not a true-peak limiter. Events before the
chosen start are rejected rather than silently discarded. Empty buffers consume
nothing. Validation errors leave output/state untouched. Allocation-free source
inspection is not an audio-thread deadline/underrun guarantee. No hardware or
callback allocation instrumentation has been run.

`offline-render` owns allocations and WAV file I/O, generates 16 quarter-note
impulses with a change from 140 to 120 BPM after eight beats, then writes mono
PCM16. It prints actual event frame positions for inspection. This fixture proves
timing and is not a musical deck/player.

## Review findings

- Dependencies point inward: CLI → scheduler → timebase; neither core crate
  imports platform, UI or filesystem APIs. Core crates forbid unsafe code.
- Immutable tempo compilation is appropriate for offline M0. Before M2, retain
  future commands in beat space and translate near execution; never reuse this
  frozen frame list for editable live tempo. Add command IDs, bounded admission,
  cancel/replace ordering and acknowledgments as their own tested issue.
- Bounds are explicit. Conversion errors propagate to callers. Extreme horizons
  are rejected rather than overflowing integer casts. Current floating math is a
  prototype, not an arbitrary-duration exact rational transport.
- No third-party dependencies, network fetches, GUI or native audio linkage are
  required to build/test the current workspace once Rust is installed.
- Hardware independence: Windows 11 x64 is the initial user target. M1 should add
  a backend adapter (initial CPAL experiment), enumerate capabilities and expose
  configurable main/cue channel pairs. DSP and clock must remain independent of
  device brand and manufacturer drivers. Never infer a private cue path from a
  stereo device. Physical interface and four-channel validation remain open.

## Evidence and remaining gates

Local Linux Rust 1.85.1: `cargo fmt --all --check`, clippy with warnings denied,
workspace tests and the dependency inventory gate. Timing tests cover 44.1/48/96
kHz at 1/31/64/127/128/256/511/1024/8192-frame blocks against an independent
piecewise analytical reference, fractional tempo boundaries, 10,000-beat drift,
negative preroll, strict/inclusive quantization, deck epochs, collisions, empty
blocks, exact right edges, malformed inputs and overflow. WAV inspection is
performed separately with Python's standard-library `wave` module.

CI runs core checks on Windows and Linux and `cargo deny` on Linux. Hosted CI and
Windows execution are pending until the workflow actually runs; Linux success
does not establish a Windows audio-device result. M0 is ready for feature review;
M1 should start with device enumeration and silence-safe startup, then isolated
two-deck routing, smoothing and protective DSP. No M1 hardware gate is claimed.
