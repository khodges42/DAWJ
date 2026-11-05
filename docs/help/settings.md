# Audio, appearance and recovery

## Audio

Open **Tools > Audio settings**. DAWJ remembers the configured output by name.
On first launch it uses the system default. Missing or ambiguous configured names
produce an error instead of silently choosing another device. Rescan, select the
intended endpoint and retry. Opening output never launches parts.

Device/routing changes require closing the output stream in Settings. This is
separate from musical Pause/Stop. A new stream begins with stopped parts and a
fresh clock. Stereo is master-only; four-channel routing uses master 1–2 and
cue 3–4. The existing engine uses prepared 48 kHz audio.

## Appearance

Tools contains skins, font scaling, contrast and reduced-motion preferences.
Overview style and workspace remain independent of the selected skin.

**Restore default interface** in Settings and **Ctrl+Alt+Shift+R** provide recovery.
Launching with `--safe-mode` ignores saved skin selection. `--reset-skin` clears
local skin data. Recovery does not delete compositions or audio files.
The configured audio endpoint is retained through skin recovery and safe mode.

## Sessions

File contains New, Open, Save and Save As. New/Open asks before discarding an
unsaved session. Closing the Session dialog leaves the session intact; unsaved
form fields require a discard decision. Save captures accepted content and mixer
settings, not automatic transport resume, queued execution or held input modes.
