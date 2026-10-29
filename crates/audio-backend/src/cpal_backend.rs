// SPDX-License-Identifier: GPL-3.0-or-later
//! CPAL adapter. On Windows the default host is WASAPI; ASIO is not enabled.
use crate::mixer::{MixerControl, StereoMixer};
use crate::playback::{PlaybackMonitor, PlaybackSource, Renderer, Shared};
use cpal::traits::StreamTrait;
use cpal::traits::{DeviceTrait, HostTrait};
use std::sync::{Arc, atomic::Ordering};

#[derive(Clone, Copy, Debug)]
pub enum SampleEncoding {
    F32,
    I16,
    I32,
    U8,
}

impl SampleEncoding {
    fn cpal(self) -> cpal::SampleFormat {
        match self {
            Self::F32 => cpal::SampleFormat::F32,
            Self::I16 => cpal::SampleFormat::I16,
            Self::I32 => cpal::SampleFormat::I32,
            Self::U8 => cpal::SampleFormat::U8,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RequestedBuffer {
    Default,
    Fixed(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct PlaybackConfig {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub encoding: SampleEncoding,
    pub buffer: RequestedBuffer,
}

/// Control-thread owner. Stop is terminal; create a new stream to restart.
pub struct CpalPlayback {
    stream: cpal::Stream,
    monitor: PlaybackMonitor,
    started: bool,
    stopped: bool,
    device_name: String,
    config: PlaybackConfig,
}

impl CpalPlayback {
    pub fn open_mixer(
        ordinal: Option<usize>,
        config: PlaybackConfig,
        a: PlaybackSource,
        b: PlaybackSource,
    ) -> Result<(Self, MixerControl), InspectionError> {
        if config.channels != 2 {
            return Err(error(
                "configure mixer",
                "stereo master requires two output channels",
            ));
        }
        let (mixer, control) =
            StereoMixer::new(a, b, config.sample_rate_hz).map_err(|e| error("prepare mixer", e))?;
        let playback = Self::open_inner(ordinal, config, Input::Mixer(mixer))?;
        Ok((playback, control))
    }
    /// None selects the OS default output; Some selects a fresh enumeration ordinal.
    /// Configuration is explicit and checked without fallback or resampling.
    pub fn open(
        ordinal: Option<usize>,
        config: PlaybackConfig,
        source: PlaybackSource,
    ) -> Result<Self, InspectionError> {
        Self::open_inner(ordinal, config, Input::Single(source))
    }

    fn open_inner(
        ordinal: Option<usize>,
        config: PlaybackConfig,
        input: Input,
    ) -> Result<Self, InspectionError> {
        let host = cpal::default_host();
        let device = match ordinal {
            Some(index) => host
                .output_devices()
                .map_err(|e| error("enumerate outputs", e))?
                .nth(index),
            None => host.default_output_device(),
        }
        .ok_or_else(|| error("select output", "output device unavailable"))?;
        let device_name = device.name().map_err(|e| error("device name", e))?;
        let supported = device
            .supported_output_configs()
            .map_err(|e| error("supported configurations", e))?
            .any(|row| {
                row.channels() == config.channels
                    && row.sample_format() == config.encoding.cpal()
                    && (row.min_sample_rate().0..=row.max_sample_rate().0)
                        .contains(&config.sample_rate_hz)
                    && match config.buffer {
                        RequestedBuffer::Default => true,
                        RequestedBuffer::Fixed(0) => false,
                        RequestedBuffer::Fixed(size) => match *row.buffer_size() {
                            cpal::SupportedBufferSize::Range { min, max } => {
                                (min..=max).contains(&size)
                            }
                            cpal::SupportedBufferSize::Unknown => true, // Backend decides at build time.
                        },
                    }
            });
        if !supported {
            return Err(error(
                "configure output",
                "unsupported requested configuration",
            ));
        }
        let shared = Arc::new(Shared::default());
        let renderer = match input {
            Input::Single(source) => Engine::Single(
                Renderer::new(
                    source,
                    config.sample_rate_hz,
                    config.channels,
                    shared.clone(),
                )
                .map_err(|e| error("prepare renderer", e))?,
            ),
            Input::Mixer(mixer) => Engine::Mixer(mixer, shared.clone()),
        };
        let stream_config = cpal::StreamConfig {
            channels: config.channels,
            sample_rate: cpal::SampleRate(config.sample_rate_hz),
            buffer_size: match config.buffer {
                RequestedBuffer::Default => cpal::BufferSize::Default,
                RequestedBuffer::Fixed(frames) => cpal::BufferSize::Fixed(frames),
            },
        };
        let stream = match config.encoding {
            SampleEncoding::F32 => build::<f32>(&device, &stream_config, renderer, shared.clone()),
            SampleEncoding::I16 => build::<i16>(&device, &stream_config, renderer, shared.clone()),
            SampleEncoding::I32 => build::<i32>(&device, &stream_config, renderer, shared.clone()),
            SampleEncoding::U8 => build::<u8>(&device, &stream_config, renderer, shared.clone()),
        }
        .map_err(|e| error("build output stream", e))?;
        Ok(Self {
            stream,
            monitor: PlaybackMonitor(shared),
            started: false,
            stopped: false,
            device_name,
            config,
        })
    }

    pub fn monitor(&self) -> PlaybackMonitor {
        self.monitor.clone()
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn config(&self) -> PlaybackConfig {
        self.config
    }

    pub fn start(&mut self) -> Result<(), InspectionError> {
        if self.stopped || self.monitor.status().failed {
            return Err(error("start", "stream stopped or failed"));
        }
        if self.started {
            return Ok(());
        }
        self.monitor.0.enabled.store(true, Ordering::Release);
        if let Err(e) = self.stream.play() {
            self.monitor.0.enabled.store(false, Ordering::Release);
            self.monitor.0.failed.store(true, Ordering::Release);
            return Err(error("start output stream", e));
        }
        self.started = true;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), InspectionError> {
        self.monitor.0.enabled.store(false, Ordering::Release);
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        self.stream
            .pause()
            .map_err(|e| error("pause output stream", e))
    }
}

impl Drop for CpalPlayback {
    fn drop(&mut self) {
        self.monitor.0.enabled.store(false, Ordering::Release);
    }
}

enum Input {
    Single(PlaybackSource),
    Mixer(StereoMixer),
}
enum Engine {
    Single(Renderer),
    Mixer(StereoMixer, Arc<Shared>),
}

impl Engine {
    fn render<T: Copy>(&mut self, output: &mut [T], convert: impl Fn(f32) -> T) {
        match self {
            // The legacy single-source path also receives the conservative sample ceiling.
            Self::Single(renderer) => renderer.render(output, |s| {
                convert(s.clamp(-crate::mixer::MASTER_CEILING, crate::mixer::MASTER_CEILING))
            }),
            Self::Mixer(mixer, shared) => {
                let count = (output.len() / 2) as u64;
                if shared.enabled.load(Ordering::Acquire) && !shared.failed.load(Ordering::Acquire)
                {
                    mixer.render_converted(output, convert);
                } else {
                    output.fill(convert(0.0));
                }
                shared.clock.store(
                    shared.clock.load(Ordering::Relaxed).saturating_add(count),
                    Ordering::Relaxed,
                );
                shared.source_frames.store(
                    mixer.positions().into_iter().max().unwrap_or(0),
                    Ordering::Relaxed,
                );
                shared.finished.store(mixer.finished(), Ordering::Release);
                shared.max_callback_frames.store(
                    shared
                        .max_callback_frames
                        .load(Ordering::Relaxed)
                        .max(count),
                    Ordering::Relaxed,
                );
            }
        }
    }
}

fn build<T: cpal::SizedSample + cpal::FromSample<f32> + Copy>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut renderer: Engine,
    shared: Arc<Shared>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    device.build_output_stream(
        config,
        move |data: &mut [T], _| renderer.render(data, T::from_sample),
        move |_| {
            shared.failed.store(true, Ordering::Release);
            shared.enabled.store(false, Ordering::Release);
        },
        None,
    )
}

use crate::{BufferCapabilities, InspectionError, OutputConfig, OutputDevice, OutputDiscovery};

pub struct CpalDiscovery {
    host: cpal::Host,
}

impl Default for CpalDiscovery {
    fn default() -> Self {
        Self {
            host: cpal::default_host(),
        }
    }
}

fn error(operation: &'static str, cause: impl std::fmt::Display) -> InspectionError {
    InspectionError {
        operation,
        message: cause.to_string(),
    }
}

fn buffer(value: &cpal::SupportedBufferSize) -> BufferCapabilities {
    match *value {
        cpal::SupportedBufferSize::Range { min, max } => BufferCapabilities::Range { min, max },
        cpal::SupportedBufferSize::Unknown => BufferCapabilities::Unknown,
    }
}

fn configuration(value: cpal::SupportedStreamConfigRange) -> OutputConfig {
    OutputConfig {
        channels: value.channels(),
        min_sample_rate_hz: value.min_sample_rate().0,
        max_sample_rate_hz: value.max_sample_rate().0,
        buffer: buffer(value.buffer_size()),
        sample_format: value.sample_format().to_string(),
    }
}

impl OutputDiscovery for CpalDiscovery {
    fn output_devices(&self) -> Result<Vec<OutputDevice>, InspectionError> {
        let devices = self
            .host
            .output_devices()
            .map_err(|e| error("enumerate outputs", e))?;
        Ok(devices
            .enumerate()
            .map(|(ordinal, device)| OutputDevice {
                ordinal,
                name: device.name().map_err(|e| error("device name", e)),
                configurations: device
                    .supported_output_configs()
                    .map(|configs| configs.map(configuration).collect())
                    .map_err(|e| error("supported output configurations", e)),
                default_configuration: device
                    .default_output_config()
                    .map(|config| OutputConfig {
                        channels: config.channels(),
                        min_sample_rate_hz: config.sample_rate().0,
                        max_sample_rate_hz: config.sample_rate().0,
                        buffer: buffer(config.buffer_size()),
                        sample_format: config.sample_format().to_string(),
                    })
                    .map_err(|e| error("default output configuration", e)),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_gates_mixer_and_caps_legacy_output() {
        use crate::playback::PreparedBuffer;
        let source =
            || PlaybackSource::Buffer(PreparedBuffer::new(vec![1.0; 4000], 48000, 2).unwrap());
        let shared = Arc::new(Shared::default());
        let (mixer, _) = StereoMixer::new(source(), source(), 48000).unwrap();
        let mut engine = Engine::Mixer(mixer, shared.clone());
        let mut samples = [0_u8; 64];
        engine.render(&mut samples, <u8 as cpal::FromSample<f32>>::from_sample_);
        assert_eq!(samples, [128; 64]);
        assert_eq!(shared.source_frames.load(Ordering::Relaxed), 0);
        shared.enabled.store(true, Ordering::Release);
        engine.render(&mut samples, <u8 as cpal::FromSample<f32>>::from_sample_);
        assert_eq!(shared.source_frames.load(Ordering::Relaxed), 32);
        shared.failed.store(true, Ordering::Release);
        engine.render(&mut samples, <u8 as cpal::FromSample<f32>>::from_sample_);
        assert_eq!(samples, [128; 64]);
        assert_eq!(shared.source_frames.load(Ordering::Relaxed), 32);
        let renderer = Renderer::new(source(), 48000, 2, shared.clone()).unwrap();
        shared.failed.store(false, Ordering::Release);
        let mut engine = Engine::Single(renderer);
        let mut samples = [0.0; 64];
        engine.render(&mut samples, |s| s);
        assert_eq!(samples, [crate::mixer::MASTER_CEILING; 64]);
    }

    #[test]
    fn preserves_correlated_ranges_and_frame_units() {
        let value = cpal::SupportedStreamConfigRange::new(
            6,
            cpal::SampleRate(44_100),
            cpal::SampleRate(96_000),
            cpal::SupportedBufferSize::Range { min: 64, max: 2048 },
            cpal::SampleFormat::F32,
        );
        assert_eq!(
            configuration(value),
            OutputConfig {
                channels: 6,
                min_sample_rate_hz: 44_100,
                max_sample_rate_hz: 96_000,
                buffer: BufferCapabilities::Range { min: 64, max: 2048 },
                sample_format: "f32".into(),
            }
        );
    }

    #[test]
    fn unknown_buffer_and_single_rate_remain_explicit() {
        let value = cpal::SupportedStreamConfigRange::new(
            2,
            cpal::SampleRate(48_000),
            cpal::SampleRate(48_000),
            cpal::SupportedBufferSize::Unknown,
            cpal::SampleFormat::I16,
        );
        let config = configuration(value);
        assert_eq!(config.buffer, BufferCapabilities::Unknown);
        assert_eq!(config.min_sample_rate_hz, config.max_sample_rate_hz);
        assert_eq!(config.sample_format, "i16");
    }
}
