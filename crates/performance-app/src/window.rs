// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{
    OutputDevice, OutputDiscovery,
    cpal_backend::{CpalDiscovery, CpalPlayback, PlaybackConfig, RequestedBuffer, SampleEncoding},
    demo::{self, NAMES, RATE, TITLES},
    live::Quantize,
    routing::OutputRouting,
};
use dawj::{Coordinator, Intent, Mode, lifecycle_label, musical_position};
use eframe::egui::{self, Color32, RichText};
use std::time::Duration;
mod smoke;
pub use smoke::Smoke;

// Centralized initial bindings. Each shortcut produces the same Intent as a mouse control.
const BINDINGS: &[(egui::Key, Intent, &str)] = &[
    (egui::Key::A, Intent::Deck(0), "A / B: select deck"),
    (egui::Key::B, Intent::Deck(1), ""),
    (
        egui::Key::Num1,
        Intent::Slot(0),
        "1-8: select part on focused deck",
    ),
    (egui::Key::Num2, Intent::Slot(1), ""),
    (egui::Key::Num3, Intent::Slot(2), ""),
    (egui::Key::Num4, Intent::Slot(3), ""),
    (egui::Key::Num5, Intent::Slot(4), ""),
    (egui::Key::Num6, Intent::Slot(5), ""),
    (egui::Key::Num7, Intent::Slot(6), ""),
    (egui::Key::Num8, Intent::Slot(7), ""),
    (
        egui::Key::Space,
        Intent::Toggle,
        "Space: launch / stop selected part",
    ),
    (egui::Key::X, Intent::Stop, "X: stop selected part"),
    (
        egui::Key::Escape,
        Intent::Cancel,
        "Escape: cancel queued action by its engine ID",
    ),
    (
        egui::Key::R,
        Intent::Retrigger,
        "R: retrigger selected playing part",
    ),
    (
        egui::Key::V,
        Intent::Mode(Mode::Volume),
        "V / F: VOLUME / FILTER mode",
    ),
    (egui::Key::F, Intent::Mode(Mode::Filter), ""),
    (
        egui::Key::F1,
        Intent::Quantize(Quantize::Immediate),
        "F1 / F2 / F3 / F4: immediate / beat / bar / phrase",
    ),
    (egui::Key::F2, Intent::Quantize(Quantize::Beat), ""),
    (egui::Key::F3, Intent::Quantize(Quantize::Bar), ""),
    (egui::Key::F4, Intent::Quantize(Quantize::Phrase), ""),
    (
        egui::Key::Q,
        Intent::Cue(0),
        "Q / W: toggle cue A / B (separate channels required)",
    ),
    (egui::Key::W, Intent::Cue(1), ""),
];

