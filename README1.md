# DAWJ

A GPL-3.0-or-later prototype for a two-deck live composition instrument. The
current foundation implements deterministic musical timing, an offline impulse
renderer, output discovery, realtime playback, a stereo two-deck mixer and
independent master/cue channel routing. Advanced effects and GUI remain future work.

Install Rust with rustup; the repository pins Rust 1.85.1 with rustfmt/clippy.
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
