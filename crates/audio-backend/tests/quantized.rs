// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{
    live::{LiveControl, LiveEngine, Outcome, Quantize, Request},
    mixer::CueSelection,
    routing::OutputRouting,
    samples::{BoundaryPolicy, LoadedSample, SampleVoice},
};
use event_engine::live::{Event, MusicalScheduler};
use parts::{
    Applied, AssetId, Part, PartId, PlaybackConfiguration, PlaybackMode, State, TimingPolicy,
};
use timebase::{BeatTicks, TempoChange, TempoMap};

fn voice(id: u64, rate: u32, oneshot: bool) -> SampleVoice {
    let data = if oneshot {
        vec![8192_i16; 3]
    } else {
        vec![8192_i16, 16384, -8192, -16384]
    };
    let size = (data.len() * 2) as u32;
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend((36 + size).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16_u32.to_le_bytes());
    wav.extend(1_u16.to_le_bytes());
    wav.extend(1_u16.to_le_bytes());
    wav.extend(rate.to_le_bytes());
    wav.extend((rate * 2).to_le_bytes());
    wav.extend(2_u16.to_le_bytes());
    wav.extend(16_u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(size.to_le_bytes());
    for v in data {
        wav.extend(v.to_le_bytes());
    }
    let part = Part {
        id: PartId(id),
        playback: PlaybackConfiguration {
            asset: AssetId(id),
            mode: if oneshot {
                PlaybackMode::OneShot
            } else {
                PlaybackMode::Loop { start: 0, end: 4 }
            },
        },
        timing: TimingPolicy {
            nominal_length: BeatTicks(960),
            quantum: BeatTicks(960),
        },
    };
    SampleVoice::prepare(
        part,
        LoadedSample::decode(AssetId(id), &wav, rate).unwrap(),
        rate,
        BoundaryPolicy::Preserve,
    )
    .unwrap()
}
fn engine(rate: u32) -> (LiveEngine, LiveControl) {
    engine_phrase(rate, [BeatTicks(7680), BeatTicks(3840)])
}
fn engine_phrase(rate: u32, phrases: [BeatTicks; 2]) -> (LiveEngine, LiveControl) {
    let map = TempoMap::new(
        rate,
        &[TempoChange {
            at: BeatTicks(0),
            bpm: 120.0,
        }],
    )
    .unwrap();
    let scheduler = MusicalScheduler::new(map, [BeatTicks(0), BeatTicks(-960)], phrases).unwrap();
    let (engine, control, mixer) = LiveEngine::new(
        scheduler,
        vec![
            voice(1, rate, false),
            voice(2, rate, false),
            voice(3, rate, true),
        ],
    )
    .unwrap();
    mixer.set_cue(CueSelection::B);
    (engine, control)
}
fn render(engine: &mut LiveEngine, frames: usize, block: usize) -> (Vec<f32>, Vec<Event>) {
    let mut output = vec![0.0; frames * 4];
    let mut events = Vec::new();
    let route = OutputRouting::new(4, [0, 1], Some([2, 3])).unwrap();
    for chunk in output.chunks_mut(block * 4) {
        engine.render(chunk, route);
        events.extend(engine.events());
        assert!(!engine.failed());
    }
    (output, events)
}
#[test]
fn immediate_and_arrival_either_side_of_beat() {
    for (arrival, expected) in [(3999, 4000), (4000, 4000), (4001, 8000)] {
        let (mut e, mut c) = engine(8000);
        render(&mut e, arrival, 8192);
        let ticket = c.submit(0, 0, Request::Launch(0), Quantize::Beat).unwrap();
        let (_, events) = render(&mut e, expected - arrival + 1, 16384);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].frame.0, expected as i64);
        assert_eq!(ticket.outcome(), Outcome::Applied);
    }
    let (mut e, mut c) = engine(8000);
    render(&mut e, 3999, 8192);
    c.submit(0, 0, Request::Launch(0), Quantize::Immediate)
        .unwrap();
    let (_, events) = render(&mut e, 2, 2);
    assert_eq!(events[0].frame.0, 3999);
    assert_eq!(e.source_position(0, 0), Some(2));
}
#[test]
fn concurrent_decks_multiple_events_and_partition_matrix() {
    fn run(rate: u32, block: usize) -> (Vec<f32>, Vec<(usize, usize, i64)>) {
        let (mut e, mut c) = engine(rate);
        render(&mut e, 1, 1);
        c.submit(0, 0, Request::Launch(0), Quantize::Beat).unwrap();
        c.submit(0, 1, Request::Launch(1), Quantize::Bar).unwrap();
        c.submit(1, 0, Request::Launch(0), Quantize::Phrase)
            .unwrap();
        c.submit(1, 1, Request::Launch(1), Quantize::Beat).unwrap();
        let (audio, events) = render(&mut e, rate as usize * 3, block);
        let trace: Vec<_> = events.iter().map(|v| (v.deck, v.slot, v.frame.0)).collect();
        assert_eq!(
            trace,
            vec![
                (0, 0, i64::from(rate) / 2),
                (1, 1, i64::from(rate) / 2),
                (1, 0, i64::from(rate) * 3 / 2),
                (0, 1, i64::from(rate) * 2)
            ]
        );
        assert!(e.scheduler().phase(0).is_ok());
        assert!(e.scheduler().part_phase(0, 0).is_some());
        (audio, trace)
    }
    for rate in [44100, 48000, 96000] {
        let reference = run(rate, 1);
        for block in [31, 128, 511, 8192, 16384] {
            assert_eq!(run(rate, block), reference);
        }
    }
}
#[test]
fn tempo_edit_keeps_pending_beats_and_fractional_anchor() {
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    c.submit(0, 0, Request::Launch(0), Quantize::Bar).unwrap();
    let edit = c
        .submit(0, 0, Request::Tempo(60.0), Quantize::Beat)
        .unwrap();
    let (_, events) = render(&mut e, 28000, 16384);
    assert_eq!(edit.outcome(), Outcome::Applied);
    assert_eq!(events[0].frame.0, 28000);
    let mut map = TempoMap::new(
        44100,
        &[TempoChange {
            at: BeatTicks(0),
            bpm: 140.0,
        }],
    )
    .unwrap();
    map.reserve_changes(64);
    map.edit_future(
        timebase::SampleFrame(0),
        TempoChange {
            at: BeatTicks(960),
            bpm: 120.0,
        },
    )
    .unwrap();
    assert_eq!(
        map.frame_at(BeatTicks(1920)).unwrap().0,
        (44100.0_f64 * 3.0 / 7.0 + 22050.0).round() as i64
    );
}
#[test]
fn cancellation_precedence_conflicts_and_retrigger() {
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    let a = c.submit(0, 0, Request::Launch(0), Quantize::Beat).unwrap();
    let cancel = c
        .submit(0, 0, Request::Cancel(a.id()), Quantize::Immediate)
        .unwrap();
    let b = c.submit(0, 0, Request::Launch(1), Quantize::Beat).unwrap();
    let stale = c
        .submit(0, 0, Request::Cancel(a.id()), Quantize::Immediate)
        .unwrap();
    let (_, events) = render(&mut e, 4000, 8192);
    assert_eq!(events.len(), 1);
    assert_eq!(a.outcome(), Outcome::Cancelled);
    assert_eq!(cancel.outcome(), Outcome::Applied);
    assert_eq!(stale.outcome(), Outcome::Rejected);
    assert_eq!(b.outcome(), Outcome::Applied);
    let conflict = c
        .submit(0, 0, Request::Launch(0), Quantize::Immediate)
        .unwrap();
    let retrigger = c
        .submit(0, 0, Request::Retrigger, Quantize::Immediate)
        .unwrap();
    let (_, events) = render(&mut e, 1, 1);
    assert!(matches!(
        events[0].action,
        Applied::Start {
            retrigger: true,
            ..
        }
    ));
    assert_eq!(e.source_position(0, 0), Some(1));
    assert_eq!(conflict.outcome(), Outcome::Rejected);
    assert_eq!(retrigger.outcome(), Outcome::Applied);
    c.submit(0, 0, Request::Replace(0), Quantize::Immediate)
        .unwrap();
    let (_, events) = render(&mut e, 1, 1);
    assert!(matches!(
        events[0].action,
        Applied::Start {
            retrigger: false,
            ..
        }
    ));
    c.submit(0, 0, Request::Stop, Quantize::Immediate).unwrap();
    render(&mut e, 1, 1);
    assert_eq!(e.source_position(0, 0), None);
    assert_eq!(e.scheduler().state(0, 0), Some(State::Stopped));
}
#[test]
fn queue_overflow_expiry_empty_callback_and_eof() {
    let (mut e, mut c) = engine(8000);
    let first = c
        .submit(0, 0, Request::Launch(2), Quantize::Immediate)
        .unwrap();
    for _ in 1..32 {
        c.submit(0, 1, Request::Stop, Quantize::Immediate).unwrap();
    }
    assert!(c.submit(0, 2, Request::Stop, Quantize::Immediate).is_err());
    e.render(&mut [], OutputRouting::stereo_master_only());
    assert_eq!(first.outcome(), Outcome::Pending);
    render(&mut e, 4, 128);
    assert_eq!(e.scheduler().state(0, 0), Some(State::Stopped));
    assert_eq!(first.outcome(), Outcome::Applied);
    c.submit(0, 1, Request::Stop, Quantize::Immediate).unwrap();
    assert_eq!(first.outcome(), Outcome::Expired);
}

