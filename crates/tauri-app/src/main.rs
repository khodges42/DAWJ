// SPDX-License-Identifier: GPL-3.0-or-later
//! Thin adapter. CPAL, application control and MIDI belong to dawj-core's worker.
use dawj_core::{
    ipc::{ErrorCode, Fault, Reply, Request},
    service::{Client, Service},
};
#[tauri::command]
async fn request(client: tauri::State<'_, Client>, request: Request) -> Result<Reply, String> {
    let id = request.id;
    let pending = match client.submit(request) {
        Ok(p) => p,
        Err(e) => return Ok(Reply::new(id, Err(e))),
    };
    Ok(tauri::async_runtime::spawn_blocking(move || pending.wait())
        .await
        .unwrap_or_else(|e| Reply::new(id, Err(Fault::new(ErrorCode::Closed, e.to_string())))))
}
#[tauri::command]
fn smoke_enabled() -> bool {
    std::env::args().any(|a| a == "--smoke")
}
#[tauri::command]
fn report(app: tauri::AppHandle, metrics: serde_json::Value) -> Result<(), String> {
    if !smoke_enabled() {
        return Err("Smoke reporting is disabled".into());
    }
    eprintln!("TAURI_METRICS {metrics}");
    app.exit(if metrics.get("error").is_some() { 1 } else { 0 });
    Ok(())
}
fn main() {
    let mut service = Service::spawn().expect("start application control worker");
    tauri::Builder::default()
        .manage(service.client())
        .invoke_handler(tauri::generate_handler![request, smoke_enabled, report])
        .build(tauri::generate_context!())
        .expect("Tauri runtime")
        .run(move |_, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                service.shutdown();
            }
        });
}
