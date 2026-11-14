mod assets;
mod content_browser;
mod drag;
mod editor;
mod game_object;
mod hierarchy;
mod inspector;
mod scene;
mod viewport;
mod physics;

use editor::Editor;

fn main() -> eframe::Result {
    let mut editor = Editor::new();

    eframe::run_simple_native("Engine", Default::default(), move |ctx, _frame| {
        editor.ui(ctx);
    })
}
