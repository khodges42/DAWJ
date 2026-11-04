# Local sample packs

Place each pack in its own child directory here, for example:

```text
resources/samples/my-pack/Drums/Kick_C.wav
resources/samples/my-pack/Loops/Perc_Loop_120_Cm.wav
```

DAWJ discovers child directories without a configured pack-name list. Nested
folders become categories. WAV sounds can be browsed, searched, auditioned and
assigned in Simple / Browse or Sketch. Filename note/key/BPM fields are optional
hints; DAWJ does not analyze or time-stretch these sounds.

An optional `pack.json` supplies version `1`, a stable `id`, `name`, and optional
`license`, `creator` and `provenance`. Missing distribution information is reported
in the browser. Use the actual asset license and provenance; the application's
GPL license does not grant permission to redistribute third-party recordings.

Local WAVs are Git-ignored. This tracked directory guide keeps resource discovery
and builds usable in a fresh clone without requiring anyone's private samples.
External WAV import and the explicitly selected performance demo also work when
no factory pack is installed. Native factory-content smoke checks require locally
supplied assets and never silently substitute the demo.

See [local compositions and patterns](../../docs/user-guide/local-sessions.md).
