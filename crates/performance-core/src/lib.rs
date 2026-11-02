// SPDX-License-Identifier: GPL-3.0-or-later
//! Frontend-neutral performance control. GUI, keyboard and MIDI share Intent.
#[cfg(test)]
use audio_backend::live::SlotState;
use audio_backend::{
    live::{LiveControl, Outcome, Quantize, Request, Ticket},
    mixer::{CueSelection, MixerControl, MixerParameters},
    part_dsp::PartParameters,
    performance_state::{PartState, PerformanceState, TransportState},
};
use parts::SLOTS;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use timebase::BeatTicks;
pub mod controls;
pub mod ipc;
pub mod midi;
pub mod output;
pub mod reliability;
pub mod service;
pub mod session;
pub mod timeline;
pub mod web_contract;
pub mod web_timeline;
pub use audio_backend::performance_state;
pub use controls::{InputSource, PartAction, RuleAction};

/// Compensate the fixed demo's sample attenuation and two 1/8 mixing stages.
/// The backend's existing master/cue limiters still cap final samples at 0.125.
pub const DEMO_OUTPUT_TRIM_DB: f32 = 36.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mode {
    Volume,
    Filter,
}

#[derive(Clone, Copy, Debug)]
pub enum Intent {
    SelectPart {
        deck: usize,
        slot: usize,
    },
    Deck(usize),
    Slot(usize),
    Mode(Mode),
    Quantize(Quantize),
    Launch,
    Stop,
    Toggle,
    Retrigger,
    Cancel,
    Gain(f32),
    Cutoff(Option<f32>),
    DeckGain(usize, f32),
    Crossfader(f32),
    CrossfaderStep(f32),
    OutputTrim(f32),
    Cue(usize),
    Tempo(f64),
    Part {
        deck: usize,
        slot: usize,
        action: PartAction,
    },
    HoldMode {
        token: u16,
        mode: Mode,
        pressed: bool,
    },
    Rule {
        deck: usize,
        slot: usize,
        action: RuleAction,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Focus {
    pub deck: usize,
    pub slots: [usize; 2],
    pub mode: Mode,
    pub quantization: Quantize,
}
impl Default for Focus {
    fn default() -> Self {
        Self {
            deck: 0,
            slots: [0; 2],
            mode: Mode::Volume,
            quantization: Quantize::Beat,
        }
    }
}
impl Focus {
    pub fn address(&self) -> (usize, usize) {
        (self.deck, self.slots[self.deck])
    }
    pub fn update(&mut self, intent: Intent) -> bool {
        match intent {
            Intent::SelectPart { deck, slot } if deck < 2 && slot < SLOTS => {
                self.deck = deck;
                self.slots[deck] = slot;
            }
            Intent::Deck(d) if d < 2 => self.deck = d,
            Intent::Slot(s) if s < SLOTS => self.slots[self.deck] = s,
            Intent::Mode(m) => self.mode = m,
            Intent::Quantize(q) => self.quantization = q,
            _ => return false,
        }
        true
    }
}

pub fn musical_position(beat: BeatTicks) -> String {
    let quarters = beat.0.div_euclid(960);
    format!(
        "{}.{}",
        quarters.div_euclid(4) + 1,
        quarters.rem_euclid(4) + 1
    )
}

/// Pure projection: no submitted command can change these labels.
pub fn lifecycle_label(part: &PartState) -> &'static str {
    ipc::Lifecycle::from(part).label()
}

/// Translate performer intent using the latest actual slot and matching pending ID.
pub fn request_for(
    intent: Intent,
    focus: &Focus,
    state: &PerformanceState,
) -> Result<(Request, Quantize), &'static str> {
    let (d, s) = focus.address();
    let part = state.decks[d].slots[s];
    let q = focus.quantization;
    Ok(match intent {
        Intent::Launch => (Request::Launch((d * SLOTS + s) as u8), q),
        Intent::Stop => (Request::Stop, q),
        Intent::Toggle if part.active.is_some() => (Request::Stop, q),
        Intent::Toggle => (Request::Launch((d * SLOTS + s) as u8), q),
        Intent::Retrigger => (Request::Retrigger, q),
        Intent::Cancel => (
            Request::Cancel(
                part.pending
                    .and_then(|p| p.command_id)
                    .ok_or("no cancellable action in engine snapshot")?,
            ),
            Quantize::Immediate,
        ),
        Intent::Gain(gain) => (
            Request::Parameters(PartParameters {
                gain,
                ..part.parameters
            }),
            Quantize::Immediate,
        ),
        Intent::Cutoff(cutoff_hz) => (
            Request::Parameters(PartParameters {
                cutoff_hz,
                ..part.parameters
            }),
            Quantize::Immediate,
        ),
        Intent::Tempo(bpm) => (
            Request::Tempo(bpm),
            if q == Quantize::Immediate {
                Quantize::Beat
            } else {
                q
            },
        ),
        _ => return Err("not an engine action"),
    })
}

