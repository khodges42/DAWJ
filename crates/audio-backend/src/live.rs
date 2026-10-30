// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded SPSC command delivery and callback-owned two-deck part scheduling.
use crate::part_dsp::{PartDsp, PartParameters};
use crate::{
    mixer::{MixerControl, StereoMixer},
    playback::PlaybackSource,
    routing::OutputRouting,
    samples::SampleVoice,
};
pub use event_engine::live::Quantization as Quantize;
use event_engine::live::{Action, Event, MusicalScheduler, Quantization};
pub use event_engine::rules::{RulePhase, RuleUnit, TriggerRule};
use parts::{Applied, SLOTS, State};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use timebase::SampleFrame;
const CAPACITY: usize = 32;
pub const MAX_TRACE_EVENTS: usize = 128;
#[derive(Clone, Copy, Debug)]
pub enum Request {
    Launch(u8),
    Stop,
    Replace(u8),
    Retrigger,
    Cancel(u64),
    Tempo(f64),
    Parameters(PartParameters),
    Rule { bank: u8, rule: TriggerRule },
    EnableRule(bool),
    CancelRule,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Outcome {
    Pending = 0,
    Queued = 1,
    Applied = 2,
    Rejected = 3,
    Cancelled = 4,
    Superseded = 5,
    Expired = 6,
    Closed = 7,
}
struct Cell {
    data: [AtomicU64; 2],
    stamp: AtomicU64,
}
struct SlotMeter {
    state: AtomicU64,
    part: AtomicU64,
    position: AtomicU64,
    peak: AtomicU64,
    processed: AtomicU64,
    parameters: AtomicU64,
    rule: AtomicU64,
    rule_fired: AtomicU64,
    rule_skipped: AtomicU64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotState {
    Stopped,
    QueuedStart,
    Playing,
    QueuedStop,
    QueuedReplace,
}
#[derive(Clone, Copy, Debug)]
pub struct SlotStatus {
    pub state: SlotState,
    pub part_id: Option<parts::PartId>,
    pub position: u64,
    pub peak: [f32; 2],
    pub processed_frames: u64,
    pub parameters: PartParameters,
    pub closed: bool,
    pub rule_installed: bool,
    pub rule_enabled: bool,
    pub rule_spent: bool,
    pub rule_fired: u64,
    pub rule_skipped: u64,
}
struct Queue {
    slots: [[SlotMeter; SLOTS]; 2],
    cells: [Cell; CAPACITY],
    read: AtomicUsize,
    write: AtomicUsize,
    closed: Arc<AtomicBool>,
    snapshot: crate::performance_state::Publication,
}
pub struct LiveControl {
    queue: Arc<Queue>,
    next: u64,
}
pub struct Ticket {
    queue: Arc<Queue>,
    row: usize,
    id: u64,
}
impl Ticket {
    pub fn id(&self) -> u64 {
        self.id
    }
    /// A bounded observation window: a reused ring row explicitly reports Expired.
    pub fn outcome(&self) -> Outcome {
        let stamp = self.queue.cells[self.row].stamp.load(Ordering::Acquire);
        if stamp >> 8 != self.id {
            return Outcome::Expired;
        }
        if self.queue.closed.load(Ordering::Acquire) && matches!(stamp as u8, 0 | 1) {
            return Outcome::Closed;
        }
        match stamp as u8 {
            0 => Outcome::Pending,
            1 => Outcome::Queued,
            2 => Outcome::Applied,
            3 => Outcome::Rejected,
            4 => Outcome::Cancelled,
            5 => Outcome::Superseded,
            _ => Outcome::Expired,
        }
    }
}
fn publish(queue: &Queue, row: usize, id: u64, outcome: Outcome) {
    let current = queue.cells[row].stamp.load(Ordering::Acquire);
    if current >> 8 == id {
        let _ = queue.cells[row].stamp.compare_exchange(
            current,
            (id << 8) | outcome as u64,
            Ordering::Release,
            Ordering::Relaxed,
        );
    }
}
impl LiveControl {
    /// One bounded coherent read; retain the previous snapshot if publication overlaps.
    pub fn performance_state(&self) -> Option<crate::performance_state::PerformanceState> {
        let mut state = self.queue.snapshot.read()?;
        if self.is_closed() {
            state.transport = crate::performance_state::TransportState::Closed;
        }
        Some(state)
    }
    /// Terminal lifecycle flag. This may become true after the last audio snapshot.
    pub fn is_closed(&self) -> bool {
        self.queue.closed.load(Ordering::Acquire)
    }
    pub fn slot(&self, deck: usize, slot: usize) -> Result<SlotStatus, &'static str> {
        let meter = self
            .queue
            .slots
            .get(deck)
            .and_then(|d| d.get(slot))
            .ok_or("invalid address")?;
        let state = match meter.state.load(Ordering::Acquire) {
            1 => SlotState::QueuedStart,
            2 => SlotState::Playing,
            3 => SlotState::QueuedStop,
            4 => SlotState::QueuedReplace,
            _ => SlotState::Stopped,
        };
        let peaks = meter.peak.load(Ordering::Relaxed);
        let rule = meter.rule.load(Ordering::Relaxed);
        Ok(SlotStatus {
            state,
            part_id: if state == SlotState::Stopped {
                None
            } else {
                Some(parts::PartId(meter.part.load(Ordering::Relaxed)))
            },
            position: meter.position.load(Ordering::Relaxed),
            peak: [
                f32::from_bits(peaks as u32),
                f32::from_bits((peaks >> 32) as u32),
            ],
            processed_frames: meter.processed.load(Ordering::Relaxed),
            parameters: PartParameters::decode(meter.parameters.load(Ordering::Relaxed)),
            closed: self.queue.closed.load(Ordering::Acquire),
            rule_installed: rule & 1 != 0,
            rule_enabled: rule & 2 != 0,
            rule_spent: rule & 4 != 0,
            rule_fired: meter.rule_fired.load(Ordering::Relaxed),
            rule_skipped: meter.rule_skipped.load(Ordering::Relaxed),
        })
    }

