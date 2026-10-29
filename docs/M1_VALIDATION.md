# M1 integration and stress validation

## Assessment

The engine passes deterministic software integration checks and the measured
Windows stream checks described below. It is a reasonable base for subsequent
software work, with physical main/headphone isolation, measured acoustic output,
dropout detection and demanding-system-load validation still open. This report
does not establish stage/performance readiness or zero dropouts.

No product feature was added for M1.5. Changes are a reproducible validation
runner, integration tests, callback diagnostics and a silent device soak tool.
M0 offline scheduling remains unchanged. No new dependency or enabled backend;
the seven-package Windows license inventory still passes.

## Reproduce

Run from the repository root with the pinned Rust 1.85.1 MSVC toolchain and
dependencies already fetched:

```
python scripts/validate_m1.py
# Adds a paused device matrix and real 900-second silent soak plus restart:
python scripts/validate_m1.py --hardware
```

The runner fails on unsuccessful commands, writes individual transcripts to
`target/m1-validation-N.log` and command/exit-code/duration records to
`target/m1-validation.json`. Hardware commands use zero-amplitude samples only.
Output device availability, Windows routing and shared-mode mix format remain
environment dependent; inspect devices again before using transient ordinals.

Individual checks, including custom device settings:

```
cargo run --release -p audio-backend --example soak --locked --offline -- --probe
cargo run --release -p audio-backend --example soak --locked --offline -- --run 900
# seconds, Hz, channels, buffer request, device ordinal:
cargo run --release -p audio-backend --example soak --locked --offline -- --run 2 48000 2 128
cargo run --release -p audio-backend --example soak --locked --offline -- --run 2 48000 4 default 5
```

Without `--run`, no stream is started. `--probe` only builds paused streams.
`--run` defaults to 900 seconds, two channels, 48000 Hz f32 and default buffering
on the OS default endpoint. Four or more requested channels use master [0,1]
and cue [2,3], labeled physically unverified; stereo is master-only. Silent
sources exercise the same synthesis, crossfade, gain, cue and limiter paths,
but do not test physical signal fidelity or streaming large PCM buffers.

Every 10 ms the control loop changes gains and crossfader targets, periodically
switches linear/equal-power curves and Off/A/B/Both cue selection. It verifies
monotonic delivered-frame telemetry, source completion, no reported asynchronous
error and progress within a five-second timeout. After completion it tests
repeated stop, rejection of restart on the terminal owner, stream destruction,
stable clock after destruction, and construction/start of a fresh one-second
stream with a new clock. Repeated start while running is also checked. This is
recreation/restart, not a new pause/resume transport feature.

## Automated offline results

The standard runner executes fmt, workspace/all-target check, Clippy with
warnings denied, workspace/all-target tests, doc-tests and the exact Windows
dependency inventory. The integration matrix compares full output against a
one-frame reference at 44100, 48000 and 96000 Hz with 1,31,64,128,511,1024,8192
and 16384-frame callback partitions. Sample-indexed automation occurs at the
same source frames regardless of partition. It checks stereo marker isolation,
independent source completion, silent unused outputs and EOF, finite bounded
samples, and exact source cursor accounting. Adapter tests separately verify
clock counts through enabled/disabled callbacks and new-engine reset at all
three rates on stereo and four-channel routes.

Existing tests retain coverage for crossfader endpoints/curves, gain smoothing,
source fades, cue pre-crossfader behavior, independent master/cue limiting and
meters, invalid maps/configurations and unsigned silence. Synthetic timing tests
check exact accounting at, above and without a nominal callback budget. These
are software assertions, not measurements of driver or DAC behavior.

## Callback diagnostic semantics

Each CPAL data callback measures its renderer execution using the monotonic
`Instant` clock, then publishes callback count, total render nanoseconds,
maximum render nanoseconds and over-budget count via native x64 atomics.
The nominal budget is that callback's complete frames divided by requested
sample rate. Empty callbacks are counted but have no budget comparison. Counters
saturate instead of wrapping; one callback writer uses loads/stores without
retry loops. Monitor fields are independent observations, not a coherent snapshot.

Timing excludes the diagnostic publication itself, OS work before entry/after
return, and callback scheduling delays. Clock reads and counters add measurement
overhead. No callback logging, allocation, file access or mutex was introduced.
Source inspection supports that claim; no allocator/deadline instrumentation was
installed. Duration below a nominal budget does not prove a hardware deadline
was met. Over-budget events are not a dropout count; missing callbacks, driver
glitches, presentation discontinuities and output silence cannot be inferred
from these counters. The delivered-frame clock is not a DAC timestamp.