#[test]
fn real_producer_thread_preserves_fifo() {
    let (mut e, mut c) = engine(8000);
    let producer = std::thread::spawn(move || {
        let a = c
            .submit(0, 0, Request::Launch(0), Quantize::Immediate)
            .unwrap();
        let b = c.submit(0, 0, Request::Stop, Quantize::Immediate).unwrap();
        (a, b)
    });
    let (a, b) = producer.join().unwrap();
    render(&mut e, 1, 1);
    assert_eq!(a.outcome(), Outcome::Superseded);
    assert_eq!(b.outcome(), Outcome::Applied);
    assert_eq!(e.source_position(0, 0), None);
}

#[test]
fn immediate_does_not_pull_another_slots_quantized_action_forward() {
    let (mut e, mut c) = engine(8000);
    render(&mut e, 3999, 8192);
    c.submit(0, 0, Request::Launch(0), Quantize::Immediate)
        .unwrap();
    c.submit(0, 1, Request::Launch(1), Quantize::Beat).unwrap();
    let (_, events) = render(&mut e, 2, 2);
    assert_eq!(
        events
            .iter()
            .map(|v| (v.slot, v.frame.0))
            .collect::<Vec<_>>(),
        vec![(0, 3999), (1, 4000)]
    );
}

#[test]
fn cancellation_at_boundary_precedes_dispatch_and_after_boundary_is_stale() {
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    let launch = c.submit(0, 0, Request::Launch(0), Quantize::Beat).unwrap();
    render(&mut e, 3999, 8192);
    c.submit(0, 0, Request::Cancel(launch.id()), Quantize::Immediate)
        .unwrap();
    let (_, events) = render(&mut e, 1, 1);
    assert!(events.is_empty());
    assert_eq!(launch.outcome(), Outcome::Cancelled);
    let launch = c
        .submit(0, 0, Request::Launch(0), Quantize::Immediate)
        .unwrap();
    render(&mut e, 1, 1);
    let cancel = c
        .submit(0, 0, Request::Cancel(launch.id()), Quantize::Immediate)
        .unwrap();
    render(&mut e, 1, 1);
    assert_eq!(cancel.outcome(), Outcome::Rejected);
    assert!(e.source_position(0, 0).is_some());
}

