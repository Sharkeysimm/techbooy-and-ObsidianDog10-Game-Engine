use eframe::egui::{self, Align, Align2, Layout, Sense};

use std::path::PathBuf;

use crate::assets::{is_prefab, is_vscript};
use crate::drag::{AssetPayload, ObjectPayload};
use crate::game_object::{GameObject, Shape};
use crate::scene::Scene;
use crate::storage;

#[derive(Clone, Copy)]
enum CreateKind {
    Empty,
    Camera,
    Shape(Shape),
}

struct RenameState {
    id: u64,
    buffer: String,
    focused: bool,
}

#[derive(Default)]
pub struct HierarchyState {
    renaming: Option<RenameState>,
}

#[derive(Default)]
struct Actions {
    select: Option<u64>,
    reparent: Option<(u64, Option<u64>)>,
    delete: Option<u64>,
    create: Option<(CreateKind, Option<u64>)>,
    rename: Option<(u64, String)>,
    instantiate: Option<(PathBuf, Option<u64>)>,
    attach_script: Option<(PathBuf, u64)>,
}

pub fn show(ctx: &egui::Context, scene: &mut Scene, state: &mut HierarchyState) {
    let mut actions = Actions::default();

    egui::SidePanel::left("hierarchy")
        .resizable(true)
        .default_width(190.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Hierarchy");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button("+", |ui| create_items(ui, None, &mut actions));
                });
            });
            ui.separator();

            // added before the rows so rows sit on top of it
            let background = ui.interact(
                ui.available_rect_before_wrap(),
                ui.id().with("hierarchy_background"),
                Sense::click(),
            );
            background.context_menu(|ui| {
                ui.menu_button("Create", |ui| create_items(ui, None, &mut actions));
            });

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .drag_to_scroll(false)
                .show(ui, |ui| {
                    let header = egui::CollapsingHeader::new("Scene")
                        .default_open(true)
                        .show(ui, |ui| {
                            for id in scene.children_of(None) {
                                draw_node(ui, scene, id, &mut actions, state);
                            }
                        });

                    // dropping on the scene header unparents
                    if let Some(payload) =
                        header.header_response.dnd_release_payload::<ObjectPayload>()
                    {
                        actions.reparent = Some((payload.id, None));
                    }
                    if let Some(payload) =
                        header.header_response.dnd_release_payload::<AssetPayload>()
                    {
                        if is_prefab(&payload.path) {
                            actions.instantiate = Some((payload.path.clone(), None));
                        }
                    }
                });
        });

    if let Some(id) = actions.select {
        scene.selected = Some(id);
    }
    if let Some((id, name)) = actions.rename {
        let name = name.trim().to_string();
        if !name.is_empty() {
            if let Some(object) = scene.get_mut(id) {
                object.name = name;
            }
        }
    }
    if let Some((id, parent)) = actions.reparent {
        scene.set_parent(id, parent);
    }
    if let Some(id) = actions.delete {
        scene.remove(id);
    }
    if let Some((path, id)) = actions.attach_script {
        if let Some(graph) = storage::load_script(&path) {
            if let Some(object) = scene.get_mut(id) {
                object.script = Some(graph);
            }
            scene.selected = Some(id);
        }
    }
    if let Some((path, parent)) = actions.instantiate {
        if let Some(prefab) = storage::load_prefab(&path) {
            scene.selected = storage::instantiate(scene, &prefab, parent, None);
        }
    }
    if let Some((kind, parent)) = actions.create {
        let mut object = match kind {
            CreateKind::Empty => GameObject::empty(),
            CreateKind::Camera => GameObject::camera(),
            CreateKind::Shape(shape) => GameObject::from_shape(shape),
        };
        object.parent = parent;

        let id = scene.add(object);
        scene.selected = Some(id);
    }
}

