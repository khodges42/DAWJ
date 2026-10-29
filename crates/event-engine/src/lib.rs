// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline impulse scheduling. Compilation is control-side work; rendering walks
//! a fixed immutable schedule. Live tempo edits and command queues belong to M2.

use std::fmt;
use timebase::{BeatTicks, SampleFrame, TempoMap, TimeError};

pub const MAX_EVENTS: usize = 4096;
pub const MAX_BLOCK_FRAMES: usize = 8192;

#[derive(Clone, Copy, Debug)]
pub struct Impulse {
    pub at: BeatTicks,
    pub amplitude: f32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScheduleError {
    Time(TimeError),
    TooManyEvents,
    InvalidAmplitude,
    EventBeforeStart,
    BlockTooLarge,
    FrameOverflow,
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ScheduleError {}

impl From<TimeError> for ScheduleError {
    fn from(error: TimeError) -> Self {
        Self::Time(error)
    }
}

#[derive(Debug)]
struct ScheduledImpulse {
    frame: SampleFrame,
    amplitude: f32,
}

#[derive(Debug)]
pub struct OfflineScheduler {
    events: Vec<ScheduledImpulse>,
    cursor: usize,
    frame: SampleFrame,
}

impl OfflineScheduler {
    /// Compile a frozen tempo map once. Equal-frame events retain input order.
    /// Reject events before `start`; callers must explicitly decide preroll policy.
    pub fn new(
        map: &TempoMap,
        impulses: &[Impulse],
        start: SampleFrame,
    ) -> Result<Self, ScheduleError> {
        if impulses.len() > MAX_EVENTS {
            return Err(ScheduleError::TooManyEvents);
        }
        let mut events = Vec::with_capacity(impulses.len());
        for impulse in impulses {
            if !impulse.amplitude.is_finite() || impulse.amplitude.abs() > 1.0 {
                return Err(ScheduleError::InvalidAmplitude);
            }
            let frame = map.frame_at(impulse.at)?;
            if frame < start {
                return Err(ScheduleError::EventBeforeStart);
            }
            events.push(ScheduledImpulse {
                frame,
                amplitude: impulse.amplitude,
            });
        }
        events.sort_by_key(|event| event.frame);
        Ok(Self {
            events,
            cursor: 0,
            frame: start,
        })
    }

    pub fn frame(&self) -> SampleFrame {
        self.frame
    }

    pub fn remaining_events(&self) -> usize {
        self.events.len() - self.cursor
    }

    /// Fill a mono block for [frame, frame + len). Events on the right boundary
    /// belong to the next call. Empty blocks do not consume events. On error,
    /// output and scheduler state are unchanged. No allocation or I/O occurs here.
    /// Coincident impulses sum in f64 then clamp to [-1, 1] as fixture protection.
    pub fn render(&mut self, output: &mut [f32]) -> Result<(), ScheduleError> {
        if output.len() > MAX_BLOCK_FRAMES {
            return Err(ScheduleError::BlockTooLarge);
        }
        let end = self
            .frame
            .0
            .checked_add(output.len() as i64)
            .ok_or(ScheduleError::FrameOverflow)?;
        output.fill(0.0);
        while self.cursor < self.events.len() && self.events[self.cursor].frame.0 < end {
            let event_frame = self.events[self.cursor].frame.0;
            let mut sum = 0.0_f64;
            while self.cursor < self.events.len() && self.events[self.cursor].frame.0 == event_frame
            {
                sum += f64::from(self.events[self.cursor].amplitude);
                self.cursor += 1;
            }
            output[(event_frame - self.frame.0) as usize] = sum.clamp(-1.0, 1.0) as f32;
        }
        self.frame = SampleFrame(end);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use timebase::TempoChange;
    fn map(rate: u32) -> TempoMap {
        TempoMap::new(
            rate,
            &[
                TempoChange {
                    at: BeatTicks(0),
                    bpm: 140.0,
                },
                TempoChange {
                    at: BeatTicks(3840),
                    bpm: 120.0,
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn exact_offsets_are_independent_of_callback_partition() {
        for rate in [44_100, 48_000, 96_000] {
            let map = map(rate);
            let impulses: Vec<_> = (0..16)
                .rev()
                .map(|quarter| Impulse {
                    at: BeatTicks(quarter * 960),
                    amplitude: 0.5,
                })
                .collect();
            let total = map.frame_at(BeatTicks(16 * 960)).unwrap().0 as usize;
            let mut expected = vec![0.0; total];
            for quarter in 0..16 {
                // Independent closed-form reference: four beats at 140, then 120 BPM.
                let seconds =
                    f64::from(quarter.min(4)) * 3.0 / 7.0 + f64::from((quarter - 4).max(0)) * 0.5;
                expected[(seconds * f64::from(rate)).round() as usize] = 0.5;
            }
            for block_size in [1, 31, 64, 127, 128, 256, 511, 1024, 8192] {
                let mut scheduler = OfflineScheduler::new(&map, &impulses, SampleFrame(0)).unwrap();
                let mut actual = vec![0.0; total];
                for block in actual.chunks_mut(block_size) {
                    scheduler.render(block).unwrap();
                }
                assert_eq!(actual, expected, "rate={rate}, block={block_size}");
                assert_eq!(scheduler.remaining_events(), 0);
            }
        }
    }

    #[test]
    fn right_boundary_and_empty_block_do_not_consume_event() {
        let map = TempoMap::new(
            48_000,
            &[TempoChange {
                at: BeatTicks(0),
                bpm: 120.0,
            }],
        )
        .unwrap();
        let impulses = [Impulse {
            at: BeatTicks(1),
            amplitude: 0.25,
        }];
        let mut scheduler = OfflineScheduler::new(&map, &impulses, SampleFrame(0)).unwrap();
        scheduler.render(&mut [0.0; 25]).unwrap();
        scheduler.render(&mut []).unwrap();
        assert_eq!(scheduler.remaining_events(), 1);
        let mut output = [0.0; 1];
        scheduler.render(&mut output).unwrap();
        assert_eq!(output, [0.25]);
    }

    #[test]
    fn collision_preroll_and_errors_are_explicit() {
        let map = map(48_000);
        let impulses = [Impulse {
            at: BeatTicks(-960),
            amplitude: 0.75,
        }; 2];
        assert!(matches!(
            OfflineScheduler::new(&map, &impulses, SampleFrame(0)),
            Err(ScheduleError::EventBeforeStart)
        ));
        let start = map.frame_at(BeatTicks(-960)).unwrap();
        let mut scheduler = OfflineScheduler::new(&map, &impulses, start).unwrap();
        let mut output = [0.0];
        scheduler.render(&mut output).unwrap();
        assert_eq!(output, [1.0]);
        assert!(
            OfflineScheduler::new(
                &map,
                &[Impulse {
                    at: BeatTicks(0),
                    amplitude: f32::NAN
                }],
                SampleFrame(0)
            )
            .is_err()
        );
        assert!(matches!(
            OfflineScheduler::new(
                &map,
                &vec![
                    Impulse {
                        at: BeatTicks(0),
                        amplitude: 0.0
                    };
                    MAX_EVENTS + 1
                ],
                SampleFrame(0)
            ),
            Err(ScheduleError::TooManyEvents)
        ));
        let mut large = vec![0.25; MAX_BLOCK_FRAMES + 1];
        let before = scheduler.frame();
        assert_eq!(
            scheduler.render(&mut large),
            Err(ScheduleError::BlockTooLarge)
        );
        assert_eq!(scheduler.frame(), before);
        assert!(large.iter().all(|&sample| sample == 0.25));
        let mut scheduler = OfflineScheduler::new(&map, &[], SampleFrame(i64::MAX)).unwrap();
        assert_eq!(
            scheduler.render(&mut output),
            Err(ScheduleError::FrameOverflow)
        );
        assert_eq!(output, [1.0]);
    }
}