# Frontend latency validation

## Decision summary

The earlier 206.4 ms Tauri command outlier and separate 10.4 ms p95 did **not
recur** in three sequential 60-second release trials on a load-screened Windows
desktop. Tauri command p95 was 3.5-4.0 ms and worst was 6.4-11.2 ms. This is
evidence against recommending an architectural rewrite from the earlier samples.
It is not proof those tails can never occur.

The user reports playing Call of Duty during the earlier testing. Competing
CPU/GPU/driver scheduling is a plausible influence, but Web4 did not record the
necessary host traces to attribute its old outliers retrospectively. These trials
did not deliberately launch a competing workload or terminate user applications.

**Development/alpha default:** recommend Tauri, with egui retained as a fallback,
explicit alpha labeling and these diagnostic tools available. This is a
recommendation; no default package/frontend setting was changed.

**Live-show readiness:** not yet established. True input/state-to-pixel/audio
latency, physical device/controller recovery, isolation/listening, lower-power
systems and longer memory/driver soaks remain gates. The two decisions are separate.

## Conditions and reproducibility

Measured evidence is in [the result file](validation/web4_1-results.json), with
[prior Web4 results](validation/web4-results.json) retained for comparison.
Hardware/software: AMD Ryzen 9 9950X, Windows 26200.9711 / 25H2, pinned
Rust 1.93.0 x64 MSVC and locked Tauri/eframe dependencies. Final inspection found
WebView2 Evergreen 156.0.4314.50 and .58 installed, with .58 selected in the runtime
registry. Individual benchmark children were not version-stamped; do not assume
all used .50 merely because Web4 recorded it. An Evergreen patch difference is
an additional before/after confound, separate from competing load.
Audio used only **Speakers (HyperX Cloud III S Wireless)**, 48 kHz stereo master.
Missing/ambiguous named outputs fail; no sim-rig/default-device fallback occurs.

```powershell
cargo build --release -p dawj -p dawj-tauri --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/web4_1_trials.ps1 -Seconds 60 -Output HyperX
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/web4_reliability.ps1 -Seconds 1200 -Output HyperX -Run isolated-memory -Frontend dawj-tauri -ObserveHost
python scripts/web4_report.py target/web4-isolated-1 target/web4-isolated-2 target/web4-isolated-3 target/web4-isolated-profile target/web4-isolated-memory --preflight target/web4_1-preflight.json --output target/web4_1-results.json
```

The wrapper runs three trials per frontend sequentially, alternating frontend
order, followed by a separate profiled Tauri trial. Each starts a new process and
plays A1/B4 with the existing 20-batch/second gain/filter/crossfader sweep. Tauri
previews bundled/sample-PNG skins every ten seconds. No builds or other benchmark
sessions ran alongside these trials. Light documentation work continued during
the separate memory run; no intentional heavy workload was introduced.

Preflight CPU samples were 0%, 19%, 15%; busiest GPU-engine samples 0%, 1%, 0%.
No recognized game was detected. During the short trials, sampled CPU maxima
were 14-27% and GPU-engine maxima 2-10%. Egui trial 2 had 22% median CPU and
27% maximum: it was less idle than the others and is retained rather than hidden.
This is a practical low-load desktop comparison, **not a fully isolated lab**.
Game-name detection is incomplete, GPU counters may be unavailable on other
systems, and roughly six-second sampling can miss brief competing spikes.
The runner defers a busy preflight; it never closes a user's game to obtain a pass.

## Sequential release results

All values below are milliseconds. Quantiles are fixed-histogram upper bounds at
0.1 ms resolution; `<=0.1` does not mean a true median of exactly 0.1. Startup is
excluded. These are separate trials, not one pooled percentile distribution.

| Frontend / trial | Command median | p95 | Worst | Frame/update p95 | Worst frame/update |
| --- | ---: | ---: | ---: | ---: | ---: |
| Tauri 1 | 2.9 | 3.9 | 11.2 | 5.2 | 5.7 |
| Tauri 2 | 2.5 | 3.5 | 6.4 | 5.2 | 5.6 |
| Tauri 3 | 2.9 | 4.0 | 6.6 | 5.2 | 5.4 |
| egui 1 | <=0.1 | <=0.1 | 0.085 | 5.3 | 87.984 |
| egui 2 | <=0.1 | <=0.1 | 0.0274 | 5.6 | 76.324 |
| egui 3 | <=0.1 | <=0.1 | 0.0054 | 5.2 | 29.166 |

