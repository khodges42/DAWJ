# M3.Web2: complete Tauri performance frontend

The Tauri frontend now exposes the basic live-performance surface, keyboard
operation, MIDI settings and musical timeline while egui remains the default
working fallback. Rust 1.93.0, Tauri 2.12.5 and tauri-build 2.7.4 are unchanged.
No dependency, audio-engine, DSP or scheduler changes were introduced. No skin
engine, packaging work or frontend cutover was started.

```powershell
cargo run -p dawj-tauri --locked
cargo run -p dawj --locked
```

Both executables use `dawj-core`. Different processes own independent sessions;
simultaneous device sharing depends on the OS/driver. The supplied two resident
compositions are used unchanged. See the
[five-minute walkthrough](M3_WEB2_WALKTHROUGH.md) for a reproducible performance.

## Interaction and parity

| Capability | Egui fallback | Tauri Web2 |
| --- | --- | --- |
| Two decks / eight fixed slots each | Preserved | Exposed, simultaneously visible |
| Select, launch, stop, cancel, retrigger | Preserved | Slot buttons, inspector and keyboard |
| Authoritative lifecycle and pending boundary | Preserved | Slot labels, timeline, receipts kept separate |
| Quantization, BPM, transport | Preserved | Immediate/Beat/Bar/Phrase; BPM; output start/terminal stop |
| Part gain and filtering | Preserved | Sixteen fixed contextual knobs plus selected-part controls |
| Persistent/held VOLUME and FILTER | Preserved | Mode buttons, held buttons and Shift+V/F |
| Keyboard including repeat, modifiers, release | Preserved | Full performance map and Help panel |
| Deck gains, output trim, crossfader | Preserved | Persistent smoothed engine targets |
| Cue routing and master information | Preserved | Stereo or 4-channel setup, A/B cue, diagnostic peak hold |
| Once, periodic and phrase-bar rules / fills | Preserved | Inspector, fill controls, pause/resume/remove/cancel |
| Repeating phrase timeline | Preserved | Shared Rust projection, Canvas and accessible event menu |
| MIDI discovery, connect, disconnect | Preserved | Device selection and actual connection/error status |
| MIDI mappings, learn, pickup, relative controls | Preserved | Shared mapper; structured editor and conflict feedback |
| Profile persistence | Legacy default file | Default and named profiles, save/reload/select/save-as |
| Generic Note/CC LED configuration/feedback | Preserved | Editor, port send count and explicit output errors |
| Existing appearance controls | Native styling | Existing presets/accent/CSS retained without a new system |

There is no engine pause/resume or resonance control to expose. Stop/reset closes
the session, and Start creates a fresh clock. Four-channel routing describes
logical buses, never proof of private physical headphones. Stereo cue controls
are disabled. Gain/filter and mixer values are targets; part Playing states are
engine execution; meters are held diagnostic peaks. These are labelled separately.

Both decks keep their positions across modes. Part identity is captured for knob
gestures; a mode change discards unsent continuous updates, and a gesture from an
older mode must be released before operating again. Changing mode alone sends no
parameter command. MIDI absolute pickup and relative motion remain entirely in
the shared mapper, including rearming after selection/mode/GUI target changes.

Keyboard bindings match the fallback: A/B deck, 1–8 slot, Space toggle, X stop,
Escape cancel, R retrigger, V/F persistent mode, Shift+V/F held mode, F1–F4
quantization, arrows crossfader, Q/W cue, H help. Key-up handling precedes the
editing guard. Modifier release and lost focus clear temporary modes; typing in
inputs/selects/contenteditable elements is preserved. Only arrows repeat.

## Shared architecture and contract

The web files are separated into DOM composition, keyboard state, bounded
transport, pure presentation helpers, MIDI form serialization and native smoke.
No frontend framework, bundler or runtime Node dependency was added.

Additive IPC v1 commands cover timeline zoom/follow/page and named profiles.
Atomic SelectPart and Selected actions resolve against current Coordinator focus,
so rapid keyboard selection/launch does not depend on an older web snapshot.
Incremental crossfader actions similarly use the current accepted core target.
Profiles have revision guards so a stale editor cannot overwrite a newer map.
Control revisions prevent late replies repainting older control state.

