use eframe::egui;

fn main() -> eframe::Result {
    eframe::run_simple_native("Window", Default::default(), |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("window");
        });
    })
}
