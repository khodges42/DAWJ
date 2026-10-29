// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{
    demo,
    live::{LiveEngine, Quantize, Request, SlotState},
    mixer::{CrossfadeCurve, crossfade_weights},
    performance_state::TransportState,
    routing::OutputRouting,
};
use dawj::{
    Coordinator, Focus, Intent, Mode, crossfader_position, lifecycle_label, musical_position,
    request_for,
};
use timebase::BeatTicks;

fn setup(cue: bool) -> (LiveEngine, Coordinator) {
    let (engine, control, mixer) = demo::prepare().unwrap();
    (engine, Coordinator::new(control, mixer, cue))
}
fn render(engine: &mut LiveEngine, ui: &mut Coordinator, frames: usize) -> f32 {
    let mut remaining = frames;
    let mut peak = 0.0_f32;
    let mut buffer = [0.0_f32; 512];
    while remaining > 0 {
        let count = remaining.min(256);
        engine.render(
            &mut buffer[..count * 2],
            OutputRouting::stereo_master_only(),
        );
        peak = buffer[..count * 2].iter().fold(peak, |a, b| a.max(b.abs()));
        remaining -= count;
    }
    ui.poll();
    assert!(!engine.failed());
    peak
}

#[test]
fn default_demo_has_audible_level_and_all_parts_remain_within_output_ceiling() {
    let (mut engine, mut ui) = setup(true);
    assert_eq!(ui.output_trim_db, 36.0);
    ui.try_act(Intent::Slot(3)).unwrap();
    ui.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    ui.try_act(Intent::Launch).unwrap();
    let mut samples = vec![0.0; 96000 * 2];
    engine.render(&mut samples, OutputRouting::stereo_master_only());
    let peak = samples.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    eprintln!(
        "Default A4 level: peak {:.2} dBFS, RMS {:.2} dBFS",
        20.0 * peak.log10(),
        20.0 * rms.log10()
    );
    assert!(peak > 0.05 && peak <= audio_backend::mixer::MASTER_CEILING);
    assert!(
        rms > 0.02,
        "bass must have sustained energy, not just nonzero output"
    );
    ui.poll();
    for d in 0..2 {
        ui.try_act(Intent::Deck(d)).unwrap();
        ui.try_act(Intent::Cue(d)).unwrap();
        for s in 0..8 {
            ui.try_act(Intent::Slot(s)).unwrap();
            ui.try_act(Intent::Launch).unwrap();
        }
    }
    let route = OutputRouting::new(4, [0, 1], Some([2, 3])).unwrap();
    engine.render(&mut samples, route);
    assert!(
        samples
            .iter()
            .all(|s| s.is_finite() && s.abs() <= audio_backend::mixer::MASTER_CEILING)
    );
    assert!(!engine.failed());
    ui.poll();
    ui.try_act(Intent::OutputTrim(12.0)).unwrap();
    assert_eq!(ui.output_trim_db, 12.0);
    assert!(ui.try_act(Intent::OutputTrim(37.0)).is_err());
    assert_eq!(ui.output_trim_db, 12.0);
}

#[test]
fn launch_pending_playing_cancel_retrigger_and_stop_are_engine_truth() {
    let (mut engine, mut ui) = setup(false);
    assert_eq!(ui.snapshot.unwrap().bpm, Some(120.0));
    assert_eq!(
        lifecycle_label(&ui.snapshot.unwrap().decks[0].slots[0]),
        "STOPPED"
    );
    render(&mut engine, &mut ui, 1); // leave the exact initial beat boundary
    ui.try_act(Intent::Launch).unwrap();
    // Submission cannot change the lifecycle, even when it has a ticket.
    assert_eq!(
        lifecycle_label(&ui.snapshot.unwrap().decks[0].slots[0]),
        "STOPPED"
    );
    render(&mut engine, &mut ui, 1);
    let queued = ui.snapshot.unwrap().decks[0].slots[0];
    assert_eq!(queued.lifecycle, SlotState::QueuedStart);
    assert!(queued.active.is_none());
    assert_eq!(queued.pending.unwrap().target, BeatTicks(960));
    assert_eq!(lifecycle_label(&queued), "QUEUED START");
    ui.try_act(Intent::Cancel).unwrap();
    render(&mut engine, &mut ui, 1);
    assert_eq!(
        ui.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Stopped
    );
    ui.try_act(Intent::Launch).unwrap();
    let peak = render(&mut engine, &mut ui, 24100);
    assert!(peak > 0.0);
    let playing = ui.snapshot.unwrap().decks[0].slots[0];
    assert!(playing.active.is_some());
    assert_eq!(lifecycle_label(&playing), "PLAYING");
    ui.try_act(Intent::Quantize(Quantize::Bar)).unwrap();
    ui.try_act(Intent::Retrigger).unwrap();
    render(&mut engine, &mut ui, 1);
    assert_eq!(
        lifecycle_label(&ui.snapshot.unwrap().decks[0].slots[0]),
        "PLAYING / RETRIGGER"
    );
    ui.try_act(Intent::Cancel).unwrap();
    render(&mut engine, &mut ui, 1);
    ui.try_act(Intent::Stop).unwrap();
    render(&mut engine, &mut ui, 1);
    assert_eq!(
        lifecycle_label(&ui.snapshot.unwrap().decks[0].slots[0]),
        "PLAYING / QUEUED STOP"
    );
    render(&mut engine, &mut ui, 72000);
    assert_eq!(
        ui.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Stopped
    );
    assert_eq!(ui.snapshot.unwrap().transport, TransportState::Rendering);
}

