// SPDX-License-Identifier: GPL-3.0-or-later
//! CPAL adapter. On Windows the default host is WASAPI; ASIO is not enabled.
use cpal::traits::{DeviceTrait, HostTrait};

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
