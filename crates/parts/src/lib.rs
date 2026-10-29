// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure domain reducer. No audio, assets, hardware, clocks or command queues.
use timebase::BeatTicks;
pub const SLOTS: usize = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackMode {
    OneShot,
    Loop { start: u64, end: u64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaybackConfiguration {
    pub asset: AssetId,
    pub mode: PlaybackMode,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimingPolicy {
    pub nominal_length: BeatTicks,
    pub quantum: BeatTicks,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub id: PartId,
    pub playback: PlaybackConfiguration,
    pub timing: TimingPolicy,
}
impl Part {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.timing.nominal_length.0 <= 0 || self.timing.quantum.0 <= 0 {
            return Err("invalid musical timing");
        }
        if let PlaybackMode::Loop { start, end } = self.playback.mode {
            if start >= end {
                return Err("empty loop");
            }
        }
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub part: Part,
    pub at: BeatTicks,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Stopped,
    QueuedStart(Pending),
    Playing(Part),
    QueuedStop {
        active: Part,
        at: BeatTicks,
    },
    QueuedReplace {
        active: Part,
        pending: Pending,
    },
}
impl State {
    pub fn active(self) -> Option<Part> {
        match self {
            Self::Playing(p)
            | Self::QueuedStop { active: p, .. }
            | Self::QueuedReplace { active: p, .. } => Some(p),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Command {
    Start { part: Part, at: BeatTicks },
    Replace { part: Part, at: BeatTicks },
    Retrigger { at: BeatTicks },
    Stop { at: BeatTicks },
    StopNow,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    Start {
        part: Part,
        at: BeatTicks,
        retrigger: bool,
    },
    Stop {
        at: BeatTicks,
    },
}
#[derive(Clone, Debug)]
pub struct Deck {
    slots: [State; SLOTS],
    epoch: BeatTicks,
    phrase: BeatTicks,
    now: BeatTicks,
}
impl Deck {
    pub fn new(epoch: BeatTicks, phrase: BeatTicks) -> Result<Self, &'static str> {
        if phrase.0 <= 0 {
            return Err("invalid phrase length");
        }
        Ok(Self {
            slots: [State::Stopped; SLOTS],
            epoch,
            phrase,
            now: epoch,
        })
    }
    pub fn states(&self) -> &[State; SLOTS] {
        &self.slots
    }
    /// Synchronous source EOF from the reducer's owner, never an asynchronous stale acknowledgement.
    pub fn complete(&mut self, slot: usize) -> Result<(), &'static str> {
        let state = self.slots.get_mut(slot).ok_or("slot out of range")?;
        *state = match *state {
            State::Playing(_) | State::QueuedStop { .. } => State::Stopped,
            State::QueuedReplace { pending, .. } => State::QueuedStart(pending),
            _ => return Err("no active part to complete"),
        };
        Ok(())
    }
    pub fn phrase_position(&self) -> BeatTicks {
        BeatTicks(
            ((i128::from(self.now.0) - i128::from(self.epoch.0))
                .rem_euclid(i128::from(self.phrase.0))) as i64,
        )
    }
    /// Explicit caller-resolved boundary; advancing past it applies the scheduled action.
    pub fn advance(&mut self, now: BeatTicks) -> Result<[Option<Applied>; SLOTS], &'static str> {
        if now < self.now {
            return Err("domain clock cannot move backwards");
        }
        let mut applied = [None; SLOTS];
        for (index, slot) in self.slots.iter_mut().enumerate() {
            applied[index] = match *slot {
                State::QueuedStart(p) if p.at <= now => Some(Applied::Start {
                    part: p.part,
                    at: p.at,
                    retrigger: false,
                }),
                State::QueuedReplace { active, pending } if pending.at <= now => {
                    Some(Applied::Start {
                        part: pending.part,
                        at: pending.at,
                        retrigger: active.id == pending.part.id,
                    })
                }
                State::QueuedStop { at, .. } if at <= now => Some(Applied::Stop { at }),
                _ => None,
            };
            *slot = match *slot {
                State::QueuedStart(p) if p.at <= now => State::Playing(p.part),
                State::QueuedStop { at, .. } if at <= now => State::Stopped,
                State::QueuedReplace { pending, .. } if pending.at <= now => {
                    State::Playing(pending.part)
                }
                other => other,
            };
        }
        self.now = now;
        Ok(applied)
    }
    pub fn command(&mut self, slot: usize, command: Command) -> Result<(), &'static str> {
        let state = *self.slots.get(slot).ok_or("slot out of range")?;
        let check = |at| {
            if at < self.now {
                Err("command is late")
            } else {
                Ok(())
            }
        };
        let next = match command {
            Command::StopNow => State::Stopped,
            Command::Cancel => state.active().map_or(State::Stopped, State::Playing),
            Command::Start { part, at } => {
                part.validate()?;
                check(at)?;
                match state {
                    State::Stopped | State::QueuedStart(_) => {
                        State::QueuedStart(Pending { part, at })
                    }
                    State::Playing(active) | State::QueuedStop { active, .. } if active == part => {
                        State::Playing(active)
                    }
                    _ => return Err("start conflicts with active part; use replace or retrigger"),
                }
            }
            Command::Replace { part, at } => {
                part.validate()?;
                check(at)?;
                let active = state.active().ok_or("replace needs active part")?;
                State::QueuedReplace {
                    active,
                    pending: Pending { part, at },
                }
            }
            Command::Retrigger { at } => {
                check(at)?;
                let active = state.active().ok_or("retrigger needs active part")?;
                State::QueuedReplace {
                    active,
                    pending: Pending { part: active, at },
                }
            }
            Command::Stop { at } => {
                check(at)?;
                state
                    .active()
                    .map_or(State::Stopped, |active| State::QueuedStop { active, at })
            }
        };
        self.slots[slot] = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn part(id: u64) -> Part {
        Part {
            id: PartId(id),
            playback: PlaybackConfiguration {
                asset: AssetId(id),
                mode: PlaybackMode::OneShot,
            },
            timing: TimingPolicy {
                nominal_length: BeatTicks(960),
                quantum: BeatTicks(240),
            },
        }
    }
    fn deck() -> Deck {
        Deck::new(BeatTicks(0), BeatTicks(3840)).unwrap()
    }
    #[test]
    fn launch_replace_cancel_stop_and_retrigger() {
        let mut d = deck();
        d.command(
            0,
            Command::Start {
                part: part(1),
                at: BeatTicks(240),
            },
        )
        .unwrap();
        d.advance(BeatTicks(239)).unwrap();
        assert!(matches!(d.states()[0], State::QueuedStart(_)));
        d.advance(BeatTicks(240)).unwrap();
        assert_eq!(d.states()[0], State::Playing(part(1)));
        d.command(
            0,
            Command::Replace {
                part: part(2),
                at: BeatTicks(480),
            },
        )
        .unwrap();
        d.command(0, Command::Cancel).unwrap();
        assert_eq!(d.states()[0], State::Playing(part(1)));
        d.command(0, Command::Retrigger { at: BeatTicks(480) })
            .unwrap();
        assert!(matches!(d.states()[0], State::QueuedReplace { .. }));
        d.advance(BeatTicks(480)).unwrap();
        d.command(0, Command::Stop { at: BeatTicks(720) }).unwrap();
        d.command(0, Command::Cancel).unwrap();
        assert_eq!(d.states()[0], State::Playing(part(1)));
        d.command(0, Command::Stop { at: BeatTicks(720) }).unwrap();
        d.advance(BeatTicks(720)).unwrap();
        assert_eq!(d.states()[0], State::Stopped);
    }
    #[test]
    fn conflicts_are_transactional_and_latest_pending_wins() {
        let mut d = deck();
        assert!(d.command(8, Command::StopNow).is_err());
        assert!(
            d.command(0, Command::Retrigger { at: BeatTicks(0) })
                .is_err()
        );
        d.command(
            0,
            Command::Start {
                part: part(1),
                at: BeatTicks(10),
            },
        )
        .unwrap();
        d.command(
            0,
            Command::Start {
                part: part(2),
                at: BeatTicks(20),
            },
        )
        .unwrap();
        d.advance(BeatTicks(20)).unwrap();
        assert_eq!(d.states()[0], State::Playing(part(2)));
        assert!(
            d.command(
                0,
                Command::Start {
                    part: part(3),
                    at: BeatTicks(30)
                }
            )
            .is_err()
        );
        assert_eq!(d.states()[0], State::Playing(part(2)));
        d.command(
            0,
            Command::Replace {
                part: part(3),
                at: BeatTicks(30),
            },
        )
        .unwrap();
        d.command(0, Command::Stop { at: BeatTicks(40) }).unwrap();
        d.advance(BeatTicks(30)).unwrap();
        assert!(matches!(d.states()[0], State::QueuedStop { .. }));
        assert!(d.advance(BeatTicks(29)).is_err());
        assert!(d.command(0, Command::Stop { at: BeatTicks(0) }).is_err());
    }
    #[test]
    fn independent_slots_and_phrase_do_not_follow_part_length() {
        let mut d = deck();
        for slot in 0..SLOTS {
            d.command(
                slot,
                Command::Start {
                    part: part(slot as u64),
                    at: BeatTicks(0),
                },
            )
            .unwrap();
        }
        d.advance(BeatTicks(2000)).unwrap();
        assert_eq!(d.phrase_position(), BeatTicks(2000));
        d.command(0, Command::StopNow).unwrap();
        assert!(matches!(d.states()[1], State::Playing(_)));
        d.advance(BeatTicks(4000)).unwrap();
        assert_eq!(d.phrase_position(), BeatTicks(160));
        d.command(
            0,
            Command::Start {
                part: part(1),
                at: BeatTicks(4000),
            },
        )
        .unwrap();
        d.command(0, Command::Cancel).unwrap();
        assert_eq!(d.states()[0], State::Stopped);
        let mut p = part(9);
        p.timing.nominal_length = BeatTicks(0);
        assert!(
            d.command(
                0,
                Command::Start {
                    part: p,
                    at: BeatTicks(4000)
                }
            )
            .is_err()
        );
    }
}
