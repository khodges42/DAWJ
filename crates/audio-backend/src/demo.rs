// SPDX-License-Identifier: GPL-3.0-or-later
//! Resident M2.7 demo assets, prepared off the audio thread.
use crate::{
    live::{LiveControl, LiveEngine},
    mixer::MixerControl,
    samples::{BoundaryPolicy, LoadedSample, SampleVoice},
};
use event_engine::live::MusicalScheduler;
use parts::{AssetId, Part, PartId, PlaybackConfiguration, PlaybackMode, SLOTS, TimingPolicy};
use timebase::{BeatTicks, TempoChange, TempoMap};
pub const RATE: u32 = 48000;
pub const BEAT: usize = 24000;
pub const NAMES: [&str; SLOTS] = [
    "kick", "snare", "hats", "bass", "chord", "arp", "shaker", "fill",
];
pub const TITLES: [&str; 2] = ["Copper Steps", "Glass Tide"];

pub fn prepare() -> Result<(LiveEngine, LiveControl, MixerControl), &'static str> {
    let mut bank = Vec::with_capacity(2 * SLOTS);
    for deck in 0..2 {
        for slot in 0..SLOTS {
            let frames = BEAT * 4;
            let bytes = (frames * 8) as u32;
            let mut wav = Vec::with_capacity(bytes as usize + 44);
            wav.extend(b"RIFF");
            wav.extend((36 + bytes).to_le_bytes());
            wav.extend(b"WAVEfmt ");
            wav.extend(16_u32.to_le_bytes());
            wav.extend(3_u16.to_le_bytes());
            wav.extend(2_u16.to_le_bytes());
            wav.extend(RATE.to_le_bytes());
            wav.extend((RATE * 8).to_le_bytes());
            wav.extend(8_u16.to_le_bytes());
            wav.extend(32_u16.to_le_bytes());
            wav.extend(b"data");
            wav.extend(bytes.to_le_bytes());
            for frame in 0..frames {
                let beat = frame / BEAT;
                let within = (frame % BEAT) as f64 / f64::from(RATE);
                let time = frame as f64 / f64::from(RATE);
                let noise = (((frame as u32)
                    .wrapping_mul(1664525)
                    .wrapping_add((deck as u32 + 1) * 1013904223)
                    .rotate_left(13)
                    >> 8) as f64
                    / 8388608.0)
                    - 1.0;
                let root = if deck == 0 { 65.406 } else { 77.782 };
                let tone = |freq: f64, t: f64| (std::f64::consts::TAU * freq * t).sin();
                let gate = |period: usize| {
                    let t = (frame % period) as f64 / period as f64;
                    (std::f64::consts::PI * t).sin().powi(2)
                };
                let sample = match slot {
                    0 => {
                        tone(48.0 + if deck == 0 { 8.0 } else { 14.0 }, within)
                            * (-within * 24.0).exp()
                    }
                    1 => {
                        if beat % 2 == 1 {
                            noise * (-within * 32.0).exp()
                        } else {
                            0.0
                        }
                    }
                    2 => {
                        noise * gate(BEAT / 2) * if frame % (BEAT / 2) < 1800 { 0.45 } else { 0.0 }
                    }
                    3 => {
                        tone(
                            root * [1.0, 1.0, 1.5, if deck == 0 { 1.25 } else { 2.0 }][beat],
                            time,
                        ) * gate(BEAT)
                    }
                    4 => {
                        (tone(root * 2.0, time) + tone(root * 2.4, time) + tone(root * 3.0, time))
                            / 3.0
                            * gate(frames)
                    }
                    5 => {
                        tone(root * [4.0, 5.0, 6.0, 8.0][(frame / (BEAT / 2)) % 4], time)
                            * gate(BEAT / 2)
                            * 0.6
                    }
                    6 => noise * gate(BEAT / 4) * 0.25,
                    _ => {
                        let t = (frame % (BEAT / 4)) as f64 / f64::from(RATE);
                        (noise * 0.7 + tone(root * 3.0, t) * 0.3) * (-t * 60.0).exp()
                    }
                } as f32
                    * 0.18;
                let pan = if (deck + slot) % 2 == 0 {
                    [1.0_f32, 0.8]
                } else {
                    [0.8, 1.0]
                };
                for gain in pan {
                    wav.extend((sample * gain).to_le_bytes());
                }
            }
            let id = (deck * SLOTS + slot + 1) as u64;
            let part = Part {
                id: PartId(id),
                playback: PlaybackConfiguration {
                    asset: AssetId(id),
                    mode: if slot == SLOTS - 1 {
                        PlaybackMode::OneShot
                    } else {
                        PlaybackMode::Loop {
                            start: 0,
                            end: frames as u64,
                        }
                    },
                },
                timing: TimingPolicy {
                    nominal_length: BeatTicks(3840),
                    quantum: BeatTicks(960),
                },
            };
            bank.push(SampleVoice::prepare(
                part,
                LoadedSample::decode(AssetId(id), &wav, RATE)?,
                RATE,
                BoundaryPolicy::Fade { frames: 64 },
            )?);
        }
    }
    let map = TempoMap::new(
        RATE,
        &[TempoChange {
            at: BeatTicks(0),
            bpm: 120.0,
        }],
    )
    .map_err(|_| "tempo setup failed")?;
    let scheduler = MusicalScheduler::new(map, [BeatTicks(0); 2], [BeatTicks(32 * 960); 2])?;
    let (engine, control, mixer) = LiveEngine::new(scheduler, bank)?;
    Ok((engine, control, mixer))
}
