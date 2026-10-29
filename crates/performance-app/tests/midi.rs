// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{
    demo,
    live::{LiveEngine, Quantize, SlotState},
    routing::OutputRouting,
};
use dawj::{
    Coordinator, InputSource, Intent, Mode, PartAction,
    midi::{self, Event, Learn, Mapper, profile::*},
};
use std::sync::atomic::{AtomicU64, Ordering};
fn setup() -> (LiveEngine, Coordinator, Mapper) {
    let (e, c, m) = demo::prepare().unwrap();
    (
        e,
        Coordinator::new(c, m, true),
        Mapper::new(Profile::default()).unwrap(),
    )
}
fn render(e: &mut LiveEngine, c: &mut Coordinator, frames: usize) {
    let mut buffer = [0.0; 512];
    let mut left = frames;
    while left > 0 {
        let n = left.min(256);
        e.render(&mut buffer[..n * 2], OutputRouting::stereo_master_only());
        left -= n;
    }
    c.poll();
    assert!(!e.failed());
}
fn send(m: &mut Mapper, c: &mut Coordinator, status: u8, number: u8, value: u8) {
    m.handle(midi::normalize(&[status, number, value]).unwrap(), Some(c));
}
fn cc(m: &mut Mapper, c: &mut Coordinator, n: u8, v: u8) {
    send(m, c, 0xB0, n, v);
}

#[test]
fn normalization_is_channel_aware_and_rejects_invalid_or_unsupported_messages() {
    assert_eq!(
        midi::normalize(&[0x9F, 60, 0]),
        Some(Event {
            control: Control {
                channel: 15,
                kind: Kind::Note,
                number: 60
            },
            value: 0
        })
    );
    assert_eq!(midi::normalize(&[0x82, 60, 127]).unwrap().value, 0);
    assert_eq!(midi::normalize(&[0xB2, 1, 127]).unwrap().control.channel, 2);
    for bytes in [
        vec![],
        vec![0x90, 60],
        vec![0x90, 60, 128],
        vec![0xB0, 128, 0],
        vec![0xF8],
        vec![0xE0, 0, 64],
        vec![0xF0, 0, 0],
        vec![0x40, 0, 0],
        vec![0x90, 1, 2, 3],
    ] {
        assert!(midi::normalize(&bytes).is_none(), "{bytes:?}");
    }
}

#[test]
fn real_callback_bridge_is_bounded_tagged_and_never_owns_the_engine() {
    use dawj::midi::device::{EVENT_CAPACITY, input_bridge, unique_port};
    let (bridge, rx) = input_bridge(7);
    let producer = std::thread::spawn(move || {
        for _ in 0..EVENT_CAPACITY + 3 {
            bridge.receive(&[0x90, 36, 127]);
        }
        bridge.receive(&[0xF8]); // ignored timing does not count as a dropped action
        bridge.dropped.load(Ordering::Relaxed)
    });
    assert_eq!(producer.join().unwrap(), 3);
    let packets: Vec<_> = rx.try_iter().collect();
    assert_eq!(packets.len(), EVENT_CAPACITY);
    assert!(packets.iter().all(|p| p.epoch == 7));
    let names = vec!["Generic".into(), "Other".into()];
    assert_eq!(unique_port(&names, "Generic").unwrap(), Some(0));
    assert_eq!(unique_port(&names, "Missing").unwrap(), None);
    assert!(unique_port(&["Generic".into(), "Generic".into()], "Generic").is_err());
}

