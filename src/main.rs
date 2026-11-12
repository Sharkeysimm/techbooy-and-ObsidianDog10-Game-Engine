use eframe::egui;

fn main() -> eframe::Result {
    eframe::run_simple_native("Viewport", Default::default(), |ctx, _frame| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, painter) = ui.allocate_painter(
                ui.available_size(),
                egui::Sense::hover(),
            );

            painter.rect_filled(rect.rect, 0.0, egui::Color32::BLACK);

            let box_rect = egui::Rect::from_center_size(
                rect.rect.center(),
                egui::vec2(50.0, 50.0),
            );
            painter.rect_filled(box_rect, 0.0, egui::Color32::WHITE);
        });
    })
}
