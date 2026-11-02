# Vision and roadmap

DAWJ treats a composition as an instrument: a producer prepares musical parts,
and a performer decides when they enter, how they sound and how they transition
into another composition. Both decks remain visible so the incoming arrangement
can be understood while the outgoing one plays.

The longer-term direction is to mix these live compositions alongside conventional
DJ tracks in one application. Conventional track playback is **planned**, not
implemented. DAWJ currently performs the two supplied demo compositions.

The interaction is intentionally smaller than a DAW: select a part, launch or stop
it at a musical boundary, shape its volume/filter, cue and mix. The repeating
timeline explains current and upcoming activity; it is not an arrangement editor.

## Available now

- Two decks with eight independently controlled parts each.
- Resident sample playback, loops, one-shots and quantized launch/stop/retrigger.
- A shared musical clock, BPM changes, periodic triggers and fills.
- Part gain/filter, deck gain, crossfader, master diagnostics and logical cue buses.
- Tauri and egui frontends sharing Rust control logic; keyboard and generic MIDI
  mappings, learn, profiles, pickup and state-driven Note/CC feedback.
- Portable CSS skins, previews, accessibility preferences and native reset in Tauri.
- Offline timing tests and opt-in Windows release validation scripts.

## Planned and unverified

Prepared composition import and authoring, safe incoming-deck loading, conventional
DJ-track playback, a library/preparation workflow and recording remain future work.
The package schema is a design fixture, not a working importer. Beat matching,
time stretching, key matching and arbitrary plugin hosting are not implemented.

Windows is the development target. Linux hardware/native frontend testing and
distribution packaging remain incomplete. Live-show readiness requires physical
controller, device-reconnection, latency, listening and headphone-isolation tests.
Software meters or successful unit tests do not establish these properties.

## Design sources

The [original product specification](product-specification.md) and
[prototype plan](prototype-plan.md) are preserved design inputs. Their codename,
proposals, dated external research and milestone checklists are historical context,
not claims that every described capability exists. Current behavior is documented
in the user guides and contributor architecture. Changes to scope should update
this roadmap rather than silently treating exploratory proposals as decisions.
