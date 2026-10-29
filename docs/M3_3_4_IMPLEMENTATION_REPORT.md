# M3.3–M3.4 implementation report

Implemented on Windows x64 with the pinned Rust 1.85.1 toolchain. This milestone
stops at M3.4: generic MIDI input/output, learn, persistence, state feedback,
contextual controls and pickup are integrated with the existing native app and
audio engine. Run `cargo run -p dawj --locked`; open **MIDI: off/on** for settings.
The [hands-on guide](M3_HANDS_ON_CONTROLS.md) contains the configuration workflow,
factory map, relative-encoder convention and manual controller acceptance steps.

## Starting baseline and audio correction

The M3.1–M3.2 README, architecture, M2.9 snapshot contract and actual engine/UI
interfaces were inspected. Before adding MIDI dependencies, the full baseline
validator passed **99 tests**, formatting, all-target checking, Clippy and the
174-package Windows inventory after the user's low-audio report was corrected.
The level correction is a separate commit: +36 dB smoothed pre-limiter demo trim,
with both existing output ceilings retained. A single default bass now measures
-24.05 dBFS peak / -32.18 dBFS RMS instead of approximately -60 dBFS peak. The
native smoke check now requires a useful peak above 0.02, rather than merely
nonzero audio. Backend/CLI default trim remains 0 dB. No listening result is
inferred from these numeric checks.

## Integrated implementation

- **One semantic path:** `Intent` plus `Coordinator::dispatch(intent, source)`
  serves GUI, keyboard and MIDI. Addressed `PartAction` controls either deck/slot
  without stealing GUI focus. Existing engine commands and coherent snapshots
  retain scheduling/playback authority. No second scheduler or audio engine exists.
- **MIDI ports:** pinned midir 0.10.3 on a dedicated worker, WinMM on Windows.
  Input normalization supports channel-aware Note On/Off and CC, including
  velocity-zero releases. The callback owns only a bounded ingress adapter;
  no audio/control engine handle is captured. Device discovery, connect, close,
  retry and output sending occur on that worker. The app remains functional
  with no input device.
- **Mappings and learn:** manual channel/type/number/action/target editing,
  button/absolute/relative mode, raw range, inversion, relative step, transactional
  learn, conflict rejection and Save/Load. Versioned bounded JSON stores mappings,
  LED definitions and desired unique port names. Saved devices reconnect at
  startup; named copies of the profile support different setups. Invalid saved
  data remains on disk and produces a visible factory-map fallback message.
- **Context:** VOLUME/FILTER persistent modes and source/token-tagged held modes.
  Press/release, repeats, overlapping holds, focus loss, profile changes,
  input overflow and disconnect cleanup are explicit. Selected-part controls and
  both fixed grids keep the same positions. Main controls remain independent.
- **Pickup:** absolute values cannot take over until near/crossing the current
  target. Resolved parameter changes and external edits rearm pickup. Relative
  two's-complement CC deltas modify the current target with bounded steps.
  Gain and cutoff changes to one slot merge through bounded in-flight/latest
  intent; displayed playback/accepted parameters still come from the engine.
- **Feedback:** ordinary configurable Note/CC values project all five part
  lifecycles from actual coherent snapshots. Queued stop/retrigger remain distinct
  from Playing. Initial/reconnection refresh, changed-state caching, bounded
  batches and queue-admission retry avoid routine flooding. Shutdown attempts
  configured stopped values before closing output. No vendor RGB/SysEx is assumed.
- **GUI:** one MIDI badge opens a separate configuration window. Pickup/rejection
  detail, connection state, learned/conflicting input, dirty mappings, profile
  path and drop count are visible. HELD/base-mode feedback supplements the active
  mode highlight. Rendering and semantics remain separate; no skin framework exists.