fn create_items(ui: &mut egui::Ui, parent: Option<u64>, actions: &mut Actions) {
    if ui.button("Empty").clicked() {
        actions.create = Some((CreateKind::Empty, parent));
        ui.close_menu();
    }
    if ui.button("Camera").clicked() {
        actions.create = Some((CreateKind::Camera, parent));
        ui.close_menu();
    }

    ui.menu_button("2D Shape", |ui| {
        for shape in Shape::ALL {
            if ui.button(format!("{shape:?}")).clicked() {
                actions.create = Some((CreateKind::Shape(shape), parent));
                ui.close_menu();
            }
        }
    });
}

fn draw_node(
    ui: &mut egui::Ui,
    scene: &Scene,
    id: u64,
    actions: &mut Actions,
    state: &mut HierarchyState,
) {
    let Some(object) = scene.get(id) else {
        return;
    };
    let children = scene.children_of(Some(id));

    if children.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            row(ui, scene, object, actions, state);
        });
        return;
    }

    let state_id = ui.make_persistent_id(("hierarchy_node", id));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, true)
        .show_header(ui, |ui| row(ui, scene, object, actions, state))
        .body(|ui| {
            for child in children {
                draw_node(ui, scene, child, actions, state);
            }
        });
}

fn start_rename(state: &mut HierarchyState, object: &GameObject) {
    state.renaming = Some(RenameState {
        id: object.id,
        buffer: object.name.clone(),
        focused: false,
    });
}

fn rename_field(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id: u64,
    actions: &mut Actions,
    state: &mut HierarchyState,
) {
    let Some(rename) = state.renaming.as_mut() else {
        return;
    };

    let edit = ui.put(
        rect,
        egui::TextEdit::singleline(&mut rename.buffer).desired_width(rect.width()),
    );

    if !rename.focused {
        edit.request_focus();
        rename.focused = true;
        return;
    }

    if edit.lost_focus() {
        let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if !cancelled {
            actions.rename = Some((id, rename.buffer.clone()));
        }
        state.renaming = None;
    }
}

fn row(
    ui: &mut egui::Ui,
    scene: &Scene,
    object: &GameObject,
    actions: &mut Actions,
    state: &mut HierarchyState,
) {
    let selected = scene.selected == Some(object.id);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(60.0), 20.0),
        Sense::click_and_drag(),
    );

    if state.renaming.as_ref().is_some_and(|r| r.id == object.id) {
        rename_field(ui, rect, object.id, actions, state);
        return;
    }

    let drop_target = response
        .dnd_hover_payload::<ObjectPayload>()
        .is_some_and(|p| p.id != object.id)
        || response
            .dnd_hover_payload::<AssetPayload>()
            .is_some_and(|p| is_prefab(&p.path) || is_vscript(&p.path));

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

    let text_color = if object.visible {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };

    ui.painter().text(
        egui::pos2(rect.min.x + 4.0, rect.center().y),
        Align2::LEFT_CENTER,
        &object.name,
        egui::TextStyle::Body.resolve(ui.style()),
        text_color,
    );

    if response.clicked() {
        actions.select = Some(object.id);
    }
    if response.double_clicked() {
        actions.select = Some(object.id);
        start_rename(state, object);
    }

    response.dnd_set_drag_payload(ObjectPayload { id: object.id });

    if let Some(payload) = response.dnd_release_payload::<ObjectPayload>() {
        if payload.id != object.id {
            actions.reparent = Some((payload.id, Some(object.id)));
        }
    }
    if let Some(payload) = response.dnd_release_payload::<AssetPayload>() {
        if is_prefab(&payload.path) {
            actions.instantiate = Some((payload.path.clone(), Some(object.id)));
        } else if is_vscript(&payload.path) {
            actions.attach_script = Some((payload.path.clone(), object.id));
        }
    }

    response.context_menu(|ui| {
        if ui.button("Rename").clicked() {
            start_rename(state, object);
            ui.close_menu();
        }

        ui.menu_button("Create Child", |ui| {
            create_items(ui, Some(object.id), actions)
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
