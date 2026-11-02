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

The global frame clock advances while streaming even with all parts stopped.
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

Stream/engine failure and terminal stop close command admission. Unresolved
tickets become Closed even without another callback. Restart constructs a fresh
engine/clock and stopped parts; no pause/resume transport is implied. A stale
publication cannot be made fresh just by rereading the same generation.

Run workspace regressions before changing these guarantees. Existing offline
tests cover boundary inclusion, late admission, tempo retiming, cancellation,
rule precedence, bounded transitions, smoothing and concurrent publication.

User content uses the same engine with an initially empty resident bank. Its
64 cells support producer-only immutable publication; current and retired voices
share a 256 MiB PCM cap until output closes. See the [ownership contract](overview.md#local-content-and-resident-publication).
Loading/replacing content requires the affected deck(s) stopped with no pending
actions or rules. Another deck can continue on the same sample-frame clock.
