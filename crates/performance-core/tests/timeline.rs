// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{
    demo,
    live::{LiveEngine, Quantize, RulePhase, RuleUnit, SlotState, TriggerRule},
    routing::OutputRouting,
};
use dawj_core::{
    Coordinator, Intent, PartAction, RuleAction,
    timeline::{self, BAR, MarkerKind, View},
};
use timebase::BeatTicks;
fn setup(bars: u8) -> (LiveEngine, Coordinator) {
    let (e, c, m) = demo::prepare_with_phrase_bars(bars).unwrap();
    (e, Coordinator::new(c, m, false))
}
fn render(e: &mut LiveEngine, c: &mut Coordinator, frames: usize) {
    let mut buffer = [0.0; 1024];
    let mut left = frames;
    while left > 0 {
        let n = left.min(512);
        e.render(&mut buffer[..n * 2], OutputRouting::stereo_master_only());
        left -= n;
    }
    c.poll();
    assert!(!e.failed());
}
fn part(c: &mut Coordinator, d: usize, s: usize, action: PartAction) {
    c.try_act(Intent::Part {
        deck: d,
        slot: s,
        action,
    })
    .unwrap();
}
fn rule(c: &mut Coordinator, d: usize, s: usize, action: RuleAction) {
    c.try_act(Intent::Rule {
        deck: d,
        slot: s,
        action,
    })
    .unwrap();
}

#[test]
fn queued_phrase_boundary_is_visible_on_both_sides_of_wrap_and_only_audio_applies_it() {
    let (mut e, mut c) = setup(8);
    let view = View::default();
    render(&mut e, &mut c, 1);
    c.try_act(Intent::Quantize(Quantize::Phrase)).unwrap();
    part(&mut c, 0, 0, PartAction::Launch);
    render(&mut e, &mut c, 1);
    let s = c.snapshot.unwrap();
    let w = view.window(&s);
    assert_eq!(w.start, BeatTicks(0));
    assert_eq!(w.end, BeatTicks(8 * BAR));
    let pending = timeline::markers(&s, w)
        .into_iter()
        .find(|m| m.manual)
        .unwrap();
    assert_eq!(pending.at, BeatTicks(8 * BAR));
    assert_eq!(pending.kind, MarkerKind::Launch);
    assert_eq!(w.fraction(pending.at), 1.0);
    let mut fill = pending;
    fill.kind = MarkerKind::Fill;
    fill.slot = 7;
    let mut stop = pending;
    stop.kind = MarkerKind::Stop;
    stop.slot = 3;
    assert_eq!(timeline::marker_label(&[pending, stop, fill]), "L/S/F");
    let mut projected = fill;
    projected.kind = MarkerKind::RuleFill;
    projected.projected = true;
    assert!(timeline::marker_label(&[pending, projected]).contains("~F*"));
    render(&mut e, &mut c, 768000 - 25 - 2);
    assert_eq!(
        timeline::phrase_position(&c.snapshot.unwrap(), 0),
        Some((8, 8))
    );
    render(&mut e, &mut c, 25);
    let s = c.snapshot.unwrap();
    assert_eq!(s.frame.0, 768000);
    assert_eq!(s.beat, BeatTicks(8 * BAR));
    assert_eq!(timeline::phrase_position(&s, 0), Some((1, 8)));
    let w = view.window(&s);
    assert_eq!(w.start, BeatTicks(8 * BAR));
    assert_eq!(s.decks[0].slots[0].lifecycle, SlotState::QueuedStart); // half-open render boundary
    assert_eq!(
        timeline::markers(&s, w)
            .iter()
            .find(|m| m.manual)
            .unwrap()
            .at,
        w.start
    );
    render(&mut e, &mut c, 1);
    let s = c.snapshot.unwrap();
    assert_eq!(s.decks[0].slots[0].lifecycle, SlotState::Playing);
    assert!(
        timeline::markers(&s, view.window(&s))
            .iter()
            .all(|m| !m.manual)
    );
    assert_eq!(s.decks[0].slots[0].nominal_length, Some(BeatTicks(BAR)));
}

