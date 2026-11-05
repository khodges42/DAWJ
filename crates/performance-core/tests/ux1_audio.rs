// SPDX-License-Identifier: GPL-3.0-or-later
//! Measured end-to-end control routing, using prepared media and the real renderer.
use audio_backend::routing::OutputRouting;
use dawj_core::{Coordinator, InputSource, Intent, PartAction, factory::Catalog, session::*};

fn measured(
    catalog: &Catalog,
    index: usize,
    gain: f32,
    cutoff: Option<f32>,
    audition: bool,
) -> f64 {
    let mut p = Project::default();
    let n = p.create("Measured sound".into(), 0).unwrap();
    let s = &mut p.composition_mut(n).unwrap().parts[2];
    s.factory = Some(catalog.entries[index].reference.clone());
    s.looping = true;
    let prepared = prepare_catalog(&p, None, [true; 2], true, catalog).unwrap();
    let (mut engine, control, mixer) = engine(&p).unwrap();
    let mut co = Coordinator::new(control, mixer, false);
    install(&mut co, &p, prepared, [true; 2]).unwrap();
    configure(&mut co, &p.settings).unwrap();
    for _ in 0..8 {
        co.flush_parameters();
        engine.render(&mut [0.; 512], OutputRouting::stereo_master_only());
        co.poll();
    }
    co.dispatch(Intent::SelectPart { deck: 0, slot: 2 }, InputSource::Gui)
        .unwrap();
    for action in [PartAction::Gain(gain), PartAction::Cutoff(cutoff)] {
        co.dispatch(
            Intent::Part {
                deck: 0,
                slot: 2,
                action,
            },
            InputSource::Gui,
        )
        .unwrap();
    }
    co.try_act(Intent::Quantize(audio_backend::live::Quantize::Immediate))
        .unwrap();
    if audition {
        co.preview(co.banks[0][2]).unwrap();
    } else {
        co.try_act(Intent::Launch).unwrap();
    }
    let mut squares = 0.;
    let mut count = 0;
    for block in 0..750 {
        co.flush_parameters();
        let mut out = [0.; 512];
        engine.render(&mut out, OutputRouting::stereo_master_only());
        co.poll();
        if block > 50 {
            for v in out {
                squares += f64::from(v).powi(2);
                count += 1;
            }
        }
    }
    let state = co.snapshot.unwrap();
    let actual = state.decks[0].slots[2].parameters;
    assert!((actual.gain - gain).abs() < 0.0001);
    assert_eq!(actual.cutoff_hz, cutoff);
    assert_eq!(state.decks[1].slots[2].parameters.gain, 1.);
    (squares / count as f64).sqrt()
}

#[test]
#[ignore = "Requires actual local factory samples; native-content diagnosis"]
fn assigned_sound_gain_filter_and_audition_have_measured_distinct_paths() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/samples");
    let catalog = Catalog::discover(&root).unwrap();
    println!(
        "UX1_LIBRARY packs={} samples={} available={} unavailable={}",
        catalog.packs.len(),
        catalog.entries.len(),
        catalog.entries.iter().filter(|e| e.error.is_none()).count(),
        catalog.entries.iter().filter(|e| e.error.is_some()).count()
    );
    let index = catalog
        .entries
        .iter()
        .position(|e| e.filename.contains("Garage_Loop") && e.error.is_none())
        .expect("Garage loop");
    let full = measured(&catalog, index, 1., None, false);
    let quiet = measured(&catalog, index, 0.2, None, false);
    let filtered = measured(&catalog, index, 1., Some(80.), false);
    let muted = measured(&catalog, index, 0., None, false);
    let listen = measured(&catalog, index, 1., None, true);
    let listen_shaped = measured(&catalog, index, 0., Some(20.), true);
    println!(
        "UX1_AUDIO full={full:.7} gain_0.2={quiet:.7} lowpass_80={filtered:.7} mute={muted:.7} audition={listen:.7} audition_with_part_muted={listen_shaped:.7}"
    );
    assert!(full > 0.00001);
    assert!(quiet / full < 0.4);
    assert!(filtered / full < 0.8);
    assert!(muted < 1e-8);
    assert!(
        (listen - listen_shaped).abs() < 1e-8,
        "Audition is explicitly separate from part shaping"
    );
}

