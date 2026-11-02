# Performance validation

Use release builds and the same scenario for both frontends. Functional debug
smoke is useful for correctness, not a frontend latency comparison. The
[controlled latency study](../M3_WEB4_1_LATENCY.md) records measured results,
limits and recommendations. [Earlier results](validation/web4-results.json) are
retained as engineering evidence; they include uncontrolled/shared-load sessions.

## Reproduce

```powershell
cargo build --release -p dawj -p dawj-tauri --locked --offline
# Three trials per frontend, alternating order, then an opt-in profiled trial:
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/web4_1_trials.ps1 -Seconds 60 -Output HyperX
# 30 minutes per frontend, sequential by default:
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/web4_reliability.ps1 -Seconds 1800 -Output HyperX -Run soak -ObserveHost
# A single 20-minute memory investigation:
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/web4_reliability.ps1 -Seconds 1200 -Output HyperX -Run isolated-memory -Frontend dawj-tauri -ObserveHost
```

Scripts open actual audio. A uniquely matched HyperX output is required by default;
missing/ambiguous devices fail, without changing the system default or selecting
another output. `-Output` explicitly changes that test requirement. No unrelated
applications are terminated. Close games/heavy loads before latency trials.

The trial wrapper samples host CPU/GPU before starting and defers if CPU average
exceeds 20%, a GPU engine exceeds 30%, or a recognized game is running. Game-name
detection is incomplete; sampled low load is not proof of total isolation.
Per-trial host CSV records CPU, busiest GPU engine and game detection alongside
process-tree private memory. Sampling can miss short spikes and adds observer cost.

Both frontends use the prepared demo, A1/B4, immediate launch, and roughly 20
crossfader/gain/filter batches per second. Tauri also previews bundled/sample-PNG
skins. No alternate scheduler or JavaScript audio is used. The automated workload
uses command replies and an explicit snapshot per batch; normal idle polling is
suspended until automation returns. Keep that definition when comparing runs.

## Definitions

- **Command:** software injection to accepted/rejected reply in Tauri; direct
  Coordinator acceptance in egui. These are different production paths, not equal
  transport costs. They exclude hardware key/MIDI scanning and audible execution.
- **Frame/update interval:** RAF cadence or native update cadence. Continuous
  repaint is requested for egui. Neither measures GPU rendering time.
- **Presentation proxy:** two RAF opportunities after reply/DOM projection in
  Tauri, next UI update after dispatch in egui. These are not pixel/DAC timestamps.
- **Snapshot:** coherent engine publication, separately timed roundtrip in Tauri.
  Engine frame is the next unrendered sample, not audio-device presentation time.
- **Profile mode:** gated native diagnostic requests measure worker queue, poll,
  handling and total native duration. Residual bridge time includes serialization,
  runtime dispatch and JS resumption; it is not a pure IPC wire measurement.

Timing histograms have fixed storage, 0.1 ms upper-bound quantiles and a 1000 ms
overflow bucket; worst duration is retained exactly. Startup is separate from
steady command/frame measurements. Diagnostic outlier traces are bounded at 64;
long-task/RAF/projection events have wall timestamps only for correlation, never
musical scheduling. Compare profiled runs separately because observation adds work.

## Interpretation and gates

The engine schedules against admission at its unrendered frontier. Even a short
control delay can miss an imminent boundary; late commands must not be backdated
from a UI clock. Regression tests explicitly compare admissions before/after a
beat. Queued labels remain authoritative, not promises tied to a keypress timestamp.

Memory summaries include browser/GPU descendants, warmup, caches and allocator
high-water marks. A small positive slope is not a leak diagnosis. Compare
post-warmup ranges, process counts, themes, load and longer runs before changing
architecture. A passing script means lifecycle assertions and exit succeeded,
not that every proposed live-performance gate passed.

Before live-show readiness, require true input/state-to-audio/pixel measurements,
lower-power Windows trials, physical MIDI/LED and audio reconnect tests, mixed-DPI
focus/release tests, cue isolation and listening, longer soaks and healthy shutdown
during device operations. Driver calls outside audio can still stall their owner;
in-process cancellation of a permanently blocked driver is not guaranteed.

For functional native checks:

```powershell
cargo build -p dawj -p dawj-tauri --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke_frontends.ps1
```

Those checks cover the actual UI, keyboard releases, software stop/start/relaunch,
skin recovery, five viewports and WebView zoom. They do not certify physical
controllers, headphone isolation, human usability or Linux behavior.