#[test]
fn tempo_edits_keep_musical_markers_and_rules_synchronized_with_exact_engine_frames() {
    let (mut e, mut c) = setup(8);
    render(&mut e, &mut c, 1);
    c.try_act(Intent::Quantize(Quantize::Bar)).unwrap();
    part(&mut c, 0, 0, PartAction::Launch);
    rule(
        &mut c,
        1,
        7,
        RuleAction::Install(TriggerRule::Every {
            n: 1,
            unit: RuleUnit::Bars,
            phase: RulePhase::Deck,
        }),
    );
    c.try_act(Intent::Tempo(180.0)).unwrap();
    render(&mut e, &mut c, 1);
    let s = c.snapshot.unwrap();
    assert_eq!(s.bpm, Some(120.0));
    assert_eq!(s.decks[1].slots[7].rule.unwrap().next, Some(BeatTicks(BAR)));
    assert!(
        timeline::markers(&s, View::default().window(&s))
            .iter()
            .any(|m| m.kind == MarkerKind::RuleFill && m.at == BeatTicks(BAR) && !m.projected)
    );
    render(&mut e, &mut c, 96000 - 2);
    let s = c.snapshot.unwrap();
    assert_eq!(s.bpm, Some(180.0));
    assert_eq!(s.decks[0].slots[0].lifecycle, SlotState::QueuedStart);
    assert_eq!(s.decks[1].slots[7].rule.unwrap().fired, 0);
    render(&mut e, &mut c, 1);
    let s = c.snapshot.unwrap();
    assert_eq!(s.decks[0].slots[0].lifecycle, SlotState::Playing);
    let r = s.decks[1].slots[7].rule.unwrap();
    assert_eq!(r.fired, 1);
    assert_eq!(r.last_fired, Some(BeatTicks(BAR)));
    assert_eq!(r.next, Some(BeatTicks(2 * BAR)));
    render(&mut e, &mut c, 63999);
    assert_eq!(c.snapshot.unwrap().frame.0, 160000);
    assert_eq!(c.snapshot.unwrap().decks[1].slots[7].rule.unwrap().fired, 1);
    render(&mut e, &mut c, 1);
    let s = c.snapshot.unwrap();
    assert_eq!(
        s.decks[1].slots[7].rule.unwrap().last_fired,
        Some(BeatTicks(2 * BAR))
    );
    assert_eq!(s.decks[1].slots[7].position, Some(1));
}

#[test]
fn cancelling_manual_fill_and_rules_never_fakes_a_completed_or_stopped_source() {
    let (mut e, mut c) = setup(8);
    render(&mut e, &mut c, 1);
    c.try_act(Intent::Quantize(Quantize::Phrase)).unwrap();
    part(&mut c, 0, 7, PartAction::Launch);
    render(&mut e, &mut c, 1);
    let s = c.snapshot.unwrap();
    let p = s.decks[0].slots[7];
    assert!(p.pending_one_shot);
    assert!(!p.active_one_shot);
    assert_eq!(
        timeline::markers(&s, View::default().window(&s))[0].kind,
        MarkerKind::Fill
    );
    part(&mut c, 0, 7, PartAction::Cancel);
    assert!(c.snapshot.unwrap().decks[0].slots[7].pending.is_some());
    render(&mut e, &mut c, 1);
    assert!(c.snapshot.unwrap().decks[0].slots[7].pending.is_none());
    rule(
        &mut c,
        0,
        7,
        RuleAction::Install(TriggerRule::Once {
            boundary: Quantize::Phrase,
        }),
    );
    render(&mut e, &mut c, 1);
    assert!(
        c.snapshot.unwrap().decks[0].slots[7]
            .rule
            .unwrap()
            .next
            .is_some()
    );
    rule(&mut c, 0, 7, RuleAction::Cancel);
    render(&mut e, &mut c, 1);
    assert!(c.snapshot.unwrap().decks[0].slots[7].rule.is_none());
    rule(
        &mut c,
        0,
        7,
        RuleAction::Install(TriggerRule::Once {
            boundary: Quantize::Immediate,
        }),
    );
    render(&mut e, &mut c, 1);
    let p = c.snapshot.unwrap().decks[0].slots[7];
    assert!(p.active_one_shot);
    assert!(p.rule.unwrap().consumed);
    assert_eq!(p.rule.unwrap().fired, 1);
    assert!(timeline::inspector(p).contains("spent"));
    rule(&mut c, 0, 7, RuleAction::Cancel);
    render(&mut e, &mut c, 1);
    assert!(c.snapshot.unwrap().decks[0].slots[7].active.is_some()); // remove future intent, not the sounding fill
    assert!(c.snapshot.unwrap().decks[0].slots[7].rule.is_none());
}

