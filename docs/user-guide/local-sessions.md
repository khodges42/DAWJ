# Your own two-deck jam

DAWJ opens with an empty local session in Tauri. No bundled composition is
selected. Open **Session** to create, load and save your own material.
The egui fallback retains its supplied performance demo; authoring is in Tauri.

## Create and perform

1. Press **Sketch** on Deck A. Enter a composition name and press **New**.
2. Select a fixed part slot. Choose a pack/category, search, **Listen**, then
   **Assign**. Use **External WAV...** for your own file. Read the source details;
   filename pitch/key/BPM are optional hints, not audio analysis.
3. Turn on **Pattern** for a one-shot sound, then press steps to add hits. Select
   a step and set velocity (1-127; zero is rest). Velocity 115+ shows an accent.
   Choose 1, 2 or 4 bars, then Page to edit each bar. Rotate, Clear and Duplicate
   operate on the actual pattern. A looping sample must have Loop turned off
   before using a pattern; use the Sample region section to change it.
4. Repeat on Deck B with a distinct composition. Parts keep their eight stable
   addresses on each deck. Select names/colors and assign more sounds as needed.
5. Open **Audio output**, choose your intended output, routing and phrase length,
   then **Start output**. Audition explicitly plays through the shared master,
   including on stereo hardware; stop it with **Stop listen** in Sketch.
6. Select **Performance** and launch parts. Mix A/B from the central strip, shape
   the focused part with Gain/Filter, or use Extended for every part's contextual
   control and scheduling details. Switching workspaces or Sketch keeps audio running.
7. **File > Save As...** chooses a `.dawj.json` file; **Ctrl+S** saves subsequent
   changes. Close DAWJ, reopen it and choose **File > Open...** to restore the session.
   Playback never auto-resumes. Select and start your intended output again.

**New empty**, **Supplied demo** and Open require output closed. Stop individual
parts for musical stops; **Close output** shuts down the audio stream and clock.
Unsaved changes appear beside the session name and in Session/Sketch. New/Open/Demo
ask before discarding them. Save before closing; there is no autosave/crash recovery.

## Factory packs and patterns

Every immediate child folder in `resources/samples/` is discovered generically.
Nested folders become categories. An optional `pack.json` supplies stable ID,
display name, license and provenance; missing release metadata is reported.
The checkout's user-supplied WAVs may be local and Git-ignored: a fresh clone can
always import external WAVs or add its own packs. Do not redistribute samples
without checking their actual license. No pack name is required by the workflow.

The browser filters pack, category, name, type, pitch/key and source BPM range.
Metadata corrections are saved in the session; they do not rename source files.
Unsupported/broken sounds report errors. Source BPM never changes playback speed.
Sample regions use prepared 48 kHz frames. **Listen step** auditions its velocity.

The Rust engine schedules sixteenth-note hits (240 ticks at 960 PPQ). Pattern
steps loop over 1/2/4 bars and follow musical tempo. Editing a running pattern
activates at the **next global bar**, resets its pattern phase at that boundary,
and shows a separate pending-edit message until the engine confirms execution.
Stopped-pattern edits activate at the next admitted frame. New edits replace the
pending edit for that part. Stop the part before disabling its pattern. Patterns
and periodic rules cannot occupy the same slot simultaneously.

Select **Variation 1-4** and **Store pattern** to remember the current grid. Recall
uses the same safe activation boundary as a pattern edit. An edited grid is
labelled separately from a stored variation; pending targets are not confirmed
playing state. Variations retain pattern data, not another sound or part identity.

**Edit > Undo/Redo** and Sketch's buttons cover composition/content edits, sound
assignment, grids, metadata and variation storage/recall (32 history entries).
Live gain, filter, fader, transport and MIDI gestures are not content undo steps;
Save captures their current accepted targets. Redo clears after another content edit.

## Audio and playback controls

Supported files are RIFF WAV, mono or stereo, PCM 16/24/32-bit or float32.
Original rates from 1 kHz through 384 kHz are accepted within decoded limits.
Non-48 kHz audio is converted offline using linear interpolation; the realtime
engine always receives resident 48 kHz stereo PCM. This simple conversion has
no mastering-quality anti-aliasing guarantee; prepare important material at
48 kHz externally when that matters. There is no time stretching, beat detection
or automatic length matching. MP3 is unsupported and reports an import error.

