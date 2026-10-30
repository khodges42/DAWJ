// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{Coordinator, Mode};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub const MAX_MAPPINGS: usize = 128;
pub const MAX_LEDS: usize = 32;
pub const MAX_PROFILE_BYTES: usize = 65536;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Note,
    CC,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub channel: u8,
    pub kind: Kind,
    pub number: u8,
}
impl Control {
    pub fn valid(self) -> bool {
        self.channel < 16 && self.number < 128
    }
    pub fn message(self, value: u8) -> [u8; 3] {
        [
            match self.kind {
                Kind::Note => 0x90,
                Kind::CC => 0xB0,
            } | self.channel,
            self.number,
            value,
        ]
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Selected,
    DeckSlot { deck: usize, slot: usize },
    FocusedDeck { slot: usize },
}
impl Target {
    pub fn valid(self) -> bool {
        match self {
            Self::Selected => true,
            Self::DeckSlot { deck, slot } => deck < 2 && slot < 8,
            Self::FocusedDeck { slot } => slot < 8,
        }
    }
    pub fn address(self, co: &Coordinator) -> (usize, usize) {
        match self {
            Self::Selected => co.focus.address(),
            Self::DeckSlot { deck, slot } => (deck, slot),
            Self::FocusedDeck { slot } => (co.focus.deck, slot),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grid {
    Immediate,
    Beat,
    Bar,
    Phrase,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    SelectDeck(usize),
    SelectSlot(usize),
    SelectPart { deck: usize, slot: usize },
    Launch(Target),
    Stop(Target),
    Cancel(Target),
    Retrigger(Target),
    Toggle(Target),
    Gain(Target),
    Cutoff(Target),
    Context(Target),
    DeckGain(usize),
    Crossfader,
    Cue(usize),
    Quantize(Grid),
    Mode(Mode),
    Momentary(Mode),
}
impl Action {
    pub fn continuous(self) -> bool {
        matches!(
            self,
            Self::Gain(_)
                | Self::Cutoff(_)
                | Self::Context(_)
                | Self::DeckGain(_)
                | Self::Crossfader
        )
    }
    pub fn target(self) -> Option<Target> {
        match self {
            Self::Launch(t)
            | Self::Stop(t)
            | Self::Cancel(t)
            | Self::Retrigger(t)
            | Self::Toggle(t)
            | Self::Gain(t)
            | Self::Cutoff(t)
            | Self::Context(t) => Some(t),
            _ => None,
        }
    }
    pub fn with_target(self, t: Target) -> Self {
        match self {
            Self::Launch(_) => Self::Launch(t),
            Self::Stop(_) => Self::Stop(t),
            Self::Cancel(_) => Self::Cancel(t),
            Self::Retrigger(_) => Self::Retrigger(t),
            Self::Toggle(_) => Self::Toggle(t),
            Self::Gain(_) => Self::Gain(t),
            Self::Cutoff(_) => Self::Cutoff(t),
            Self::Context(_) => Self::Context(t),
            _ => self,
        }
    }
    fn valid(self) -> bool {
        match self {
            Self::SelectDeck(d) | Self::DeckGain(d) | Self::Cue(d) => d < 2,
            Self::SelectSlot(s) => s < 8,
            Self::SelectPart { deck, slot } => deck < 2 && slot < 8,
            _ => self.target().is_none_or(Target::valid),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Behavior {
    Button,
    Absolute,
    RelativeTwosComplement,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub input: Control,
    pub action: Action,
    pub behavior: Behavior,
    pub min: u8,
    pub max: u8,
    pub invert: bool,
    pub step: f32,
}
impl Mapping {
    pub fn new(input: Control, action: Action) -> Self {
        Self {
            input,
            action,
            behavior: if action.continuous() {
                Behavior::Absolute
            } else {
                Behavior::Button
            },
            min: 0,
            max: 127,
            invert: false,
            step: 1.0 / 127.0,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Led {
    pub output: Control,
    pub deck: usize,
    pub slot: usize,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedValues {
    pub stopped: u8,
    pub queued: u8,
    pub playing: u8,
    pub stopping: u8,
    pub replacing: u8,
}
impl Default for LedValues {
    fn default() -> Self {
        Self {
            stopped: 0,
            queued: 16,
            playing: 127,
            stopping: 64,
            replacing: 32,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u8,
    pub mappings: Vec<Mapping>,
    pub leds: Vec<Led>,
    pub values: LedValues,
    pub input: Option<String>,
    pub output: Option<String>,
}
impl Default for Profile {
    fn default() -> Self {
        let mut mappings = Vec::new();
        let mut leds = Vec::new();
        let control = |kind, number| Control {
            channel: 0,
            kind,
            number,
        };
        for deck in 0..2 {
            for slot in 0..8 {
                let n = (deck * 8 + slot) as u8;
                mappings.push(Mapping::new(
                    control(Kind::Note, 36 + n),
                    Action::Toggle(Target::DeckSlot { deck, slot }),
                ));
                mappings.push(Mapping::new(
                    control(Kind::CC, 16 + n),
                    Action::Context(Target::DeckSlot { deck, slot }),
                ));
                leds.push(Led {
                    output: control(Kind::Note, 36 + n),
                    deck,
                    slot,
                });
            }
        }
        for (n, a) in [
            (1, Action::Crossfader),
            (2, Action::DeckGain(0)),
            (3, Action::DeckGain(1)),
        ] {
            mappings.push(Mapping::new(control(Kind::CC, n), a));
        }
        for (n, a) in [
            (80, Action::Mode(Mode::Volume)),
            (81, Action::Mode(Mode::Filter)),
            (82, Action::Momentary(Mode::Volume)),
            (83, Action::Momentary(Mode::Filter)),
            (84, Action::Cue(0)),
            (85, Action::Cue(1)),
            (86, Action::Quantize(Grid::Beat)),
            (87, Action::Quantize(Grid::Bar)),
            (88, Action::Quantize(Grid::Phrase)),
            (89, Action::Quantize(Grid::Immediate)),
            (90, Action::SelectDeck(0)),
            (91, Action::SelectDeck(1)),
        ] {
            mappings.push(Mapping::new(control(Kind::Note, n), a));
        }
        for slot in 0..8 {
            mappings.push(Mapping::new(
                control(Kind::Note, 92 + slot as u8),
                Action::SelectSlot(slot),
            ));
        }
        Self {
            version: 1,
            mappings,
            leds,
            values: LedValues::default(),
            input: None,
            output: None,
        }
    }
}
impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.mappings.len() > MAX_MAPPINGS || self.leds.len() > MAX_LEDS {
            return Err("unsupported version or profile capacity exceeded".into());
        }
        for name in [&self.input, &self.output].into_iter().flatten() {
            if name.is_empty() || name.len() > 256 {
                return Err("invalid device name".into());
            }
        }
        for (i, m) in self.mappings.iter().enumerate() {
            if !m.input.valid()
                || !m.action.valid()
                || m.min >= m.max
                || m.max > 127
                || !m.step.is_finite()
                || !(0.0001..=1.0).contains(&m.step)
                || (m.action.continuous() == (m.behavior == Behavior::Button))
                || (m.behavior == Behavior::RelativeTwosComplement && m.input.kind != Kind::CC)
            {
                return Err(format!("invalid mapping {i}"));
            }
            if self.mappings[..i].iter().any(|old| old.input == m.input) {
                return Err(format!(
                    "mapping conflict: channel {} {:?} {}",
                    m.input.channel + 1,
                    m.input.kind,
                    m.input.number
                ));
            }
        }
        for (i, l) in self.leds.iter().enumerate() {
            if !l.output.valid()
                || l.deck >= 2
                || l.slot >= 8
                || self.leds[..i].iter().any(|old| old.output == l.output)
            {
                return Err(format!("invalid or conflicting LED {i}"));
            }
        }
        let v = self.values;
        if [v.stopped, v.queued, v.playing, v.stopping, v.replacing]
            .iter()
            .any(|v| *v > 127)
        {
            return Err("LED values must be 0..127".into());
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_PROFILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_PROFILE_BYTES {
            return Err("profile exceeds 64 KiB".into());
        }
        let p: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        p.validate()?;
        Ok(p)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_PROFILE_BYTES {
            return Err("profile exceeds 64 KiB".into());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temp = path.with_extension("json.tmp");
        let mut f = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
        f.write_all(&bytes)
            .and_then(|()| f.sync_all())
            .map_err(|e| e.to_string())?;
        drop(f);
        std::fs::rename(temp, path).map_err(|e| e.to_string())
    }
}
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("DAWJ").join("midi.json")
}
