use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Painter, PointerButton, Pos2, Rect, Sense, Stroke,
    TextureOptions, Vec2,
};
use std::path::PathBuf;

use crate::assets::{image_uri, is_image};
use crate::drag::AssetPayload;
use crate::game_object::{GameObject, Shape, SpriteRenderer, TextureFilter, BASE};
use crate::gizmo::{Gizmo, Snap, Tool};
use crate::math::{capsule_outline, point_in_polygon, regular_polygon, rotate, Affine};
use crate::scene::{Scene, WorldShape};
use crate::view::View;

#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    Scene,
    Game,
    Both,
    Script,
}

struct SceneCamera {
    center: Vec2,
    zoom: f32,
}

pub struct ViewportState {
    pub tool: Tool,
    pub mode: ViewMode,
    pub show_colliders: bool,
    pub show_grid: bool,
    pub snap: Snap,
    camera: SceneCamera,
    gizmo: Gizmo,
    game_aspect: f32,
}

impl ViewportState {
    pub fn new() -> Self {
        Self {
            tool: Tool::Move,
            mode: ViewMode::Scene,
            show_colliders: true,
            show_grid: true,
            snap: Snap::default(),
            camera: SceneCamera {
                center: Vec2::ZERO,
                zoom: 1.0,
            },
            gizmo: Gizmo::new(),
            game_aspect: 16.0 / 9.0,
        }
    }
}

fn scene_view(
    ui: &mut egui::Ui,
    scene: &mut Scene,
    state: &mut ViewportState,
    playing: bool,
    spawn: &mut Option<(PathBuf, Vec2)>,
) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = response.rect;

    camera_input(ui, &response, rect, &mut state.camera);
    let view = View::new(rect.center(), state.camera.center, state.camera.zoom);

    painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 34));
    if state.show_grid {
        draw_grid(&painter, rect, &view, state.snap.move_step);
    }

    for id in scene.draw_order() {
        draw_object(ui, &painter, scene, id, &view, true);
    }

    draw_camera_frames(&painter, scene, &view, state.game_aspect);

    if state.show_colliders {
        for id in scene.draw_order() {
            if scene.is_visible(id) {
                draw_collider(&painter, scene, id, &view);
            }
        }
    }

    if let Some(id) = scene.selected {
        draw_outline(&painter, scene, id, &view);
    }

    state.snap.active = ui.input(|i| i.modifiers.ctrl);

    if playing {
        state.gizmo.finish();
    } else {
        if response.drag_started_by(PointerButton::Primary) {
            if let Some(press) = ui.input(|i| i.pointer.press_origin()) {
                state.gizmo.begin(state.tool, scene, &view, press);
            }
        }
        if response.dragged_by(PointerButton::Primary) {
            if let Some(pointer) = response.interact_pointer_pos() {
                state.gizmo.update(scene, pointer, &state.snap);
            }
        }
        if response.drag_stopped() {
            state.gizmo.finish();
        }

        state
            .gizmo
            .paint(&painter, state.tool, scene, &view, response.hover_pos());
    }

    if response.clicked_by(PointerButton::Primary) {
        if let Some(point) = response.interact_pointer_pos() {
            let on_handle = !playing && Gizmo::hits(state.tool, scene, &view, point);
            if !on_handle {
                scene.selected = pick(scene, &view, point);
            }
        }
    }

    let hovering_image = response
        .dnd_hover_payload::<AssetPayload>()
        .is_some_and(|p| is_image(&p.path));
    if hovering_image {
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(70, 130, 220, 40));
    }

    if let Some(payload) = response.dnd_release_payload::<AssetPayload>() {
        let pointer = ui.input(|i| i.pointer.latest_pos());
        if let (true, Some(pointer)) = (is_image(&payload.path), pointer) {
            *spawn = Some((payload.path.clone(), view.to_world(pointer)));
        }
    }

    painter.text(
        rect.left_bottom() + vec2(8.0, -8.0),
        Align2::LEFT_BOTTOM,
        format!("{:.0}%", state.camera.zoom * 100.0),
        FontId::proportional(12.0),
        Color32::from_white_alpha(120),
    );
}

