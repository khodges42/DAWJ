# Troubleshooting

## Nothing plays

Check the output status, use Audio settings if initialization failed, then launch
a loaded part. Global Play starts the clock; it does not launch every sound.
Check both deck volume and crossfader. Missing files appear as unavailable.

## Volume or filter seems ineffective

Check the highlighted deck/part and its explicit Volume/Low-pass values.
VOLUME/FILTER buttons select contextual control meaning only. Source Listen uses
a separate audition path and does not pass through part shaping. Stop Listen
and use Play part to test the controls. A low-pass affects high-frequency content
most; bass-only sounds change less at high cutoffs. Sustained limiter activity
can mask gain changes; reduce output trim for more mix headroom.

## Assign does not replace the sound

Read the status next to Assign. A destination with playing audio, a queued action
or installed rule cannot change its source. Stop that part, cancel pending intent
or remove its rule, then retry. Other parts can continue. If it became active
during preparation, DAWJ keeps the previous sound and explains why.

## Steps do not change the loop

A continuous loop is not a step pattern. Choose Use as step sample while that
part is stopped, then add triggers. Pattern edits during playback activate at the
next global bar; the queued edit indicator shows the wait. Retriggers cut previous
sample tails. They do not beatmatch or stretch a long loop into sixteenth notes.

## Audio device disappeared or stream failed

Open Audio settings, close failed output, rescan and explicitly select the
intended endpoint. A restart creates a new clock with parts stopped. DAWJ does
not switch to an unrelated device behind your back.

## Interface is unreadable

Use Ctrl+Alt+Shift+R or launch with --safe-mode, then choose a readable skin and
font scale in Settings. Help and user sessions remain local and offline.
