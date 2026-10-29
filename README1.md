# DAWJ

A GPL-3.0-or-later prototype for a two-deck live composition instrument. The
current **M0 foundation** implements deterministic musical timing and an offline
impulse renderer. Live audio, cue routing, playback and GUI are future milestones.

Install Rust with rustup; the repository pins Rust 1.85.1 with rustfmt/clippy.
The core and demo have no external crate dependencies. On Windows 11 x64, install
the Rust MSVC toolchain and Visual Studio C++ Build Tools / Windows SDK.

```sh
cargo test --workspace --locked --offline
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
python scripts/audit_dependencies.py
cargo run --locked --offline -p offline-render -- impulses.wav 48000 128
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