`web_timeline` invokes the existing pure `timeline::View`, marker, grouping-label,
phase and inspector logic on the retained engine publication. Its generation
matches the performance snapshot. Simultaneous markers are grouped in Rust;
all individual events remain selectable in the menu. JavaScript draws supplied
fractions and performs hit testing. It never chooses musical boundaries from
frame timestamps. Zoom/page/follow never alter the scheduler or phrase length.
Phrase length is selected before starting, matching existing engine limits.

The Rust control worker still owns assets, devices and file I/O outside the
WebView thread. Normal frontend admission caps 24 jobs, leaving headroom within
the core's 32-job bound. Knob updates keep bounded latest values and are serialized;
crossfader updates coalesce. Source release additionally reaches a bounded atomic
latch even when admission is full. Older queued held presses carry source epochs
and reject after release. The frontend also discards unsent old presses and places
a release barrier after in-flight work. No audio-thread lock or GUI operation was
added, and existing audio/receipt queue bounds remain intact.

`web_contract` generates checked-in `contract.generated.mjs` fixtures directly
from Rust serializers: command/action samples, supported modes/grids, MIDI action
choices, continuous-action metadata and the default profile. Rust and Node check
them. This reduces schema drift without adding a schema library. It is generated
validation data, not a claim of complete TypeScript type generation.

```powershell
cargo run -p dawj-core --example web_contract --locked --offline
cargo run -p dawj-core --example web_contract --locked --offline -- --check
```

Named profiles live under the fixed MIDI configuration directory. Names accept
only 1–48 ASCII letters/digits/underscore/hyphen; reserved OS names are rejected.
`default` addresses the legacy
file. Profile save and active-name persistence occur on the control worker;
restart reads that active name. Missing named files report a factory-memory
fallback explicitly. Profile configuration is validated by the existing Rust
Profile/Mapper APIs, not a JavaScript mapping engine. Learn templates may reuse
an old input; conflicts are checked when the actual learn gesture supplies its
new input. LED counters count successful port sends, not physical illumination.

## Verification

Formatting, workspace check, Clippy with warnings denied, all **131 Rust workspace
tests**, **17 Node tests**, generated-fixture checking and JavaScript syntax checks
passed. The unchanged Windows dependency inventory gate passed with 347 external
resolved packages. Existing offline, deterministic scheduling, realtime, MIDI,
pickup and egui layout regressions remain intact.

New regressions cover all sixteen contextual addresses, keyboard repeat/typing/
release, bounded transport and stale held presses, rejected replies, MIDI form
serialization, named-profile save/load/selection, stale map edits, learn conflicts,
generated contract drift, timeline cancellation/tempo/phrase projection and marker
hit testing. Tests use real core/engine behavior or explicitly synthetic projection
fixtures; they do not require physical MIDI hardware.

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
node --test crates/tauri-app/web/model.test.mjs crates/tauri-app/web/protocol.test.mjs crates/tauri-app/web/performance.test.mjs
node --check crates/tauri-app/web/app.js
node --check crates/tauri-app/web/smoke.mjs
python scripts/audio_dependencies.py
cargo build -p dawj -p dawj-tauri --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke_frontends.ps1
```

Native smoke opens actual Windows WebView2/CPAL and exercises two-deck playback,
queued cancellation, keyboard selection, persistent/held modes, focus release,
gain/filter, mixer controls, tempo, recurring fills, timeline selection, MIDI
editor conflicts/learn cancellation, independent stopping and output shutdown.
It verifies essential controls fit a 1280×820 client viewport. The MIDI editor
smoke restores the original in-memory profile and does not save user profile files.
Egui's live-audio/window-capture smoke also passes. Logs retain the existing
`target/web1-*.log` names used by the shared runner.

A completed native debug run measured 100 snapshot IPC calls at approximately
1.8 ms median, 2.7 ms p95 and 4.0 ms maximum. These are diagnostic observations, not audible-latency
or worst-case performance guarantees. All scheduling decisions still occur in
the engine. The walkthrough is supplied for manual reproduction; no five-minute
human listening/usability session, controller/LED hardware test, private cue
isolation, Linux build, long soak or device-loss recovery validation is claimed.
WebView2's previously observed teardown class-unregister warning 1412 remains a
release-triage item after successful smoke metrics/process exit. Existing CSS
overrides can affect visibility; no new theme recovery or skin system was added.

GPL-3.0-or-later and the existing dependency/license policy are preserved. Egui
remains enabled and default. Work stops at Web2; themes, packaging and cutover
remain outside this milestone.
