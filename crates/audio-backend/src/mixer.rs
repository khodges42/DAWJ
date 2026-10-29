// SPDX-License-Identifier: GPL-3.0-or-later
//! Backend-independent stereo two-deck mixer. All ownership is prepared before rendering.
use crate::playback::{PlaybackSource, Renderer, Shared};
use std::sync::{
    Arc,
    atomic::{AtomicU32, AtomicU64, Ordering},
};

/// About -18 dBFS. Sample-peak protection, not a true-peak or hearing-safety guarantee.
pub const MASTER_CEILING: f32 = 0.125;
pub const MASTER_HEADROOM: f32 = 0.125;

#[derive(Clone, Copy, Debug, Default)]
pub enum CrossfadeCurve {
    Linear,
    #[default]
    EqualPower,
}

#[derive(Clone, Copy, Debug)]
pub struct MixerParameters {
    pub gain_a: f32,
    pub gain_b: f32,
    /// 0 = A only, 1 = B only.
    pub crossfader: f32,
    pub curve: CrossfadeCurve,
}

impl Default for MixerParameters {
    fn default() -> Self {
        Self {
            gain_a: 0.5,
            gain_b: 0.5,
            crossfader: 0.5,
            curve: CrossfadeCurve::EqualPower,
        }
    }
}

pub fn crossfade_weights(position: f32, curve: CrossfadeCurve) -> Result<[f32; 2], &'static str> {
    if !position.is_finite() || !(0.0..=1.0).contains(&position) {
        return Err("crossfader must be finite in 0..=1");
    }
    if position == 0.0 {
        return Ok([1.0, 0.0]);
    }
    if position == 1.0 {
        return Ok([0.0, 1.0]);
    }
    Ok(match curve {
        CrossfadeCurve::Linear => [1.0 - position, position],
        CrossfadeCurve::EqualPower => {
            let angle = position * std::f32::consts::FRAC_PI_2;
            [angle.cos(), angle.sin()]
        }
    })
}

fn pack(p: MixerParameters) -> Result<u64, &'static str> {
    let mut bits = 0;
    for (shift, value) in [(0, p.gain_a), (16, p.gain_b), (32, p.crossfader)] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err("mixer parameters must be finite in 0..=1");
        }
        bits |= u64::from((value * 65535.0).round() as u16) << shift;
    }
    if matches!(p.curve, CrossfadeCurve::EqualPower) {
        bits |= 1 << 48;
    }
    Ok(bits)
}

struct Controls {
    parameters: AtomicU64,
    peak_l: AtomicU32,
    peak_r: AtomicU32,
}

/// One atomic publication updates all parameters together. Last writer wins.
#[derive(Clone)]
pub struct MixerControl(Arc<Controls>);
impl MixerControl {
    pub fn set(&self, parameters: MixerParameters) -> Result<(), &'static str> {
        self.0
            .parameters
            .store(pack(parameters)?, Ordering::Release);
        Ok(())
    }
    /// Post-limiter absolute sample peaks held for the lifetime of this mixer.
    pub fn peaks(&self) -> [f32; 2] {
        [
            f32::from_bits(self.0.peak_l.load(Ordering::Relaxed)),
            f32::from_bits(self.0.peak_r.load(Ordering::Relaxed)),
        ]
    }
}

#[derive(Default)]
struct Ramp {
    value: f32,
    target: f32,
    step: f32,
    left: u32,
}
impl Ramp {
    fn next(&mut self, target: f32, frames: u32) -> f32 {
        if target != self.target {
            self.target = target;
            self.left = frames;
            self.step = (target - self.value) / frames as f32;
        }
        if self.left > 0 {
            self.value += self.step;
            self.left -= 1;
            if self.left == 0 {
                self.value = self.target;
            }
        }
        self.value
    }
}

pub struct StereoMixer {
    decks: [Renderer; 2],
    controls: MixerControl,
    ramps: [Ramp; 2],
    ramp_frames: u32,
    release: f32,
    limiter_gain: f32,
    peak: [f32; 2],
}

