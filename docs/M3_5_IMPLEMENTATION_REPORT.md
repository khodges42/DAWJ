# M3.5 implementation and verification

M3.5 adds a native repeating phrase visualization and direct performance feedback
to the existing DAWJ GUI/MIDI application. The baseline M3.4 validation passed
111 tests before modification. README, the M3 architecture/implementation notes,
M2.9 state contract, live scheduler, rules, snapshot publisher and GUI/coordinator
were inspected. Work stops at M3.5; no later milestone, push or merge is included.

## Delivered

- Default eight-bar ruler with global bar/beat/BPM, global phrase reference,
  independent deck phase, A/B activity lanes and selected-part cycle inspection.
- Explicit launch/stop/retrigger/replacement/fill markers, grouped mixed-action
  labels, next rule and conditional recurrence projections, and retained last-fired
  feedback. Source identity and deadlines remain authoritative engine observations.
- Marker/card selection without hiding either deck or changing VOLUME/FILTER/
  held modes. Existing MIDI pickup and semantic dispatch remain intact.
- One/two/four/eight-bar zoom, frozen phrase paging and Follow. Four/eight-bar
  engine configuration is available before Start; the engine has no runtime
  phrase-length editor, so the UI does not invent one.
- Direct per-deck fill/cancel, selected-part Once/Every-N/PhraseBar arming,
  pause/resume/removal and manual trigger/cancellation. All commands reach the
  existing scheduler through the single coordinator. No fake local acknowledgment
  changes a Playing state or installed rule.
- Compact fixed 2×4 cards per deck retain all sixteen launch/stop/cancel controls.
  Essential live controls remain visible at 1280×820 and 1100×740 logical viewports.
  Detailed interaction and cancellation semantics are in the
  [phrase guide](M3_5_PHRASE_VISUALIZATION.md).

## Architecture and realtime contract

`performance-app/src/timeline.rs` is a pure tick/snapshot projection with bounded
recurrence expansion (32 per rule). `window/phrase.rs` paints and dispatches;
there is no GUI musical clock, seeking, arrangement editing or note-pattern
inference. A future recurrence is explicitly a conditional projection from the
published next deadline/period, since manual actions can skip it. Nominal musical
cycle is not source EOF. No new dependency, license exception, DSP or scheduler
timing policy was introduced; the reviewed Windows inventory remains 185 packages.

The old diagnostic rule atomics were insufficient for this view. The coherent
performance publication was extended explicitly: deck epoch/phrase length;
active/pending one-shot identity flags and nominal length; installed rule definition,
PartId, one-shot flag, period, next target, consumed flag, fired/skipped counts and
last successful fired tick. Fixed payload grows from 167 to **299 atomic words**,
still with one SC generation, no writer retries, reader-dependent waits, locks or
allocation. The updated [state contract](PERFORMANCE_STATE_CONTRACT.md) documents
the layout and semantics. Existing diagnostic reads are not mixed into this view.
The demo exposes existing constructor phrase configuration; legacy prepare() and
offline demo behavior still default to eight bars.

## Verification

The requested formatting, workspace check, Clippy with warnings denied and all
workspace tests pass through `python scripts/validate_m1.py`, including additional
locked/offline/all-target and doc-test checks. **119 tests pass** with the exact
185-package dependency inventory. Existing offline rendering, schedule, mixer,
MIDI and realtime-hardening regressions remain enabled.

Eight new tests (seven integration, one egui interaction) cover:

- An eight-bar action shown at the current window's right boundary, then the next
  window's start, while remaining queued until its exact sample is rendered.
- Tempo-change boundary and unchanged musical pending/rule targets; actual rule
  firing at the new sample intervals.
- Manual fill cancellation, Once cancellation/consumption, periodic skip,
  pause/resume, manual-stop disabling and removal without stopping an active fill.
- Four-bar constructor validation, zoom/follow/frozen paging without scheduling.
- Rule codec variants, bounded projection after long sessions and simultaneous
  coherent clock/deadline/counter reads across all sixteen rules.
- Mixed marker labels preserving action types and projection indication.
- A real pointer click selecting a timeline fill marker while retaining FILTER,
  frame position, pending identity and both visible decks.

Existing headless viewport and contextual mode tests also run with the timeline
and compact cards. A test timing fixture was corrected to advance beyond the
frozen page before asserting that Follow moves it; no engine expectation or
existing assertion was removed to obtain a pass.

The Windows native/audio smoke check returned exit 0 with passed=true. It opened
Speakers (USB2.0 Device), 48 kHz stereo f32/default buffer; launched A1/B4, exercised
gain/filter/cancellation/crossfader, then armed an A8 periodic fill and queued a
launch, stop, retrigger and one-shot fill for visualization. The engine snapshots
confirmed the rule and distinct pending kinds while active sources kept playing.
The capture was inspected and grouped-action readability was improved afterward.
The short stream reported 371 callbacks / 178656 rendered frames, maximum 1056
frames/callback, maximum render 963.2 µs, zero nominal budget exceedances and
failed=false. Master held peaks were about 0.07909 / 0.06327. Stream/window/MIDI
worker shutdown completed. This is delivery evidence, not a listening, physical
cue or sustained-dropout claim.

The existing sixteen-part/four-channel release probe passes with zero watched
render allocations/frees, failed=false and unchanged zero first-step filtered
EOF/stop fixtures:

| Frames/block | Mean µs | Max µs | Nominal budget µs |
| --- | ---: | ---: | ---: |
| 32 | 11 | 137 | 666 |
| 128 | 38 | 160 | 2666 |
| 512 | 145 | 299 | 10666 |
| 2048 | 554 | 1037 | 42666 |

Ignored evidence: `target/m3_5-baseline-validation.log`,
`target/m3_5-validation.log`, `target/m1-validation.json`,
`target/m3_5-realtime-probe.log`, `target/m3_5-live.png`, `target/m3_5-live.log`.

## Limits

Live phrase-length edits are unavailable; Stop/Start creates a new engine and
clock. Global ruler reference uses deck A's configured phrase length anchored at
zero; deck phases are displayed from each epoch. Only the last successful firing
per installed rule is retained, not durable event history, all notes, waveforms
or future audio. Rule projections remain conditional. Native sample duration and
tempo remain separate domains. Dense events group at a shared boundary; hover or
the context menu lists members individually. UI Every-N is limited to 1–32 bars;
PhraseBar follows the current four/eight-bar engine.

No physical MIDI input was available (WinMM exposed only the Microsoft synth
output). Physical controller/LED operation, unplug/recovery, Linux builds,
audition, physical cue isolation, input-to-output latency and a sustained loaded
performance remain manual acceptance gates. No hardware result is inferred from
unit tests. This remains a performance timeline, with no arrangement editor,
pattern authoring, new DSP or post-M3.5 functionality.
