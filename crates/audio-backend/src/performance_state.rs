// SPDX-License-Identifier: GPL-3.0-or-later
//! Coherent, bounded audio-to-control observation. No audio-thread reader waits.
use crate::{live::SlotState, part_dsp::PartParameters};
pub use event_engine::rules::RuleStatus;
use event_engine::{
    live::Quantization,
    rules::{RulePhase, RuleUnit, TriggerRule},
};
use parts::{PartId, SLOTS};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use timebase::{BeatTicks, SampleFrame};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportState {
    Ready,
    Rendering,
    Failed,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingKind {
    Launch,
    Stop,
    Retrigger,
    Replace,
}
#[derive(Clone, Copy, Debug)]
pub struct PendingAction {
    pub kind: PendingKind,
    pub part: PartId,
    pub target: BeatTicks,
    pub command_id: Option<u64>,
}
#[derive(Clone, Copy, Debug)]
pub struct PartState {
    pub lifecycle: SlotState,
    pub active: Option<PartId>,
    pub pending: Option<PendingAction>,
    pub position: Option<u64>,
    pub phase: Option<BeatTicks>,
    pub parameters: PartParameters,
    pub release_frames: u32,
    pub active_one_shot: bool,
    pub pending_one_shot: bool,
    pub nominal_length: Option<BeatTicks>,
    pub rule: Option<RuleStatus>,
}
#[derive(Clone, Copy, Debug)]
pub struct DeckState {
    pub phase: BeatTicks,
    pub epoch: BeatTicks,
    pub phrase_length: BeatTicks,
    pub slots: [PartState; SLOTS],
}
#[derive(Clone, Copy, Debug)]
pub struct PerformanceState {
    pub transport: TransportState,
    pub generation: u64,
    /// Next unrendered frame. Snapshot describes state after the preceding block.
    pub frame: SampleFrame,
    pub beat: BeatTicks,
    pub sample_rate: u32,
    /// Current scheduler tempo, never inferred from GUI frame timing.
    pub bpm: Option<f64>,
    pub failed: bool,
    pub decks: [DeckState; 2],
}
const HEADER: usize = 11;
const SLOT_WORDS: usize = 18;
const WORDS: usize = HEADER + 2 * SLOTS * SLOT_WORDS;
pub(crate) struct Publication {
    generation: AtomicU64,
    words: [AtomicU64; WORDS],
}
impl Publication {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            words: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
    pub fn publish(&self, s: PerformanceState) {
        let mut w = [0; WORDS];
        w[..HEADER].copy_from_slice(&[
            s.frame.0 as u64,
            s.beat.0 as u64,
            u64::from(s.sample_rate),
            s.failed as u64,
            s.decks[0].phase.0 as u64,
            s.decks[1].phase.0 as u64,
            s.bpm.unwrap_or(f64::NAN).to_bits(),
            s.decks[0].epoch.0 as u64,
            s.decks[1].epoch.0 as u64,
            s.decks[0].phrase_length.0 as u64,
            s.decks[1].phrase_length.0 as u64,
        ]);
        for d in 0..2 {
            for i in 0..SLOTS {
                let p = s.decks[d].slots[i];
                let n = HEADER + (d * SLOTS + i) * SLOT_WORDS;
                let state = match p.lifecycle {
                    SlotState::Stopped => 0,
                    SlotState::QueuedStart => 1,
                    SlotState::Playing => 2,
                    SlotState::QueuedStop => 3,
                    SlotState::QueuedReplace => 4,
                };
                let pending = p.pending;
                w[n..n + 10].copy_from_slice(&[
                    state
                        | ((p.active.is_some() as u64) << 8)
                        | ((p.active_one_shot as u64) << 9)
                        | ((p.pending_one_shot as u64) << 10),
                    p.active.map_or(0, |id| id.0),
                    pending.map_or(0, |a| match a.kind {
                        PendingKind::Launch => 1,
                        PendingKind::Stop => 2,
                        PendingKind::Retrigger => 3,
                        PendingKind::Replace => 4,
                    }),
                    pending.map_or(0, |a| a.part.0),
                    pending.map_or(0, |a| a.target.0 as u64),
                    pending.and_then(|a| a.command_id).unwrap_or(0),
                    p.position.unwrap_or(u64::MAX),
                    p.phase.map_or(u64::MAX, |b| b.0 as u64),
                    p.parameters.encode().unwrap_or(0),
                    u64::from(p.release_frames),
                ]);
                if let Some(rule) = p.rule {
                    w[n + 10..n + 17].copy_from_slice(&[
                        encode_rule(rule),
                        rule.part.0,
                        rule.next.map_or(0, |t| t.0 as u64),
                        rule.period.map_or(0, |t| t.0 as u64),
                        rule.fired,
                        rule.skipped,
                        rule.last_fired.map_or(0, |t| t.0 as u64),
                    ]);
                }
                w[n + 17] = p.nominal_length.map_or(0, |t| t.0 as u64);
            }
        }
        // All accesses are SC: a reader accepting equal even generations cannot
        // observe a word from an intervening publication. No non-atomic data race.
        self.generation.fetch_add(1, SeqCst);
        for (dst, src) in self.words.iter().zip(w) {
            dst.store(src, SeqCst);
        }
        self.generation.fetch_add(1, SeqCst);
    }
    pub fn read(&self) -> Option<PerformanceState> {
        let g = self.generation.load(SeqCst);
        if g == 0 || g & 1 != 0 {
            return None;
        }
        let w = self.words.each_ref().map(|a| a.load(SeqCst));
        if self.generation.load(SeqCst) != g {
            return None;
        }
        Some(PerformanceState {
            transport: if w[3] != 0 {
                TransportState::Failed
            } else if w[0] == 0 {
                TransportState::Ready
            } else {
                TransportState::Rendering
            },
            generation: g,
            frame: SampleFrame(w[0] as i64),
            beat: BeatTicks(w[1] as i64),
            sample_rate: w[2] as u32,
            bpm: f64::from_bits(w[6])
                .is_finite()
                .then(|| f64::from_bits(w[6])),
            failed: w[3] != 0,
            decks: std::array::from_fn(|d| DeckState {
                phase: BeatTicks(w[4 + d] as i64),
                epoch: BeatTicks(w[7 + d] as i64),
                phrase_length: BeatTicks(w[9 + d] as i64),
                slots: std::array::from_fn(|i| {
                    let n = HEADER + (d * SLOTS + i) * SLOT_WORDS;
                    PartState {
                        lifecycle: match w[n] & 255 {
                            0 => SlotState::Stopped,
                            1 => SlotState::QueuedStart,
                            2 => SlotState::Playing,
                            3 => SlotState::QueuedStop,
                            _ => SlotState::QueuedReplace,
                        },
                        active: if w[n] & 256 != 0 {
                            Some(PartId(w[n + 1]))
                        } else {
                            None
                        },
                        pending: if w[n + 2] == 0 {
                            None
                        } else {
                            Some(PendingAction {
                                kind: match w[n + 2] {
                                    1 => PendingKind::Launch,
                                    2 => PendingKind::Stop,
                                    3 => PendingKind::Retrigger,
                                    _ => PendingKind::Replace,
                                },
                                part: PartId(w[n + 3]),
                                target: BeatTicks(w[n + 4] as i64),
                                command_id: if w[n + 5] == 0 { None } else { Some(w[n + 5]) },
                            })
                        },
                        position: if w[n + 6] == u64::MAX {
                            None
                        } else {
                            Some(w[n + 6])
                        },
                        phase: if w[n + 7] == u64::MAX {
                            None
                        } else {
                            Some(BeatTicks(w[n + 7] as i64))
                        },
                        parameters: PartParameters::decode(w[n + 8]),
                        release_frames: w[n + 9] as u32,
                        active_one_shot: w[n] & (1 << 9) != 0,
                        pending_one_shot: w[n] & (1 << 10) != 0,
                        rule: decode_rule(&w[n + 10..n + 17]),
                        nominal_length: (w[n + 17] != 0).then_some(BeatTicks(w[n + 17] as i64)),
                    }
                }),
            }),
        })
    }
}

fn encode_rule(s: RuleStatus) -> u64 {
    let rule = match s.rule {
        TriggerRule::Once { boundary } => {
            1 | ((match boundary {
                Quantization::Immediate => 0,
                Quantization::Beat => 1,
                Quantization::Bar => 2,
                Quantization::Phrase => 3,
            }) << 8)
        }
        TriggerRule::Every { n, unit, phase } => {
            2 | (u64::from(n) << 8)
                | (((unit == RuleUnit::Bars) as u64) << 24)
                | (((phase == RulePhase::Deck) as u64) << 25)
        }
        TriggerRule::PhraseBar {
            bar,
            phrase_bars,
            phase,
        } => {
            3 | (u64::from(bar) << 8)
                | (u64::from(phrase_bars) << 16)
                | (((phase == RulePhase::Deck) as u64) << 25)
        }
    };
    rule | ((s.one_shot as u64) << 32)
        | ((s.consumed as u64) << 33)
        | ((s.next.is_some() as u64) << 34)
        | ((s.last_fired.is_some() as u64) << 35)
        | ((s.period.is_some() as u64) << 36)
}
fn decode_rule(w: &[u64]) -> Option<RuleStatus> {
    let bits = w[0];
    let phase = if bits & (1 << 25) != 0 {
        RulePhase::Deck
    } else {
        RulePhase::Global
    };
    let rule = match bits & 255 {
        1 => TriggerRule::Once {
            boundary: match (bits >> 8) & 3 {
                0 => Quantization::Immediate,
                1 => Quantization::Beat,
                2 => Quantization::Bar,
                _ => Quantization::Phrase,
            },
        },
        2 => TriggerRule::Every {
            n: ((bits >> 8) & 65535) as u16,
            unit: if bits & (1 << 24) != 0 {
                RuleUnit::Bars
            } else {
                RuleUnit::Beats
            },
            phase,
        },
        3 => TriggerRule::PhraseBar {
            bar: ((bits >> 8) & 255) as u8,
            phrase_bars: ((bits >> 16) & 255) as u8,
            phase,
        },
        _ => return None,
    };
    Some(RuleStatus {
        rule,
        part: PartId(w[1]),
        one_shot: bits & (1 << 32) != 0,
        consumed: bits & (1 << 33) != 0,
        next: (bits & (1 << 34) != 0).then_some(BeatTicks(w[2] as i64)),
        period: (bits & (1 << 36) != 0).then_some(BeatTicks(w[3] as i64)),
        fired: w[4],
        skipped: w[5],
        last_fired: (bits & (1 << 35) != 0).then_some(BeatTicks(w[6] as i64)),
    })
}
