// SPDX-License-Identifier: GPL-3.0-or-later
//! Thin adapter. CPAL, application control and MIDI belong to dawj-core's worker.
use dawj_core::{
    ipc::{Command, ErrorCode, Fault, Reply, Request, VERSION},
    service::{Client, Service},
};
use tauri::{
    Manager,
    menu::{Menu, MenuItem, Submenu},
};
const SKIN_RESET_SCRIPT: &str = include_str!("../web/skin-recovery.js");
fn restore_skin(app: &tauri::AppHandle) -> Result<(), String> {
    // Existing semantic release path; no new engine command or audio ownership.
    let client = app.state::<Client>();
    for source in [
        dawj_core::InputSource::Gui,
        dawj_core::InputSource::Keyboard,
        dawj_core::InputSource::Midi,
    ] {
        let _ = client.submit(Request {
            version: VERSION,
            id: u32::MAX,
            command: Command::ReleaseModes { source },
        });
    }
    app.get_webview_window("main")
        .ok_or("Main window unavailable")?
        .eval(SKIN_RESET_SCRIPT)
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn skin_startup() -> &'static str {
    let args: Vec<_> = std::env::args().collect();
    skin_startup_mode(args.iter().map(String::as_str))
}
fn skin_startup_mode<'a>(args: impl Iterator<Item = &'a str>) -> &'static str {
    let mut mode = "normal";
    for arg in args {
        match arg {
            "--reset-skin" => return "reset",
            "--safe-mode" | "--safe-skin" => mode = "safe",
            _ => {}
        }
    }
    mode
}
#[cfg(test)]
mod skin_tests {
    use super::skin_startup_mode;
    #[test]
    fn safe_launch_and_reset_priority_are_independent_of_persisted_ui() {
        assert_eq!(skin_startup_mode(["dawj"].into_iter()), "normal");
        assert_eq!(
            skin_startup_mode(["dawj", "--safe-mode"].into_iter()),
            "safe"
        );
        assert_eq!(
            skin_startup_mode(["dawj", "--safe-skin"].into_iter()),
            "safe"
        );
        for flags in [
            ["--safe-mode", "--reset-skin"],
            ["--reset-skin", "--safe-skin"],
        ] {
            assert_eq!(skin_startup_mode(flags.into_iter()), "reset");
        }
    }
}
#[tauri::command]
fn skin_test_reset(app: tauri::AppHandle) -> Result<(), String> {
    if !smoke_enabled() {
        return Err("Skin smoke helpers disabled".into());
    }
    restore_skin(&app)
}
#[tauri::command]
fn skin_test_window(app: tauri::AppHandle, width: u32, height: u32) -> Result<(), String> {
    if !smoke_enabled() || !(1024..=1920).contains(&width) || !(640..=1200).contains(&height) {
        return Err("Invalid smoke window size".into());
    }
    app.get_webview_window("main")
        .ok_or("Main window unavailable")?
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())
}
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
        .menu(|app| {
            let reset = MenuItem::with_id(
                app,
                "restore-default-skin",
                "Restore default interface",
                true,
                Some("Ctrl+Alt+Shift+R"),
            )?;
            let appearance = Submenu::with_items(app, "&Appearance", true, &[&reset])?;
            Menu::with_items(app, &[&appearance])
        })
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "restore-default-skin"
                && let Err(error) = restore_skin(app)
            {
                eprintln!("Skin recovery failed: {error}");
            }
        })
        .invoke_handler(tauri::generate_handler![
            request,
            smoke_enabled,
            report,
            skin_startup,
            skin_test_reset,
            skin_test_window
        ])
        .build(tauri::generate_context!())
        .expect("Tauri runtime")
        .run(move |_, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                service.shutdown();
            }
        });
}
