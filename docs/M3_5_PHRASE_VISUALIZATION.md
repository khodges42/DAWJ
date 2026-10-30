# M3.5 phrase visualization and performance feedback

Run `cargo run -p dawj --locked`, select output and Start demo output. The
performance screen now places a repeating musical ruler above both eight-slot
decks. Transport, BPM, contextual VOLUME/FILTER controls, mixer, cue and MIDI
remain available. No new dependency, DSP mode, arrangement editor or scheduler
was introduced.

## Read the phrase

The default engine phrase is **eight bars** of four beats. The top transport
shows absolute global bar/beat and current scheduler BPM. The phrase header
shows global position within that repeating reference. Each deck independently
shows its engine-published phrase position. Ruler numbers repeat 1–8; deadlines
in cards and tooltips use absolute global bar/beat, so the following phrase's
first bar can read 9.1 while appearing at the ruler's right-hand wrap boundary.
The common ruler is anchored at global zero; deck epochs remain distinct engine
metadata rather than inferred from the first launched sample.

The white playhead is the engine's next unrendered musical tick position. It
updates only with coherent snapshots, not interpolation from UI frame time. It
is not a DAC timestamp. A pending action stays queued until the callback renders
its boundary sample, including when the displayed playhead has already reached
that boundary. The existing stale/closed indication freezes the last musical
observation and disables audio actions.

- **A/B lanes:** current active-source counts and that deck's pending/rule markers.
- **Selected lane:** selected part's nominal musical cycle, corresponding markers
  and inspector tooltip. This cycle is independent of native sample loop length;
  tempo edits do not stretch samples or predict their EOF.
- **Cards:** all sixteen slot identities, actual lifecycle, pending deadline or
  next rule, and the existing direct Launch/Stop/Cancel controls. One-shot sources
  are labelled FILL. Selection remains highlighted in its fixed card position.

Marker codes supplement color:

| Code | Meaning |
| --- | --- |
| L | Queued manual launch |
| S | Queued stop; source is still active |
| R | Queued retrigger; source is still active |
| X | Queued replacement where present in engine state |
| F | Queued manual one-shot fill |
| P | Next periodic/Once trigger for a looping Part |
| F* | Next rule trigger for a one-shot Part |
| ~P / ~F* | Later conditional recurrence projected from published period |
| ! | Last successfully fired event retained by the installed rule |

A single marker includes its slot number, such as R1 or F8. Actions sharing a
deadline are grouped: **L/S/F** exposes mixed action types; **Lx3** means three
launches. Hover for the complete list, identities, absolute targets and command
IDs. Rule triggers can be skipped by manual precedence; future projection is
not a promise of additional queued actions. A last-fired marker is one retained
rule observation, not an archival activity log or a MIDI-note/pattern display.

## Inspect and navigate

Click a card or timeline marker to select that part. Click a deck lane label to
focus that deck. A marker group selects its first manual action (or first rule
entry); its right-click menu lists every member for explicit selection. Both
decks remain visible. Selection preserves the active/persistent/held control mode,
and M3.4 pickup still rearms when a MIDI knob resolves to a different parameter.

Use **View 1/2/4/8b** to zoom, limited to the engine phrase length. **Follow**
pages with the actual musical position. Turn it off to freeze the current phrase
window; `<`/`>` page within that window and also stop following. Turning Follow
on returns to the current engine phrase. Zoom, paging and selection never seek,
schedule or retime audio. Active counts and inspector data remain current even
while viewing a frozen window; the view is not a historical arrangement.

Choose **4 bars** or **8 bars** before starting output. The scheduler fixes phrase
length at construction, so this selector is disabled during playback. Stop output,
choose the length, then Start for a fresh engine/clock. Existing queued deadlines
are not silently changed. Other phrase sizes and live phrase editing are not
offered by this demo adapter.

## Fill and periodic controls

**Fill A / Fill B** launch the resident one-shot in that deck's slot 8 using the
currently selected quantization. **Cancel A / Cancel B** cancel the observed
manual pending action in slot 8; they do not stop an already sounding fill or
remove a periodic rule. For an active source use its Stop control. Factory MIDI
notes 43/51 already address these slots through M3.4's semantic mapping.

The row below the ruler operates on the selected part:

1. **Once** installs an existing one-time engine rule at current quantization.
   Once describes one trigger; a looping Part will keep looping after that trigger.
2. Choose **Every N bars** (1–32) or **Phrase bar** (one-based bar in the current
   four/eight-bar deck phrase), then **Arm** to install/replace the engine rule.
   No rule is installed until Arm. The row's fields are proposed rule intent;
   actual next deadline, fired/skipped counts and paused/spent status come from
   the engine snapshot. Hover the selected part/status for its accepted definition.
3. **Pause / Resume** disables/rearms an existing unspent rule using the engine's
   boundary policy. A spent Once cannot resume; install another Once to rearm.
4. **Remove rule** cancels future rule intent. It leaves an active source sounding.
   **Trigger** manually launches the selected part at launch quantization.
   **Cancel queued** cancels its observed manual pending ID, preserving playback.

Right-click a timeline marker for selection and direct **Cancel queued** or
**Remove rule**. There is no dragging, note drawing, seeking or grid-cell editing.
Manual cancellation IDs may become stale; rejection is visible and never treated
as success. Rule removal uses the existing slot-scoped CancelRule command.

The M2 precedence policy remains intact: manual pending intent can skip periodic
occurrences, and manual Stop disables recurrence. Cancelling a manual action does
not rearm a disabled rule. The inspector displays actual skipped counts and
paused status. Removing a rule clears its retained observation; it is not an
erase-history action or a stop-all command. No UI timestamp decides a boundary.

## Short rehearsal

1. Start at eight bars, launch A1 and B4. Watch both current counts and cycles.
2. Select Phrase quantization, queue B1 and B8, and retrigger A1. At the right
   wrap boundary inspect the grouped L/F and R markers. Cancel B8 before its
   target; its marker clears only after engine admission.
3. Select A8, choose Every 1 bar and Arm. Observe F*8, then its fired count and
   last-fired marker. Later ~F*8 occurrences are conditional projections.
4. Queue a later manual launch in A8; periodic occurrences can now skip. Confirm
   the skipped counter. Cancel the manual pending action and inspect the next
   accepted recurrence. Pause, Resume and Remove the rule independently.
5. Zoom to one bar, page, toggle Follow, select a marker and switch/hold VOLUME
   or FILTER. The clock and other deck continue. Queue a tempo edit; current BPM
   changes at its engine boundary while pending musical targets retain their ticks.
6. Stop output and choose four bars, then Start. Confirm the shorter actual phrase
   and available zoom choices. This is a new session, not live phrase retiming.

Software and native-check evidence, plus remaining listening, physical-controller,
cue, disconnect and sustained-performance limits, are in the
[implementation report](M3_5_IMPLEMENTATION_REPORT.md).
