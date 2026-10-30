// SPDX-License-Identifier: GPL-3.0-or-later
fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tauri-app/web/contract.generated.mjs");
    let text = dawj_core::web_contract::generated();
    if std::env::args().any(|a| a == "--check") {
        assert_eq!(
            std::fs::read_to_string(path)
                .expect("Generate frontend contract first")
                .replace("\r\n", "\n"),
            text,
            "Frontend fixtures drifted; regenerate with the web_contract example"
        );
    } else {
        std::fs::write(path, text).expect("Write frontend contract");
    }
}