pub struct PerformanceWindow {
    session: Option<Coordinator>,
    stream: Option<CpalPlayback>,
    devices: Vec<OutputDevice>,
    selected_device: usize,
    four_channels: bool,
    error: String,
    help: bool,
    bpm_edit: f64,
    smoke: Option<Smoke>,
    #[cfg(test)]
    cards: Vec<egui::Rect>,
}
impl PerformanceWindow {
    pub fn new(cc: &eframe::CreationContext<'_>, smoke: Option<Smoke>) -> Self {
        Self::configure(&cc.egui_ctx);
        let mut app = Self::empty();
        app.scan();
        app.smoke = smoke;
        app
    }
    fn configure(ctx: &egui::Context) {
        ctx.set_theme(egui::Theme::Dark);
        ctx.set_visuals(egui::Visuals::dark());
        ctx.all_styles_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 6.0);
            style.spacing.interact_size.y = 30.0;
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(16.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(16.0));
        });
    }
    fn empty() -> Self {
        Self {
            session: None,
            stream: None,
            devices: Vec::new(),
            selected_device: 0,
            four_channels: false,
            error: String::new(),
            help: false,
            bpm_edit: 120.0,
            smoke: None,
            #[cfg(test)]
            cards: Vec::new(),
        }
    }
    fn scan(&mut self) {
        match CpalDiscovery::default().output_devices() {
            Ok(devices) => {
                self.devices = devices;
                self.selected_device = 0;
                self.error = if self.devices.is_empty() {
                    "No audio output device. Connect one and rescan.".into()
                } else {
                    String::new()
                };
            }
            Err(e) => self.error = e.to_string(),
        }
    }
    fn start(&mut self) {
        if let Err(e) = self.open_output() {
            self.error = e;
        }
    }
    fn open_output(&mut self) -> Result<(), String> {
        self.stop();
        let device = self
            .devices
            .get(self.selected_device)
            .ok_or("No audio output device")?;
        let channels = if self.four_channels { 4 } else { 2 };
        let configs = device
            .configurations
            .as_ref()
            .map_err(ToString::to_string)?;
        let encoding = [SampleEncoding::F32, SampleEncoding::I16, SampleEncoding::I32, SampleEncoding::U8]
            .into_iter().find(|encoding| {
                let name = match encoding { SampleEncoding::F32 => "f32", SampleEncoding::I16 => "i16", SampleEncoding::I32 => "i32", SampleEncoding::U8 => "u8" };
                configs.iter().any(|c| c.channels == channels && c.sample_format == name && (c.min_sample_rate_hz..=c.max_sample_rate_hz).contains(&RATE))
            }).ok_or("Unsupported configuration: demo requires 48 kHz, 2 or 4 channels, f32/i16/i32/u8. Choose another output or routing.")?;
        // Recheck ordinal/name after discovery. Users explicitly rescan after device changes.
        let fresh = CpalDiscovery::default()
            .output_devices()
            .map_err(|e| e.to_string())?;
        if fresh.get(self.selected_device).map(|d| &d.name) != Some(&device.name) {
            return Err("Output enumeration changed. Rescan and select the output again.".into());
        }
        let routing = OutputRouting::new(channels, [0, 1], self.four_channels.then_some([2, 3]))?;
        let (engine, control, mixer) =
            demo::prepare().map_err(|e| format!("Demo asset / engine preparation failed: {e}"))?;
        let mut session = Coordinator::new(control, mixer, self.four_channels);
        let mut stream = CpalPlayback::open_live(
            Some(device.ordinal),
            PlaybackConfig {
                sample_rate_hz: RATE,
                channels,
                encoding,
                buffer: RequestedBuffer::Default,
            },
            routing,
            engine,
        )
        .map_err(|e| e.to_string())?;
        stream.start().map_err(|e| e.to_string())?;
        eprintln!(
            "Started {}: {:?}, {:?}",
            stream.device_name(),
            stream.config(),
            stream.routing()
        );
        session.notice = "Output started. Parts are stopped; select a slot and Launch.".into();
        self.stream = Some(stream);
        self.session = Some(session);
        self.error.clear();
        self.bpm_edit = 120.0;
        Ok(())
    }
    fn stop(&mut self) {
        if let Some(mut stream) = self.stream.take() {
            if let Some(session) = &mut self.session {
                session.poll();
                eprintln!(
                    "Closing output: {:?}; master held peaks {:?}; snapshot {:?}",
                    stream.monitor().status(),
                    session.peaks(),
                    session.snapshot
                );
            }
            if let Err(e) = stream.stop() {
                self.error = e.to_string();
            }
        }
        if let Some(session) = &mut self.session {
            session.close();
            session.poll();
        }
    }
    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx
            .memory(|m| m.focused())
            .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some())
        {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::H)) {
            self.help = !self.help;
        }
        let Some(session) = &mut self.session else {
            return;
        };
        for &(key, intent, _) in BINDINGS {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                session.act(intent);
            }
        }
        for (key, delta) in [(egui::Key::ArrowLeft, -0.05), (egui::Key::ArrowRight, 0.05)] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                session.act(Intent::Crossfader(
                    (session.parameters.crossfader + delta).clamp(0.0, 1.0),
                ));
            }
        }
    }
    fn transport(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("transport").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("DAWJ");
                if self.stream.is_some() {
                    if ui
                        .button("Stop output")
                        .on_hover_text("Terminal stop; Start creates a fresh musical clock")
                        .clicked()
                    {
                        self.stop();
                    }
                } else if ui.button("Start demo output").clicked() {
                    self.start();
                }
                let state = self.session.as_ref().and_then(|s| s.snapshot);
                if let Some(s) = state {
                    ui.strong(format!(
                        "{:?}  |  BAR {}",
                        s.transport,
                        musical_position(s.beat)
                    ));
                    ui.label(
                        s.bpm
                            .map_or("BPM unavailable".into(), |b| format!("{b:.1} BPM")),
                    );
                } else {
                    ui.label("Output offline | demo: 120 BPM");
                }
                ui.add_enabled_ui(
                    self.session.as_ref().is_some_and(Coordinator::available),
                    |ui| {
                        ui.add(
                            egui::DragValue::new(&mut self.bpm_edit)
                                .range(40.0..=240.0)
                                .speed(0.2)
                                .suffix(" BPM"),
                        );
                        if ui.button("Queue tempo").clicked() {
                            if let Some(s) = &mut self.session {
                                s.act(Intent::Tempo(self.bpm_edit));
                            }
                        }
                    },
                );
                if ui.button("Help [H]").clicked() {
                    self.help = !self.help;
                }
            });
            ui.horizontal(|ui| {
                ui.add_enabled_ui(self.stream.is_none(), |ui| {
                    egui::ComboBox::from_id_salt("output")
                        .selected_text(
                            self.devices
                                .get(self.selected_device)
                                .and_then(|d| d.name.as_ref().ok())
                                .map_or("No output", String::as_str),
                        )
                        .width(290.0)
                        .show_ui(ui, |ui| {
                            for (index, device) in self.devices.iter().enumerate() {
                                let name = device
                                    .name
                                    .as_ref()
                                    .map_or("Device name unavailable", String::as_str);
                                ui.selectable_value(&mut self.selected_device, index, name);
                            }
                        });
                    ui.checkbox(&mut self.four_channels, "4 channels: master 1/2, cue 3/4");
                    if ui.button("Rescan").clicked() {
                        self.scan();
                    }
                });
                ui.small("48 kHz | native loops; tempo edits do not stretch samples");
            });
        });
    }
    fn bottom(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("mixer").show(ctx, |ui| {
            if let Some(s) = &mut self.session {
                let (d, slot) = s.focus.address();
                ui.horizontal(|ui| {
                    ui.strong(format!("{}{} {}", ['A','B'][d], slot + 1, NAMES[slot]));
                    for (mode, label) in [(Mode::Volume, "VOLUME [V]"), (Mode::Filter, "FILTER [F]")] {
                        if ui.selectable_label(s.focus.mode == mode, label).clicked() { s.act(Intent::Mode(mode)); }
                    }
                    let part = s.snapshot.map(|state| state.decks[d].slots[slot]);
                    ui.add_enabled_ui(s.available(), |ui| {
                        if let Some(part) = part {
                            match s.focus.mode {
                                Mode::Volume => {
                                    let mut gain = part.parameters.gain;
                                    if ui.add(egui::Slider::new(&mut gain, 0.0..=1.0).text("Part gain")).changed() { s.act(Intent::Gain(gain)); }
                                }
                                Mode::Filter => {
                                    let mut cutoff = part.parameters.cutoff_hz.unwrap_or(20000.0);
                                    if ui.add(egui::Slider::new(&mut cutoff, 20.0..=20000.0).logarithmic(true).text("Hz")).changed() { s.act(Intent::Cutoff(Some(cutoff))); }
                                    if ui.button(if part.parameters.cutoff_hz.is_none() { "Bypassed" } else { "Bypass" }).clicked() { s.act(Intent::Cutoff(None)); }
                                }
                            }
                        }
                        if ui.button("Retrigger [R]").clicked() { s.act(Intent::Retrigger); }
                        if ui.button("Cancel [Esc]").clicked() { s.act(Intent::Cancel); }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label("Launch quantization");
                    for (q, label) in [(Quantize::Immediate, "Immediate"), (Quantize::Beat, "Beat"), (Quantize::Bar, "Bar"), (Quantize::Phrase, "Phrase")] {
                        if ui.selectable_label(s.focus.quantization == q, label).clicked() { s.act(Intent::Quantize(q)); }
                    }
                    ui.small("Tempo uses Beat when Immediate is selected");
                });
                ui.separator();
                ui.add_enabled_ui(s.available(), |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("A");
                        let mut x = s.parameters.crossfader;
                        ui.spacing_mut().slider_width = 350.0;
                        if ui.add(egui::Slider::new(&mut x, 0.0..=1.0).text("B  Crossfader")).changed() { s.act(Intent::Crossfader(x)); }
                        let peaks = s.peaks();
                        ui.small(format!("Master held peaks  L {:.4}  R {:.4}", peaks[0], peaks[1]));
                    });
                });
                let (color, status) = if s.stale() || !s.available() {
                    (Color32::YELLOW, "STALE / UNAVAILABLE: controls disabled; last engine state retained")
                } else { (Color32::GRAY, "LIVE: displaying coherent engine state") };
                ui.add(egui::Label::new(RichText::new(format!("{status} | {}", s.notice)).size(13.0).color(color)).truncate()).on_hover_text(&s.notice);
            } else { ui.label("Start demo output to load both compositions and enable performance controls."); }
            if self.four_channels {
                ui.small("Cue on logical channels 3/4. Physical headphone isolation and channel mapping are unverified.");
            } else {
                ui.small("STEREO MASTER ONLY: headphone cue unavailable; no private monitoring or cue fold-down.");
            }
            if let Some(stream) = &self.stream {
                let status = stream.monitor().status();
                ui.small(format!("{} | callbacks {} | max {} frames / {:.0} us | over budget {} (not a dropout count)",
                    stream.device_name(), status.callbacks, status.max_callback_frames,
                    status.callback_max_ns as f64 / 1000.0, status.callback_over_budget));
            }
            if !self.error.is_empty() { ui.colored_label(Color32::LIGHT_RED, &self.error); }
        });
    }
    fn decks(&mut self, ctx: &egui::Context) {
        #[cfg(test)]
        self.cards.clear();
        egui::CentralPanel::default().show(ctx, |ui| {
            let card_height = ((ui.available_height() - 180.0) / 4.0).clamp(78.0, 130.0);
            ui.columns(2, |columns| {
                for (d, ui) in columns.iter_mut().enumerate() {
                    let color = if d == 0 {
                        Color32::from_rgb(114, 192, 220)
                    } else {
                        Color32::from_rgb(224, 170, 108)
                    };
                    ui.horizontal(|ui| {
                        let focused = self.session.as_ref().is_some_and(|s| s.focus.deck == d);
                        if ui
                            .selectable_label(
                                focused,
                                RichText::new(format!("DECK {}", ['A', 'B'][d]))
                                    .size(22.0)
                                    .color(color),
                            )
                            .clicked()
                        {
                            if let Some(s) = &mut self.session {
                                s.act(Intent::Deck(d));
                            }
                        }
                        ui.heading(TITLES[d]);
                        if let Some(state) = self.session.as_ref().and_then(|s| s.snapshot) {
                            ui.small(format!(
                                "Phrase {}/8",
                                state.decks[d].phase.0.div_euclid(3840) + 1
                            ));
                        }
                    });
                    ui.horizontal(|ui| {
                        let available = self.session.as_ref().is_some_and(Coordinator::available);
                        ui.add_enabled_ui(available, |ui| {
                            if let Some(s) = &mut self.session {
                                ui.spacing_mut().slider_width = 160.0;
                                let mut gain = if d == 0 {
                                    s.parameters.gain_a
                                } else {
                                    s.parameters.gain_b
                                };
                                if ui
                                    .add(egui::Slider::new(&mut gain, 0.0..=1.0).text("Deck gain"))
                                    .changed()
                                {
                                    s.act(Intent::DeckGain(d, gain));
                                }
                                ui.add_enabled_ui(self.four_channels, |ui| {
                                    if ui.selectable_label(s.cues[d], "Headphone cue").clicked() {
                                        s.act(Intent::Cue(d));
                                    }
                                });
                            } else {
                                ui.label("Deck gain | cue unavailable");
                            }
                        });
                    });
                    for row in 0..4 {
                        ui.columns(2, |cards| {
                            for (col, ui) in cards.iter_mut().enumerate() {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.spacing_mut().interact_size.y = 26.0;
                                let slot = row * 2 + col;
                                let part = self
                                    .session
                                    .as_ref()
                                    .and_then(|s| s.snapshot)
                                    .map(|s| s.decks[d].slots[slot]);
                                let selected = self
                                    .session
                                    .as_ref()
                                    .is_some_and(|s| s.focus.address() == (d, slot));
                                let mut action = None;
                                let _card = egui::Frame::new()
                                    .fill(if selected {
                                        Color32::from_gray(43)
                                    } else {
                                        Color32::from_gray(28)
                                    })
                                    .stroke(egui::Stroke::new(
                                        if selected { 2.0 } else { 1.0 },
                                        if selected {
                                            color
                                        } else {
                                            Color32::from_gray(65)
                                        },
                                    ))
                                    .inner_margin(4.0)
                                    .show(ui, |ui| {
                                        ui.set_min_height(card_height);
                                        ui.set_width(ui.available_width());
                                        let feedback = self
                                            .session
                                            .as_ref()
                                            .map_or("", |s| s.feedback[d][slot].as_str());
                                        if ui
                                            .selectable_label(
                                                selected,
                                                RichText::new(format!(
                                                    "{}{}  {}{}",
                                                    ['A', 'B'][d],
                                                    slot + 1,
                                                    NAMES[slot],
                                                    if slot == 7 { " / one-shot" } else { "" }
                                                ))
                                                .strong(),
                                            )
                                            .on_hover_text(feedback)
                                            .clicked()
                                        {
                                            action = Some(Intent::Slot(slot));
                                        }
                                        if let Some(p) = part {
                                            let state_color = if p.pending.is_some() {
                                                Color32::LIGHT_YELLOW
                                            } else if p.active.is_some() {
                                                Color32::LIGHT_GREEN
                                            } else {
                                                Color32::GRAY
                                            };
                                            ui.label(
                                                RichText::new(lifecycle_label(&p))
                                                    .size(13.0)
                                                    .color(state_color),
                                            );
                                            let detail = p.pending.map_or_else(
                                                || {
                                                    p.active.map_or(
                                                        "Resident / ready".into(),
                                                        |id| {
                                                            format!(
                                                                "Active #{} | frame {}",
                                                                id.0,
                                                                p.position.unwrap_or(0)
                                                            )
                                                        },
                                                    )
                                                },
                                                |a| {
                                                    format!(
                                                        "{} > {:?} #{} @ {}",
                                                        p.active.map_or("-".into(), |id| format!(
                                                            "#{}",
                                                            id.0
                                                        )),
                                                        a.kind,
                                                        a.part.0,
                                                        musical_position(a.target)
                                                    )
                                                },
                                            );
                                            ui.add(
                                                egui::Label::new(RichText::new(&detail).size(12.0))
                                                    .truncate(),
                                            )
                                            .on_hover_text(detail);
                                        } else {
                                            ui.small("UNAVAILABLE / OUTPUT OFFLINE");
                                            ui.small("Resident demo loads at Start");
                                        }
                                        ui.add_enabled_ui(
                                            self.session
                                                .as_ref()
                                                .is_some_and(Coordinator::available),
                                            |ui| {
                                                ui.horizontal(|ui| {
                                                    if ui.button("Launch").clicked() {
                                                        action = Some(Intent::Launch);
                                                    }
                                                    if ui.button("Stop").clicked() {
                                                        action = Some(Intent::Stop);
                                                    }
                                                    if ui
                                                        .add_enabled(
                                                            part.is_some_and(|p| {
                                                                p.pending.is_some()
                                                            }),
                                                            egui::Button::new("Cancel"),
                                                        )
                                                        .clicked()
                                                    {
                                                        action = Some(Intent::Cancel);
                                                    }
                                                });
                                            },
                                        );
                                    });
                                #[cfg(test)]
                                self.cards.push(_card.response.rect);
                                if let (Some(s), Some(action)) = (&mut self.session, action) {
                                    s.act(Intent::Deck(d));
                                    s.act(Intent::Slot(slot));
                                    if !matches!(action, Intent::Slot(_)) {
                                        s.act(action);
                                    }
                                }
                            }
                        });
                        ui.add_space(6.0);
                    }
                }
            });
        });
    }
}
impl eframe::App for PerformanceWindow {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Some(s) = &mut self.session {
            s.poll();
        }
        if self
            .stream
            .as_ref()
            .is_some_and(|s| s.monitor().status().failed)
        {
            self.stop();
            self.error = "Audio stream / engine failed. Output closed. Check device, rescan, then Start for a fresh session.".into();
        }
        self.keyboard(ctx);
        self.transport(ctx);
        self.bottom(ctx);
        self.decks(ctx);
        egui::Window::new("Performance help").open(&mut self.help).resizable(false).show(ctx, |ui| {
            ui.label("Select a deck and slot, then Launch. All lifecycle labels come from the engine.");
            for &(_, _, help) in BINDINGS { if !help.is_empty() { ui.label(help); } }
            ui.label("Left / Right: crossfader by 5% | H: this help panel");
            ui.separator();
            ui.label("Controls are ignored while editing a value or text field.");
            ui.label("Stop output is terminal. Start creates a new clock and stopped demo parts.");
            ui.label("Mode and quantization selection alone do not change sound.");
        });
        ctx.request_repaint_after(Duration::from_millis(33));
        self.smoke_tick(ctx);
    }
}
impl Drop for PerformanceWindow {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_backend::live::Request;

