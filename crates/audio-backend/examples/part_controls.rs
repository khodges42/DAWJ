// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline stdin controls; file I/O stays outside render. Never opens an audio device.
use audio_backend::{
    live::{LiveEngine, Quantize, Request},
    mixer::CueSelection,
    part_dsp::PartParameters,
    routing::OutputRouting,
    samples::{BoundaryPolicy, LoadedSample, SampleVoice},
};
use event_engine::live::MusicalScheduler;
use parts::{AssetId, Part, PartId, PlaybackConfiguration, PlaybackMode, TimingPolicy};
use std::{
    error::Error,
    io::{self, BufRead, Seek, SeekFrom, Write},
    path::Path,
};
use timebase::{BeatTicks, TempoChange, TempoMap};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        println!(
            "part_controls <48000-Hz-WAV> <output.wav>\nOffline controls on stdin:\nlaunch <deck 0|1> <slot 0..7>\nstop <deck> <slot>\nset <deck> <slot> <gain 0..1> <muted true|false> <cutoff-Hz|off>\nrender <frames 1..2880000>\nstatus <deck> <slot>\nquit\nOutput is four-channel PCM16: master L/R then cue L/R; cue selects deck B. No sound is started."
        );
        return Ok(());
    }
    if Path::new(&args[0]).canonicalize().ok() == Path::new(&args[1]).canonicalize().ok() {
        return Err("input and output must differ".into());
    }
    let sample = LoadedSample::load(AssetId(1), Path::new(&args[0]), 48000)?;
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
    let scheduler = MusicalScheduler::new(map, [BeatTicks(0); 2], [BeatTicks(3840); 2])?;
    let (mut engine, mut control, mixer) = LiveEngine::new(scheduler, vec![voice])?;
    mixer.set_cue(CueSelection::B);
    let routing = OutputRouting::new(4, [0, 1], Some([2, 3]))?;
    let mut file = std::fs::File::create(&args[1])?;
    file.write_all(b"RIFF")?;
    file.write_all(&36_u32.to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&4_u16.to_le_bytes())?;
    file.write_all(&48000_u32.to_le_bytes())?;
    file.write_all(&384000_u32.to_le_bytes())?;
    file.write_all(&8_u16.to_le_bytes())?;
    file.write_all(&16_u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&0_u32.to_le_bytes())?;
    let mut bytes = 0_u32;
    for line in io::stdin().lock().lines() {
        let line = line?;
        let words: Vec<_> = line.split_whitespace().collect();
        let result = (|| -> Result<(), Box<dyn Error>> {
            match words.as_slice() {
                ["quit"] => {}
                ["launch", deck, slot] | ["stop", deck, slot] => {
                    let request = if words[0] == "launch" {
                        Request::Launch(0)
                    } else {
                        Request::Stop
                    };
                    let ticket = control.submit(
                        deck.parse()?,
                        slot.parse()?,
                        request,
                        Quantize::Immediate,
                    )?;
                    println!("queued {}", ticket.id());
                }
                ["set", deck, slot, gain, muted, cutoff] => {
                    let cutoff_hz = if *cutoff == "off" {
                        None
                    } else {
                        Some(cutoff.parse()?)
                    };
                    let ticket = control.submit(
                        deck.parse()?,
                        slot.parse()?,
                        Request::Parameters(PartParameters {
                            gain: gain.parse()?,
                            muted: muted.parse()?,
                            cutoff_hz,
                        }),
                        Quantize::Immediate,
                    )?;
                    println!("queued {}", ticket.id());
                }
                ["status", deck, slot] => {
                    println!("{:?}", control.slot(deck.parse()?, slot.parse()?)?)
                }
                ["render", frames] => {
                    let mut remaining: usize = frames.parse()?;
                    if !(1..=2880000).contains(&remaining) {
                        return Err("invalid render size".into());
                    }
                    let extra = u32::try_from(remaining * 8)?;
                    let new = bytes
                        .checked_add(extra)
                        .filter(|n| *n <= u32::MAX - 36)
                        .ok_or("WAV size exhausted")?;
                    let mut buffer = [0.0; 512 * 4];
                    while remaining > 0 {
                        let count = remaining.min(512);
                        engine.render(&mut buffer[..count * 4], routing);
                        if engine.failed() {
                            return Err("engine failure".into());
                        }
                        for sample in &buffer[..count * 4] {
                            file.write_all(&((sample * 32767.0) as i16).to_le_bytes())?;
                        }
                        remaining -= count;
                    }
                    bytes = new;
                    println!("frame {}", engine.frame().0);
                }
                _ => return Err("invalid command".into()),
            }
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("Rejected: {error}");
        }
        if words == ["quit"] {
            break;
        }
    }
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&(36 + bytes).to_le_bytes())?;
    file.seek(SeekFrom::Start(40))?;
    file.write_all(&bytes.to_le_bytes())?;
    Ok(())
}
