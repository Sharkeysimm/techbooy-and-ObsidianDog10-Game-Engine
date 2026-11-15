mod assets;
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
mod view;
mod viewport;

use editor::Editor;

fn main() -> eframe::Result {
    let mut editor = Editor::new();

    eframe::run_simple_native("Engine", Default::default(), move |ctx, _frame| {
        editor.ui(ctx);
    })
}