#[test]
fn periodic_skip_pause_resume_and_stop_feedback_are_authoritative() {
    let (mut e, mut c) = setup(4);
    render(&mut e, &mut c, 1);
    let definition = TriggerRule::Every {
        n: 1,
        unit: RuleUnit::Bars,
        phase: RulePhase::Deck,
    };
    rule(&mut c, 0, 7, RuleAction::Install(definition));
    c.try_act(Intent::Quantize(Quantize::Phrase)).unwrap();
    part(&mut c, 0, 7, PartAction::Launch);
    render(&mut e, &mut c, 96000);
    let r = c.snapshot.unwrap().decks[0].slots[7].rule.unwrap();
    assert_eq!(r.fired, 0);
    assert_eq!(r.skipped, 1);
    part(&mut c, 0, 7, PartAction::Cancel);
    render(&mut e, &mut c, 1);
    rule(&mut c, 0, 7, RuleAction::Enable(false));
    render(&mut e, &mut c, 1);
    assert!(
        c.snapshot.unwrap().decks[0].slots[7]
            .rule
            .unwrap()
            .next
            .is_none()
    );
    rule(&mut c, 0, 7, RuleAction::Enable(true));
    render(&mut e, &mut c, 1);
    assert_eq!(
        c.snapshot.unwrap().decks[0].slots[7].rule.unwrap().next,
        Some(BeatTicks(2 * BAR))
    );
    let remaining = 192001 - c.snapshot.unwrap().frame.0 as usize;
    render(&mut e, &mut c, remaining);
    assert_eq!(c.snapshot.unwrap().decks[0].slots[7].rule.unwrap().fired, 1);
    c.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    part(&mut c, 0, 7, PartAction::Stop);
    render(&mut e, &mut c, 1);
    assert!(
        c.snapshot.unwrap().decks[0].slots[7]
            .rule
            .unwrap()
            .next
            .is_none()
    );
    assert!(timeline::inspector(c.snapshot.unwrap().decks[0].slots[7]).contains("paused"));
}

#[test]
fn zoom_and_phrase_configuration_change_view_without_rescheduling() {
    assert!(demo::prepare_with_phrase_bars(6).is_err());
    let (mut e, mut c) = setup(4);
    render(&mut e, &mut c, 120000);
    let s = c.snapshot.unwrap();
    assert_eq!(s.decks[0].phrase_length, BeatTicks(4 * BAR));
    assert_eq!(s.decks[0].epoch, BeatTicks(0));
    let mut view = View {
        zoom_bars: 1,
        ..Default::default()
    };
    let w = view.window(&s);
    assert_eq!(w.start, BeatTicks(BAR));
    assert_eq!(w.end, BeatTicks(2 * BAR));
    view.set_follow(false, &s);
    assert_eq!(view.anchor, Some(BeatTicks(0)));
    view.page_by(&s, 1);
    let w = view.window(&s);
    assert_eq!(w.start, BeatTicks(2 * BAR));
    assert!(!view.follow);
    let generation = s.generation;
    render(&mut e, &mut c, 240000);
    let later = c.snapshot.unwrap();
    assert!(later.generation > generation);
    assert_eq!(view.window(&later), w);
    view.follow = true;
    assert_ne!(view.window(&later), w);
    assert!(c.snapshot.unwrap().decks.iter().all(|d| {
        d.slots
            .iter()
            .all(|p| p.active.is_none() && p.pending.is_none())
    }));
}

