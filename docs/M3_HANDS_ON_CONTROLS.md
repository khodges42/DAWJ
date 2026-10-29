# M3.3–M3.4 hands-on controls

Run `cargo run -p dawj --locked`. The supplied two-deck demo and all mouse/
keyboard controls work without any MIDI device. **MIDI: off/on/pickup/rejected**
opens device and mapping settings; hover it for details. Settings are separate
from the fixed performance grid. Close the settings window to return to the
full two-deck surface. Physical MIDI/LED behavior still requires controller tests.

The demo output-level correction defaults to +36 dB makeup before the existing
engine limiters. Output dB lowers that level. Both logical buses retain their
0.125 sample ceiling. Restart an old binary to use the corrected level.

## Connect and map a controller

1. Open MIDI settings and select an input. Select an output only if you want
   Note/CC feedback to the controller. Press **Connect / retry**. Ports are
   discovered on a worker thread, and the settings report actual connection
   state. No input is a normal supported condition. Microsoft GS Wavetable Synth
   is an instrument output, not a controller LED port.
2. Start demo audio. Use the factory map below or select a binding row. The
   editor supports channel, Note/CC number, action, fixed/selected/focused-deck
   target, button/absolute/relative behavior, absolute min/max, inversion and
   relative step. Part addresses shown as deck 0/1 and slot 0–7 in configuration
   mean A/B and slots 1–8 on the performance surface.
3. Press **Learn input (new)** to learn a new binding or **Learn selected** to
   replace a selected row's input. The next Note press or CC event supplies the
   channel/type/number; Note Off and velocity-zero Note On do not teach a button.
   Learning consumes the event instead of performing it. Existing conflicting
   inputs are rejected and remain unchanged; choose another input or replace the
   existing row. Cancel learn ends listening.
   A learned button must be released before its next press can perform; held
   repeats of the learn gesture are consumed. Replacing a map discards queued
   events from the old mapping rather than reinterpreting them as new actions.
4. **Add binding** or **Replace selected** applies manual edits. A button action
   requires Button behavior; gain/cutoff/context/deck-gain/crossfader actions
   require Absolute or RelativeTwosComplement behavior. Invalid ranges,
   addresses and conflicts produce a visible error instead of replacing the map.
5. **Save profile** persists mappings, LED configuration and desired port names.
   The location is shown in settings: `%APPDATA%/DAWJ/midi.json` on Windows;
   `$XDG_CONFIG_HOME/DAWJ/midi.json`, or `~/.config/DAWJ/midi.json`, on Linux.
   Saved profiles load automatically at startup. Existing saved port choices
   reconnect on startup; choosing None disables a side. **Load saved** changes
   the map; use Connect to apply its port choices during an existing session.

One version-1 JSON profile is active at a time. The checked-in
[generic example](examples/generic-midi.json) is the complete factory map and
can be used as a starting profile. To switch named profiles, keep copies of
this file and replace the displayed active profile file, then Load saved. There
is no library/browser or scripting language. Saving writes a bounded temporary
file, syncs it and renames it over the destination. A malformed/unsupported saved
file is reported and the application uses an in-memory factory map without
overwriting the bad file. Limits are 64 KiB, 128 mappings and 32 feedback slots.

Input/output names must uniquely identify a currently enumerated port. Duplicate
names are rejected for connection rather than choosing an arbitrary device.
Port disappearance closes its connection and clears temporary input state;
the worker retries the requested unique name every two seconds. Explicit
Disconnect closes both sides, while the saved choices remain available for
Connect / retry. If a driver retains stale port names after unplug, use Disconnect
and Rescan. Presence detection is limited to the backend's enumeration; a quiet
controller is not assumed to be disconnected.

## Factory map (MIDI channel 1)

These are editable generic conventions, not assumptions about a manufacturer.
Channels in the editor are 1–16; JSON stores them as 0–15. All data bytes are
0–127.

| MIDI input | Action |
| --- | --- |
| Notes 36–43 | Toggle A1–A8 on press |
| Notes 44–51 | Toggle B1–B8 on press |
| CC 16–23 | Contextual knobs A1–A8 |
| CC 24–31 | Contextual knobs B1–B8 |
| CC 1 | Crossfader, A=0 and B=127 |
| CC 2 / 3 | Deck A / B gain |
| Notes 80 / 81 | Persistent VOLUME / FILTER |
| Notes 82 / 83 | Hold VOLUME / FILTER until release |
| Notes 84 / 85 | Toggle cue A / B |
| Notes 86 / 87 / 88 / 89 | Beat / Bar / Phrase / Immediate |
| Notes 90 / 91 | Select deck A / B |
| Notes 92–99 | Select slot 1–8 on focused deck |

Launch, Stop, Cancel and Retrigger are available as separate editor actions,
along with Toggle. A fixed-address part mapping does not change GUI selection.
Selected-part and focused-deck-slot targets are also available. MIDI Note Off
and Note On velocity zero are releases. Repeated held notes/CC-button values do
not retrigger button actions. CC buttons use values 64–127 for pressed and 0–63
for released. Timing, active sensing, SysEx, pitch bend and raw running-status
streams are not mapped by this milestone.

## Modes and pickup

V/F select persistent VOLUME/FILTER from the keyboard. Hold **Shift+V** or
**Shift+F** for a temporary mode; release returns to the persistent mode. MIDI
Momentary actions use the same source-tagged mode stack. The screen highlights
the active mode and shows **HELD (base …)** during temporary use. Overlapping
holds restore the most recent remaining hold, then the persistent mode. Repeated
presses are idempotent. Releases are processed even during text entry and after
Shift is released. Window focus loss clears current keyboard and MIDI holds;
new MIDI presses can still perform while the application is in the background.
Disconnect, profile replacement and input overflow also reset MIDI holds.

