// SPDX-License-Identifier: GPL-3.0-or-later
//! CPAL adapter. On Windows the default host is WASAPI; ASIO is not enabled.
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
    /// None selects the OS default output; Some selects a fresh enumeration ordinal.
    /// Configuration is explicit and checked without fallback or resampling.
    pub fn open(
        ordinal: Option<usize>,
        config: PlaybackConfig,
        source: PlaybackSource,
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
        let renderer = Renderer::new(
            source,
            config.sample_rate_hz,
            config.channels,
            shared.clone(),
        )
        .map_err(|e| error("prepare renderer", e))?;
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

fn build<T: cpal::SizedSample + cpal::FromSample<f32> + Copy>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut renderer: Renderer,
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