#[test]
fn spsc_under_simultaneous_producer_and_renderer_activity() {
    let (mut e, mut c) = engine(8000);
    let producer = std::thread::spawn(move || {
        let mut last = None;
        for _ in 0..1000 {
            loop {
                match c.submit(0, 0, Request::Stop, Quantize::Immediate) {
                    Ok(ticket) => {
                        last = Some(ticket);
                        break;
                    }
                    Err("command queue full") => std::thread::yield_now(),
                    Err(error) => panic!("{error}"),
                }
            }
        }
        last.unwrap()
    });
    while !producer.is_finished() {
        render(&mut e, 64, 64);
        std::thread::yield_now();
    }
    let last = producer.join().unwrap();
    render(&mut e, 64, 64);
    assert_eq!(last.id(), 1000);
    assert_eq!(last.outcome(), Outcome::Applied);
}

#[test]
fn tempo_edits_reject_past_invalid_and_exhausted_storage_transactionally() {
    let mut map = TempoMap::new(
        8000,
        &[TempoChange {
            at: BeatTicks(0),
            bpm: 120.0,
        }],
    )
    .unwrap();
    map.reserve_changes(64);
    let original = map.frame_at(BeatTicks(1920)).unwrap();
    assert!(
        map.edit_future(
            timebase::SampleFrame(1),
            TempoChange {
                at: BeatTicks(0),
                bpm: 60.0
            }
        )
        .is_err()
    );
    assert!(
        map.edit_future(
            timebase::SampleFrame(0),
            TempoChange {
                at: BeatTicks(960),
                bpm: f64::NAN
            }
        )
        .is_err()
    );
    assert_eq!(map.frame_at(BeatTicks(1920)).unwrap(), original);
    for beat in 1..64 {
        map.edit_future(
            timebase::SampleFrame(0),
            TempoChange {
                at: BeatTicks(beat * 960),
                bpm: 120.0,
            },
        )
        .unwrap();
    }
    assert!(
        map.edit_future(
            timebase::SampleFrame(0),
            TempoChange {
                at: BeatTicks(64 * 960),
                bpm: 60.0
            }
        )
        .is_err()
    );
    assert_eq!(map.frame_at(BeatTicks(64 * 960)).unwrap().0, 64 * 4000);
}

