// SPDX-License-Identifier: GPL-3.0-or-later
//! Hardware-independent, single-source renderer. Preparation allocates; rendering does not.
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

/// Interleaved, finite PCM at an explicit rate. No decoding or resampling.
pub struct PreparedBuffer {
    pub(crate) samples: Box<[f32]>,
    pub(crate) rate: u32,
    pub(crate) channels: u16,
}

impl PreparedBuffer {
    pub fn new(samples: Vec<f32>, rate: u32, channels: u16) -> Result<Self, &'static str> {
        if rate == 0
            || channels == 0
            || samples.is_empty()
            || samples.len() % usize::from(channels) != 0
            || samples.iter().any(|s| !s.is_finite() || s.abs() > 1.0)
        {
            return Err("invalid prepared PCM geometry, rate, or sample");
        }
        Ok(Self {
            samples: samples.into_boxed_slice(),
            rate,
            channels,
        })
    }
}

pub enum PlaybackSource {
    Tone {
        frequency_hz: f64,
        amplitude: f32,
        frames: u64,
    },
    Buffer(PreparedBuffer),
}

#[derive(Default)]
pub(crate) struct Shared {
    pub enabled: AtomicBool,
    pub failed: AtomicBool,
    pub finished: AtomicBool,
    pub clock: AtomicU64,
    pub source_frames: AtomicU64,
    pub max_callback_frames: AtomicU64,
}

/// Atomic telemetry only: no locks, commands carrying heap ownership, or source mutation.
#[derive(Clone)]
pub struct PlaybackMonitor(pub(crate) Arc<Shared>);

#[derive(Debug)]
pub struct PlaybackStatus {
    /// Delivered device frames since construction, including silence; not a DAC timestamp.
    pub sample_clock: u64,
    pub source_frames: u64,
    pub finished: bool,
    pub failed: bool,
    pub max_callback_frames: u64,
}

impl PlaybackMonitor {
    /// Fields are independent atomic observations, not a transactional snapshot.
    pub fn status(&self) -> PlaybackStatus {
        PlaybackStatus {
            sample_clock: self.0.clock.load(Ordering::Relaxed),
            source_frames: self.0.source_frames.load(Ordering::Relaxed),
            finished: self.0.finished.load(Ordering::Acquire),
            failed: self.0.failed.load(Ordering::Acquire),
            max_callback_frames: self.0.max_callback_frames.load(Ordering::Relaxed),
        }
    }
}

pub(crate) struct Renderer {
    source: PlaybackSource,
    rate: u32,
    channels: usize,
    position: u64,
    phase: f64,
    shared: Arc<Shared>,
}

