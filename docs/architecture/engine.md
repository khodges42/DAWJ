# Audio and musical scheduling contracts

These contracts describe current behavior, including corrections established by
the architecture reviews. They are not a new engine design.

## Callback constraints

No GUI, filesystem, network, decoding, blocking locks, heap allocation or
unbounded work belongs in the realtime callback. Media and immutable voices are
prepared off-thread before publication; another deck can continue streaming. Rendering uses fixed slot counts and bounded buffers;
command admission and diagnostic history are bounded. Empty buffers do not
advance musical time or admit commands.

`LiveControl` is a single producer for a 32-entry SPSC command ring. The consumer
takes an entry snapshot per nonempty callback. Commands arriving afterward wait
until another entry; a producer cannot schedule retroactively inside rendered
audio. Mixer targets use the existing atomic/smoothed parameter path.

The engine starts from a pristine scheduler at frame zero, with no active or
pending parts/rules. A partially advanced scheduler cannot be handed to a fresh
audio engine as if its cursors already existed. There is no invented silent
continuation when a resident source or setup is unavailable.

## Clock and boundaries

The global frame clock advances while musical transport is playing, even with
all parts stopped. Musical Pause freezes it while device callbacks continue.
Time uses 960 ticks per beat, fixed 4/4, absolute nearest-frame rounding and deck
phrase epochs. Nominal part length is metadata; it does not determine sample
loop duration, phrase wrap, time stretching or the global tempo.

| Quantization | Engine eligibility |
| --- | --- |
| Immediate | First renderable frame after callback-entry admission |
| Beat | Next unrendered global multiple of 960 ticks |
| Bar | Next unrendered global multiple of 3840 ticks |
| Phrase | Next unrendered phrase multiple relative to the deck epoch |

An exact boundary is inclusive only while its rounded sample frame remains
unrendered. A delayed GUI/MIDI/control-worker request can miss that opportunity
and wait a whole further beat/bar/phrase. Quantization is not a latency
compensator. Tempo changes preserve musical deadlines rather than freezing them
to old frame positions. Live tempo history is retired within fixed capacity;
excess simultaneous future intent rejects transactionally.

## Lifecycle and rules

Each deck has eight slots and one active/pending part identity per slot. Launch,
stop, retrigger, replacement and cancellation use engine command identities and
outcomes. A cancellation after execution is stale; a frontend must report that
rather than manufacture a successful cancel. Queued state is distinct from Playing.

Rules support Once, every N beats/bars and phrase-bar occurrences with global or
deck phase. Manual actions precede automatic rules. A pending manual action
reserves its slot; conflicting occurrences skip instead of catching up later.
Stop disables its rule immediately; cancelling that stop does not re-enable the
rule. Explicit enable/installation is required. A spent Once rule cannot simply
be enabled again: installation intentionally rearms it. Removing a rule does not
stop an already playing source. One-shot playback naturally stops at EOF.

## Samples and DSP

The resident WAV loader supports mono/stereo PCM 16/24/32 and IEEE float32 at the
configured rate. Invalid/truncated media, non-finite or out-of-range samples and
unsupported encodings reject during preparation. There is no streaming importer,
time stretching in the performance demo. Local Jam1 WAV preparation can convert
rates off-thread before resident publication; it does not resample in the callback.

Part gain and low-pass cutoff use existing engine smoothing. Resonance/EQ/synth
controls are not implemented. Bypass, retrigger, stop and EOF preserve bounded
transition behavior; interruption de-clicking uses a bounded 2 ms held-output
transition rather than a new voice. Short samples must not disappear because
automatic start/end fades overlap: this is guarded by regression tests.

Deck/master normalization and the existing 0.125 logical output ceiling are
conservative engine conventions. The supplied demo applies +36 dB makeup before
the limiters to compensate its sample/mixing attenuation. This is not a hardware
volume guarantee or automatic mastering. Output trim can lower the level.

## Monitoring, publication and failure