    /// One producer, nonblocking admission. Full means rejected before publication.
    pub fn submit(
        &mut self,
        deck: usize,
        slot: usize,
        request: Request,
        q: Quantization,
    ) -> Result<Ticket, &'static str> {
        if self.queue.closed.load(Ordering::Acquire) {
            return Err("engine closed");
        }
        if deck >= 2 || slot >= SLOTS {
            return Err("invalid address");
        }
        if self.next >= (1_u64 << 48) {
            return Err("command IDs exhausted");
        }
        let write = self.queue.write.load(Ordering::Relaxed);
        if write.wrapping_sub(self.queue.read.load(Ordering::Acquire)) >= CAPACITY {
            return Err("command queue full");
        }
        if matches!(
            request,
            Request::Parameters(_)
                | Request::Rule { .. }
                | Request::EnableRule(_)
                | Request::CancelRule
        ) && q != Quantization::Immediate
        {
            return Err("part parameters require Immediate");
        }
        let (kind, arg) = match request {
            Request::Launch(b) => (0, u64::from(b)),
            Request::Stop => (1, 0),
            Request::Replace(b) => (2, u64::from(b)),
            Request::Retrigger => (3, 0),
            Request::Cancel(id) => (4, id),
            Request::Tempo(bpm) => (5, bpm.to_bits()),
            Request::Parameters(parameters) => (6, parameters.encode()?),
            Request::Rule { bank, rule } => (7, encode_rule(bank, rule)),
            Request::EnableRule(enabled) => (8, enabled as u64),
            Request::CancelRule => (9, 0),
        };
        let quant = match q {
            Quantization::Immediate => 0,
            Quantization::Beat => 1,
            Quantization::Bar => 2,
            Quantization::Phrase => 3,
        };
        let row = write % CAPACITY;
        let id = self.next;
        self.next += 1;
        self.queue.cells[row].data[1].store(arg, Ordering::Relaxed);
        // Fixed Copy payload, published through the write index.
        self.queue.cells[row].data[0].store(
            (id << 16) | ((slot as u64) << 8) | (kind << 4) | (quant << 2) | deck as u64,
            Ordering::Relaxed,
        );
        self.queue.cells[row]
            .stamp
            .store(id << 8, Ordering::Release);
        self.queue
            .write
            .store(write.wrapping_add(1), Ordering::Release);
        Ok(Ticket {
            queue: self.queue.clone(),
            row,
            id,
        })
    }
}
#[derive(Clone, Copy)]
struct Receipt {
    id: u64,
    row: usize,
}
#[derive(Clone, Copy)]
struct Active {
    bank: usize,
    elapsed: u64,
}
pub struct LiveEngine {
    scheduler: MusicalScheduler,
    bank: Box<[SampleVoice]>,
    mixer: StereoMixer,
    queue: Arc<Queue>,
    active: [[Option<Active>; SLOTS]; 2],
    dsp: [[PartDsp; SLOTS]; 2],
    pending: [[Option<Receipt>; SLOTS]; 2],
    events: [Option<Event>; MAX_TRACE_EVENTS],
    dropped_events: u64,
    event_count: usize,
    failed: bool,
    transitions: [[Transition; SLOTS]; 2],
}
impl LiveEngine {
    pub fn new(
        scheduler: MusicalScheduler,
        bank: Vec<SampleVoice>,
    ) -> Result<(Self, LiveControl, MixerControl), &'static str> {
        if scheduler.frame().0 != 0
            || (0..2).any(|d| {
                (0..SLOTS).any(|s| {
                    scheduler.state(d, s) != Some(State::Stopped)
                        || scheduler.rule_status(d, s).is_some()
                })
            })
        {
            return Err("live engine requires a pristine scheduler");
        }
        if bank.is_empty() || bank.len() > 64 || bank.iter().any(|v| v.rate() != scheduler.rate()) {
            return Err("invalid resident bank or rate");
        }
        if bank.iter().map(SampleVoice::resident_frames).sum::<usize>()
            > 2 * crate::samples::MAX_DECK_FRAMES
        {
            return Err("resident bank exceeds 256 MiB PCM limit");
        }
        for (i, voice) in bank.iter().enumerate() {
            if bank[..i].iter().any(|other| other.part() == voice.part()) {
                return Err("bank Part definitions must be unique");
            }
        }
        let silence = || PlaybackSource::Tone {
            frequency_hz: 1.0,
            amplitude: 0.0,
            frames: u64::MAX,
        };
        let (mixer, controls) = StereoMixer::new(silence(), silence(), scheduler.rate())?;
        let queue = Arc::new(Queue {
            slots: std::array::from_fn(|_| {
                std::array::from_fn(|_| SlotMeter {
                    rule: AtomicU64::new(0),
                    rule_fired: AtomicU64::new(0),
                    rule_skipped: AtomicU64::new(0),
                    state: AtomicU64::new(0),
                    part: AtomicU64::new(0),
                    position: AtomicU64::new(0),
                    peak: AtomicU64::new(0),
                    processed: AtomicU64::new(0),
                    parameters: AtomicU64::new(PartParameters::default().encode().unwrap_or(0)),
                })
            }),
            cells: std::array::from_fn(|_| Cell {
                data: std::array::from_fn(|_| AtomicU64::new(0)),
                stamp: AtomicU64::new(0),
            }),
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
            closed: Arc::new(AtomicBool::new(false)),
            snapshot: crate::performance_state::Publication::new(),
        });
        let control = LiveControl {
            queue: queue.clone(),
            next: 1,
        };
        let result = (
            Self {
                scheduler,
                bank: bank.into_boxed_slice(),
                mixer,
                queue,
                active: [[None; SLOTS]; 2],
                dsp: std::array::from_fn(|_| std::array::from_fn(|_| PartDsp::default())),
                pending: [[None; SLOTS]; 2],
                events: [None; MAX_TRACE_EVENTS],
                dropped_events: 0,
                event_count: 0,
                failed: false,
                transitions: [[Transition::default(); SLOTS]; 2],
            },
            control,
            controls,
        );
        result.0.publish_state();
        Ok(result)
    }
    pub(crate) fn close_handle(&self) -> Arc<AtomicBool> {
        self.queue.closed.clone()
    }
    pub fn scheduler(&self) -> &MusicalScheduler {
        &self.scheduler
    }
    pub fn frame(&self) -> SampleFrame {
        self.scheduler.frame()
    }
    pub fn rate(&self) -> u32 {
        self.scheduler.rate()
    }
    pub fn failed(&self) -> bool {
        self.failed
    }
    pub fn dropped_events(&self) -> u64 {
        self.dropped_events
    }
    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events[..self.event_count].iter().flatten().copied()
    }
    pub fn source_position(&self, deck: usize, slot: usize) -> Option<u64> {
        Some(self.active.get(deck)?.get(slot)?.as_ref()?.elapsed)
    }
    fn ingress(&mut self) {
        let mut read = self.queue.read.load(Ordering::Relaxed);
        let end = self.queue.write.load(Ordering::Acquire); // exactly one callback-entry snapshot
        while read != end {
            let row = read % CAPACITY;
            let word = self.queue.cells[row].data[0].load(Ordering::Relaxed);
            let arg = self.queue.cells[row].data[1].load(Ordering::Relaxed);
            let id = word >> 16;
            let slot = ((word >> 8) & 255) as usize;
            let deck = (word & 3) as usize;
            let q = match (word >> 2) & 3 {
                0 => Quantization::Immediate,
                1 => Quantization::Beat,
                2 => Quantization::Bar,
                _ => Quantization::Phrase,
            };
            let kind = (word >> 4) & 15;
            let result = if kind == 7 {
                let (bank, rule) = decode_rule(arg);
                self.bank
                    .get(bank as usize)
                    .ok_or("invalid bank index")
                    .and_then(|voice| self.scheduler.install_rule(deck, slot, voice.part(), rule))
            } else if kind == 8 {
                self.scheduler.enable_rule(deck, slot, arg != 0)
            } else if kind == 9 {
                self.scheduler.cancel_rule(deck, slot)
            } else if kind == 6 {
                self.dsp[deck][slot].set(
                    PartParameters::decode(arg),
                    self.scheduler.rate(),
                    self.active[deck][slot].is_some(),
                )
            } else if kind == 5 {
                self.scheduler.tempo(f64::from_bits(arg), q)
            } else if kind == 4 {
                if self.pending[deck][slot].is_some_and(|p| p.id == arg) {
                    self.scheduler.command(deck, slot, Action::Cancel, q)
                } else {
                    Err("stale cancellation")
                }
            } else {
                let action = match kind {
                    0 | 2 => self.bank.get(arg as usize).map(|v| {
                        if kind == 0 {
                            Action::Launch(v.part())
                        } else {
                            Action::Replace(v.part())
                        }
                    }),
                    1 => Some(Action::Stop),
                    3 => Some(Action::Retrigger),
                    _ => None,
                };
                action
                    .ok_or("invalid bank index")
                    .and_then(|action| self.scheduler.command(deck, slot, action, q))
            };
            if result.is_err() {
                publish(&self.queue, row, id, Outcome::Rejected);
            } else if kind >= 5 {
                // Applied means tempo intent installed in the map, not that its boundary passed.
                publish(&self.queue, row, id, Outcome::Applied);
            } else {
                if let Some(old) = self.pending[deck][slot].take() {
                    publish(
                        &self.queue,
                        old.row,
                        old.id,
                        if kind == 4 {
                            Outcome::Cancelled
                        } else {
                            Outcome::Superseded
                        },
                    );
                }
                let pending = matches!(
                    self.scheduler.state(deck, slot),
                    Some(
                        State::QueuedStart(_)
                            | State::QueuedStop { .. }
                            | State::QueuedReplace { .. }
                    )
                );
                if pending {
                    self.pending[deck][slot] = Some(Receipt { id, row });
                    publish(&self.queue, row, id, Outcome::Queued);
                } else {
                    publish(&self.queue, row, id, Outcome::Applied);
                }
            }
            read = read.wrapping_add(1);
        }
        self.queue.read.store(read, Ordering::Release);
    }
    pub fn render(&mut self, output: &mut [f32], routing: OutputRouting) {
        self.render_converted(output, routing, |s| s);
    }
    pub(crate) fn render_converted<T: Copy>(
        &mut self,
        output: &mut [T],
        routing: OutputRouting,
        convert: impl Fn(f32) -> T,
    ) {
        output.fill(convert(0.0));
        self.event_count = 0;
        self.dropped_events = 0;
        if self.failed || output.len() < usize::from(routing.channels()) {
            return;
        }
        for deck in &mut self.dsp {
            for slot in deck {
                slot.peak = [0.0; 2];
            }
        }
        self.ingress();
        for frame in output.chunks_exact_mut(usize::from(routing.channels())) {
            let Self {
                scheduler,
                bank,
                active,
                dsp,
                pending,
                queue,
                events,
                event_count,
                dropped_events,
                transitions,
                ..
            } = self;
            let scheduler_rate = scheduler.rate();
            if scheduler
                .due(|event| {
                    transitions[event.deck][event.slot].begin((scheduler_rate / 500).max(1));
                    if matches!(event.action, Applied::Start { .. }) {
                        dsp[event.deck][event.slot].start(active[event.deck][event.slot].is_some());
                    }
                    active[event.deck][event.slot] = match event.action {
                        Applied::Start { part, .. } => bank
                            .iter()
                            .position(|v| v.part() == part)
                            .map(|bank| Active { bank, elapsed: 0 }),
                        Applied::Stop { .. } => None,
                    };
                    if let Some(receipt) = pending[event.deck][event.slot].take() {
                        publish(queue, receipt.row, receipt.id, Outcome::Applied);
                    }
                    if *event_count < events.len() {
                        events[*event_count] = Some(event);
                        *event_count += 1;
                    } else {
                        *dropped_events = dropped_events.saturating_add(1);
                    }
                })
                .is_err()
            {
                self.failed = true;
                break;
            }
            let mut sources = [[0.0; 2]; 2];
            for (deck, source) in sources.iter_mut().enumerate() {
                for slot in 0..SLOTS {
                    if let Some(voice) = &mut self.active[deck][slot] {
                        let asset = &self.bank[voice.bank];
                        let sample = self.dsp[deck][slot].process(asset.at_elapsed(voice.elapsed));
                        let sample = self.transitions[deck][slot].process(sample);
                        for ch in 0..2 {
                            source[ch] += sample[ch] / SLOTS as f32;
                        }
                        voice.elapsed = voice.elapsed.saturating_add(1);
                        if voice.elapsed >= asset.length() {
                            self.active[deck][slot] = None;
                            self.transitions[deck][slot]
                                .begin((self.scheduler.rate() / 500).max(1));
                            if self.scheduler.complete(deck, slot).is_err() {
                                self.failed = true;
                            }
                            if matches!(self.scheduler.state(deck, slot), Some(State::Stopped)) {
                                if let Some(receipt) = self.pending[deck][slot].take() {
                                    publish(
                                        &self.queue,
                                        receipt.row,
                                        receipt.id,
                                        Outcome::Superseded,
                                    );
                                }
                            }
                        }
                    } else if self.transitions[deck][slot].remaining > 0 {
                        let sample = self.transitions[deck][slot].process([0.0; 2]);
                        if self.transitions[deck][slot].remaining == 0 {
                            self.transitions[deck][slot].last = [0.0; 2];
                        }
                        for ch in 0..2 {
                            source[ch] += sample[ch] / SLOTS as f32;
                        }
                    }
                }
            }
            let buses = self.mixer.mix_external(sources, [1.0; 2]);
            for (ch, sample) in routing.master().into_iter().zip(buses.master) {
                frame[usize::from(ch)] = convert(sample);
            }
            if let Some(pair) = routing.cue() {
                for (ch, sample) in pair.into_iter().zip(buses.cue) {
                    frame[usize::from(ch)] = convert(sample);
                }
            }
            if self.scheduler.step().is_err() {
                self.failed = true;
                break;
            }
        }
        self.publish_state();
        self.mixer.publish_peaks();
        for deck in 0..2 {
            for slot in 0..SLOTS {
                let state = self.scheduler.state(deck, slot).unwrap_or(State::Stopped);
                let (code, part) = match state {
                    State::Stopped => (0, None),
                    State::QueuedStart(p) => (1, Some(p.part)),
                    State::Playing(p) => (2, Some(p)),
                    State::QueuedStop { active, .. } => (3, Some(active)),
                    State::QueuedReplace { active, .. } => (4, Some(active)),
                };
                let meter = &self.queue.slots[deck][slot];
                let dsp = &self.dsp[deck][slot];
                meter
                    .part
                    .store(part.map_or(0, |p| p.id.0), Ordering::Relaxed);
                meter.position.store(
                    self.active[deck][slot].map_or(0, |v| v.elapsed),
                    Ordering::Relaxed,
                );
                meter.peak.store(
                    u64::from(dsp.peak[0].to_bits()) | (u64::from(dsp.peak[1].to_bits()) << 32),
                    Ordering::Relaxed,
                );
                meter.processed.store(dsp.processed, Ordering::Relaxed);
                meter
                    .parameters
                    .store(dsp.parameters.encode().unwrap_or(0), Ordering::Relaxed);
                let rule = self.scheduler.rule_status(deck, slot);
                meter.rule.store(
                    rule.map_or(0, |r| {
                        1 | ((r.next.is_some() as u64) << 1) | ((r.consumed as u64) << 2)
                    }),
                    Ordering::Relaxed,
                );
                meter
                    .rule_fired
                    .store(rule.map_or(0, |r| r.fired), Ordering::Relaxed);
                meter
                    .rule_skipped
                    .store(rule.map_or(0, |r| r.skipped), Ordering::Relaxed);
                meter.state.store(code, Ordering::Release);
            }
        }
        if self.failed {
            self.queue.closed.store(true, Ordering::Release);
        }
    }
    fn publish_state(&self) {
        use crate::performance_state::*;
        let decks = std::array::from_fn(|d| DeckState {
            phase: self.scheduler.phase(d).unwrap_or(timebase::BeatTicks(0)),
            epoch: self.scheduler.epoch(d).unwrap_or(timebase::BeatTicks(0)),
            phrase_length: self
                .scheduler
                .phrase_length(d)
                .unwrap_or(timebase::BeatTicks(0)),
            slots: std::array::from_fn(|s| {
                let state = self.scheduler.state(d, s).unwrap_or(State::Stopped);
                let pending = match state {
                    State::QueuedStart(p) => Some((PendingKind::Launch, p.part.id, p.at)),
                    State::QueuedStop { active, at } => Some((PendingKind::Stop, active.id, at)),
                    State::QueuedReplace { active, pending } => Some((
                        if active == pending.part {
                            PendingKind::Retrigger
                        } else {
                            PendingKind::Replace
                        },
                        pending.part.id,
                        pending.at,
                    )),
                    _ => None,
                }
                .map(|(kind, part, target)| PendingAction {
                    kind,
                    part,
                    target,
                    command_id: self.pending[d][s].map(|r| r.id),
                });
                PartState {
                    lifecycle: match state {
                        State::Stopped => SlotState::Stopped,
                        State::Playing(_) => SlotState::Playing,
                        State::QueuedStart(_) => SlotState::QueuedStart,
                        State::QueuedStop { .. } => SlotState::QueuedStop,
                        State::QueuedReplace { .. } => SlotState::QueuedReplace,
                    },
                    active: state.active().map(|p| p.id),
                    pending,
                    position: self.active[d][s].map(|a| a.elapsed),
                    phase: self.scheduler.part_phase(d, s),
                    parameters: self.dsp[d][s].parameters,
                    release_frames: self.transitions[d][s].remaining,
                    active_one_shot: state
                        .active()
                        .is_some_and(|p| matches!(p.playback.mode, parts::PlaybackMode::OneShot)),
                    pending_one_shot: match state {
                        State::QueuedStart(p) | State::QueuedReplace { pending: p, .. } => {
                            matches!(p.part.playback.mode, parts::PlaybackMode::OneShot)
                        }
                        State::QueuedStop { active, .. } => {
                            matches!(active.playback.mode, parts::PlaybackMode::OneShot)
                        }
                        _ => false,
                    },
                    nominal_length: state.active().map(|p| p.timing.nominal_length),
                    rule: self.scheduler.rule_status(d, s),
                }
            }),
        });
        self.queue.snapshot.publish(PerformanceState {
            transport: if self.failed {
                TransportState::Failed
            } else if self.frame().0 == 0 {
                TransportState::Ready
            } else {
                TransportState::Rendering
            },
            generation: 0,
            frame: self.frame(),
            beat: self.scheduler.beat().unwrap_or(timebase::BeatTicks(0)),
            sample_rate: self.rate(),
            bpm: self.scheduler.bpm(),
            failed: self.failed,
            decks,
        });
    }
}
impl Drop for LiveEngine {
    fn drop(&mut self) {
        self.queue.closed.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn terminal_admission_flag_closes_unconsumed_tickets() {
        // Exercise the same monotonic flag shared with the CPAL owner/error callback.
        let queue = Arc::new(Queue {
            slots: std::array::from_fn(|_| {
                std::array::from_fn(|_| SlotMeter {
                    state: AtomicU64::new(0),
                    part: AtomicU64::new(0),
                    position: AtomicU64::new(0),
                    peak: AtomicU64::new(0),
                    processed: AtomicU64::new(0),
                    parameters: AtomicU64::new(0),
                    rule: AtomicU64::new(0),
                    rule_fired: AtomicU64::new(0),
                    rule_skipped: AtomicU64::new(0),
                })
            }),
            cells: std::array::from_fn(|_| Cell {
                data: std::array::from_fn(|_| AtomicU64::new(0)),
                stamp: AtomicU64::new(0),
            }),
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
            closed: Arc::new(AtomicBool::new(false)),
            snapshot: crate::performance_state::Publication::new(),
        });
        let mut control = LiveControl {
            queue: queue.clone(),
            next: 1,
        };
        let ticket = control
            .submit(0, 0, Request::Stop, Quantize::Immediate)
            .unwrap();
        crate::cpal_backend::fail_stream(&crate::playback::Shared::default(), Some(&queue.closed));
        assert!(control.is_closed());
        assert_eq!(ticket.outcome(), Outcome::Closed);
        assert!(
            control
                .submit(0, 0, Request::Stop, Quantize::Immediate)
                .is_err()
        );
    }
}

fn encode_rule(bank: u8, rule: TriggerRule) -> u64 {
    let phase = |phase| {
        if phase == RulePhase::Deck {
            1_u64 << 10
        } else {
            0
        }
    };
    u64::from(bank)
        | match rule {
            TriggerRule::Once { boundary } => {
                let q = match boundary {
                    Quantization::Immediate => 0,
                    Quantization::Beat => 1,
                    Quantization::Bar => 2,
                    Quantization::Phrase => 3,
                };
                q << 11
            }
            TriggerRule::Every { n, unit, phase: p } => {
                ((if unit == RuleUnit::Beats { 1 } else { 2 }) << 8)
                    | phase(p)
                    | (u64::from(n) << 16)
            }
            TriggerRule::PhraseBar {
                bar,
                phrase_bars,
                phase: p,
            } => (3 << 8) | phase(p) | (u64::from(bar) << 32) | (u64::from(phrase_bars) << 40),
        }
}
fn decode_rule(bits: u64) -> (u8, TriggerRule) {
    let phase = if bits & (1 << 10) == 0 {
        RulePhase::Global
    } else {
        RulePhase::Deck
    };
    let rule = match (bits >> 8) & 3 {
        0 => TriggerRule::Once {
            boundary: match (bits >> 11) & 3 {
                0 => Quantization::Immediate,
                1 => Quantization::Beat,
                2 => Quantization::Bar,
                _ => Quantization::Phrase,
            },
        },
        1 | 2 => TriggerRule::Every {
            n: ((bits >> 16) & 65535) as u16,
            unit: if (bits >> 8) & 3 == 1 {
                RuleUnit::Beats
            } else {
                RuleUnit::Bars
            },
            phase,
        },
        _ => TriggerRule::PhraseBar {
            bar: ((bits >> 32) & 255) as u8,
            phrase_bars: ((bits >> 40) & 255) as u8,
            phase,
        },
    };
    (bits as u8, rule)
}

#[derive(Clone, Copy, Default)]
struct Transition {
    last: [f32; 2],
    held: [f32; 2],
    remaining: u32,
    total: u32,
}
impl Transition {
    fn begin(&mut self, frames: u32) {
        self.remaining = 0;
        if self.last != [0.0; 2] {
            self.held = self.last;
            self.total = frames;
            self.remaining = frames;
        }
    }
    fn process(&mut self, input: [f32; 2]) -> [f32; 2] {
        let mut output = input;
        if self.remaining > 0 {
            let old = self.remaining as f32 / self.total as f32;
            for ch in 0..2 {
                output[ch] = self.held[ch] * old + input[ch] * (1.0 - old);
            }
            self.remaining -= 1;
        }
        self.last = output;
        output
    }
}