#[test]
fn selection_modes_command_generation_and_shaping_preserve_identity() {
    let (mut engine, mut ui) = setup(true);
    let initial = ui.snapshot.unwrap();
    ui.try_act(Intent::Slot(3)).unwrap();
    ui.try_act(Intent::Deck(1)).unwrap();
    ui.try_act(Intent::Slot(7)).unwrap();
    ui.try_act(Intent::Mode(Mode::Filter)).unwrap();
    assert_eq!(ui.focus.address(), (1, 7));
    assert_eq!(ui.snapshot.unwrap().generation, initial.generation);
    assert!(matches!(
        request_for(Intent::Launch, &ui.focus, &initial).unwrap().0,
        Request::Launch(15)
    ));
    ui.try_act(Intent::Deck(0)).unwrap();
    assert_eq!(ui.focus.address(), (0, 3));
    assert_eq!(ui.focus.mode, Mode::Filter);
    ui.try_act(Intent::Cutoff(Some(500.0))).unwrap();
    // Don't overwrite the last accepted target with unacknowledged desired values.
    assert!(
        ui.snapshot.unwrap().decks[0].slots[3]
            .parameters
            .cutoff_hz
            .is_none()
    );
    render(&mut engine, &mut ui, 128);
    ui.try_act(Intent::Gain(0.4)).unwrap();
    render(&mut engine, &mut ui, 128);
    let p = ui.snapshot.unwrap().decks[0].slots[3].parameters;
    assert!((p.gain - 0.4).abs() < 0.0001);
    assert_eq!(p.cutoff_hz, Some(500.0));
    ui.try_act(Intent::Mode(Mode::Volume)).unwrap();
    assert_eq!(ui.snapshot.unwrap().decks[0].slots[3].parameters, p);
    assert!(!ui.focus.update(Intent::Deck(2)));
    assert!(!ui.focus.update(Intent::Slot(8)));
}

#[test]
fn crossfader_mapping_cue_limitations_and_invalid_targets() {
    for x in [0.0, 0.5, 1.0] {
        assert_eq!(crossfader_position(x).unwrap(), x);
    }
    for x in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(crossfader_position(x).is_err());
    }
    assert_eq!(
        crossfade_weights(
            crossfader_position(0.0).unwrap(),
            CrossfadeCurve::EqualPower
        )
        .unwrap(),
        [1.0, 0.0]
    );
    assert_eq!(
        crossfade_weights(
            crossfader_position(1.0).unwrap(),
            CrossfadeCurve::EqualPower
        )
        .unwrap(),
        [0.0, 1.0]
    );
    let (_engine, mut ui) = setup(false);
    assert!(ui.try_act(Intent::Cue(0)).is_err());
    assert_eq!(ui.cues, [false; 2]);
    ui.try_act(Intent::Crossfader(1.0)).unwrap();
    assert_eq!(ui.parameters.crossfader, 1.0);
    assert!(ui.try_act(Intent::DeckGain(0, 2.0)).is_err());
    assert_eq!(ui.parameters.gain_a, 0.5);
}

#[test]
fn rejected_commands_queue_exhaustion_and_shutdown_keep_last_actual_state() {
    let (mut engine, mut ui) = setup(false);
    ui.act(Intent::Retrigger); // stopped source: admitted, then rejected by scheduler
    render(&mut engine, &mut ui, 128);
    assert!(ui.notice.contains("Rejected"));
    assert_eq!(
        ui.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Stopped
    );
    assert!(ui.try_act(Intent::Cancel).is_err());
    ui.try_act(Intent::Cutoff(Some(20001.0))).unwrap_err();
    for _ in 0..32 {
        ui.try_act(Intent::Launch).unwrap();
    }
    assert!(ui.try_act(Intent::Launch).is_err());
    // No successful-looking lifecycle update while the consumer hasn't run.
    assert_eq!(
        ui.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Stopped
    );
    drop(engine);
    ui.poll();
    assert!(!ui.available());
    assert_eq!(ui.snapshot.unwrap().transport, TransportState::Closed);
    assert!(ui.try_act(Intent::Launch).is_err());
    assert!(ui.feedback[0][0].contains("Closed"));
    ui.close();
    ui.close();
    assert!(!ui.available());
}

#[test]
fn full_sixteen_slot_demo_and_tempo_boundary_are_observed() {
    let (mut engine, mut ui) = setup(true);
    ui.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    for d in 0..2 {
        ui.try_act(Intent::Deck(d)).unwrap();
        for s in 0..8 {
            ui.try_act(Intent::Slot(s)).unwrap();
            ui.try_act(Intent::Launch).unwrap();
        }
    }
    assert!(render(&mut engine, &mut ui, 256) > 0.0);
    for deck in ui.snapshot.unwrap().decks {
        for p in deck.slots {
            assert_eq!(p.lifecycle, SlotState::Playing);
        }
    }
    ui.try_act(Intent::Cue(0)).unwrap();
    ui.try_act(Intent::Cue(1)).unwrap();
    assert_eq!(ui.cues, [true, true]);
    ui.try_act(Intent::Tempo(130.0)).unwrap();
    render(&mut engine, &mut ui, 128);
    assert_eq!(ui.snapshot.unwrap().bpm, Some(120.0)); // admitted future intent != current tempo
    render(&mut engine, &mut ui, 24000);
    assert_eq!(ui.snapshot.unwrap().bpm, Some(130.0));
    assert_eq!(musical_position(BeatTicks(3840)), "2.1");
    // Both deck clocks keep advancing after one deck's independent part stop.
    ui.try_act(Intent::Stop).unwrap();
    render(&mut engine, &mut ui, 128);
    assert_eq!(
        ui.snapshot.unwrap().decks[1].slots[7].lifecycle,
        SlotState::Stopped
    );
    assert_eq!(
        ui.snapshot.unwrap().decks[0].slots[0].lifecycle,
        SlotState::Playing
    );
    assert_eq!(Focus::default().address(), (0, 0));
}
