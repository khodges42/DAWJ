// SPDX-License-Identifier: GPL-3.0-or-later
#[path = "../examples/support/performance.rs"]
mod support;
use audio_backend::{
    live::{LiveEngine, SlotState},
    routing::OutputRouting,
};
use support::{BEAT, Instruction};
fn render(engine: &mut LiveEngine, frames: usize, block: usize, hash: &mut u64) -> (f32, f32) {
    let mut buffer = vec![0.0; block * 4];
    let route = OutputRouting::new(4, [0, 1], Some([2, 3])).unwrap();
    let mut remaining = frames;
    let mut peaks = [0.0_f32; 2];
    while remaining > 0 {
        let count = remaining.min(block);
        engine.render(&mut buffer[..count * 4], route);
        assert!(!engine.failed());
        assert_eq!(engine.dropped_events(), 0);
        for (i, sample) in buffer[..count * 4].iter().enumerate() {
            assert!(sample.is_finite() && sample.abs() <= 0.125);
            *hash = (*hash ^ u64::from(sample.to_bits())).wrapping_mul(1099511628211);
            peaks[usize::from(i % 4 >= 2)] = peaks[usize::from(i % 4 >= 2)].max(sample.abs());
        }
        remaining -= count;
    }
    (peaks[0], peaks[1])
}
#[test]
fn walkthrough_is_reproducible_and_preserves_the_shared_transport() {
    fn run(block: usize) -> u64 {
        let (mut engine, mut session) =
            support::prepare(OutputRouting::new(4, [0, 1], Some([2, 3])).unwrap()).unwrap();
        let mut hash = 1469598103934665603;
        let mut max_rule_firings = 0;
        let mut frames = 0;
        for line in include_str!("../../../docs/examples/m2-performance.txt").lines() {
            match session.command(line).unwrap() {
                Instruction::Wait(count) => {
                    let before = engine.frame().0;
                    render(&mut engine, count, block, &mut hash);
                    frames += count;
                    assert_eq!(engine.frame().0, before + count as i64);
                    max_rule_firings =
                        max_rule_firings.max(session.control.slot(1, 7).unwrap().rule_fired);
                }
                Instruction::Quit => break,
                Instruction::Continue => {}
                Instruction::CueUnavailable => panic!("logical four-channel cue should exist"),
            }
        }
        assert_eq!(frames, 36 * BEAT);
        assert_eq!(engine.frame().0, 864000);
        assert!(max_rule_firings >= 2);
        for slot in 0..5 {
            assert_eq!(
                session.control.slot(0, slot).unwrap().state,
                SlotState::Stopped
            );
        }
        for slot in [0, 2, 3, 5] {
            assert_eq!(
                session.control.slot(1, slot).unwrap().state,
                SlotState::Playing
            );
            assert_eq!(
                session.control.slot(1, slot).unwrap().position,
                24 * BEAT as u64
            );
        }
        assert!(session.status().contains("Glass Tide"));
        assert!(session.status().contains("Applied"));
        session.fadeout().unwrap();
        render(&mut engine, 4800, block, &mut hash);
        assert_eq!(engine.frame().0, 868800);
        hash
    }
    assert_eq!(run(127), run(512));
}
#[test]
fn private_cue_and_crossfade_do_not_restart_incoming_deck() {
    let (mut engine, mut session) =
        support::prepare(OutputRouting::new(4, [0, 1], Some([2, 3])).unwrap()).unwrap();
    for command in ["deck b", "quant immediate", "launch 4", "cue b"] {
        session.command(command).unwrap();
    }
    let mut hash = 0;
    let (master, cue) = render(&mut engine, BEAT, 512, &mut hash);
    assert_eq!(master, 0.0);
    assert!(cue > 0.0);
    assert_eq!(session.control.slot(1, 3).unwrap().position, BEAT as u64);
    session.command("cross 1").unwrap();
    let (master, cue) = render(&mut engine, BEAT, 512, &mut hash);
    assert!(master > 0.0 && cue > 0.0);
    assert_eq!(
        session.control.slot(1, 3).unwrap().position,
        2 * BEAT as u64
    );
    session.command("cue").unwrap();
    assert!(session.status().contains("cue=Off"));
}
#[test]
fn stereo_fallback_and_queued_state_display_are_explicit() {
    let routing = OutputRouting::stereo_master_only();
    let (mut engine, mut session) = support::prepare(routing).unwrap();
    assert_eq!(
        session.command("cue b").unwrap(),
        Instruction::CueUnavailable
    );
    assert!(session.status().contains("DisabledMasterOnly"));
    engine.render(&mut [0.0; 2], routing);
    session.command("quant bar").unwrap();
    session.command("launch 1").unwrap();
    engine.render(&mut [0.0; 2], routing);
    assert!(session.status().contains("QueuedStart"));
    assert!(session.status().contains("Stopped"));
    for command in [
        "deck c",
        "launch 9",
        "gain 1 NaN",
        "cutoff 1 30000",
        "cross 2",
        "wait 0",
        "every 0 bars",
    ] {
        assert!(session.command(command).is_err(), "{command}");
    }
}

#[test]
fn both_compositions_expose_eight_distinct_parts_and_one_shot_fills() {
    let (mut engine, mut session) =
        support::prepare(OutputRouting::new(4, [0, 1], Some([2, 3])).unwrap()).unwrap();
    session.command("quant immediate").unwrap();
    for deck in ["a", "b"] {
        session.command(&format!("deck {deck}")).unwrap();
        for slot in 1..=parts::SLOTS {
            session.command(&format!("launch {slot}")).unwrap();
        }
    }
    let mut hash = 0;
    render(&mut engine, 1, 1, &mut hash);
    for deck in 0..2 {
        for slot in 0..parts::SLOTS {
            let status = session.control.slot(deck, slot).unwrap();
            assert_eq!(
                status.part_id,
                Some(parts::PartId((deck * parts::SLOTS + slot + 1) as u64))
            );
        }
    }
    render(&mut engine, 4 * BEAT, 512, &mut hash);
    for deck in 0..2 {
        for slot in 0..parts::SLOTS {
            assert_eq!(
                session.control.slot(deck, slot).unwrap().state,
                if slot == parts::SLOTS - 1 {
                    SlotState::Stopped
                } else {
                    SlotState::Playing
                }
            );
        }
    }
}
