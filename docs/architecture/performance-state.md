# Performance state contract (M2.9)

Use `LiveControl::performance_state() -> Option<PerformanceState>` for GUI and
MIDI feedback. Types live in `audio_backend::performance_state`. This is a
read-only observation of actual engine state, not a desired-state model. Keep
submitting commands through the existing single-producer `LiveControl` and
continuous deck controls through `MixerControl`. A non-realtime coordinator can
own both handles; UI and MIDI workers must not mutate DSP/scheduler objects.

## Snapshot and clock semantics

A coherent initial snapshot is published at construction. Thereafter publication
occurs at the end of every nonempty render block. Empty buffers do not admit
commands, move the clock or change the generation. `frame` is the next unrendered
frame, `beat` its nearest 960-PPQ position, and `sample_rate` is explicit. Neither
is a DAC timestamp. Global clock runs continuously even with all slots stopped.
Deck phase is independent of Part nominal length; each active Part phase is
relative to its last start/retrigger. Tempo edits affect musical scheduling, not
native-rate sample duration or time stretching.

`TransportState::Ready` means no frames rendered; `Rendering` means the rendered
transport has advanced (not necessarily audible, or a promise the OS is presently
calling back); `Failed` records an engine scheduling failure. `Closed` means
terminal command admission, overlaid from the monotonic lifecycle flag. The
musical fields in Closed are the last coherent audio publication; an in-flight
callback can finish afterward. `failed` records an engine failure, while device
failure details still require `PlaybackMonitor`. Terminal stop/restart uses a
new stream/engine with a new clock and generation domain; no pause/resume API is
introduced. Closing a ready stream also reports Closed.

M3 adds `bpm: Option<f64>` to the same coherent publication. This is the tempo
at the next unrendered frame, resolved from the scheduler map at its rounded
sample boundary. A tempo ticket reporting Applied means future map intent was
installed; it does not imply that the displayed current BPM has changed. None
means the scheduler cannot provide a valid tempo, and must display unavailable.

`generation` changes by two per audio publication. The reader performs one
bounded attempt and returns None if publication overlaps it. Retain the previous
snapshot and try on the next UI update; do not spin on the audio thread. Compare
both generation and transport because a terminal close can happen without any
further audio publication. Generations are engine-local, wrapping u64 counters
(wrap is not a practical session horizon). Do not compare generations between
newly created engines. Snapshot copies do not retain PCM ownership.

The Jam2 publication has 413 atomic words and one atomic generation (M3.5 used
299 words; M2.9 used
166 words; the M3 current-tempo extension used 167). All are
sequentially consistent: the writer marks odd, stores the words, then marks even;
the reader accepts only an unchanged even generation. There are no non-atomic
concurrent payload accesses, locks, allocation, retry loops or reader-dependent
writer waits. Audio writes a fixed number of words irrespective of readers.
The Windows x64 target provides native 64-bit atomics. The old `slot()` meters
remain independently observed diagnostics, and must not be combined to reconstruct
authoritative state.

## Part semantics

Each deck has exactly `parts::SLOTS` (eight) fixed addresses. `DeckState.phase`
and all its PartState records belong to the same audio publication. Deck activity
is derivable from active/pending slots; selecting a deck in the UI is control
focus, not an engine playback state.

| Lifecycle | Active source | Pending intention |
| --- | --- | --- |
| Stopped | None | None; a short output release may remain |
| QueuedStart | None | Launch identity, musical target and command ID |
| Playing | Present | None |
| QueuedStop | Present | Stop at target |
| QueuedReplace | Present | Replace or retrigger at target |

`active` is actual PartId and `position` is consumed source frames; both are
None without an active source. These are independent of gain/mute, zero-valued
PCM, crossfader, cue selection and measured energy. A muted/zero-valued source
remains Playing and advances. A nominal musical cycle does not imply sample EOF.

`pending` includes a typed kind, target PartId, resolved BeatTicks boundary and
command ID. Launch/stop/retrigger/replacement apply before the target sample.
Immediate means next admitted safe frame; its beat is storage-tick precision,
not an OS input timestamp. QueuedReplace with the same complete Part definition
is reported as Retrigger (equivalent cursor reset); differing definitions report
Replace, even if they share a PartId. Retriggering is an action at a precise frame,
not a separately prolonged lifecycle. After applying it, position resets and
lifecycle is Playing. Recurring rules dispatch immediately when due and therefore
do not occupy the manual pending record; use existing rule diagnostics for their
enable/fired/skipped status.