The public [pinned midir API](https://docs.rs/midir/0.10.3/midir/) was checked along
with local source. All new software dependencies are MIT/Apache-2.0 selections;
the exact Windows inventory now has **185** external resolved packages. See the
GPL policy's M3.3–M3.4 section for versions, notices and platform limits. No
proprietary virtual MIDI driver, SDK or firmware is bundled.

## Boundedness and failure semantics

| Boundary | Bound / behavior |
| --- | --- |
| Profile | 64 KiB; version 1; 128 mappings; 32 LED entries |
| MIDI ingress | 256 Copy packets; try-send; counted drops |
| UI input drain | 128 events/update |
| Worker command queue | 8 commands; explicit rejection on saturation |
| Parameter intent | 16 latest targets; merge gain/cutoff per slot |
| Parameter admission | 4/update, round-robin; one outstanding parameter/slot |
| Receipt window | 32; continuous admission stops at 24 to reserve discrete room |
| Engine queue | Existing 32 commands, unchanged |
| Held modes | 16 source/token records; repeat presses are idempotent |
| MIDI output | Changed values; at most 16 messages/50 ms in normal operation |
| Port presence/retry | Every 2 seconds; first 64 ports/side inspected |

Input packets carry a connection epoch. Old connection messages are dropped after
reconnect; overflow discards untrusted queued input and resets pickup/holds so a
lost release cannot strand a temporary mode. Discrete action rejection remains
visible. Profile revisions discard the old event batch; learning consumes held
button repeats until release, preventing an accidental launch immediately after
learning. Discrete actions are
never silently retried as a successful launch. Only latest
continuous targets are retained for later admission. Round-robin prevents hot
Deck A knobs starving Deck B. Output cache advances only after bounded queue
admission; send failure triggers a reconnection refresh. A failed/disconnected
device cannot be promised to receive a final LED-off message.

The worker's status mutex, channels, device calls, file I/O, serialization and
allocation are all outside the audio thread. The app polls one coherent snapshot
per update as before. No new locks, queues, allocation or MIDI work were added to
`LiveEngine::render`. The separate audio-level correction adds only fixed per-frame
trim/ramp arithmetic and an atomic target load to the existing mixer.

## Verification

Full validator: `python scripts/validate_m1.py` passes **111 tests**, doc-tests,
formatting, workspace/all-target checks, Clippy with warnings denied and the exact
dependency inventory. It includes the requested gates with locked/offline options:

```text
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The original offline renderer, M2 scheduling, routing, constructor/snapshot,
transition, tempo and arrangement regressions remain enabled. No assertion or
test was removed to obtain a pass. The existing text-focus test now supplies a
physical key release between presses and additionally asserts that leaving text
entry while holding a key cannot become a new performance gesture. Exact modifier
matching prevents Shift+V/F from also overwriting the persistent base mode.

New coverage includes ten synthetic MIDI integration tests and two additional
egui tests. Tests cover normalization/invalid messages, channel conflicts,
transactional learn, all sixteen slot mappings and held-note repeat suppression,
absolute pickup, mode/external-edit rearming, relative motion, source-tagged
momentary restoration, profile replacement/reset, actual queued/Playing/queued
stop/retrigger/cancel LED projection, unavailable/closed feedback, save/reload,
oversized/unknown configuration, bounded producer-thread ingress, ambiguous
device identity, controller floods, parameter fairness and independent Stop.
Egui tests cover held keyboard release/focus loss and contextual-control visibility
at 1100×740, in addition to the existing sixteen-slot viewport/text-entry checks.

`cargo run -p dawj --example synthetic_midi --locked --offline` passes through
the same bounded ingress and dispatcher without hardware. It renders actual
queued/Playing states, prints a queued value 16 then a changed Playing value 127,
demonstrates FILTER/held VOLUME/pickup, and reloads its written profile. This is
an in-process virtual source, not a claim that an OS virtual port was installed.

Windows native checks also passed:

- Silent window/settings launch, actual WinMM discovery and clean worker/window
  shutdown. Capture inspected. The environment exposed **no MIDI inputs** and
  **Microsoft GS Wavetable Synth** as its only output; neither was fabricated as
  a connected controller.
- Short real audio GUI run with MIDI discovery active: **Speakers (USB2.0
  Device)**, 48 kHz stereo f32/default buffer, 343 callbacks / 165216 source
  frames, max 1056 frames/callback, max render 1047.9 µs, zero reported callback
  budget exceedances, failed=false. Actual A1/B4 sources, A1 gain/cutoff,
  cancellation and crossfader checks passed; held master peaks were about
  **0.079088 / 0.063270**. Stream and worker closed cleanly. This is delivery
  evidence, not a listening, physical cue or sustained-dropout result.

The existing release realtime probe also passes with zero watched render
allocations/frees and failed=false for sixteen loops, sixteen rules, 32 incoming
commands per block and four output channels:

| Block frames | Mean µs | Max µs | Nominal budget µs |
| --- | ---: | ---: | ---: |
| 32 | 12 | 113 | 666 |
| 128 | 45 | 538 | 2666 |
| 512 | 167 | 1071 | 10666 |
| 2048 | 634 | 1657 | 42666 |

These offline probe timings were captured alongside software validation, not a
loaded physical MIDI/audio soak. Filtered stop/EOF first-step continuity fixtures
remain zero. Ignored evidence files:
`target/m3_3_4-validation.log`, `target/m1-validation.json`,
`target/m3_3_4-synthetic-midi.log`, `target/m3_3_4-realtime-probe.log`,
`target/m3_3_4-midi-settings.png`, `target/m3_3_4-midi-window.log`,
`target/m3_3_4-live.png`, `target/m3_3_4-live.log`.

## Limits and readiness for M3.5

The software milestone is complete through M3.4. Physical controller input,
LED interpretation, real unplug/reconnect, a Windows virtual-cable round trip,
Linux compilation/hardware, listening quality and a sustained UI/audio/MIDI soak
remain unverified because no MIDI input/controller was available. WinMM exposes
no virtual-port creation here; use separately supplied ports for manual tests.
Disconnect detection depends on backend port enumeration, and duplicate names
cannot be selected unambiguously. Shutdown/device-close latency remains subject
to the driver; no audio callback waits on it. A generic controller can have
binary-only LEDs or a different encoder convention, requiring profile changes.
Only 7-bit Note/CC, two's-complement relative encoding and the two engine-supported
control modes are implemented. No MIDI clock sync, SysEx/RGB protocol, script
engine, full skin engine, arrangement editor or new DSP feature was added.

M3.5 can build on the semantic/rendering boundary and the bounded coordinator
without replacing the engine. First complete the documented real-controller,
pickup/LED, disconnect, latency, physical cue and sustained-performance acceptance
checks on the intended interface. Release notice/source packaging and advisory
review remain separate gates. No M3.5 implementation, push or merge is performed.