## Actual Windows observations

Windows x64, Rust 1.85.1 MSVC, CPAL 0.15.3/WASAPI, optimized release soak build.
The default endpoint was Speakers (HyperX Cloud III S Wireless). All actual
playback in this task used silent samples. The paused probe matrix covered
44100/48000/96000 Hz, two/four channels, default/128/512/zero buffer requests:
only the three nonzero/default 48000 Hz stereo configurations built on HyperX;
the remaining 21 requests were explicitly rejected. The probe fails if no
tested configuration builds, or if a zero-frame buffer request is accepted.

Short silent runs with 128 and 512 requested frames on HyperX each completed
96000 source frames and a fresh 48000-frame restart with no reported error.
Largest observed callbacks were 1056 frames for both requests. Fixed requests
therefore do not prove actual endpoint buffer or callback lengths. The DualSense
four-channel endpoint also completed the same short run/restart at 48000 Hz f32
with default buffering. Software channel isolation is tested offline; that silent
stream run does not establish connector mapping or private headphone monitoring.
The additional paused probes and short stream runs occurred during the long soak;
there was no controlled CPU/disk-load generator or workload isolation.

The initial 900-second soak completed 43200000 source frames (43200096 delivered
frames), 89999 callbacks and 88468 control updates in 900.063 seconds including
shutdown checks. Total measured render time was 1931232600 ns; maximum 372400 ns;
nominal render-budget exceedances: 0; reported asynchronous errors: none. Its
fresh one-second stream also completed and passed destruction/clock checks.
The initial PowerShell wrapper nevertheless returned exit code 1 without a
harness error in the transcript. That wrapper status is unresolved and is not
treated as a clean aggregate validation result. A full rerun through the Python
runner captures the actual Cargo child exit status separately.

Raw initial/short-run evidence is in [validation](validation/). The raw timing
values are measurements from this machine/run, not performance guarantees.

## Authoritative full-run result

`python scripts/validate_m1.py --hardware` completed with exit code 0. All eight
child commands returned 0, including all 35 software tests, the paused device
matrix and the final 900-second soak plus restart. See the captured
[command results](validation/m1-results.json) and
[final soak transcript](validation/m1-final-7.txt). The earlier wrapper ambiguity
does not affect this independently recorded clean result. Both full-duration
soaks completed; this second run is the authoritative aggregate gate.

| Final soak measurement | Result |
| --- | --- |
| Completed source frames | 43200000 |
| Delivered frames including final silence | 43200096 |
| Callback count | 89999 |
| Parameter updates | 88415 |
| Wall time including shutdown checks | 900.058 seconds |
| Total measured renderer duration | 1936185900 ns |
| Mean renderer duration | approximately 21.51 microseconds |
| Maximum renderer duration | 525700 ns (0.5257 ms) |
| Nominal render-budget exceedances | 0 |
| Reported stream errors | none |
| Largest observed callback | 1056 frames |
| Fresh-stream restart | 48000 source frames, completed, exit 0 |

Final harness assertions also confirmed zero logical master/cue peaks for the
silent sources, terminal-owner restart rejection, and stable frame telemetry
after stream destruction. This measures software execution, not zero dropouts.
No engine defect was reproduced within the exercised cases. The harness was
hardened to fail when every probe configuration fails, when a zero buffer request
is accepted, or when a nominally silent run produces nonzero logical bus peaks.

## Remaining validation gates

Physical main/cue separation still requires an appropriate independently routed
interface and controlled listening or loopback capture. As described in
[M1.4](M1_4.md), the current HyperX endpoint is stereo-only and the four-channel
DualSense is not a validated main/headphone interface. ASIO requirements remain
dependent on the eventual device and its exposed WASAPI outputs; ASIO is disabled
and any added crates/SDK require a separate compatibility review.

No claim is made about zero dropouts, audible quality, physical latency,
underruns under load, unplug/recovery, hardware presentation-clock continuity,
or long nonzero/multichannel playback. M1 currently has a source-frame cursor and
delivered sample clock, not a musical live transport/seek/loop implementation.
Cargo-deny is not installed locally, so no new advisory scan was available.
