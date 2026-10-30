// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared non-realtime demo/device setup. Construct CPAL on its owning thread.
use crate::Coordinator;
use audio_backend::{
    OutputDevice, OutputDiscovery,
    cpal_backend::{CpalDiscovery, CpalPlayback, PlaybackConfig, RequestedBuffer, SampleEncoding},
    demo::{self, RATE},
    routing::OutputRouting,
};
pub struct OutputSession {
    pub stream: CpalPlayback,
    pub coordinator: Coordinator,
}
pub fn discover() -> Result<Vec<OutputDevice>, String> {
    CpalDiscovery::default()
        .output_devices()
        .map_err(|e| e.to_string())
}
impl OutputSession {
    pub fn open(device: &OutputDevice, cue: bool, phrase_bars: u8) -> Result<Self, String> {
        let channels = if cue { 4 } else { 2 };
        let configs = device
            .configurations
            .as_ref()
            .map_err(ToString::to_string)?;
        let encoding = [
            SampleEncoding::F32,
            SampleEncoding::I16,
            SampleEncoding::I32,
            SampleEncoding::U8,
        ]
        .into_iter()
        .find(|encoding| {
            let name = match encoding {
                SampleEncoding::F32 => "f32",
                SampleEncoding::I16 => "i16",
                SampleEncoding::I32 => "i32",
                SampleEncoding::U8 => "u8",
            };
            configs.iter().any(|c| {
                c.channels == channels
                    && c.sample_format == name
                    && (c.min_sample_rate_hz..=c.max_sample_rate_hz).contains(&RATE)
            })
        })
        .ok_or(
            "Unsupported configuration: demo requires 48 kHz, 2 or 4 channels, f32/i16/i32/u8",
        )?;
        let fresh = discover()?;
        if fresh
            .iter()
            .find(|d| d.ordinal == device.ordinal)
            .map(|d| &d.name)
            != Some(&device.name)
        {
            return Err("Output enumeration changed. Rescan and select the output again.".into());
        }
        let routing = OutputRouting::new(channels, [0, 1], cue.then_some([2, 3]))?;
        let (engine, control, mixer) = demo::prepare_with_phrase_bars(phrase_bars)
            .map_err(|e| format!("Demo asset / engine preparation failed: {e}"))?;
        let coordinator = Coordinator::new(control, mixer, cue);
        let mut stream = CpalPlayback::open_live(
            Some(device.ordinal),
            PlaybackConfig {
                sample_rate_hz: RATE,
                channels,
                encoding,
                buffer: RequestedBuffer::Default,
            },
            routing,
            engine,
        )
        .map_err(|e| e.to_string())?;
        stream.start().map_err(|e| e.to_string())?;
        Ok(Self {
            stream,
            coordinator,
        })
    }
    pub fn stop(&mut self) -> Result<(), String> {
        self.coordinator.close();
        self.stream.stop().map_err(|e| e.to_string())
    }
}
