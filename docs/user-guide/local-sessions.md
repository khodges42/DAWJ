# Your own two-deck jam

DAWJ opens with an empty local session in Tauri. No bundled composition is
selected. Open **Session / sounds** to create, load and save your own material.
The egui fallback retains its supplied performance demo; authoring is in Tauri.

## Create and perform

1. In **Session / sounds**, select Deck A, enter a composition name, and press
   **Create & load**. Choose a slot (1-8).
2. Press **Assign / replace WAV...** and select your sound. Alternatively, drop
   one WAV on a part card. Drops elsewhere use the editor's selected deck/slot;
   when the editor is closed they use the performance selection. Multiple files
   in one drop are rejected. Picker cancellation leaves content untouched.
3. Check the filename, duration, original sample rate, channels and prepared
   frame count. Set the part name and choose **Loop** or one-shot playback.
   Press **Apply part settings** after changing playback fields.
4. Repeat for Deck B with a second composition. Each composition has eight
   stable slots; an empty slot cannot launch. Select **Return to performance**.
5. Choose your intended audio output, routing and phrase length, then
   **Start output**. Launch parts, change gain/filter, queue/cancel actions,
   operate the crossfader and stop each part using the existing surface,
   keyboard or MIDI controls. The engine determines all musical boundaries.
6. Use **Save As...** to choose a `.dawj.json` session file, then **Save** for
   subsequent updates. Rename the session or composition in the same panel.
7. After closing DAWJ, choose **Open...** in a fresh application. Inspect any
   missing-media messages before starting output. Playback never auto-resumes.

**New empty** and **Supplied demo** require output stopped, as does Open.
The optional demo remains useful for the [performance walkthrough](performance.md).
Unsaved changes are marked with `*`; New/Open/Demo asks before discarding them.
Save explicitly before closing the application; there is no autosave or crash
recovery in this version.

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

Version 1 JSON stores session/composition/part identities and names, up to eight
compositions with eight slots each, deck assignments, local media references,
loop bounds, nominal length, quantum, gain, cutoff and periodic definitions.
It also stores BPM, 4/8-bar phrase length, deck gains, crossfader, output trim,
cue selections, quantization and persistent VOLUME/FILTER mode. Per-deck targets
retain independent shaping when both decks reference one composition.

Saved periodic definitions survive live rule cancellation, but are **not armed
automatically**: select their
part in the editor and use **Arm saved rule** after output starts. Arming is an
existing semantic engine action and can start musical activity. Paused/spent
execution history, playback cursors, pending launches/stops, held modes and meter
values are not resumed. The audio device must be selected each run. MIDI profiles
and skins retain their separate existing persistence.

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

The current engine has no pattern-note model or sequencer. Jam1 exposes its real
sample/loop/timing data operations; pattern creation/editing and corresponding
serialization belong to Jam2. Unsupported JSON fields and versions are rejected,
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
