// SPDX-License-Identifier: GPL-3.0-or-later
//! Temporary example support, shared with integration tests; not a GUI/application framework.
use audio_backend::live::Ticket;
use audio_backend::{
    live::{LiveControl, LiveEngine, Quantize, Request, RulePhase, RuleUnit, TriggerRule},
    mixer::{CueSelection, MixerControl, MixerParameters},
    part_dsp::PartParameters,
    routing::OutputRouting,
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

pub fn prepare(routing: OutputRouting) -> Result<(LiveEngine, Performance), &'static str> {
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
    let parameters = MixerParameters {
        crossfader: 0.0,
        ..MixerParameters::default()
    };
    mixer.set(parameters)?;
    Ok((
        engine,
        Performance {
            control,
            tickets: Vec::new(),
            mixer,
            selected: 0,
            quantization: Quantize::Beat,
            parts: [[PartParameters::default(); SLOTS]; 2],
            parameters,
            cue: CueSelection::Off,
            routing,
        },
    ))
}
pub struct Performance {
    tickets: Vec<Ticket>,
    pub control: LiveControl,
    pub mixer: MixerControl,
    pub selected: usize,
    pub quantization: Quantize,
    parts: [[PartParameters; SLOTS]; 2],
    parameters: MixerParameters,
    cue: CueSelection,
    pub routing: OutputRouting,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Instruction {
    Continue,
    Wait(usize),
    Quit,
    CueUnavailable,
}
fn slot(text: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let value: usize = text.parse()?;
    if !(1..=SLOTS).contains(&value) {
        return Err("slot must be 1..=8".into());
    }
    Ok(value - 1)
}
fn quant(text: &str) -> Result<Quantize, &'static str> {
    match text {
        "immediate" => Ok(Quantize::Immediate),
        "beat" => Ok(Quantize::Beat),
        "bar" => Ok(Quantize::Bar),
        "phrase" => Ok(Quantize::Phrase),
        _ => Err("invalid quantization"),
    }
}
impl Performance {
    fn send(&mut self, slot: usize, request: Request, q: Quantize) -> Result<(), &'static str> {
        let ticket = self.control.submit(self.selected, slot, request, q)?;
        if self.tickets.len() == 32 {
            self.tickets.remove(0);
        }
        self.tickets.push(ticket);
        Ok(())
    }

    pub fn command(&mut self, line: &str) -> Result<Instruction, Box<dyn std::error::Error>> {
        let words: Vec<_> = line
            .split('#')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        match words.as_slice() {
            [] => {}
            ["quit"] => return Ok(Instruction::Quit),
            ["deck", deck] => {
                self.selected = match *deck {
                    "a" | "A" => 0,
                    "b" | "B" => 1,
                    _ => return Err("deck must be a or b".into()),
                }
            }
            ["quant", value] => self.quantization = quant(value)?,
            [command, value] if matches!(*command, "launch" | "stop" | "retrigger") => {
                let slot = slot(value)?;
                let request = match *command {
                    "launch" => Request::Launch((self.selected * SLOTS + slot) as u8),
                    "stop" => Request::Stop,
                    _ => Request::Retrigger,
                };
                self.send(slot, request, self.quantization)?;
            }
            [command, index, value] if matches!(*command, "gain" | "cutoff" | "mute") => {
                let slot = slot(index)?;
                let mut parameters = self.parts[self.selected][slot];
                match *command {
                    "gain" => parameters.gain = value.parse()?,
                    "mute" => parameters.muted = value.parse()?,
                    _ => {
                        parameters.cutoff_hz = if *value == "off" {
                            None
                        } else {
                            Some(value.parse()?)
                        }
                    }
                };
                self.send(slot, Request::Parameters(parameters), Quantize::Immediate)?;
                self.parts[self.selected][slot] = parameters;
            }
            ["fill"] | ["fill", _] => {
                let boundary = if words.len() == 2 {
                    quant(words[1])?
                } else {
                    self.quantization
                };
                self.send(
                    SLOTS - 1,
                    Request::Rule {
                        bank: (self.selected * SLOTS + SLOTS - 1) as u8,
                        rule: TriggerRule::Once { boundary },
                    },
                    Quantize::Immediate,
                )?;
            }
            ["every", n, unit] => {
                let n: u16 = n.parse()?;
                if !(1..=1024).contains(&n) {
                    return Err("invalid interval".into());
                }
                let unit = match *unit {
                    "beats" => RuleUnit::Beats,
                    "bars" => RuleUnit::Bars,
                    _ => return Err("use beats or bars".into()),
                };
                self.send(
                    SLOTS - 1,
                    Request::Rule {
                        bank: (self.selected * SLOTS + SLOTS - 1) as u8,
                        rule: TriggerRule::Every {
                            n,
                            unit,
                            phase: RulePhase::Deck,
                        },
                    },
                    Quantize::Immediate,
                )?;
            }
            ["phrasefill", bar] => {
                let bar: u8 = bar.parse()?;
                if !(1..=8).contains(&bar) {
                    return Err("bar must be 1..=8".into());
                }
                self.send(
                    SLOTS - 1,
                    Request::Rule {
                        bank: (self.selected * SLOTS + SLOTS - 1) as u8,
                        rule: TriggerRule::PhraseBar {
                            bar,
                            phrase_bars: 8,
                            phase: RulePhase::Deck,
                        },
                    },
                    Quantize::Immediate,
                )?;
            }
            ["rule", value] => {
                let request = match *value {
                    "on" => Request::EnableRule(true),
                    "off" => Request::EnableRule(false),
                    "cancel" => Request::CancelRule,
                    _ => return Err("use on, off or cancel".into()),
                };
                self.send(SLOTS - 1, request, Quantize::Immediate)?;
            }
            ["cross", value] => {
                let x: f32 = value.parse()?;
                if !x.is_finite() || !(0.0..=1.0).contains(&x) {
                    return Err("crossfader outside 0..=1".into());
                }
                let mut p = self.parameters;
                p.crossfader = x;
                self.mixer.set(p)?;
                self.parameters = p;
            }
            ["cue", value] => {
                let cue = match *value {
                    "a" => CueSelection::A,
                    "b" => CueSelection::B,
                    "both" => CueSelection::Both,
                    "off" => CueSelection::Off,
                    _ => return Err("use a, b, both or off".into()),
                };
                if self.routing.cue().is_none() && !matches!(cue, CueSelection::Off) {
                    return Ok(Instruction::CueUnavailable);
                }
                self.mixer.set_cue(cue);
                self.cue = cue;
            }
            ["cue"] => {
                let bits = match self.cue {
                    CueSelection::Off => 0,
                    CueSelection::A => 1,
                    CueSelection::B => 2,
                    CueSelection::Both => 3,
                };
                let cue = match bits ^ (1 << self.selected) {
                    0 => "off",
                    1 => "a",
                    2 => "b",
                    _ => "both",
                };
                return self.command(&format!("cue {cue}"));
            }
            ["wait", beats] => {
                let beats: usize = beats.parse()?;
                if !(1..=128).contains(&beats) {
                    return Err("wait outside 1..=128 beats".into());
                }
                return Ok(Instruction::Wait(beats * BEAT));
            }
            ["status"] => {}
            _ => return Err("unknown command; consult performance --help".into()),
        }
        Ok(Instruction::Continue)
    }
    pub fn status(&self) -> String {
        let mut text = format!(
            "selected={} quant={:?} cross={} cue={:?} {:?}\n",
            TITLES[self.selected],
            self.quantization,
            self.parameters.crossfader,
            self.cue,
            self.routing.cue_availability()
        );
        for (deck, title) in TITLES.iter().enumerate() {
            text.push_str(&format!("{}: {title}\n", if deck == 0 { "A" } else { "B" }));
            for (slot, name) in NAMES.iter().enumerate() {
                let s = self.control.slot(deck, slot).expect("fixed valid slot");
                text.push_str(&format!(
                    " {} {name:7} {:?} pos={} gain={:.2} cutoff={:?} peak={:.3} rule={}/{}/{}\n",
                    slot + 1,
                    s.state,
                    s.position,
                    s.parameters.gain,
                    s.parameters.cutoff_hz,
                    s.peak[0].max(s.peak[1]),
                    s.rule_enabled,
                    s.rule_fired,
                    s.rule_skipped
                ));
            }
        }
        for ticket in &self.tickets {
            text.push_str(&format!("command {} {:?}\n", ticket.id(), ticket.outcome()));
        }
        text
    }
    pub fn fadeout(&mut self) -> Result<(), &'static str> {
        let mut p = self.parameters;
        p.gain_a = 0.0;
        p.gain_b = 0.0;
        self.mixer.set(p)
    }
}
