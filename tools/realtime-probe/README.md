# Realtime render probe

Run from the repository root:

```
cargo run --manifest-path tools/realtime-probe/Cargo.toml --release --locked --offline
```

This separate GPL-3.0-or-later diagnostic executable keeps its instrumented system
allocator outside the engine's unsafe-forbidden workspace. Allocation/free counts
are enabled only around render calls; nonzero counts fail assertions. Preparation,
control submissions, output formatting and engine destruction are outside the
watched interval. No device stream is opened and no audible output is produced.

It reproduces filtered EOF and stop steps, then measures 1000 callbacks at each
of four sizes with sixteen active parts, installed rules and full command batches.
Results are offline timing observations, not hard realtime or zero-dropout proof.
Its independent lockfile uses the same pinned CPAL dependency; run its formatting
and Clippy checks explicitly because it is not a workspace product target.
