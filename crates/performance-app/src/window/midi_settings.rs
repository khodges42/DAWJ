// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use dawj::midi::{
    Learn, Mapper,
    device::{DeviceStatus, Devices},
    profile::*,
};
use std::{path::PathBuf, time::Instant};

pub(super) struct MidiPanel {
    pub open: bool,
    pub mapper: Mapper,
    pub devices: Devices,
    state: DeviceStatus,
    path: PathBuf,
    notice: String,
    edit: Mapping,
    selected: Option<usize>,
    target: Target,
    dropped: u64,
    feedback_tick: Instant,
    led_edit: Vec<Led>,
    led_values: LedValues,
}
impl MidiPanel {
    pub fn badge(&self) -> &'static str {
        if self.state.input.is_none() {
            "MIDI: off"
        } else if self.mapper.status.starts_with("Pickup") {
            "MIDI: pickup"
        } else if self.mapper.status.contains("rejected") {
            "MIDI: rejected"
        } else {
            "MIDI: on"
        }
    }
    pub fn hint(&self) -> String {
        format!(
            "Open MIDI settings | {} | {}",
            self.state.message, self.mapper.status
        )
    }
    pub fn new() -> Self {
        let path = default_path();
        let (profile, notice) = dawj::midi::runtime::load_profile(&path);
        let devices = Devices::new(profile.input.clone(), profile.output.clone());
        let edit = profile.mappings.first().cloned().unwrap_or_else(|| {
            Mapping::new(
                Control {
                    channel: 0,
                    kind: Kind::Note,
                    number: 0,
                },
                Action::Launch(Target::Selected),
            )
        });
        let led_edit = profile.leds.clone();
        let led_values = profile.values;
        Self {
            open: false,
            mapper: Mapper::new(profile).expect("validated profile"),
            devices,
            state: DeviceStatus::default(),
            path,
            notice,
            edit,
            selected: None,
            target: Target::Selected,
            dropped: 0,
            feedback_tick: Instant::now(),
            led_edit,
            led_values,
        }
    }
    pub fn reset(&mut self, co: Option<&mut Coordinator>) {
        self.mapper.reset(co);
    }
    pub fn poll(&mut self, co: Option<&mut Coordinator>) {
        dawj::midi::runtime::poll(
            &mut self.mapper,
            &self.devices,
            &mut self.state,
            &mut self.dropped,
            &mut self.feedback_tick,
            co,
        );
    }
    pub fn shutdown(&mut self) {
        dawj::midi::runtime::shutdown(&self.mapper, &mut self.devices);
    }
    pub fn show(&mut self, ctx: &egui::Context, mut co: Option<&mut Coordinator>) {
        let revision = self.mapper.revision;
        let mut open = self.open;
        egui::Window::new("MIDI devices and mappings").open(&mut open).default_width(650.0).default_height(650.0).vscroll(true).show(ctx,|ui| {
            ui.label(format!("Input: {} | Output: {}",self.state.input.as_deref().unwrap_or("disconnected"),self.state.output.as_deref().unwrap_or("disconnected")));
            ui.label(&self.state.message);ui.label(&self.mapper.status);
            ui.small(format!("Input drops: {} | {}",self.dropped,self.path.display()));
            device_combo(ui,"MIDI input",&self.state.inputs,&mut self.mapper.profile.input);
            device_combo(ui,"MIDI output",&self.state.outputs,&mut self.mapper.profile.output);
            ui.horizontal(|ui| {
                if ui.button("Connect / retry").clicked() {
                    self.mapper.reset(co.as_deref_mut());
                    self.notice=match self.devices.connect(self.mapper.profile.input.clone(),self.mapper.profile.output.clone()) {Ok(())=>"Connection requested; save to remember device choices".into(),Err(e)=>e};
                }
                if ui.button("Disconnect").clicked() {self.mapper.reset(co.as_deref_mut());if let Err(e)=self.devices.connect(None,None){self.notice=e;}}
                if ui.button("Rescan").clicked()&& let Err(e)=self.devices.rescan(){self.notice=e;}
            });
            ui.horizontal(|ui| {
                if ui.button("Save profile").clicked() {self.notice=match self.mapper.profile.save(&self.path){Ok(())=>{self.mapper.dirty=false;"Profile saved".into()},Err(e)=>e};}
                if ui.button("Load saved").clicked() {match Profile::load(&self.path) {
                    Ok(p)=>{self.led_edit=p.leds.clone();self.led_values=p.values;self.notice=match self.mapper.replace_profile(p,co.as_deref_mut()){Ok(())=>"Profile loaded; Connect to apply device choices".into(),Err(e)=>e};},Err(e)=>self.notice=e,
                }}
                if ui.button("Factory map").clicked() {let p=Profile::default();self.led_edit=p.leds.clone();self.led_values=p.values;let _=self.mapper.replace_profile(p,co.as_deref_mut());self.mapper.dirty=true;}
                if self.mapper.dirty {ui.label("Unsaved mappings");}
            });
            ui.colored_label(Color32::LIGHT_YELLOW,&self.notice);
            ui.separator();ui.strong("Mapping editor: select a row or create a new binding");
            control_editor(ui,"input",&mut self.edit.input);
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("target").selected_text(format!("{:?}",self.target)).show_ui(ui,|ui| {
                    ui.selectable_value(&mut self.target,Target::Selected,"Selected part");
                    for slot in 0..8 {ui.selectable_value(&mut self.target,Target::FocusedDeck{slot},format!("Focused deck, slot {}",slot+1));}
                    for deck in 0..2 {for slot in 0..8 {ui.selectable_value(&mut self.target,Target::DeckSlot{deck,slot},format!("{}{}",['A','B'][deck],slot+1));}}
                });
                self.edit.action=self.edit.action.with_target(self.target);
                egui::ComboBox::from_id_salt("action").selected_text(format!("{:?}",self.edit.action)).show_ui(ui,|ui| {
                    for action in actions(self.target) {ui.selectable_value(&mut self.edit.action,action,format!("{action:?}"));}
                });
            });
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("behavior").selected_text(format!("{:?}",self.edit.behavior)).show_ui(ui,|ui| {
                    for b in [Behavior::Button,Behavior::Absolute,Behavior::RelativeTwosComplement] {ui.selectable_value(&mut self.edit.behavior,b,format!("{b:?}"));}
                });
                ui.checkbox(&mut self.edit.invert,"Invert");
                ui.label("Min/max");ui.add(egui::DragValue::new(&mut self.edit.min).range(0..=126));ui.add(egui::DragValue::new(&mut self.edit.max).range(1..=127));
                ui.add(egui::DragValue::new(&mut self.edit.step).range(0.0001..=1.0).speed(0.001).prefix("Relative step "));
            });
            ui.horizontal(|ui| {
                if ui.button("Add binding").clicked() {self.notice=match self.mapper.install(self.edit.clone(),None,co.as_deref_mut()){Ok(())=>"Binding added; save profile".into(),Err(e)=>e};}
                if ui.add_enabled(self.selected.is_some(),egui::Button::new("Replace selected")).clicked() {self.notice=match self.mapper.install(self.edit.clone(),self.selected,co.as_deref_mut()){Ok(())=>"Binding replaced; save profile".into(),Err(e)=>e};}
                if ui.button("Learn input (new)").clicked() {self.mapper.learn=Some(Learn{mapping:self.edit.clone(),replace:None});self.mapper.status="Learning: move one CC or press a note; conflicting inputs are rejected".into();}
                if ui.add_enabled(self.selected.is_some(),egui::Button::new("Learn selected")).clicked() {self.mapper.learn=Some(Learn{mapping:self.edit.clone(),replace:self.selected});}
                if ui.button("Cancel learn").clicked() {self.mapper.learn=None;}
            });
            let mut remove=None;
            for (i,m) in self.mapper.profile.mappings.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.selected==Some(i),format!("Ch{} {:?} {} → {:?} ({:?})",m.input.channel+1,m.input.kind,m.input.number,m.action,m.behavior)).clicked() {self.selected=Some(i);self.edit=m.clone();self.target=m.action.target().unwrap_or(Target::Selected);}
                    if ui.small_button("Remove").clicked(){remove=Some(i);}
                });
            }
            if let Some(i)=remove {let mut p=self.mapper.profile.clone();p.mappings.remove(i);let _=self.mapper.replace_profile(p,co.as_deref_mut());self.selected=None;self.mapper.dirty=true;}
            egui::CollapsingHeader::new("Generic Note/CC LED feedback").show(ui,|ui| {
                ui.small("Values are device-dependent, not RGB colors. Feedback always uses engine lifecycle.");
                ui.horizontal(|ui| {
                    for (label,value) in [("Stopped",&mut self.led_values.stopped),("Queued",&mut self.led_values.queued),("Playing",&mut self.led_values.playing),("Stop queued",&mut self.led_values.stopping),("Replace",&mut self.led_values.replacing)] {ui.label(label);ui.add(egui::DragValue::new(value).range(0..=127));}
                });
                let mut remove=None;
                for (i,l) in self.led_edit.iter_mut().enumerate() {ui.push_id(i,|ui| {
                    control_editor(ui,"led",&mut l.output);
                    ui.horizontal(|ui| {ui.label("Deck 0=A / 1=B, slot 0–7");ui.add(egui::DragValue::new(&mut l.deck).range(0..=1));ui.add(egui::DragValue::new(&mut l.slot).range(0..=7));if ui.small_button("Remove LED").clicked(){remove=Some(i);}});
                });}
                if let Some(i)=remove{self.led_edit.remove(i);}
                ui.horizontal(|ui| {
                    if self.led_edit.len()<MAX_LEDS && ui.button("Add LED").clicked(){self.led_edit.push(Led{output:Control{channel:0,kind:Kind::Note,number:0},deck:0,slot:0});}
                    if ui.button("Apply LED changes").clicked(){let mut p=self.mapper.profile.clone();p.leds=self.led_edit.clone();p.values=self.led_values;self.notice=match self.mapper.replace_profile(p,co){Ok(())=>{self.mapper.dirty=true;"LED configuration applied; save profile".into()},Err(e)=>e};}
                });
            });
        });
        self.open = open;
        if self.mapper.revision != revision {
            for _ in self
                .devices
                .events
                .try_iter()
                .take(dawj::midi::device::EVENT_CAPACITY)
            {}
        }
    }
}
fn device_combo(ui: &mut egui::Ui, label: &str, names: &[String], selected: &mut Option<String>) {
    egui::ComboBox::from_id_salt(label)
        .selected_text(format!(
            "{label}: {}",
            selected.as_deref().unwrap_or("None")
        ))
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "None");
            for name in names {
                ui.selectable_value(selected, Some(name.clone()), name);
            }
        });
}
fn control_editor(ui: &mut egui::Ui, id: &str, c: &mut Control) {
    ui.horizontal(|ui| {
        let mut channel = c.channel + 1;
        ui.label("Channel");
        ui.add(egui::DragValue::new(&mut channel).range(1..=16));
        c.channel = channel - 1;
        egui::ComboBox::from_id_salt(id)
            .selected_text(format!("{:?}", c.kind))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut c.kind, Kind::Note, "Note");
                ui.selectable_value(&mut c.kind, Kind::CC, "CC");
            });
        ui.label("Number");
        ui.add(egui::DragValue::new(&mut c.number).range(0..=127));
    });
}
fn actions(t: Target) -> Vec<Action> {
    let mut a = vec![
        Action::Launch(t),
        Action::Stop(t),
        Action::Cancel(t),
        Action::Retrigger(t),
        Action::Toggle(t),
        Action::Gain(t),
        Action::Cutoff(t),
        Action::Context(t),
        Action::Crossfader,
        Action::Mode(Mode::Volume),
        Action::Mode(Mode::Filter),
        Action::Momentary(Mode::Volume),
        Action::Momentary(Mode::Filter),
    ];
    for d in 0..2 {
        a.extend([Action::SelectDeck(d), Action::DeckGain(d), Action::Cue(d)]);
        for s in 0..8 {
            a.push(Action::SelectPart { deck: d, slot: s });
        }
    }
    for s in 0..8 {
        a.push(Action::SelectSlot(s));
    }
    for q in [Grid::Immediate, Grid::Beat, Grid::Bar, Grid::Phrase] {
        a.push(Action::Quantize(q));
    }
    a
}