#[test]
fn half_open_callback_boundary_and_sample_output() {
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    let ticket = c.submit(0, 0, Request::Launch(0), Quantize::Beat).unwrap();
    let (audio, events) = render(&mut e, 3999, 8192);
    assert!(audio.iter().all(|s| *s == 0.0));
    assert!(events.is_empty());
    assert_eq!(ticket.outcome(), Outcome::Queued);
    let (audio, events) = render(&mut e, 1, 1);
    assert_eq!(events[0].frame.0, 4000);
    assert!(audio[0] > 0.0);
    assert_eq!(e.source_position(0, 0), Some(1));
    c.submit(0, 0, Request::Stop, Quantize::Immediate).unwrap();
    let (audio, events) = render(&mut e, 1, 1);
    assert_eq!(events[0].frame.0, 4001);
    assert!(audio.iter().all(|s| *s == 0.0));
}

#[test]
fn dropped_engine_closes_control_and_pending_tickets() {
    let (e, mut c) = engine(8000);
    let ticket = c.submit(0, 0, Request::Launch(0), Quantize::Beat).unwrap();
    drop(e);
    assert_eq!(ticket.outcome(), Outcome::Closed);
    assert!(c.submit(0, 0, Request::Stop, Quantize::Immediate).is_err());
}

#[test]
fn second_tempo_edit_retimes_the_same_pending_bar_again() {
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    c.submit(0, 0, Request::Launch(0), Quantize::Bar).unwrap();
    c.submit(0, 0, Request::Tempo(60.0), Quantize::Beat)
        .unwrap();
    render(&mut e, 5000, 1024);
    c.submit(0, 0, Request::Tempo(240.0), Quantize::Beat)
        .unwrap();
    let (_, events) = render(&mut e, 11000, 16384);
    // Beat 1 at 4000; beat 2 at 12000; beats 3 and 4 at 14000 and 16000.
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].frame.0, 16000);
}

#[test]
fn inclusive_boundary_survives_inverse_tick_rounding_at_high_tempo() {
    let map = TempoMap::new(
        8000,
        &[TempoChange {
            at: BeatTicks(0),
            bpm: 997.0,
        }],
    )
    .unwrap();
    let frame = map.frame_at(BeatTicks(5760)).unwrap();
    assert_eq!(map.beat_at(frame).unwrap(), BeatTicks(5761));
    let mut scheduler =
        MusicalScheduler::new(map, [BeatTicks(0); 2], [BeatTicks(3840); 2]).unwrap();
    for _ in 0..frame.0 {
        scheduler.step().unwrap();
    }
    assert_eq!(
        scheduler.boundary(0, Quantize::Beat).unwrap(),
        BeatTicks(5760)
    );
    scheduler.step().unwrap();
    assert_eq!(
        scheduler.boundary(0, Quantize::Beat).unwrap(),
        BeatTicks(6720)
    );
}

