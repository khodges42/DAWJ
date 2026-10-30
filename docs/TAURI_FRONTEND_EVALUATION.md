# Tauri 2 performance frontend evaluation

Evaluated 2026-10-29 on Windows x64, against the completed M3.5 application.

**Recommendation: use Tauri for the next customization-focused frontend iteration,
while retaining the existing native GUI until performance feature parity and
long-running tests pass.** HTML/CSS is a strong match for the desired user
customization. The experiment successfully controls the real engine; there is no
evidence here requiring an audio-engine rewrite. It is not yet a replacement for
the complete egui application.

## Where the implementation lives

The prototype is committed on **`experiment/tauri2-performance`**, ending at
**`d3aade5`**:

- `726a60b`: native Tauri frontend, engine adapter, two decks and CSS themes.
- `d3aade5`: tests, measured results, dependency inventory and reproduction notes.

Its independent nested workspace is `tools/tauri-eval`. **Only this findings
document is added to main.** The experiment was not merged, and nothing was pushed.
Production GUI source, workspace membership, root lockfile, audio callback and
Rust 1.85.1 pin remain unchanged.

To try it from a clean checkout:

```powershell
git switch experiment/tauri2-performance
cargo +1.93.0 run --manifest-path tools/tauri-eval/Cargo.toml --locked
```

Prerequisites are the installed Rust 1.93.0 toolchain, MSVC C++ build tools and
WebView2. No npm install, Node application server, Tauri CLI or JavaScript framework
is required. Choose an output device, Start output, then launch parts on either
deck. Stop output releases audio. Return with `git switch main` after testing.
The branch README contains full commands and the CSS example.

## Implemented vertical slice

- Two simultaneously visible decks, with eight fixed slots each.
- Existing Copper Steps / Glass Tide resident demo assets and actual CPAL output.
- Independent Launch/Stop actions through the existing semantic `Intent` dispatcher.
- Authoritative stopped, queued start, playing, queued stop and retrigger/replacement
  lifecycle labels, including pending musical boundary ticks when present.
- Engine bar/beat, BPM, transport, output availability, errors and diagnostic peaks.
- Existing engine crossfader target with engine-side smoothing; input is coalesced.
- Dark, light and high-contrast CSS themes, an accent picker, and local CSS import
  with reset. Theme, accent and imported CSS persist in WebView local storage.

The demo retains existing +36 dB output compensation and the backend limiter;
no alternate gain staging or DSP was implemented. Output is explicitly stereo
master only, without independent headphone cue. It requires supported 48 kHz,
stereo float output and reports configuration/start errors.

The adapter follows DOM -> typed Tauri IPC -> existing Coordinator -> bounded
engine controls. Audio publishes the same coherent M2.9 snapshots; the frontend
does not infer Playing from submitted actions. Snapshot polling waits for its
previous reply and then 33 ms. Browser and Rust IPC admission each cap outstanding
requests at 32; crossfader updates retain one latest unsent value and one in flight.
Existing receipt, scheduler and audio queue bounds are preserved. Serialization,
device access and WebView rendering all happen outside the realtime callback.

The main thread owns CPAL and Coordinator in a thread-local session; no Send/Sync
wrapper or audio-thread lock was added. Native exit closes admission and drops
the stream. Asset preparation/device opening can briefly stall initialization on
that thread; a production adapter should prepare assets off the UI thread. The
crossfader display reports an accepted target, and meters remain diagnostic
atomics; neither is presented as a coherent DSP readback.

## Measurements and verification

These are **debug-build measurements**, not release benchmarks. The native smoke
test executed JavaScript inside an actual Windows Tauri/WebView2 window, invoked
Rust, opened CPAL output, and observed engine publications. It also checked an
independent part stop, invalid-command rejection and output shutdown.

| Observation | Result |
| --- | --- |
| First complete integration run, 100 serial snapshot IPC calls | Median 1.1 ms; p95 1.7 ms; max 2.1 ms |
| Repeat with theme/layout checks, 100 calls | Median 1.2 ms; p95 1.7 ms; max 2.4 ms |
| Launch response to observed active snapshot | 503.5 ms / 475.2 ms in those runs |
| Engine lifecycle | Queued launch observed before active A1 and B4 |
| Independent stop | A1 stopped while B4 remained active |
| Invalid inputs | Deck 2 and crossfader 2 rejected through real IPC |
| Engine output meters | Nonzero stereo peaks, approximately 0.0565 / 0.0452 |
| Output shutdown | Subsequent snapshot reported offline |
| CSS presets | Three distinct computed background colors |
| Control geometry at 1280 x 820 client size | All 32 part buttons and crossfader within viewport |

The launch measurement starts after launch replies and includes deliberate beat
quantization and observation delay. At 120 BPM one beat is 500 ms; these waits are
consistent with scheduler behavior and are not IPC latency. IPC samples measure
invoke-to-reply time after the frontend serial queue admits a request, excluding
display paint and polling delay. A separate real-engine Rust test observed command
admission in roughly 0.2–8.2 microseconds over sixteen submissions, but this is a
different path and **not** an apples-to-apples egui/Tauri latency comparison.