#[test]
fn snapshot_rule_codec_covers_definitions_and_projection_is_bounded() {
    let (mut e, mut c) = setup(8);
    render(&mut e, &mut c, 1);
    let mut definitions = vec![];
    for q in [
        Quantize::Immediate,
        Quantize::Beat,
        Quantize::Bar,
        Quantize::Phrase,
    ] {
        definitions.push(TriggerRule::Once { boundary: q });
    }
    for unit in [RuleUnit::Beats, RuleUnit::Bars] {
        for phase in [RulePhase::Global, RulePhase::Deck] {
            definitions.push(TriggerRule::Every {
                n: 1024,
                unit,
                phase,
            });
        }
    }
    for phase in [RulePhase::Global, RulePhase::Deck] {
        definitions.push(TriggerRule::PhraseBar {
            bar: 8,
            phrase_bars: 8,
            phase,
        });
    }
    for definition in definitions {
        rule(&mut c, 1, 7, RuleAction::Install(definition));
        render(&mut e, &mut c, 1);
        let status = c.snapshot.unwrap().decks[1].slots[7].rule.unwrap();
        assert_eq!(status.rule, definition);
        assert!(status.one_shot);
        assert_eq!(status.part.0, 16);
    }
    let mut long = *c.snapshot.as_ref().unwrap();
    long.beat = BeatTicks(960 * 1_000_000_000);
    let view = View::default();
    let w = view.window(&long);
    let r = long.decks[1].slots[7].rule.as_mut().unwrap();
    r.next = Some(BeatTicks(0));
    r.period = Some(BeatTicks(960));
    assert!(timeline::markers(&long, w).len() <= timeline::MAX_OCCURRENCES + 1);
    assert!(
        c.try_act(Intent::Rule {
            deck: 2,
            slot: 0,
            action: RuleAction::Cancel
        })
        .is_err()
    );
}

#[test]
fn concurrent_rule_snapshots_keep_all_sixteen_deadlines_in_one_clock_generation() {
    use audio_backend::live::Request;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (mut engine, mut control, _) = demo::prepare().unwrap();
    for deck in 0..2 {
        for slot in 0..8 {
            control
                .submit(
                    deck,
                    slot,
                    Request::Rule {
                        bank: (deck * 8 + slot) as u8,
                        rule: TriggerRule::Every {
                            n: 1,
                            unit: RuleUnit::Beats,
                            phase: RulePhase::Deck,
                        },
                    },
                    Quantize::Immediate,
                )
                .unwrap();
        }
    }
    engine.render(&mut [0.0; 2], OutputRouting::stereo_master_only());
    let done = Arc::new(AtomicBool::new(false));
    let worker_done = done.clone();
    let writer = std::thread::spawn(move || {
        for _ in 0..2000 {
            engine.render(&mut [0.0; 128], OutputRouting::stereo_master_only());
        }
        assert!(!engine.failed());
        worker_done.store(true, Ordering::Release);
        engine
    });
    let mut reads = 0;
    for _ in 0..100000 {
        if let Some(s) = control.performance_state() {
            reads += 1;
            let fired = 1 + (s.frame.0 - 1) / 24000;
            assert_eq!(
                s.beat.0,
                (s.frame.0 as f64 * 960.0 / 24000.0).round() as i64
            );
            for (d, deck) in s.decks.iter().enumerate() {
                assert_eq!(deck.phase.0, s.beat.0.rem_euclid(8 * BAR));
                assert_eq!(deck.phrase_length, BeatTicks(8 * BAR));
                for (i, p) in deck.slots.iter().enumerate() {
                    let r = p.rule.unwrap();
                    assert_eq!(r.part.0, (d * 8 + i + 1) as u64);
                    assert_eq!(r.one_shot, i == 7);
                    assert_eq!(r.fired, fired as u64);
                    assert_eq!(r.next, Some(BeatTicks(fired * 960)));
                    assert_eq!(r.last_fired, Some(BeatTicks((fired - 1) * 960)));
                }
            }
        }
        if done.load(Ordering::Acquire) {
            break;
        }
        std::thread::yield_now();
    }
    let _ = writer.join().unwrap();
    assert!(reads > 0);
}
