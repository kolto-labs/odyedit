//! Program entry point.
//!
//! Opens the window, with a file already loaded when one is named on the
//! command line.

#[cfg(feature = "gui")]
fn main() -> eframe::Result<()> {
    use odyedit::ui::{App, APP_NAME, VERSION};

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(format!("{APP_NAME} {VERSION}"))
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    let requested = std::env::args().nth(1);

    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |_cc| {
            let app = match requested {
                Some(path) => App::with_file(&path),
                None => App::new(),
            };
            Ok(Box::new(app))
        }),
    )
}

/// Built without a window, there is nothing to run.
#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("This build has no window. Rebuild with the \"gui\" feature.");
    std::process::exit(1);
}
