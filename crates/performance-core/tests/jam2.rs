// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::routing::OutputRouting;
use dawj_core::{
    Coordinator, InputSource, Intent,
    factory::{Catalog, hints},
    session::{Project, configure, engine, install, prepare_catalog},
};

#[test]
fn filename_hints_do_not_invent_pitch_octaves_or_synchronization() {
    let h = hints("AB12_Warm_Kick_C.wav", "Drums/Kicks");
    assert_eq!(h.identifier.as_deref(), Some("AB12"));
    assert_eq!(h.note.as_deref(), Some("C"));
    assert_eq!(h.display_name, "Warm Kick");
    assert_eq!(h.bpm, None);
    let h = hints("XY9_Drum_Loop_128_F#m.wav", "Loops");
    assert_eq!(h.bpm, Some(128.0));
    assert_eq!(h.key.as_deref(), Some("F#m"));
    assert_eq!(h.note, None);
    assert_eq!(hints("sound_94bpm.wav", "").bpm, Some(94.0));
    assert_eq!(hints("noise_17.wav", "").note, None);
}

#[test]
fn arbitrary_pack_directory_is_discovered_and_traversal_rejected() {
    let root = std::env::temp_dir().join(format!("dawj-pack-audit-{}", std::process::id()));
    let category = root.join("a-new-user-pack/Drums/Kicks");
    std::fs::create_dir_all(&category).unwrap();
    std::fs::write(category.join("Broken_C.wav"), b"not a wave").unwrap();
    let catalog = Catalog::discover(&root).unwrap();
    assert_eq!(catalog.packs[0].id, "a-new-user-pack");
    assert_eq!(catalog.entries[0].category, "Drums/Kicks");
    assert!(catalog.entries[0].error.is_some());
    assert!(
        dawj_core::factory::Reference {
            sample_id: None,
            pack: "a-new-user-pack".into(),
            path: "../outside.wav".into()
        }
        .validate()
        .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}

// Independent from local assets: this test runs in a fresh clone as well.
#[test]
fn factory_references_patterns_and_metadata_roundtrip_without_demo_identity() {
    let mut p = Project::default();
    for d in 0..2 {
        let id = p.create(format!("Original {d}"), d).unwrap();
        let s = &mut p.composition_mut(id).unwrap().parts[0];
        s.factory = Some(dawj_core::factory::Reference {
            sample_id: None,
            pack: format!("user-pack-{d}"),
            path: "Drums/Kick.wav".into(),
        });
        s.looping = false;
        let mut pattern = parts::pattern::Pattern::default();
        pattern.steps[0][d * 4] = 64 + d as u8 * 63;
        s.pattern = Some(pattern);
        s.color = Some("#73c5df".into());
    }
    p.validate().unwrap();
    let reopened: Project = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
    reopened.validate().unwrap();
    assert_ne!(reopened.decks[0], reopened.decks[1]);
    assert_ne!(
        reopened.compositions[0].parts[0].id,
        reopened.compositions[1].parts[0].id
    );
    for d in 0..2 {
        assert_eq!(
            reopened.compositions[d].parts[0].pattern,
            p.compositions[d].parts[0].pattern
        );
        assert_eq!(
            reopened.compositions[d].parts[0].factory,
            p.compositions[d].parts[0].factory
        );
    }
}

#[test]
#[ignore = "Requires locally supplied factory assets; run explicitly for native-content audit"]
fn actual_factory_samples_prepare_and_render_both_original_patterns() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/samples");
    let catalog = Catalog::discover(&root).unwrap();
    let entries: Vec<_> = catalog
        .entries
        .iter()
        .filter(|e| e.error.is_none() && e.hints.kind != "Loop")
        .take(2)
        .collect();
    assert_eq!(entries.len(), 2, "Need two actual local factory WAV assets");
    let mut p = Project::default();
    for (d, entry) in entries.iter().enumerate() {
        let id = p.create(format!("Factory original {d}"), d).unwrap();
        let s = &mut p.composition_mut(id).unwrap().parts[0];
        s.factory = Some(entry.reference.clone());
        s.looping = false;
        let mut pattern = parts::pattern::Pattern::default();
        pattern.steps[0][0] = 127;
        pattern.steps[0][4 + d] = 64;
        s.pattern = Some(pattern);
    }
    let prepared = prepare_catalog(&p, None, [true; 2], true, &catalog).unwrap();
    let (mut engine, control, mixer) = engine(&p).unwrap();
    let mut co = Coordinator::new(control, mixer, false);
    install(&mut co, &p, prepared, [true; 2]).unwrap();
    configure(&mut co, &p.settings).unwrap();
    for _ in 0..8 {
        co.flush_parameters();
        engine.render(&mut [0.; 512], OutputRouting::stereo_master_only());
        co.poll();
    }
    for d in 0..2 {
        co.dispatch(Intent::SelectPart { deck: d, slot: 0 }, InputSource::Gui)
            .unwrap();
        co.dispatch(Intent::Launch, InputSource::Gui).unwrap();
    }
    let mut nonzero = false;
    for _ in 0..900 {
        let mut out = [0.; 512];
        engine.render(&mut out, OutputRouting::stereo_master_only());
        nonzero |= out.iter().any(|s| s.abs() > 0.00001);
        co.poll();
    }
    assert!(nonzero);
    let state = co.snapshot.unwrap();
    assert!(
        state
            .decks
            .iter()
            .all(|d| d.slots[0].pattern.running && d.slots[0].pattern.triggers > 2)
    );
}

#[test]
fn stored_variations_roundtrip_and_recall_the_actual_pattern() {
    let mut p = Project::default();
    let id = p.create("Original variations".into(), 0).unwrap();
    let s = &mut p.composition_mut(id).unwrap().parts[0];
    s.looping = false;
    let mut pattern = parts::pattern::Pattern::default();
    pattern.steps[0][0] = 127;
    s.pattern = Some(pattern);
    s.variation(0, true).unwrap();
    pattern.steps[0][4] = 64;
    s.pattern = Some(pattern);
    s.variation(1, true).unwrap();
    s.variation(0, false).unwrap();
    assert_eq!(s.pattern.unwrap().steps[0][4], 0);
    assert!(s.variation(4, false).is_err());
    assert!(s.variation(3, false).is_err());
    let mut restored: Project = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
    restored.validate().unwrap();
    let s = &mut restored.composition_mut(id).unwrap().parts[0];
    s.variation(1, false).unwrap();
    assert_eq!(s.pattern.unwrap().steps[0][4], 64);
    assert_eq!(s.variation, Some(1));
}