/// The slider is already the engine's normalized A=0 / B=1 domain.
pub fn crossfader_position(value: f32) -> Result<f32, &'static str> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err("crossfader outside 0..=1")
    }
}

struct Receipt {
    ticket: Ticket,
    deck: usize,
    slot: usize,
    parameter: bool,
    parameters: Option<PartParameters>,
}

pub struct Coordinator {
    pub banks: [[Option<u8>; SLOTS]; 2],
    pub names: [[String; SLOTS]; 2],
    control: LiveControl,
    mixer: MixerControl,
    pub focus: Focus,
    pub snapshot: Option<PerformanceState>,
    pub parameters: MixerParameters,
    pub output_trim_db: f32,
    pub cues: [bool; 2],
    pub feedback: [[String; SLOTS]; 2],
    pub notice: String,
    receipts: VecDeque<Receipt>,
    cue_available: bool,
    last_publication: Instant,
    terminal: bool,
    latest_receipt: Option<u64>,
    pub persistent_mode: Mode,
    temporary_modes: Vec<(InputSource, u16, Mode)>,
    queued_parameters: [[Option<PartParameters>; SLOTS]; 2],
    parameter_cursor: usize,
}
impl Coordinator {
    pub fn new(control: LiveControl, mixer: MixerControl, cue_available: bool) -> Self {
        mixer
            .set_output_trim_db(DEMO_OUTPUT_TRIM_DB)
            .expect("demo trim is within engine bounds");
        Self {
            banks: std::array::from_fn(|d| std::array::from_fn(|s| Some((d * SLOTS + s) as u8))),
            names: std::array::from_fn(|_| {
                std::array::from_fn(|s| audio_backend::demo::NAMES[s].into())
            }),
            snapshot: control.performance_state(),
            control,
            mixer,
            focus: Focus::default(),
            parameters: MixerParameters::default(),
            output_trim_db: DEMO_OUTPUT_TRIM_DB,
            cues: [false; 2],
            feedback: std::array::from_fn(|_| std::array::from_fn(|_| String::new())),
            notice: "Ready; start output to advance the musical clock".into(),
            receipts: VecDeque::with_capacity(32),
            cue_available,
            last_publication: Instant::now(),
            terminal: false,
            latest_receipt: None,
            persistent_mode: Mode::Volume,
            temporary_modes: Vec::with_capacity(16),
            queued_parameters: [[None; SLOTS]; 2],
            parameter_cursor: 0,
        }
    }
    pub fn install_voices(
        &mut self,
        voices: Vec<audio_backend::samples::SampleVoice>,
    ) -> Result<Vec<u8>, &'static str> {
        self.control.install_voices(voices)
    }
    fn mapped_request(
        &self,
        deck: usize,
        slot: usize,
        request: Request,
    ) -> Result<Request, &'static str> {
        Ok(match request {
            Request::Launch(_) => {
                Request::Launch(self.banks[deck][slot].ok_or("No prepared sound assigned")?)
            }
            Request::Replace(_) => {
                Request::Replace(self.banks[deck][slot].ok_or("No prepared sound assigned")?)
            }
            other => other,
        })
    }
    pub fn deck_editable(&self, deck: usize) -> bool {
        deck < 2
            && self.available()
            && !self.receipts.iter().any(|r| r.deck == deck && !r.parameter)
            && self.snapshot.is_some_and(|s| {
                s.decks[deck]
                    .slots
                    .iter()
                    .all(|p| p.active.is_none() && p.pending.is_none() && p.rule.is_none())
            })
    }
    pub fn poll(&mut self) {
        if let Some(next) = self.control.performance_state() {
            if self
                .snapshot
                .is_none_or(|old| old.generation != next.generation)
            {
                self.last_publication = Instant::now();
            }
            self.snapshot = Some(next);
        }
        if self.control.is_closed() {
            self.terminal = true;
        }
        self.receipts.retain(|r| {
            let outcome = r.ticket.outcome();
            self.feedback[r.deck][r.slot] = format!("#{} {outcome:?}", r.ticket.id());
            if self.latest_receipt == Some(r.ticket.id()) {
                self.notice = format!(
                    "Command #{} {outcome:?}; lifecycle shown from engine",
                    r.ticket.id()
                );
            }
            if matches!(
                outcome,
                Outcome::Rejected | Outcome::Closed | Outcome::Expired
            ) {
                self.notice = format!(
                    "Command #{} {outcome:?}; displayed state remains the engine snapshot",
                    r.ticket.id()
                );
            }
            matches!(outcome, Outcome::Pending | Outcome::Queued)
        });
    }
    pub fn stale(&self) -> bool {
        self.last_publication.elapsed() > Duration::from_secs(1)
    }
    pub fn available(&self) -> bool {
        !self.terminal
            && !self.control.is_closed()
            && !self.stale()
            && self.snapshot.is_some_and(|s| {
                !matches!(s.transport, TransportState::Closed | TransportState::Failed)
            })
    }
    pub fn peaks(&self) -> [f32; 2] {
        self.mixer.peaks()
    }
    pub fn close(&mut self) {
        self.terminal = true;
        self.temporary_modes.clear();
        self.focus.mode = self.persistent_mode;
        self.queued_parameters = [[None; SLOTS]; 2];
        self.notice = "Output closed; last musical display is stale".into();
    }
    pub fn act(&mut self, intent: Intent) {
        if let Err(error) = self.try_act(intent) {
            self.notice = format!("Rejected: {error}");
        }
    }
    pub fn try_act(&mut self, intent: Intent) -> Result<(), &'static str> {
        self.dispatch(intent, InputSource::Gui)
    }
    fn execute_intent(&mut self, intent: Intent) -> Result<(), &'static str> {
        if self.focus.update(intent) {
            return Ok(());
        }
        if !self.available() {
            return Err("engine unavailable, closed or snapshot stale");
        }
        let mut parameters = self.parameters;
        match intent {
            Intent::OutputTrim(db) => {
                self.mixer.set_output_trim_db(db)?;
                self.output_trim_db = db;
                return Ok(());
            }
            Intent::Crossfader(x) => parameters.crossfader = crossfader_position(x)?,
            Intent::DeckGain(0, gain) => parameters.gain_a = gain,
            Intent::DeckGain(1, gain) => parameters.gain_b = gain,
            Intent::Cue(d) if d < 2 => {
                if !self.cue_available {
                    return Err("stereo output has no independent cue channels");
                }
                self.cues[d] = !self.cues[d];
                self.mixer.set_cue(match self.cues {
                    [false, false] => CueSelection::Off,
                    [true, false] => CueSelection::A,
                    [false, true] => CueSelection::B,
                    [true, true] => CueSelection::Both,
                });
                return Ok(());
            }
            _ => {
                let state = self.snapshot.as_ref().ok_or("no engine snapshot")?;
                let (request, q) = request_for(intent, &self.focus, state)?;
                let (deck, slot) = self.focus.address();
                return self.submit_request(
                    deck,
                    slot,
                    self.mapped_request(deck, slot, request)?,
                    q,
                );
            }
        }
        self.mixer.set(parameters)?;
        self.parameters = parameters;
        Ok(())
    }
    fn submit_request(
        &mut self,
        deck: usize,
        slot: usize,
        request: Request,
        q: Quantize,
    ) -> Result<(), &'static str> {
        let parameters = if let Request::Parameters(p) = request {
            Some(p)
        } else {
            None
        };
        let parameter = parameters.is_some();
        // At most one unacknowledged knob publication per slot. Dispatch merges
        // later gestures in the bounded latest-target array before reaching here.
        if parameter
            && self
                .receipts
                .iter()
                .any(|r| r.parameter && r.deck == deck && r.slot == slot)
        {
            return Err("previous parameter update awaiting engine; try again");
        }
        if self.receipts.len() == 32 {
            return Err("UI receipt window full; wait for audio");
        }
        let ticket = self.control.submit(deck, slot, request, q)?;
        self.feedback[deck][slot] = format!("#{} submitted; awaiting engine", ticket.id());
        self.notice = self.feedback[deck][slot].clone();
        self.latest_receipt = Some(ticket.id());
        self.receipts.push_back(Receipt {
            ticket,
            deck,
            slot,
            parameter,
            parameters,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_backend::{demo, routing::OutputRouting};

    #[test]
    fn replacement_projection_preserves_both_actual_and_pending_identity() {
        let (mut engine, mut control, mixer) = demo::prepare().unwrap();
        let route = OutputRouting::stereo_master_only();
        control
            .submit(0, 0, Request::Launch(0), Quantize::Immediate)
            .unwrap();
        engine.render(&mut [0.0; 256], route);
        control
            .submit(0, 0, Request::Replace(1), Quantize::Phrase)
            .unwrap();
        engine.render(&mut [0.0; 256], route);
        let ui = Coordinator::new(control, mixer, false);
        let part = ui.snapshot.unwrap().decks[0].slots[0];
        assert_eq!(lifecycle_label(&part), "PLAYING / REPLACE");
        assert_eq!(part.active, Some(parts::PartId(1)));
        assert_eq!(part.pending.unwrap().part, parts::PartId(2));
        assert_eq!(part.pending.unwrap().target, BeatTicks(32 * 960));
    }

    #[test]
    fn stale_snapshots_and_local_shutdown_disable_submission_without_changing_parts() {
        let (_engine, control, mixer) = demo::prepare().unwrap();
        let mut ui = Coordinator::new(control, mixer, false);
        ui.last_publication = Instant::now() - Duration::from_secs(2);
        ui.poll(); // rereading an unchanged generation does not freshen it
        assert!(ui.stale());
        assert!(ui.try_act(Intent::Launch).is_err());
        assert_eq!(
            ui.snapshot.unwrap().decks[0].slots[0].lifecycle,
            SlotState::Stopped
        );
        ui.last_publication = Instant::now();
        ui.close();
        assert!(!ui.available());
        assert!(ui.try_act(Intent::Launch).is_err());
    }

    #[test]
    fn expired_and_stale_cancel_receipts_never_invent_success() {
        let (mut engine, control, mixer) = demo::prepare().unwrap();
        let mut ui = Coordinator::new(control, mixer, false);
        let route = OutputRouting::stereo_master_only();
        let mut buffer = [0.0; 256];
        engine.render(&mut buffer, route);
        ui.poll();
        ui.try_act(Intent::Launch).unwrap();
        engine.render(&mut buffer, route);
        ui.poll();
        // Supersede the pending ID from another coordinator operation, without
        // polling: cancellation must use the old observed ID and be rejected.
        ui.control
            .submit(0, 0, Request::Launch(0), Quantize::Bar)
            .unwrap();
        engine.render(&mut buffer, route);
        ui.try_act(Intent::Cancel).unwrap();
        engine.render(&mut buffer, route);
        ui.poll();
        assert!(ui.notice.contains("Rejected"));
        assert!(ui.snapshot.unwrap().decks[0].slots[0].pending.is_some());
        // Reuse receipt rows before observation to exercise explicit Expired.
        ui.try_act(Intent::Stop).unwrap();
        engine.render(&mut buffer, route);
        for _ in 0..32 {
            ui.control
                .submit(1, 0, Request::Stop, Quantize::Immediate)
                .unwrap();
            engine.render(&mut buffer, route);
        }
        ui.poll();
        assert!(ui.notice.contains("Expired"));
    }
}
