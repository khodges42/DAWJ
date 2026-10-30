# M3.Web3: portable skins and CSS customization

Tauri now provides versioned, portable skins while retaining the complete Web2
performance interface and egui fallback. The audio engine, scheduler, MIDI
runtime, Coordinator and typed performance IPC are unchanged. Rust 1.93.0 and
the tested Tauri versions remain pinned. No dependencies were added.

Run `cargo run -p dawj-tauri --locked`, then open **MIDI / settings → Skins &
accessibility**. Choose a skin to preview it live. A confirmation strip, isolated
in a closed shadow root, offers Keep and Undo; otherwise the saved appearance
returns after 15 seconds. Font scale, enhanced contrast and reduced motion can
be previewed and kept too. Preview closes configuration so the performance
surface can be inspected. All audio ownership remains outside the WebView.

## Included skins and authoring

Bundled Dark, Light and High contrast remain available. The following requested
palettes are included with their supplied colors unchanged:

| Skin | Mode | Deck A | Deck B |
| --- | --- | --- | --- |
| Psychic Genesis | dark | #FBE439 | #FF883A |
| Happyaki-Hachi | dark | #F15E38 | #F3D04D |
| No-oh-no | light | #D96237 | #66757D |
| Get more stupider | dark | #F1843E | #54B7AE |
| Georgio | dark | #6AC8B5 | #D98F81 |
| Dilla | light | #315E8F | #B44A4F |

The [authoring guide](SKIN_AUTHORING.md) describes creating/importing/exporting
packages. The [variable and selector reference](SKIN_REFERENCE.md) defines the
surface-v1 contract. Download the original, self-contained
[Soft Grid sample](../crates/tauri-app/web/skins/soft-grid.dawjskin.json), also
linked from the application. Every bundled palette is provided as portable JSON
under `crates/tauri-app/web/skins/` and can be exported from the application.

Packages contain schemaVersion 1, surfaceVersion 1, metadata with semantic
version, semantic variables, component CSS and optional base64 PNG assets.
Unknown fields/versions, bad metadata, insufficient base text contrast, unsafe
CSS, missing assets, invalid PNGs and decoding failures produce errors. Limits
are 2 MiB per package, four static PNGs of 256 KiB/1024×1024 each, 32,768 CSS
characters and 96 rules. The local library supports twelve user packages / 4 MiB
of serialized JSON, subject to the browser's storage quota.

Keep persists validated user packages and selected skin/accessibility preferences
in versioned browser storage. Imported IDs cannot overwrite bundled IDs. User
skins can be removed individually. Corrupt/missing/incompatible saved selections
fall back to bundled Dark. Automatic rendering fallback retains imported packages
for repair; explicit reset clears local user packages and legacy raw CSS.
Old unvalidated `customCSS` is no longer automatically loaded. Export/import is
the portable backup path; browser storage is not a composition library.

## Security and recovery boundary

Imported skins are JSON data only. There are no executable hooks, HTML templates,
JavaScript, native paths, plugins or unrestricted native API access. CSS uses a
bounded parser/allowlist: documented selectors and presentation/layout properties,
opaque colors, semantic variables and bounded media queries. No escapes,
`!important`, imports, font-face, animations, positioning, clipping, opacity,
generated content or paint effects outside component bounds are accepted.
Images resolve only through `asset:path.png` references to embedded, validated
static PNGs; no SVG, animation, remote URLs or directory traversal is supported.
Fonts use a documented installed-font list with generic fallback, not imported
font files. The bundled sample texture and palette data are original GPL project
assets; no third-party artwork or fonts were redistributed.

CSP remains restrictive: scripts are self-only; styles require self or Tauri's
runtime nonce, with no unsafe-inline/unsafe-eval exception. Dynamic validated
styles and the confirmation strip carry that nonce. Self-origin fetch is allowed
to read bundled sample JSON; there are no remote connect endpoints beyond the
existing local IPC transport. Images are self/data, and object/frame/media and
base/form injection paths are disabled. CSP modification protections remain on.