    fn input(size: [f32; 2]) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0], size[1]),
            )),
            ..Default::default()
        }
    }
    #[test]
    fn all_sixteen_controls_fit_desktop_viewport_with_pending_states() {
        let (mut engine, mut control, mixer) = demo::prepare().unwrap();
        for d in 0..2 {
            for s in 0..8 {
                control
                    .submit(
                        d,
                        s,
                        Request::Launch((d * 8 + s) as u8),
                        Quantize::Immediate,
                    )
                    .unwrap();
            }
        }
        engine.render(&mut [0.0; 256], OutputRouting::stereo_master_only());
        for d in 0..2 {
            for s in 0..8 {
                control
                    .submit(d, s, Request::Stop, Quantize::Phrase)
                    .unwrap();
            }
        }
        engine.render(&mut [0.0; 256], OutputRouting::stereo_master_only());
        let mut app = PerformanceWindow::empty();
        app.session = Some(Coordinator::new(control, mixer, false));
        let ctx = egui::Context::default();
        PerformanceWindow::configure(&ctx);
        for size in [[1280.0, 820.0], [1100.0, 740.0]] {
            for _ in 0..3 {
                let _ = ctx.run(input(size), |ctx| {
                    app.transport(ctx);
                    app.bottom(ctx);
                    let available = ctx.available_rect();
                    app.decks(ctx);
                    assert_eq!(app.cards.len(), 16);
                    for card in &app.cards {
                        assert!(
                            available.contains_rect(*card),
                            "card {card:?} outside {available:?} at {size:?}"
                        );
                    }
                });
            }
        }
    }
    #[test]
    fn keyboard_changes_focus_but_respects_text_entry() {
        let (_engine, control, mixer) = demo::prepare().unwrap();
        let mut app = PerformanceWindow::empty();
        app.session = Some(Coordinator::new(control, mixer, false));
        let ctx = egui::Context::default();
        let mut text = String::new();
        let _ = ctx.run(input([1280.0, 820.0]), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(egui::TextEdit::singleline(&mut text).id_salt("editor"))
                    .request_focus();
            });
        });
        let mut typed = input([1280.0, 820.0]);
        typed.events.push(egui::Event::Key {
            key: egui::Key::B,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(typed.clone(), |ctx| {
            app.keyboard(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(egui::TextEdit::singleline(&mut text).id_salt("editor"));
            });
        });
        assert_eq!(app.session.as_ref().unwrap().focus.deck, 0);
        let _ = ctx.run(typed, |ctx| {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("a performance button")));
            app.keyboard(ctx);
        });
        assert_eq!(app.session.as_ref().unwrap().focus.deck, 1);
    }
}
