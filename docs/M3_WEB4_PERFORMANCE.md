# M3.Web4: Tauri performance and reliability

Web4 hardens the two existing frontends without adding musical features, changing
the engine callback, IPC schema, scheduler or DSP. Rust 1.93.0, pinned Tauri and
all dependencies remain unchanged. Egui remains the default.

## Fixes and test boundaries

The initial release pilot reproduced `previous parameter update awaiting engine`
when gain and cutoff were operated back to back. Shared Coordinator dispatch now
merges later GUI/keyboard targets into the existing bounded per-slot target array,
just as MIDI already did. One parameter publication per slot, four flushes per
tick, 24 continuous receipts and eight reserved discrete receipts are preserved.
Validation happens before acceptance. The engine snapshot still determines
execution; accepting a target is not a claim that audio has applied it. A new
offline regression sends 20,000 GUI parameter gestures before rendering, keeps
Stop admissible, and verifies the final gain/filter and stopped engine state.
A second regression
reproduced an acknowledgement-to-flush gap replaying an older target. New input
now also merges whenever a waiting target already exists, so the latest value
cannot be overwritten by that older publication.

Automated audio now requires a unique case-insensitive **HyperX** output match.
Missing/ambiguous matches fail closed. Both smoke and reliability paths use this
policy; the normal application's interactive device selection is unchanged.
The actual test endpoint is **Speakers (HyperX Cloud III S Wireless)**, stereo
master only. No sim-rig/default-output fallback is permitted.

The success-report helper now closes the WebView window normally instead of
calling application exit from inside the IPC callback. This is a focused test
harness lifecycle correction, not an upstream WebView2 patch. Error reports
retain nonzero exit. The control worker still owns audio shutdown on native exit.

## Reproduction

Use Windows PowerShell from the repository root. These scripts open native GUI
windows and actual audio. Keep other demanding applications out of timing trials.

```powershell
cargo build --release -p dawj -p dawj-tauri --locked --offline
# Equivalent sequential short trials; 60 seconds per frontend:
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/web4_reliability.ps1 -Seconds 60 -Output HyperX -Run sequential
# 30 minutes PER frontend, sequential by default (about one hour):
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/web4_reliability.ps1 -Seconds 1800 -Output HyperX -Run soak
```

`-Frontend dawj-tauri` / `-Frontend dawj` select an individual application. Two
such runs can overlap using distinct `-Run` names; label those as shared-load
soaks rather than isolated timing comparisons. There is no auto-reconnect to an
unrelated output. `DAWJ_TEST_OUTPUT` and `DAWJ_TEST_SECONDS` are restored afterward.

Both release scenarios use the same 1280×820 window, resident demo, immediate
launches of A1/B4, and a 100-step crossfader/gain/filter sweep at approximately
20 batches/second. Every command reaches the existing Coordinator. Both decks
must remain active, the engine must remain available, and native shutdown must
complete. A passing harness result means these lifecycle assertions passed; it
does not automatically certify the proposed latency gates. Controls and rendering run on their actual frontend threads. Tauri
also previews/cancels a theme every ten seconds, alternating bundled colors and
the portable PNG sample, exercising fetch/validation/decode during playback.
There is no JavaScript audio or replacement engine.

The harness bounds duration at 3600 seconds and times out after duration+90.
Timeout kills only the process it launched and fails the run; this is test
containment, not a claim of bounded OS driver teardown. Fixed-size 0.1 ms timing
histograms avoid per-frame sample accumulation. Each has an overflow bin at
1000 ms; quantiles hitting that bin are lower bounds, while worst is exact.
Quantiles are rounded upward to the next 0.1 ms. Device startup measurements
are separated from steady-state command measurements.

The Windows runner samples root-plus-descendant process private bytes and working
set every approximately five seconds, including WebView browser/render/GPU
children. Sampling includes CIM overhead and is not a heap leak detector. Memory
after ten minutes is summarized with a least-squares slope; warming and browser
caches must be distinguished from a persistent slope.

```powershell
python scripts/web4_report.py target/web4-sequential target/web4-sequential-steady target/web4-soak-tauri target/web4-soak-egui target/web4-final-regression --output docs/validation/web4-results.json
```

## Timing definitions and measured results

