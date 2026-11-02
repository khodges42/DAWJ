// SPDX-License-Identifier: GPL-3.0-or-later
//! Version 1 frontend boundary. Acceptance is not an audio-thread acknowledgement.
use crate::{Coordinator, InputSource, Intent, Mode, PartAction, RuleAction};
use audio_backend::{
    live::{Quantize, RulePhase, RuleUnit, SlotState, TriggerRule},
    performance_state::PendingKind,
};
use serde::{Deserialize, Serialize};
pub const VERSION: u16 = 1;
pub const CAPACITY: usize = 32;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub id: u32,
    pub command: Command,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Session {
        action: crate::session::Action,
    },
    Devices {},
    Start {
        device: usize,
        #[serde(default)]
        cue: bool,
        #[serde(default = "eight")]
        phrase_bars: u8,
    },
    Stop {},
    Snapshot {},
    TimelineView {
        zoom_bars: u8,
        follow: bool,
        page_delta: i8,
    },
    Perform {
        action: Action,
        #[serde(default = "gui")]
        source: InputSource,
    },
    ReleaseModes {
        source: InputSource,
    },
    MidiProfile {},
    MidiReplace {
        profile: crate::midi::profile::Profile,
        #[serde(default)]
        expected_revision: Option<u64>,
    },
    MidiConnect {
        input: Option<String>,
        output: Option<String>,
    },
    MidiSave {},
    MidiLoad {},
    MidiRescan {},
    MidiLearn {
        mapping: crate::midi::profile::Mapping,
        replace: Option<usize>,
    },
    MidiCancelLearn {},
    MidiProfiles {},
    MidiSelectProfile {
        name: String,
    },
    MidiSaveAs {
        name: String,
    },
    MidiFactory {},
}
fn eight() -> u8 {
    8
}
fn gui() -> InputSource {
    InputSource::Gui
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Grid {
    Immediate,
    Beat,
    Bar,
    Phrase,
}
impl From<Grid> for Quantize {
    fn from(v: Grid) -> Self {
        match v {
            Grid::Immediate => Self::Immediate,
            Grid::Beat => Self::Beat,
            Grid::Bar => Self::Bar,
            Grid::Phrase => Self::Phrase,
        }
    }
}
impl From<Quantize> for Grid {
    fn from(v: Quantize) -> Self {
        match v {
            Quantize::Immediate => Self::Immediate,
            Quantize::Beat => Self::Beat,
            Quantize::Bar => Self::Bar,
            Quantize::Phrase => Self::Phrase,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Phase {
    Global,
    Deck,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Unit {
    Beats,
    Bars,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
    Once {
        boundary: Grid,
    },
    Every {
        n: u16,
        unit: Unit,
        phase: Phase,
    },
    PhraseBar {
        bar: u8,
        phrase_bars: u8,
        phase: Phase,
    },
}
impl From<Phase> for RulePhase {
    fn from(p: Phase) -> Self {
        match p {
            Phase::Global => Self::Global,
            Phase::Deck => Self::Deck,
        }
    }
}
impl From<RulePhase> for Phase {
    fn from(p: RulePhase) -> Self {
        match p {
            RulePhase::Global => Self::Global,
            RulePhase::Deck => Self::Deck,
        }
    }
}
impl From<Rule> for TriggerRule {
    fn from(r: Rule) -> Self {
        match r {
            Rule::Once { boundary } => Self::Once {
                boundary: boundary.into(),
            },
            Rule::Every { n, unit, phase } => Self::Every {
                n,
                unit: match unit {
                    Unit::Beats => RuleUnit::Beats,
                    Unit::Bars => RuleUnit::Bars,
                },
                phase: phase.into(),
            },
            Rule::PhraseBar {
                bar,
                phrase_bars,
                phase,
            } => Self::PhraseBar {
                bar,
                phrase_bars,
                phase: phase.into(),
            },
        }
    }
}
impl From<TriggerRule> for Rule {
    fn from(r: TriggerRule) -> Self {
        match r {
            TriggerRule::Once { boundary } => Self::Once {
                boundary: boundary.into(),
            },
            TriggerRule::Every { n, unit, phase } => Self::Every {
                n,
                unit: match unit {
                    RuleUnit::Beats => Unit::Beats,
                    RuleUnit::Bars => Unit::Bars,
                },
                phase: phase.into(),
            },
            TriggerRule::PhraseBar {
                bar,
                phrase_bars,
                phase,
            } => Self::PhraseBar {
                bar,
                phrase_bars,
                phase: phase.into(),
            },
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    CrossfaderStep {
        delta: f32,
    },
    SelectPart {
        deck: usize,
        slot: usize,
    },
    Selected {
        action: PartAction,
    },
    Deck {
        deck: usize,
    },
    Slot {
        slot: usize,
    },
    Mode {
        mode: Mode,
    },
    HoldMode {
        token: u16,
        mode: Mode,
        pressed: bool,
    },
    Quantize {
        grid: Grid,
    },
    Part {
        deck: usize,
        slot: usize,
        action: PartAction,
    },
    Launch {
        deck: usize,
        slot: usize,
    },
    StopPart {
        deck: usize,
        slot: usize,
    },
    Crossfader {
        value: f32,
    },
    DeckGain {
        deck: usize,
        value: f32,
    },
    Cue {
        deck: usize,
    },
    Gain {
        value: f32,
    },
    Cutoff {
        hz: Option<f32>,
    },
    OutputTrim {
        db: f32,
    },
    Tempo {
        bpm: f64,
    },
    RuleInstall {
        deck: usize,
        slot: usize,
        rule: Rule,
    },
    RuleEnable {
        deck: usize,
        slot: usize,
        enabled: bool,
    },
    RuleCancel {
        deck: usize,
        slot: usize,
    },
}
impl From<Action> for Intent {
    fn from(a: Action) -> Self {
        match a {
            Action::CrossfaderStep { delta } => Self::CrossfaderStep(delta),
            Action::SelectPart { deck, slot } => Self::SelectPart { deck, slot },
            Action::Selected { action } => match action {
                PartAction::Launch => Self::Launch,
                PartAction::Stop => Self::Stop,
                PartAction::Cancel => Self::Cancel,
                PartAction::Retrigger => Self::Retrigger,
                PartAction::Toggle => Self::Toggle,
                PartAction::Gain(v) => Self::Gain(v),
                PartAction::Cutoff(v) => Self::Cutoff(v),
            },
            Action::Deck { deck } => Self::Deck(deck),
            Action::Slot { slot } => Self::Slot(slot),
            Action::Mode { mode } => Self::Mode(mode),
            Action::HoldMode {
                token,
                mode,
                pressed,
            } => Self::HoldMode {
                token,
                mode,
                pressed,
            },
            Action::Quantize { grid } => Self::Quantize(grid.into()),
            Action::Part { deck, slot, action } => Self::Part { deck, slot, action },
            Action::Launch { deck, slot } => Self::Part {
                deck,
                slot,
                action: PartAction::Launch,
            },
            Action::StopPart { deck, slot } => Self::Part {
                deck,
                slot,
                action: PartAction::Stop,
            },
            Action::Crossfader { value } => Self::Crossfader(value),
            Action::DeckGain { deck, value } => Self::DeckGain(deck, value),
            Action::Cue { deck } => Self::Cue(deck),
            Action::Gain { value } => Self::Gain(value),
            Action::Cutoff { hz } => Self::Cutoff(hz),
            Action::OutputTrim { db } => Self::OutputTrim(db),
            Action::Tempo { bpm } => Self::Tempo(bpm),
            Action::RuleInstall { deck, slot, rule } => Self::Rule {
                deck,
                slot,
                action: RuleAction::Install(rule.into()),
            },
            Action::RuleEnable {
                deck,
                slot,
                enabled,
            } => Self::Rule {
                deck,
                slot,
                action: RuleAction::Enable(enabled),
            },
            Action::RuleCancel { deck, slot } => Self::Rule {
                deck,
                slot,
                action: RuleAction::Cancel,
            },
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    Busy,
    UnsupportedVersion,
    Invalid,
    Unavailable,
    Failed,
    Closed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fault {
    pub code: ErrorCode,
    pub message: String,
}
impl Fault {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub version: u16,
    pub id: u32,
    pub result: Result<Response, Fault>,
}
impl Reply {
    pub fn new(id: u32, result: Result<Response, Fault>) -> Self {
        Self {
            version: VERSION,
            id,
            result,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Response {
    Snapshot(Box<Snapshot>),
    Devices(Vec<Device>),
    MidiProfile(crate::midi::profile::Profile),
    MidiProfiles(Vec<String>),
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Device {
    pub id: usize,
    pub name: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub enum PendingType {
    Launch,
    Stop,
    Retrigger,
    Replace,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Pending {
    pub kind: PendingType,
    pub tick: i64,
    pub command_id: Option<u64>,
    pub part: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct RuleState {
    pub definition: Rule,
    pub next: Option<i64>,
    pub period: Option<i64>,
    pub last_fired: Option<i64>,
    pub fired: u64,
    pub skipped: u64,
    pub consumed: bool,
    pub one_shot: bool,
    pub part: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub enum Lifecycle {
    #[serde(rename = "STOPPED")]
    Stopped,
    #[serde(rename = "QUEUED START")]
    QueuedStart,
    #[serde(rename = "PLAYING")]
    Playing,
    #[serde(rename = "PLAYING / QUEUED STOP")]
    QueuedStop,
    #[serde(rename = "PLAYING / RETRIGGER")]
    Retrigger,
    #[serde(rename = "PLAYING / REPLACE")]
    Replace,
}
impl From<&audio_backend::performance_state::PartState> for Lifecycle {
    fn from(p: &audio_backend::performance_state::PartState) -> Self {
        match p.lifecycle {
            SlotState::Stopped => Self::Stopped,
            SlotState::QueuedStart => Self::QueuedStart,
            SlotState::Playing => Self::Playing,
            SlotState::QueuedStop => Self::QueuedStop,
            SlotState::QueuedReplace => {
                if p.pending.is_some_and(|p| p.kind == PendingKind::Retrigger) {
                    Self::Retrigger
                } else {
                    Self::Replace
                }
            }
        }
    }
}
impl Lifecycle {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stopped => "STOPPED",
            Self::QueuedStart => "QUEUED START",
            Self::Playing => "PLAYING",
            Self::QueuedStop => "PLAYING / QUEUED STOP",
            Self::Retrigger => "PLAYING / RETRIGGER",
            Self::Replace => "PLAYING / REPLACE",
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub enum Transport {
    Ready,
    Rendering,
    Failed,
    Closed,
}
impl From<audio_backend::performance_state::TransportState> for Transport {
    fn from(t: audio_backend::performance_state::TransportState) -> Self {
        use audio_backend::performance_state::TransportState as T;
        match t {
            T::Ready => Self::Ready,
            T::Rendering => Self::Rendering,
            T::Failed => Self::Failed,
            T::Closed => Self::Closed,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Part {
    pub available: bool,
    pub state: Lifecycle,
    pub active: Option<u64>,
    pub pending: Option<Pending>,
    pub name: String,
    pub position: Option<u64>,
    pub phase: Option<i64>,
    pub gain: f32,
    pub muted: bool,
    pub cutoff_hz: Option<f32>,
    pub release_frames: u32,
    pub active_one_shot: bool,
    pub pending_one_shot: bool,
    pub nominal_length: Option<i64>,
    pub rule: Option<RuleState>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DeckClock {
    pub phase: i64,
    pub epoch: i64,
    pub phrase_length: i64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Selection {
    pub deck: usize,
    pub slots: [usize; 2],
    pub mode: Mode,
    pub persistent_mode: Mode,
    pub temporary: bool,
    pub quantization: Grid,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub session: Option<crate::session::Workspace>,
    pub control_revision: u64,
    pub online: bool,
    pub failed: bool,
    pub generation: Option<u64>,
    pub frame: Option<i64>,
    pub beat: Option<i64>,
    pub bpm: Option<f64>,
    pub sample_rate: Option<u32>,
    pub transport: Option<Transport>,
    pub decks: Option<[[Part; 8]; 2]>,
    pub musical_decks: Option<[DeckClock; 2]>,
    pub selection: Option<Selection>,
    pub crossfader: f32,
    pub deck_gain: [f32; 2],
    pub cue: [bool; 2],
    pub cue_available: bool,
    pub output_trim_db: f32,
    pub peaks: [f32; 2],
    pub notice: String,
    /// Coordinator receipts are control feedback, separate from authoritative part lifecycle.
    pub feedback: [[String; 8]; 2],
    pub midi: Option<crate::midi::device::DeviceStatus>,
    pub midi_notice: String,
    pub timeline: Option<crate::web_timeline::Projection>,
    pub midi_revision: u64,
    pub midi_dirty: bool,
    pub midi_learning: bool,
    pub midi_dropped: u64,
    pub profile_name: String,
    pub profile_path: String,
}
impl Snapshot {
    pub fn project(co: &Coordinator, failed: bool) -> Self {
        // Copy exactly one engine publication before projecting any clock/part fields.
        let s = co.snapshot;
        Self {
            online: co.available() && !failed,
            failed: failed || s.is_some_and(|s| s.failed),
            generation: s.map(|s| s.generation),
            frame: s.map(|s| s.frame.0),
            beat: s.map(|s| s.beat.0),
            bpm: s.and_then(|s| s.bpm),
            sample_rate: s.map(|s| s.sample_rate),
            transport: s.map(|s| s.transport.into()),
            musical_decks: s.map(|s| {
                s.decks.map(|d| DeckClock {
                    phase: d.phase.0,
                    epoch: d.epoch.0,
                    phrase_length: d.phrase_length.0,
                })
            }),
            decks: s.map(|s| {
                std::array::from_fn(|deck| {
                    let d = s.decks[deck];
                    std::array::from_fn(|i| {
                        let p = d.slots[i];
                        Part {
                            available: co.banks[deck][i].is_some(),
                            state: Lifecycle::from(&p),
                            active: p.active.map(|p| p.0),
                            pending: p.pending.map(|p| Pending {
                                kind: match p.kind {
                                    PendingKind::Launch => PendingType::Launch,
                                    PendingKind::Stop => PendingType::Stop,
                                    PendingKind::Retrigger => PendingType::Retrigger,
                                    PendingKind::Replace => PendingType::Replace,
                                },
                                tick: p.target.0,
                                command_id: p.command_id,
                                part: p.part.0,
                            }),
                            name: co.names[deck][i].clone(),
                            position: p.position,
                            phase: p.phase.map(|p| p.0),
                            gain: p.parameters.gain,
                            muted: p.parameters.muted,
                            cutoff_hz: p.parameters.cutoff_hz,
                            release_frames: p.release_frames,
                            active_one_shot: p.active_one_shot,
                            pending_one_shot: p.pending_one_shot,
                            nominal_length: p.nominal_length.map(|b| b.0),
                            rule: p.rule.map(|r| RuleState {
                                definition: r.rule.into(),
                                next: r.next.map(|b| b.0),
                                period: r.period.map(|b| b.0),
                                last_fired: r.last_fired.map(|b| b.0),
                                fired: r.fired,
                                skipped: r.skipped,
                                consumed: r.consumed,
                                one_shot: r.one_shot,
                                part: r.part.0,
                            }),
                        }
                    })
                })
            }),
            selection: Some(Selection {
                deck: co.focus.deck,
                slots: co.focus.slots,
                mode: co.focus.mode,
                persistent_mode: co.persistent_mode,
                temporary: co.temporary_mode(),
                quantization: co.focus.quantization.into(),
            }),
            crossfader: co.parameters.crossfader,
            deck_gain: [co.parameters.gain_a, co.parameters.gain_b],
            cue: co.cues,
            cue_available: co.cue_available,
            output_trim_db: co.output_trim_db,
            peaks: co.peaks(),
            notice: co.notice.clone(),
            feedback: co.feedback.clone(),
            ..Self::default()
        }
    }
}