Product CSS protects both deck columns, card functional rows, critical colors,
minimum font/hit sizes, focus visibility and reduced motion. Component overrides
cannot shrink buttons below 28×32 px or ranges below 28×24 px. Rendering checks
detect clipping, occlusion, low contrast, hidden controls, excessive height and
horizontal overflow. Bad layouts revert. Narrow windows keep both decks side by
side with two slot columns per deck; vertical scrolling is available. Larger
fonts may also require vertical scrolling rather than shrinking hit targets.

Recovery does not depend on skin-styled controls:

- Native **Appearance → Restore default interface** / **Ctrl+Alt+Shift+R**.
- `cargo run -p dawj-tauri --locked -- --safe-mode` ignores saved selection.
- `cargo run -p dawj-tauri --locked -- --reset-skin` clears user skin state.

The native reset uses a fixed project-owned script, clears persistence, invokes
the manager synchronously, and has a clear/reload fallback if the manager is not
available. Native input-mode release uses the existing semantic control boundary;
it does not stop audio. Recovery epochs invalidate pending previews/imports so an
older asynchronous operation cannot restore a skin after reset. The native menu
is outside the WebView and cannot be hidden or restyled by package CSS.

Only UI-local startup/recovery helpers and gated smoke window/reset helpers were
added to the Tauri executable. The performance IPC contract and all engine crates
are unchanged; no asset filesystem plugin or new event bus was introduced.

## Verification

Full checks completed on Windows x64:

- Formatting, workspace check and Clippy with warnings denied.
- **132 Rust workspace tests**, including existing scheduling, offline rendering,
  MIDI and egui regressions, plus recovery launch-mode precedence.
- **31 Node tests**, including package versions/metadata, exact palettes, CSS
  policy, PNG checksums and paths, import/export roundtrip, storage reload/reset,
  quota failures, native recovery-script behavior and the existing performance UI.
- Unchanged dependency inventory: **347 external Windows-resolved packages**.
- Generated performance-contract drift check and JavaScript syntax checks.
- Native WebView2 rendering of all nine palettes at logical viewports
  **1280×820, 1366×768, 1600×900, 1920×1080 and 1100×740**, checking computed
  palette tokens/painted backgrounds, minimum sizes, visibility and reachability.
- Native 125% text / enhanced contrast, actual sample PNG decode/paint,
  persistence roundtrip, preview expiry, queued-preview recovery race, bad-layout
  fallback, and the exact native reset routine while page CSS hides the body.
- Full Tauri live-audio Web2 smoke and egui live-audio/window-capture smoke.

The preview-expiry smoke uses an accelerated timer callback; production previews
retain the 15-second lease. Smoke restores the user's pre-test skin-storage
values afterward. Rendering at narrow sizes tests reachability with vertical
scrolling, not a claim that all controls fit without scrolling at every scale.
The source authoring files and JSON payloads are tested; a human confirmation of
the operating system's download/save dialog was not performed. Actual browser
process restart persistence is implemented through the same startup loader;
tests reload storage instances rather than claiming a manually reopened session.

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo test --workspace --locked --offline
node --test crates/tauri-app/web/model.test.mjs crates/tauri-app/web/protocol.test.mjs crates/tauri-app/web/performance.test.mjs crates/tauri-app/web/skins.test.mjs
python scripts/audio_dependencies.py
cargo run -p dawj-core --example web_contract --locked --offline -- --check
cargo build -p dawj -p dawj-tauri --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke_frontends.ps1
```

Logs retain the shared runner's `target/web1-*.log` names, with skin matrix and
recovery flags in TAURI_METRICS. The existing WebView2 class-unregister warning
1412 at teardown remains a release-triage item after successful metrics/exit.
No physical MIDI/LEDs, isolated headphone routing, human usability/listening,
Linux rendering, full accessibility certification or long performance soak is
claimed. No marketplace, executable plugin system, packaging or frontend cutover
was built. Work stops at Web3; egui remains available and default.
