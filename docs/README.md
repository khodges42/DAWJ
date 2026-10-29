# DUET live composition design bundle

**Start with:** `PRODUCT_AND_TECHNICAL_DESIGN.md` (v0.1 design and technical architecture).

**Companion artifacts:**
- `track.schema.json`: Draft 2020-12 JSON Schema for a bounded track manifest.
- `example.duettrack.manifest.json`: Schema-valid *illustrative* track with eight parts and one synth patch. **The audio/MIDI assets are intentionally not included, and the checksums are placeholders. This example is not a playable importable package.**

Important: JSON Schema validates basic shape, bounds and syntax, **not** reference identity, unique part slots, referenced asset presence, actual SHA-256 matches, whether a MIDI variant points to a synth part, tempo range ordering, duration matching, DSP CPU budget, legal rights, or peak safety. A production importer requires these additional semantic/resource checks before marking a package READY.

Research date: 2026-10-28. License and product details should be rechecked before commercialization.