fn game_view(ui: &mut egui::Ui, scene: &Scene, state: &mut ViewportState) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
    let rect = response.rect;
    state.game_aspect = (rect.width() / rect.height().max(1.0)).max(0.1);

    let Some((camera, position)) = scene.main_camera() else {
        painter.rect_filled(rect, 0.0, Color32::BLACK);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No camera rendering",
            FontId::proportional(14.0),
            Color32::from_white_alpha(140),
        );
        return;
    };

    painter.rect_filled(rect, 0.0, camera.background);

    // camera size is half the visible height in units
    let zoom = rect.height() / (2.0 * camera.size.max(0.01) * BASE);
    let view = View::new(rect.center(), position, zoom);

    for id in scene.draw_order() {
        draw_object(ui, &painter, scene, id, &view, false);
    }
}

fn script_view(
    ui: &mut egui::Ui,
    script_source: &mut String,
    script_error: Option<&str>,
) -> bool {
    ui.heading("Lua Script");

    ui.add(
        egui::TextEdit::multiline(script_source)
            .code_editor()
            .desired_width(f32::INFINITY)
            .desired_rows(24),
    );

    let run_clicked = ui.button("Run script").clicked();

    if let Some(error) = script_error {
        ui.colored_label(egui::Color32::RED, error);
    }

    run_clicked
}

pub fn show(
    ctx: &egui::Context,
    scene: &mut Scene,
    state: &mut ViewportState,
    playing: bool,
    script_source: &mut String,
    script_error: Option<&str>,
) -> bool {
    let mut spawn: Option<(PathBuf, Vec2)> = None;
    let mut requested = false;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.mode, ViewMode::Scene, "Scene");
            ui.selectable_value(&mut state.mode, ViewMode::Game, "Game");
            ui.selectable_value(&mut state.mode, ViewMode::Both, "Both");
            ui.selectable_value(&mut state.mode, ViewMode::Script, "Script");
        });
        ui.separator();

        match state.mode {
            ViewMode::Scene => scene_view(ui, scene, state, playing, &mut spawn),
            ViewMode::Game => game_view(ui, scene, state),
            ViewMode::Script => {
                requested = script_view(ui, script_source, script_error);
            }
            ViewMode::Both => {
                ui.columns(2, |columns| {
                    scene_view(&mut columns[0], scene, state, playing, &mut spawn);
                    game_view(&mut columns[1], scene, state);
                });
            }
        }
    });

    if let Some((path, position)) = spawn {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sprite".into());

        let id = scene.add(GameObject::sprite(
            name,
            image_uri(&path),
            position.x,
            position.y,
        ));
        scene.selected = Some(id);
    }

    requested
}

fn camera_input(ui: &egui::Ui, response: &egui::Response, rect: Rect, camera: &mut SceneCamera) {
    if response.hovered() {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = (scroll * 0.0015).exp() * pinch;

        if (factor - 1.0).abs() > 1e-4 {
            if let Some(cursor) = response.hover_pos() {
                let before = View::new(rect.center(), camera.center, camera.zoom).to_world(cursor);
                camera.zoom = (camera.zoom * factor).clamp(0.05, 20.0);
                let after = View::new(rect.center(), camera.center, camera.zoom).to_world(cursor);

                // keeps the point under the cursor fixed while zooming
                camera.center += before - after;
            }
        }
    }

    if response.dragged_by(PointerButton::Middle) || response.dragged_by(PointerButton::Secondary)
    {
        camera.center -= response.drag_delta() / camera.zoom;
    }
}

