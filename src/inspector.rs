use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Sense};

use crate::assets::{image_uri, is_image};
use crate::drag::AssetPayload;
use crate::game_object::{
    BodyType, Camera2D, Collider2D, ColliderShape, GameObject, Rigidbody2D, Shape, ShapeRenderer,
    SpriteRenderer, TextureFilter,
};
use crate::scene::Scene;
use crate::script::ScriptGraph;
use crate::viewport::ViewMode;

const COLLIDER_NAMES: [&str; 4] = ["Box", "Circle", "Capsule", "Polygon"];

pub fn show(ctx: &egui::Context, scene: &mut Scene, mode: &mut ViewMode) {
    egui::SidePanel::right("inspector")
        .resizable(true)
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.heading("Inspector");
            ui.separator();

            let Some(id) = scene.selected else {
                ui.label("Nothing selected");
                return;
            };

            let parent_name = scene
                .get(id)
                .and_then(|o| o.parent)
                .and_then(|p| scene.get(p))
                .map(|p| p.name.clone());

            let Some(object) = scene.get_mut(id) else {
                return;
            };

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.text_edit_singleline(&mut object.name);
                ui.checkbox(&mut object.visible, "Active");
                ui.label(format!(
                    "Parent: {}",
                    parent_name.as_deref().unwrap_or("None")
                ));
                ui.separator();

                components(ui, object, mode);
            });
        });
}

fn collider_kind(shape: &ColliderShape) -> usize {
    match shape {
        ColliderShape::Box { .. } => 0,
        ColliderShape::Circle { .. } => 1,
        ColliderShape::Capsule { .. } => 2,
        ColliderShape::Polygon { .. } => 3,
    }
}

fn default_collider(kind: usize) -> ColliderShape {
    match kind {
        0 => Collider2D::boxed().shape,
        1 => Collider2D::circle().shape,
        2 => Collider2D::capsule().shape,
        _ => Collider2D::polygon(6).shape,
    }
}

