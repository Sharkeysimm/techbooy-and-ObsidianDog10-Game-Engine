use eframe::egui::{self, vec2, Color32, Painter, Pos2, Rect, Stroke, Vec2};

use crate::math::Affine;
use crate::scene::Scene;
use crate::view::View;

const ARM: f32 = 70.0;
const GRAB: f32 = 7.0;
const HANDLE: f32 = 7.0;

const RED: Color32 = Color32::from_rgb(230, 70, 70);
const GREEN: Color32 = Color32::from_rgb(90, 210, 90);
const BLUE: Color32 = Color32::from_rgb(90, 160, 255);
const YELLOW: Color32 = Color32::from_rgb(240, 220, 90);
const HIGHLIGHT: Color32 = Color32::from_rgb(255, 245, 140);

#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    Move,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, PartialEq)]
enum Handle {
    MoveX,
    MoveY,
    MoveFree,
    Rotate,
    ScaleX,
    ScaleY,
    ScaleUniform,
}

// active is set each frame from the ctrl key
pub struct Snap {
    pub active: bool,
    pub move_step: f32,
    pub rotate_step: f32,
    pub scale_step: f32,
}

impl Default for Snap {
    fn default() -> Self {
        Self {
            active: false,
            move_step: 10.0,
            rotate_step: 15.0,
            scale_step: 0.1,
        }
    }
}

struct ActiveDrag {
    handle: Handle,
    id: u64,
    zoom: f32,
    center: Pos2,
    axis_x: Vec2,
    axis_y: Vec2,
    start_pointer: Pos2,
    start_world: Vec2,
    start_rotation: f32,
    start_scale: Vec2,
}

pub struct Gizmo {
    drag: Option<ActiveDrag>,
}

impl Gizmo {
    pub fn new() -> Self {
        Self { drag: None }
    }

    pub fn hits(tool: Tool, scene: &Scene, view: &View, point: Pos2) -> bool {
        handle_at(tool, scene, view, point).is_some()
    }

    pub fn begin(&mut self, tool: Tool, scene: &Scene, view: &View, press: Pos2) {
        let Some(handle) = handle_at(tool, scene, view, press) else {
            return;
        };
        let Some(id) = scene.selected else {
            return;
        };
        let Some(object) = scene.get(id) else {
            return;
        };

        let affine = scene.world_affine(id);
        let (axis_x, axis_y) = axes(&affine);

        self.drag = Some(ActiveDrag {
            handle,
            id,
            zoom: view.zoom,
            center: view.to_screen(affine.position()),
            axis_x,
            axis_y,
            start_pointer: press,
            start_world: affine.position(),
            start_rotation: object.transform.rotation,
            start_scale: vec2(object.transform.scale_x, object.transform.scale_y),
        });
    }

    pub fn update(&mut self, scene: &mut Scene, pointer: Pos2, snap: &Snap) {
        let Some(drag) = &self.drag else {
            return;
        };

        // screen movement converted to world movement
        let delta = (pointer - drag.start_pointer) / drag.zoom;

        match drag.handle {
            Handle::MoveFree | Handle::MoveX | Handle::MoveY => {
                let mut position = drag.start_world;
                if drag.handle != Handle::MoveY {
                    position.x += delta.x;
                    if snap.active {
                        position.x = snap_to(position.x, snap.move_step);
                    }
                }
                if drag.handle != Handle::MoveX {
                    position.y += delta.y;
                    if snap.active {
                        position.y = snap_to(position.y, snap.move_step);
                    }
                }
                scene.set_world_position(drag.id, position);
            }
            Handle::Rotate => {
                let from = drag.start_pointer - drag.center;
                let to = pointer - drag.center;

                if from.length() > 1.0 && to.length() > 1.0 {
                    let angle = (from.x * to.y - from.y * to.x).atan2(from.dot(to));
                    let mut rotation = drag.start_rotation - angle.to_degrees();
                    if snap.active {
                        rotation = snap_to(rotation, snap.rotate_step);
                    }
                    if let Some(object) = scene.get_mut(drag.id) {
                        object.transform.rotation = rotation;
                    }
                }
            }
            Handle::ScaleX => {
                let ratio = axis_ratio(drag, pointer, drag.axis_x);
                let value = scaled(drag.start_scale.x, ratio, snap);
                if let Some(object) = scene.get_mut(drag.id) {
                    object.transform.scale_x = value;
                }
            }
            Handle::ScaleY => {
                let ratio = axis_ratio(drag, pointer, drag.axis_y);
                let value = scaled(drag.start_scale.y, ratio, snap);
                if let Some(object) = scene.get_mut(drag.id) {
                    object.transform.scale_y = value;
                }
            }
            Handle::ScaleUniform => {
                let screen = pointer - drag.start_pointer;
                let ratio = 1.0 + (screen.x - screen.y) / 120.0;
                let x = scaled(drag.start_scale.x, ratio, snap);
                let y = scaled(drag.start_scale.y, ratio, snap);
                if let Some(object) = scene.get_mut(drag.id) {
                    object.transform.scale_x = x;
                    object.transform.scale_y = y;
                }
            }
        }
    }

    pub fn finish(&mut self) {
        self.drag = None;
    }

