# M3 performance walkthrough

Install the pinned Rust MSVC toolchain and Visual Studio C++ Build Tools/Windows
SDK as described in README. On Windows 11 x64, from the repository root:

```powershell
cargo run -p dawj --locked
```

The first build downloads GUI dependencies. Once cached, `--offline` is supported.
The application opens with both decks visible and audio off. No external samples
or manifest are required. **Start demo output** prepares the original Copper Steps
and Glass Tide assets at 120 BPM / 48 kHz. Slots 1–7 loop; slot 8 is a one-shot fill.

Choose the output in the top row before starting. The initial selection is the
first enumerated device, which may differ from the OS default. Stereo is master
only: headphone cue is unavailable. Enable four channels only for an output
supporting 48 kHz / four channels. This routes logical master to 1/2 and cue to
3/4; verify physical routing separately on your interface. Unsupported formats
remain visible errors, with no successful-looking silent fallback.

## A five-minute performance

1. Start output. Confirm **Rendering**, advancing bar/beat and **120.0 BPM**.
   The clock continues while every part is stopped. Select **Beat** quantization.
2. Select **A**, then slot **1** and press **Space** (or click its Launch).
   Repeat for slots 2 and 3. A launch received between boundaries displays
   **QUEUED START**, with target bar/beat, before **PLAYING**. A command arriving
   exactly on a boundary may apply immediately. The master crossfader starts
   centered and deck gains at 0.5; use the crossfader to favor A.
3. Select A4 (bass), Launch, then **VOLUME**. Move Part gain. Switch to **FILTER**
   and lower cutoff to about 500 Hz, then open it or use Bypass. Mode selection
   itself changes no audio. Focus and slot positions remain stable.
4. Select **Phrase**, launch A5, then Cancel before its displayed boundary.
   It should return to Stopped. Launch it again and let it reach the boundary.
   While another part plays, queue its Stop and cancel that Stop; its source
   should keep playing. A stale cancellation may explicitly reject.
5. Select B, launch B1/B4/B6 with Beat or Bar quantization. Both deck grids remain
   visible. On four-channel output, toggle B's Headphone cue; on stereo the
   control stays disabled with an explanation. A cue selection is not proof of
   physical isolation. Bring the crossfader toward B while both clocks advance.
6. Select an active part and press **R**. Observe its queued retrigger and boundary;
   its source position resets after that boundary. Slot 8 plays one full bar as
   a one-shot and returns to Stopped naturally. No automation is secretly enabled.
7. Stop A's parts independently with their Stop buttons. B continues. Try a
   tempo edit using **Queue tempo**: the field is proposed intent; the separate
   current BPM changes only at the engine's scheduled boundary. Native sample
   loops are not time-stretched and can drift against changed tempo.
8. Use **Stop output** to end. This closes the stream and retains a stale last
   musical display. **Start demo output** begins a new stopped session and clock;
   it does not resume the previous position. Closing the window releases audio.

All sixteen slots show actual engine lifecycle. The footer reports command
admission and receipts; hover a slot name to inspect that slot's last receipt.
Rejected, stale and expired commands do not create a Playing indication.
Cutoff is the existing nonresonant low-pass; there is no resonance control.

## Initial keyboard map

Press **H** or Help for the same bindings inside the application. They are
centralized in `performance-app/src/window.rs`; no external configuration format
or MIDI device is required in this milestone.

| Key | Action |
| --- | --- |
| A / B | Select deck; each remembers its selected slot |
| 1–8 | Select slot on focused deck |
| Space | Launch selected inactive part / stop selected active part |
| X | Stop selected part |
| Escape | Cancel the pending ID observed in the engine snapshot |
| R | Retrigger selected active part |
| F1 / F2 / F3 / F4 | Immediate / Beat / Bar / Phrase quantization |
| V / F | VOLUME / FILTER mode |
| Left / Right | Crossfader toward A / B in 5% steps |
| Q / W | Toggle cue A / B when separate cue channels exist |
| H | Toggle help |

Shortcuts do not intercept text/numeric entry. Finish editing a number before
using performance shortcuts. Selecting quantization affects subsequent actions;
it does not retime an already queued action. Tempo uses Beat if Immediate is
selected, as required by the engine's existing tempo interface.

## Reproduction and recovery

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
python scripts/audio_dependencies.py
```

The original offline fixture still runs without opening hardware:

```powershell
cargo run -p audio-backend --example performance --locked --offline -- --render target/performance.wav --script docs/examples/m2-performance.txt
```

For an opt-in visible Windows smoke check after building `dawj`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke_m3.ps1
# Explicit real audio, short keyboard sequence, screenshots and graceful close:
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke_m3.ps1 -Play
```

The smoke helper runs `--smoke-window` or `--smoke-test`. The latter drives the
same coordinator as the UI, checks actual playing sources and accepted gain/
cutoff after launch and cancellation, captures the viewport through eframe, and
closes. It writes `target/m3-native-capture.png` and stream summaries to
`target/m3-native-smoke.log`, returning failure if audio/state/capture checks fail.
It does not inject OS keyboard/mouse input or capture the desktop. The helper is
a delivery check, not a listening, physical cue or manual interaction test. The
application writes a stream/configuration summary on start and a final diagnostic/
snapshot summary on close.

No device: connect one and Rescan. Unsupported configuration: choose another
device or stereo routing. Runtime failure: output closes, shows an error and
keeps the last display; check the device, Rescan and Start a new session.
If callback snapshots stop advancing for one second, audio controls disable
and the display is marked stale. Stop/Start explicitly recreates the engine.
Queue exhaustion or rejected controls appear in the footer; no automatic replay
of an old gesture happens later. Held peaks and callback-budget counts are
diagnostics, not dropout measurements or headphone-isolation evidence.
