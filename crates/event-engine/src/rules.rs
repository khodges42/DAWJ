// SPDX-License-Identifier: GPL-3.0-or-later
//! Declarative, bounded periodic intent. All periods are stored in musical ticks.
use crate::live::Quantization;
use parts::Part;
use timebase::BeatTicks;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulePhase {
    Global,
    Deck,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleUnit {
    Beats,
    Bars,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriggerRule {
    Once {
        boundary: Quantization,
    },
    Every {
        n: u16,
        unit: RuleUnit,
        phase: RulePhase,
    },
    /// One-based bar number. Global phrases have the declared length; deck phrases must match.
    PhraseBar {
        bar: u8,
        phrase_bars: u8,
        phase: RulePhase,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleStatus {
    pub rule: TriggerRule,
    pub next: Option<BeatTicks>,
    pub consumed: bool,
    pub fired: u64,
    pub skipped: u64,
}
#[derive(Clone, Copy)]
pub(crate) struct CompiledRule {
    pub part: Part,
    pub status: RuleStatus,
    pub period: Option<BeatTicks>,
}
