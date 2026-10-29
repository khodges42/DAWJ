// SPDX-License-Identifier: GPL-3.0-or-later
//! Output discovery and minimal realtime playback.
//! Public snapshots contain no CPAL handles or hardware-specific assumptions.

use std::fmt;

pub mod cpal_backend;
pub mod playback;

/// Buffer lengths in frames, not samples or bytes. Unknown is not a fixed size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BufferCapabilities {
    Range { min: u32, max: u32 },
    Unknown,
}

/// A correlated configuration: do not combine channels/rates from different rows.
/// Rates are the inclusive range reported by the backend, not a probed list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputConfig {
    pub channels: u16,
    pub min_sample_rate_hz: u32,
    pub max_sample_rate_hz: u32,
    pub buffer: BufferCapabilities,
    pub sample_format: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectionError {
    pub operation: &'static str,
    pub message: String,
}

impl fmt::Display for InspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.operation, self.message)
    }
}

impl std::error::Error for InspectionError {}

/// Ordinals identify entries only within this snapshot. Names may be duplicated.
/// Individual query failures are preserved rather than hiding the device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDevice {
    pub ordinal: usize,
    pub name: Result<String, InspectionError>,
    pub configurations: Result<Vec<OutputConfig>, InspectionError>,
    /// One rate (min == max), with the backend's buffer capability, not a selected size.
    pub default_configuration: Result<OutputConfig, InspectionError>,
}

pub trait OutputDiscovery {
    /// Empty success means no outputs. Enumeration failure is a separate result.
    fn output_devices(&self) -> Result<Vec<OutputDevice>, InspectionError>;
}