fn components(ui: &mut egui::Ui, object: &mut GameObject, mode: &mut ViewMode) {
    let id = object.id;
    let mut remove_shape = false;
    let mut remove_sprite = false;
    let mut remove_camera = false;
    let mut remove_body = false;
    let mut remove_collider = false;
    let mut remove_script = false;

    component(ui, ("transform", id), "Transform", false, |ui| {
        let t = &mut object.transform;
        egui::Grid::new(("transform_grid", id))
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Position");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut t.x).prefix("X ").speed(1.0));
                    ui.add(egui::DragValue::new(&mut t.y).prefix("Y ").speed(1.0));
                });
                ui.end_row();

                ui.label("Rotation");
                ui.add(egui::DragValue::new(&mut t.rotation).suffix("°").speed(0.5));
                ui.end_row();

                ui.label("Scale");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut t.scale_x).prefix("X ").speed(0.01));
                    ui.add(egui::DragValue::new(&mut t.scale_y).prefix("Y ").speed(0.01));
                });
                ui.end_row();
            });
    });

    if let Some(camera) = object.camera.as_mut() {
        remove_camera = component(ui, ("camera", id), "Camera 2D", true, |ui| {
            ui.horizontal(|ui| {
                ui.label("Size");
                ui.add(egui::DragValue::new(&mut camera.size).speed(0.05));
            });
            ui.horizontal(|ui| {
                ui.label("Background");
                ui.color_edit_button_srgba(&mut camera.background);
            });
        });
    }

    if let Some(renderer) = object.shape_renderer.as_mut() {
        remove_shape = component(ui, ("shape", id), "Shape Renderer", true, |ui| {
            egui::ComboBox::from_label("Shape")
                .selected_text(format!("{:?}", renderer.shape))
                .show_ui(ui, |ui| {
                    for shape in Shape::ALL {
                        ui.selectable_value(&mut renderer.shape, shape, format!("{shape:?}"));
                    }
                });
            ui.horizontal(|ui| {
                ui.label("Color");
                ui.color_edit_button_srgba(&mut renderer.color);
            });
        });
    }

    if let Some(sprite) = object.sprite_renderer.as_mut() {
        remove_sprite = component(ui, ("sprite", id), "Sprite Renderer", true, |ui| {
            let name = sprite.texture.rsplit('/').next().unwrap_or(&sprite.texture);
            ui.label(format!("Texture: {name}"));

            egui::ComboBox::from_label("Filter")
                .selected_text(format!("{:?}", sprite.filter))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut sprite.filter, TextureFilter::Nearest, "Nearest");
                    ui.selectable_value(&mut sprite.filter, TextureFilter::Linear, "Linear");
                });
            ui.horizontal(|ui| {
                ui.label("Tint");
                ui.color_edit_button_srgba(&mut sprite.tint);
            });
        });
    }

    let has_collider = object.collider.is_some();
    if let Some(body) = object.rigidbody.as_mut() {
        remove_body = component(ui, ("rigidbody", id), "Rigidbody 2D", true, |ui| {
            egui::ComboBox::from_label("Body")
                .selected_text(format!("{:?}", body.body))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut body.body, BodyType::Dynamic, "Dynamic");
                    ui.selectable_value(&mut body.body, BodyType::Kinematic, "Kinematic");
                    ui.selectable_value(&mut body.body, BodyType::Fixed, "Fixed");
                });
            ui.horizontal(|ui| {
                ui.label("Gravity scale");
                ui.add(egui::DragValue::new(&mut body.gravity_scale).speed(0.05));
            });
            ui.checkbox(&mut body.lock_rotation, "Lock rotation");

            if !has_collider {
                ui.colored_label(Color32::YELLOW, "needs a collider to collide");
            }
        });
    }

    let match_shape = object
        .shape_renderer
        .map(|r| Collider2D::for_shape(r.shape).shape);
    if let Some(collider) = object.collider.as_mut() {
        remove_collider = component(ui, ("collider", id), "Collider 2D", true, |ui| {
            let mut kind = collider_kind(&collider.shape);
            let before = kind;

            egui::ComboBox::from_label("Collider")
                .selected_text(COLLIDER_NAMES[kind])
                .show_ui(ui, |ui| {
                    for (index, name) in COLLIDER_NAMES.iter().enumerate() {
                        ui.selectable_value(&mut kind, index, *name);
                    }
                });
            if kind != before {
                collider.shape = default_collider(kind);
            }

            if let Some(shape) = match_shape {
                if ui.button("Match shape").clicked() {
                    collider.shape = shape;
                }
            }

            match &mut collider.shape {
                ColliderShape::Box { size_x, size_y } => {
                    ui.horizontal(|ui| {
                        ui.label("Size");
                        ui.add(egui::DragValue::new(size_x).prefix("X ").speed(0.01));
                        ui.add(egui::DragValue::new(size_y).prefix("Y ").speed(0.01));
                    });
                }
                ColliderShape::Circle { radius } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius");
                        ui.add(egui::DragValue::new(radius).speed(0.01));
                    });
                }
                ColliderShape::Capsule { width, height } => {
                    ui.horizontal(|ui| {
                        ui.label("Size");
                        ui.add(egui::DragValue::new(width).prefix("W ").speed(0.01));
                        ui.add(egui::DragValue::new(height).prefix("H ").speed(0.01));
                    });
                }
                ColliderShape::Polygon { sides, radius } => {
                    ui.add(egui::Slider::new(sides, 3..=16).text("Sides"));
                    ui.horizontal(|ui| {
                        ui.label("Radius");
                        ui.add(egui::DragValue::new(radius).speed(0.01));
                    });
                }
            }

            ui.add(egui::Slider::new(&mut collider.bounciness, 0.0..=1.0).text("Bounciness"));
        });
    }

    if let Some(graph) = &object.script {
        let nodes = graph.nodes.len();
        remove_script = component(ui, ("script", id), "Visual Script", true, |ui| {
            ui.label(format!("{nodes} nodes"));
            if ui.button("Open Script Editor").clicked() {
                *mode = ViewMode::Script;
            }
        });
    }

    if remove_script {
        object.script = None;
    }
    if remove_camera {
        object.camera = None;
    }
    if remove_shape {
        object.shape_renderer = None;
    }
    if remove_sprite {
        object.sprite_renderer = None;
    }
    if remove_body {
        object.rigidbody = None;
    }
    if remove_collider {
        object.collider = None;
    }

    ui.separator();
    add_component_menu(ui, object);
    sprite_drop_zone(ui, object);
}

