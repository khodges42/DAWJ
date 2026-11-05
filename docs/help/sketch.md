# Compose in Sketch

Choose a part, browse a sound, assign it, and program its steps in the focused
editor. The editor names the exact sound and part being edited.

## One-shots and patterns

Click a step to add or remove a trigger. A pattern can be 1, 2 or 4 bars long;
the page selector chooses the bar to edit. Set a step velocity from 0 to 127;
0 is a rest, and strong velocities are shown as accents. Duplicate, rotate and
clear operate on the pattern. Stored variations use the same engine edit path.

**Play part** launches the groove using the selected quantization. While playing,
pattern edits become active at the next global bar. The editor distinguishes
accepted, queued and confirmed pattern state. A stopped edit applies on the next
engine admission. Use Undo/Redo for content edits.

## Continuous loops

A continuous loop plays its source region at its natural speed. Its steps are
not a second gate sequencer. Choose **Use as step sample** to turn it into a
one-shot trigger source, stopping that part first. Each trigger restarts the
region; later triggers cut the previous hit. There is no time stretching.

You can assign to an empty or stopped part while other parts play. An active,
queued or rule-controlled destination must be stopped and cleared before changing
its audio. Every attempt reports preparation, success or rejection near Assign.

## Optional musical sections

Open **Advanced musical sections** to add, rename, move or remove descriptive
labels such as Intro, Build and Drop. They annotate the overview; they do not
launch parts, change volume or create an arrangement. Emphasis is visual only.
Sections, display length and label visibility are saved with the composition.

[Sounds and graphical trimming](samples.md)
