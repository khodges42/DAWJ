// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{
    cpal_backend::{CpalPlayback, PlaybackConfig, RequestedBuffer, SampleEncoding},
    mixer::{CrossfadeCurve, MixerParameters},
    playback::PlaybackSource,
};
use std::{
    error::Error,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("--play") {
        println!(
            "Two-deck demo: mixer --play [default|output-ordinal]\nPlays very quiet 220/330 Hz tones for 4 seconds, A -> center -> B. Stereo 48 kHz f32. Three-second countdown before playback. Lower your headphone volume first."
        );
        return Ok(());
    }
    if args.len() > 2 {
        return Err("usage: mixer --play [default|output-ordinal]".into());
    }
    let ordinal = match args.get(1).map(String::as_str) {
        None | Some("default") => None,
        Some(v) => Some(v.parse()?),
    };
    let tone = |frequency_hz| PlaybackSource::Tone {
        frequency_hz,
        amplitude: 0.02,
        frames: 48000 * 4,
    };
    let config = PlaybackConfig {
        sample_rate_hz: 48000,
        channels: 2,
        encoding: SampleEncoding::F32,
        buffer: RequestedBuffer::Default,
    };
    let (mut stream, controls) =
        CpalPlayback::open_mixer(ordinal, config, tone(220.0), tone(330.0))?;
    println!(
        "Device: {}; {:?}. Very quiet demo starts in 3 seconds.",
        stream.device_name(),
        stream.config()
    );
    let parameters = |crossfader| MixerParameters {
        crossfader,
        curve: CrossfadeCurve::EqualPower,
        ..MixerParameters::default()
    };
    controls.set(parameters(0.0))?;
    for count in (1..=3).rev() {
        println!("{count}...");
        std::thread::sleep(Duration::from_secs(1));
    }
    let monitor = stream.monitor();
    stream.start()?;
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let status = monitor.status();
        if status.failed {
            return Err("stream failed".into());
        }
        if status.finished {
            break;
        }
        if Instant::now() >= deadline {
            return Err("callback timeout".into());
        }
        // Use the source sample clock rather than wall-time for fade position.
        let x = (status.source_frames as f32 / (48000.0 * 3.0)).min(1.0);
        controls.set(parameters(x))?;
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(200));
    stream.stop()?;
    println!(
        "{:?}; post-limiter lifetime stereo peaks: {:?}",
        monitor.status(),
        controls.peaks()
    );
    if monitor.status().failed {
        return Err("stream failed".into());
    }
    Ok(())
}
