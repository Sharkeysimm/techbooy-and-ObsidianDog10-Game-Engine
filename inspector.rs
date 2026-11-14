use eframe::egui;

use crate::scene::Scene;
use crate::game_object::{ColliderShape, PhysicsBody, PhysicsConfig};
use crate::physics::PhysicsSystem;

pub fn show(ctx: &egui::Context, scene: &mut Scene, physics: &mut PhysicsSystem) {
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
            let x_changed = ui.add(egui::DragValue::new(&mut object.x).prefix("X: ")).changed();
            let y_changed = ui.add(egui::DragValue::new(&mut object.y).prefix("Y: ")).changed();
            ui.add(egui::DragValue::new(&mut object.size).prefix("Size: "));

            if x_changed || y_changed {
                physics.teleport(id, object.x, object.y);
            }

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

        ui.separator();
        ui.label("Physics");

        let mut physics_enabled = object.physics.is_some();
        if ui.checkbox(&mut physics_enabled, "Enabled").changed() {
            object.physics = if physics_enabled {
                Some(PhysicsConfig {
                    body: PhysicsBody::Dynamic,
                    collider: ColliderShape::Box {
                        half_width: object.size / 2.0,
                        half_height: object.size / 2.0,
                    },
                })
            } else {
                None
            };
        }

        if let Some(config) = object.physics.as_mut() {
            egui::ComboBox::from_label("Body")
                .selected_text(format!("{:?}", config.body))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut config.body, PhysicsBody::Dynamic, "Dynamic");
                    ui.selectable_value(&mut config.body, PhysicsBody::Fixed, "Fixed");
                    ui.selectable_value(&mut config.body, PhysicsBody::Kinematic, "Kinematic");
                });
        }
        });
}