#[test]
#[ignore = "Requires actual local factory media; run with --include-ignored"]
fn stopped_slot_install_keeps_same_deck_pattern_running_and_rechecks_late_launches() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/samples");
    let catalog = Catalog::discover(&root).unwrap();
    let entry = catalog
        .entries
        .iter()
        .find(|e| e.filename.contains("A01_808_Kick") && e.error.is_none())
        .unwrap();
    let mut p = Project::default();
    let n = p.create("Live authoring".into(), 0).unwrap();
    let first = &mut p.composition_mut(n).unwrap().parts[0];
    first.factory = Some(entry.reference.clone());
    first.looping = false;
    let mut pattern = parts::pattern::Pattern::default();
    pattern.steps[0][0] = 127;
    first.pattern = Some(pattern);
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
    co.try_act(Intent::Quantize(audio_backend::live::Quantize::Immediate))
        .unwrap();
    co.dispatch(
        Intent::Part {
            deck: 0,
            slot: 0,
            action: PartAction::Launch,
        },
        InputSource::Gui,
    )
    .unwrap();
    engine.render(&mut [0.; 512], OutputRouting::stereo_master_only());
    co.poll();
    let playing = co.snapshot.unwrap().decks[0].slots[0];
    let mut next = p.clone();
    next.composition_mut(n).unwrap().parts[1].factory = Some(entry.reference.clone());
    next.composition_mut(n).unwrap().parts[1].looping = false;
    let prepared = prepare_catalog(&next, Some(&p), [true, false], true, &catalog).unwrap();
    install(&mut co, &next, prepared, [true, false]).unwrap();
    engine.render(&mut [0.; 512], OutputRouting::stereo_master_only());
    co.poll();
    let after = co.snapshot.unwrap().decks[0].slots[0];
    assert_eq!(after.active, playing.active);
    assert_eq!(after.pattern.revision, playing.pattern.revision);
    assert!(after.pattern.running);
    // Preparation alone does not reserve execution. A subsequent launch must reject commit.
    let mut candidate = next.clone();
    candidate.composition_mut(n).unwrap().parts[1].sample_start = 1;
    let prepared = prepare_catalog(&candidate, Some(&next), [true, false], true, &catalog).unwrap();
    co.dispatch(
        Intent::Part {
            deck: 0,
            slot: 1,
            action: PartAction::Launch,
        },
        InputSource::Gui,
    )
    .unwrap();
    engine.render(&mut [0.; 512], OutputRouting::stereo_master_only());
    co.poll();
    let bank = co.banks;
    assert!(
        install(&mut co, &candidate, prepared, [true, false])
            .unwrap_err()
            .contains("Destination part")
    );
    assert_eq!(co.banks, bank);
}
#[test]
#[ignore = "Full factory library decode audit; run explicitly outside native audio trials"]
fn every_bundled_factory_wave_decodes_and_prepares_without_modifying_its_source() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/samples");
    let catalog = Catalog::discover(&root).unwrap();
    assert!(!catalog.entries.is_empty());
    let mut failures = Vec::new();
    for entry in &catalog.entries {
        let mut p = Project::default();
        let n = p.create("Asset check".into(), 0).unwrap();
        let part = &mut p.composition_mut(n).unwrap().parts[0];
        part.factory = Some(entry.reference.clone());
        part.looping = entry.hints.kind == "Loop";
        match prepare_catalog(&p, None, [true, false], true, &catalog) {
            Ok(prepared) => assert_eq!(prepared.voices.len(), 1),
            Err(error) => failures.push(format!("{}: {error}", entry.reference.key())),
        }
    }
    assert!(
        failures.is_empty(),
        "Factory decode failures: {failures:#?}"
    );
    println!(
        "UX1_FACTORY_DECODE {} factory sounds prepared successfully",
        catalog.entries.len()
    );
}
