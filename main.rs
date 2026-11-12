use eframe::egui;

struct GameObject {
    name: String,
    x: f32,
    y: f32,
    size: f32,
    visible: bool,
}

fn main() -> eframe::Result {
let mut objects = vec![
    GameObject {
        name: "Square".into(),
        x: 0.0,
        y: 0.0,
        size: 50.0,
        visible: true,
    },
];

let mut selected = 0;

    eframe::run_simple_native("Engine", Default::default(), move |ctx, _frame| {
        // Hierarchy panel
        egui::SidePanel::left("hierarchy")
            .resizable(true)
            .default_width(180.0)
            .show(ctx, |ui| {
                ui.heading("Hierarchy");
                ui.separator();

                egui::CollapsingHeader::new("Scene")
                    .default_open(true)
                    .show(ui, |ui| {
                        for (index, object) in objects.iter().enumerate() {
                            if ui
                                .selectable_label(selected == index, &object.name)
                                .clicked()
                            {
                                selected = index;
                            }
                        }
                    });
            });

        egui::SidePanel::right("inspector")
            .resizable(false)
            if let Some(object) = objects.get_mut(selected) {
                ui.text_edit_singleline(&mut object.name);
                ui.checkbox(&mut object.visible, "Visible");

                ui.separator();
                ui.add(egui::DragValue::new(&mut object.x).prefix("X: "));
                ui.add(egui::DragValue::new(&mut object.y).prefix("Y: "));
                ui.add(egui::DragValue::new(&mut object.size).prefix("Size: "));
            }

        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, painter) = ui.allocate_painter(
                ui.available_size(),
                egui::Sense::hover(),
            );

            painter.rect_filled(rect.rect, 0.0, egui::Color32::BLACK);

            for object in &objects {
                if object.visible {
                    let pos = rect.rect.center() + egui::vec2(object.x, object.y);
                    let box_rect = egui::Rect::from_center_size(
                        pos,
                        egui::vec2(object.size, object.size),
                    );
                    painter.rect_filled(box_rect, 0.0, egui::Color32::WHITE);
                }
            }

        });
    })
}
