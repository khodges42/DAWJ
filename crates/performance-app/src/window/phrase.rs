// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use audio_backend::{
    live::{RulePhase, RuleUnit, TriggerRule},
    performance_state::PerformanceState,
};
use dawj::{
    PartAction, RuleAction,
    timeline::{self, Marker, MarkerKind},
};

impl PerformanceWindow {
    pub(super) fn phrase_timeline(&mut self, ui: &mut egui::Ui) {
        let state = self.session.as_ref().and_then(|s| s.snapshot);
        let available = self.session.as_ref().is_some_and(Coordinator::available);
        let mut actions = Vec::new();
        ui.horizontal(|ui| {
            ui.strong("PHRASE");
            ui.add_enabled_ui(self.stream.is_none(),|ui| {
                egui::ComboBox::from_id_salt("phrase-bars").width(66.0).selected_text(format!("{} bars",self.phrase_bars)).show_ui(ui,|ui| {
                    ui.selectable_value(&mut self.phrase_bars,4,"4 bars");ui.selectable_value(&mut self.phrase_bars,8,"8 bars");
                }).response.on_hover_text("Phrase length is set when starting a new engine. Stop output before changing it.");
            });
            let phrase_bars=state.map_or(self.phrase_bars,|s|(s.decks[0].phrase_length.0/timeline::BAR).max(1) as u8);
            self.phrase_view.zoom_bars=self.phrase_view.zoom_bars.min(phrase_bars);
            egui::ComboBox::from_id_salt("phrase-zoom").width(65.0).selected_text(format!("View {}b",self.phrase_view.zoom_bars)).show_ui(ui,|ui| {
                for n in [1,2,4,8].into_iter().filter(|n|*n<=phrase_bars){if ui.selectable_value(&mut self.phrase_view.zoom_bars,n,format!("{n} bars")).changed(){self.phrase_view.page=0;}}
            });
            if let Some(s)=state {
                if ui.small_button("<").on_hover_text("Previous page in this phrase; stops following").clicked(){self.phrase_view.page_by(&s,-1);}
                if ui.small_button(">").on_hover_text("Next page in this phrase; stops following").clicked(){self.phrase_view.page_by(&s,1);}
                let mut following=self.phrase_view.follow;
                if ui.checkbox(&mut following,"Follow").changed(){self.phrase_view.set_follow(following,&s);}
                ui.small(format!("Global {}/{}",s.beat.0.rem_euclid(s.decks[0].phrase_length.0)/timeline::BAR+1,s.decks[0].phrase_length.0/timeline::BAR));
            }else{ui.small("Start output for musical time");}
            for deck in 0..2 {
                if ui.add_enabled(available,egui::Button::new(format!("Fill {}",['A','B'][deck]))).on_hover_text("Launch the supplied one-shot fill in slot 8 using current quantization").clicked(){actions.push(Intent::Part{deck,slot:7,action:PartAction::Launch});}
                if ui.add_enabled(available && state.is_some_and(|s|s.decks[deck].slots[7].pending.is_some()),egui::Button::new(format!("Cancel {}",['A','B'][deck]))).on_hover_text("Cancel slot 8's observed manual pending action; does not stop an active fill or remove a rule").clicked(){actions.push(Intent::Part{deck,slot:7,action:PartAction::Cancel});}
            }
        });
        if let Some(s) = state {
            let w = self.phrase_view.window(&s);
            let markers = timeline::markers(&s, w);
            let (rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 76.0), egui::Sense::hover());
            let left = rect.left() + 118.0;
            let right = rect.right() - 12.0;
            let x = |at: timebase::BeatTicks| left + w.fraction(at) * (right - left);
            let p = ui.painter();
            p.rect_filled(rect, 2.0, Color32::from_gray(22));
            for tick in (w.start.0..=w.end.0).step_by(960) {
                let bar = tick % timeline::BAR == 0;
                let px = x(timebase::BeatTicks(tick));
                p.line_segment(
                    [
                        egui::pos2(px, rect.top() + 14.0),
                        egui::pos2(px, rect.bottom()),
                    ],
                    egui::Stroke::new(1.0, Color32::from_gray(if bar { 70 } else { 36 })),
                );
                if bar {
                    p.text(
                        egui::pos2(px + 2.0, rect.top()),
                        egui::Align2::LEFT_TOP,
                        format!(
                            "{}",
                            (tick - w.phrase_start.0).rem_euclid(w.phrase_length.0) / timeline::BAR
                                + 1
                        ),
                        egui::FontId::proportional(11.0),
                        Color32::GRAY,
                    );
                }
            }
            let selected = self
                .session
                .as_ref()
                .map(|c| c.focus.address())
                .unwrap_or((0, 0));
            for lane in 0..3 {
                let y = rect.top() + 26.0 + lane as f32 * 19.0;
                let color = if lane == 0 {
                    Color32::from_rgb(114, 192, 220)
                } else if lane == 1 {
                    Color32::from_rgb(224, 170, 108)
                } else {
                    Color32::LIGHT_GREEN
                };
                let label = if lane < 2 {
                    format!(
                        "{}  {} playing",
                        ['A', 'B'][lane],
                        s.decks[lane]
                            .slots
                            .iter()
                            .filter(|p| p.active.is_some())
                            .count()
                    )
                } else {
                    let part = s.decks[selected.0].slots[selected.1];
                    format!(
                        "{}{} cycle {}",
                        ['A', 'B'][selected.0],
                        selected.1 + 1,
                        part.phase.map_or("stopped".into(), musical_position)
                    )
                };
                ui.painter().text(
                    egui::pos2(rect.left() + 5.0, y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(12.0),
                    color,
                );
                let hit = ui.interact(
                    egui::Rect::from_min_max(
                        egui::pos2(rect.left(), y - 8.0),
                        egui::pos2(left - 2.0, y + 8.0),
                    ),
                    ui.id().with(("phrase-lane", lane)),
                    egui::Sense::click(),
                );
                if lane < 2 && hit.clicked() {
                    actions.push(Intent::Deck(lane));
                }
                if lane == 2 {
                    hit.on_hover_text(timeline::inspector(s.decks[selected.0].slots[selected.1]));
                }
                let mut groups: Vec<(timebase::BeatTicks, Vec<Marker>)> = Vec::new();
                for m in markers.iter().filter(|m| {
                    if lane < 2 {
                        m.deck == lane
                    } else {
                        (m.deck, m.slot) == selected
                    }
                }) {
                    if let Some((_, g)) = groups.iter_mut().find(|(at, _)| *at == m.at) {
                        g.push(*m);
                    } else {
                        groups.push((m.at, vec![*m]));
                    }
                }
                for (at, group) in groups {
                    let first = group.iter().find(|m| m.manual).unwrap_or(&group[0]);
                    let label = timeline::marker_label(&group);
                    let width = (label.len() as f32 * 6.5 + 6.0).clamp(24.0, 96.0);
                    let position =
                        egui::pos2(x(at).clamp(left + width / 2.0, right - width / 2.0), y);
                    let hit_rect = egui::Rect::from_center_size(position, egui::vec2(width, 17.0));
                    let hit = ui.interact(
                        hit_rect,
                        ui.id().with(("phrase-marker", lane, at.0)),
                        egui::Sense::click(),
                    );
                    #[cfg(test)]
                    self.timeline_hits.push((first.deck, first.slot, hit_rect));
                    let fill = if group.iter().all(|m| m.projected) {
                        Color32::from_gray(30)
                    } else {
                        Color32::from_gray(60)
                    };
                    ui.painter().rect_filled(hit_rect, 2.0, fill);
                    let ink = match first.kind {
                        MarkerKind::Stop => Color32::LIGHT_RED,
                        MarkerKind::Fill | MarkerKind::RuleFill => Color32::LIGHT_YELLOW,
                        MarkerKind::Fired => Color32::LIGHT_GREEN,
                        _ => color,
                    };
                    ui.painter().text(
                        position,
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(11.0),
                        ink,
                    );
                    if hit.clicked() {
                        actions.push(Intent::Deck(first.deck));
                        actions.push(Intent::Slot(first.slot));
                    }
                    hit.context_menu(|ui| {
                        for m in &group {
                            ui.horizontal(|ui| {
                                if ui
                                    .button(format!(
                                        "Select {}{} {}",
                                        ['A', 'B'][m.deck],
                                        m.slot + 1,
                                        m.kind.glyph()
                                    ))
                                    .clicked()
                                {
                                    actions.push(Intent::Deck(m.deck));
                                    actions.push(Intent::Slot(m.slot));
                                    ui.close_menu();
                                }
                                if available && m.manual && ui.button("Cancel queued").clicked() {
                                    actions.push(Intent::Part {
                                        deck: m.deck,
                                        slot: m.slot,
                                        action: PartAction::Cancel,
                                    });
                                    ui.close_menu();
                                }
                                if available && !m.manual && ui.button("Remove rule").clicked() {
                                    actions.push(Intent::Rule {
                                        deck: m.deck,
                                        slot: m.slot,
                                        action: RuleAction::Cancel,
                                    });
                                    ui.close_menu();
                                }
                            });
                        }
                    });
                    hit.on_hover_text(
                        group
                            .iter()
                            .map(|m| {
                                format!(
                                    "{}{} {:?} @ {}{}{}",
                                    ['A', 'B'][m.deck],
                                    m.slot + 1,
                                    m.kind,
                                    musical_position(m.at),
                                    if m.projected {
                                        " (recurring projection; may skip)"
                                    } else if !m.manual && m.kind != MarkerKind::Fired {
                                        " (next rule; may skip)"
                                    } else {
                                        ""
                                    },
                                    m.command_id.map_or(String::new(), |id| format!(" #{id}"))
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                }
            }
            if s.beat >= w.start && s.beat <= w.end {
                let px = x(s.beat);
                ui.painter().line_segment(
                    [
                        egui::pos2(px, rect.top() + 13.0),
                        egui::pos2(px, rect.bottom()),
                    ],
                    egui::Stroke::new(1.5, Color32::WHITE),
                );
            }
            self.rule_inspector(ui, &s, available, &mut actions);
        } else {
            ui.small("L launch | S stop | R retrigger | F fill | P periodic | ! last fired. No engine snapshot.");
        }
        if let Some(co) = &mut self.session {
            for intent in actions {
                co.act(intent);
            }
        }
    }
    fn rule_inspector(
        &mut self,
        ui: &mut egui::Ui,
        s: &PerformanceState,
        available: bool,
        actions: &mut Vec<Intent>,
    ) {
        let Some(co) = &self.session else {
            return;
        };
        let (deck, slot) = co.focus.address();
        let part = s.decks[deck].slots[slot];
        ui.horizontal(|ui| {
            ui.strong(format!("{}{}",['A','B'][deck],slot+1)).on_hover_text(timeline::inspector(part));
            ui.add_enabled_ui(available,|ui| {
                if ui.button("Once").on_hover_text("Install a one-time trigger using current quantization").clicked(){actions.push(Intent::Rule{deck,slot,action:RuleAction::Install(TriggerRule::Once{boundary:co.focus.quantization})});}
                egui::ComboBox::from_id_salt("period-kind").width(82.0).selected_text(if self.rule_kind==0{"Every N bars"}else{"Phrase bar"}).show_ui(ui,|ui| {ui.selectable_value(&mut self.rule_kind,0,"Every N bars");ui.selectable_value(&mut self.rule_kind,1,"Phrase bar");});
                let max=if self.rule_kind==0{32}else{(s.decks[deck].phrase_length.0/timeline::BAR) as u16};
                self.rule_number=self.rule_number.clamp(1,max);ui.add(egui::DragValue::new(&mut self.rule_number).range(1..=max));
                if ui.button("Arm").on_hover_text("Install or replace the selected slot's rule in the engine").clicked(){
                    let rule=if self.rule_kind==0{TriggerRule::Every{n:self.rule_number,unit:RuleUnit::Bars,phase:RulePhase::Deck}}else{TriggerRule::PhraseBar{bar:self.rule_number as u8,phrase_bars:(s.decks[deck].phrase_length.0/timeline::BAR) as u8,phase:RulePhase::Deck}};
                    actions.push(Intent::Rule{deck,slot,action:RuleAction::Install(rule)});
                }
                let enabled=part.rule.is_some_and(|r|r.next.is_some());
                if ui.add_enabled(part.rule.is_some_and(|r|!r.consumed),egui::Button::new(if enabled{"Pause"}else{"Resume"})).clicked(){actions.push(Intent::Rule{deck,slot,action:RuleAction::Enable(!enabled)});}
                if ui.add_enabled(part.rule.is_some(),egui::Button::new("Remove rule")).clicked(){actions.push(Intent::Rule{deck,slot,action:RuleAction::Cancel});}
                if ui.button("Trigger").clicked(){actions.push(Intent::Part{deck,slot,action:PartAction::Launch});}
                if ui.add_enabled(part.pending.is_some(),egui::Button::new("Cancel queued")).clicked(){actions.push(Intent::Part{deck,slot,action:PartAction::Cancel});}
            });
            let status=part.rule.map_or("No rule".into(),|r|format!("{} | fired {}/skip {}",r.next.map_or(if r.consumed{"Spent".into()}else{"Paused".into()},|t|format!("Next {}",musical_position(t))),r.fired,r.skipped));
            ui.add(egui::Label::new(RichText::new(status).size(11.0)).truncate()).on_hover_text(timeline::inspector(part));
            ui.small("L/S/R/F/P/!").on_hover_text("Launch / Stop / Retrigger / Fill / Periodic / last fired; X = replacement. Future recurrence projections are conditional. Right-click markers for actions.");
        });
    }
}
