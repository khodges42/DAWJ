// SPDX-License-Identifier: GPL-3.0-or-later
//! Thin adapter. CPAL, application control and MIDI belong to dawj-core's worker.
use dawj_core::{
    ipc::{Command, ErrorCode, Fault, Reply, Request, VERSION},
    service::{Client, Service},
};
use tauri::{
    Emitter, Manager,
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
fn skin_test_zoom(app: tauri::AppHandle, zoom: f64) -> Result<(), String> {
    if !smoke_enabled() || ![1.0, 1.25, 1.5, 2.0].contains(&zoom) {
        return Err("Invalid smoke zoom".into());
    }
    app.get_webview_window("main")
        .ok_or("Main window unavailable")?
        .set_zoom(zoom)
        .map_err(|e| e.to_string())
}
static PICKER_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
struct PickerPermit;
impl Drop for PickerPermit {
    fn drop(&mut self) {
        PICKER_OPEN.store(false, std::sync::atomic::Ordering::Release);
    }
}
/// Native dialogs run on a blocking pool, never the WebView or audio callback.
#[tauri::command]
async fn pick_local_file(kind: String) -> Result<Option<String>, String> {
    PICKER_OPEN
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .map_err(|_| "A file picker is already open")?;
    let permit = PickerPermit;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let dialog = rfd::FileDialog::new();
        let path = match kind.as_str() {
            "audio" => dialog
                .set_title("Assign WAV sound")
                .add_filter("WAV audio", &["wav"])
                .pick_file(),
            "open" => dialog
                .set_title("Open DAWJ session")
                .add_filter("DAWJ session", &["json"])
                .pick_file(),
            "save" => dialog
                .set_title("Save DAWJ session")
                .add_filter("DAWJ session", &["json"])
                .set_file_name("Untitled.dawj.json")
                .save_file(),
            _ => return Err("Unknown file operation".to_string()),
        };
        Ok(path.map(|p| p.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
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
async fn reliability_request(
    client: tauri::State<'_, Client>,
    request: Request,
) -> Result<serde_json::Value, String> {
    if !smoke_enabled() {
        return Err("Diagnostic requests disabled".into());
    }
    let start = std::time::Instant::now();
    let pending = client.submit_profiled(request).map_err(|e| e.message)?;
    let (reply, timing) = tauri::async_runtime::spawn_blocking(move || pending.wait_profiled())
        .await
        .map_err(|e| e.to_string())?;
    Ok(
        serde_json::json!({"reply":reply,"worker":timing,"native_ms":start.elapsed().as_secs_f64()*1000.0}),
    )
}
#[tauri::command]
fn reliability_options() -> serde_json::Value {
    let seconds = if std::env::args().any(|a| a == "--reliability") {
        std::env::var("DAWJ_TEST_SECONDS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|n| (5..=3600).contains(n))
            .unwrap_or(1800)
    } else {
        0
    };
    serde_json::json!({"profile":std::env::var("DAWJ_TEST_PROFILE").is_ok_and(|s|s=="1"),"seconds":seconds,"output":std::env::var("DAWJ_TEST_OUTPUT").unwrap_or_else(|_| "HyperX".into())})
}
#[tauri::command]
fn jam_options() -> Result<Option<serde_json::Value>, String> {
    if !std::env::args().any(|a| a == "--jam-smoke") {
        return Ok(None);
    }
    let phase = std::env::var("DAWJ_JAM_PHASE").map_err(|e| e.to_string())?;
    if !["create", "reopen"].contains(&phase.as_str()) {
        return Err("Unknown Jam smoke phase".into());
    }
    let root = std::path::PathBuf::from(std::env::var("DAWJ_JAM_ROOT").map_err(|e| e.to_string())?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let target = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !root.starts_with(&target) || root == target {
        return Err("Jam smoke fixtures must be in a fresh target subdirectory".into());
    }
    Ok(Some(
        serde_json::json!({"phase":phase,"path":root.join("session.dawj.json").display().to_string(),"audio":[root.join("my-percussion.wav").display().to_string(),root.join("my-tone.wav").display().to_string()]}),
    ))
}
#[tauri::command]
fn smoke_enabled() -> bool {
    std::env::args().any(|a| a == "--smoke" || a == "--reliability" || a == "--jam-smoke")
}
#[tauri::command]
fn report(app: tauri::AppHandle, metrics: serde_json::Value) -> Result<(), String> {
    if !smoke_enabled() {
        return Err("Smoke reporting is disabled".into());
    }
    eprintln!("TAURI_METRICS {metrics}");
    if metrics.get("error").is_some() {
        app.exit(1);
    } else {
        // Use normal window destruction, rather than terminating from an IPC callback.
        app.get_webview_window("main")
            .ok_or("Main window unavailable")?
            .close()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn main() {
    let mut service = Service::spawn().expect("start application control worker");
    tauri::Builder::default()
        .manage(service.client())
        .on_window_event(|window,event|{
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop{paths,position,..})=event {
                // One selected slot per drop; no file I/O here.
                if paths.len()==1 {let _=window.emit("dawj-audio-drop",serde_json::json!({"path":paths[0].display().to_string(),"x":position.x,"y":position.y}));}
                else {let _=window.emit("dawj-drop-error","Drop one WAV sound at a time");}
            }
        })
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
            pick_local_file,
            jam_options,
            smoke_enabled,
            report,
            skin_startup,
            reliability_options,
            reliability_request,
            skin_test_reset,
            skin_test_zoom,
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
