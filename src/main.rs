mod app;
mod icon;
mod settings;
mod sticky;
mod tray;

use app::TimerApp;
use eframe::egui;

fn main() -> eframe::Result {
    let settings = settings::Settings::load();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("FloatingTimer")
        .with_app_id("floating-timer-rs")
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_taskbar(false)
        .with_always_on_top()
        .with_inner_size([340.0, 170.0])
        .with_icon(icon::egui_icon());
    if let Some([x, y]) = settings.pos {
        viewport = viewport.with_position([x, y]);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        centered: settings.pos.is_none(),
        ..Default::default()
    };

    eframe::run_native(
        "FloatingTimer",
        options,
        Box::new(move |cc| Ok(Box::new(TimerApp::new(cc, settings)))),
    )
}
