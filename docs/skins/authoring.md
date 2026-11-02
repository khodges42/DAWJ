# Make a DAWJ skin

Open Tauri with `cargo run -p dawj --locked`, then **MIDI / settings → Skins
& accessibility**. Export a bundled skin, edit its JSON in a text editor, and
import it. Selection previews live for 15 seconds; Keep saves it, Undo or expiry
restores the saved selection. Changing a skin never schedules musical actions.
Font scale, enhanced contrast and reduced-motion settings are saved with Keep.

Start with the downloadable [Soft Grid sample](../../crates/tauri-app/web/skins/soft-grid.dawjskin.json).
The application also provides this download in the skin panel. It contains an
original 2×2 PNG texture, its metadata and a few permitted CSS rules in one file.
Export uses the WebView's download mechanism and suggests `<id>.dawjskin.json`.
Keep copies outside browser storage: reset clears the local user library.

## Portable package v1

The format is ordinary UTF-8 JSON, not a zip, HTML file or executable module:

```json
{
  "schemaVersion": 1,
  "surfaceVersion": 1,
  "metadata": {
    "id": "my-performance-skin",
    "name": "My Performance Skin",
    "version": "1.0.0",
    "author": "Your name",
    "license": "GPL-3.0-or-later"
  },
  "mode": "dark",
  "variables": {
    "background": "#171A20",
    "surface": "#242A34",
    "raised": "#313B49",
    "text": "#EDF2F9",
    "deckA": "#73C5DF",
    "deckB": "#E2B67E"
  },
  "css": ".deck { border-radius: 8px; } .actions button { border-radius: 2px; }",
  "assets": []
}
```

Both version numbers must be 1. Metadata version uses `major.minor.patch`, with
an optional lowercase prerelease suffix. IDs use 1–48 lowercase ASCII letters,
digits and hyphens. Names are limited to 80 characters. Optional author/license
fields are limited to 120 characters and description to 512. Unknown fields
reject. `mode` is dark, light or high-contrast. Missing variables use documented
dark defaults, so start by exporting Light when making a light skin.

Use the [variable and selector reference](reference.md). Color variables are
opaque six-digit hex. Text must contrast at least 4.5:1 with background, surface
and raised. Critical control text/backgrounds use these semantic tokens and
cannot be made transparent through component CSS. Focus and disabled states
remain visible, and minimum hit sizes cannot be reduced.

## Component CSS

CSS changes documented component presentation and layout. For example:

```css
.deck { border-radius: 8px; padding: 8px; }
.actions button { border-radius: 2px; font-weight: 600; }
.deck button:focus-visible { outline-color: var(--color-focus); }
@media (min-width: 1600px) { .slots { gap: 10px; } }
```

Selectors are a restricted, documented subset. No `!important`, CSS escapes,
imports, font-face, keyframes, positioning, transforms, opacity, clipping,
generated content, arbitrary custom-property definitions or executable hooks.
Global body/state rules may change permitted paint properties only. Display
changes are restricted to layout containers; both decks stay side by side and
each card retains its four functional rows. Some critical foreground/background,
font and size properties are protected: use package variables for those colors.

Only one level of bounded min/max width/height media queries is supported, with
pixel thresholds 500–2599. There is no arbitrary component scripting API. A
layout that clips, covers, shrinks or pushes essential controls horizontally
off-screen is rejected or reverted to a bundled skin. Responsive narrow layouts
and larger text may require vertical scrolling; controls remain reachable.

## Optional local assets

Version 1 permits static PNG only, embedded in JSON:

```json
{"path":"images/tile.png","mime":"image/png","data":"BASE64_PNG_BYTES"}
```

Reference it as `background-image: url("asset:images/tile.png")`. This symbolic
reference is resolved inside the package, never against the native filesystem.
Images can decorate panels; essential controls and state labels retain protected,
image-free backgrounds so a texture cannot obscure their text.
There are no remote images, SVG, animation, font files or absolute/traversal
paths. Missing references, invalid PNG structure/checksums and decoding failures
produce errors. Paths use letters, digits, underscores, hyphens, `/` and a `.png`
extension. To encode your own file, for example:

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes('tile.png'))
```

Limits are 2 MiB per package, four assets of at most 256 KiB each, and at most
1024×1024 pixels per image. CSS is limited to 32,768 characters and 96 rules.
The local library holds at most twelve user skins and 4 MiB of JSON; browser
storage quota may be lower. Save failures are explicit. Bundled skins cannot be
removed. Imported IDs are namespaced separately from bundled IDs.

## Recovery

Use the **native Appearance → Restore default interface** menu or
**Ctrl+Alt+Shift+R**, even if web controls are hidden. It resets skin/accessibility
preferences, deletes local user skin copies and clears old raw CSS overrides.
Audio ownership remains with the existing worker; temporary input modes release.

You can also launch with:

```powershell
cargo run -p dawj --locked -- --safe-mode
cargo run -p dawj --locked -- --reset-skin
```

Safe mode ignores the saved skin for that launch without deleting the library.
Reset clears it before applying the bundled default. These paths are outside
user CSS. Automatic layout fallback retains imported packages so they can be
repaired; explicit reset is the operation that clears them.