Tauri processed 9,861 steady parameter commands across the three trials; egui
processed 10,236. Different completed batch counts reflect timer/cadence variation,
not dropped assertions or a different requested scenario. Both decks stayed
active and all processes shut down successfully. Normal closes recorded no 1412
warnings. No physical listening/USB-controller result follows from those assertions.

Tauri snapshot roundtrip median/p95/worst was 2.9/3.7/6.5 ms,
2.7/3.4/23.4 ms and 3.0/3.8/8.8 ms. The isolated 23.4 ms snapshot is retained;
it did not coincide with a recorded >25 ms RAF/projection/long-task event.

Tauri's reply-to-two-RAF presentation proxy median/p95/worst was 6.9/8.2/8.5 ms,
7.4/8.3/9.0 ms and 7.0/8.0/8.5 ms. Egui dispatch-to-next-update was
5.0/5.3/6.093 ms, 5.0/5.8/11.822 ms and 5.0/5.2/5.983 ms. Those proxies
have different endpoints; neither is a compositor/GPU/pixel measurement.

## Attribution: what the extra instrumentation establishes

The diagnostic trial uses an opt-in bounded non-realtime probe beside the
unchanged typed IPC v1 reply. Normal admission, shutdown and receipt semantics
are preserved; normal requests allocate no timing probe. There is no additional
audio-callback work. The profiled trial is kept separate because observation
adds channels, clocks and JSON work.

| Profiled Tauri span | Median | p95 | Worst ms |
| --- | ---: | ---: | ---: |
| Software command to reply | 2.7 | 3.8 | 5.9 |
| Frontend serial dispatch to invoke | <=0.1 | 0.1 | 0.200 |
| Native helper total | 0.2 | 0.2 | 0.370 |
| Worker queue | <=0.1 | <=0.1 | 0.218 |
| Worker polling before handling | <=0.1 | <=0.1 | 0.222 |
| Worker handling / snapshot projection | <=0.1 | <=0.1 | 0.199 |
| Invoke roundtrip minus native span | 1.7 | 2.5 | 4.594 |
| DOM/Canvas projection callback | 1.0 | 1.3 | 1.700 |

The stage histograms contain 4,496 steady replies, including explicit snapshots;
the command histogram contains 3,372 parameter commands. Percentiles of stages
cannot be added as if they belonged to the same request.

The residual includes input/reply serialization, native runtime dispatch before
the helper, delivery back to WebView and JS continuation scheduling. It is **not**
a pure WebView transport cost. JS projection is measured separately. Input
injection bypasses physical keyboard/MIDI scanning; synthetic event handling is
not a hardware input benchmark. Egui directly calls the same Coordinator, so its
acceptance metric omits IPC/snapshot serialization by design.

No >25 ms Tauri command/invoke/RAF/projection or >=50 ms JS long-task trace
was recorded in the short diagnostic trials. There are therefore no extreme
events with which to establish a worker/memory correlation in those runs. The
host-data tool can align future bounded outliers to nearest host samples, but
coarse proximity alone does not establish causation. Native egui's transient
update gaps lack per-event timestamps in this harness; their precise cause
cannot be inferred from its aggregate statistics.

There is no monotonic audio-publication timestamp in the IPC contract. The
snapshot is coherent and its generation is retained, but publication-to-visible
pixel age cannot be reconstructed by subtracting GUI clocks. Engine frame means
next unrendered sample, not DAC presentation. Adding a GUI clock as a scheduling
authority would invalidate the engine contract rather than measure this gap.

## Quantization and delayed controls

A new deterministic regression submits the same launch policy before and after
a beat at 8 kHz / 120 BPM. Admission at frame 3900 launches at 4000; admission at
4100 launches at 8000. The intended gesture's wall time cannot backdate a request.
The existing scheduler remains authoritative and no DSP/scheduler behavior changed.

Thus a delayed launch/stop/retrigger can miss an imminent beat/bar/phrase. At
120 BPM that can add a whole 0.5-second beat, 2-second bar or 16-second eight-bar
phrase, depending on policy. Even a normal 3 ms request can miss a boundary if
operated too close to it. Callback-entry admission and rendering lookahead also
matter; these statistics do not promise a keypress-to-boundary guarantee.
Continuous mixer/filter targets use existing smoothing rather than musical
quantization. UI queued labels must continue to report engine-selected deadlines.