    pub fn paint(
        &self,
        painter: &Painter,
        tool: Tool,
        scene: &Scene,
        view: &View,
        hover: Option<Pos2>,
    ) {
        let Some(id) = scene.selected else {
            return;
        };
        if scene.get(id).is_none() {
            return;
        }

        let affine = scene.world_affine(id);
        let center = view.to_screen(affine.position());
        let (axis_x, axis_y) = axes(&affine);

        let active = match &self.drag {
            Some(drag) => Some(drag.handle),
            None => hover.and_then(|p| handle_at(tool, scene, view, p)),
        };
        let lit = |handle: Handle, base: Color32| {
            if active == Some(handle) {
                HIGHLIGHT
            } else {
                base
            }
        };

        match tool {
            Tool::Move => {
                arrow(painter, center, vec2(1.0, 0.0), lit(Handle::MoveX, RED));
                arrow(painter, center, vec2(0.0, 1.0), lit(Handle::MoveY, GREEN));
                painter.rect_filled(
                    Rect::from_center_size(center, vec2(HANDLE * 2.0, HANDLE * 2.0)),
                    1.0,
                    lit(Handle::MoveFree, YELLOW).gamma_multiply(0.7),
                );
            }
            Tool::Rotate => {
                painter.circle_stroke(
                    center,
                    ARM,
                    Stroke::new(2.5_f32, lit(Handle::Rotate, BLUE)),
                );
            }
            Tool::Scale => {
                scale_handle(painter, center, axis_x, lit(Handle::ScaleX, RED));
                scale_handle(painter, center, axis_y, lit(Handle::ScaleY, GREEN));
                painter.rect_filled(
                    Rect::from_center_size(center, vec2(HANDLE * 2.0, HANDLE * 2.0)),
                    1.0,
                    lit(Handle::ScaleUniform, YELLOW),
                );
            }
        }
    }
}

fn handle_at(tool: Tool, scene: &Scene, view: &View, point: Pos2) -> Option<Handle> {
    let id = scene.selected?;
    scene.get(id)?;

    let affine = scene.world_affine(id);
    let center = view.to_screen(affine.position());
    let (axis_x, axis_y) = axes(&affine);
    let offset = point - center;
    let reach = HANDLE + 3.0;

    match tool {
        Tool::Move => {
            if offset.abs().max_elem() <= reach {
                Some(Handle::MoveFree)
            } else if distance_to_segment(point, center, center + vec2(ARM, 0.0)) <= GRAB {
                Some(Handle::MoveX)
            } else if distance_to_segment(point, center, center + vec2(0.0, ARM)) <= GRAB {
                Some(Handle::MoveY)
            } else {
                None
            }
        }
        Tool::Rotate => ((offset.length() - ARM).abs() <= GRAB).then_some(Handle::Rotate),
        Tool::Scale => {
            if offset.abs().max_elem() <= reach {
                Some(Handle::ScaleUniform)
            } else if (point - (center + axis_x * ARM)).abs().max_elem() <= reach {
                Some(Handle::ScaleX)
            } else if (point - (center + axis_y * ARM)).abs().max_elem() <= reach {
                Some(Handle::ScaleY)
            } else {
                None
            }
        }
    }
}

fn axes(affine: &Affine) -> (Vec2, Vec2) {
    let unit = |v: Vec2, fallback: Vec2| {
        if v.length() > 1e-4 {
            v.normalized()
        } else {
            fallback
        }
    };

    (
        unit(affine.apply_vector(vec2(1.0, 0.0)), vec2(1.0, 0.0)),
        unit(affine.apply_vector(vec2(0.0, 1.0)), vec2(0.0, 1.0)),
    )
}

fn axis_ratio(drag: &ActiveDrag, pointer: Pos2, axis: Vec2) -> f32 {
    let start = (drag.start_pointer - drag.center).dot(axis).max(10.0);
    (pointer - drag.center).dot(axis) / start
}

fn snap_to(value: f32, step: f32) -> f32 {
    if step > 0.0 {
        (value / step).round() * step
    } else {
        value
    }
}

fn scaled(start: f32, ratio: f32, snap: &Snap) -> f32 {
    let value = start.abs().max(0.01) * ratio.max(0.02) * start.signum();
    if !snap.active || snap.scale_step <= 0.0 {
        return value;
    }

    let snapped = snap_to(value, snap.scale_step);
    if snapped == 0.0 {
        snap.scale_step * value.signum()
    } else {
        snapped
    }
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

fn arrow(painter: &Painter, from: Pos2, dir: Vec2, color: Color32) {
    let base = from + dir * ARM;
    painter.line_segment([from, base], Stroke::new(2.5_f32, color));

    let side = vec2(-dir.y, dir.x) * 5.0;
    let tip = base + dir * 12.0;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, base + side, base - side],
        color,
        Stroke::NONE,
    ));
}

fn scale_handle(painter: &Painter, center: Pos2, axis: Vec2, color: Color32) {
    let end = center + axis * ARM;
    painter.line_segment([center, end], Stroke::new(2.0_f32, color));
    painter.rect_filled(
        Rect::from_center_size(end, vec2(HANDLE * 2.0, HANDLE * 2.0)),
        1.0,
        color,
    );
}
