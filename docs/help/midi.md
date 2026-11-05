# MIDI controllers

No controller is required. **Tools > MIDI configuration** opens input/output
selection, saved profiles, learn and the mapping editor.

1. Select the intended MIDI input. Choose an LED output only if your controller
   supports ordinary Note/CC feedback; a synthesizer output is not an LED port.
2. Connect, choose a semantic action and target, then use Learn or enter its
   channel/message/number explicitly.
3. Choose button, absolute or relative behavior. Absolute parameters use pickup
   to avoid jumps. Save the profile when satisfied.

Fixed deck/slot mappings retain their identity across Sketch, workspace and skin
changes. Selected-part mappings follow the highlighted part. Focus loss releases
momentary modes; device disconnect is reported rather than silently ignored.

MIDI diagnostics describe logical messages, not proof of physical LED colors or
controller compatibility. Real hardware should be checked before a performance.
