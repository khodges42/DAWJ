# Licensing and distribution review

Original DAWJ application code is **GPL-3.0-or-later**, declared in Cargo metadata
and SPDX headers. The complete GPL v3 text is in [LICENSE](../../LICENSE).
[NOTICE](../../NOTICE) preserves application and supplied-design provenance.

The exact Windows x64 dependency review is
[windows-dependencies.json](windows-dependencies.json). Run
`python scripts/audio_dependencies.py` to compare its name/version/source/license
tuples with locked Windows-filtered Cargo metadata. It currently inventories 348
external packages, including optional resolved nodes; this is not a list of only
code linked into every executable.

## Software and native components

The reviewed graph includes CPAL, eframe/egui, midir, serde/serde_json and Tauri.
Native local file selection uses rfd 0.15.4 under MIT; retain its upstream LICENSE.
Its Windows dependencies were already in the reviewed graph. Linux GTK system
components still require platform packaging review.
Exact versions and selected alternatives belong in the inventory and lockfile,
not a manually duplicated milestone list. Apache-2.0 is selected for dual
MIT/Apache packages where recorded. Other reviewed permissive terms include MIT,
ISC, Zlib, CC0-1.0, BSL-1.0 and Unicode-3.0; preserve combined AND terms.

Scoped MPL-2.0 screening exceptions cover cssparser 0.37.0, cssparser-macros 0.7.1,
dtoa-short 0.3.5, option-ext 0.2.0 and selectors 0.38.0. Retain covered source and
notices. No third-party source has been modified. License-screening configuration
is not proof of every release obligation or a vulnerability audit.

WebView2 is a Windows runtime prerequisite, not redistributed here. Proprietary
MIDI SDKs, vendor firmware and a WebView installer are not bundled. Factory sample
collections retain their own supplied licenses and provenance, independently of
the application GPL. See [factory library metadata](factory-library.md), original
SOURCE.md/README/license files and the aggregated sample attribution notice.
Linux native/system component packaging requires its own review.

## Fonts and creative assets

Egui's unmodified default-font package declares
`(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`. Select Apache-2.0 for the
Rust wrapper and retain fonts' original terms/names. Distribution needs
`fonts/OFL.txt`, `fonts/UFL.txt`, `fonts/Hack-Regular.txt` and
`fonts/emoji-icon-font-mit-license.txt`, including copyright notices. Hack's
notice includes Bitstream Vera ancestry. Font exceptions are scoped to
`epaint_default_fonts =0.31.1`, not arbitrary application source. Modifying or
substituting fonts needs a separate asset/rename review.

The supplied original design documents, schema and illustrative manifest came
without a separate redistribution grant. Preserve their provenance; do not claim
they are relicensed by the application GPL notice. Resolve redistribution rights
or exclude those inputs from release artifacts. Referenced example media are
absent and hashes are illustrative. Existing procedural demos and the original
Soft Grid skin texture do not introduce third-party sample libraries.

## Before distribution or new dependencies

Review exact version, source, SPDX expression/selected alternative, bundled
assets, native code and transitive obligations before adding a dependency.
Unknown, proprietary/source-available-only or conflicting terms cannot silently
pass the gate. Conditional licenses need a documented distribution plan.

Gather corresponding source/build instructions, upstream license texts/NOTICE
files and creative-asset permissions. Empty inventory `license_files` entries
mean upstream texts still need collecting from the exact revision; they are not
waivers. Include Rust/compiler-runtime notices as applicable. Use advisory tools
when available, and record their actual results separately from metadata review.
No signed/package release or legal certification is implied by passing the tests.