#[test]
fn sixteen_slots_sum_without_normalization_jumps_and_stop_independently() {
    use parts::SLOTS;
    let (mut single, mut one) = engine(8000);
    one.submit(0, 0, Request::Launch(0), Quantize::Immediate)
        .unwrap();
    let (reference, _) = render(&mut single, 1000, 127);
    let (mut all, mut control) = engine(8000);
    for deck in 0..2 {
        for slot in 0..SLOTS {
            control
                .submit(deck, slot, Request::Launch(0), Quantize::Immediate)
                .unwrap();
        }
    }
    let (audio, events) = render(&mut all, 1000, 127);
    assert_eq!(events.len(), 2 * SLOTS);
    for (a, b) in audio.chunks_exact(4).zip(reference.chunks_exact(4)) {
        assert!((a[0] - b[0] * (2 * SLOTS) as f32).abs() < 1e-6);
        assert_eq!(a[0], a[1]);
        assert!(a.iter().all(|s| s.abs() <= 0.125));
    }
    for deck in 0..2 {
        for slot in 0..SLOTS {
            let status = control.slot(deck, slot).unwrap();
            assert_eq!(status.state, audio_backend::live::SlotState::Playing);
            assert_eq!(status.position, 1000);
            assert_eq!(status.processed_frames, 1000);
            assert!(status.peak[0] > 0.0);
        }
    }
    for slot in 0..SLOTS {
        control
            .submit(0, slot, Request::Stop, Quantize::Immediate)
            .unwrap();
    }
    render(&mut all, 100, 128);
    for slot in 0..SLOTS {
        assert_eq!(control.slot(0, slot).unwrap().processed_frames, 1000);
        assert_eq!(control.slot(0, slot).unwrap().peak, [0.0; 2]);
        assert_eq!(control.slot(1, slot).unwrap().processed_frames, 1100);
    }
    for slot in 0..SLOTS {
        control
            .submit(1, slot, Request::Stop, Quantize::Immediate)
            .unwrap();
    }
    let (silence, _) = render(&mut all, 1000, 8192);
    assert!(silence.iter().all(|s| *s == 0.0));
}

#[test]
fn part_controls_are_smoothed_independent_and_preserve_cue_phase() {
    use audio_backend::part_dsp::PartParameters;
    let (mut e, mut c) = engine(8000);
    for deck in 0..2 {
        c.submit(deck, 0, Request::Launch(0), Quantize::Immediate)
            .unwrap();
    }
    render(&mut e, 1000, 128);
    let mute = c
        .submit(
            0,
            0,
            Request::Parameters(PartParameters {
                muted: true,
                ..PartParameters::default()
            }),
            Quantize::Immediate,
        )
        .unwrap();
    render(&mut e, 200, 128);
    assert_eq!(mute.outcome(), Outcome::Applied);
    let (output, _) = render(&mut e, 4, 4);
    // A is muted; B remains in both master and private logical cue and advances normally.
    assert!(output[0] != 0.0 && output[2] != 0.0);
    assert_eq!(e.source_position(0, 0), e.source_position(1, 0));
    assert_eq!(c.slot(0, 0).unwrap().peak, [0.0; 2]);
    assert!(c.slot(1, 0).unwrap().peak[0] > 0.0);
    let reject = c
        .submit(
            0,
            0,
            Request::Parameters(PartParameters {
                cutoff_hz: Some(20000.0),
                ..PartParameters::default()
            }),
            Quantize::Immediate,
        )
        .unwrap();
    render(&mut e, 1, 1);
    assert_eq!(reject.outcome(), Outcome::Rejected);
    assert!(
        c.submit(
            0,
            0,
            Request::Parameters(PartParameters {
                gain: f32::NAN,
                ..PartParameters::default()
            }),
            Quantize::Immediate
        )
        .is_err()
    );
    assert!(
        c.submit(
            0,
            0,
            Request::Parameters(PartParameters::default()),
            Quantize::Beat
        )
        .is_err()
    );
    c.submit(
        0,
        1,
        Request::Parameters(PartParameters {
            muted: true,
            ..PartParameters::default()
        }),
        Quantize::Immediate,
    )
    .unwrap();
    c.submit(0, 1, Request::Launch(0), Quantize::Immediate)
        .unwrap();
    render(&mut e, 1, 1);
    assert_eq!(c.slot(0, 1).unwrap().peak, [0.0; 2]);
}

