use eframe::egui;

use crate::scene::Scene;

pub fn show(ctx: &egui::Context, scene: &mut Scene) {
    let Some(id) = scene.selected else {
        egui::SidePanel::right("inspector")
            .resizable(false)
            .show(ctx, |_ui| {});
        return;
    };

    let parent_name = scene
        .get(id)
        .and_then(|o| o.parent)
        .and_then(|p| scene.get(p))
        .map(|p| p.name.clone());

    egui::SidePanel::right("inspector")
        .resizable(false)
        .show(ctx, |ui| {
            let Some(object) = scene.get_mut(id) else {
                return;
            };

            ui.text_edit_singleline(&mut object.name);
            ui.checkbox(&mut object.visible, "Visible");

            ui.separator();
            ui.add(egui::DragValue::new(&mut object.x).prefix("X: "));
            ui.add(egui::DragValue::new(&mut object.y).prefix("Y: "));
            ui.add(egui::DragValue::new(&mut object.size).prefix("Size: "));

            ui.separator();
            ui.label(format!(
                "Parent: {}",
                parent_name.as_deref().unwrap_or("None")
            ));

            if let Some(texture) = &object.texture {
                let name = texture.rsplit('/').next().unwrap_or(texture);
                ui.label(format!("Texture: {name}"));
                if ui.button("Clear texture").clicked() {
                    object.texture = None;
                }
            }
        });
}