Stereo output is master-only. Independent logical cue requires four channels
with separate master/cue routing; physical headphone isolation needs a listening
and routing test. Cue is not simulated as an isolated bus on a shared stereo pair.

The [performance-state contract](performance-state.md) describes coherent
publications, lifecycle and parameter meanings. Independent diagnostic atomics
must not be combined into a fabricated coherent state. Held peaks are not RMS
meters, a historical waveform, or proof of physical output.

Stream/engine failure and terminal device close stop command admission. Unresolved
tickets become Closed even without another callback. Restart constructs a fresh
engine/clock and stopped parts. Musical Play/Pause/Stop are separate from this
device lifecycle. A stale
publication cannot be made fresh just by rereading the same generation.

Run workspace regressions before changing these guarantees. Existing offline
tests cover boundary inclusion, late admission, tempo retiming, cancellation,
rule precedence, bounded transitions, smoothing and concurrent publication.

User content uses the same engine with an initially empty resident bank. Its
64 cells support producer-only immutable publication; current and retired voices
share a 256 MiB PCM cap until output closes. See the [ownership contract](overview.md#local-content-and-resident-publication).
Loading/replacing content requires each changed slot stopped with no pending
action, pattern receipt or rule. Unchanged slots, including on the same deck,
can continue. Both preflight and commit check slot quiescence. Composition
replacement changes all its slots and therefore requires the destination deck
stopped. Prepared voices remain immutable; no active source is hot-swapped.

## Musical transport

Play, Pause, global Stop, deck Stop and addressed Halt use the same bounded
callback command ring. Pause freezes the scheduler, pattern phase, sample cursors
and queued musical deadlines. Device callbacks, control admission, coherent
publication and mixer smoothing/diagnostic decay continue. Part filter/gain state
and voice elapsed frames hold. Native-rate audition also pauses. Bounded 2 ms
master/cue fades handle pause/resume without cursor resets or allocation.

Global Stop clears active/pending slots and installed rules on both decks,
discards source cursors and freezes the clock at its current position. Accepted
pattern replacements become stopped definitions; source data and tempo intent
remain loaded. Play resumes the clock without relaunching material. Deck Stop
does the same clearing for one deck without pausing the shared clock. Deck Play
submits quantized launches for its loaded stopped slots. Shared core routing
resumes paused transport for manual launches, including keyboard and MIDI.
The last admitted transport intent composes commands before the next publication;
it never substitutes for authoritative published playback state. Independent
deck pause and seeking are absent.

Addressed Halt provides immediate stop/cancel/rule-clear for the explicit
**Stop / clear destination and assign** operation. It is not a source swap;
ordinary preparation and both quiescence checks still follow it.

## Step patterns and live editing

Each fixed slot can hold a bounded 1/2/4-bar pattern (four fixed arrays of sixteen
velocities, 0 = rest, 1-127 = hit). The Rust frame clock dispatches sixteenth-note
sample retriggers at 240 ticks; callback partitioning does not change event frames
or rendered audio. Launch anchors pattern phase; one-shot sample EOF does not stop
the pattern clock. Manual Stop does. Patterns and periodic rules are exclusive.

A running pattern edit activates at the next global bar and resets the anchor;
a stopped edit activates at callback admission, including while transport is
paused; it executes no trigger. The publication reports
current revision, pending revision/boundary, running step and trigger count. The
application reports accepted target revision separately. Recall of a stored
variation submits through this same bounded path. Disabling a running pattern
requires stopping its slot. All file inspection, indexing, decoding and audition
preparation run outside the callback. Preview playback is an explicitly shared
master audition, never an isolated headphone substitute.

Worker-side WAV preparation accepts complete terminal DATA chunks with an omitted
alignment byte or a RIFF length overstated by exactly one byte; payload bounds and
frame alignment remain strict. Finite floating-point overs are attenuated uniformly
to unit peak before resampling; `MediaInfo.preparation_gain_db` reports the applied
nonpositive gain. Strict `LoadedSample::decode` still rejects out-of-range PCM.
Nonfinite values and truncated audio are rejected. No source file is rewritten.
