// SPDX-License-Identifier: GPL-3.0-or-later
//! Generic MIDI normalization, mappings and state feedback. No audio callback access.
pub mod device;
pub mod profile;
pub mod runtime;
use crate::{Coordinator, InputSource, Intent, Mode, PartAction};
use audio_backend::{
    live::{Quantize, SlotState},
    performance_state::TransportState,
};
use profile::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub control: Control,
    pub value: u8,
}
pub fn normalize(bytes: &[u8]) -> Option<Event> {
    if bytes.len() != 3 || bytes[1] > 127 || bytes[2] > 127 {
        return None;
    }
    let kind = match bytes[0] & 0xF0 {
        0x80 | 0x90 => Kind::Note,
        0xB0 => Kind::CC,
        _ => return None,
    };
    Some(Event {
        control: Control {
            channel: bytes[0] & 15,
            kind,
            number: bytes[1],
        },
        value: if bytes[0] & 0xF0 == 0x80 { 0 } else { bytes[2] },
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parameter {
    Gain(usize, usize),
    Cutoff(usize, usize),
    DeckGain(usize),
    Crossfader,
}
impl Parameter {
    fn for_action(action: Action, co: &Coordinator) -> Option<Self> {
        Some(match action {
            Action::Gain(t) => {
                let (d, s) = t.address(co);
                Self::Gain(d, s)
            }
            Action::Cutoff(t) => {
                let (d, s) = t.address(co);
                Self::Cutoff(d, s)
            }
            Action::Context(t) => {
                let (d, s) = t.address(co);
                if co.focus.mode == Mode::Volume {
                    Self::Gain(d, s)
                } else {
                    Self::Cutoff(d, s)
                }
            }
            Action::DeckGain(d) => Self::DeckGain(d),
            Action::Crossfader => Self::Crossfader,
            _ => return None,
        })
    }
    fn value(self, co: &Coordinator) -> Option<f32> {
        Some(match self {
            Self::Gain(d, s) => co.parameter_target(d, s)?.gain,
            Self::Cutoff(d, s) => co
                .parameter_target(d, s)?
                .cutoff_hz
                .map_or(1.0, |hz| (hz / 20.0).ln() / 1000.0_f32.ln()),
            Self::DeckGain(0) => co.parameters.gain_a,
            Self::DeckGain(_) => co.parameters.gain_b,
            Self::Crossfader => co.parameters.crossfader,
        })
    }
    fn intent(self, v: f32) -> Intent {
        match self {
            Self::Gain(deck, slot) => Intent::Part {
                deck,
                slot,
                action: PartAction::Gain(v),
            },
            Self::Cutoff(deck, slot) => Intent::Part {
                deck,
                slot,
                action: PartAction::Cutoff(if v >= 0.9999 {
                    None
                } else {
                    Some(20.0 * 1000.0_f32.powf(v))
                }),
            },
            Self::DeckGain(d) => Intent::DeckGain(d, v),
            Self::Crossfader => Intent::Crossfader(v),
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Pickup {
    key: Option<Parameter>,
    previous: Option<f32>,
    latched: bool,
    pressed: bool,
    expected: Option<f32>,
}
pub struct Learn {
    pub mapping: Mapping,
    pub replace: Option<usize>,
}
pub struct Mapper {
    pub profile: Profile,
    runtime: [Pickup; MAX_MAPPINGS],
    pub learn: Option<Learn>,
    pub status: String,
    pub dirty: bool,
    pub revision: u64,
    led_cache: [Option<u8>; MAX_LEDS],
}
impl Mapper {
    pub fn new(profile: Profile) -> Result<Self, String> {
        profile.validate()?;
        Ok(Self {
            profile,
            runtime: [Pickup::default(); MAX_MAPPINGS],
            learn: None,
            status: "No MIDI events received".into(),
            dirty: false,
            revision: 0,
            led_cache: [None; MAX_LEDS],
        })
    }
    pub fn reset(&mut self, co: Option<&mut Coordinator>) {
        if let Some(co) = co {
            co.discard_midi_intent();
        }
        self.runtime = [Pickup::default(); MAX_MAPPINGS];
        self.led_cache = [None; MAX_LEDS];
    }
    pub fn refresh_feedback(&mut self) {
        self.led_cache = [None; MAX_LEDS];
    }
    pub fn replace_profile(
        &mut self,
        profile: Profile,
        co: Option<&mut Coordinator>,
    ) -> Result<(), String> {
        profile.validate()?;
        self.reset(co);
        self.profile = profile;
        self.revision = self.revision.wrapping_add(1);
        self.learn = None;
        self.dirty = false;
        Ok(())
    }
    pub fn install(
        &mut self,
        mapping: Mapping,
        replace: Option<usize>,
        co: Option<&mut Coordinator>,
    ) -> Result<(), String> {
        let mut profile = self.profile.clone();
        if let Some(i) = replace {
            *profile
                .mappings
                .get_mut(i)
                .ok_or("mapping index unavailable")? = mapping;
        } else {
            profile.mappings.push(mapping);
        }
        self.replace_profile(profile, co)?;
        self.dirty = true;
        Ok(())
    }
    pub fn handle(&mut self, event: Event, co: Option<&mut Coordinator>) {
        if !event.control.valid() || event.value > 127 {
            self.status = "Invalid MIDI event rejected".into();
            return;
        }
        if let Some(learn) = &self.learn {
            if event.control.kind == Kind::Note && event.value == 0 {
                return;
            }
            let mut mapping = learn.mapping.clone();
            mapping.input = event.control;
            let replace = learn.replace;
            let button = mapping.behavior == Behavior::Button;
            match self.install(mapping, replace, co) {
                Ok(()) => {
                    if button
                        && let Some(i) = self
                            .profile
                            .mappings
                            .iter()
                            .position(|m| m.input == event.control)
                    {
                        self.runtime[i].pressed = true;
                    }
                    self.status = format!("Learned {:?}; save profile to persist", event.control);
                }
                Err(e) => self.status = e,
            }
            return;
        }
        let Some(i) = self
            .profile
            .mappings
            .iter()
            .position(|m| m.input == event.control)
        else {
            return;
        };
        let Some(co) = co else {
            self.status = "MIDI received; start audio output to perform".into();
            return;
        };
        let m = &self.profile.mappings[i];
        let action = m.action;
        let result = if m.behavior == Behavior::Button {
            let pressed = if event.control.kind == Kind::Note {
                event.value > 0
            } else {
                event.value >= 64
            };
            if self.runtime[i].pressed == pressed {
                return;
            }
            self.runtime[i].pressed = pressed;
            let intent = match action {
                Action::Momentary(mode) => Intent::HoldMode {
                    token: i as u16,
                    mode,
                    pressed,
                },
                _ if !pressed => return,
                Action::SelectDeck(d) => Intent::Deck(d),
                Action::SelectSlot(s) => Intent::Slot(s),
                Action::SelectPart { deck, slot } => {
                    if let Err(e) = co.dispatch(Intent::Deck(deck), InputSource::Midi) {
                        self.status = e.into();
                        return;
                    }
                    Intent::Slot(slot)
                }
                Action::Mode(mode) => Intent::Mode(mode),
                Action::Quantize(q) => Intent::Quantize(match q {
                    Grid::Immediate => Quantize::Immediate,
                    Grid::Beat => Quantize::Beat,
                    Grid::Bar => Quantize::Bar,
                    Grid::Phrase => Quantize::Phrase,
                }),
                Action::Cue(d) => Intent::Cue(d),
                Action::Launch(t)
                | Action::Stop(t)
                | Action::Cancel(t)
                | Action::Retrigger(t)
                | Action::Toggle(t) => {
                    let (deck, slot) = t.address(co);
                    Intent::Part {
                        deck,
                        slot,
                        action: match action {
                            Action::Launch(_) => PartAction::Launch,
                            Action::Stop(_) => PartAction::Stop,
                            Action::Cancel(_) => PartAction::Cancel,
                            Action::Retrigger(_) => PartAction::Retrigger,
                            _ => PartAction::Toggle,
                        },
                    }
                }
                _ => return,
            };
            co.dispatch(intent, InputSource::Midi)
        } else {
            let Some(key) = Parameter::for_action(action, co) else {
                return;
            };
            let Some(target) = key.value(co) else {
                return;
            };
            let state = &mut self.runtime[i];
            if state.key != Some(key)
                || state
                    .expected
                    .is_some_and(|own| (target - own).abs() > 2.0 / 127.0)
            {
                state.key = Some(key);
                state.latched = false;
                state.previous = None;
                state.expected = None;
            }
            let value = if m.behavior == Behavior::RelativeTwosComplement {
                let delta = if event.value < 64 {
                    i16::from(event.value)
                } else {
                    i16::from(event.value) - 128
                };
                (target + delta as f32 * m.step * if m.invert { -1.0 } else { 1.0 }).clamp(0.0, 1.0)
            } else {
                let mut v = ((f32::from(event.value) - f32::from(m.min))
                    / f32::from(m.max - m.min))
                .clamp(0.0, 1.0);
                if m.invert {
                    v = 1.0 - v;
                }
                state.latched |= (v - target).abs() <= 2.0 / 127.0
                    || state
                        .previous
                        .is_some_and(|p| (p - target) * (v - target) <= 0.0);
                state.previous = Some(v);
                if !state.latched {
                    self.status = format!(
                        "Pickup waiting: {:?}, move knob through {:.0}%",
                        event.control,
                        target * 100.0
                    );
                    return;
                }
                v
            };
            let result = co.dispatch(key.intent(value), InputSource::Midi);
            if result.is_ok() {
                state.expected = Some(value);
            } else {
                state.latched = false;
                state.previous = None;
            }
            result
        };
        self.status = match result {
            Ok(()) => format!(
                "MIDI {:?} dispatched; engine state remains authoritative",
                event.control
            ),
            Err(e) => format!("MIDI rejected: {e}"),
        };
    }
    /// Latest desired LED values only. Call mark_sent after queue admission so a
    /// full MIDI output queue never loses a state update. No meter-driven traffic.
    pub fn feedback(&self, co: Option<&Coordinator>) -> Vec<(usize, [u8; 3])> {
        let mut changed = Vec::with_capacity(MAX_LEDS);
        let v = self.profile.values;
        for (i, l) in self.profile.leds.iter().enumerate() {
            let value = co
                .filter(|co| co.available())
                .and_then(|co| co.snapshot)
                .filter(|s| !matches!(s.transport, TransportState::Closed | TransportState::Failed))
                .map_or(v.stopped, |s| {
                    match s.decks[l.deck].slots[l.slot].lifecycle {
                        SlotState::Stopped => v.stopped,
                        SlotState::QueuedStart => v.queued,
                        SlotState::Playing => v.playing,
                        SlotState::QueuedStop => v.stopping,
                        SlotState::QueuedReplace => v.replacing,
                    }
                });
            if self.led_cache[i] != Some(value) {
                changed.push((i, l.output.message(value)));
            }
        }
        changed
    }
    pub fn mark_sent(&mut self, updates: &[(usize, [u8; 3])]) {
        for &(i, m) in updates {
            if i < MAX_LEDS {
                self.led_cache[i] = Some(m[2]);
            }
        }
    }
}