#[test]
fn dsp_automation_partition_invariance() {
    use audio_backend::part_dsp::PartParameters;
    fn run(block: usize) -> Vec<f32> {
        let (mut e, mut c) = engine(8000);
        c.submit(0, 0, Request::Launch(0), Quantize::Immediate)
            .unwrap();
        let mut audio = Vec::new();
        for parameters in [
            PartParameters::default(),
            PartParameters {
                gain: 0.5,
                cutoff_hz: Some(200.0),
                muted: false,
            },
            PartParameters {
                gain: 1.0,
                muted: true,
                cutoff_hz: Some(2000.0),
            },
            PartParameters::default(),
        ] {
            c.submit(0, 0, Request::Parameters(parameters), Quantize::Immediate)
                .unwrap();
            audio.extend(render(&mut e, 1000, block).0);
        }
        audio
    }
    assert_eq!(run(1), run(127));
    assert_eq!(run(8192), run(31));
}

#[test]
fn eight_bar_fills_have_deck_relative_phase_and_partition_invariance() {
    use audio_backend::live::{RulePhase, TriggerRule};
    fn run(block: usize) -> (Vec<f32>, Vec<(usize, i64)>) {
        let (mut e, mut c) = engine_phrase(8000, [BeatTicks(32 * 960); 2]);
        render(&mut e, 1, 1);
        for deck in 0..2 {
            c.submit(
                deck,
                0,
                Request::Rule {
                    bank: 2,
                    rule: TriggerRule::PhraseBar {
                        bar: 8,
                        phrase_bars: 8,
                        phase: RulePhase::Deck,
                    },
                },
                Quantize::Immediate,
            )
            .unwrap();
        }
        let (audio, events) = render(&mut e, 256000, block);
        assert_eq!(e.dropped_events(), 0);
        let trace: Vec<_> = events
            .iter()
            .map(|event| (event.deck, event.frame.0))
            .collect();
        assert_eq!(
            trace,
            vec![(1, 108000), (0, 112000), (1, 236000), (0, 240000)]
        );
        for deck in 0..2 {
            let status = c.slot(deck, 0).unwrap();
            assert_eq!(status.rule_fired, 2);
            assert_eq!(status.state, audio_backend::live::SlotState::Stopped);
        }
        (audio, trace)
    }
    assert_eq!(run(31), run(8192));
    assert_eq!(run(127), run(16384));
}

#[test]
fn four_bar_global_phase_and_every_beats_or_bars() {
    use audio_backend::live::{RulePhase, RuleUnit, TriggerRule};
    let (mut e, mut c) = engine_phrase(8000, [BeatTicks(16 * 960); 2]);
    render(&mut e, 1, 1);
    c.submit(
        1,
        0,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::PhraseBar {
                bar: 4,
                phrase_bars: 4,
                phase: RulePhase::Global,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::Every {
                n: 2,
                unit: RuleUnit::Beats,
                phase: RulePhase::Global,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    c.submit(
        0,
        1,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::Every {
                n: 2,
                unit: RuleUnit::Bars,
                phase: RulePhase::Global,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    let (_, events) = render(&mut e, 64000, 1024);
    assert_eq!(
        events
            .iter()
            .filter(|v| v.deck == 1)
            .map(|v| v.frame.0)
            .collect::<Vec<_>>(),
        vec![48000]
    );
    assert_eq!(
        events
            .iter()
            .filter(|v| v.deck == 0 && v.slot == 0)
            .map(|v| v.frame.0)
            .collect::<Vec<_>>(),
        (1..=8).map(|i| i * 8000).collect::<Vec<_>>()
    );
    assert_eq!(
        events
            .iter()
            .filter(|v| v.deck == 0 && v.slot == 1)
            .map(|v| v.frame.0)
            .collect::<Vec<_>>(),
        vec![32000, 64000]
    );
}

#[test]
fn once_fill_is_spent_after_one_boundary_and_cannot_be_enabled_again() {
    use audio_backend::live::TriggerRule;
    let (mut e, mut c) = engine(8000);
    render(&mut e, 3999, 8192);
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::Once {
                boundary: Quantize::Beat,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    let (_, events) = render(&mut e, 9000, 1024);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].frame.0, 4000);
    let status = c.slot(0, 0).unwrap();
    assert!(status.rule_spent && !status.rule_enabled);
    assert_eq!(status.state, audio_backend::live::SlotState::Stopped);
    let ticket = c
        .submit(0, 0, Request::EnableRule(true), Quantize::Immediate)
        .unwrap();
    let (_, events) = render(&mut e, 8000, 8192);
    assert!(events.is_empty());
    assert_eq!(ticket.outcome(), Outcome::Rejected);
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::Once {
                boundary: Quantize::Immediate,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    let before = e.frame();
    let (_, events) = render(&mut e, 1, 1);
    assert_eq!(events[0].frame, before);
}

#[test]
fn disable_enable_and_cancel_rule_are_not_manual_part_cancellation() {
    use audio_backend::live::{RulePhase, RuleUnit, TriggerRule};
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::Every {
                n: 1,
                unit: RuleUnit::Beats,
                phase: RulePhase::Global,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    c.submit(0, 0, Request::EnableRule(false), Quantize::Immediate)
        .unwrap();
    assert!(render(&mut e, 10000, 8192).1.is_empty());
    c.submit(0, 0, Request::EnableRule(true), Quantize::Immediate)
        .unwrap();
    let (_, events) = render(&mut e, 2000, 1024);
    assert_eq!(events[0].frame.0, 12000);
    c.submit(0, 0, Request::CancelRule, Quantize::Immediate)
        .unwrap();
    assert!(render(&mut e, 16000, 8192).1.is_empty());
    assert!(!c.slot(0, 0).unwrap().rule_installed);
}

#[test]
fn manual_actions_win_collisions_and_stop_disables_recurring_restart() {
    use audio_backend::live::{RulePhase, RuleUnit, TriggerRule};
    use event_engine::live::EventOrigin;
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 0,
            rule: TriggerRule::Every {
                n: 1,
                unit: RuleUnit::Beats,
                phase: RulePhase::Global,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    c.submit(0, 0, Request::Launch(0), Quantize::Beat).unwrap();
    let (_, events) = render(&mut e, 4000, 8192);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].origin, EventOrigin::Manual);
    assert_eq!(c.slot(0, 0).unwrap().rule_skipped, 1);
    c.submit(0, 0, Request::Retrigger, Quantize::Beat).unwrap();
    let (_, events) = render(&mut e, 4000, 8192);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].origin, EventOrigin::Manual);
    c.submit(0, 0, Request::Stop, Quantize::Beat).unwrap();
    let (_, events) = render(&mut e, 12000, 8192);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].action, Applied::Stop { .. }));
    assert!(!c.slot(0, 0).unwrap().rule_enabled);
}

