// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit opt-in native delivery check. Calls the same coordinator as UI input.
use super::*;
use audio_backend::live::{RulePhase, RuleUnit, TriggerRule};
use dawj::{PartAction, RuleAction};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

pub struct Smoke {
    audio: bool,
    started: Instant,
    step: usize,
    last_step: Instant,
    command_failed: bool,
    capture_requested: bool,
    captured: bool,
    pub result: Arc<AtomicBool>,
}
impl Smoke {
    pub fn new(audio: bool) -> Self {
        Self {
            audio,
            started: Instant::now(),
            step: 0,
            last_step: Instant::now(),
            command_failed: false,
            capture_requested: false,
            captured: false,
            result: Arc::new(AtomicBool::new(false)),
        }
    }
}
impl PerformanceWindow {
    pub(super) fn smoke_tick(&mut self, ctx: &egui::Context) {
        let Some(mut smoke) = self.smoke.take() else {
            return;
        };
        let elapsed = smoke.started.elapsed().as_secs_f32();
        if !smoke.audio {
            if let Some(midi) = &mut self.midi {
                midi.open = true;
            }
            if smoke.step == 0 {
                smoke.last_step = Instant::now();
                smoke.step = 1;
            }
        }
        if smoke.audio {
            // Space test gestures after each completed UI operation. Startup can
            // take seconds; overdue wall-clock steps must not collapse into one
            // callback admission batch or pretend earlier intents were observed.
            let ready = smoke.step != 4
                || self
                    .session
                    .as_ref()
                    .and_then(|s| s.snapshot)
                    .is_some_and(|state| state.decks[1].slots[3].active.is_some());
            if smoke.step < 8 && ready && smoke.last_step.elapsed() >= Duration::from_millis(350) {
                match smoke.step {
                    0 => {
                        let requested =
                            std::env::var("DAWJ_TEST_OUTPUT").unwrap_or_else(|_| "HyperX".into());
                        match dawj_core::reliability::test_output(
                            &self
                                .devices
                                .iter()
                                .map(|d| d.name.clone().unwrap_or_default())
                                .collect::<Vec<_>>(),
                            &requested,
                        ) {
                            Ok(index) => {
                                self.selected_device = index;
                                self.start();
                            }
                            Err(error) => {
                                self.error = error;
                                smoke.command_failed = true;
                            }
                        }
                    }
                    step => {
                        if let Some(s) = &mut self.session {
                            let intents: &[Intent] = match step {
                                1 => &[
                                    Intent::Deck(0),
                                    Intent::Slot(0),
                                    Intent::Quantize(Quantize::Beat),
                                    Intent::Launch,
                                ],
                                2 => &[Intent::Gain(0.6)],
                                3 => &[
                                    Intent::Cutoff(Some(500.0)),
                                    Intent::Deck(1),
                                    Intent::Slot(3),
                                    Intent::Launch,
                                ],
                                4 => &[Intent::Quantize(Quantize::Phrase), Intent::Stop],
                                5 => &[Intent::Cancel],
                                6 => &[
                                    Intent::Crossfader(0.7),
                                    Intent::Deck(0),
                                    Intent::Slot(0),
                                    Intent::Mode(Mode::Filter),
                                ],
                                _ => &[
                                    Intent::Rule {
                                        deck: 0,
                                        slot: 7,
                                        action: RuleAction::Install(TriggerRule::Every {
                                            n: 1,
                                            unit: RuleUnit::Bars,
                                            phase: RulePhase::Deck,
                                        }),
                                    },
                                    Intent::Part {
                                        deck: 1,
                                        slot: 7,
                                        action: PartAction::Launch,
                                    },
                                    Intent::Part {
                                        deck: 1,
                                        slot: 0,
                                        action: PartAction::Launch,
                                    },
                                    Intent::Part {
                                        deck: 0,
                                        slot: 0,
                                        action: PartAction::Retrigger,
                                    },
                                    Intent::Part {
                                        deck: 1,
                                        slot: 3,
                                        action: PartAction::Stop,
                                    },
                                ],
                            };
                            for &intent in intents {
                                if let Err(e) = s.try_act(intent) {
                                    eprintln!("Smoke command rejected: {intent:?}: {e}");
                                    smoke.command_failed = true;
                                }
                            }
                        }
                    }
                }
                smoke.step += 1;
                smoke.last_step = Instant::now();
            }
        }
        let complete = !smoke.audio || smoke.step == 8;
        let settled = complete && smoke.last_step.elapsed() >= Duration::from_millis(800);
        if settled && !smoke.capture_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            smoke.capture_requested = true;
        }
        ctx.input(|input| {
            for event in &input.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    let capture = std::fs::create_dir_all("target")
                        .map_err(|e| e.to_string())
                        .and_then(|()| {
                            image::save_buffer(
                                "target/m3-native-capture.png",
                                &pixels,
                                image.width() as u32,
                                image.height() as u32,
                                image::ColorType::Rgba8,
                            )
                            .map_err(|e| e.to_string())
                        });
                    smoke.captured = capture.is_ok();
                    if let Err(e) = capture {
                        eprintln!("Smoke capture failed: {e}");
                    }
                }
            }
        });
        if (settled && smoke.captured) || elapsed >= 15.0 {
            let engine_ok = if smoke.audio {
                self.session.as_ref().is_some_and(|s| {
                    let actual = s.snapshot.is_some_and(|state| {
                        let a = state.decks[0].slots[0];
                        let b = state.decks[1].slots[3];
                        a.active == Some(parts::PartId(1))
                            && b.active == Some(parts::PartId(12))
                            && a.pending.is_some_and(|p| {
                                p.kind == audio_backend::performance_state::PendingKind::Retrigger
                            })
                            && b.pending.is_some_and(|p| {
                                p.kind == audio_backend::performance_state::PendingKind::Stop
                            })
                            && state.decks[1].slots[7].pending_one_shot
                            && state.decks[0].slots[7]
                                .rule
                                .is_some_and(|r| r.one_shot && r.next.is_some())
                            && (a.parameters.gain - 0.6).abs() < 0.001
                            && a.parameters.cutoff_hz == Some(500.0)
                    });
                    actual && s.available() && s.peaks().into_iter().any(|p| p > 0.02)
                }) && self.stream.as_ref().is_some_and(|s| {
                    let status = s.monitor().status();
                    status.callbacks > 0 && !status.failed
                })
            } else {
                true
            };
            let passed = smoke.captured && engine_ok && !smoke.command_failed;
            if let Some(midi) = &self.midi {
                eprintln!("MIDI_STATUS {:?}", midi.devices.status());
            }
            eprintln!(
                "SMOKE_RESULT passed={passed} audio={} captured={} error={}",
                smoke.audio, smoke.captured, self.error
            );
            smoke.result.store(passed, Ordering::Relaxed);
            self.stop();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.smoke = Some(smoke);
        }
    }
}
