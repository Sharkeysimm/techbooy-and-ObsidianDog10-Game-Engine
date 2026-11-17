mod assets;
mod collision;
mod content_browser;
mod drag;
mod editor;
mod game_object;
mod gizmo;
mod hierarchy;
mod inspector;
mod math;
mod physics;
mod scene;
mod script;
mod script_editor;
mod serde_util;
mod storage;
mod view;
mod viewport;

use editor::Editor;

fn main() -> eframe::Result {
    let mut editor = Editor::new();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_maximized(true),
        ..Default::default()
    };

    eframe::run_simple_native("Engine", options, move |ctx, _frame| {
        editor.ui(ctx);
    })
}