fn draw_grid(painter: &Painter, rect: Rect, view: &View, base_step: f32) {
    let mut step = base_step.max(1.0);
    while step * view.zoom < 14.0 {
        step *= 10.0;
    }

    let min = view.to_world(rect.min);
    let max = view.to_world(rect.max);
    let line = |index: i32, axis_color: Color32| {
        if index == 0 {
            axis_color
        } else if index % 10 == 0 {
            Color32::from_white_alpha(30)
        } else {
            Color32::from_white_alpha(11)
        }
    };

    for i in (min.x / step).floor() as i32..=(max.x / step).ceil() as i32 {
        let x = view.to_screen(vec2(i as f32 * step, 0.0)).x;
        painter.line_segment(
            [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
            Stroke::new(1.0_f32, line(i, Color32::from_rgba_unmultiplied(90, 210, 90, 110))),
        );
    }
    for i in (min.y / step).floor() as i32..=(max.y / step).ceil() as i32 {
        let y = view.to_screen(vec2(0.0, i as f32 * step)).y;
        painter.line_segment(
            [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
            Stroke::new(1.0_f32, line(i, Color32::from_rgba_unmultiplied(230, 70, 70, 110))),
        );
    }
}

fn shape_points(shape: Shape) -> Vec<Vec2> {
    let r = BASE / 2.0;

    match shape {
        Shape::Square => vec![vec2(-r, -r), vec2(r, -r), vec2(r, r), vec2(-r, r)],
        Shape::Circle => regular_polygon(48, r),
        Shape::Capsule => capsule_outline(r * 0.5, r * 0.5, 12),
        Shape::Triangle => regular_polygon(3, r),
        Shape::Diamond => regular_polygon(4, r),
        Shape::Pentagon => regular_polygon(5, r),
        Shape::Hexagon => regular_polygon(6, r),
    }
}

fn draw_object(
    ui: &egui::Ui,
    painter: &Painter,
    scene: &Scene,
    id: u64,
    view: &View,
    markers: bool,
) {
    let Some(object) = scene.get(id) else {
        return;
    };
    if !scene.is_visible(id) {
        return;
    }

    let affine = scene.world_affine(id);

    if let Some(sprite) = &object.sprite_renderer {
        draw_sprite(ui, painter, &affine, view, sprite);
    } else if let Some(renderer) = &object.shape_renderer {
        draw_shape(painter, &affine, view, renderer.shape, renderer.color);
    } else if markers {
        draw_marker(painter, view.to_screen(affine.position()), object.camera.is_some());
    }
}

fn draw_shape(painter: &Painter, affine: &Affine, view: &View, shape: Shape, color: Color32) {
    let points: Vec<Pos2> = shape_points(shape)
        .into_iter()
        .map(|p| view.to_screen(affine.apply(p)))
        .collect();

    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

// icons for objects that have nothing to draw
fn draw_marker(painter: &Painter, center: Pos2, is_camera: bool) {
    let color = Color32::from_rgb(200, 200, 215);

    if is_camera {
        painter.rect_filled(Rect::from_center_size(center, vec2(18.0, 12.0)), 2.0, color);
        painter.add(egui::Shape::convex_polygon(
            vec![
                center + vec2(9.0, 0.0),
                center + vec2(16.0, -6.0),
                center + vec2(16.0, 6.0),
            ],
            color,
            Stroke::NONE,
        ));
    } else {
        let stroke = Stroke::new(1.5_f32, color);
        painter.line_segment([center - vec2(8.0, 0.0), center + vec2(8.0, 0.0)], stroke);
        painter.line_segment([center - vec2(0.0, 8.0), center + vec2(0.0, 8.0)], stroke);
    }
}

fn draw_sprite(
    ui: &egui::Ui,
    painter: &Painter,
    affine: &Affine,
    view: &View,
    sprite: &SpriteRenderer,
) {
    let options = match sprite.filter {
        TextureFilter::Nearest => TextureOptions::NEAREST,
        TextureFilter::Linear => TextureOptions::LINEAR,
    };

    let image = egui::Image::new(sprite.texture.as_str()).texture_options(options);
    let loaded = image.load_for_size(ui.ctx(), Vec2::splat(BASE));

    let (texture_id, size) = match loaded {
        Ok(poll) => match (poll.texture_id(), poll.size()) {
            (Some(id), Some(size)) => (id, size),
            _ => return,
        },
        Err(_) => {
            draw_shape(
                painter,
                affine,
                view,
                Shape::Square,
                Color32::from_rgb(255, 0, 255),
            );
            return;
        }
    };

    let fit = (BASE / size.x).min(BASE / size.y);
    let half = size * fit * 0.5;

    let corners = [
        vec2(-half.x, -half.y),
        vec2(half.x, -half.y),
        vec2(half.x, half.y),
        vec2(-half.x, half.y),
    ];
    let uvs = [
        egui::pos2(0.0, 0.0),
        egui::pos2(1.0, 0.0),
        egui::pos2(1.0, 1.0),
        egui::pos2(0.0, 1.0),
    ];

    let mut mesh = egui::Mesh::with_texture(texture_id);
    for (corner, uv) in corners.iter().zip(uvs) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: view.to_screen(affine.apply(*corner)),
            uv,
            color: sprite.tint,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);

    painter.add(egui::Shape::mesh(mesh));
}

fn draw_camera_frames(painter: &Painter, scene: &Scene, view: &View, aspect: f32) {
    for object in &scene.objects {
        let Some(camera) = object.camera else {
            continue;
        };
        if !scene.is_visible(object.id) {
            continue;
        }

        let center = scene.world_affine(object.id).position();
        let half_h = camera.size.max(0.01) * BASE;
        let half_w = half_h * aspect;

        let points: Vec<Pos2> = [
            vec2(-half_w, -half_h),
            vec2(half_w, -half_h),
            vec2(half_w, half_h),
            vec2(-half_w, half_h),
        ]
        .iter()
        .map(|corner| view.to_screen(center + *corner))
        .collect();

        painter.add(egui::Shape::closed_line(
            points,
            Stroke::new(1.0_f32, Color32::from_white_alpha(110)),
        ));
    }
}

fn draw_collider(painter: &Painter, scene: &Scene, id: u64, view: &View) {
    let Some(collider) = scene.world_collider(id) else {
        return;
    };
    let stroke = Stroke::new(1.5_f32, Color32::from_rgb(80, 220, 120));
    let to_screen = |local: Vec2| view.to_screen(collider.position + rotate(local, collider.angle));

    match &collider.shape {
        WorldShape::Box { half_x, half_y } => {
            let points: Vec<Pos2> = [
                vec2(-half_x, -half_y),
                vec2(*half_x, -half_y),
                vec2(*half_x, *half_y),
                vec2(-half_x, *half_y),
            ]
            .iter()
            .map(|p| to_screen(*p))
            .collect();
            painter.add(egui::Shape::closed_line(points, stroke));
        }
        WorldShape::Circle { radius } => {
            painter.circle_stroke(view.to_screen(collider.position), radius * view.zoom, stroke);
            painter.line_segment(
                [
                    view.to_screen(collider.position),
                    to_screen(vec2(*radius, 0.0)),
                ],
                stroke,
            );
        }
        WorldShape::Capsule {
            radius,
            half_segment,
        } => {
            let points: Vec<Pos2> = capsule_outline(*radius, *half_segment, 12)
                .into_iter()
                .map(to_screen)
                .collect();
            painter.add(egui::Shape::closed_line(points, stroke));
        }
        WorldShape::Polygon { points } => {
            let points: Vec<Pos2> = points.iter().map(|p| to_screen(*p)).collect();
            painter.add(egui::Shape::closed_line(points, stroke));
        }
    }
}

fn draw_outline(painter: &Painter, scene: &Scene, id: u64, view: &View) {
    let Some(object) = scene.get(id) else {
        return;
    };
    if !scene.is_visible(id) {
        return;
    }

    let affine = scene.world_affine(id);
    let stroke = Stroke::new(1.5_f32, Color32::from_rgb(255, 200, 60));

    let local = if object.sprite_renderer.is_some() {
        shape_points(Shape::Square)
    } else if let Some(renderer) = &object.shape_renderer {
        shape_points(renderer.shape)
    } else {
        painter.circle_stroke(view.to_screen(affine.position()), 14.0, stroke);
        return;
    };

    let points: Vec<Pos2> = local
        .into_iter()
        .map(|p| view.to_screen(affine.apply(p)))
        .collect();
    painter.add(egui::Shape::closed_line(points, stroke));
}

fn pick(scene: &Scene, view: &View, point: Pos2) -> Option<u64> {
    for id in scene.draw_order().into_iter().rev() {
        let Some(object) = scene.get(id) else {
            continue;
        };
        if !scene.is_visible(id) {
            continue;
        }

        let affine = scene.world_affine(id);
        let local = affine.inverse().apply(view.to_world(point));
        let half = BASE / 2.0;

        let hit = if object.sprite_renderer.is_some() {
            local.x.abs() <= half && local.y.abs() <= half
        } else if let Some(renderer) = &object.shape_renderer {
            point_in_polygon(local, &shape_points(renderer.shape))
        } else {
            (view.to_screen(affine.position()) - point).length() <= 14.0
        };

        if hit {
            return Some(id);
        }
    }

    None
}