impl Renderer {
    pub(crate) fn position(&self) -> u64 {
        self.position
    }
    pub(crate) fn length(&self) -> u64 {
        match &self.source {
            PlaybackSource::Tone { frames, .. } => *frames,
            PlaybackSource::Buffer(b) => (b.samples.len() / self.channels) as u64,
        }
    }
    pub(crate) fn next_frame(&mut self, frame: &mut [f32]) {
        frame.fill(0.0);
        if self.position >= self.length() {
            return;
        }
        match &self.source {
            PlaybackSource::Tone {
                frequency_hz,
                amplitude,
                ..
            } => {
                frame.fill(self.phase.sin() as f32 * amplitude);
                self.phase = (self.phase
                    + std::f64::consts::TAU * frequency_hz / f64::from(self.rate))
                    % std::f64::consts::TAU;
            }
            PlaybackSource::Buffer(buffer) => {
                let start = self.position as usize * self.channels;
                frame.copy_from_slice(&buffer.samples[start..start + self.channels]);
            }
        }
        self.position += 1;
    }
    pub fn new(
        source: PlaybackSource,
        rate: u32,
        channels: u16,
        shared: Arc<Shared>,
    ) -> Result<Self, &'static str> {
        if rate == 0 || channels == 0 {
            return Err("zero rate or channels");
        }
        match &source {
            PlaybackSource::Tone {
                frequency_hz,
                amplitude,
                frames,
            } => {
                if !frequency_hz.is_finite()
                    || *frequency_hz <= 0.0
                    || *frequency_hz >= f64::from(rate) / 2.0
                    || !amplitude.is_finite()
                    || !(0.0..=1.0).contains(amplitude)
                    || *frames == 0
                {
                    return Err("invalid tone parameters");
                }
            }
            PlaybackSource::Buffer(buffer) => {
                if buffer.rate != rate || buffer.channels != channels {
                    return Err("PCM must match device rate and channels");
                }
            }
        }
        Ok(Self {
            source,
            rate,
            channels: usize::from(channels),
            position: 0,
            phase: 0.0,
            shared,
        })
    }

    /// Handles arbitrary frame counts. A malformed trailing partial frame is silenced.
    /// Only bounded slice access, arithmetic, sample conversion and atomic loads/stores.
    pub fn render<T: Copy>(&mut self, output: &mut [T], convert: impl Fn(f32) -> T) {
        output.fill(convert(0.0));
        let count = output.len() / self.channels;
        let active = self.shared.enabled.load(Ordering::Acquire)
            && !self.shared.failed.load(Ordering::Acquire);
        let length = match &self.source {
            PlaybackSource::Tone { frames, .. } => *frames,
            PlaybackSource::Buffer(buffer) => (buffer.samples.len() / self.channels) as u64,
        };
        if active {
            for frame in output.chunks_exact_mut(self.channels) {
                if self.position >= length {
                    break;
                }
                match &self.source {
                    PlaybackSource::Tone {
                        frequency_hz,
                        amplitude,
                        ..
                    } => {
                        let sample = self.phase.sin() as f32 * amplitude;
                        frame.fill(convert(sample));
                        self.phase = (self.phase
                            + std::f64::consts::TAU * frequency_hz / f64::from(self.rate))
                            % std::f64::consts::TAU;
                    }
                    PlaybackSource::Buffer(buffer) => {
                        let start = self.position as usize * self.channels;
                        for (dst, &sample) in frame
                            .iter_mut()
                            .zip(&buffer.samples[start..start + self.channels])
                        {
                            *dst = convert(sample);
                        }
                    }
                }
                self.position += 1;
            }
        }
        self.shared.clock.store(
            self.shared
                .clock
                .load(Ordering::Relaxed)
                .saturating_add(count as u64),
            Ordering::Relaxed,
        );
        self.shared
            .source_frames
            .store(self.position, Ordering::Relaxed);
        // Single data-callback writer; no compare/exchange retry loop.
        self.shared.max_callback_frames.store(
            self.shared
                .max_callback_frames
                .load(Ordering::Relaxed)
                .max(count as u64),
            Ordering::Relaxed,
        );
        self.shared
            .finished
            .store(self.position == length, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared() -> Arc<Shared> {
        let shared = Arc::new(Shared::default());
        shared.enabled.store(true, Ordering::Release);
        shared
    }

    #[test]
    fn buffer_is_exact_across_variable_callbacks_and_silences_eof() {
        let pcm: Vec<_> = (0..200).map(|i| (i as f32 - 100.0) / 100.0).collect();
        let state = shared();
        let mut renderer = Renderer::new(
            PlaybackSource::Buffer(PreparedBuffer::new(pcm.clone(), 48000, 2).unwrap()),
            48000,
            2,
            state.clone(),
        )
        .unwrap();
        let mut output = vec![9.0; 240];
        let mut cursor = 0;
        for frames in [0, 1, 17, 31, 2, 64, 5] {
            let end = cursor + frames * 2;
            renderer.render(&mut output[cursor..end], |s| s);
            cursor = end;
        }
        assert_eq!(&output[..200], pcm);
        assert!(output[200..].iter().all(|&s| s == 0.0));
        assert_eq!(state.clock.load(Ordering::Relaxed), 120);
        assert_eq!(state.source_frames.load(Ordering::Relaxed), 100);
        assert!(state.finished.load(Ordering::Acquire));
    }

    #[test]
    fn tone_partition_does_not_change_phase_or_clock() {
        fn run(blocks: &[usize]) -> Vec<f32> {
            let mut renderer = Renderer::new(
                PlaybackSource::Tone {
                    frequency_hz: 440.0,
                    amplitude: 0.1,
                    frames: 1000,
                },
                48000,
                2,
                shared(),
            )
            .unwrap();
            let mut samples = vec![0.0; 2200];
            let mut cursor = 0;
            for &frames in blocks {
                renderer.render(&mut samples[cursor..cursor + frames * 2], |s| s);
                cursor += frames * 2;
            }
            samples
        }
        assert_eq!(run(&[1100]), run(&[0, 1, 31, 1024, 44]));
        let samples = run(&[1100]);
        assert!(samples[..2000].iter().any(|s| s.abs() > 0.09));
        assert!(samples[2000..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn control_gate_and_errors_silence_without_advancing_source() {
        let state = shared();
        let mut renderer = Renderer::new(
            PlaybackSource::Tone {
                frequency_hz: 440.0,
                amplitude: 0.1,
                frames: 100,
            },
            48000,
            2,
            state.clone(),
        )
        .unwrap();
        state.enabled.store(false, Ordering::Release);
        let mut output = [0_u8; 11];
        renderer.render(&mut output, <u8 as cpal::FromSample<f32>>::from_sample_);
        assert_eq!(output, [128; 11]);
        assert_eq!(renderer.position, 0);
        state.enabled.store(true, Ordering::Release);
        state.failed.store(true, Ordering::Release);
        renderer.render(&mut output, <u8 as cpal::FromSample<f32>>::from_sample_);
        assert_eq!(output, [128; 11]);
        assert_eq!(renderer.position, 0);
        assert_eq!(state.clock.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn rejects_bad_sources_before_callback() {
        for samples in [vec![], vec![f32::NAN], vec![1.1]] {
            assert!(PreparedBuffer::new(samples, 48000, 1).is_err());
        }
        assert!(PreparedBuffer::new(vec![0.0; 3], 48000, 2).is_err());
        let buffer = PreparedBuffer::new(vec![0.0; 2], 44100, 2).unwrap();
        assert!(Renderer::new(PlaybackSource::Buffer(buffer), 48000, 2, shared()).is_err());
        for hz in [f64::NAN, 0.0, 24000.0] {
            assert!(
                Renderer::new(
                    PlaybackSource::Tone {
                        frequency_hz: hz,
                        amplitude: 0.1,
                        frames: 100
                    },
                    48000,
                    2,
                    shared()
                )
                .is_err()
            );
        }
    }
}