M2.1 precedence remains: latest valid pending command wins; invalid commands
leave prior intent untouched. Cancel matches the pending ID and preserves current
playback. Manual Stop disables recurrence; cancellation does not rearm it.
EOF clears an active source but retains a future replacement as QueuedStart.
Tickets are bounded acknowledgments and can expire; the snapshot remains the
source of current truth. Neither snapshots nor tickets are an archival event log.

`parameters` are accepted gain/mute/cutoff targets, not the instantaneous ramp
coefficient. `release_frames` is remaining post-filter transition work, including
active retrigger/replacement crossfades. Zero means no transition. It may be
nonzero while Stopped: the source and filter have stopped, while a held last
output decays for at most `max(sample_rate/500,1)` frames. This avoids confusing
short de-click output with an active Part.

## Boundaries and UI usage

After stop/EOF the engine does no further source or filter processing. Its held
stereo output fades to zero over 2 ms. At retrigger/replacement it blends that
held output into the new source over the same fixed interval. One transition
record per slot bounds overlap; another action replaces the transition from its
current output, without creating more voices. Initial starts from silence preserve
prepared onsets. This changes audible tails, not event frame/cursor timing.

On each UI/LED tick, attempt one read, retain successful snapshots, diff against
the previous generation, and render active plus pending identity separately.
Display QueuedStart as queued rather than Playing; distinguish a pending stop or
replacement from its currently active Part. Reconcile local desired parameters
with accepted snapshot parameters after rejection. Use queue admission errors to
show immediate rejection. Do not infer successful launch from ticket submission
or silence from lifecycle alone. Keep meter polling independent of operational
feedback. Deck gain/crossfader/cue targets remain with the control coordinator;
the audio snapshot supplies per-part accepted controls, not an acknowledgment of
the separate latest-value MixerControl path.

During terminal shutdown disable controls when Closed, preserve the last musical
display as stale, and obtain device errors from PlaybackMonitor. Physical cue
availability still comes from validated OutputRouting/device configuration;
no state value establishes private physical monitoring.

## M3.5 phrase and rule observation

Each coherent `DeckState` now includes its scheduler `epoch` and `phrase_length`,
alongside its phase. These are constructor-defined musical ticks, not GUI
settings. The demo can construct four- or eight-bar engines; phrase length cannot
be changed on a running engine. Zoom/page/follow are entirely view state.

Each PartState includes `active_one_shot`, `pending_one_shot`, and the active
Part's optional `nominal_length`. These identify actual source/timing definitions;
the nominal length/phase does not predict sample EOF or imply time stretching.
A pending stop's one-shot flag describes its active source. Releases remain
independent of source lifecycle as before.

`PartState.rule: Option<RuleStatus>` publishes the installed rule atomically with
the clock and slot. It contains the exact rule definition, PartId, one-shot flag,
optional musical period, next deadline, consumed flag, fired/skipped counters and
optional `last_fired` musical target. None means no installed rule. A rule with
next=None is paused/disabled or consumed; `consumed` distinguishes spent Once
rules. `last_fired` records a successful rule dispatch, not command installation
or a skipped trigger. It is one retained event, not durable history. Cancelling
the installed rule removes its observation; it does not stop an active source.

The timeline uses the published next deadline and period to project conditional
later recurrences. Manual conflicts can skip them. Projected repetitions are
not additional queued scheduler actions and must be marked as projections. A
queued action at the next phrase boundary remains queued until its sample is
rendered, even if the tick-rounded clock/playhead has reached that boundary.
No event scheduling, extrapolated musical clock or historical performance trace
is reconstructed by GUI frame time. Existing diagnostic `slot()` fields remain
separate and are not combined into timeline state.

## Pattern and audition observation

Each slot additionally publishes current pattern revision, pending revision and
boundary, bar count, current step, running flag, trigger count and last-trigger
tick. Launching a pattern preserves its logical active identity across individual
one-shot EOFs; Stop ends that pattern clock. `position` remains the current sample
cursor and can be absent during a pattern rest. `active_one_shot` is false for
an active pattern even though its hits use one-shot samples. See the engine
contract for global-bar edit activation. The application target revision is
accepted intent, not an executed revision; Tauri labels it separately.

The same coherent header carries optional audition PartId and cursor. Audition
is a separate prepared source mixed into the shared master. It does not change
fixed deck identities or establish private cue availability. Deck meter atomics
are independent diagnostics and never reconstruct operational lifecycle.
