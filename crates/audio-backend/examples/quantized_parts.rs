// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline trace by default; optional paused live-stream construction, no sound.
use audio_backend::{
    cpal_backend::{CpalPlayback, PlaybackConfig, RequestedBuffer, SampleEncoding},
    live::{LiveEngine, Quantize, Request},
    mixer::CueSelection,
    routing::OutputRouting,
    samples::{BoundaryPolicy, LoadedSample, SampleVoice},
};
use event_engine::live::MusicalScheduler;
use parts::{AssetId, Part, PartId, PlaybackConfiguration, PlaybackMode, TimingPolicy};
use std::{error::Error, path::Path};
use timebase::{BeatTicks, TempoChange, TempoMap};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (check, path) = match args.as_slice() {
        [path] if path != "--help" => (false, path),
        [flag, path] if flag == "--check" => (true, path),
        _ => {
            println!(
                "quantized_parts [--check] <48000-Hz-WAV>\nDefault: render four seconds offline, print exact action frames. --check: build a paused stereo CPAL stream only. No playback."
            );
            return Ok(());
        }
    };
    let sample = LoadedSample::load(AssetId(1), Path::new(path), 48000)?;
    let part = Part {
        id: PartId(1),
        playback: PlaybackConfiguration {
            asset: AssetId(1),
            mode: PlaybackMode::Loop {
                start: 0,
                end: sample.frames() as u64,
            },
        },
        timing: TimingPolicy {
            nominal_length: BeatTicks(960),
            quantum: BeatTicks(960),
        },
    };
    let voice = SampleVoice::prepare(part, sample, 48000, BoundaryPolicy::Fade { frames: 32 })?;
    let map = TempoMap::new(
        48000,
        &[TempoChange {
            at: BeatTicks(0),
            bpm: 120.0,
        }],
    )?;
    let scheduler = MusicalScheduler::new(
        map,
        [BeatTicks(0), BeatTicks(-960)],
        [BeatTicks(7680), BeatTicks(3840)],
    )?;
    let (mut engine, mut commands, mixer) = LiveEngine::new(scheduler, vec![voice])?;
    if check {
        let stream = CpalPlayback::open_live(
            None,
            PlaybackConfig {
                sample_rate_hz: 48000,
                channels: 2,
                encoding: SampleEncoding::F32,
                buffer: RequestedBuffer::Default,
            },
            OutputRouting::stereo_master_only(),
            engine,
        )?;
        println!(
            "Live scheduling stream built paused: {} {:?}; no playback",
            stream.device_name(),
            stream.config()
        );
        return Ok(());
    }
    mixer.set_cue(CueSelection::B);
    let route = OutputRouting::new(4, [0, 1], Some([2, 3]))?;
    engine.render(&mut [0.0; 4], route);
    commands.submit(0, 0, Request::Launch(0), Quantize::Beat)?;
    commands.submit(1, 0, Request::Launch(0), Quantize::Phrase)?;
    commands.submit(0, 0, Request::Tempo(60.0), Quantize::Beat)?;
    let mut block = [0.0; 127 * 4];
    let mut remaining = 48000 * 4 - 1;
    while remaining > 0 {
        let frames = remaining.min(127);
        engine.render(&mut block[..frames * 4], route);
        for event in engine.events() {
            println!("{event:?}");
        }
        remaining -= frames;
    }
    if engine.failed() {
        return Err("offline engine failed".into());
    }
    println!(
        "Global frame: {}; deck phases: {:?}, {:?}",
        engine.frame().0,
        engine.scheduler().phase(0)?,
        engine.scheduler().phase(1)?
    );
    Ok(())
}
