// SPDX-License-Identifier: GPL-3.0-or-later
mod window;
fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let smoke = match args.as_slice() {
        [] => None,
        [arg] if arg == "--reliability" => None,
        [arg] if arg == "--smoke-window" => Some(window::Smoke::new(false)),
        [arg] if arg == "--smoke-test" => Some(window::Smoke::new(true)),
        _ => {
            eprintln!(
                "Usage: dawj-egui [--smoke-window | --smoke-test | --reliability]\nSmoke modes capture and close. Audio tests require DAWJ_TEST_OUTPUT (default HyperX); reliability duration uses DAWJ_TEST_SECONDS (default 1800)."
            );
            std::process::exit(2);
        }
    };
    let result = smoke.as_ref().map(|s| s.result.clone());
    eframe::run_native(
        "DAWJ | egui fallback (Alpha)",
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