Results and completed durations are recorded in
[the measured JSON](validation/web4-results.json). Hardware: AMD Ryzen 9 9950X,
Windows build 26200.9711 / 25H2, WebView2 156.0.4314.50, Rust 1.93.0 x64 MSVC.
The Windows registry's legacy ProductName says Windows 10 Home; the build is
reported here explicitly rather than relying on that label.

Command timing is software gesture injection to accepted/rejected command reply:
Tauri includes serialized frontend admission, WebView IPC, worker dispatch,
snapshot serialization and reply; egui directly calls the same Coordinator.
These are the two production paths under equivalent operations, not identical
transport overhead. Device startup is excluded from steady-state statistics.
Keyboard scanning, hardware MIDI transport, DSP ramp completion and DAC output
are not included. The crossfader reaches existing engine-smoothed mixer targets;
offline tests verify smoothing, not hardware click-free listening.

`receipt_to_frame_ms` is a **presentation opportunity proxy**, not a photon
measurement: Tauri waits two requestAnimationFrame callbacks after reply/DOM
projection; egui measures until its next UI update after dispatch. Egui's next
update can precede buffer presentation. Neither proves compositor/GPU completion.
`frame_ms` measures requestAnimationFrame cadence or native update cadence, not
GPU render duration. Continuous repaint is requested during the egui trial.
Tauri automation receives snapshots with command replies and explicitly polls
once per batch; the normal idle poll loop starts after automation, so this is a
documented automated workload, not a claim to reproduce every idle polling cost.

Snapshot roundtrip is separately measured in Tauri. The existing coherent
publication is copied once and timeline projection uses its generation. There is
no cross-process monotonic publication timestamp in IPC v1: true audio-publication
to visible-pixel age cannot be derived from GUI frame clocks. Engine `frame`
is the next unrendered sample, not a DAC timestamp. That absolute latency remains
a readiness gate; no new callback work was added to fabricate a measurement.

### Sequential release comparison (60 seconds each)

No other DAWJ automated runs overlapped these sequential trials. This was an
ordinary Windows desktop session, not an isolated scheduling laboratory. The
final `sequential-steady` trial excludes device-start frames consistently in both
frontends. Values are milliseconds; presentation proxies have the limits above.

| Measurement | Frontend | Median | p95 | Worst observed |
| --- | --- | ---: | ---: | ---: |
| Input injection to command reply | Tauri | 4.0 | 10.4 | 41.5 |
| Input injection to direct acceptance | egui | <=0.1 | <=0.1 | 0.020 |
| Reply to two RAF opportunities | Tauri | 6.4 | 8.0 | 54.0 |
| Dispatch to next UI update | egui | 5.0 | 6.7 | 10.961 |
| Frame/update interval | Tauri | 5.0 | 5.2 | 60.1 |
| Frame/update interval | egui | 5.1 | 6.7 | 144.955 |
| Snapshot IPC roundtrip | Tauri | 3.9 | 10.2 | 66.9 |

Tauri completed 1,172 control batches / 3,516 steady commands; egui completed
1,128 / 3,384. Startup's longest command took 4,271.3 ms in Tauri, including
preparation/device opening, excluded above. Six skin previews occurred while
playing. Both decks remained active and both applications exited successfully.

The earlier sequential trial is retained in JSON: Tauri command median 4.4,
p95 6.6 and worst 206.4 ms, with snapshot roundtrip 4.4 / 6.5 / 9.6 ms.
Its RAF collection included device startup, so its 189.9 ms frame maximum is not
used for the steady-playback frame comparison. That startup exclusion changes
instrumentation only: it does not fix or excuse the earlier 206.4 ms command tail.
The proposed p95 and worst-case cutover gates are **not met across the trials**.
Profile WebView main-thread, compositor, worker scheduling and OS preemption
before a default switch. No broad rendering rewrite was made without a trace
attributing those stalls.

### Completed 30-minute release soaks

Both frontends ran for 1,800 seconds with A1 and B4 playing, concurrently on the
same HyperX endpoint. Early soak time also included functional resize/zoom smoke
and builds on the same host; this is a shared-load stress run, not the sequential
comparison. The soak Tauri RAF summary also includes device-start time; use the
normalized sequential trial for frame comparison. No physical routing/isolation
claim follows from two shared streams.

