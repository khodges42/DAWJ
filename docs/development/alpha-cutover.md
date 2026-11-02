# Default development frontend

**Decision:** Tauri is DAWJ's primary development/alpha application. From the
workspace root, `cargo run --release --locked` launches `dawj` (Tauri).
`cargo run --release -p dawj-egui --locked` launches the maintained fallback.
This is an alpha cutover, not live-show certification or binary-release clearance.

## Review

| Area | Evidence and assessment |
| --- | --- |
| Performance surface | Both visible eight-slot decks, quantized launch/stop/cancel/retrigger, BPM, modes, gain/filter, timeline, periodic rules and fills are connected. |
| Control and IPC | Both use `dawj-core`. Typed v1 fixtures, stale replies, bounded worker/frontend admission, latest targets and release epochs are tested. No second engine or scheduler. |
| State truth | Lifecycle comes from coherent engine publication; accepted targets, pending receipts and diagnostic peaks remain distinct. |
| Audio isolation | GUI, asset/device preparation, profile I/O and MIDI ownership stay outside the realtime callback. No DSP, clock or scheduling behavior changed for cutover. |
| Keyboard and MIDI | Shared actions, learn/profiles/pickup, relative encoders, feedback, key-up/text-entry/focus-loss paths are present. Physical controller certification remains open. |
| Mixer and cue | Engine smoothing and existing routing are retained; stereo explicitly has no isolated headphone cue. Four-channel hardware isolation remains unverified. |
| Skins | Versioned packages, persistence, preview expiry, restrictive CSS/CSP and a native reset path are present. Default-app smoke exercises recovery and safe startup. |
| Windows lifecycle | Both executable paths build; functional smoke covers live demo output, independent stop, software restart and clean exit. Busy OS driver calls can still stall their owner. |
| Licensing | GPL-3.0-or-later application code and the unchanged 347-package Windows inventory pass screening. Font/native notices, corresponding source and supplied-design rights still gate redistribution. |
| Performance risk | Controlled release trials did not reproduce the earlier 206 ms command tail; memory and rare smaller tails are not claimed solved. |

No credible critical **alpha** blocker was found in the reviewed paths. The
[controlled study](../M3_WEB4_1_LATENCY.md) measured Tauri command p95 3.5-4.0 ms,
worst 6.4-11.2 ms across three trials. Its 20-minute run saw two roughly 26 ms
commands, a roughly 302 MiB late memory median and a positive fitted slope.
Host load and Evergreen runtime patches are confounds; the old outlier is not
treated as a proven architectural defect or dismissed as impossible.

## Names and retained data

| Before | Now |
| --- | --- |
| `dawj-tauri` package/binary | `dawj` / `dawj.exe`, workspace default |
| `dawj` egui package/binary | `dawj-egui` / `dawj-egui.exe`, fallback |
| Egui Rust library import `dawj` | `dawj_egui`, compatibility re-export of shared core |

Rebuild both applications. Old executables in an existing `target/` directory
are not renamed or trusted; staging scripts copy only the new canonical names.
Historical measured JSON retains its original labels; do not reinterpret its
old `dawj` filename as Tauri without checking the recorded frontend field.

The Tauri identifier **`org.dawj.performance`**, main window identity, local origin,
storage keys and shared MIDI profile locations are unchanged. Renaming the
application/package does not intentionally move/reset user skins or profiles.
Native **Appearance > Restore default interface**, **Ctrl+Alt+Shift+R**,
`dawj --safe-mode` and `dawj --reset-skin` remain available.

## Verification and release procedure

On Windows x64, formatting, workspace check, Clippy with warnings denied, all
**138 Rust tests**, **33 Node tests**, four Python regression tests, generated
IPC fixtures and the **347-package** licensing inventory passed. Cargo metadata
asserts the default package/bin mapping and unchanged persistence identity.
The end-to-end Windows procedure successfully ran the **staged release binaries**:
primary Tauri, egui fallback and primary `--safe-mode`, using HyperX without
fallback. Both decks, software restart, native skin reset, viewports/zoom and
clean shutdown passed. Safe startup was checked before the skin test reset.
The registered native reset routine was exercised; no physical shortcut, MIDI
controller or headphone-isolation certification is implied.
The documented unqualified Cargo launch and explicit `dawj-egui` Cargo launch
also passed their native smoke modes. A fresh tracked-file snapshot passed
workspace checking, default-target assertions, documentation links and the
licensing check without depending on ignored notes.

The [Windows alpha procedure](windows-alpha.md) builds and checks both frontends,
records hashes/provenance and optionally tests the staged release executables.
It does not publish, sign, create an installer or declare redistribution cleared.
See the [development guide](README.md) for all software checks and
[performance validation](performance-validation.md) for hardware gates.

Before live shows: validate physical MIDI/LED and audio reconnect, driver-busy
shutdown, cue isolation/listening, true input-to-audio/state-to-pixel latency,
mixed-DPI focus handling, lower-power Windows systems and longer memory runs.
Linux native behavior remains unverified. Egui stays available during this work.