The fixed sixteen-slot layout is identical in both modes. The selected-part GUI
control and each mapped contextual knob address the same part in either mode:
VOLUME changes gain, FILTER changes logarithmic cutoff. The cutoff maximum
bypasses the filter; there is no resonance. Mode changes send no parameter
commands. Launch/stop, deck gain, crossfader and cue remain available in both.

Absolute controls use soft pickup: the first value does not jump the parameter.
Move the knob to within two MIDI steps of the current target, or cross it. The
main MIDI badge says pickup, with details in its tooltip/settings. Pickup rearms
when its resolved parameter changes (mode/part/deck), after an external target
edit or after reconnection/profile reset. Absolute min/max calibrate raw input;
inversion follows calibration. Gain/crossfader are normalized 0–1; cutoff spans
20–20000 Hz logarithmically, with bypass at the top. Targets are derived from
accepted engine state plus bounded in-flight/coalesced parameter intent, never
GUI timestamps. Relative encoders need no pickup: **RelativeTwosComplement**
decodes 1…63 as positive steps, 127…64 as −1…−64, and 0 as no movement. Step is
normalized parameter travel per increment, default 1/127. Sign-magnitude and
offset/binary-64 encoders require a different mapping conversion and are not
claimed supported.

## State feedback and bounded delivery

Expand **Generic Note/CC LED feedback** to configure output messages and lifecycle
values. Each LED names a fixed deck/slot. Apply LED changes, then Save profile.
Defaults use the matching Note 36–51 with velocities:

| Engine lifecycle | Default output value |
| --- | ---: |
| Stopped / unavailable / closed | 0 |
| Queued start | 16 |
| Playing | 127 |
| Playing with queued stop | 64 |
| Playing with queued replacement/retrigger | 32 |

Values are ordinary 7-bit Note velocity or CC data, not color codes. Some
controllers treat every nonzero value identically; consult the device's MIDI
implementation and change values as needed. No RGB/SysEx protocol is assumed.
Route the feedback return to the controller, not directly into DAWJ's own input
or an instrument, which could echo messages or play notes.

Feedback follows the coherent engine snapshot. Receipt of a Note or successful
queue admission never produces a Playing LED. Connection/profile changes refresh
all configured LEDs; unchanged states send nothing. The worker sends at most
16 changed messages per 50 ms during normal operation. A full output queue retains
the unsent state for retry, and output failure resets feedback on reconnect.
Shutdown attempts the configured stopped values before dropping the output;
physical delivery cannot be guaranteed after device failure.

MIDI callbacks normalize into a 256-event bounded channel with no engine handle.
The GUI/control thread drains at most 128 events per update, then dispatches
through the same application `Intent` API as mouse and keyboard. Per-part knob
traffic coalesces into sixteen latest targets. At most four parameter requests
are admitted per update, round-robin, with eight receipt slots reserved for
discrete actions. Gain and cutoff edits to the same slot merge without losing
the other parameter. The existing audio command queue remains 32; discrete
exhaustion/rejection stays explicit. Input overflow drops the untrusted batch,
resets momentary state/pickup and reports a counter. Connection epochs prevent
old-device messages from launching parts after reconnect. Device open/close,
discovery, output sends and profile I/O never run in the audio callback.

## Reproduce without hardware

```powershell
cargo run -p dawj --example synthetic_midi --locked --offline
cargo test -p dawj --test midi --locked --offline
```

The example injects normalized synthetic MIDI, renders the real engine offline,
prints queued and Playing LED values, switches FILTER/held VOLUME with pickup,
and reloads its profile from `target/synthetic-midi-profile.json`. It opens no
MIDI/audio hardware and is not evidence of a physical controller or LED working.
The same bounded ingress adapter is separately exercised from a producer thread.

## Manual controller acceptance

1. On Windows, supply an existing virtual MIDI input/feedback pair (for example
   ports provided by a separately installed virtual cable), or a real generic
   controller. DAWJ's WinMM backend cannot create virtual ports itself. Connect
   the input and optional feedback output in settings. Observe actual status.
2. Start audio, select Bar, send channel-1 Note 36 with nonzero velocity, then its
   release. Confirm Queued start / pending bar in the GUI, then Playing at the
   scheduler boundary. Capture the corresponding 16 then 127 output values.
3. Send Note 81 to select FILTER. Move CC16 through its pickup value, then shape
   cutoff. Hold Note82, move CC16 through the gain target, change gain, release
   Note82 and confirm FILTER restoration without a parameter jump. Also test
   overlapping holds and Shift+V/F release with text entry and focus loss.
4. While A plays, launch B with Note44 and adjust CC1/2/3. Verify independent cue
   only on a physically verified four-channel interface; stereo remains master
   only. Confirm state-driven queued-stop and retrigger feedback.
5. Learn a spare input, test a deliberate conflict, save, restart and confirm
   mapping/port choices. Disconnect while holding a temporary mode, reconnect,
   confirm restored persistent mode, rearmed pickup and refreshed feedback.
6. Repeat with encoder relative messages, rapid knobs plus Stop/Cancel, device
   loss and driver errors. Record actual devices, MIDI mode, routing, callback
   diagnostics and listening/output continuity over a sustained set.

Current software verification and the exact limits of Windows/hardware testing
are in [the implementation report](M3_3_4_IMPLEMENTATION_REPORT.md). No skin,
MIDI scripting, new scheduler, pattern editor or additional DSP mode is included.