#[test]
fn periodic_deadlines_retime_and_trace_overflow_is_explicit() {
    use audio_backend::live::{RulePhase, RuleUnit, TriggerRule};
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::Every {
                n: 1,
                unit: RuleUnit::Beats,
                phase: RulePhase::Global,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    c.submit(0, 0, Request::Tempo(60.0), Quantize::Beat)
        .unwrap();
    let (_, events) = render(&mut e, 20000, 16384);
    assert_eq!(
        events.iter().map(|v| v.frame.0).collect::<Vec<_>>(),
        vec![4000, 12000, 20000]
    );
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    for deck in 0..2 {
        for slot in 0..parts::SLOTS {
            c.submit(
                deck,
                slot,
                Request::Rule {
                    bank: 2,
                    rule: TriggerRule::Every {
                        n: 1,
                        unit: RuleUnit::Beats,
                        phase: RulePhase::Global,
                    },
                },
                Quantize::Immediate,
            )
            .unwrap();
        }
    }
    let mut output = vec![0.0; 40001 * 4];
    e.render(
        &mut output,
        OutputRouting::new(4, [0, 1], Some([2, 3])).unwrap(),
    );
    assert_eq!(e.events().count(), 128);
    assert_eq!(e.dropped_events(), 32);
    assert!(!e.failed());
    assert_eq!(c.slot(0, 0).unwrap().rule_fired, 10);
}

#[test]
fn invalid_rule_replacement_is_transactional() {
    use audio_backend::live::{RulePhase, RuleUnit, TriggerRule};
    let (mut e, mut c) = engine_phrase(8000, [BeatTicks(16 * 960); 2]);
    let valid = TriggerRule::Every {
        n: 1,
        unit: RuleUnit::Beats,
        phase: RulePhase::Global,
    };
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 2,
            rule: valid,
        },
        Quantize::Immediate,
    )
    .unwrap();
    render(&mut e, 1, 1);
    for rule in [
        TriggerRule::Every {
            n: 0,
            unit: RuleUnit::Beats,
            phase: RulePhase::Global,
        },
        TriggerRule::PhraseBar {
            bar: 9,
            phrase_bars: 8,
            phase: RulePhase::Global,
        },
        TriggerRule::PhraseBar {
            bar: 1,
            phrase_bars: 8,
            phase: RulePhase::Deck,
        },
    ] {
        let ticket = c
            .submit(0, 0, Request::Rule { bank: 2, rule }, Quantize::Immediate)
            .unwrap();
        render(&mut e, 1, 1);
        assert_eq!(ticket.outcome(), Outcome::Rejected);
        assert_eq!(e.scheduler().rule_status(0, 0).unwrap().rule, valid);
    }
}

