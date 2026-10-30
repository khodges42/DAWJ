// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{demo, live::Quantize, routing::OutputRouting};
use dawj_core::{
    Coordinator, InputSource, Intent, Mode,
    ipc::{Action, Grid, Phase, Rule, Unit},
    midi::{
        self, Mapper,
        profile::{Control, Kind, Mapping, Profile, Target},
    },
    timeline::View,
    web_timeline::Projection,
};
#[test]
fn generated_frontend_fixture_is_current_and_rust_deserializes_every_command() {
    let file = include_str!("../../tauri-app/web/contract.generated.mjs").replace("\r\n", "\n");
    assert_eq!(file, dawj_core::web_contract::generated());
    let json = file
        .split("export const contract = ")
        .nth(1)
        .unwrap()
        .trim()
        .trim_end_matches(';');
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    for command in value["commands"].as_array().unwrap() {
        let _: dawj_core::ipc::Command = serde_json::from_value(command.clone()).unwrap();
    }
    for action in value["actions"].as_array().unwrap() {
        let _: Action = serde_json::from_value(action.clone()).unwrap();
    }
}
#[test]
fn atomic_selection_selected_actions_and_incremental_crossfader_share_core_semantics() {
    let (mut e, c, m) = demo::prepare().unwrap();
    let mut co = Coordinator::new(c, m, false);
    co.dispatch(
        Action::SelectPart { deck: 1, slot: 7 }.into(),
        InputSource::Keyboard,
    )
    .unwrap();
    assert_eq!(co.focus.address(), (1, 7));
    co.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    co.dispatch(
        Action::Selected {
            action: dawj_core::PartAction::Launch,
        }
        .into(),
        InputSource::Keyboard,
    )
    .unwrap();
    e.render(&mut [0.; 512], OutputRouting::stereo_master_only());
    co.poll();
    assert!(co.snapshot.unwrap().decks[1].slots[7].active.is_some());
    for _ in 0..3 {
        co.dispatch(
            Action::CrossfaderStep { delta: 0.05 }.into(),
            InputSource::Keyboard,
        )
        .unwrap();
    }
    assert!((co.parameters.crossfader - 0.65).abs() < 0.0001);
    let before = co.focus.address();
    assert!(
        co.dispatch(
            Action::SelectPart { deck: 9, slot: 0 }.into(),
            InputSource::Gui
        )
        .is_err()
    );
    assert_eq!(co.focus.address(), before);
    co.try_act(Intent::Mode(Mode::Filter)).unwrap();
    co.try_act(Intent::HoldMode {
        token: 1,
        mode: Mode::Volume,
        pressed: true,
    })
    .unwrap();
    co.release_modes(InputSource::Gui);
    assert_eq!(co.focus.mode, Mode::Filter);
}
#[test]
fn web_timeline_observes_phrase_boundary_tempo_rules_and_cancellation() {
    let (mut e, c, m) = demo::prepare().unwrap();
    let mut co = Coordinator::new(c, m, false);
    let mut view = View::default();
    e.render(&mut [0.; 2], OutputRouting::stereo_master_only());
    co.poll();
    co.dispatch(Action::Launch { deck: 0, slot: 7 }.into(), InputSource::Gui)
        .unwrap();
    e.render(&mut [0.; 2], OutputRouting::stereo_master_only());
    co.poll();
    let projected = Projection::from_coordinator(&co, &view).unwrap();
    assert_eq!(projected.generation, co.snapshot.unwrap().generation);
    assert!(
        projected
            .markers
            .iter()
            .any(|m| m.kind == dawj_core::timeline::MarkerKind::Fill)
    );
    co.dispatch(
        Action::Part {
            deck: 0,
            slot: 7,
            action: dawj_core::PartAction::Cancel,
        }
        .into(),
        InputSource::Gui,
    )
    .unwrap();
    e.render(&mut [0.; 2], OutputRouting::stereo_master_only());
    co.poll();
    assert!(
        Projection::from_coordinator(&co, &view)
            .unwrap()
            .markers
            .is_empty()
    );
    co.dispatch(
        Action::RuleInstall {
            deck: 1,
            slot: 7,
            rule: Rule::Every {
                n: 1,
                unit: Unit::Bars,
                phase: Phase::Deck,
            },
        }
        .into(),
        InputSource::Gui,
    )
    .unwrap();
    co.dispatch(
        Action::Quantize { grid: Grid::Beat }.into(),
        InputSource::Gui,
    )
    .unwrap();
    co.try_act(Intent::Tempo(140.)).unwrap();
    e.render(&mut vec![0.; 48002], OutputRouting::stereo_master_only());
    co.poll();
    assert_eq!(co.snapshot.unwrap().bpm, Some(140.));
    view.zoom_bars = 2;
    view.page_by(&co.snapshot.unwrap(), 1);
    let p = Projection::from_coordinator(&co, &view).unwrap();
    assert_eq!(p.end - p.start, 2 * 3840);
    assert!(p.markers.iter().any(|m| m.projected));
    let mut s = co.snapshot.unwrap();
    s.beat = timebase::BeatTicks(8 * 3840);
    s.decks[0].phase = timebase::BeatTicks(0);
    co.snapshot = Some(s);
    let p = Projection::from_coordinator(&co, &View::default()).unwrap();
    assert_eq!(p.phrase_start, 8 * 3840);
    assert_eq!(p.phrase_position[0], (1, 8));
}
#[test]
fn learn_template_can_reuse_an_existing_input_but_conflicts_are_validated_after_gesture() {
    let p = Profile::default();
    let mut mapper = Mapper::new(p.clone()).unwrap();
    let template = p.mappings[0].clone();
    mapper.begin_learn(template, None).unwrap();
    let input = p.mappings[1].input;
    mapper.handle(
        midi::Event {
            control: input,
            value: 127,
        },
        None,
    );
    assert_eq!(mapper.profile.mappings.len(), p.mappings.len());
    assert!(mapper.status.contains("conflict"));
    mapper
        .begin_learn(
            Mapping::new(
                Control {
                    channel: 15,
                    kind: Kind::Note,
                    number: 127,
                },
                midi::profile::Action::Launch(Target::Selected),
            ),
            None,
        )
        .unwrap();
    mapper.handle(
        midi::Event {
            control: Control {
                channel: 15,
                kind: Kind::Note,
                number: 127,
            },
            value: 127,
        },
        None,
    );
    assert_eq!(mapper.profile.mappings.len(), p.mappings.len() + 1);
}
