// SPDX-License-Identifier: GPL-3.0-or-later
//! Release-build automation only; application/control thread, never audio.
use super::*;
use dawj_core::reliability::{Timing, test_output};
use std::time::Instant;
pub struct Bench {
    started: Option<Instant>,
    last: Instant,
    change: Instant,
    count: u64,
    ack: Timing,
    frames: Timing,
    display: Timing,
    pending: Option<Instant>,
    duration: u64,
    failed: bool,
}
impl Bench {
    pub fn new() -> Self {
        Self {
            started: None,
            last: Instant::now(),
            change: Instant::now(),
            count: 0,
            ack: Timing::default(),
            frames: Timing::default(),
            display: Timing::default(),
            pending: None,
            duration: std::env::var("DAWJ_TEST_SECONDS")
                .ok()
                .and_then(|n| n.parse().ok())
                .filter(|n| (5..=3600).contains(n))
                .unwrap_or(1800),
            failed: false,
        }
    }
}
impl PerformanceWindow {
    pub(super) fn reliability_tick(&mut self, ctx: &egui::Context) {
        let Some(mut b) = self.bench.take() else {
            return;
        };
        if b.started.is_none() {
            let output = std::env::var("DAWJ_TEST_OUTPUT").unwrap_or_else(|_| "HyperX".into());
            match test_output(
                &self
                    .devices
                    .iter()
                    .map(|d| d.name.clone().unwrap_or_default())
                    .collect::<Vec<_>>(),
                &output,
            ) {
                Ok(index) => {
                    self.selected_device = index;
                    self.start();
                }
                Err(e) => self.error = e,
            }
            if let Some(s) = &mut self.session {
                for intent in [
                    Intent::Quantize(Quantize::Immediate),
                    Intent::Part {
                        deck: 0,
                        slot: 0,
                        action: dawj::PartAction::Launch,
                    },
                    Intent::Part {
                        deck: 1,
                        slot: 3,
                        action: dawj::PartAction::Launch,
                    },
                ] {
                    if s.try_act(intent).is_err() {
                        b.failed = true
                    }
                }
            } else {
                b.failed = true;
            }
            eprintln!(
                "RELIABILITY_OUTPUT {}",
                self.devices
                    .get(self.selected_device)
                    .map(|d| d.name.as_deref().unwrap_or("unavailable"))
                    .unwrap_or("unavailable")
            );
            b.started = Some(Instant::now());
            b.last = Instant::now();
        } else {
            b.frames.record(b.last.elapsed().as_secs_f64() * 1000.0);
            b.last = Instant::now();
            if let Some(p) = b.pending.take() {
                b.display.record(p.elapsed().as_secs_f64() * 1000.0)
            }
            if b.change.elapsed() >= Duration::from_millis(50) {
                b.change = Instant::now();
                let x = (b.count % 100) as f32 / 100.0;
                if let Some(s) = &mut self.session {
                    for intent in [
                        Intent::Crossfader(x),
                        Intent::Gain(0.3 + x * 0.5),
                        Intent::Cutoff(Some(200.0 + x * 10000.0)),
                    ] {
                        let start = Instant::now();
                        if s.try_act(intent).is_err() {
                            b.failed = true
                        }
                        b.ack.record(start.elapsed().as_secs_f64() * 1000.0);
                    }
                    b.pending = Some(Instant::now());
                    b.count += 1;
                    if !s.available() {
                        b.failed = true;
                    }
                } else {
                    b.failed = true;
                }
            }
        }
        if b.failed || b.started.unwrap().elapsed().as_secs() >= b.duration {
            let playing = self
                .session
                .as_ref()
                .and_then(|s| s.snapshot)
                .is_some_and(|s| {
                    s.decks[0].slots[0].active.is_some() && s.decks[1].slots[3].active.is_some()
                });
            let passed = !b.failed && playing;
            eprintln!(
                "RELIABILITY_METRICS {{\"frontend\":\"egui\",\"passed\":{passed},\"seconds\":{},\"changes\":{},\"command_ms\":{},\"frame_ms\":{},\"receipt_to_frame_ms\":{}}}",
                b.started.unwrap().elapsed().as_secs_f64(),
                b.count,
                b.ack.summary(),
                b.frames.summary(),
                b.display.summary()
            );
            self.stop();
            if !passed {
                self.error = "Reliability scenario failed".into();
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.bench = Some(b);
            ctx.request_repaint();
        }
    }
}
