// SPDX-License-Identifier: GPL-3.0-or-later
//! Silent, real-device M1 stress harness. No sound-generating mode.
use audio_backend::{
    cpal_backend::{CpalPlayback, PlaybackConfig, RequestedBuffer, SampleEncoding},
    mixer::{CrossfadeCurve, CueSelection, MixerParameters},
    playback::PlaybackSource,
    routing::OutputRouting,
};
use std::{
    error::Error,
    time::{Duration, Instant},
};

fn open(
    seconds: u64,
    rate: u32,
    channels: u16,
    buffer: RequestedBuffer,
    ordinal: Option<usize>,
) -> Result<(CpalPlayback, audio_backend::mixer::MixerControl), Box<dyn Error>> {
    let route = OutputRouting::new(
        channels,
        [0, 1],
        if channels >= 4 { Some([2, 3]) } else { None },
    )?;
    let source = |frequency_hz| PlaybackSource::Tone {
        frequency_hz,
        amplitude: 0.0,
        frames: seconds * u64::from(rate),
    };
    Ok(CpalPlayback::open_routed_mixer(
        ordinal,
        PlaybackConfig {
            sample_rate_hz: rate,
            channels,
            encoding: SampleEncoding::F32,
            buffer,
        },
        route,
        source(220.0),
        source(330.0),
    )?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--probe") && args.len() == 1 {
        let mut accepted = 0;
        for rate in [44100, 48000, 96000] {
            for channels in [2, 4] {
                for buffer in [
                    RequestedBuffer::Default,
                    RequestedBuffer::Fixed(128),
                    RequestedBuffer::Fixed(512),
                    RequestedBuffer::Fixed(0),
                ] {
                    match open(1, rate, channels, buffer, None) {
                        Ok((stream, _)) => {
                            if matches!(buffer, RequestedBuffer::Fixed(0)) {
                                return Err("invalid zero buffer accepted".into());
                            }
                            accepted += 1;
                            println!(
                                "PROBE {:?} {} {:?}: paused build OK, {}",
                                rate,
                                channels,
                                buffer,
                                stream.device_name()
                            );
                        }
                        Err(e) => {
                            println!("PROBE {:?} {} {:?}: rejected: {e}", rate, channels, buffer)
                        }
                    }
                }
            }
        }
        if accepted == 0 {
            return Err("no tested configuration could build on the default endpoint".into());
        }
        return Ok(());
    }
    if args.first().map(String::as_str) != Some("--run") || args.len() > 6 {
        println!(
            "Silent stream soak: --run [seconds=900] [rate=48000] [channels=2] [default|buffer-frames] [default|ordinal]\n--probe: paused configuration matrix on default device. --run completes a soak, drops the stream, then opens a fresh one-second stream. No audible signal; no dropout guarantee."
        );
        return Ok(());
    }
    let seconds: u64 = args.get(1).map_or(Ok(900), |v| v.parse())?;
    let rate: u32 = args.get(2).map_or(Ok(48000), |v| v.parse())?;
    let channels: u16 = args.get(3).map_or(Ok(2), |v| v.parse())?;
    if !(1..=3600).contains(&seconds) || !(8000..=192000).contains(&rate) {
        return Err("seconds/rate outside harness bounds".into());
    }
    let buffer = match args.get(4).map(String::as_str) {
        None | Some("default") => RequestedBuffer::Default,
        Some(v) => RequestedBuffer::Fixed(v.parse()?),
    };
    let ordinal = match args.get(5).map(String::as_str) {
        None | Some("default") => None,
        Some(v) => Some(v.parse()?),
    };
    for (cycle, duration) in [seconds, 1].into_iter().enumerate() {
        let (mut stream, controls) = open(duration, rate, channels, buffer, ordinal)?;
        println!(
            "START cycle={cycle} seconds={duration} device={} config={:?} route={:?}",
            stream.device_name(),
            stream.config(),
            stream.routing()
        );
        let monitor = stream.monitor();
        stream.start()?;
        stream.start()?;
        let start = Instant::now();
        let mut previous = 0;
        let mut progress = Instant::now();
        let mut updates = 0_u64;
        let mut last_report = 0;
        loop {
            let status = monitor.status();
            if status.failed || status.sample_clock < previous {
                return Err("stream failure or regressed clock".into());
            }
            if status.sample_clock > previous {
                progress = Instant::now();
            }
            previous = status.sample_clock;
            if progress.elapsed() > Duration::from_secs(5)
                || start.elapsed() > Duration::from_secs(duration + 10)
            {
                return Err("callback progress timeout".into());
            }
            if status.finished {
                break;
            }
            let phase = (status.source_frames % (u64::from(rate) * 4)) as f32 / (rate as f32 * 4.0);
            controls.set(MixerParameters {
                gain_a: 0.25 + phase * 0.5,
                gain_b: 0.75 - phase * 0.5,
                crossfader: phase,
                curve: if updates % 200 < 100 {
                    CrossfadeCurve::EqualPower
                } else {
                    CrossfadeCurve::Linear
                },
            })?;
            controls.set_cue(
                [
                    CueSelection::Off,
                    CueSelection::A,
                    CueSelection::B,
                    CueSelection::Both,
                ][(updates / 50 % 4) as usize],
            );
            updates += 1;
            let elapsed = start.elapsed().as_secs();
            if elapsed / 30 > last_report {
                last_report = elapsed / 30;
                println!("PROGRESS elapsed={elapsed} updates={updates} {status:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        stream.stop()?;
        stream.stop()?;
        if stream.start().is_ok() {
            return Err("terminal stream restarted".into());
        }
        drop(stream);
        let status = monitor.status();
        if controls.peaks() != [0.0; 2] || controls.cue_peaks() != [0.0; 2] {
            return Err("silent soak produced nonzero mixer samples".into());
        }
        if status.failed
            || status.source_frames != duration * u64::from(rate)
            || status.callbacks == 0
        {
            return Err("incomplete soak".into());
        }
        let clock = status.sample_clock;
        std::thread::sleep(Duration::from_millis(50));
        if monitor.status().sample_clock != clock {
            return Err("callbacks continued after stream destruction".into());
        }
        println!(
            "PASS cycle={cycle} wall_seconds={:.3} updates={updates} {status:?}; silent samples only; dropout count unmeasured",
            start.elapsed().as_secs_f64()
        );
    }
    Ok(())
}
