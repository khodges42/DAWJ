// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{
    cpal_backend::{CpalPlayback, PlaybackConfig, RequestedBuffer, SampleEncoding},
    playback::{PlaybackSource, PreparedBuffer},
};
use event_engine::{Impulse, OfflineScheduler};
use std::{
    error::Error,
    time::{Duration, Instant},
};
use timebase::{BeatTicks, SampleFrame, TempoChange, TempoMap};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--help") {
        println!(
            "Usage: playback tone|buffer [default|output-ordinal] [rate-Hz] [channels] [default|buffer-frames]\nDefaults: tone default 48000 2 default; f32, two seconds, low-level 440 Hz tone or four M0 impulses. All requested channels carry the same signal."
        );
        return Ok(());
    }
    if args.len() > 5 {
        return Err("too many arguments; use --help".into());
    }
    let mode = args.first().map_or("tone", String::as_str);
    let ordinal = match args.get(1).map(String::as_str) {
        None | Some("default") => None,
        Some(v) => Some(v.parse()?),
    };
    let rate: u32 = args.get(2).map_or(Ok(48_000), |v| v.parse())?;
    let channels: u16 = args.get(3).map_or(Ok(2), |v| v.parse())?;
    // Bound demo allocations on the control thread.
    if !(8000..=192000).contains(&rate) || !(1..=8).contains(&channels) {
        return Err("demo rate/channels out of bounds".into());
    }
    let buffer = match args.get(4).map(String::as_str) {
        None | Some("default") => RequestedBuffer::Default,
        Some(v) => RequestedBuffer::Fixed(v.parse()?),
    };
    let source = match mode {
        "tone" => PlaybackSource::Tone {
            frequency_hz: 440.0,
            amplitude: 0.1,
            frames: u64::from(rate) * 2,
        },
        "buffer" => {
            let map = TempoMap::new(
                rate,
                &[TempoChange {
                    at: BeatTicks(0),
                    bpm: 120.0,
                }],
            )?;
            let impulses: Vec<_> = (0..4)
                .map(|beat| Impulse {
                    at: BeatTicks(beat * 960),
                    amplitude: 0.25,
                })
                .collect();
            let mut scheduler = OfflineScheduler::new(&map, &impulses, SampleFrame(0))?;
            let mut mono = vec![0.0; rate as usize * 2];
            for block in mono.chunks_mut(127) {
                scheduler.render(block)?;
            }
            let interleaved = mono
                .iter()
                .flat_map(|&s| std::iter::repeat_n(s, usize::from(channels)))
                .collect();
            PlaybackSource::Buffer(PreparedBuffer::new(interleaved, rate, channels)?)
        }
        _ => return Err("mode must be tone or buffer".into()),
    };
    let config = PlaybackConfig {
        sample_rate_hz: rate,
        channels,
        encoding: SampleEncoding::F32,
        buffer,
    };
    let mut stream = CpalPlayback::open(ordinal, config, source)?;
    println!(
        "Device: {}; requested settings: {:?}; mode: {mode}",
        stream.device_name(),
        stream.config()
    );
    let monitor = stream.monitor();
    stream.start()?;
    stream.start()?; // Idempotent while running.
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        let status = monitor.status();
        if status.failed {
            return Err("CPAL asynchronous stream error (stream will be dropped)".into());
        }
        if status.finished {
            break;
        }
        if Instant::now() >= deadline {
            return Err("timed out waiting for audio callbacks".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    // Allow queued endpoint audio to drain; this is not a hardware presentation timestamp.
    std::thread::sleep(Duration::from_millis(200));
    stream.stop()?;
    stream.stop()?; // Idempotent terminal stop.
    if stream.start().is_ok() {
        return Err("stopped stream unexpectedly restarted".into());
    }
    let status = monitor.status();
    println!("{status:?}");
    if status.failed {
        return Err("CPAL asynchronous stream error".into());
    }
    Ok(())
}