## Memory investigation

The separate 20-minute run completed 22,563 batches / 67,689 parameter commands,
with 120 skin previews and no overlapping native smoke/build sessions. Command
median/p95/worst was 3.0/4.2/26.9 ms; frame interval was 5.0/5.2/20.1 ms.
Held peaks were nonzero, both decks remained active, and native close was healthy.
Host CPU median/max was 12%/36%, GPU-engine maximum 5%, with no detected game.
The occasional background CPU activity means this is still not laboratory isolation.

| Memory observation | MiB |
| --- | ---: |
| First process-tree sample | 280.1 |
| Peak across 20 minutes | 309.6 |
| Median minutes 0-5 | 287.8 |
| Median minutes 5-10 | 301.5 |
| Median minutes 10-15 | 301.5 |
| Median final five minutes | 302.0 |

The post-ten-minute least-squares slope was **+0.709 MiB/min** over this shorter
window, compared with +0.240 in the earlier 30-minute shared-load run. It would be
incorrect to call that an improvement, a proven leak, or a directly comparable
trend. Five-minute medians show initial warmup and then a roughly 301.5-302.0 MiB
band; later samples repeatedly fell below earlier peaks. Eight processes remained
in the tree. Repeated allocation/GC/cache cycles and endpoint choice influence a
short fitted slope. The old 483.6 MiB peak was not reproduced without overlapping
resize/zoom/native smoke work; that is evidence about conditions, not a causal
heap attribution.

Two >25 ms command outliers (26.9 and 26.1 ms) occurred in this longer run.
Their nearest host samples reported CPU 26% and 25%, GPU 1%, approximately 1.12 s
and 0.016 s away. No corresponding >25 ms RAF/projection gap or JS long task was
recorded; projection worst was 2.7 ms. This weakens a sustained JS-render-stall
explanation for these two events, but native timing was deliberately off during
memory sampling, so worker wakeup, OS scheduling and bridge delivery cannot be
separated for them. Coarse host correlation is not evidence that CPU load caused
either delay. The result file retains event timestamps and nearest-sample offsets.
Use a longer `-Profile -ObserveHost` run if these smaller tails need attribution.

Fixed histograms/outlier retention, bounded worker queues, per-slot latest targets
and replacement of the same sample-skin ID were reviewed for unbounded growth.
No unbounded accumulation was demonstrated in those structures. Browser caches,
JIT/GC, native/GPU high-water marks and the shorter window remain plausible
contributors. No forced GC, periodic reload or ownership change was used to hide
retained memory.

This is not a heap-retainer trace or a multi-hour leak certification. Process
private bytes cannot identify a JS object leak or prove that all retained memory
is reclaimable. Do not force GC, periodically reload the app, or change engine
ownership to disguise growth. Use allocation/heap traces if repeated longer
isolated runs show a persistent post-warmup trend.

## Recommendations

**Default development/alpha frontend: yes.** The earlier tail is not reproducible
in these three short load-screened Tauri trials, feature parity and shared engine
ownership remain intact, and worker timing does not indicate a queueing bottleneck.
Retain egui, expose native skin reset, label the frontend alpha, and keep the
preflight/diagnostic tools available. Do not infer that Call of Duty is proven to
be the only possible cause of every historical stall. No broad IPC/frontend
rewrite or speculative dependency upgrade is justified by this evidence.

**Performance-ready for live shows: not yet.** Measure real MIDI/keyboard to audio
and state-to-pixel age, repeat on lower-power systems and mixed-DPI displays,
exercise physical audio/MIDI unplug/reconnect and driver-busy shutdown, validate
headphone isolation/listening, and run longer isolated memory sessions. Test
under realistic permitted background load as a separate robustness scenario.
No physical MIDI, private headphone cue, human usability or Linux-native result
is claimed. Driver calls outside audio can still block their owner indefinitely.

## Verification

Formatting, workspace check, Clippy with warnings denied, all 138 Rust workspace
tests, 33 Node tests, result-parser checks and the unchanged 347-package Windows
licensing inventory passed. Existing offline/render/scheduling regressions remain
enabled. The new tests cover opt-in probe reply/admission semantics and delayed
admission across a musical boundary. Native trials, not unit tests, supplied the
measurements above. No new musical feature, DSP change or default cutover shipped.
