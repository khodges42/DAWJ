# Prototype Plan: Two-Deck Live Composition Instrument

**Status:** Proposed implementation roadmap  
**Source of truth:** `PRODUCT_AND_TECHNICAL_DESIGN.md` and `GPL_COMPATIBILITY_AND_RELEASE_POLICY.md`  
**Owner:** Founder / solo engineer, with Codex as implementation assistant  
**Approach:** Thin, musical, testable vertical slices. Each milestone should leave the application more playable.  
**License target:** GPL-3.0-or-later for original application code (final version selection pending).

## North-star goal

**Perform and record an enjoyable 15-minute set using two prepared compositions, eight playable parts per deck, live arrangement changes, headphone cue, and a DJ-style transition, without opening a DAW editor.**

Success is not measured by feature count. It is measured by uninterrupted audio, confidence in what's queued/playing, and whether performing feels better than managing a Session View template.

## Principles and boundaries

- One global musical clock, with deck-local phrase epochs.
- Performance interface is deliberately simpler than the preparation workflow.
- No filesystem access, UI rendering, blocking locks, unbounded work, or unpredictable allocation in the audio callback.
- No proprietary application dependency. Audit crates, native components, bundled samples and fonts before redistribution.
- Early prototype targets **one known development OS + one verified multichannel audio interface**. Cross-platform polish follows evidence of a playable instrument.
- Initially use short, pre-rendered loops at a declared tempo. Robust live time stretching, long-stem streaming, VST hosting, automatic mastering, generative MIDI, Link, and elaborate production editing are **not** prerequisites for proving the concept.

## Milestone roadmap

| Milestone | Goal | Build | Done when |
|---|---|---|---|
| **M0: Foundation** | A trustworthy project skeleton | Rust workspace, GPL/NOTICE policy, CI, dependency audits, transport types, offline renderer, deterministic timing tests | Fresh checkout builds/tests; beat-to-sample conversion and quantization pass across buffer sizes |
| **M1: Make sound** | Prove independent two-deck audio and cue | CPAL device enumeration, two stereo deck buses, crossfader, main/cue outputs, master peak protection, simple diagnostics | A plays on main while B is audible in headphones only; crossfading reveals B without phase reset |
| **M2: Make it musical** | Every launch behaves predictably | Eight fixed parts per deck, one-shot/loop player, next-beat/bar/phrase queueing, cancel/replace/stop states, smooth part gain/filter | 16 lanes can be launched/stopped; test impulses land at intended sample frames; UI/command state cannot contradict audio-engine acknowledgments |
| **M3: Make it playable** | Build the instrument interaction, not a dashboard | Two-deck persistent grid, focused detail view, mode-based VOLUME/FILTER/PATTERN affordances, keyboard bindings, one MIDI controller profile and LED feedback | Perform a five-minute arrangement and transition without opening configuration screens; queued and playing states are distinguishable at a glance |
| **M4: Make tracks portable** | Load a prepared composition without engineering intervention | Versioned ZIP+JSON package reader/validator, simple CLI or minimal prep exporter, asset hashes, decode on worker, safe idle-deck swaps | Create two different demo packs and load B while A plays without an audible glitch; malformed packs fail safely |
| **M5: Add composition** | Go beyond remixing stems | One simple subtractive synth, MIDI step/pattern playback, pattern variation changes, one periodic fill/one-shot rule | Change a bass pattern at the next phrase while drums continue; fill fires on the specified bar and cancels cleanly |
| **M6: Capture & protect** | Make rehearsals trustworthy | Stereo master PCM recording, writer thread and overflow telemetry, meters, headroom presets, disconnect/reconnect behavior, soak tests | Record 15+ minutes; no callback allocations/locks; no observed underruns during specified load/decode test; failure states are visible |
| **M7: Field alpha** | Validate product thesis | Four cleared demo packs, controller mapping quick-start, scripted 15-minute performance, comparison against an Ableton template | At least one uninterrupted 15-minute set with two deck transitions, cues, pattern/arrangement changes, and a usable recording; collect 3–6 performer sessions and identify top UX failures |

*Milestones are sequenced by dependency, not promised delivery dates. M1–M3 may use hard-coded demo assets; do not delay playable audio while building a full package editor.*

