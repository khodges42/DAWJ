# Factory library metadata

Factory audio lives under `resources/samples/<collection>/`, with arbitrary nested
directories. Discovery reads WAV headers and metadata on a background worker;
audition and assignment retain their independent preparation paths.

```powershell
python scripts/sample_manifests.py --update
python scripts/sample_manifests.py --check
python scripts/test_sample_manifests.py
```

`--update` adds missing facts/identities to versioned `pack.json` manifests. Existing
values, including explicit nulls and human corrections, win. It never edits audio,
SOURCE.md, original READMEs, license texts or renaming maps. Changed source/audio
hashes require review instead of silently replacing documented facts. `--check`
validates coverage, IDs, source links, hashes and the aggregated attribution notice.

## Identity and provenance

Manifest version 2 extends the existing version-1 collection manifest:

- `id` is the persistent collection/pack identity; `name` is only a display title.
- `sources` maps original documentation paths to stable source IDs and extracted
  title, description, creator/contributors, source/license URLs, license,
  copyright, attribution, redistribution and modification/preservation notes.
- `samples` maps current collection-relative filenames to stable sample IDs,
  originating source record, recorded original path/category, content hash,
  sample-specific metadata and optional creator/license/attribution overrides.
- Existing `license`, `creator`, `provenance` and `extra` remain supported. Version
  1 manifests and manifest-free local packs remain discoverable.

Sample override, originating SOURCE record, then collection metadata determine
applicable provenance. Null means unspecified/inherit where applicable; the UI
marks undocumented facts Unknown. Different licenses remain different strings;
extraction does not relicense material under GPL or CC0. The current collection's
owner dedication remains in its existing LICENSE.md.

New IDs are deterministic initially and preserved thereafter. A uniquely
identifiable file relocation reuses its recorded ID by SHA-256. Ambiguous duplicate
content is not guessed. Original CSV mappings supply published filenames where
provided. The original SOURCE.md remains the human-readable record, identified
by path and hash; the browser can show its preserved text offline.

Factory references retain `pack` and `path` and optionally include `sample_id`.
New assignment attaches the catalog's ID. Resolution prefers stable ID while
legacy pack/path references continue to work. Session display-name/type/pitch
corrections remain separate from provenance. Neither virtual categories nor
future collections determine a sample's creator or license.

The four supplied collections contain 853 WAVs and 50 SOURCE records. Explicit
attribution-requirement fields are undocumented in those 50 records and remain
null; supplied licensing and redistribution text is preserved. Use the original
documents and [aggregated notice](../../resources/samples/ATTRIBUTIONS.md) rather
than interpreting a null field as a waiver.

## Distribution

Tauri's resource mapping retains `resources/samples/` recursively, including
manifests, original SOURCE.md, licenses, READMEs, original-name maps and auxiliary
metadata. The portable Windows alpha staging script copies the same directory
beside its executables. Audio is not re-encoded. Canonical help is embedded from
`docs/help/` and also retained in the local alpha documentation directory.
Git attributes disable newline conversion inside factory collections so original
provenance bytes and manifest hashes survive checkout on Windows and Linux.

Factory source identity is separate from Jam4's rebuildable waveform content
identity. The latter can be regenerated without changing ownership or session
references. No portable DJ library format or asset-management service is added.
