// SPDX-License-Identifier: GPL-3.0-or-later
//! Canonical repository Markdown embedded for offline installations. No copied help strings.
#[tauri::command]
pub fn help_documents() -> serde_json::Value {
    [
        ("index", include_str!("../../../docs/help/index.md")),
        (
            "getting-started",
            include_str!("../../../docs/help/getting-started.md"),
        ),
        ("interface", include_str!("../../../docs/help/interface.md")),
        ("sketch", include_str!("../../../docs/help/sketch.md")),
        ("perform", include_str!("../../../docs/help/perform.md")),
        ("samples", include_str!("../../../docs/help/samples.md")),
        ("midi", include_str!("../../../docs/help/midi.md")),
        ("settings", include_str!("../../../docs/help/settings.md")),
        (
            "troubleshooting",
            include_str!("../../../docs/help/troubleshooting.md"),
        ),
    ]
    .into_iter()
    .map(|(id, markdown)| serde_json::json!({"id":id,"markdown":markdown}))
    .collect()
}
