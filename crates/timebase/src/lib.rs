// SPDX-License-Identifier: GPL-3.0-or-later
//! Musical time for the offline prototype. Quarter notes have 960 ticks.
//! Tempo changes are instantaneous; ramps and transport stop/resume are deferred.

use std::fmt;

pub const TICKS_PER_QUARTER: i64 = 960;
/// Bound float conversions well below f64's integer precision limit.
pub const MAX_ABS_TICKS: i64 = 1 << 40;
const MAX_ABS_FRAME: f64 = (1_u64 << 52) as f64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct BeatTicks(pub i64);

/// Signed timeline frames support preroll. Device frame counters are separate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SampleFrame(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeError {
    InvalidQuantum,
    InvalidTempo,
    InvalidSampleRate,
    InvalidTempoMap,
    OutOfRange,
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TimeError {}

impl BeatTicks {
    /// Quantize relative to a deck epoch using Euclidean division, including preroll.
    /// `strict` means an event exactly on a boundary moves to the following boundary.
    pub fn quantize(self, quantum: Self, epoch: Self, strict: bool) -> Result<Self, TimeError> {
        if quantum.0 <= 0 {
            return Err(TimeError::InvalidQuantum);
        }
        let relative = i128::from(self.0) - i128::from(epoch.0);
        let q = i128::from(quantum.0);
        let mut index = relative.div_euclid(q);
        if strict || relative.rem_euclid(q) != 0 {
            index += 1;
        }
        let tick = i128::from(epoch.0) + index * q;
        Ok(Self(
            i64::try_from(tick).map_err(|_| TimeError::OutOfRange)?,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TempoChange {
    pub at: BeatTicks,
    pub bpm: f64,
}

#[derive(Clone, Debug)]
struct Segment {
    change: TempoChange,
    start_frame: f64,
    frames_per_tick: f64,
}

/// Immutable piecewise-constant tempo map, anchored at beat zero / frame zero.
/// Construction allocates on the control side. Conversion does not allocate.
#[derive(Clone, Debug)]
pub struct TempoMap {
    sample_rate: u32,
    segments: Vec<Segment>,
}

impl TempoMap {
    /// First change must be at zero, subsequent changes strictly increasing.
    /// The first tempo extends backwards into negative preroll.
    pub fn new(sample_rate: u32, changes: &[TempoChange]) -> Result<Self, TimeError> {
        if !(8_000..=384_000).contains(&sample_rate) {
            return Err(TimeError::InvalidSampleRate);
        }
        if changes.first().map(|change| change.at) != Some(BeatTicks(0)) {
            return Err(TimeError::InvalidTempoMap);
        }
        let mut segments: Vec<Segment> = Vec::with_capacity(changes.len());
        for &change in changes {
            if !change.bpm.is_finite() || !(1.0..=1_000.0).contains(&change.bpm) {
                return Err(TimeError::InvalidTempo);
            }
            if !(0..=MAX_ABS_TICKS).contains(&change.at.0) {
                return Err(TimeError::OutOfRange);
            }
            let start_frame = if let Some(previous) = segments.last() {
                if change.at <= previous.change.at {
                    return Err(TimeError::InvalidTempoMap);
                }
                previous.start_frame
                    + (change.at.0 - previous.change.at.0) as f64 * previous.frames_per_tick
            } else {
                0.0
            };
            if start_frame >= MAX_ABS_FRAME {
                return Err(TimeError::OutOfRange);
            }
            segments.push(Segment {
                change,
                start_frame,
                frames_per_tick: f64::from(sample_rate) * 60.0
                    / (change.bpm * TICKS_PER_QUARTER as f64),
            });
        }
        Ok(Self {
            sample_rate,
            segments,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Round once at the final absolute position, nearest frame, ties away from zero.
    /// Periodic events must use their absolute tick targets, never rounded periods.
    pub fn frame_at(&self, beat: BeatTicks) -> Result<SampleFrame, TimeError> {
        if !(-MAX_ABS_TICKS..=MAX_ABS_TICKS).contains(&beat.0) {
            return Err(TimeError::OutOfRange);
        }
        let index = self
            .segments
            .partition_point(|segment| segment.change.at <= beat);
        let segment = &self.segments[index.saturating_sub(1)];
        let frame =
            segment.start_frame + (beat.0 - segment.change.at.0) as f64 * segment.frames_per_tick;
        if frame.abs() >= MAX_ABS_FRAME {
            return Err(TimeError::OutOfRange);
        }
        Ok(SampleFrame(frame.round() as i64))
    }

    /// Nearest storage tick at a frame; this inverse intentionally loses subtick detail.
    pub fn beat_at(&self, frame: SampleFrame) -> Result<BeatTicks, TimeError> {
        let value = frame.0 as f64;
        if value.abs() >= MAX_ABS_FRAME {
            return Err(TimeError::OutOfRange);
        }
        let index = self
            .segments
            .partition_point(|segment| segment.start_frame <= value);
        let segment = &self.segments[index.saturating_sub(1)];
        let beat =
            segment.change.at.0 as f64 + (value - segment.start_frame) / segment.frames_per_tick;
        if beat.abs() > MAX_ABS_TICKS as f64 {
            return Err(TimeError::OutOfRange);
        }
        Ok(BeatTicks(beat.round() as i64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_include_preroll_and_deck_epochs() {
        for (input, strict, expected) in [
            (-961, false, -960),
            (-960, false, -960),
            (-960, true, 0),
            (-1, false, 0),
            (0, true, 960),
            (1, false, 960),
        ] {
            assert_eq!(
                BeatTicks(input).quantize(BeatTicks(960), BeatTicks(0), strict),
                Ok(BeatTicks(expected))
            );
        }
        assert_eq!(
            BeatTicks(4000).quantize(BeatTicks(7680), BeatTicks(3840), true),
            Ok(BeatTicks(11520))
        );
        assert_eq!(
            BeatTicks(i64::MAX).quantize(BeatTicks(1), BeatTicks(0), true),
            Err(TimeError::OutOfRange)
        );
        assert_eq!(
            BeatTicks(0).quantize(BeatTicks(0), BeatTicks(0), false),
            Err(TimeError::InvalidQuantum)
        );
    }

    #[test]
    fn absolute_conversion_does_not_accumulate_period_rounding() {
        for rate in [44_100, 48_000, 96_000] {
            let map = TempoMap::new(
                rate,
                &[TempoChange {
                    at: BeatTicks(0),
                    bpm: 140.0,
                }],
            )
            .unwrap();
            for quarter in -16..=10_000 {
                let expected = (f64::from(quarter) * f64::from(rate) * 3.0 / 7.0).round() as i64;
                let beat = BeatTicks(i64::from(quarter) * 960);
                assert_eq!(map.frame_at(beat), Ok(SampleFrame(expected)));
                assert_eq!(map.beat_at(SampleFrame(expected)), Ok(beat));
            }
        }
    }

    #[test]
    fn tempo_change_keeps_fractional_frame_remainder() {
        let map = TempoMap::new(
            48_000,
            &[
                TempoChange {
                    at: BeatTicks(0),
                    bpm: 140.0,
                },
                TempoChange {
                    at: BeatTicks(960),
                    bpm: 120.0,
                },
            ],
        )
        .unwrap();
        assert_eq!(map.frame_at(BeatTicks(-960)), Ok(SampleFrame(-20571)));
        assert_eq!(map.frame_at(BeatTicks(960)), Ok(SampleFrame(20571)));
        assert_eq!(map.frame_at(BeatTicks(1920)), Ok(SampleFrame(44571)));
        assert_eq!(map.beat_at(SampleFrame(44571)), Ok(BeatTicks(1920)));
    }

    #[test]
    fn rejects_invalid_maps_and_conversion_ranges() {
        assert!(TempoMap::new(48_000, &[]).is_err());
        for bpm in [0.0, -1.0, f64::NAN, f64::INFINITY, 1001.0] {
            assert!(
                TempoMap::new(
                    48_000,
                    &[TempoChange {
                        at: BeatTicks(0),
                        bpm
                    }]
                )
                .is_err()
            );
        }
        let change = TempoChange {
            at: BeatTicks(0),
            bpm: 120.0,
        };
        assert!(TempoMap::new(0, &[change]).is_err());
        assert!(TempoMap::new(48_000, &[change, change]).is_err());
        let map = TempoMap::new(48_000, &[change]).unwrap();
        assert!(map.frame_at(BeatTicks(i64::MIN)).is_err());
        assert!(map.beat_at(SampleFrame(i64::MAX)).is_err());
    }
}
