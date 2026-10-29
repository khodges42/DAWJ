// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{
    cpal_backend::{CpalPlayback, PlaybackConfig, RequestedBuffer, SampleEncoding},
    mixer::{MixerParameters, StereoMixer},
    playback::PlaybackSource,
    samples::{BoundaryPolicy, LoadedSample, SampleDeck},
};
use parts::{
    AssetId, Command, Deck, Part, PartId, PlaybackConfiguration, PlaybackMode, TimingPolicy,
};
use std::{
    error::Error,
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
use timebase::BeatTicks;
fn wav(path: &Path, samples: &[f32], channels: u16) -> Result<(), Box<dyn Error>> {
    let mut f = std::fs::File::create(path)?;
    let size = u32::try_from(samples.len() * 2)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + size).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16_u32.to_le_bytes())?;
    for field in [1, channels] {
        f.write_all(&field.to_le_bytes())?;
    }
    f.write_all(&48000_u32.to_le_bytes())?;
    f.write_all(&(48000 * u32::from(channels) * 2).to_le_bytes())?;
    f.write_all(&(channels * 2).to_le_bytes())?;
    f.write_all(&16_u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&size.to_le_bytes())?;
    for s in samples {
        f.write_all(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "--fixture" {
        let samples: Vec<_> = (0..4800)
            .map(|i| (f64::from(i) * std::f64::consts::TAU * 220.0 / 48000.0).sin() as f32 * 0.02)
            .collect();
        return wav(Path::new(&args[1]), &samples, 1);
    }
    if args.len() < 2
        || !matches!(args[0].as_str(), "--render" | "--check" | "--play")
        || (args[0] == "--render" && args.len() != 3)
        || (args[0] != "--render" && args.len() != 2)
    {
        println!(
            "sample_parts --fixture output.wav | --render input.wav output.wav | --check input.wav | --play input.wav\n48 kHz WAV required. Render loops for 2 seconds offline. Check builds a paused stream only. Play explicitly starts 4 seconds of very quiet loop playback after a three-second countdown; lower device volume first."
        );
        return Ok(());
    }
    let asset = LoadedSample::load(AssetId(1), Path::new(&args[1]), 48000)?;
    let part = Part {
        id: PartId(1),
        playback: PlaybackConfiguration {
            asset: AssetId(1),
            mode: PlaybackMode::Loop {
                start: 0,
                end: asset.frames() as u64,
            },
        },
        timing: TimingPolicy {
            nominal_length: BeatTicks(960),
            quantum: BeatTicks(240),
        },
    };
    let mut model = Deck::new(BeatTicks(0), BeatTicks(3840))?;
    model.command(
        0,
        Command::Start {
            part,
            at: BeatTicks(0),
        },
    )?;
    model.advance(BeatTicks(0))?;
    let mut assets = std::array::from_fn(|_| None);
    assets[0] = Some(asset);
    let deck = SampleDeck::prepare(&model, assets, 48000, BoundaryPolicy::Fade { frames: 32 })?;
    let source = PlaybackSource::Parts(Box::new(deck));
    let silence = PlaybackSource::Tone {
        frequency_hz: 220.0,
        amplitude: 0.0,
        frames: u64::MAX,
    };
    if args[0] == "--render" {
        let (mut mixer, _) = StereoMixer::new(source, silence, 48000)?;
        let mut out = vec![0.0; 48000 * 2 * 2];
        for chunk in out.chunks_mut(254) {
            mixer.render(chunk);
        }
        wav(Path::new(&args[2]), &out, 2)?;
        println!("Rendered 96000 stereo frames offline");
        return Ok(());
    }
    let (mut stream, control) = CpalPlayback::open_mixer(
        None,
        PlaybackConfig {
            sample_rate_hz: 48000,
            channels: 2,
            encoding: SampleEncoding::F32,
            buffer: RequestedBuffer::Default,
        },
        source,
        silence,
    )?;
    println!("{} {:?}", stream.device_name(), stream.config());
    if args[0] == "--check" {
        println!("Sample-backed stream built paused; no playback");
        return Ok(());
    }
    for second in (1..=3).rev() {
        println!("Starts in {second}...");
        std::thread::sleep(Duration::from_secs(1));
    }
    let monitor = stream.monitor();
    stream.start()?;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(4) {
        if monitor.status().failed {
            return Err("sample playback stream failed".into());
        }
        if start.elapsed() > Duration::from_millis(3800) {
            control.set(MixerParameters {
                gain_a: 0.0,
                gain_b: 0.0,
                ..MixerParameters::default()
            })?;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    stream.stop()?;
    println!("{:?}", monitor.status());
    Ok(())
}