| Measurement | Tauri | egui |
| --- | ---: | ---: |
| Completed control batches | 35,474 | 34,076 |
| Steady parameter commands | 106,422 | 102,228 |
| Command median / p95 / worst ms | 3.5 / 5.0 / 53.3 | <=0.1 / <=0.1 / 0.199 |
| Presentation proxy median / p95 / worst ms | 6.4 / 8.0 / 55.5 | 5.0 / 5.7 / 15.263 |
| Frame/update median / p95 / worst ms | 5.0 / 5.2 / 55.1 | 5.0 / 5.7 / 85.079 |
| Snapshot IPC median / p95 / worst ms | 3.5 / 4.9 / 42.0 | Not an IPC path |
| Private memory first / peak MiB | 280.4 / 483.6 | 105.2 / 105.2 |
| Last five minutes median private MiB | 317.8 | 104.6 |
| Post-ten-minute memory slope MiB/min | +0.240 | +0.008 |
| Process-tree size observed | 8-9 | 2 |
| Normal-close 1412 warnings | 0 | 0 |

Tauri performed 179 theme previews, alternating bundled color and decoded local
PNG packages. Its memory peak occurred during the early overlapping functional
checks; shared WebView caches/processes and allocator high-water marks have not
been heap-profiled. The later memory range fluctuated rather than growing
monotonically, but the small positive slope is **not proof of leak freedom**.
Repeat isolated multi-hour runs and attribute retained memory before cutover.

Egui's monitor recorded 180,047 callbacks, zero over-budget callbacks, and a
0.3552 ms maximum callback processing duration during its soak. The Tauri
performance IPC does not expose these counters; its no-failure checks/held peaks
are not a substitute for callback-budget evidence. Both streams reported active
engine publications and closed cleanly. The long soaks preceded the final
acknowledgement-to-flush regression correction; that additional edge case was
validated separately with the failing-then-passing offline test. A full additional
30-minute run after that one-line correction was not claimed. Both final release
binaries also passed a five-second two-deck regression, retained in JSON as
`final-regression`; those short samples are not used for the timing comparison.
Hardware underruns and audible dropout
absence were not independently measured.

## Coverage and remaining hardware work

All MIDI flood/encoder tests here use synthetic bytes or mapped events through
production code. No physical controller or OS virtual-MIDI loop was connected.

| Area | Automated coverage | Remaining validation |
| --- | --- | --- |
| Admission / input floods | 24 frontend calls, 24 latest keys, 32 worker permits; 100,000 coalesced frontend moves while IPC stalls; bounded release latch tests | Real controller/driver packet rates and human control feel |
| MIDI / encoders | Real bounded callback bridge, 32,000 mapped CC changes, relative modes, pickup, stale epochs and disconnect reset tests | Physical USB unplug/replug, LED behavior, maximum-rate hardware encoders |
| Keyboard | Editing guards, auto-repeat, modifier/key release, blur/visibility release; native focus-loss smoke | OS shortcuts, Alt-Tab and focus switching with an actual performer |
| Crossfader / audio | Shared mixer targets, ongoing two-deck playback, existing smoothing regressions | DAC/listening test; physical master/headphone isolation |
| Skins / assets | Native palette matrix, live preview, PNG decoding, reset recovery | Largest permitted skin assets and slower storage/GPU combinations |
| Resize / DPI | Five viewport sizes; WebView zoom 125%, 150%, 200% with reported DPR and control reachability | Moving between mixed-DPI physical monitors, GPU/driver coverage |
| Audio disconnect | Existing CPAL fault/terminal admission and silence tests; native stop/start/relaunch with fresh stopped parts | HyperX unplug/replug while playing, device renumbering, real stream-error callbacks |
| MIDI reconnect | Epoch resets/pickup/feedback refresh tests and named-port reconnect loop inspected | Real controller loss/reappearance and output-send failure |
| Shutdown | Full admission queue, stalled worker initialization, abandoned replies, out-of-band release/closed admission tests; native clean exits | A permanently stalled vendor driver call is not cancellable in-process |
| Memory | Process-tree sampling throughout extended playback; fixed-size instrumentation | Multi-hour runs, heap allocation traces if a slope persists |