## First engineering backlog: small Codex-sized issues

1. Initialize Cargo workspace, root licensing files, formatting/lints/tests, and initial crate license audit.
2. Implement `BeatTicks`, tempo map, frame conversion and next-boundary quantization with deterministic tests.
3. Create an offline event scheduler that renders impulses to a buffer at exact sample offsets.
4. Build a CPAL output spike with device/channel selection, underrun/health counters, and silence-safe startup.
5. Add two independent stereo buses and main/headphone-cue routing; write an isolation test.
6. Add a crossfader with smooth gain transitions and a conservative output ceiling.
7. Implement a resident PCM loop/one-shot player; bound its voice/memory use.
8. Define launch/stop/replace/cancel commands, command IDs and part state transition tests.
9. Expand to 8 slots per deck; add per-part gain/filter smoothing and engine acknowledgments.
10. Add a simple performance UI with permanent deck/cue/mixer controls and unmistakable queued vs active indicators.
11. Add keyboard input and one controller profile; resync LED state after reconnect.
12. Implement minimal package import/export with validation and async idle-deck loading.
13. Add one synth + MIDI variation swapping at phrase boundaries and one bar-counted fill rule.
14. Add a nonblocking audio recorder, diagnostics, and long-run test harness.
15. Rehearse, record, identify failure points, then prioritize only improvements that unblock the live set.

**Codex workflow:** One issue per task/branch. Require a runnable demo or test fixture, `cargo fmt`, `cargo clippy`, tests, a short architecture note for any boundary changes, and explicit evidence for real-time safety claims. Run architecture reviews at M0, M2 and M4 rather than after every minor change.

## Musical acceptance script (alpha)

1. Load prepared Track A; start its kick, percussion, bass and atmosphere in a controlled order.
2. Queue and cancel a part; verify the cancellation happens before the requested boundary.
3. Open a filter and change arrangement density; no clicks or surprise level jumps.
4. Load Track B while A plays. Cue B privately and assemble B's first phrase.
5. Bring B into the main mix with the crossfader; B maintains its cue-phase position.
6. Swap a MIDI bass pattern and trigger a recurring or one-time fill at the intended bar.
7. Perform a second transition or return to A; keep the set going for 15 minutes.
8. Stop recording and play back the resulting stereo file; capture warnings and subjective feedback.

## Product gates and measures

- **Timing:** offline event boundaries accurate within one output sample at tested sample rates/buffer sizes.
- **Cue isolation:** B contributes no signal to application's main mix while fully cued and crossfader closed; distinguish digital isolation from physical interface bleed.
- **Safe loading:** 100 idle-deck pack loads during playback with zero measured callback gaps on the recorded reference machine, or document the exact failure condition.
- **Operational clarity:** performers can distinguish *off / queued / playing / stopping* without opening menus.
- **Audio safety:** no NaN/inf output; conservative headroom and visible limiting diagnostics; check difficult bass combinations by ear.
- **Musical quality:** demonstrate at least two convincing prepared compositions, not just metronomes and sine waves.
- **Performance test:** record the full 15-minute workflow and compare it against an equivalent Ableton Live setup for cognitive effort and number of gestures.

## Stop / change direction rules

- If dual-bus audio or safe concurrent loading proves unstable, fix that before building new performance features.
- If users can't reliably tell queued from playing, redesign the visual/hardware feedback before expanding effects.
- If authoring a useful pack takes longer than making the music itself, build a faster stem/MIDI import workflow, not a bigger prep editor.
- If experienced DJs consistently perform the same tasks more fluidly in Ableton with a good template, investigate the interaction model rather than adding features to compensate.

## Explicitly postponed

VST hosting; automatic key matching; pitch-perfect real-time stretch at extreme BPM ratios; full DAW-like piano roll; elaborate generative algorithms; live collaboration; cloud library; arbitrary effect graph editing; mobile app; custom manufactured hardware.

## Next action

Give Codex **Issue 1** plus the two architectural source documents. Once the workspace is healthy, tackle **Issue 2 and Issue 3 before GUI work**. The earliest satisfying hands-on checkpoint is the end of **M1**: two audible decks, a working crossfader, and private headphone cue.
