# DAWJ

[M3.Web1 shared frontend architecture](docs/M3_WEB1_IMPLEMENTATION.md) keeps egui
as the default and adds a parallel Tauri frontend using the same Rust control layer:

```sh
cargo run -p dawj --locked
cargo run -p dawj-tauri --locked
```

The repository pins Rust **1.93.0** for both frontends. Tauri on Windows also
requires Microsoft WebView2. [M3.Web2](docs/M3_WEB2_REPORT.md) exposes the complete
basic performance surface, keyboard modes, MIDI settings and shared musical
timeline. Both applications use the existing audio engine; egui remains the
default fallback. Follow the [five-minute Tauri walkthrough](docs/M3_WEB2_WALKTHROUGH.md).

The [native M3 performance application](docs/M3_GUI_ARCHITECTURE.md) controls the
existing audio engine with two visible eight-part decks. Run it on Windows:

```sh
cargo run -p dawj --locked
```

Choose an output and press **Start demo output** to load Copper Steps and Glass
Tide. [Performance walkthrough and keyboard map](docs/M3_PERFORMANCE_WALKTHROUGH.md).
Stereo output is master-only; independent cue requires four-channel routing and
separate physical verification. [Implementation and verification report](docs/M3_IMPLEMENTATION_REPORT.md).

[M3.3–M3.4 hands-on controls](docs/M3_HANDS_ON_CONTROLS.md) add generic MIDI
input/output, learn, persistent mappings, state feedback, soft pickup and held
VOLUME/FILTER modes. Open **MIDI: off/on** for settings. No controller is needed:
`cargo run -p dawj --example synthetic_midi --locked --offline` exercises the
real engine through synthetic MIDI. [MIDI implementation report](docs/M3_3_4_IMPLEMENTATION_REPORT.md).

[M3.5 phrase visualization](docs/M3_5_PHRASE_VISUALIZATION.md) adds a repeating
eight-bar timeline, zoom/follow, pending-action and fill markers, selected-part
inspection and direct periodic-rule controls while keeping both decks visible.
Choose a four- or eight-bar engine before starting output.
[M3.5 verification report](docs/M3_5_IMPLEMENTATION_REPORT.md).

[M2.9 hardening report](docs/M2_9_HARDENING_REPORT.md) and
[performance state contract](docs/PERFORMANCE_STATE_CONTRACT.md) define the
coherent read-only API for future UI/controller feedback and bounded audio transitions.

[M2.7 playable demo and walkthrough](docs/M2_7.md) combine two eight-part procedural
compositions with a temporary performance CLI. Render the reference transition
without starting audio:

```sh
cargo run -p audio-backend --example performance --locked --offline -- --render target/performance.wav --script docs/examples/m2-performance.txt
```

Use `--play` explicitly for live input or scripted playback; `--help` lists controls.

[M2.6 periodic triggers and fills](docs/M2_6.md) add bounded declarative once,
every-N and phrase-bar rules with explicit manual-command precedence and phase.

[M2.4/M2.5 part mixing and controls](docs/M2_4_M2_5.md) add per-slot state/meters,
smoothed gain/mute and a stable low-pass filter to the existing sixteen-part
live engine. The `part_controls` example provides an offline stdin/WAV interface.

[M2.3 quantized scheduling](docs/M2_3.md) connects the Part lifecycle to M0's
musical clock with bounded live command delivery and sample-accurate actions.
Run `cargo test -p audio-backend --test quantized --locked --offline` for its
offline scheduling and concurrent-command checks.

[M2.1 Parts and M2.2 sample playback](docs/M2_1_M2_2.md) define the eight-slot
state model, transition table, resident WAV loader, one-shot/loop semantics and
offline/opt-in audio demo. Sample rates must match the selected stream rate.

[M1 integration validation report](docs/M1_VALIDATION.md): reproduce software
checks with `python scripts/validate_m1.py`. Add `--hardware` for paused probes
and a 15-minute silent device soak with callback diagnostics and stream restart.

A GPL-3.0-or-later prototype for a two-deck live composition instrument. The
current foundation implements deterministic musical timing, an offline impulse
renderer, output discovery, realtime playback, a stereo two-deck mixer and
independent master/cue channel routing. M3 adds the native performance window;
advanced effects remain future work.

Install Rust with rustup; the repository pins Rust 1.93.0 with rustfmt/clippy.
The timing and offline renderer have no external crate dependencies. On Windows 11 x64, install
the Rust MSVC toolchain and Visual Studio C++ Build Tools / Windows SDK.

```sh
cargo test --workspace --locked --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
python scripts/audio_dependencies.py
cargo run --locked --offline -p offline-render -- impulses.wav 48000 128
```

Realtime demos (two seconds, amplitude 0.1 tone or prepared M0 impulses):

```sh
cargo run -p audio-backend --example playback --locked --offline -- tone
cargo run -p audio-backend --example playback --locked --offline -- buffer
cargo run -p audio-backend --example playback --locked --offline -- --help
```

See [M1.2 verification and constraints](docs/M1_2.md) for settings and clock semantics.

The [M1.3 mixer demo](docs/M1_3.md) adds smoothed deck gains, configurable
crossfading, conservative master limiting and stereo peak meters. It produces
no sound without explicit playback opt-in:

```sh
cargo run -p audio-backend --example mixer --locked --offline -- --play
```

[M1.4 routing and WASAPI findings](docs/M1_4.md) document master/cue mapping,
the explicit master-only stereo fallback, and the open physical headphone gate.
Inspect current capabilities without playing audio:

```sh
cargo run -p audio-backend --example routing_check --locked --offline
```

The last command writes about 7.43 seconds of mono PCM16 impulses and prints the
intended tick and actual frame of every event. Optional arguments are output
path, sample rate and block size; `--help` describes limits. WAVs are ignored by
Git. The impulses are a timing fixture, not a finished musical demo.

- `crates/timebase`: tempo map, signed beat/frame types, epoch-aware quantization.
- `crates/event-engine`: bounded offline event compilation and block rendering.
- `tools/offline-render`: runnable synthetic WAV fixture.
- [Architecture review and handoff](docs/M0_ARCHITECTURE.md).
- [Prototype plan](docs/source/PROTOTYPE_PLAN.md) and
  [product design](docs/source/PRODUCT_AND_TECHNICAL_DESIGN.md), preserved inputs.
- [Provisional GPL compatibility policy](docs/GPL_COMPATIBILITY_AND_RELEASE_POLICY.md)
  and [NOTICE](NOTICE).

The supplied [schema](schemas/track.schema.json) and
[example manifest](examples/example.duettrack.manifest.json) are design fixtures.
Their referenced assets are absent and checksums are placeholders; import and
semantic validation are not implemented. Initial hardware target is Windows 11
64-bit with an as-yet unselected interface. Audio backends and channel routing
must remain hardware independent.

M1.1 adds CPAL output discovery only. See docs/M1_1.md and
`cargo run -p audio-backend --example inspect_outputs --locked`.
Core timing/render crates remain independent of CPAL. CPAL and the Windows
transitive dependency inventory are in docs/windows-dependencies.json; run
`python scripts/audio_dependencies.py` for the Windows x64 inventory gate.
The earlier M0 no-external-dependencies statements describe the original baseline.
