# Build, test and contribute

## Windows prerequisites

The supported development target is Windows 11 x64 with Rustup, the MSVC toolchain,
Visual Studio C++ Build Tools/Desktop development with C++, a Windows SDK and
Microsoft WebView2 Runtime. `rust-toolchain.toml` pins Rust 1.93.0 and its
rustfmt/Clippy components. Use the checked-in lockfile and pinned frontend versions.

From the repository root:

```powershell
cargo build --release -p dawj -p dawj-egui --locked
cargo run --release --locked
# Egui fallback:
cargo run --release -p dawj-egui --locked
```

The first build needs registry/network access. Add `--offline` once dependencies
are cached. No Node runtime, npm build, development server or Tauri CLI is needed
to run either application. Node and Python are used for validation tools.

Choose the intended output in the application before starting. Tauri starts empty:
create/load WAV compositions through [local sessions](../user-guide/local-sessions.md),
or explicitly select the supplied demo. Egui retains the demo performance fallback. See the [performance guide](../user-guide/performance.md).

## Checks

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
node --test crates/tauri-app/web/model.test.mjs crates/tauri-app/web/protocol.test.mjs crates/tauri-app/web/performance.test.mjs crates/tauri-app/web/skins.test.mjs crates/tauri-app/web/reliability.test.mjs crates/tauri-app/web/content.test.mjs
python scripts/audio_dependencies.py
python scripts/test_web4_report.py
python scripts/test_alpha_layout.py
python scripts/check_docs.py
cargo run -p dawj-core --example web_contract --locked --offline -- --check
```

The workspace tests include offline scheduler/rendering, mixer/DSP, publication,
MIDI, IPC and frontend projection regressions. Generated performance-contract
fixtures are checked against Rust types. Do not weaken tests to make a build pass.
The Windows dependency inventory check is not a vulnerability or final release audit.

The [Jam1 end-to-end script](../../scripts/jam1_smoke.ps1) tests real session
authoring, save/close and reopen in two application processes. Native smoke and
release tests open real audio. Frontend automation requires a
unique **HyperX** output by default and fails if missing/ambiguous; it never falls
back to the default endpoint. Set `DAWJ_TEST_OUTPUT` or the runner's `-Output`
explicitly for a different test device. Other old audio examples have their own
device arguments: read `--help` before opting into playback.

See [performance validation](performance-validation.md) for release trials, host-load
preflight, measurement definitions and manual hardware gates. Do not benchmark
while playing a game, compiling, or intentionally loading the CPU/GPU.

## Local alpha builds

See the [repeatable Windows procedure](windows-alpha.md) and
[cutover review](alpha-cutover.md). Tauri is the default `dawj` application;
`dawj-egui` remains an independently runnable fallback. Alpha status does not
clear packaging/licenses or certify a live-show hardware setup.

## Contributing

Start with the [architecture](../architecture/overview.md),
[state contract](../architecture/performance-state.md) and
[vision/roadmap](../design/vision.md). Keep changes focused and include reproduction
steps, relevant tests, and the hardware/OS used for audio or controller findings.
Separate a proposed product decision from an implemented capability.

Preserve the callback constraints and bounded application admission. New controls
must use semantic actions and authoritative state rather than mutate engine internals.
Review dependency and asset licenses before adding them; see [licensing](licensing.md).

Linux frontend, audio/MIDI, packaging and mixed-display behavior require separate
validation. Windows results are not Linux certification; native platform
dependencies and even fresh-clone build details must be verified there.

## Documentation convention

- `README.md`: concise public introduction and quick start.
- `docs/`: maintained user/contributor guides, original specifications, contracts
  and selected reproducible engineering evidence.
- `AGENTS.md`: concise persistent coding-agent instructions useful in a fresh clone.
- `.dev-notes/codex/`: local, Git-ignored implementation reports, handoffs,
  scratchpads and superseded run summaries. No essential build information belongs here.
- `target/`: generated builds, raw measurements, captures and validation output.

New permanent behavior/API/architecture changes update the relevant tracked guide.
Ephemeral milestone reports go in `.dev-notes/codex/` by default. The focused
[latency investigation](../M3_WEB4_1_LATENCY.md) remains tracked as engineering
evidence explicitly requested for the frontend decision. Historical reports remain
available in Git history; existing local copies are archived rather than destroyed.
