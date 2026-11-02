# Composition package format: design status

The [manifest schema](track.schema.json) is a Draft 2020-12 JSON Schema for an
exploratory version 0.1.0 package. It describes metadata, tempo/phrase information,
asset references and eight performable parts. It is not the runtime engine format.

The [complete illustrative manifest](example.duettrack.manifest.json) includes
sample, MIDI-pattern and built-in-synth proposals. Referenced media are absent and
checksums are placeholders. It is **not a playable/importable demo**. Current demos
are generated and prepared by `audio-backend::demo` when output starts.

No composition ZIP importer, library browser, synthesizer, ordinary-song player
or general preparation editor is implemented. Schema fields describing those
features are proposals; do not infer support from a schema-valid manifest.

## Importer requirements

JSON Schema checks shape and bounds, not semantic or resource safety. A future
importer must also validate unique part/asset identities, reference types, hashes,
paths and traversal, actual media formats/rates/durations, tempo ordering, feature
compatibility, decoded resource limits, rights/attribution and runtime CPU budget.
All loading/decoding must happen outside the realtime callback. A validated package
must not become READY before those checks succeed.

The original specification discusses container alternatives and preparation
contracts; see [the preserved design](product-specification.md). The complete
schema/example are retained here; redundant truncated copies from earlier paths
were archived locally with the development notes, without inventing missing data.
