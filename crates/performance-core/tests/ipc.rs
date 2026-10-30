// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{demo, live::Quantize, routing::OutputRouting};
use dawj_core::{Coordinator, InputSource, Intent, Mode, PartAction, ipc::*};
use serde_json::{Value, json};

#[test]
fn rust_and_web_examples_share_the_versioned_wire_contract() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../tauri-app/web/ipc-v1.examples.json")).unwrap();
    assert_eq!(fixture["version"], VERSION);
    let request: Request = serde_json::from_value(fixture["request"].clone()).unwrap();
    assert_eq!(request.version, VERSION);
    assert_eq!(request.id, 42);
    assert!(matches!(
        request.command,
        Command::Perform {
            action: Action::Launch { deck: 0, slot: 0 },
            source: InputSource::Gui
        }
    ));
    let reply: Reply = serde_json::from_value(fixture["reply"].clone()).unwrap();
    assert_eq!(reply.result.as_ref().unwrap_err().code, ErrorCode::Busy);
    assert_eq!(serde_json::to_value(reply).unwrap(), fixture["reply"]);
    for invalid in [
        json!({"version":1,"id":0,"command":{"kind":"perform","action":{"kind":"launch","deck":0,"slot":0,"fake":true}}}),
        json!({"version":1,"id":0,"command":{"kind":"snapshot","fake":true}}),
        json!({"version":1,"id":0,"fake":true,"command":{"kind":"snapshot"}}),
        json!({"version":1,"id":0,"command":{"kind":"perform","action":{"kind":"crossfader","value":"nan"}}}),
    ] {
        assert!(serde_json::from_value::<Request>(invalid).is_err());
    }
}

#[test]
fn ipc_actions_and_native_actions_share_all_sixteen_real_engine_slots() {
    let (mut engine, control, mixer) = demo::prepare().unwrap();
    let mut co = Coordinator::new(control, mixer, true);
    co.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    for deck in 0..2 {
        for slot in 0..8 {
            co.dispatch(Action::Launch { deck, slot }.into(), InputSource::Gui)
                .unwrap();
        }
    }
    assert!(
        Snapshot::project(&co, false)
            .decks
            .unwrap()
            .iter()
            .all(|d| d.iter().all(|s| s.active.is_none()))
    );
    engine.render(&mut [0.0; 1024], OutputRouting::stereo_master_only());
    co.poll();
    let projected = Snapshot::project(&co, false);
    let actual = co.snapshot.unwrap();
    assert_eq!(projected.generation, Some(actual.generation));
    assert_eq!(projected.frame, Some(actual.frame.0));
    assert_eq!(projected.beat, Some(actual.beat.0));
    assert_eq!(projected.bpm, actual.bpm);
    let clocks = projected.musical_decks.unwrap();
    let decks = projected.decks.unwrap();
    for deck in 0..2 {
        assert_eq!(clocks[deck].phase, actual.decks[deck].phase.0);
        assert_eq!(
            clocks[deck].phrase_length,
            actual.decks[deck].phrase_length.0
        );
        for (slot, p) in decks[deck].iter().enumerate() {
            assert!(matches!(p.state, Lifecycle::Playing));
            assert_eq!(p.active, actual.decks[deck].slots[slot].active.map(|p| p.0));
            assert_eq!(p.position, actual.decks[deck].slots[slot].position);
        }
    }
    co.dispatch(Action::Crossfader { value: 0.7 }.into(), InputSource::Gui)
        .unwrap();
    co.try_act(Intent::Cue(0)).unwrap();
    co.dispatch(Action::Cue { deck: 1 }.into(), InputSource::Gui)
        .unwrap();
    let p = Snapshot::project(&co, false);
    assert_eq!(p.crossfader, 0.7);
    assert_eq!(p.cue, [true, true]);
    co.dispatch(
        Action::StopPart { deck: 0, slot: 0 }.into(),
        InputSource::Gui,
    )
    .unwrap();
    engine.render(&mut [0.0; 512], OutputRouting::stereo_master_only());
    co.poll();
    assert!(co.snapshot.unwrap().decks[0].slots[0].active.is_none());
    assert!(co.snapshot.unwrap().decks[1].slots[0].active.is_some());
}

