# M3.1–M3.2 implementation and verification report

Completed on Windows x64, Rust 1.85.1, 29 October 2026. The repository now has a
runnable native performance instrument: `cargo run -p dawj --locked`.
Choose an output and Start demo output; Copper Steps and Glass Tide load into
two fixed eight-slot decks controlling the existing `LiveEngine` and mixer.
No substitute audio engine or simulated playback path was introduced.

## Delivered behavior

- Sixteen visible interactive slots, with selected address, engine lifecycle,
  active identity/source position and pending identity/boundary. Launch, Stop,
  Cancel and Retrigger use the existing bounded command interface. Replacements
  are truthfully projected when present in engine state; this fixed demo has no
  variation/replacement browser.
- Shared transport status, current scheduler BPM and bar/beat, deck phrase
  position, four launch quantizations, persistent deck gain/cue, crossfader and
  existing master/callback diagnostics.
- VOLUME and FILTER contextual modes preserve focus and issue no audio commands
  on mode change. Gain and cutoff updates use existing engine smoothing. The
  nonresonant engine has no resonance control.
- Centralized keyboard performance bindings and Help. Text/numeric editing
  suppresses those bindings; ordinary button focus does not.
- Explicit device selection, stereo master-only and optional four-channel
  logical master/cue routing. Unsupported configuration/preparation/start errors,
  stream failure, stale publication, queue rejection/expiry and terminal closure
  remain visible. Restart creates a new clock and stopped demo.
- Pure projection/intent logic, real-engine integration tests, native rendering
  layout/input tests, an opt-in native delivery smoke mode, architecture notes
  and a reproducible performance walkthrough.

M2.9 was present and usable. Its coherent publication gained one current-tempo
word, with a sample-boundary regression. The M2.7 generator was extracted into a
public backend demo module so CLI, offline tests and GUI share identical asset
preparation. The existing scheduling, renderer, DSP and routing mechanisms remain
in place. The missing M0 architecture file is documented in the architecture
notes; the missing file did not require inventing a replacement state contract.

## Software verification

`python scripts/validate_m1.py` completed successfully with **97 tests**, including
the existing offline scheduling/renderer and M2.7 walkthrough regressions, plus
doc-tests and the exact Windows dependency inventory. Ignored log:
`target/m3-final-validation.log`; structured exits: `target/m1-validation.json`.
The validator runs these gates, with additional locked/offline/all-target checks:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Ten app tests cover actual engine-to-UI projection, submitted versus queued/
playing states, pending start/stop/retrigger/replacement, cancellation and stale
IDs, focus/deck memory, control requests, gain/filter reconciliation, crossfader
endpoints and invalid values, cue limitations, mode changes, queue exhaustion,
expired/rejected/closed receipts and shutdown. Two of those tests run egui
headlessly to check text-entry keyboard suppression and that all sixteen pending
cards fit 1280×820 and 1100×740 logical viewports. One additional timebase test
checks current BPM at rounded boundaries at 44.1/48/96 kHz.

The dependency gate passes **174 external packages** from Windows-filtered Cargo
metadata. The reviewed graph includes optional resolved nodes beyond the actual
Glow binary. Selected software licenses, font-specific terms and remaining
release notice collection are recorded in the policy/inventory. Cargo-deny was
not installed; no advisory scan or release-package audit is claimed.

## Native Windows and live audio evidence

The completed build was launched visibly through:

```powershell
cargo build -p dawj --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke_m3.ps1 -Play
```

The final run returned **exit 0**, `SMOKE_RESULT passed=true audio=true captured=true`.
It opened **Speakers (USB2.0 Device)**, 48 kHz, two-channel f32, default buffer,
master on channels 1/2, no cue. The automation uses the same `Intent` coordinator
as UI controls. It confirmed actual A1 and B4 playing sources, A1 gain 0.6 and
cutoff 500 Hz, cancelled B4's queued phrase stop, moved the crossfader to 0.7 and
selected FILTER. It captured the application viewport through eframe and closed
the stream/window cleanly. Capture was inspected for dark-theme contrast,
both grids and control visibility. Evidence is in ignored files
`target/m3-native-smoke.log` and `target/m3-native-capture.png`.

Final short stream diagnostics: 159936 rendered frames (about 3.33 seconds),
332 callbacks, maximum 1056 frames/callback, maximum callback render time
943.7 microseconds, zero nominal callback-budget exceedances, failed=false.
Master lifetime peaks were approximately 0.00125346 / 0.00100277. These are real
callback/render observations, not external audio capture or a listening result.

Earlier desktop-input harness attempts were discarded: helper windows and OS
focus made injected clicks/captures unreliable. The retained smoke mode captures
only its own viewport and checks actual engine state rather than claiming those
attempts passed. A light-Windows-theme contrast defect found during visible
inspection was corrected by explicit dark theme/style selection. Smoke gestures
wait for B's actual Playing state before testing its queued Stop: stopping a
still-queued launch has different valid engine semantics. Native smoke checks do
not replace the separate keyboard/GUI event tests or human performance testing.

## Realtime regression probe

The existing separate release probe was rebuilt and run successfully against
the extended snapshot: `cargo run --manifest-path tools/realtime-probe/Cargo.toml
--release --locked --offline`. Native exit was explicitly captured as 0; initial
PowerShell stderr redirection had misleadingly marked the shell invocation as
failed despite successful measurements. Final log: `target/m3-realtime-probe.log`.

| Frames/callback | Mean render µs | Maximum render µs | Nominal budget µs |
| --- | ---: | ---: | ---: |
| 32 | 10 | 25 | 666 |
| 128 | 33 | 114 | 2666 |
| 512 | 126 | 282 | 10666 |
| 2048 | 494 | 697 | 42666 |

The probe exercised sixteen active loops, sixteen rules and 32 incoming commands
per callback, 1000 callbacks per block size, four logical output channels at
48 kHz. It observed **zero render allocations and deallocations**, failed=false,
and zero first discontinuity step in the filtered EOF/stop fixtures. This is
offline stress evidence, not a loaded GUI hardware soak or dropout measurement.

## Remaining limitations and M3.3 steps

The software vertical slice and short stereo delivery check are complete. Human
listening, a five-/fifteen-minute performance usability trial, loaded sixteen-part
hardware soak, measured output continuity/latency, physical four-channel headphone
isolation, disconnect/sleep/recovery and Linux builds remain unverified. No claim
of stage readiness, headphone privacy or dropout-free sustained output is made.
The backend retains only a generic runtime stream failure flag. Terminal output
stop is an immediate gate rather than a click-free musical fade. Native-rate
sample loops do not stretch with tempo edits. The UI supports the fixed supplied
demo, 48 kHz output and viewports of at least 1100×740 logical points; it has no
composition importer, browser, controller hardware or arrangement editor.

The smallest M3.3 extension is a non-realtime MIDI input adapter mapping device
controls to `Intent`, with bounded parameter coalescing and stop/cancel admission
priority. Keep a single owner of `LiveControl`; project feedback from the same
coherent snapshots and explicitly handle expiry/rejection. Add controller pickup,
disconnect/reconnect and LED mapping tests, then run sustained UI/audio/controller
hardware and physical cue acceptance on the intended interface. No scheduler or
DSP rewrite is required for those mappings.

Changes are recorded as local logical Git commits. Nothing was pushed or merged.
