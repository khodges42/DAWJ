# Sample naming

All 116 WAV samples are in `drums/`, named like `BD01_808_Kick.wav`.
The two-digit number is a sequential variant index within each instrument,
starting at 01, ordered by increasing original knob position (first knob,
then second). It is not a knob setting or velocity.

`sample-map.csv` records every original path, new path, original knob
settings in order, and SHA-256 checksum. Audio contents are unchanged.
The JSON files in `_soundmeta/` reference the new paths.

## Original knob settings

The original author's README.md is preserved. Its codes mean:
00 = 0.0, 25 = 2.5, 50 = 5.0, 75 = 7.5, and 10 = 10.0 (maximum, not 1.0).
Two-knob filenames put tone/tuning before decay/snappy. The author calls
the bass drum's first knob tuning. Snare uses tone then snappy; cymbal
uses tone then decay. Toms and congas use tuning; open hat uses decay.
Level was always at maximum and is not encoded.
For example, BD2575.WAV has first knob 2.5 and decay 7.5.

CL means Claves; LC means Low Conga. Instrument prefixes retain the
original author's abbreviations. Settings are kept in the CSV to keep
sample filenames short.
