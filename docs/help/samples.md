# Sounds, trimming and sample credits

The browser discovers sample packs recursively under `resources/samples/`.
Use pack, category, search, type, pitch/key and BPM filters. Filename values are
hints: a bare C has no assumed octave, and a loop's harmonic key is different
from an instrument sample's root pitch.

**Listen** previews the source through the master. **Stop listen** ends preview.
**Assign** or dragging a browser sound to a part prepares the audio outside the
audio callback. If that destination is active, stop it and retry. Other parts
can continue. Preparation is not yet a committed assignment; watch its status.

## Source and license

Select a sound and choose **About / Source / License**. This shows its stable
sample and pack identities, original path, documented creator, applicable license,
per-sample overrides and original provenance text. Unknown means the fact is not
documented, not that attribution or permission is unnecessary.

Display names and categories are organizational metadata. They never determine
ownership. The original `SOURCE.md`, licenses and renaming maps remain preserved
with each pack. DAWJ's GPL license does not relicense third-party sounds.

## Nondestructive source region

In Sketch, drag the source waveform's start/end handles to select the playable
region. Loop handles select a loop inside that region. Numerical frame controls
remain available for precision. Apply changes while that part is stopped.

These edits retain the original file. Prepared frames use 48 kHz, even when the
original file has another rate. Duration and current-tempo beats help compare
the region with musical time. A loop that does not fit whole bars keeps its
natural duration; DAWJ does not silently stretch or transpose it.

Some floating-point WAVs contain finite peaks above full scale. During preparation,
DAWJ attenuates that entire source just enough to fit its bounded playback range,
preserving dynamics rather than clipping. The source-region readout reports this
headroom adjustment; previews show the prepared audio. The original file stays
unchanged. Nonfinite samples and truncated audio remain errors.
