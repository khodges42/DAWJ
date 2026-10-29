// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only enumeration by default; optional paused-stream build check, never playback.
use audio_backend::{
    OutputDiscovery,
    cpal_backend::{CpalDiscovery, CpalPlayback, PlaybackConfig, RequestedBuffer, SampleEncoding},
    playback::PlaybackSource,
    routing::OutputRouting,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() {
        println!("WASAPI on Windows; stream-channel capabilities, physical outputs unverified:");
        for device in CpalDiscovery::default().output_devices()? {
            println!("{}: {:?}", device.ordinal, device.name);
            match device.configurations {
                Ok(rows) => {
                    for row in rows {
                        println!(
                            "  {} channels, {}..={} Hz, {}, {:?}",
                            row.channels,
                            row.min_sample_rate_hz,
                            row.max_sample_rate_hz,
                            row.sample_format,
                            row.buffer
                        );
                    }
                }
                Err(e) => println!("  inspection failed: {e}"),
            }
        }
        println!(
            "Optional silent build check: routing_check --check <ordinal> <channels>\nUses 48 kHz f32, master [0,1]; channels >= 4 request cue [2,3], otherwise master-only. Never starts playback."
        );
        return Ok(());
    }
    if args.len() != 3 || args[0] != "--check" {
        return Err("usage: routing_check [--check <ordinal> <channels>]".into());
    }
    let ordinal = args[1].parse()?;
    let channels: u16 = args[2].parse()?;
    let routing = OutputRouting::new(
        channels,
        [0, 1],
        if channels >= 4 { Some([2, 3]) } else { None },
    )?;
    let source = || PlaybackSource::Tone {
        frequency_hz: 220.0,
        amplitude: 0.0,
        frames: 48000,
    };
    let config = PlaybackConfig {
        sample_rate_hz: 48000,
        channels,
        encoding: SampleEncoding::F32,
        buffer: RequestedBuffer::Default,
    };
    let (stream, _) =
        CpalPlayback::open_routed_mixer(Some(ordinal), config, routing, source(), source())?;
    println!(
        "Paused stream built: {}; {:?}; {:?}; {:?}. No playback; physical output isolation unverified.",
        stream.device_name(),
        stream.config(),
        stream.routing(),
        routing.cue_availability()
    );
    Ok(())
}
