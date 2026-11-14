use eframe::egui::{self, Align, Align2, Layout, Sense};

use crate::drag::ObjectPayload;
use crate::game_object::{ColliderShape, GameObject, PhysicsBody, PhysicsConfig};
use crate::scene::Scene;

#[derive(Default)]
struct Actions {
    select: Option<u64>,
    reparent: Option<(u64, Option<u64>)>,
    delete: Option<u64>,
    attach_physics: Option<u64>,
    attach_collider: Option<u64>,
}

pub fn show(ctx: &egui::Context, scene: &mut Scene) {
    let mut actions = Actions::default();
    let mut add_square = false;

    egui::SidePanel::left("hierarchy")
        .resizable(true)
        .default_width(180.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Hierarchy");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button("+", |ui| {
                        if ui.button("Square").clicked() {
                            add_square = true;
                            ui.close_menu();
                        }
                    });
                });
            });
            ui.separator();

            let header = egui::CollapsingHeader::new("Scene")
                .default_open(true)
                .show(ui, |ui| {
                    for id in scene.children_of(None) {
                        draw_node(ui, scene, id, &mut actions);
                    }
                });

            // dropping on the scene header unparents
            if let Some(payload) = header.header_response.dnd_release_payload::<ObjectPayload>() {
                actions.reparent = Some((payload.id, None));
            }
        });

    if let Some(id) = actions.select {
        scene.selected = Some(id);
    }
    if let Some((id, parent)) = actions.reparent {
        scene.set_parent(id, parent);
    }
    if let Some(id) = actions.delete {
        scene.remove(id);
    }

    if let Some(id) = actions.attach_physics {
    if let Some(object) = scene.get_mut(id) {
        let half_size = object.size * 0.5;

        object.physics = Some(PhysicsConfig {
            body: PhysicsBody::Dynamic,
            collider: ColliderShape::Box {
                half_width: half_size,
                half_height: half_size,
            },
        });
    }
}

    if add_square {
        let id = scene.add(GameObject::square());
        scene.selected = Some(id);
    }
}

fn draw_node(ui: &mut egui::Ui, scene: &Scene, id: u64, actions: &mut Actions) {
    let Some(object) = scene.get(id) else {
        return;
    };
    let children = scene.children_of(Some(id));

    if children.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            row(ui, scene, object, actions);
        });
        return;
    }

    let state_id = ui.make_persistent_id(("hierarchy_node", id));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, true)
        .show_header(ui, |ui| row(ui, scene, object, actions))
        .body(|ui| {
            for child in children {
                draw_node(ui, scene, child, actions);
            }
        });
}

fn row(ui: &mut egui::Ui, scene: &Scene, object: &GameObject, actions: &mut Actions) {
    let selected = scene.selected == Some(object.id);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(60.0), 20.0),
        Sense::click_and_drag(),
    );

    let drop_target = response
        .dnd_hover_payload::<ObjectPayload>()
        .is_some_and(|p| p.id != object.id);

    let visuals = ui.visuals();
    if drop_target {
        ui.painter().rect_filled(
            rect,
            3.0,
            egui::Color32::from_rgba_unmultiplied(70, 130, 220, 110),
        );
    } else if selected {
        ui.painter().rect_filled(rect, 3.0, visuals.selection.bg_fill);
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 3.0, visuals.widgets.hovered.weak_bg_fill);
    }

    ui.painter().text(
        egui::pos2(rect.min.x + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        &object.name,
        egui::TextStyle::Body.resolve(ui.style()),
        ui.visuals().text_color(),
    );

    if response.clicked() {
        actions.select = Some(object.id);
    }

    response.dnd_set_drag_payload(ObjectPayload { id: object.id });

    if let Some(payload) = response.dnd_release_payload::<ObjectPayload>() {
        if payload.id != object.id {
            actions.reparent = Some((payload.id, Some(object.id)));
        }
    }

    response.context_menu(|ui| {
        ui.menu_button("Attach", |ui| {
            if ui.button("PhysicalBody2D").clicked() {
                actions.attach_physics = Some(object.id);
                ui.close_menu();
            }

            if ui.button("ColliderBox2D").clicked() {
                actions.attach_collider = Some(object.id);
                ui.close_menu();
            }
        });

        if object.parent.is_some() && ui.button("Unparent").clicked() {
            actions.reparent = Some((object.id, None));
            ui.close_menu();
        }
        if ui.button("Delete").clicked() {
            actions.delete = Some(object.id);
            ui.close_menu();
        }
    });
}