No audio device was physically disconnected remotely. Driver calls occur outside
the audio callback, but an in-progress OS enumeration/open/send/close can still
hold the control worker or its shutdown join. Bounded queues prevent unbounded
admission; they cannot bound a stalled driver. The test barrier is eventually
released and is not evidence that an infinitely blocked driver can be cancelled.
Native resident audio assets prepare on the worker; loading a second composition
into a running engine is not an existing feature and was not added.

For hardware validation, start the supplied two-deck demo on an explicitly named
output, then unplug/replug that device. Confirm a visible error/unavailable state,
no successful-looking silent continuation, and recovery by Stop, Rescan, selecting
the same named endpoint and Start. Old parts must not resume automatically.
With a mapped controller, hold a temporary mode and move encoders while
disconnecting it; confirm the base mode returns and pickup rearms after reconnect.
Check state-driven LEDs against queued and executed boundaries. Repeat with the
window unfocused, during a skin preview and while quitting. Perform four-channel
master/cue loopback and a separate physical headphone isolation/listening check;
stereo software meters cannot establish isolation.

## WebView2 teardown 1412

The old forced-exit release pilot reproduced `Chrome_WidgetWin_0` unregister error
1412. Microsoft defines it as ERROR_CLASS_HAS_WINDOWS, meaning the class still
has open windows ([Win32 error reference](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--1300-1699-)).
Normal window close instead of `app.exit(0)` is consistent with allowing window
destruction before class cleanup. This is an inference from the changed lifecycle
and observed logs, not proof of Chromium internals. Completed-run warning counts
are retained in the measured JSON; no pinned dependency was upgraded to guess at
a fix. Error-exit/forced-kill paths may still log it. Test child-process exit and
repeat launch/close across supported WebView2 releases before declaring it resolved.

## Verification

Windows workspace formatting, check and Clippy with warnings denied passed.
All **136 Rust workspace tests**, **33 Node tests** and the Python result-parser
test passed. Existing realtime/offline scheduling, renderer, filter, smoothing,
MIDI and egui tests remain enabled. The generated IPC fixture drift check passed;
the Windows licensing inventory remains **347 external packages**.
Native functional smoke passed in both frontends, including actual HyperX
playback and clean output shutdown. Resize, zoom and keyboard checks are
functional tests; their debug IPC samples are deliberately excluded from the
release comparison below.

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
node --test crates/tauri-app/web/model.test.mjs crates/tauri-app/web/protocol.test.mjs crates/tauri-app/web/performance.test.mjs crates/tauri-app/web/skins.test.mjs crates/tauri-app/web/reliability.test.mjs
python scripts/test_web4_report.py
python scripts/audio_dependencies.py
cargo run -p dawj-core --example web_contract --locked --offline -- --check
cargo build -p dawj -p dawj-tauri --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke_frontends.ps1
```

## Readiness gates for default cutover

1. Repeat sequential release trials on a lower-power Windows system. Initial
   targets: command p95 below 10 ms, worst below 50 ms during steady control;
   frame p95 within two refresh periods. Treat these as proposed gates, not
   guarantees on all hardware. Keep startup/driver/theme stalls separate.
2. Instrument or externally measure authoritative state-to-pixel age and hardware
   input-to-audio timing. Confirm publication freshness, queued boundary accuracy,
   compositor stalls and crossfader ramp behavior under controller load.
3. Pass physical audio and MIDI disconnect/reconnect, focus-loss, mixed-DPI,
   controller LEDs, headphone isolation and listening checks. Preserve an explicit
   stereo monitoring limitation.
4. Repeat 30-minute release soaks in isolation, then multi-hour sessions; require
   no unexplained growing process tree or sustained post-warmup private-memory
   slope. Investigate any budget overrun with callback diagnostics before release.
5. Repeat clean close, busy close, device startup failure and quit-during-driver
   operations. Document noncancellable driver limitations and require recovery on
   next launch. Keep the native skin reset and egui fallback available.

Linux native rendering/audio/MIDI and release comparisons were not run: this is
a Windows environment without Linux audio/display hardware. Shared pure/offline
tests do not certify ALSA/PipeWire, WebKitGTK, Wayland/X11 or Linux device teardown.
No packaging, default cutover or new musical features are included in Web4.
