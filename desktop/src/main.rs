//! Native Linux / macOS front end for the `fqapi_core` backend.
//!
//! The backend owns all protocol, crypto and caching work and exposes a local REST surface
//! (`/api/v1/...`). This binary links it as a library and drives it through
//! `fqapi_core::api::request`, so no HTTP server or Flutter runtime is involved. Short dramas
//! are handed to the system `mpv` together with the CENC key the backend resolves.

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("qieyue only supports Linux and macOS");

mod app;
mod platform;
mod views;
mod worker;

use std::sync::mpsc::channel;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel::<worker::Command>();
    let (msg_tx, msg_rx) = channel::<worker::Reply>();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(worker::run(cmd_rx, msg_tx));
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 860.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("茄阅"),
        ..Default::default()
    };

    eframe::run_native(
        "qieyue",
        options,
        Box::new(move |cc| {
            platform::install_fonts(&cc.egui_ctx);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app::App::new(cmd_tx, msg_rx)))
        }),
    )
}