#[test]
fn coherent_pending_identity_is_preserved_until_real_engine_cancellation() {
    let (mut e, c, m) = demo::prepare().unwrap();
    let mut co = Coordinator::new(c, m, false);
    e.render(&mut [0.0; 2], OutputRouting::stereo_master_only());
    co.poll();
    co.dispatch(Action::Launch { deck: 0, slot: 0 }.into(), InputSource::Gui)
        .unwrap();
    e.render(&mut [0.0; 2], OutputRouting::stereo_master_only());
    co.poll();
    let s = Snapshot::project(&co, false);
    let slot = &s.decks.unwrap()[0][0];
    assert!(matches!(slot.state, Lifecycle::QueuedStart));
    assert!(slot.active.is_none());
    let pending = slot.pending.as_ref().unwrap();
    assert_eq!(pending.tick, 960);
    assert_eq!(
        pending.command_id,
        co.snapshot.unwrap().decks[0].slots[0]
            .pending
            .unwrap()
            .command_id
    );
    co.dispatch(
        Action::Part {
            deck: 0,
            slot: 0,
            action: PartAction::Cancel,
        }
        .into(),
        InputSource::Gui,
    )
    .unwrap();
    assert!(
        Snapshot::project(&co, false).decks.unwrap()[0][0]
            .pending
            .is_some()
    );
    e.render(&mut [0.0; 2], OutputRouting::stereo_master_only());
    co.poll();
    assert!(
        Snapshot::project(&co, false).decks.unwrap()[0][0]
            .pending
            .is_none()
    );
    co.close();
    let s = Snapshot::project(&co, false);
    assert!(!s.online);
    assert_eq!(s.frame, Some(co.snapshot.unwrap().frame.0));
}

#[test]
fn contextual_modes_rule_projection_and_rejections_use_the_existing_coordinator() {
    let (mut e, c, m) = demo::prepare().unwrap();
    let mut co = Coordinator::new(c, m, false);
    for action in [
        Action::Deck { deck: 1 },
        Action::Slot { slot: 3 },
        Action::Mode { mode: Mode::Filter },
        Action::HoldMode {
            token: 7,
            mode: Mode::Volume,
            pressed: true,
        },
    ] {
        co.dispatch(action.into(), InputSource::Keyboard).unwrap();
    }
    assert_eq!(co.focus.mode, Mode::Volume);
    assert!(Snapshot::project(&co, false).selection.unwrap().temporary);
    co.release_modes(InputSource::Keyboard);
    assert_eq!(co.focus.mode, Mode::Filter);
    let before = co.parameters.crossfader;
    for action in [
        Action::Crossfader { value: f32::NAN },
        Action::Crossfader { value: 2.0 },
        Action::Launch { deck: 2, slot: 0 },
        Action::Cue { deck: 0 },
    ] {
        assert!(co.dispatch(action.into(), InputSource::Gui).is_err());
    }
    assert_eq!(co.parameters.crossfader, before);
    co.dispatch(
        Action::RuleInstall {
            deck: 0,
            slot: 7,
            rule: Rule::Every {
                n: 2,
                unit: Unit::Bars,
                phase: Phase::Deck,
            },
        }
        .into(),
        InputSource::Gui,
    )
    .unwrap();
    e.render(&mut [0.0; 512], OutputRouting::stereo_master_only());
    co.poll();
    let s = Snapshot::project(&co, false);
    let rule = s.decks.unwrap()[0][7].rule.as_ref().unwrap().definition;
    assert!(matches!(
        rule,
        Rule::Every {
            n: 2,
            unit: Unit::Bars,
            phase: Phase::Deck
        }
    ));
    assert!(!Snapshot::project(&co, true).online);
}
