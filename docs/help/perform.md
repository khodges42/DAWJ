# Perform and mix

## Audio and musical transport

The output device remains open during normal use. **Play/Pause** controls musical
time, independently of opening or closing the device.

- **Play** resumes the shared clock. It does not launch every loaded part.
- **Pause** freezes both decks' source positions, patterns and queued deadlines
  at the current musical position. Short fades prevent abrupt output edges.
  Resume continues from those positions. Audition pauses too.
- **Stop** immediately stops both decks, clears queued launches/stops, rules and
  source cursors, then freezes the shared clock at its current position. It keeps
  sounds, pattern definitions and accepted edits. Play does not relaunch them.
- **Deck Play** launches that deck's loaded stopped parts at the current grid.
  **Deck Stop** immediately clears that deck's execution and rules. The other
  deck and shared clock continue. Independent deck pause is not available.

Manual Launch or Retrigger resumes a paused shared transport, including from
existing keyboard and MIDI mappings. Other held parts resume too. Stop/Cancel
do not resume the clock. Toggling an already active part requests its stop.

The timeline cannot seek or scrub. Moving the shared clock while preserving
quantized intent requires a separate transport design.

Drag BPM to adjust tempo; hold Shift for fine control. Double-click to type an
exact value and press Enter. Tempo changes follow the selected quantization;
Immediate tempo intent still waits for the next beat. Requested and Applied
values are separate; Applied comes from the engine. Sample speed is unchanged.

## Parts and mixer

Launch, Stop, Cancel queued and Retrigger operate on the addressed part. Quantize
selects Immediate, Beat, Bar or Phrase. Pending launches are distinct from playing.
Fills use slot 8; periodic rules are available in Extended.

Part **Volume** and **Low-pass** controls shape that part's rendered signal.
V/F choose the meaning of contextual controls; they do not change sound alone.
Listen is a separate source audition path. Heavy limiting can reduce the audible
effect of gain changes; lower output trim if the mix is constantly at its ceiling.

The center strip has A/B volumes, crossfader, output trim and meters. Stereo
hardware has no isolated headphone cue. Cue requires real four-channel routing;
verify physical connections before relying on it.

## Performance keys

Outside text entry: **A/B** select deck, **1–8** select part, **Space** toggles
that part, **X** stops it, **R** retriggers and **Escape** cancels its queued
action. **V/F** choose volume/filter; **Shift+V/F** hold a temporary mode.
**F1–F4** select quantization; **left/right arrows** move the crossfader;
**Q/W** toggle deck cue. Focus loss releases held modes.