#[test]
fn pickup_rearms_after_gui_edits_and_continuous_admission_is_fair_to_both_decks() {
    let (mut e, mut c, mut m) = setup();
    cc(&mut m, &mut c, 16, 127);
    cc(&mut m, &mut c, 16, 50);
    c.flush_parameters();
    render(&mut e, &mut c, 128);
    c.try_act(Intent::Gain(0.9)).unwrap();
    render(&mut e, &mut c, 128);
    cc(&mut m, &mut c, 16, 51);
    c.flush_parameters();
    render(&mut e, &mut c, 128);
    assert!((c.snapshot.unwrap().decks[0].slots[0].parameters.gain - 0.9).abs() < 0.001);
    assert!(m.status.contains("Pickup"));
    for d in 0..2 {
        for s in 0..8 {
            c.dispatch(
                Intent::Part {
                    deck: d,
                    slot: s,
                    action: PartAction::Gain(0.2),
                },
                InputSource::Midi,
            )
            .unwrap();
        }
    }
    for _ in 0..4 {
        // Keep the first four hot; later slots still have to get their turn.
        for s in 0..4 {
            c.dispatch(
                Intent::Part {
                    deck: 0,
                    slot: s,
                    action: PartAction::Gain(0.2),
                },
                InputSource::Midi,
            )
            .unwrap();
        }
        c.flush_parameters();
        render(&mut e, &mut c, 128);
    }
    for deck in c.snapshot.unwrap().decks {
        for p in deck.slots {
            assert!((p.parameters.gain - 0.2).abs() < 0.001);
        }
    }
}
#[test]
fn profile_validation_conflicts_ranges_and_learn_are_transactional() {
    let mut m = Mapper::new(Profile::default()).unwrap();
    let before = m.profile.mappings.len();
    let draft = Mapping::new(
        Control {
            channel: 1,
            kind: Kind::Note,
            number: 70,
        },
        Action::Launch(Target::Selected),
    );
    m.learn = Some(Learn {
        mapping: draft.clone(),
        replace: None,
    });
    m.handle(midi::normalize(&[0x80, 72, 127]).unwrap(), None);
    assert!(m.learn.is_some());
    m.handle(midi::normalize(&[0x90, 36, 127]).unwrap(), None);
    assert!(m.status.contains("conflict"));
    assert_eq!(m.profile.mappings.len(), before);
    m.handle(midi::normalize(&[0x91, 36, 127]).unwrap(), None);
    assert!(m.learn.is_none());
    assert_eq!(m.profile.mappings.len(), before + 1);
    assert!(m.dirty);
    let (mut engine, mut co, _) = setup();
    send(&mut m, &mut co, 0x91, 36, 127); // held/repeated learn press must not perform
    render(&mut engine, &mut co, 128);
    assert_eq!(
        co.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Stopped
    );
    send(&mut m, &mut co, 0x81, 36, 0);
    send(&mut m, &mut co, 0x91, 36, 127);
    render(&mut engine, &mut co, 24000);
    assert_eq!(
        co.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Playing
    );
    let mut invalid = draft.clone();
    invalid.max = 200;
    assert!(m.install(invalid, None, None).is_err());
    let mut p = m.profile.clone();
    p.mappings[0].action = Action::Launch(Target::DeckSlot { deck: 2, slot: 0 });
    assert!(p.validate().is_err());
    let mut p = m.profile.clone();
    p.leds[0].output.channel = 16;
    assert!(p.validate().is_err());
    let mut p = m.profile.clone();
    p.mappings[0].step = f32::NAN;
    assert!(p.validate().is_err());
}
#[test]
fn all_sixteen_note_mappings_launch_real_parts_without_changing_focus() {
    let (mut e, mut c, mut m) = setup();
    c.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    for n in 36..52 {
        send(&mut m, &mut c, 0x90, n, 127);
        send(&mut m, &mut c, 0x90, n, 127);
    } // repeats do not toggle twice
    assert_eq!(c.focus.address(), (0, 0));
    assert_eq!(
        c.snapshot.unwrap().decks[1].slots[7].lifecycle,
        SlotState::Stopped
    );
    render(&mut e, &mut c, 128);
    for d in 0..2 {
        for s in 0..8 {
            let p = c.snapshot.unwrap().decks[d].slots[s];
            assert_eq!(p.lifecycle, SlotState::Playing);
            assert_eq!(p.active.unwrap().0, (d * 8 + s + 1) as u64);
        }
    }
    for n in 36..52 {
        send(&mut m, &mut c, 0x80, n, 0);
        send(&mut m, &mut c, 0x90, n, 127);
    }
    render(&mut e, &mut c, 128);
    assert!(
        c.snapshot
            .unwrap()
            .decks
            .iter()
            .all(|d| d.slots.iter().all(|p| p.lifecycle == SlotState::Stopped))
    );
}
#[test]
fn absolute_pickup_modes_relative_motion_and_coalescing_preserve_parameters() {
    let (mut e, mut c, mut m) = setup();
    cc(&mut m, &mut c, 16, 0);
    c.flush_parameters();
    render(&mut e, &mut c, 128);
    assert_eq!(c.snapshot.unwrap().decks[0].slots[0].parameters.gain, 1.0);
    assert!(m.status.contains("Pickup"));
    cc(&mut m, &mut c, 16, 127);
    cc(&mut m, &mut c, 16, 64);
    c.flush_parameters();
    render(&mut e, &mut c, 128);
    assert!((c.snapshot.unwrap().decks[0].slots[0].parameters.gain - 64.0 / 127.0).abs() < 0.001);
    c.try_act(Intent::Mode(Mode::Filter)).unwrap();
    let before = c.snapshot.unwrap();
    cc(&mut m, &mut c, 16, 30);
    c.flush_parameters();
    render(&mut e, &mut c, 128);
    assert_eq!(
        c.snapshot.unwrap().decks[0].slots[0].parameters,
        before.decks[0].slots[0].parameters
    );
    cc(&mut m, &mut c, 16, 127);
    cc(&mut m, &mut c, 16, 64);
    c.flush_parameters();
    render(&mut e, &mut c, 128);
    assert!(
        c.snapshot.unwrap().decks[0].slots[0]
            .parameters
            .cutoff_hz
            .is_some()
    );
    // Persistent filter + temporary volume: no parameter changes on mode alone.
    send(&mut m, &mut c, 0x90, 82, 127);
    assert_eq!(c.focus.mode, Mode::Volume);
    let params = c.snapshot.unwrap().decks[0].slots[0].parameters;
    send(&mut m, &mut c, 0x90, 82, 127);
    send(&mut m, &mut c, 0x90, 82, 0);
    assert_eq!(c.focus.mode, Mode::Filter);
    assert_eq!(c.snapshot.unwrap().decks[0].slots[0].parameters, params);
    let relative = Mapping {
        behavior: Behavior::RelativeTwosComplement,
        ..Mapping::new(
            Control {
                channel: 0,
                kind: Kind::CC,
                number: 60,
            },
            Action::Gain(Target::Selected),
        )
    };
    m.install(relative, None, Some(&mut c)).unwrap();
    for _ in 0..1000 {
        cc(&mut m, &mut c, 60, 1);
    }
    assert_eq!(c.parameter_target(0, 0).unwrap().gain, 1.0);
    c.flush_parameters();
    render(&mut e, &mut c, 128);
    assert_eq!(c.snapshot.unwrap().decks[0].slots[0].parameters.gain, 1.0);
    cc(&mut m, &mut c, 60, 127);
    c.flush_parameters();
    render(&mut e, &mut c, 128);
    assert!(c.snapshot.unwrap().decks[0].slots[0].parameters.gain < 1.0);
}
#[test]
fn momentary_sources_stack_release_and_disconnect_without_stuck_modes() {
    let (_e, mut c, mut m) = setup();
    c.try_act(Intent::Mode(Mode::Filter)).unwrap();
    send(&mut m, &mut c, 0x90, 82, 127);
    assert!(c.temporary_mode());
    assert_eq!(c.focus.mode, Mode::Volume);
    c.dispatch(
        Intent::HoldMode {
            token: 1,
            mode: Mode::Filter,
            pressed: true,
        },
        InputSource::Keyboard,
    )
    .unwrap();
    c.release_modes(InputSource::Keyboard);
    assert_eq!(c.focus.mode, Mode::Volume);
    m.reset(Some(&mut c));
    assert_eq!(c.focus.mode, Mode::Filter);
    assert!(!c.temporary_mode());
    // Old release after disconnect cannot change persistent mode.
    send(&mut m, &mut c, 0x80, 82, 0);
    assert_eq!(c.focus.mode, Mode::Filter);
}
#[test]
fn feedback_tracks_queued_playing_stop_and_closed_snapshots_not_input_receipts() {
    let (mut e, mut c, mut m) = setup();
    for (number, action) in [
        (60, Action::Retrigger(Target::DeckSlot { deck: 0, slot: 0 })),
        (61, Action::Cancel(Target::DeckSlot { deck: 0, slot: 0 })),
    ] {
        m.install(
            Mapping::new(
                Control {
                    channel: 0,
                    kind: Kind::Note,
                    number,
                },
                action,
            ),
            None,
            Some(&mut c),
        )
        .unwrap();
    }
    render(&mut e, &mut c, 1);
    let initial = m.feedback(Some(&c));
    assert_eq!(initial.len(), 16);
    assert!(initial.iter().all(|(_, v)| v[2] == 0));
    m.mark_sent(&initial);
    assert!(m.feedback(Some(&c)).is_empty());
    send(&mut m, &mut c, 0x90, 36, 127);
    assert!(m.feedback(Some(&c)).is_empty()); // receipt alone is not playing
    render(&mut e, &mut c, 1);
    let queued = m.feedback(Some(&c));
    assert_eq!(queued, vec![(0, [0x90, 36, 16])]);
    m.mark_sent(&queued);
    render(&mut e, &mut c, 24000);
    let playing = m.feedback(Some(&c));
    assert_eq!(playing, vec![(0, [0x90, 36, 127])]);
    m.mark_sent(&playing);
    c.try_act(Intent::Quantize(Quantize::Phrase)).unwrap();
    send(&mut m, &mut c, 0x90, 60, 127);
    assert!(m.feedback(Some(&c)).is_empty());
    render(&mut e, &mut c, 1);
    let retrigger = m.feedback(Some(&c));
    assert_eq!(retrigger, vec![(0, [0x90, 36, 32])]);
    m.mark_sent(&retrigger);
    send(&mut m, &mut c, 0x90, 61, 127);
    render(&mut e, &mut c, 1);
    let cancelled = m.feedback(Some(&c));
    assert_eq!(cancelled, vec![(0, [0x90, 36, 127])]);
    m.mark_sent(&cancelled);
    send(&mut m, &mut c, 0x80, 36, 0);
    send(&mut m, &mut c, 0x90, 36, 127);
    render(&mut e, &mut c, 1);
    let stopping = m.feedback(Some(&c));
    assert_eq!(stopping, vec![(0, [0x90, 36, 64])]);
    // If sending is unavailable, not marking retains the change for retry.
    assert_eq!(m.feedback(Some(&c)), stopping);
    m.mark_sent(&stopping);
    drop(e);
    c.poll();
    assert_eq!(m.feedback(Some(&c)), vec![(0, [0x90, 36, 0])]);
    m.refresh_feedback();
    assert_eq!(m.feedback(Some(&c)).len(), 16);
}
#[test]
fn mapping_profiles_survive_restart_and_reject_oversized_or_unknown_data() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "dawj-midi-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let path = dir.join("midi.json");
    let mut p = Profile {
        input: Some("Generic input".into()),
        ..Profile::default()
    };
    p.save(&path).unwrap();
    p.mappings[0].input.channel = 2;
    p.save(&path).unwrap();
    let loaded = Profile::load(&path).unwrap();
    assert_eq!(
        serde_json::to_value(&p).unwrap(),
        serde_json::to_value(&loaded).unwrap()
    );
    std::fs::write(&path, vec![b' '; MAX_PROFILE_BYTES + 1]).unwrap();
    assert!(Profile::load(&path).is_err());
    let mut json = serde_json::to_value(&p).unwrap();
    json["unknown"] = serde_json::Value::Bool(true);
    std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(Profile::load(&path).is_err());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
#[test]
fn continuous_flood_leaves_room_for_stop_and_invalid_events_cannot_panic() {
    let (mut e, mut c, mut m) = setup();
    c.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    c.try_act(Intent::Launch).unwrap();
    render(&mut e, &mut c, 128);
    for _ in 0..2000 {
        for n in 16..32 {
            cc(&mut m, &mut c, n, 127);
        }
    }
    c.flush_parameters();
    c.dispatch(
        Intent::Part {
            deck: 0,
            slot: 0,
            action: PartAction::Stop,
        },
        InputSource::Keyboard,
    )
    .unwrap();
    render(&mut e, &mut c, 128);
    assert_eq!(
        c.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Stopped
    );
    m.handle(
        Event {
            control: Control {
                channel: 16,
                kind: Kind::CC,
                number: 0,
            },
            value: 255,
        },
        Some(&mut c),
    );
    assert!(m.status.contains("Invalid"));
    drop(e);
    c.poll();
    cc(&mut m, &mut c, 1, 64);
    assert!(m.status.contains("rejected"));
}