#[test]
fn rule_boundary_inclusion_and_disable_at_due_frame() {
    use audio_backend::live::TriggerRule;
    for (arrival, expected) in [(3999, 4000), (4000, 4000), (4001, 8000)] {
        let (mut e, mut c) = engine(8000);
        render(&mut e, arrival, 8192);
        c.submit(
            0,
            0,
            Request::Rule {
                bank: 2,
                rule: TriggerRule::Once {
                    boundary: Quantize::Beat,
                },
            },
            Quantize::Immediate,
        )
        .unwrap();
        let (_, events) = render(&mut e, expected - arrival + 1, 8192);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].frame.0, expected as i64);
    }
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::Once {
                boundary: Quantize::Beat,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    render(&mut e, 3999, 8192);
    c.submit(0, 0, Request::EnableRule(false), Quantize::Immediate)
        .unwrap();
    assert!(render(&mut e, 1, 1).1.is_empty());
    assert!(!c.slot(0, 0).unwrap().rule_spent);
}

#[test]
fn pending_manual_action_skips_occurrences_without_postponing_them() {
    use audio_backend::live::{RulePhase, RuleUnit, TriggerRule};
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::Every {
                n: 1,
                unit: RuleUnit::Beats,
                phase: RulePhase::Global,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    c.submit(0, 0, Request::Launch(0), Quantize::Bar).unwrap();
    let (_, events) = render(&mut e, 20000, 8192);
    assert_eq!(
        events.iter().map(|v| v.frame.0).collect::<Vec<_>>(),
        vec![16000, 20000]
    );
    assert_eq!(c.slot(0, 0).unwrap().rule_skipped, 4);
}

#[test]
fn rule_tempo_and_audio_matrix_at_device_rates() {
    use audio_backend::live::{RulePhase, RuleUnit, TriggerRule};
    fn run(rate: u32, block: usize) -> (Vec<f32>, Vec<i64>) {
        let (mut e, mut c) = engine(rate);
        render(&mut e, 1, 1);
        c.submit(
            0,
            0,
            Request::Rule {
                bank: 2,
                rule: TriggerRule::Every {
                    n: 1,
                    unit: RuleUnit::Beats,
                    phase: RulePhase::Global,
                },
            },
            Quantize::Immediate,
        )
        .unwrap();
        c.submit(0, 0, Request::Tempo(60.0), Quantize::Beat)
            .unwrap();
        let (audio, events) = render(&mut e, rate as usize * 3, block);
        let frames: Vec<_> = events.iter().map(|v| v.frame.0).collect();
        assert_eq!(
            frames,
            vec![
                i64::from(rate) / 2,
                i64::from(rate) * 3 / 2,
                i64::from(rate) * 5 / 2
            ]
        );
        (audio, frames)
    }
    for rate in [44100, 48000, 96000] {
        assert_eq!(run(rate, 31), run(rate, 8192));
    }
}

#[test]
fn once_fill_losing_to_manual_launch_is_spent_not_deferred() {
    use audio_backend::live::TriggerRule;
    let (mut e, mut c) = engine(8000);
    render(&mut e, 1, 1);
    c.submit(
        0,
        0,
        Request::Rule {
            bank: 2,
            rule: TriggerRule::Once {
                boundary: Quantize::Beat,
            },
        },
        Quantize::Immediate,
    )
    .unwrap();
    c.submit(0, 0, Request::Launch(2), Quantize::Beat).unwrap();
    let (_, events) = render(&mut e, 16000, 8192);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].frame.0, 4000);
    let status = c.slot(0, 0).unwrap();
    assert!(status.rule_spent);
    assert_eq!(status.rule_fired, 0);
    assert_eq!(status.rule_skipped, 1);
    let enable = c
        .submit(0, 0, Request::EnableRule(true), Quantize::Immediate)
        .unwrap();
    assert!(render(&mut e, 8000, 8192).1.is_empty());
    assert_eq!(enable.outcome(), Outcome::Rejected);
}