Loop start/end are frames of the **prepared 48 kHz** sound; blank end means EOF.
Musical length uses quarter-note beats (960 ticks per beat) and changes the
engine-reported part phase, without changing PCM speed. The existing
`TimingPolicy.quantum` is preserved as validated session metadata; the scheduler
does not use it to select live boundaries, so no authoring control advertises it.
Global Beat/Bar/Phrase quantization still governs performance commands. Gain and
cutoff are operated on the performance surface and saved from accepted targets.
A part in slot 8 can be configured one-shot for the existing Fill buttons.

Preparation runs in a background job. Busy indicates work in progress, not
successful assignment or Playing. Decode failure leaves the previous assignment
intact. Re-importing the same path actually reloads the file. Remove clears the
sound and its saved periodic definition. Names and knob edits do not duplicate PCM.

## Replacing one deck

The destination must have **no playing parts, pending actions or installed
periodic rules**. Stop its parts, cancel pending actions, and remove its rules.
Stopping output globally is unnecessary: the other deck can keep playing.
The condition is checked again when preparation completes; if the destination
became active, the edit is rejected and its previous content remains.

Select another existing composition and **Load selected**, or create one.
Selecting `(empty deck)` unloads that destination after the same safety checks.
Both decks may reference the same composition; editing its content requires
both stopped. Session performance gain/filter targets remain independent per deck.
New/Open replaces the complete document and therefore requires output closed.

## What is saved

Version 2 JSON (with older local documents accepted) stores session/composition/part identities and names, up to eight
compositions with eight slots each, deck assignments, local media references,
sample regions, loop bounds, names/colors, factory-relative references, metadata
corrections, full velocity patterns, stored variations, nominal length, quantum,
gain, cutoff and periodic definitions.
It also stores BPM, 4/8-bar phrase length, deck gains, crossfader, output trim,
cue selections, quantization and persistent VOLUME/FILTER mode. Per-deck targets
retain independent shaping when both decks reference one composition.

Saved periodic definitions survive live rule cancellation, but are **not armed
automatically**: select their
part in the editor and use **Arm saved rule** after output starts. Arming is an
existing semantic engine action and can start musical activity. Paused/spent
execution history, playback cursors, pending launches/stops, held modes and meter
values are not resumed. The audio device must be selected each run. MIDI profiles
and skins retain their existing persistence. Workspace choice extends the existing
interface preferences and survives restart independently of the opened session.

Media remain at their local paths; they are not copied or embedded. Paths beneath
the session directory save relatively, other paths absolutely. Save As rebases
references while keeping the same source files. Missing or invalid files leave
only those slots unavailable; replace their paths with **Assign / replace WAV...**.
Open and Start recheck the referenced files. A session is not a portable library
or the exploratory composition ZIP format.

## Limits and remaining work

JSON is limited to 512 KiB, a WAV file to 64 MiB, prepared PCM per sound to 128 MiB,
and installed PCM to 256 MiB total. At most 64 prepared voice versions can reside
in one running engine. Replaced versions stay immutable until output closes;
exhaustion rejects the edit with instructions to stop/restart output and retry.
Only changed voice definitions consume versions; name, gain and filter changes
do not. At most one media job runs; performance commands continue independently.

The engine sequences sample-trigger patterns; pitched note synthesis is not
implemented. Pattern creation, velocity edits and variations are persisted with
the composition. Unsupported JSON fields and versions are rejected,
not silently discarded. Streaming long tracks, library/catalog infrastructure,
portable media packages and proprietary import are outside this workflow.

## Reproduce the end-to-end check

```powershell
cargo build -p dawj -p dawj-egui --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/jam1_smoke.ps1
```

The script creates two distinct PCM WAV files under a fresh `target/jam1-*`,
opens actual Tauri/CPAL, requires a unique HyperX output, authors two compositions,
launches/stops independently, shapes/mixes, edits stopped A while B continues,
saves, closes the process and reopens the same document in a second process.
It verifies restored identities, media, playback configuration and executed gain/
filter targets before new gestures. Logs/results stay under target. It does not
establish physical headphone isolation, physical MIDI operation or human usability.
