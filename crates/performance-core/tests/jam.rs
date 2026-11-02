// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{live::Quantize, routing::OutputRouting, samples::LoadedSample};
use dawj_core::{Coordinator, Intent, PartAction, session::*};
use parts::AssetId;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "dawj-jam1-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        assert!(self.0.is_absolute() && self.0.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn wav(path: &Path, rate: u32, channels: u16, frequency: f64) {
    let frames = rate / 10;
    let bytes = frames * u32::from(channels) * 2;
    let mut b = b"RIFF".to_vec();
    b.extend((36 + bytes).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(channels.to_le_bytes());
    b.extend(rate.to_le_bytes());
    b.extend((rate * u32::from(channels) * 2).to_le_bytes());
    b.extend((channels * 2).to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(bytes.to_le_bytes());
    for f in 0..frames {
        let value =
            ((f as f64 / rate as f64 * frequency * std::f64::consts::TAU).sin() * 8000.0) as i16;
        for _ in 0..channels {
            b.extend(value.to_le_bytes());
        }
    }
    std::fs::write(path, b).unwrap();
}
fn project(files: &Files) -> Project {
    let mut p = Project::default();
    for d in 0..2 {
        let n = p
            .create(format!("Original composition {}", d + 1), d)
            .unwrap();
        let path = files.path(&format!("original-{}.wav", d + 1));
        wav(
            &path,
            if d == 0 { 44100 } else { 48000 },
            if d == 0 { 1 } else { 2 },
            220.0 + 110.0 * d as f64,
        );
        p.composition_mut(n).unwrap().parts[0].media = Some(path.display().to_string());
    }
    p
}
fn setup(p: &Project) -> (audio_backend::live::LiveEngine, Coordinator) {
    let prepared = prepare(p, [true; 2], true).unwrap();
    let (mut engine, c, m) = engine(p).unwrap();
    let mut co = Coordinator::new(c, m, false);
    install(&mut co, p, prepared, [true; 2]).unwrap();
    configure(&mut co, &p.settings).unwrap();
    for _ in 0..4 {
        co.flush_parameters();
        engine.render(&mut [0.0; 512], OutputRouting::stereo_master_only());
        co.poll();
    }
    (engine, co)
}
#[test]
fn original_two_deck_jam_roundtrips_and_reopens_stopped() {
    let files = Files::new();
    let mut p = project(&files);
    p.settings.crossfader = 0.37;
    p.settings.phrase_bars = 4;
    p.settings.bpm = 132.0;
    let (mut engine, mut co) = setup(&p);
    co.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    for d in 0..2 {
        co.try_act(Intent::Part {
            deck: d,
            slot: 0,
            action: PartAction::Launch,
        })
        .unwrap();
    }
    let mut output = [0.0; 2048];
    engine.render(&mut output, OutputRouting::stereo_master_only());
    co.poll();
    assert!(output.iter().any(|v| v.abs() > 0.0001));
    for d in 0..2 {
        assert_eq!(
            co.snapshot.unwrap().decks[d].slots[0].active.unwrap().0,
            p.composition(p.decks[d].unwrap()).unwrap().parts[0].id
        );
    }
    co.try_act(Intent::Crossfader(0.73)).unwrap();
    co.try_act(Intent::Part {
        deck: 0,
        slot: 0,
        action: PartAction::Gain(0.42),
    })
    .unwrap();
    co.try_act(Intent::Part {
        deck: 1,
        slot: 0,
        action: PartAction::Cutoff(Some(880.0)),
    })
    .unwrap();
    engine.render(&mut output, OutputRouting::stereo_master_only());
    co.poll();
    co.try_act(Intent::Part {
        deck: 0,
        slot: 0,
        action: PartAction::Stop,
    })
    .unwrap();
    engine.render(&mut output, OutputRouting::stereo_master_only());
    co.poll();
    assert!(co.snapshot.unwrap().decks[0].slots[0].active.is_none());
    assert!(co.snapshot.unwrap().decks[1].slots[0].active.is_some());
    let mut workspace = Workspace {
        project: p,
        ..Default::default()
    };
    workspace.capture(&co);
    let path = files.path("original-session.dawj.json");
    workspace.project.save(&path).unwrap();
    workspace.project.save(&path).unwrap();
    let disk = std::fs::read_to_string(&path).unwrap();
    assert!(!disk.contains(&files.0.display().to_string().replace('\\', "\\\\")));
    let loaded = Project::load(&path).unwrap();
    assert_eq!(
        serde_json::to_value(&workspace.project).unwrap(),
        serde_json::to_value(&loaded).unwrap()
    );
    drop(co);
    drop(engine);
    let (_, restored) = setup(&loaded);
    assert!(
        restored
            .snapshot
            .unwrap()
            .decks
            .iter()
            .flat_map(|d| d.slots)
            .all(|s| s.active.is_none() && s.pending.is_none())
    );
    assert!((restored.parameters.crossfader - 0.73).abs() < 0.0001);
    assert!((restored.parameter_target(0, 0).unwrap().gain - 0.42).abs() < 0.001);
    assert_eq!(
        restored.parameter_target(1, 0).unwrap().cutoff_hz,
        Some(880.0)
    );
}
#[test]
fn destination_replacement_preserves_other_deck_clock_audio_and_identity() {
    let files = Files::new();
    let mut p = project(&files);
    let (mut engine, mut co) = setup(&p);
    co.try_act(Intent::Quantize(Quantize::Immediate)).unwrap();
    co.try_act(Intent::Part {
        deck: 1,
        slot: 0,
        action: PartAction::Launch,
    })
    .unwrap();
    assert!(
        !co.deck_editable(1),
        "Unpublished launch receipt must block edits"
    );
    engine.render(&mut [0.0; 1024], OutputRouting::stereo_master_only());
    co.poll();
    assert!(co.deck_editable(0));
    assert!(!co.deck_editable(1));
    let before = co.snapshot.unwrap();
    p.composition_mut(p.decks[0].unwrap()).unwrap().parts[0].loop_start = 100;
    install(
        &mut co,
        &p,
        prepare(&p, [true, false], true).unwrap(),
        [true, false],
    )
    .unwrap();
    engine.render(&mut [0.0; 1024], OutputRouting::stereo_master_only());
    co.poll();
    let after = co.snapshot.unwrap();
    assert!(after.frame.0 > before.frame.0);
    assert_eq!(
        before.decks[1].slots[0].active,
        after.decks[1].slots[0].active
    );
    assert!(after.decks[1].slots[0].position.unwrap() > before.decks[1].slots[0].position.unwrap());
}
#[test]
fn queued_user_launch_can_cancel_without_manufacturing_playback() {
    let files = Files::new();
    let p = project(&files);
    let (mut e, mut c) = setup(&p);
    c.try_act(Intent::Quantize(Quantize::Bar)).unwrap();
    c.try_act(Intent::Launch).unwrap();
    assert!(!c.deck_editable(0));
    e.render(&mut [0.0; 128], OutputRouting::stereo_master_only());
    c.poll();
    assert!(c.snapshot.unwrap().decks[0].slots[0].pending.is_some());
    c.try_act(Intent::Cancel).unwrap();
    e.render(&mut [0.0; 128], OutputRouting::stereo_master_only());
    c.poll();
    assert!(c.deck_editable(0));
    assert!(c.snapshot.unwrap().decks[0].slots[0].active.is_none());
}
#[test]
fn wav_preparation_reports_source_metadata_and_conversion() {
    let files = Files::new();
    let path = files.path("recorded.wav");
    wav(&path, 44100, 1, 330.0);
    let (sample, info) = LoadedSample::prepare_file(AssetId(1), &path, 48000).unwrap();
    assert_eq!(
        (
            info.sample_rate,
            info.channels,
            info.frames,
            info.prepared_frames
        ),
        (44100, 1, 4410, 4800)
    );
    assert_eq!(sample.frames(), 4800);
    std::fs::write(&path, b"not WAV or MP3").unwrap();
    assert!(LoadedSample::prepare_file(AssetId(1), &path, 48000).is_err());
    assert!(LoadedSample::prepare_file(AssetId(1), &files.path("missing.wav"), 48000).is_err());
}
#[test]
fn validation_atomic_save_as_and_missing_media_diagnostics() {
    let files = Files::new();
    let p = project(&files);
    let path = files.path("jam.json");
    p.save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let mut invalid = p.clone();
    invalid.settings.crossfader = 2.0;
    assert!(invalid.save(&path).is_err());
    assert_eq!(original, std::fs::read(&path).unwrap());
    invalid = p.clone();
    invalid.version += 1;
    assert!(invalid.validate().is_err());
    invalid = p.clone();
    invalid.compositions[1].parts[0].id = invalid.compositions[0].parts[0].id;
    assert!(invalid.validate().is_err());
    invalid = p.clone();
    invalid.compositions[0].parts[0].loop_end = Some(0);
    assert!(invalid.validate().is_err());
    std::fs::create_dir(files.path("subfolder")).unwrap();
    let new = files.path("subfolder/copy.json");
    p.save(&new).unwrap();
    assert_eq!(
        serde_json::to_value(&p).unwrap(),
        serde_json::to_value(Project::load(&new).unwrap()).unwrap()
    );
    std::fs::remove_file(files.path("original-1.wav")).unwrap();
    let loaded = Project::load(&path).unwrap();
    assert!(prepare(&loaded, [true; 2], true).is_err());
    let ready = prepare(&loaded, [true; 2], false).unwrap();
    assert_eq!(ready.voices.len(), 1);
    assert_eq!(ready.media.iter().filter(|m| m.error.is_some()).count(), 1);
    assert!(
        files.0.read_dir().unwrap().all(|e| e
            .unwrap()
            .path()
            .extension()
            .is_none_or(|s| s != "tmp"))
    );
}

#[test]
fn same_composition_on_both_decks_retains_independent_parameters_and_saved_rules() {
    let files = Files::new();
    let mut p = project(&files);
    p.decks[1] = p.decks[0];
    let (mut e, mut c) = setup(&p);
    for (d, gain) in [(0, 0.22), (1, 0.77)] {
        c.try_act(Intent::Part {
            deck: d,
            slot: 0,
            action: PartAction::Gain(gain),
        })
        .unwrap();
        c.try_act(Intent::Rule {
            deck: d,
            slot: 0,
            action: dawj_core::RuleAction::Install(audio_backend::live::TriggerRule::Every {
                n: d as u16 + 1,
                unit: audio_backend::live::RuleUnit::Bars,
                phase: audio_backend::live::RulePhase::Deck,
            }),
        })
        .unwrap();
    }
    for _ in 0..4 {
        c.flush_parameters();
        e.render(&mut [0.0; 512], OutputRouting::stereo_master_only());
        c.poll();
    }
    let mut w = Workspace {
        project: p,
        ..Default::default()
    };
    w.capture(&c);
    let path = files.path("linked.json");
    w.project.save(&path).unwrap();
    let reopened = Project::load(&path).unwrap();
    let (_, restored) = setup(&reopened);
    assert!((restored.parameter_target(0, 0).unwrap().gain - 0.22).abs() < 0.001);
    assert!((restored.parameter_target(1, 0).unwrap().gain - 0.77).abs() < 0.001);
    for d in 0..2 {
        assert!(reopened.settings.part_targets[d][0].unwrap().rule.is_some());
        assert!(
            restored.snapshot.unwrap().decks[d].slots[0].rule.is_none(),
            "Saved rules must be explicitly armed"
        );
    }
    let mut again = Workspace {
        project: reopened,
        ..Default::default()
    };
    again.capture(&restored);
    assert!(
        again.project.settings.part_targets[0][0]
            .unwrap()
            .rule
            .is_some()
    );
}
#[test]
fn metadata_edits_do_not_decode_or_consume_another_resident_version() {
    let files = Files::new();
    let p = project(&files);
    let mut changed = p.clone();
    changed.compositions[0].name = "Renamed".into();
    changed.compositions[0].parts[0].gain = 0.5;
    std::fs::remove_file(files.path("original-1.wav")).unwrap();
    let prepared = prepare_changes(&changed, &p, [true, false], true).unwrap();
    assert!(prepared.voices.is_empty());
    assert!(!prepared.changed[0][0]);
}

#[test]
fn dirty_detection_tracks_midi_targets_but_not_selection_or_quantization_roundoff() {
    let files = Files::new();
    let p = project(&files);
    let (mut e, mut c) = setup(&p);
    let mut w = Workspace {
        project: p,
        ..Default::default()
    };
    w.capture(&c);
    assert!(!w.changed(&c));
    c.dispatch(
        Intent::SelectPart { deck: 1, slot: 7 },
        dawj_core::InputSource::Midi,
    )
    .unwrap();
    assert!(!w.changed(&c));
    c.dispatch(
        Intent::Part {
            deck: 0,
            slot: 0,
            action: PartAction::Gain(0.53123),
        },
        dawj_core::InputSource::Midi,
    )
    .unwrap();
    assert!(w.changed(&c));
    w.capture(&c);
    assert!(!w.changed(&c));
    for _ in 0..3 {
        c.flush_parameters();
        e.render(&mut [0.0; 512], OutputRouting::stereo_master_only());
        c.poll();
    }
    assert!(!w.changed(&c));
}