The small IPC cost is encouraging for launch/mixer controls. It does not establish
audible input latency, worst-case delivery under load, or equal native performance.
No frontend clock participates in quantization or scheduling.

Checks completed:

- Production: formatting, workspace check, Clippy with warnings denied, and all
  **119 workspace tests** passed, including offline, timing, realtime, MIDI and
  timeline regression coverage.
- Production dependency inventory: **185 external Windows packages**, unchanged,
  passed `python scripts/audio_dependencies.py`.
- Isolated Tauri workspace: formatting, check, Clippy with warnings denied and
  **2 Rust tests** passed. Integration exercises all sixteen real engine slots.
- Frontend: **4 Node tests** passed, covering addresses, authoritative labels,
  valid part ID zero, and crossfader mapping/validation; application syntax checked.
- Native smoke: actual CPAL startup, two-deck playback state, queued state,
  crossfader admission, rejection, independent stop and audio shutdown passed.
  Raw numeric results are committed as `tools/tauri-eval/smoke-results.json`.

The window smoke was automated. Human visual usability and listening were not
verified. Neither physical cue isolation nor speaker/DAC latency was measured.
No Linux build, installer, touch/controller test, device-loss recovery test,
heavy-rendering benchmark or long soak was performed. WebView2 logged class
unregistration error 1412 at teardown after successful metrics and process exit;
triage this before shipping. A smoke process exit alone is not a success verdict:
the README instructs checking its `TAURI_METRICS` record for errors.

## Customization direction

CSS provides a much easier user-facing skin path than exposing egui rendering
internals. Colors, fonts, spacing, borders and deck emphasis already change without
editing Rust or rebuilding audio code. Basic users should get a small theme panel
with safe tokens, live preview, reset and theme import/export. Advanced users can
edit CSS against documented stable class names and variables. Portable, versioned
theme files should replace reliance on local storage before broader distribution.

Retain the two-deck geometry and minimum control sizes as a performance contract.
Offer font scaling and contrast options first. CSS files should change presentation;
semantic actions and state projection should remain in tested application code.
Do not require users to write JavaScript to recolor or rearrange permitted areas.
The prototype imports CSS only, with a 64 KiB cap, local assets and restrictive CSP;
no filesystem/shell/remote-content plugins are enabled. Custom CSS is trusted input
and can hide controls, so production reset/recovery must remain reachable outside
the themed surface. No arbitrary HTML/JavaScript theme execution is needed.

## Costs and migration gates

Tauri 2.12.5 / tauri-build 2.7.4 were pinned for this test. The resolved Tauri
utilities require Rust >=1.90; the experiment uses the already installed 1.93.0.
Rust 1.85.1 could not build that graph. Pinning older top-level Tauri alone also
produced a transitive build API mismatch. Adopt a tested, locked toolchain/framework
set deliberately; no production toolchain change was made for this evaluation.

The isolated Windows metadata inventory has **339 external packages**, including
optional resolved nodes and inherited egui dependencies, versus production's 185.
The coordinator library currently shares a package with eframe, so this experiment
also resolves native GUI dependencies. Extracting that framework-independent
library is a useful small refactor before a permanent second frontend.

The branch inventory records exact source/version/license and packaged notice
filenames. Tauri selects Apache-2.0; other selections are GPLv3-compatible
permissive terms and MPL-2.0, with notice and covered-source obligations. Inherited
egui font assets retain their existing separate OFL/Ubuntu font terms. Collect
missing upstream license texts before distribution. This is metadata screening,
not an advisory scan or complete redistribution audit. No proprietary audio SDK,
installer or WebView2 runtime was redistributed. Linux native dependencies require
a separate packaging/license review. See the existing repository release policy.

Before switching the default application:

1. Extract/reuse the current coordinator and MIDI ownership without changing audio
   scheduling. Extend a typed, versioned IPC contract with bounded controls.
2. Bring across selection, cancellation/retrigger, VOLUME/FILTER and temporary
   modes, pickup, device routing/cue, MIDI learn/profiles/LEDs, diagnostics and the
   M3.5 phrase timeline. The experimental UI does not yet expose those controls.
3. Add portable theme profiles, preview/reset/export, and viewport/accessibility
   tests. Preserve stable semantic selectors and both decks during customization.
4. Compare release-build native/Tauri input-to-engine and state-to-paint timing
   under controller floods, heavy themes, resizing, device failures and long sets.
   Test focus/release behavior, shutdown, Windows packaging and Linux explicitly.
5. Switch the default only after parity and those checks; keep egui as a fallback
   during the transition. There is no reason to replace the audio engine.

Framework behavior was checked against the official
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/),
[Rust command documentation](https://v2.tauri.app/develop/calling-rust/), and
[capability model](https://v2.tauri.app/security/capabilities/). Windows uses
WebView2; Linux uses WebKitGTK, so rendering and native packaging need testing on
both rather than assuming identical behavior.