fn add_component_menu(ui: &mut egui::Ui, object: &mut GameObject) {
    ui.menu_button("Add Component", |ui| {
        ui.menu_button("Rendering", |ui| {
            let free = object.shape_renderer.is_none() && object.sprite_renderer.is_none();
            if ui
                .add_enabled(free, egui::Button::new("Shape Renderer"))
                .clicked()
            {
                object.shape_renderer = Some(ShapeRenderer {
                    shape: Shape::Square,
                    color: Color32::WHITE,
                });
                ui.close_menu();
            }

            if ui
                .add_enabled(object.camera.is_none(), egui::Button::new("Camera 2D"))
                .clicked()
            {
                object.camera = Some(Camera2D::default());
                ui.close_menu();
            }
        });

        ui.menu_button("Physics 2D", |ui| {
            if ui
                .add_enabled(object.rigidbody.is_none(), egui::Button::new("Rigidbody 2D"))
                .clicked()
            {
                object.rigidbody = Some(Rigidbody2D::default());
                ui.close_menu();
            }

            let free = object.collider.is_none();
            let colliders = [
                ("Box Collider 2D", Collider2D::boxed()),
                ("Circle Collider 2D", Collider2D::circle()),
                ("Capsule Collider 2D", Collider2D::capsule()),
                ("Polygon Collider 2D", Collider2D::polygon(6)),
            ];

            for (label, collider) in colliders {
                if ui.add_enabled(free, egui::Button::new(label)).clicked() {
                    object.collider = Some(collider);
                    ui.close_menu();
                }
            }
        });

        ui.menu_button("Scripting", |ui| {
            if ui
                .add_enabled(object.script.is_none(), egui::Button::new("Visual Script"))
                .clicked()
            {
                object.script = Some(ScriptGraph::starter());
                ui.close_menu();
            }
        });
    });
}

fn sprite_drop_zone(ui: &mut egui::Ui, object: &mut GameObject) {
    ui.add_space(8.0);

    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), Sense::hover());

    let hovering = response
        .dnd_hover_payload::<AssetPayload>()
        .is_some_and(|p| is_image(&p.path));
    let fill = if hovering {
        Color32::from_rgba_unmultiplied(70, 130, 220, 70)
    } else {
        ui.visuals().faint_bg_color
    };

    ui.painter().rect_filled(rect, 4.0, fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "Drop an image here to set the sprite",
        FontId::proportional(12.0),
        ui.visuals().weak_text_color(),
    );

    if let Some(payload) = response.dnd_release_payload::<AssetPayload>() {
        if is_image(&payload.path) {
            object.sprite_renderer = Some(SpriteRenderer {
                texture: image_uri(&payload.path),
                filter: TextureFilter::Nearest,
                tint: Color32::WHITE,
            });
            object.shape_renderer = None;
        }
    }
}

// collapsible section with an optional remove button, returns true when remove was clicked
fn component(
    ui: &mut egui::Ui,
    key: (&str, u64),
    title: &str,
    removable: bool,
    body: impl FnOnce(&mut egui::Ui),
) -> bool {
    let mut remove = false;
    let state_id = ui.make_persistent_id(key);

    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, true)
        .show_header(ui, |ui| {
            ui.strong(title);
            if removable {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("x").clicked() {
                        remove = true;
                    }
                });
            }
        })
        .body(body);

    remove
}