impl StereoMixer {
    pub fn new(
        a: PlaybackSource,
        b: PlaybackSource,
        rate: u32,
    ) -> Result<(Self, MixerControl), &'static str> {
        let decks = [
            Renderer::new(a, rate, 2, Arc::new(Shared::default()))?,
            Renderer::new(b, rate, 2, Arc::new(Shared::default()))?,
        ];
        let controls = MixerControl(Arc::new(Controls {
            parameters: AtomicU64::new(pack(MixerParameters::default())?),
            peak_l: AtomicU32::new(0),
            peak_r: AtomicU32::new(0),
        }));
        Ok((
            Self {
                decks,
                controls: controls.clone(),
                ramps: [Ramp::default(), Ramp::default()],
                ramp_frames: (rate / 50).max(1),
                release: (-1.0 / (rate as f32 * 0.1)).exp(),
                limiter_gain: 1.0,
                peak: [0.0; 2],
            },
            controls,
        ))
    }

    pub fn positions(&self) -> [u64; 2] {
        [self.decks[0].position(), self.decks[1].position()]
    }
    pub fn finished(&self) -> bool {
        self.decks.iter().all(|d| d.position() == d.length())
    }

    /// Atomic parameter reads per frame make offline automation independent of callback size.
    /// Muted decks continue advancing; EOF on one deck never stops the other.
    pub fn render(&mut self, output: &mut [f32]) {
        self.render_converted(output, |s| s);
    }

    pub(crate) fn render_converted<T: Copy>(
        &mut self,
        output: &mut [T],
        convert: impl Fn(f32) -> T,
    ) {
        output.fill(convert(0.0));
        for frame in output.chunks_exact_mut(2) {
            let bits = self.controls.0.parameters.load(Ordering::Acquire);
            let x = ((bits >> 32) & 65535) as f32 / 65535.0;
            let curve = if bits & (1 << 48) == 0 {
                CrossfadeCurve::Linear
            } else {
                CrossfadeCurve::EqualPower
            };
            let weights = crossfade_weights(x, curve).unwrap_or([0.0; 2]);
            let gains = [
                (bits & 65535) as f32 / 65535.0,
                ((bits >> 16) & 65535) as f32 / 65535.0,
            ];
            let mut mixed = [0.0_f32; 2];
            for deck in 0..2 {
                let position = self.decks[deck].position();
                let remaining = self.decks[deck].length() - position;
                // Fade source boundaries as well as control changes, including prepared PCM.
                let envelope = (position as f32 / self.ramp_frames as f32)
                    .min(remaining.saturating_sub(1) as f32 / self.ramp_frames as f32)
                    .min(1.0);
                let gain =
                    self.ramps[deck].next(gains[deck] * weights[deck], self.ramp_frames) * envelope;
                let mut source = [0.0; 2];
                self.decks[deck].next_frame(&mut source);
                for ch in 0..2 {
                    mixed[ch] += source[ch] * gain * MASTER_HEADROOM;
                }
            }
            let peak = mixed[0].abs().max(mixed[1].abs());
            let required = if peak > MASTER_CEILING {
                MASTER_CEILING / peak
            } else {
                1.0
            };
            self.limiter_gain = if required < self.limiter_gain {
                required
            } else {
                required + self.release * (self.limiter_gain - required)
            };
            for ch in 0..2 {
                let sample = (mixed[ch] * self.limiter_gain).clamp(-MASTER_CEILING, MASTER_CEILING);
                frame[ch] = convert(sample);
                self.peak[ch] = self.peak[ch].max(sample.abs());
            }
        }
        self.controls
            .0
            .peak_l
            .store(self.peak[0].to_bits(), Ordering::Relaxed);
        self.controls
            .0
            .peak_r
            .store(self.peak[1].to_bits(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::PreparedBuffer;
    fn source(frame: [f32; 2], frames: usize) -> PlaybackSource {
        PlaybackSource::Buffer(
            PreparedBuffer::new((0..frames).flat_map(|_| frame).collect(), 48000, 2).unwrap(),
        )
    }
    fn parameters(x: f32, curve: CrossfadeCurve) -> MixerParameters {
        MixerParameters {
            gain_a: 1.0,
            gain_b: 1.0,
            crossfader: x,
            curve,
        }
    }
    #[test]
    fn curves_have_exact_endpoints_and_equal_power_center() {
        for curve in [CrossfadeCurve::Linear, CrossfadeCurve::EqualPower] {
            assert_eq!(crossfade_weights(0.0, curve).unwrap(), [1.0, 0.0]);
            assert_eq!(crossfade_weights(1.0, curve).unwrap(), [0.0, 1.0]);
        }
        for step in 0..=100 {
            let w = crossfade_weights(step as f32 / 100.0, CrossfadeCurve::EqualPower).unwrap();
            assert!((w[0] * w[0] + w[1] * w[1] - 1.0).abs() < 1e-6);
        }
        assert_eq!(
            crossfade_weights(0.5, CrossfadeCurve::Linear).unwrap(),
            [0.5; 2]
        );
    }
    #[test]
    fn stereo_endpoints_and_deck_gain_are_isolated() {
        for (x, expected) in [(0.0, [0.0625, -0.03125]), (1.0, [-0.015625, 0.03125])] {
            let (mut mixer, controls) = StereoMixer::new(
                source([0.5, -0.25], 5000),
                source([-0.125, 0.25], 5000),
                48000,
            )
            .unwrap();
            controls
                .set(parameters(x, CrossfadeCurve::EqualPower))
                .unwrap();
            let mut samples = vec![0.0; 2400];
            mixer.render(&mut samples);
            assert_eq!(&samples[2398..], &expected);
            controls
                .set(MixerParameters {
                    gain_a: 0.0,
                    gain_b: 0.0,
                    ..parameters(x, CrossfadeCurve::Linear)
                })
                .unwrap();
            mixer.render(&mut samples);
            assert_eq!(&samples[2398..], &[0.0, 0.0]);
        }
    }
    #[test]
    fn correlated_full_scale_sources_are_limited_and_metered() {
        let (mut mixer, controls) =
            StereoMixer::new(source([1.0, 0.5], 5000), source([1.0, 0.5], 5000), 48000).unwrap();
        controls
            .set(parameters(0.5, CrossfadeCurve::EqualPower))
            .unwrap();
        let mut samples = vec![0.0; 6000];
        mixer.render(&mut samples);
        assert!(
            samples
                .iter()
                .all(|s| s.is_finite() && s.abs() <= MASTER_CEILING)
        );
        assert!((controls.peaks()[0] - MASTER_CEILING).abs() < 1e-6);
        for pair in samples.chunks_exact(2) {
            assert!((pair[0] * 0.5 - pair[1]).abs() < 1e-6);
        }
        assert_eq!(
            controls.peaks(),
            [
                samples.iter().step_by(2).copied().fold(0.0, f32::max),
                samples
                    .iter()
                    .skip(1)
                    .step_by(2)
                    .copied()
                    .fold(0.0, f32::max)
            ]
        );
    }
    #[test]
    fn muted_decks_advance_and_eof_is_independent() {
        let (mut mixer, controls) =
            StereoMixer::new(source([0.2; 2], 100), source([0.3; 2], 3000), 48000).unwrap();
        controls
            .set(parameters(0.0, CrossfadeCurve::Linear))
            .unwrap();
        mixer.render(&mut [0.0; 400]);
        assert_eq!(mixer.positions(), [100, 200]);
        assert!(!mixer.finished());
        controls
            .set(parameters(1.0, CrossfadeCurve::Linear))
            .unwrap();
        let mut samples = vec![0.0; 4000];
        mixer.render(&mut samples);
        assert!(samples.iter().any(|s| *s > 0.03));
        mixer.render(&mut samples);
        assert!(mixer.finished());
        mixer.render(&mut samples);
        assert!(samples.iter().all(|s| *s == 0.0));
    }
    #[test]
    fn parameter_ramps_are_bounded_and_bad_updates_rejected() {
        let (mut mixer, controls) =
            StereoMixer::new(source([1.0; 2], 6000), source([0.0; 2], 6000), 48000).unwrap();
        controls
            .set(parameters(0.0, CrossfadeCurve::Linear))
            .unwrap();
        mixer.render(&mut [0.0; 2400]);
        controls
            .set(parameters(1.0, CrossfadeCurve::EqualPower))
            .unwrap();
        for bad in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(
                controls
                    .set(parameters(bad, CrossfadeCurve::Linear))
                    .is_err()
            );
        }
        let mut samples = [0.0; 1920];
        mixer.render(&mut samples);
        let mut previous = MASTER_HEADROOM;
        for &sample in samples.iter().step_by(2) {
            assert!((sample - previous).abs() <= MASTER_HEADROOM / 960.0 + 1e-6);
            previous = sample;
        }
        assert_eq!(previous, 0.0);
    }
    #[test]
    fn callback_partition_does_not_change_automation() {
        fn run(block: usize) -> Vec<f32> {
            let (mut mixer, controls) =
                StereoMixer::new(source([0.4, -0.2], 4000), source([-0.3, 0.5], 3000), 48000)
                    .unwrap();
            let mut samples = vec![0.0; 9000];
            for (segment, x) in [(0..3000, 0.0), (3000..6000, 0.5), (6000..9000, 1.0)] {
                controls
                    .set(parameters(x, CrossfadeCurve::EqualPower))
                    .unwrap();
                for chunk in samples[segment].chunks_mut(block * 2) {
                    mixer.render(chunk);
                }
            }
            samples
        }
        assert_eq!(run(1), run(1024));
        assert_eq!(run(31), run(8192));
    }

    #[test]
    fn heterogeneous_sources_and_partial_frames_are_safe() {
        let tone = PlaybackSource::Tone {
            frequency_hz: 330.0,
            amplitude: 0.1,
            frames: 2000,
        };
        let (mut mixer, _) = StereoMixer::new(source([0.1, -0.1], 1000), tone, 48000).unwrap();
        mixer.render(&mut []);
        assert_eq!(mixer.positions(), [0, 0]);
        let mut output = [9.0; 4003];
        mixer.render(&mut output);
        assert_eq!(output[4002], 0.0);
        assert_eq!(mixer.positions(), [1000, 2000]);
        assert!(mixer.finished());
        assert!(output.iter().any(|s| *s != 0.0));
    }
}
