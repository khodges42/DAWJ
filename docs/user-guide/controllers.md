# MIDI controllers

Both frontends work without a controller. In Tauri, open **MIDI / settings**;
in egui, open the MIDI status/settings control. Choose a uniquely named input
and, optionally, a controller LED output, then **Connect / retry**. A software
instrument output such as Microsoft GS Wavetable Synth is not a controller LED port.

The shared Rust mapper supports channel-aware Note On/Off and Control Change,
learn, configurable absolute ranges/inversion, relative encoders and generic
Note/CC feedback. No vendor SysEx, RGB protocol or controller scripting is assumed.

## Learn and profiles

Choose an action, target and behavior before Learn. The next Note press/CC learns
its channel/type/number and is consumed rather than performed. Release it before
using it. Cancel ends learning. Conflicting inputs reject without replacing the
valid map; choose another input or explicitly replace a selected row.

Tauri supports named profile selection, Save current and Save as. The legacy
default file is `%APPDATA%/DAWJ/midi.json` on Windows, or
`$XDG_CONFIG_HOME/DAWJ/midi.json` / `~/.config/DAWJ/midi.json` on Linux. The UI
shows the active location. Loading a profile changes mappings; Connect/retry
applies its saved port choices. Egui retains its legacy default-file workflow.

The [factory profile](../examples/generic-midi.json) is a complete JSON starting
point. Profiles are versioned and bounded: 64 KiB, 128 mappings and 32 feedback
slots. Invalid/stale edits reject; malformed saved files are reported without
overwriting the original. Saving uses a temporary file and replacement.

## Factory mapping

MIDI channel 1 (JSON channel 0); configuration uses deck 0/1 and slots 0-7.

| Input | Action |
| --- | --- |
| Notes 36-43 / 44-51 | Toggle A1-A8 / B1-B8 |
| CC 16-23 / 24-31 | Contextual knobs A1-A8 / B1-B8 |
| CC 1 | Crossfader, A=0 and B=127 |
| CC 2 / 3 | Deck A / B gain |
| Notes 80 / 81 | Persistent VOLUME / FILTER |
| Notes 82 / 83 | Hold VOLUME / FILTER |
| Notes 84 / 85 | Cue A / B |
| Notes 86 / 87 / 88 / 89 | Beat / Bar / Phrase / Immediate |
| Notes 90 / 91 | Select deck A / B |
| Notes 92-99 | Select focused-deck slot 1-8 |

Separate launch/stop/cancel/retrigger and fixed/selected/focused-deck targets are
available in the editor. Fixed-address actions do not move GUI selection.
Note Off and velocity-zero Note On release buttons. Repeated held messages do
not retrigger. CC buttons treat 64-127 as pressed, 0-63 as released.

## Modes and pickup

VOLUME controls gain; FILTER controls logarithmic cutoff with bypass at the top.
Changing a mode alone sends no audio parameter change. Persistent and held modes
share source-tagged control state. Release, focus loss, disconnect, overflow and
profile replacement clear temporary input state. New MIDI presses can still
perform with the window in the background.

Absolute knobs use soft pickup: approach within two MIDI steps of the current
target or cross it before motion applies. Pickup rearms after target/mode/part
changes, GUI edits, reconnection and profile reset. The status reports pickup;
a stationary unmatched knob is not a failed connection.

`RelativeTwosComplement` interprets 1-63 as positive steps, 127-64 as -1 through
-64, and 0 as no motion. Step is normalized in the editor. Relative controls do
not need pickup. Absolute min/max calibrate raw input before inversion.

## Feedback and reconnect

LED values represent engine-published stopped, queued, playing, stopping and
replacement states. A received MIDI command does not itself light an executed
Playing state. Values are ordinary device-dependent Note/CC intensities.
Successful send counts are not proof a physical LED lit.

Names must uniquely identify a port. Disappearance closes the connection and
resets holds/pickup; the worker retries the requested name about every two
seconds. Use Disconnect/Rescan if a driver retains stale names. There is no
automatic fallback to an unrelated port.

Synthetic tests exercise production parsing, the bounded callback bridge, floods,
relative motion, pickup and state feedback. Physical device rate limits, USB
disconnect/reconnect and LED behavior still require controller testing.
