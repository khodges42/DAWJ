// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded musical scheduling using the M0 TempoMap and M2 Part reducer.
use parts::{Applied, Command, Deck, Part, SLOTS, State};
use timebase::{BeatTicks, SampleFrame, TICKS_PER_QUARTER, TempoChange, TempoMap};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantization {
    Immediate,
    Beat,
    Bar,
    Phrase,
}
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Launch(Part),
    Stop,
    Replace(Part),
    Retrigger,
    Cancel,
}
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub deck: usize,
    pub slot: usize,
    pub action: Applied,
    pub frame: SampleFrame,
}
pub struct MusicalScheduler {
    map: TempoMap,
    decks: [Deck; 2],
    epochs: [BeatTicks; 2],
    phrases: [BeatTicks; 2],
    frame: SampleFrame,
    started: [[Option<BeatTicks>; SLOTS]; 2],
    immediate: [[Option<SampleFrame>; SLOTS]; 2],
}
impl MusicalScheduler {
    pub fn new(
        mut map: TempoMap,
        epochs: [BeatTicks; 2],
        phrases: [BeatTicks; 2],
    ) -> Result<Self, &'static str> {
        if epochs.iter().any(|e| e.0 > 0) {
            return Err("initial deck epochs must not exceed beat zero");
        }
        if map.change_count() > 64 {
            return Err("too many initial tempo changes");
        }
        map.reserve_changes(64);
        Ok(Self {
            map,
            decks: [
                Deck::new(epochs[0], phrases[0])?,
                Deck::new(epochs[1], phrases[1])?,
            ],
            epochs,
            phrases,
            frame: SampleFrame(0),
            started: [[None; SLOTS]; 2],
            immediate: [[None; SLOTS]; 2],
        })
    }
    pub fn frame(&self) -> SampleFrame {
        self.frame
    }
    pub fn rate(&self) -> u32 {
        self.map.sample_rate()
    }
    pub fn state(&self, deck: usize, slot: usize) -> Option<State> {
        self.decks.get(deck)?.states().get(slot).copied()
    }
    pub fn beat(&self) -> Result<BeatTicks, &'static str> {
        self.map
            .beat_at(self.frame)
            .map_err(|_| "clock out of range")
    }
    pub fn phase(&self, deck: usize) -> Result<BeatTicks, &'static str> {
        let epoch = *self.epochs.get(deck).ok_or("deck out of range")?;
        Ok(BeatTicks(
            ((i128::from(self.beat()?.0) - i128::from(epoch.0))
                .rem_euclid(i128::from(self.phrases[deck].0))) as i64,
        ))
    }
    pub fn part_phase(&self, deck: usize, slot: usize) -> Option<BeatTicks> {
        let start = (*self.started.get(deck)?.get(slot)?)?;
        let part = self.state(deck, slot)?.active()?;
        Some(BeatTicks(
            (self.beat().ok()?.0 - start.0).rem_euclid(part.timing.nominal_length.0),
        ))
    }
    /// Inclusive boundary only if its rounded output frame has not yet been rendered.
    pub fn boundary(&self, deck: usize, q: Quantization) -> Result<BeatTicks, &'static str> {
        if deck >= 2 {
            return Err("deck out of range");
        }
        let now = self.beat()?;
        if q == Quantization::Immediate {
            // Domain tick may round backwards; retain monotonicity through the reducer.
            return Ok(now.max(self.decks[deck].current_time()));
        }
        let (quantum, epoch) = match q {
            Quantization::Beat => (BeatTicks(TICKS_PER_QUARTER), BeatTicks(0)),
            Quantization::Bar => (BeatTicks(4 * TICKS_PER_QUARTER), BeatTicks(0)),
            Quantization::Phrase => (self.phrases[deck], self.epochs[deck]),
            Quantization::Immediate => unreachable!(),
        };
        let mut at = now
            .quantize(quantum, epoch, false)
            .map_err(|_| "boundary overflow")?;
        // At high tempos the rounded frame of a boundary can invert to the tick
        // just after it. Recover that boundary before considering the next one.
        if let Some(previous) = at.0.checked_sub(quantum.0) {
            let previous = BeatTicks(previous);
            if self
                .map
                .frame_at(previous)
                .is_ok_and(|frame| frame >= self.frame)
            {
                at = previous;
            }
        }
        if self.map.frame_at(at).map_err(|_| "boundary out of range")? < self.frame {
            at = at
                .quantize(quantum, epoch, true)
                .map_err(|_| "boundary overflow")?;
        }
        self.map.frame_at(at).map_err(|_| "boundary out of range")?;
        Ok(at)
    }
    pub fn command(
        &mut self,
        deck: usize,
        slot: usize,
        action: Action,
        q: Quantization,
    ) -> Result<(), &'static str> {
        let q = if matches!(action, Action::Cancel) {
            Quantization::Immediate
        } else {
            q
        };
        let at = self.boundary(deck, q)?;
        let command = match action {
            Action::Launch(part) => Command::Start { part, at },
            Action::Stop => Command::Stop { at },
            Action::Replace(part) => Command::Replace { part, at },
            Action::Retrigger => Command::Retrigger { at },
            Action::Cancel => Command::Cancel,
        };
        self.decks
            .get_mut(deck)
            .ok_or("deck out of range")?
            .command(slot, command)?;
        self.immediate[deck][slot] = if q == Quantization::Immediate {
            Some(self.frame)
        } else {
            None
        };
        Ok(())
    }
    /// Tempo edits use musical boundaries (Immediate is rejected), preserving pending beats.
    pub fn tempo(&mut self, bpm: f64, q: Quantization) -> Result<(), &'static str> {
        if q == Quantization::Immediate {
            return Err("tempo edits require a musical boundary");
        }
        let at = self.boundary(0, q)?;
        self.map
            .edit_future(self.frame, TempoChange { at, bpm })
            .map_err(|_| "invalid or exhausted tempo edit")
    }
    /// Apply every event due before rendering this frame; callback partitions are irrelevant.
    pub fn due(&mut self, mut apply: impl FnMut(Event)) -> Result<(), &'static str> {
        for _ in 0..2 * SLOTS {
            let mut earliest = None;
            for deck in 0..2 {
                for (slot, state) in self.decks[deck].states().iter().enumerate() {
                    let at = match state {
                        State::QueuedStart(p) => Some(p.at),
                        State::QueuedStop { at, .. } => Some(*at),
                        State::QueuedReplace { pending, .. } => Some(pending.at),
                        _ => None,
                    };
                    if let Some(at) = at {
                        let due = self.immediate[deck][slot]
                            .unwrap_or(self.map.frame_at(at).map_err(|_| "event out of range")?);
                        let key = (at, deck, slot);
                        if due <= self.frame && earliest.is_none_or(|old| key < old) {
                            earliest = Some(key);
                        }
                    }
                }
            }
            let Some((at, deck, slot)) = earliest else {
                break;
            };
            if let Some(action) = self.decks[deck].apply_due_slot(slot, at)? {
                self.immediate[deck][slot] = None;
                self.started[deck][slot] = match action {
                    Applied::Start { at, .. } => Some(at),
                    Applied::Stop { .. } => None,
                };
                apply(Event {
                    deck,
                    slot,
                    action,
                    frame: self.frame,
                });
            }
        }
        Ok(())
    }
    pub fn complete(&mut self, deck: usize, slot: usize) -> Result<(), &'static str> {
        self.decks
            .get_mut(deck)
            .ok_or("deck out of range")?
            .complete(slot)?;
        self.started[deck][slot] = None;
        Ok(())
    }
    pub fn step(&mut self) -> Result<(), &'static str> {
        let next = self.frame.0.checked_add(1).ok_or("frame overflow")?;
        self.map
            .beat_at(SampleFrame(next))
            .map_err(|_| "clock horizon exhausted")?;
        self.frame = SampleFrame(next);
        Ok(())
    }
}
