// SPDX-License-Identifier: GPL-3.0-or-later
//! Fixed per-slot gain and non-resonant one-pole low-pass. No ownership or allocation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartParameters {
    pub gain: f32,
    pub muted: bool,
    pub cutoff_hz: Option<f32>,
}
impl Default for PartParameters {
    fn default() -> Self {
        Self {
            gain: 1.0,
            muted: false,
            cutoff_hz: None,
        }
    }
}
impl PartParameters {
    pub(crate) fn encode(self) -> Result<u64, &'static str> {
        if !self.gain.is_finite() || !(0.0..=1.0).contains(&self.gain) {
            return Err("part gain outside 0..=1");
        }
        let cutoff = match self.cutoff_hz {
            None => 0,
            Some(fc) if fc.is_finite() && (20.0..=20000.0).contains(&fc) => fc.round() as u64,
            _ => return Err("cutoff outside 20..=20000 Hz"),
        };
        Ok(u64::from((self.gain * 65535.0).round() as u16)
            | (cutoff << 16)
            | ((self.muted as u64) << 32))
    }
    pub(crate) fn decode(bits: u64) -> Self {
        let fc = ((bits >> 16) & 65535) as f32;
        Self {
            gain: (bits & 65535) as f32 / 65535.0,
            muted: bits & (1 << 32) != 0,
            cutoff_hz: if fc == 0.0 { None } else { Some(fc) },
        }
    }
}
#[derive(Clone, Copy)]
struct Ramp {
    value: f32,
    target: f32,
    step: f32,
    left: u32,
}
impl Ramp {
    fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            step: 0.0,
            left: 0,
        }
    }
    fn set(&mut self, target: f32, frames: u32) {
        if target != self.target {
            self.target = target;
            self.left = frames;
            self.step = (target - self.value) / frames as f32;
        }
    }
    fn settle(&mut self) {
        self.value = self.target;
        self.left = 0;
    }
    fn next(&mut self) -> f32 {
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
pub(crate) struct PartDsp {
    gain: Ramp,
    alpha: Ramp,
    state: [f32; 2],
    pub parameters: PartParameters,
    pub processed: u64,
    pub peak: [f32; 2],
}
impl Default for PartDsp {
    fn default() -> Self {
        Self {
            gain: Ramp::new(1.0),
            alpha: Ramp::new(1.0),
            state: [0.0; 2],
            parameters: PartParameters::default(),
            processed: 0,
            peak: [0.0; 2],
        }
    }
}
impl PartDsp {
    pub fn set(&mut self, p: PartParameters, rate: u32, active: bool) -> Result<(), &'static str> {
        p.encode()?;
        if p.cutoff_hz.is_some_and(|fc| fc > rate as f32 * 0.45) {
            return Err("cutoff exceeds 0.45 * sample rate");
        }
        let alpha = p.cutoff_hz.map_or(1.0, |fc| {
            1.0 - (-std::f32::consts::TAU * fc / rate as f32).exp()
        });
        let frames = (rate / 50).max(1);
        self.gain.set(if p.muted { 0.0 } else { p.gain }, frames);
        self.alpha.set(alpha, frames);
        self.parameters = p;
        if !active {
            self.gain.settle();
            self.alpha.settle();
        }
        Ok(())
    }
    pub fn start(&mut self, was_active: bool) {
        if !was_active {
            self.gain.settle();
            self.alpha.settle();
            self.state = [0.0; 2];
        }
    }
    pub fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        self.processed = self.processed.saturating_add(1);
        let gain = self.gain.next().clamp(0.0, 1.0);
        let alpha = self.alpha.next().clamp(0.0, 1.0);
        let mut output = [0.0; 2];
        for ch in 0..2 {
            self.state[ch] = if alpha == 1.0 {
                input[ch]
            } else {
                (self.state[ch] + alpha * (input[ch] - self.state[ch])).clamp(-1.0, 1.0)
            };
            if self.state[ch].abs() < 1e-20 {
                self.state[ch] = 0.0;
            }
            output[ch] = self.state[ch] * gain;
            self.peak[ch] = self.peak[ch].max(output[ch].abs());
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gain_mute_ramp_and_filter_state_continuity() {
        let mut dsp = PartDsp::default();
        dsp.process([1.0; 2]);
        dsp.set(
            PartParameters {
                gain: 0.0,
                ..PartParameters::default()
            },
            48000,
            true,
        )
        .unwrap();
        let mut previous = 1.0;
        for _ in 0..960 {
            let sample = dsp.process([1.0; 2])[0];
            assert!((sample - previous).abs() < 0.0011);
            previous = sample;
        }
        assert_eq!(previous, 0.0);
        dsp.set(
            PartParameters {
                gain: 1.0,
                muted: true,
                cutoff_hz: Some(20.0),
            },
            48000,
            true,
        )
        .unwrap();
        for _ in 0..960 {
            assert_eq!(dsp.process([1.0; 2]), [0.0; 2]);
        }
        assert!(dsp.state[0] > 0.99);
        dsp.set(
            PartParameters {
                gain: 1.0,
                muted: false,
                cutoff_hz: Some(1000.0),
            },
            48000,
            true,
        )
        .unwrap();
        assert!(dsp.state[0] > 0.99);
        for _ in 0..960 {
            dsp.process([1.0; 2]);
        }
        assert!((dsp.process([1.0; 2])[0] - 1.0).abs() < 1e-6);
    }
    #[test]
    fn stable_lowpass_attenuates_high_frequency_and_preserves_dc() {
        for rate in [8000, 44100, 48000, 96000] {
            let mut dsp = PartDsp::default();
            dsp.set(
                PartParameters {
                    cutoff_hz: Some(20.0),
                    ..PartParameters::default()
                },
                rate,
                false,
            )
            .unwrap();
            let mut energy = 0.0;
            for i in 0..rate {
                let y = dsp.process([if i % 2 == 0 { 1.0 } else { -1.0 }; 2]);
                assert!(y[0].is_finite() && y[0].abs() <= 1.0);
                energy += y[0] * y[0];
            }
            assert!(energy / (rate as f32) < 0.001);
            for _ in 0..rate {
                dsp.process([1.0; 2]);
            }
            assert!((dsp.process([1.0; 2])[0] - 1.0).abs() < 1e-4);
            assert!(
                dsp.set(
                    PartParameters {
                        cutoff_hz: Some(rate as f32),
                        ..PartParameters::default()
                    },
                    rate,
                    true
                )
                .is_err()
            );
        }
    }
    #[test]
    fn reject_bad_parameters_without_mutating_state() {
        let mut dsp = PartDsp::default();
        for value in [f32::NAN, f32::INFINITY, -1.0, 1.1] {
            assert!(
                dsp.set(
                    PartParameters {
                        gain: value,
                        ..PartParameters::default()
                    },
                    48000,
                    true
                )
                .is_err()
            );
        }
        assert_eq!(dsp.process([0.5, -0.5]), [0.5, -0.5]);
    }

    #[test]
    fn stereo_polarity_and_rapid_limit_changes_remain_bounded() {
        let mut dsp = PartDsp::default();
        for i in 0..20000 {
            if i % 13 == 0 {
                dsp.set(
                    PartParameters {
                        gain: if i % 2 == 0 { 1.0 } else { 0.0 },
                        muted: i % 3 == 0,
                        cutoff_hz: if i % 5 == 0 {
                            None
                        } else {
                            Some(if i % 2 == 0 { 20.0 } else { 20000.0 })
                        },
                    },
                    48000,
                    true,
                )
                .unwrap();
            }
            let x = if i % 2 == 0 { 1.0 } else { -1.0 };
            let output = dsp.process([x, -x]);
            assert!(output.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
            assert!((output[0] + output[1]).abs() < 1e-6);
        }
    }
}
