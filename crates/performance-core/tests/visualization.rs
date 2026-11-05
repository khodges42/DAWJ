// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{demo, live::Quantize, routing::OutputRouting};
use dawj_core::{
    Coordinator,
    session::{Project, Workspace},
    visualization::{self as viz, Section, Structure, Style},
    waveform::{Cache, Pyramid},
};
use timebase::{BeatTicks, TempoChange, TempoMap};
fn map(bpm: f64) -> TempoMap {
    TempoMap::new(
        48000,
        &[TempoChange {
            at: BeatTicks(0),
            bpm,
        }],
    )
    .unwrap()
}
fn project() -> Project {
    let mut p = Project::default();
    p.create("Original".into(), 0).unwrap();
    p
}
#[test]
fn live_trimmed_loop_cursor_wraps_elapsed_frames_instead_of_clamping_to_eof() {
    use dawj_core::session::MediaInfo;
    let (_, control, mixer) = demo::prepare().unwrap();
    let mut co = Coordinator::new(control, mixer, false);
    let mut p = project();
    let part = &mut p.compositions[0].parts[0];
    part.looping = true;
    part.sample_start = 100;
    part.sample_end = Some(500);
    part.loop_start = 150;
    part.loop_end = Some(350);
    let mut observed = co.snapshot.unwrap();
    observed.decks[0].slots[0].active = Some(parts::PartId(part.id));
    observed.decks[0].slots[0].position = Some(1150);
    co.snapshot = Some(observed);
    let w = Workspace {
        project: p,
        media: vec![MediaInfo {
            preparation_gain_db: 0.,
            waveform: None,
            deck: 0,
            slot: 0,
            path: "source.wav".into(),
            filename: "source.wav".into(),
            duration: 1.,
            sample_rate: 48000,
            channels: 2,
            prepared_frames: 1000,
            error: None,
        }],
        ..Default::default()
    };
    let positions = viz::positions(&w, &co).unwrap();
    assert_eq!(positions[0].cursors[0].source, Some(0.75));
}
fn wave() -> Pyramid {
    Pyramid::analyze(&vec![0.5; 4800], 1, 48000).unwrap()
}
#[test]
fn minmax_preserves_transients_stereo_phase_silence_and_short_sources() {
    let mut pcm = vec![0.; 20000];
    pcm[1024] = 1.;
    pcm[1025] = -1.;
    let p = Pyramid::analyze(&pcm, 2, 48000).unwrap();
    let full = p.range(0, p.frames);
    assert_eq!(full.max, [1., 0.]);
    assert_eq!(full.min, [0., -1.]);
    assert!(full.rms > 0.);
    for width in [1, 32, 512, 1024] {
        let c = p.columns(0, p.frames, width);
        assert_eq!(c.len(), width);
        assert!(c.iter().any(|b| b.max[0] == 1. && b.min[1] == -1.));
    }
    for pcm in [vec![0.], vec![0.; 100], vec![0.75]] {
        let p = Pyramid::analyze(&pcm, 1, 44100).unwrap();
        assert!(p.columns(0, p.frames, 64).iter().all(|b| b.rms.is_finite()));
        assert_eq!(p.range(0, p.frames).max, [pcm[0]; 2]);
    }
    for (pcm, ch, rate) in [
        (vec![], 1, 48000),
        (vec![f32::NAN], 1, 48000),
        (vec![2.], 1, 48000),
        (vec![0.], 3, 48000),
        (vec![0.], 1, 0),
    ] {
        assert!(Pyramid::analyze(&pcm, ch, rate).is_err())
    }
}
#[test]
fn high_frequency_sources_do_not_cancel_and_long_analysis_has_bounded_storage() {
    let pcm: Vec<_> = (0..1_000_000)
        .map(|i| if i % 2 == 0 { 1. } else { -1. })
        .collect();
    let p = Pyramid::analyze(&pcm, 1, 1000).unwrap();
    let e = p.range(0, p.frames);
    assert_eq!(e.min, [-1.; 2]);
    assert_eq!(e.max, [1.; 2]);
    assert!((e.rms - 1.).abs() < 0.00001);
    assert!(p.memory_bytes() < 100_000);
    assert_eq!(p.columns(p.frames, p.frames + 1, 1024)[0].rms, 0.);
}
#[test]
fn content_cache_reuses_identical_audio_and_invalidates_same_size_changes() {
    let mut cache = Cache::default();
    let a = wave();
    let id = cache.insert(a);
    let first = cache.get(&id).unwrap();
    assert_eq!(cache.insert(wave()), id);
    assert_eq!(cache.analyses, 1);
    assert!(std::sync::Arc::ptr_eq(&first, &cache.get(&id).unwrap()));
    let changed = Pyramid::analyze(&vec![-0.5; 4800], 1, 48000).unwrap();
    assert_ne!(cache.insert(changed), id);
    cache.pin(std::iter::once(id.clone()));
    for i in 0..200 {
        cache.insert(Pyramid::analyze(&[i as f32 / 201.], 1, 48000).unwrap());
    }
    assert!(cache.get(&id).is_some());
    assert!(cache.memory_bytes() < 8 * 1024 * 1024);
}
#[test]
fn sample_loops_use_source_frames_not_nominal_beats_or_fake_time_stretch() {
    let p = project();
    let mut s = p.compositions[0].parts[0].clone();
    s.media = Some("my.wav".into());
    s.loop_start = 100;
    s.loop_end = Some(2500);
    let wave = wave();
    let a = viz::project_part(
        &s,
        None,
        Some(&wave),
        Some(None),
        viz::PreviewWindow {
            ticks: 4 * viz::BAR,
            map: &map(120.),
            columns: 128,
            style: Style::Waveform,
        },
    );
    assert!((a.source_seconds - 0.05).abs() < 0.00001);
    assert_eq!(a.repetitions, 160.);
    s.nominal_ticks = 12 * viz::BAR;
    let b = viz::project_part(
        &s,
        None,
        Some(&wave),
        Some(None),
        viz::PreviewWindow {
            ticks: 4 * viz::BAR,
            map: &map(120.),
            columns: 128,
            style: Style::Waveform,
        },
    );
    assert_eq!(a.repetitions, b.repetitions);
    let faster = viz::project_part(
        &s,
        None,
        Some(&wave),
        Some(None),
        viz::PreviewWindow {
            ticks: 4 * viz::BAR,
            map: &map(240.),
            columns: 128,
            style: Style::Waveform,
        },
    );
    assert_eq!(faster.repetitions, 80.);
    assert_eq!(faster.source_seconds, a.source_seconds);
    let e = viz::loop_envelope(&wave, 100, 2500, 2390, 2410);
    assert_eq!(e.max, [0.5; 2]);
    assert_eq!(e.min, [0.; 2]);
    assert!((e.rms - 0.5).abs() < 0.00001);
}
#[test]
fn one_shots_do_not_become_continuous_loops_and_trim_is_respected() {
    let p = project();
    let mut s = p.compositions[0].parts[0].clone();
    s.looping = false;
    s.media = Some("my.wav".into());
    s.sample_start = 100;
    s.sample_end = Some(2500);
    let wave = wave();
    let v = viz::project_part(
        &s,
        None,
        Some(&wave),
        Some(None),
        viz::PreviewWindow {
            ticks: 8 * viz::BAR,
            map: &map(120.),
            columns: 128,
            style: Style::Hybrid,
        },
    );
    assert_eq!(v.repetitions, 1.);
    assert_eq!(v.boundaries, vec![0.]);
    assert!(v.energy[0] > 0.);
    assert!(v.energy[2..].iter().all(|e| *e == 0.));
    assert_eq!((v.source_start, v.source_end), (100, 2500));
    assert!(
        viz::project_part(
            &s,
            None,
            None,
            Some(None),
            viz::PreviewWindow {
                ticks: 8 * viz::BAR,
                map: &map(120.),
                columns: 128,
                style: Style::Structure
            }
        )
        .error
        .is_some()
    );
}
#[test]
fn pattern_density_accents_repetition_and_pending_definition_are_distinct() {
    let p = project();
    let mut s = p.compositions[0].parts[0].clone();
    s.looping = false;
    s.media = Some("my.wav".into());
    let mut old = parts::pattern::Pattern {
        bars: 2,
        ..Default::default()
    };
    for i in (0..16).step_by(4) {
        old.steps[0][i] = 127;
    }
    for i in (0..16).step_by(2) {
        old.steps[1][i] = 64;
    }
    let mut new = old;
    new.steps[0][1] = 100;
    s.pattern = Some(new);
    let v = viz::project_part(
        &s,
        None,
        Some(&wave()),
        Some(Some(old)),
        viz::PreviewWindow {
            ticks: 8 * viz::BAR,
            map: &map(120.),
            columns: 128,
            style: Style::Structure,
        },
    );
    assert_eq!(
        v.density.unwrap(),
        vec![0.25, 0.5, 0.25, 0.5, 0.25, 0.5, 0.25, 0.5]
    );
    assert_eq!(v.period_ticks, Some(2 * viz::BAR));
    assert_eq!(v.repetitions, 4.);
    assert_eq!(v.pattern, Some(old));
    assert_eq!(v.target_pattern, Some(new));
    assert!(v.peaks.is_none());
}
#[test]
fn section_positions_labels_and_manual_meaning_are_validated() {
    let mut structure = Structure {
        bars: 8,
        sections: vec![
            Section {
                id: 1,
                tick: 0,
                label: "Intro".into(),
                intensity: Some(0.9),
            },
            Section {
                id: 2,
                tick: 4 * viz::BAR,
                label: "Drop".into(),
                intensity: Some(0.1),
            },
        ],
        show_labels: true,
    };
    structure.validate().unwrap();
    structure.sections[1].tick = 1;
    assert!(structure.validate().is_err());
    structure.sections[1].tick = 8 * viz::BAR;
    assert!(structure.validate().is_err());
    structure.sections[1].tick = 4 * viz::BAR;
    structure.sections[1].id = 1;
    assert!(structure.validate().is_err());
    structure.sections[1].id = 2;
    structure.sections[1].label = "bad\nlabel".into();
    assert!(structure.validate().is_err());
}
#[test]
fn structures_roundtrip_and_old_local_documents_migrate_without_analysis_data() {
    let mut p = project();
    p.compositions[0].structure.sections = vec![
        Section {
            id: 1,
            tick: 0,
            label: "Intro".into(),
            intensity: None,
        },
        Section {
            id: 2,
            tick: 2 * viz::BAR,
            label: "Build".into(),
            intensity: None,
        },
        Section {
            id: 3,
            tick: 4 * viz::BAR,
            label: "Drop".into(),
            intensity: None,
        },
    ];
    let root = std::env::temp_dir().join(format!("dawj-visual-document-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("session.json");
    p.save(&path).unwrap();
    let reopened = Project::load(&path).unwrap();
    assert_eq!(
        reopened.compositions[0].structure,
        p.compositions[0].structure
    );
    let mut old = serde_json::to_value(&p).unwrap();
    for version in [1, 2] {
        old["version"] = version.into();
        old["compositions"][0]
            .as_object_mut()
            .unwrap()
            .remove("structure");
        std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        let reopened = Project::load(&path).unwrap();
        assert_eq!(reopened.version, 3);
        assert_eq!(reopened.compositions[0].structure, Structure::default());
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn preview_revision_is_static_while_audio_clock_advances_and_patterns_activate_at_bar() {
    let (mut e, c, m) = demo::prepare().unwrap();
    let mut co = Coordinator::new(c, m, false);
    let mut p = parts::pattern::Pattern::default();
    p.steps[0][0] = 127;
    co.set_patterns(&[(0, 0, Some(p))]).unwrap();
    e.render(&mut [0.; 128], OutputRouting::stereo_master_only());
    co.poll();
    let revision = co.snapshot.unwrap().decks[0].slots[0].pattern.revision;
    assert_eq!(co.pattern_definition(0, 0, revision), Some(Some(p)));
    co.try_act(dawj_core::Intent::Quantize(Quantize::Immediate))
        .unwrap();
    co.try_act(dawj_core::Intent::Launch).unwrap();
    e.render(&mut [0.; 128], OutputRouting::stereo_master_only());
    co.poll();
    assert!(
        co.snapshot.unwrap().decks[0].slots[0]
            .pattern
            .anchor
            .is_some()
    );
    let w = Workspace {
        project: project(),
        ..Default::default()
    };
    let before = viz::revision(&w, Some(&co));
    e.render(&mut [0.; 128], OutputRouting::stereo_master_only());
    co.poll();
    assert_eq!(before, viz::revision(&w, Some(&co)));
    p.steps[0][4] = 100;
    co.set_patterns(&[(0, 0, Some(p))]).unwrap();
    e.render(&mut [0.; 128], OutputRouting::stereo_master_only());
    co.poll();
    let current = co.snapshot.unwrap().decks[0].slots[0].pattern;
    assert_eq!(current.revision, revision);
    assert_ne!(co.pattern_definition(0, 0, revision), Some(Some(p)));
    while e.frame().0 < 100_000 {
        e.render(&mut [0.; 1024], OutputRouting::stereo_master_only());
    }
    co.poll();
    let current = co.snapshot.unwrap().decks[0].slots[0].pattern;
    assert_eq!(co.pattern_definition(0, 0, current.revision), Some(Some(p)));
    assert_ne!(before, viz::revision(&w, Some(&co)));
}
#[test]
fn request_budget_rejects_unbounded_or_duplicate_addresses() {
    let a = viz::Address { deck: 0, slot: 0 };
    assert!(viz::validate_request(0, &[a]).is_err());
    assert!(viz::validate_request(256, &[a, a]).is_err());
    assert!(viz::validate_request(1024, &[viz::Address { deck: 2, slot: 0 }]).is_err());
    viz::validate_request(256, &[a]).unwrap();
}
#[test]
fn source_rate_mapping_keeps_native_duration_and_does_not_use_nominal_timing() {
    let p = project();
    let mut s = p.compositions[0].parts[0].clone();
    s.media = Some("native.wav".into());
    s.loop_start = 0;
    s.loop_end = Some(2400);
    let preview = Pyramid::analyze(&vec![0.5; 4800], 1, 24000).unwrap();
    let part = viz::project_part(
        &s,
        None,
        Some(&preview),
        Some(None),
        viz::PreviewWindow {
            ticks: 4 * viz::BAR,
            map: &map(120.),
            columns: 128,
            style: Style::Waveform,
        },
    );
    assert_eq!(part.source_seconds, 0.1);
    assert_eq!(part.repetitions, 80.0);
}
#[test]
fn dense_long_samples_project_bounded_columns_with_monophonic_retriggers() {
    let p = project();
    let mut s = p.compositions[0].parts[0].clone();
    s.looping = false;
    s.media = Some("long.wav".into());
    let pattern = parts::pattern::Pattern {
        bars: 4,
        steps: [[127; 16]; 4],
    };
    s.pattern = Some(pattern);
    let wave = Pyramid::analyze(&vec![0.5; 1_000_000], 1, 48000).unwrap();
    for _ in 0..8 {
        let material = viz::project_part(
            &s,
            None,
            Some(&wave),
            Some(Some(pattern)),
            viz::PreviewWindow {
                ticks: 64 * viz::BAR,
                map: &map(120.),
                columns: 512,
                style: Style::Hybrid,
            },
        );
        assert_eq!(material.energy.len(), 512);
        assert_eq!(material.peaks.unwrap().len(), 512);
        assert!(material.energy.iter().all(|v| (*v - 0.5).abs() < 0.00001));
        assert_eq!(material.density.unwrap(), vec![1.0; 64]);
    }
}
#[test]
fn dense_rule_projections_do_not_hide_manual_stops_and_deck_epochs_stay_independent() {
    use audio_backend::live::{RulePhase, RuleUnit, TriggerRule};
    use dawj_core::{Intent, PartAction, RuleAction};
    let (mut engine, control, mixer) = demo::prepare().unwrap();
    let mut co = Coordinator::new(control, mixer, false);
    co.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    co.try_act(Intent::Part {
        deck: 0,
        slot: 7,
        action: PartAction::Launch,
    })
    .unwrap();
    for slot in 0..7 {
        co.try_act(Intent::Rule {
            deck: 0,
            slot,
            action: RuleAction::Install(TriggerRule::Every {
                n: 1,
                unit: RuleUnit::Beats,
                phase: RulePhase::Global,
            }),
        })
        .unwrap();
    }
    engine.render(&mut [0.; 128], OutputRouting::stereo_master_only());
    co.poll();
    co.try_act(Intent::Quantize(Quantize::Phrase)).unwrap();
    co.try_act(Intent::Part {
        deck: 0,
        slot: 7,
        action: PartAction::Stop,
    })
    .unwrap();
    engine.render(&mut [0.; 128], OutputRouting::stereo_master_only());
    co.poll();
    let mut p = project();
    p.compositions[0].structure.bars = 64;
    let w = Workspace {
        project: p,
        ..Default::default()
    };
    let positions = viz::positions(&w, &co).unwrap();
    assert!(
        positions[0]
            .events
            .iter()
            .any(|e| e.slot == 7 && e.label.contains('S'))
    );
    assert!(positions[0].events.len() <= 56);
    let mut observed = co.snapshot.unwrap();
    observed.decks[1].epoch = BeatTicks(2 * viz::BAR);
    co.snapshot = Some(observed);
    let positions = viz::positions(&w, &co).unwrap();
    assert_eq!(positions[0].tick, observed.beat.0.rem_euclid(64 * viz::BAR));
    assert_eq!(
        positions[1].tick,
        (observed.beat.0 - 2 * viz::BAR).rem_euclid(8 * viz::BAR)
    );
}
