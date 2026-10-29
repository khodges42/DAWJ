// SPDX-License-Identifier: GPL-3.0-or-later
//! Temporary example support, shared with integration tests; not a GUI/application framework.
use audio_backend::demo::{NAMES, TITLES};
use audio_backend::live::Ticket;
use audio_backend::{
    live::{LiveControl, LiveEngine, Quantize, Request, RulePhase, RuleUnit, TriggerRule},
    mixer::{CueSelection, MixerControl, MixerParameters},
    part_dsp::PartParameters,
    routing::OutputRouting,
};
use parts::SLOTS;
pub const RATE: u32 = audio_backend::demo::RATE;
pub const BEAT: usize = audio_backend::demo::BEAT;
pub fn prepare(routing: OutputRouting) -> Result<(LiveEngine, Performance), &'static str> {
    debug_assert_eq!(RATE, audio_backend::demo::RATE);
    let (engine, control, mixer) = audio_backend::demo::prepare()?;
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
