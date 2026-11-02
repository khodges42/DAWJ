# Performing with DAWJ

Use `cargo run -p dawj --locked` on Windows with WebView2. Rust 1.93.0 and
the existing Tauri versions are pinned. No controller or external assets are
needed. This is a reproducible manual performance exercise, not a claim that a
five-minute human listening or hardware session was performed during development.

Before the clock starts, choose the intended audio device, **Stereo master**,
**8 bars**, and **Beat** quantization. Use a comfortable system/headphone level.
The existing demo defaults to +36 dB makeup and retains its output limiters.
Stereo output has no isolated cue bus. Four-channel cue requires a compatible
device and separate physical verification; do not use it to infer headphone
isolation. A rejected configuration is shown as an error, not silent success.

## 0:00–1:00 — start Deck A and shape it

Press **Start output**. The clock should advance at 120 BPM; all sixteen parts
start stopped. Launch **A1 kick**, **A3 hats**, and **A4 bass** using their ▶
buttons. Observe queued labels before their engine-scheduled Playing states.
Select A4 with its name button. VOLUME knobs and the selected Gain slider control
the actual part gain through the engine; reduce it slightly.

Press **F** for FILTER. The same sixteen knob positions now show cutoff; lower
A4's cutoff, then use **Bypass filter** in the inspector to restore the bypassed
target. Changing mode alone must not change either gain or cutoff. Watch the
selected feedback line for receipts, and the status labels for actual execution.
Master meters are lifetime held diagnostic peaks, not an RMS/loudness display.

## 1:00–2:00 — queue, cancel, retrigger and temporary modes

Click a part's name or deck Select button to leave text-input focus before using
shortcuts. Press **A**, **5**, then **F4** (phrase quantization) and **Space**.
A5 should queue for the next phrase boundary, marked L on the timeline. Before
that boundary press **Escape**; observe the queued marker disappear after the
engine cancels it. A cancellation after execution is rejected rather than invented.

Press **F2** for Beat, select A4 and use **Retrigger**. Its queued retrigger and
cursor reset come from the engine. Keep persistent FILTER, hold **Shift+V** for
temporary VOLUME, then operate A4's knob. Release V or Shift: FILTER must return.
Do not expect a parameter change just from pressing/releasing a mode key. Click
elsewhere to remove input focus if a shortcut is being used for typing instead.

## 2:00–3:00 — introduce Deck B and mix

Start **B1 kick**, **B4 bass**, and **B5 chord**. Both decks remain visible.
Bring the crossfader from A toward B. Arrow keys adjust it by 5% and permit
auto-repeat; other launch shortcuts ignore repeats. Adjust the deck gains and
Output dB if needed. Their positions are accepted targets; smoothing remains
inside the engine.

Set BPM to 123 and press **Apply BPM**. Observe the published tempo change at
the scheduler's admitted boundary, with a coherent clock and phrase view. Stop
A3 independently; Deck B should continue. Cue buttons are disabled on stereo.
With a separately verified four-channel setup, Q/W toggle A/B cue selections.

## 3:00–4:00 — fills, recurrence and the repeating timeline

Press **Fill A** or **Fill B** for slot 8's existing one-shot. Select A8, choose
**Every N bars**, N=2, Deck phase, and **Arm rule**. The timeline shows next and
conditional later fill occurrences. `~` means a projection that can skip due to
manual-command precedence. `!` identifies the last fired occurrence; F identifies
a fill, S a pending stop, and R a retrigger.

Choose 2-bar zoom, move a page, then re-enable Follow. These controls only change
the view. Select an event using the canvas or **Scheduled actions** menu; both
decks stay in place and the inspector follows its part. Pause/resume the rule,
then remove it. Removing a rule does not stop an already active source; use Stop
for that. To exercise fill cancellation, use Phrase quantization, queue a fill,
then Cancel fill before its boundary. Return to Beat afterward.

## 4:00–5:00 — configure MIDI or finish with keyboard

Open **MIDI / settings**. The audio engine continues independently. Without a
controller, the device status should truthfully say disconnected; all main
performance controls remain usable after closing settings.

For configuration without hardware, choose a mapping action and inspect its
target, behavior and range. Adding a conflicting channel/type/number is rejected
by Rust. Learn can be armed and cancelled without a controller. **Save as**
`rehearsal` persists a named profile and selects it for the next Tauri restart.
Reload it to verify its contents. Default is the legacy MIDI file; if no saved
default exists, use Factory map and Save current before trying to load it.

With a real generic controller, choose its input and optional LED output, then
Connect/retry. Select Context with Absolute behavior for a knob, or Momentary
FILTER for a button; use Learn to supply its actual input. Release the learn
gesture before performing. On mode/part changes, move absolute knobs through
the shared core's pickup target before expecting motion. Relative encoders use
two's-complement steps. LED sent counts report successful port sends, not proof
of a physical light. Note/CC feedback values are device-dependent. Connect/retry
applies a loaded profile's port choices; loading alone does not switch ports.

Close settings, independently stop the remaining parts, then at **5:00** press
**Stop / reset**. This closes output; restarting constructs a fresh demo clock.
There is no pause/resume transport in the existing engine. Egui remains available
via `cargo run -p dawj-egui --locked`.
