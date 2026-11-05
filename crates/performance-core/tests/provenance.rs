// SPDX-License-Identifier: GPL-3.0-or-later
use dawj_core::{
    factory::{Catalog, Reference},
    session::Project,
};
#[test]
fn source_identity_survives_browser_overrides_folder_moves_and_session_roundtrip() {
    let root = std::env::temp_dir().join(format!("dawj-provenance-{}", std::process::id()));
    let pack = root.join("old-folder");
    std::fs::create_dir_all(pack.join("Drums")).unwrap();
    std::fs::write(
        pack.join("Drums/Kick.wav"),
        b"source for identity, invalid audio deliberately",
    )
    .unwrap();
    std::fs::write(
        pack.join("SOURCE.md"),
        "# Original record\nCreator Alice\nLicense CC-BY-4.0",
    )
    .unwrap();
    let mut manifest = serde_json::json!({"version":2,"id":"immutable-pack","name":"Original title","license":"CC0-1.0","sources":{"SOURCE.md":{"id":"source-original","title":"Original record","document":"SOURCE.md","creator":"Alice","license":"CC-BY-4.0"}},"samples":{"Drums/Kick.wav":{"id":"sample-original","source":"SOURCE.md","original_path":"original808/Kick.wav","license":"CC-BY-3.0","attribution":"Credit Alice"}}});
    std::fs::write(
        pack.join("pack.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let c = Catalog::discover(&root).unwrap();
    let r = c.entries[0].reference.clone();
    assert_eq!(r.sample_id.as_deref(), Some("sample-original"));
    let mut p = Project::default();
    let n = p.create("My composition".into(), 0).unwrap();
    p.composition_mut(n).unwrap().parts[0].factory = Some(r.clone());
    p.sample_overrides.insert(
        r.key(),
        dawj_core::factory::hints("My_display_name.wav", "Virtual/Collection"),
    );
    let p: Project = serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
    p.validate().unwrap();
    let moved = root.join("new-folder");
    std::fs::rename(pack, &moved).unwrap();
    std::fs::create_dir_all(moved.join("Virtual")).unwrap();
    std::fs::rename(
        moved.join("Drums/Kick.wav"),
        moved.join("Virtual/Named.wav"),
    )
    .unwrap();
    let record = manifest["samples"]
        .as_object_mut()
        .unwrap()
        .remove("Drums/Kick.wav")
        .unwrap();
    manifest["samples"]["Virtual/Named.wav"] = record;
    manifest["name"] = "New display title".into();
    std::fs::write(
        moved.join("pack.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let c = Catalog::discover(&root).unwrap();
    let saved = p.compositions[0].parts[0].factory.as_ref().unwrap();
    assert_eq!(
        c.resolve(saved).unwrap(),
        moved.join("Virtual/Named.wav").canonicalize().unwrap()
    );
    let e = c.entry(saved).unwrap();
    let origin = e.origin.as_ref().unwrap();
    assert_eq!(origin.original_path, "original808/Kick.wav");
    assert_eq!(origin.license.as_deref(), Some("CC-BY-3.0"));
    assert_eq!(
        c.packs[0].sources["SOURCE.md"].creator.as_deref(),
        Some("Alice")
    );
    assert!(
        c.resolve(&Reference {
            pack: r.pack,
            path: "../outside".into(),
            sample_id: r.sample_id
        })
        .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}
