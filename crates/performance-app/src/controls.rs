// SPDX-License-Identifier: GPL-3.0-or-later
//! Semantic dispatch independent of GUI, keyboard or MIDI source.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSource {
    Gui,
    Keyboard,
    Midi,
}
#[derive(Clone, Copy, Debug)]
pub enum PartAction {
    Launch,
    Stop,
    Cancel,
    Retrigger,
    Toggle,
    Gain(f32),
    Cutoff(Option<f32>),
}

impl Coordinator {
    pub fn dispatch(&mut self, intent: Intent, source: InputSource) -> Result<(), &'static str> {
        match intent {
            Intent::Mode(mode) => {
                self.persistent_mode = mode;
                self.refresh_mode();
                return Ok(());
            }
            Intent::HoldMode {
                token,
                mode,
                pressed,
            } => {
                if !pressed {
                    self.temporary_modes
                        .retain(|(s, t, _)| (*s, *t) != (source, token));
                } else if !self
                    .temporary_modes
                    .iter()
                    .any(|(s, t, _)| (*s, *t) == (source, token))
                {
                    if self.temporary_modes.len() == 16 {
                        return Err("temporary mode capacity reached");
                    }
                    self.temporary_modes.push((source, token, mode));
                }
                self.refresh_mode();
                return Ok(());
            }
            _ => {}
        }
        let addressed = match intent {
            Intent::Part { deck, slot, action } => Some((deck, slot, action)),
            Intent::Gain(v) => Some((self.focus.deck, self.focus.address().1, PartAction::Gain(v))),
            Intent::Cutoff(v) => Some((
                self.focus.deck,
                self.focus.address().1,
                PartAction::Cutoff(v),
            )),
            _ => None,
        };
        if let Some((deck, slot, action)) = addressed {
            if deck >= 2 || slot >= SLOTS {
                return Err("invalid part address");
            }
            if !self.available() {
                return Err("engine unavailable, closed or snapshot stale");
            }
            let intent = match action {
                PartAction::Launch => Intent::Launch,
                PartAction::Stop => Intent::Stop,
                PartAction::Cancel => Intent::Cancel,
                PartAction::Retrigger => Intent::Retrigger,
                PartAction::Toggle => Intent::Toggle,
                PartAction::Gain(v) => Intent::Gain(v),
                PartAction::Cutoff(v) => Intent::Cutoff(v),
            };
            let mut focus = self.focus;
            focus.deck = deck;
            focus.slots[deck] = slot;
            let state = self.snapshot.as_ref().ok_or("no engine snapshot")?;
            let (mut request, q) = request_for(intent, &focus, state)?;
            if matches!(action, PartAction::Gain(_) | PartAction::Cutoff(_)) {
                let mut parameters = self
                    .parameter_target(deck, slot)
                    .ok_or("no part snapshot")?;
                match action {
                    PartAction::Gain(v) => parameters.gain = v,
                    PartAction::Cutoff(v) => parameters.cutoff_hz = v,
                    _ => {}
                }
                if !parameters.gain.is_finite()
                    || !(0.0..=1.0).contains(&parameters.gain)
                    || parameters.cutoff_hz.is_some_and(|v| {
                        !v.is_finite()
                            || !(20.0..=20000.0).contains(&v)
                            || v > state.sample_rate as f32 * 0.45
                    })
                {
                    return Err("invalid part parameter");
                }
                if source == InputSource::Midi {
                    self.queued_parameters[deck][slot] = Some(parameters);
                    return Ok(());
                }
                request = Request::Parameters(parameters);
            }
            return self.submit_request(deck, slot, request, q);
        }
        self.execute_intent(intent)
    }
    fn refresh_mode(&mut self) {
        self.focus.mode = self
            .temporary_modes
            .last()
            .map_or(self.persistent_mode, |(_, _, m)| *m);
    }
    pub fn temporary_mode(&self) -> bool {
        !self.temporary_modes.is_empty()
    }
    pub fn release_modes(&mut self, source: InputSource) {
        self.temporary_modes.retain(|(s, _, _)| *s != source);
        self.refresh_mode();
    }
    pub fn discard_midi_intent(&mut self) {
        self.release_modes(InputSource::Midi);
        self.queued_parameters = [[None; SLOTS]; 2];
    }
    pub fn parameter_target(&self, deck: usize, slot: usize) -> Option<PartParameters> {
        if deck >= 2 || slot >= SLOTS {
            return None;
        }
        self.queued_parameters[deck][slot]
            .or_else(|| {
                self.receipts
                    .iter()
                    .rev()
                    .find(|r| r.deck == deck && r.slot == slot && r.parameter)
                    .and_then(|r| r.parameters)
            })
            .or_else(|| self.snapshot.map(|s| s.decks[deck].slots[slot].parameters))
    }
    /// Run after dispatching the current input batch. Reserve eight receipts for
    /// discrete actions; at most four latest parameter targets are admitted/tick.
    pub fn flush_parameters(&mut self) {
        if !self.available() {
            self.queued_parameters = [[None; SLOTS]; 2];
            return;
        }
        let mut budget = 4;
        let start = self.parameter_cursor;
        for offset in 0..2 * SLOTS {
            if budget == 0 || self.receipts.len() >= 24 {
                return;
            }
            let index = (start + offset) % (2 * SLOTS);
            let deck = index / SLOTS;
            let slot = index % SLOTS;
            if self
                .receipts
                .iter()
                .any(|r| r.parameter && r.deck == deck && r.slot == slot)
            {
                continue;
            }
            if let Some(p) = self.queued_parameters[deck][slot] {
                match self.submit_request(deck, slot, Request::Parameters(p), Quantize::Immediate) {
                    Ok(()) => {
                        self.queued_parameters[deck][slot] = None;
                        budget -= 1;
                        self.parameter_cursor = (index + 1) % (2 * SLOTS);
                    }
                    Err(e) => {
                        self.notice = format!("MIDI parameter awaiting admission: {e}");
                        return;
                    }
                }
            }
        }
    }
}
