// SPDX-License-Identifier: GPL-3.0-or-later
mod window;
fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let smoke = match args.as_slice() {
        [] => None,
        [arg] if arg == "--smoke-window" => Some(window::Smoke::new(false)),
        [arg] if arg == "--smoke-test" => Some(window::Smoke::new(true)),
        _ => {
            eprintln!(
                "Usage: dawj [--smoke-window | --smoke-test]\nSmoke modes capture the native window and close; --smoke-test starts real demo audio."
            );
            std::process::exit(2);
        }
    };
    let result = smoke.as_ref().map(|s| s.result.clone());
    eframe::run_native(
        "DAWJ | Performance",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([1280.0, 820.0])
                .with_min_inner_size([1100.0, 740.0]),
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(window::PerformanceWindow::new(cc, smoke)))),
    )?;
    if result.is_some_and(|s| !s.load(std::sync::atomic::Ordering::Relaxed)) {
        std::process::exit(1);
    }
    Ok(())
}
