# M2.9 realtime hardening

Implemented against M2.7 (`e7a4e26`) after reproducing the M2.8 findings on
Windows x64, Rust 1.85.1. The original review is retained as historical evidence.
The performance API is specified in [PERFORMANCE_STATE_CONTRACT.md](PERFORMANCE_STATE_CONTRACT.md).

## Findings and corrections

- Reproduced both constructor cases: Playing with no cursor, all-zero audio and
  failed=false. LiveEngine now requires a pristine scheduler at frame zero with
  no active/pending slots or installed rules. It rejects unsupported handoff
  instead of silently accepting domain state without audio cursors. Every later
  source reference originates in the immutable resident bank through validated
  commands/rules, so missing-bank dispatch is prevented at admission. This is an
  explicit startup contract, not an attempted reconstruction of elapsed playback.
- Reproduced filtered EOF master step `-0.0050794967` and single-slot stop step
  `-0.005524285`. Both first discontinuity steps are now zero in the same probe.
  A bounded 2 ms post-filter held-output transition removes abrupt voice removal
  and cursor reset. It uses one stereo record per slot, never an additional voice,
  and performs no stopped-source/filter DSP. Scheduling stays sample accurate.
  New DC/low-pass tests constrain adjacent-sample steps on master and cue, assert
  exact event frames, compare block sizes and require silence after the release.
  This is de-clicking, not a claim of perceptually perfect arbitrary edits.
- Added typed coherent performance snapshots with initial state, global clock,
  per-deck phase, all sixteen lifecycle/identity/cursor/parameter records, pending
  action kind/identity/beat/ID, transition duration and generation. A bounded SC
  atomic publication requires no writer locks/retries or allocation. Concurrent
  readers reject overlapping publications rather than combining generations.
- Reproduced 64-segment tempo capacity exhaustion with the existing test. On a
  successful future edit, history before the segment containing the current
  frame is now retired in-place while preserving absolute fractional anchors.
  999 sequential edits pass a new anchor regression. At most 64 retained/future
  segments remain; excessive simultaneous future intent still rejects
  transactionally. History queries before the retained segment explicitly return
  OutOfRange. M0 frozen maps are unchanged; use them for archival conversions.
- Confirmed stream-stop/error admission gap by inspection and exercised the shared
  terminal flag in a fault-injection test. The CPAL stream owner and its error
  callback now close the same admission flag used by LiveControl and Ticket.
  Start failure, terminal stop and destruction close admission; unresolved
  tickets report Closed even if no later render occurs. Commands racing closure
  may submit just before it and then resolve Closed. Actual unplug/recovery was
  not tested on hardware.
- Obtained canonical GPLv3 from `https://www.gnu.org/licenses/gpl-3.0.txt` over
  HTTPS. Verified header, section 17 and complete 35149-byte document, and copied
  its bytes unmodified to LICENSE. SHA-256:
  `3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986`.
  Git attributes prevent newline conversion of LICENSE. Workspace metadata
  remains GPL-3.0-or-later; no dependencies were added.

## Validation

`python scripts/validate_m1.py` passes fmt check, workspace/all-target check,
Clippy with warnings denied, **86 tests**, doc-tests and the unchanged seven-package
Windows dependency inventory. Log: `target/m2_9-final-validation.log`.
The validator uses locked/offline Cargo commands, including stricter all-target
coverage where applicable. Regression coverage includes pristine-start rejection,
muted playback vs stopped state, replacement identity/beat/ID, cancellation,
retrigger timing, coherent simultaneous sixteen-cursor reads, bounded transitions,
tempo retirement and terminal admission. Two previous immediate-silence assertions
were updated to require exact source removal plus bounded release and eventual
silence; event timing, source-work and routing assertions remain enforced.

The existing offline M2.7 arrangement and callback-size comparisons pass. The
review's release-mode allocator/timing probe was rebuilt against these changes:
16 active loops, 16 installed rules, 16 pending phrase retriggers, 32 commands per
callback, stereo master plus cue, 48 kHz, 1000 callbacks per size.

| Callback frames | Mean us | Max us | Nominal budget us |
| --- | ---: | ---: | ---: |
| 32 | 11 | 49 | 666 |
| 128 | 37 | 75 | 2666 |
| 512 | 144 | 396 | 10666 |
| 2048 | 562 | 1030 | 42666 |

Zero watched allocations/frees across these 4000 render calls; engine failed=false.
Log: `target/m2_9-stress.log`. The probe is preserved as a separate diagnostic
crate; run `cargo run --manifest-path tools/realtime-probe/Cargo.toml --release --locked --offline`.
Its allocator hooks are isolated from the unsafe-forbidden engine crates and
assert zero render allocations/frees. It measures offline render work, not WASAPI delivery. Timings
include coherent snapshot publication. Variance, maximum bank/cache pressure,
long sessions and real UI/OS contention require further testing.

## Remaining gates and M3 readiness

**GO for M3 interface development against the published contract.** No GUI or M3
feature was implemented. The API now supplies actual coherent operational state
without requiring the UI to interpret reducer internals. Snapshot generations are
not durable performance history, and mixer targets still use their established
separate latest-value control path. High-rate controller traffic still needs a
non-RT coordinator to handle explicit queue-full rejection/coalescing.

No audible playback, new device stream or hardware soak was performed. A loaded
15-minute sixteen-part M2 run, actual callback distribution/timing under UI load,
external/loopback output-continuity evidence, physical four-channel cue isolation,
latency, disconnect/sleep/restart and listening tests remain acceptance gates.
Offline results do not establish live dropout-free operation or headphone privacy.
ASIO remains hardware-dependent and disabled. License text is repaired, but full
release notice/source packaging and cargo-deny/advisory review remain release work.
