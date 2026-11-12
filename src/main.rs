use eframe::egui;

fn main() -> eframe::Result {
    let mut name = String::from("Square");
    let mut x = 0.0_f32;
    let mut y = 0.0_f32;
    let mut size = 50.0_f32;
    let mut visible = true;

    eframe::run_simple_native("Engine", Default::default(), move |ctx, _frame| {
        egui::SidePanel::right("inspector")
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Inspector");
                ui.separator();

                ui.text_edit_singleline(&mut name);
                ui.checkbox(&mut visible, "Visible");

                ui.separator();
                ui.add(egui::DragValue::new(&mut x).prefix("X: "));
                ui.add(egui::DragValue::new(&mut y).prefix("Y: "));
                ui.add(egui::DragValue::new(&mut size).prefix("Size: "));
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, painter) = ui.allocate_painter(
                ui.available_size(),
                egui::Sense::hover(),
            );

            painter.rect_filled(rect.rect, 0.0, egui::Color32::BLACK);

            if visible {
                let pos = rect.rect.center() + egui::vec2(x, y);
                let box_rect = egui::Rect::from_center_size(pos, egui::vec2(size, size));
                painter.rect_filled(box_rect, 0.0, egui::Color32::WHITE);
            }
        });
    })
}
