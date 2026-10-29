// SPDX-License-Identifier: GPL-3.0-or-later
//! A small mono PCM16 WAV fixture, written outside the rendering loop.
use event_engine::{Impulse, MAX_BLOCK_FRAMES, OfflineScheduler};
use std::{
    env,
    error::Error,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};
use timebase::{BeatTicks, SampleFrame, TempoChange, TempoMap};

fn write_wav(path: &Path, rate: u32, samples: &[f32]) -> Result<(), Box<dyn Error>> {
    let data_bytes = u32::try_from(samples.len())?
        .checked_mul(2)
        .ok_or("WAV too large")?;
    let riff_size = data_bytes.checked_add(36).ok_or("WAV too large")?;
    let mut file = BufWriter::new(File::create(path)?);
    file.write_all(b"RIFF")?;
    file.write_all(&riff_size.to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&rate.to_le_bytes())?;
    file.write_all(&(rate * 2).to_le_bytes())?;
    file.write_all(&2_u16.to_le_bytes())?;
    file.write_all(&16_u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_bytes.to_le_bytes())?;
    for &sample in samples {
        file.write_all(&((sample * f32::from(i16::MAX)).round() as i16).to_le_bytes())?;
    }
    file.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Usage: offline-render [output.wav] [sample-rate] [block-frames]\nDefaults: impulses.wav 48000 128; block frames: 1..={MAX_BLOCK_FRAMES}"
        );
        return Ok(());
    }
    if args.len() > 3 {
        return Err("too many arguments; use --help".into());
    }
    let path = args.first().map_or("impulses.wav", String::as_str);
    let rate: u32 = args.get(1).map_or(Ok(48_000), |arg| arg.parse())?;
    let block: usize = args.get(2).map_or(Ok(128), |arg| arg.parse())?;
    if !(1..=MAX_BLOCK_FRAMES).contains(&block) {
        return Err("block frames outside supported range".into());
    }
    let map = TempoMap::new(
        rate,
        &[
            TempoChange {
                at: BeatTicks(0),
                bpm: 140.0,
            },
            TempoChange {
                at: BeatTicks(8 * 960),
                bpm: 120.0,
            },
        ],
    )?;
    let impulses: Vec<_> = (0..16)
        .map(|quarter| Impulse {
            at: BeatTicks(quarter * 960),
            amplitude: if quarter % 4 == 0 { 0.5 } else { 0.25 },
        })
        .collect();
    let mut scheduler = OfflineScheduler::new(&map, &impulses, SampleFrame(0))?;
    let mut samples = vec![0.0; usize::try_from(map.frame_at(BeatTicks(16 * 960))?.0)?];
    for output in samples.chunks_mut(block) {
        scheduler.render(output)?;
    }
    write_wav(Path::new(path), rate, &samples)?;
    println!(
        "Wrote {path}: {} mono frames at {rate} Hz, block {block}",
        samples.len()
    );
    for impulse in impulses {
        println!(
            "tick {} -> frame {}",
            impulse.at.0,
            map.frame_at(impulse.at)?.0
        );
    }
    Ok(())
}
