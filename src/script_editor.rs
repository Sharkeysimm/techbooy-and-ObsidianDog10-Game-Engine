use eframe::egui::{
    self, pos2, vec2, Align, Color32, Id, Key, Layout, PointerButton, Pos2, Rect, RichText, Sense,
    Shape, Stroke, UiBuilder, Vec2,
};
use std::collections::HashMap;

use crate::scene::Scene;
use crate::script::{Category, Compare, Node, NodeKind, Property, ScriptGraph};

const NODE_W: f32 = 210.0;
const PORT_R: f32 = 6.0;

struct LinkDrag {
    from: u32,
    port: usize,
}

// key node being rebound and whether its button was drawn this frame
#[derive(Default)]
struct KeyCapture {
    active: Option<Id>,
    seen: bool,
}

struct Picker {
    at: Pos2,
    world: Vec2,
    from: Option<(u32, usize)>,
    query: String,
    focus: bool,
}

enum PickerResult {
    Open,
    Close,
    Choose(NodeKind),
}

#[derive(Default)]
pub struct ScriptEditor {
    pan: Vec2,
    link: Option<LinkDrag>,
    picker: Option<Picker>,
    capture: KeyCapture,
}

impl ScriptEditor {
    pub fn capturing(&self) -> bool {
        self.capture.active.is_some()
    }

    pub fn stop_capture(&mut self) {
        self.capture.active = None;
    }
}

struct NodeLayout {
    input: Option<Pos2>,
    outputs: Vec<Pos2>,
}

#[derive(Default)]
struct Actions {
    delete: Option<u32>,
    disconnect_all: Option<u32>,
    unplug_input: Option<u32>,
    unplug_output: Option<(u32, usize)>,
    connect: Option<(u32, usize, u32)>,
}

pub fn show(ui: &mut egui::Ui, scene: &mut Scene, state: &mut ScriptEditor) {
    let Some(id) = scene.selected else {
        ui.label("Select an object to edit its script");
        return;
    };
    let Some(object) = scene.get(id) else {
        return;
    };
    let name = object.name.clone();

    if object.script.is_none() {
        ui.label(format!("{name} has no visual script"));
        if ui.button("Add Visual Script").clicked() {
            if let Some(object) = scene.get_mut(id) {
                object.script = Some(ScriptGraph::starter());
            }
        }
        return;
    }

    ui.horizontal(|ui| {
        ui.heading(format!("Visual Script: {name}"));
        ui.menu_button("Add Node", |ui| {
            if let Some(graph) = scene.get_mut(id).and_then(|o| o.script.as_mut()) {
                add_menu(ui, graph, vec2(40.0, 40.0) - state.pan);
            }
        });
    });
    ui.label(
        "Events start a chain. Drag from a dot to another node to connect. \
         Right-click empty space to add nodes, a dot to unplug, a title to delete.",
    );
    ui.separator();

    let mut clear_log = false;
    egui::TopBottomPanel::bottom("script_console")
        .resizable(true)
        .default_height(90.0)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Console");
                if ui.small_button("Clear").clicked() {
                    clear_log = true;
                }
            });
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for line in &scene.log {
                        ui.label(line);
                    }
                });
        });
    if clear_log {
        scene.log.clear();
    }

    egui::CentralPanel::default().show_inside(ui, |ui| {
        if let Some(graph) = scene.get_mut(id).and_then(|o| o.script.as_mut()) {
            canvas(ui, graph, state, id);
        }
    });
}

fn canvas(ui: &mut egui::Ui, graph: &mut ScriptGraph, state: &mut ScriptEditor, object_id: u64) {
    let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
    let rect = response.rect;

    state.capture.seen = false;

    painter.rect_filled(rect, 0.0, Color32::from_rgb(26, 26, 30));
    draw_grid(&painter, rect, state.pan);

    if response.dragged_by(PointerButton::Primary) || response.dragged_by(PointerButton::Middle) {
        state.pan += response.drag_delta();
    }

    let origin = rect.min + state.pan;

    if response.secondary_clicked() {
        if let Some(point) = response.interact_pointer_pos() {
            state.picker = Some(Picker {
                at: point,
                world: point - origin,
                from: None,
                query: String::new(),
                focus: true,
            });
        }
    }

    // reserved so links draw underneath the nodes
    let links_slot = painter.add(Shape::Noop);

    let mut layouts: HashMap<u32, NodeLayout> = HashMap::new();
    let mut actions = Actions::default();

    for index in 0..graph.nodes.len() {
        let node = &mut graph.nodes[index];
        let layout = draw_node(ui, origin, object_id, node, state, &mut actions);
        layouts.insert(node.id, layout);
    }

    let mut shapes = Vec::new();
    for link in &graph.links {
        let (Some(from), Some(to)) = (layouts.get(&link.from), layouts.get(&link.to)) else {
            continue;
        };
        let (Some(a), Some(b)) = (from.outputs.get(link.port), to.input) else {
            continue;
        };
        shapes.push(Shape::line(
            bezier(*a, b),
            Stroke::new(2.5_f32, Color32::from_rgb(190, 190, 205)),
        ));
    }
    painter.set(links_slot, Shape::Vec(shapes));

    let pointer = ui.input(|i| i.pointer.latest_pos());

    if let Some(drag) = &state.link {
        let start = layouts.get(&drag.from).and_then(|l| l.outputs.get(drag.port));
        if let (Some(start), Some(pointer)) = (start, pointer) {
            painter.add(Shape::line(
                bezier(*start, pointer),
                Stroke::new(2.0_f32, Color32::WHITE),
            ));
        }
    }

    if state.link.is_some() && ui.input(|i| i.pointer.any_released()) {
        if let (Some(drag), Some(pointer)) = (state.link.take(), pointer) {
            let target = layouts
                .iter()
                .find(|(id, layout)| {
                    **id != drag.from
                        && layout.input.is_some_and(|p| (p - pointer).length() <= 16.0)
                })
                .map(|(id, _)| *id);

            if let Some(to) = target {
                actions.connect = Some((drag.from, drag.port, to));
            } else if rect.contains(pointer) {
                // the new node lands with its input dot under the pointer
                state.picker = Some(Picker {
                    at: pointer,
                    world: pointer - origin - vec2(0.0, 14.0),
                    from: Some((drag.from, drag.port)),
                    query: String::new(),
                    focus: true,
                });
            }
        }
    }

    if let Some(id) = actions.delete {
        graph.remove(id);
    }
    if let Some(id) = actions.disconnect_all {
        graph.disconnect_all(id);
    }
    if let Some(id) = actions.unplug_input {
        graph.disconnect_inputs(id);
    }
    if let Some((from, port)) = actions.unplug_output {
        graph.disconnect_output(from, port);
    }
    if let Some((from, port, to)) = actions.connect {
        graph.connect(from, port, to);
    }

    if let Some(picker) = state.picker.as_mut() {
        match show_picker(ui.ctx(), picker) {
            PickerResult::Open => {}
            PickerResult::Close => state.picker = None,
            PickerResult::Choose(kind) => {
                let (world, from) = (picker.world, picker.from);
                state.picker = None;

                let id = graph.add(kind, world);
                if let Some((from, port)) = from {
                    graph.connect(from, port, id);
                }
            }
        }
    }

    // a rebinding button that is no longer on screen must not keep the keyboard
    if !state.capture.seen {
        state.capture.active = None;
    }
}

fn draw_node(
    ui: &mut egui::Ui,
    origin: Pos2,
    object_id: u64,
    node: &mut Node,
    state: &mut ScriptEditor,
    actions: &mut Actions,
) -> NodeLayout {
    let node_id = node.id;
    let pos = origin + node.pos;
    let slot = ui.painter().add(Shape::Noop);

    let inner = Rect::from_min_size(pos + vec2(10.0, 8.0), vec2(NODE_W - 20.0, 2000.0));
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(inner)
            .id_salt(("script_node", object_id, node_id)),
    );
    child.set_width(NODE_W - 20.0);

    let title_rect = child
        .label(
            RichText::new(node.kind.title())
                .strong()
                .color(Color32::WHITE),
        )
        .rect;
    child.add_space(4.0);

    let salt = Id::new(("node_params", object_id, node_id));
    params(&mut child, salt, &mut node.kind, &mut state.capture);

    let mut output_ys = Vec::new();
    if node.kind.outputs().len() > 1 {
        for label in node.kind.outputs() {
            let row = child.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(*label);
            });
            output_ys.push(row.response.rect.center().y);
        }
    }

    let used = child.min_rect();
    let rect = Rect::from_min_max(pos, pos2(pos.x + NODE_W, used.max.y + 8.0));
    let header = Rect::from_min_max(pos, pos2(pos.x + NODE_W, title_rect.max.y + 5.0));

    ui.painter().set(
        slot,
        Shape::Vec(vec![
            Shape::rect_filled(rect, 5.0, Color32::from_rgb(44, 44, 52)),
            Shape::rect_filled(header, 5.0, category_color(&node.kind)),
        ]),
    );

    let mid_y = title_rect.center().y;
    let input = node.kind.has_input().then(|| pos2(pos.x, mid_y));
    let outputs: Vec<Pos2> = if node.kind.outputs().len() > 1 {
        output_ys
            .iter()
            .map(|y| pos2(pos.x + NODE_W, *y))
            .collect()
    } else {
        node.kind
            .outputs()
            .iter()
            .map(|_| pos2(pos.x + NODE_W, mid_y))
            .collect()
    };

    let header_response = ui.interact(
        header,
        Id::new(("script_header", object_id, node_id)),
        Sense::click_and_drag(),
    );
    if header_response.dragged_by(PointerButton::Primary) {
        node.pos += header_response.drag_delta();
    }
    header_response.context_menu(|ui| {
        if ui.button("Delete").clicked() {
            actions.delete = Some(node_id);
            ui.close_menu();
        }
        if ui.button("Disconnect all").clicked() {
            actions.disconnect_all = Some(node_id);
            ui.close_menu();
        }
    });

    let linking = state.link.is_some();
    let pointer = ui.input(|i| i.pointer.latest_pos());

    if let Some(point) = input {
        let response = ui.interact(
            Rect::from_center_size(point, vec2(18.0, 18.0)),
            Id::new(("script_in", object_id, node_id)),
            Sense::click(),
        );
        if response.clicked() {
            actions.unplug_input = Some(node_id);
        }

        let hot = linking && pointer.is_some_and(|p| (p - point).length() <= 16.0);
        port_dot(ui, point, hot);
    }

    for (port, point) in outputs.iter().enumerate() {
        let response = ui.interact(
            Rect::from_center_size(*point, vec2(18.0, 18.0)),
            Id::new(("script_out", object_id, node_id, port)),
            Sense::click_and_drag(),
        );
        if response.drag_started_by(PointerButton::Primary) {
            state.link = Some(LinkDrag {
                from: node_id,
                port,
            });
        }
        if response.secondary_clicked() {
            actions.unplug_output = Some((node_id, port));
        }

        port_dot(ui, *point, response.hovered());
    }

    NodeLayout { input, outputs }
}

fn port_dot(ui: &egui::Ui, center: Pos2, hot: bool) {
    let fill = if hot {
        Color32::from_rgb(255, 245, 140)
    } else {
        Color32::from_rgb(210, 210, 225)
    };
    ui.painter().circle_filled(center, PORT_R, fill);
    ui.painter()
        .circle_stroke(center, PORT_R, Stroke::new(1.5_f32, Color32::from_rgb(20, 20, 24)));
}

fn category_color(kind: &NodeKind) -> Color32 {
    match kind.category() {
        Category::Event => Color32::from_rgb(176, 68, 68),
        Category::Action => Color32::from_rgb(62, 108, 186),
        Category::Flow => Color32::from_rgb(196, 138, 48),
    }
}

fn params(ui: &mut egui::Ui, salt: Id, kind: &mut NodeKind, capture: &mut KeyCapture) {
    match kind {
        NodeKind::WhileKey { key } | NodeKind::OnKeyPressed { key } | NodeKind::IfKey { key } => {
            key_button(ui, salt, key, capture);
        }
        NodeKind::Move { x, y } => {
            ui.label("Units per second");
            xy_row(ui, x, y, 1.0);
        }
        NodeKind::Rotate { degrees } => {
            ui.horizontal(|ui| {
                ui.label("Degrees per second");
                ui.add(egui::DragValue::new(degrees).speed(1.0));
            });
        }
        NodeKind::SetPosition { x, y } => xy_row(ui, x, y, 1.0),
        NodeKind::SetScale { x, y } => xy_row(ui, x, y, 0.01),
        NodeKind::SetVisible { visible } => {
            ui.checkbox(visible, "Visible");
        }
        NodeKind::AddVelocity { x, y } => {
            ui.label("Instant speed change");
            xy_row(ui, x, y, 1.0);
        }
        NodeKind::Print { text } => {
            ui.add(egui::TextEdit::singleline(text).desired_width(NODE_W - 30.0));
        }
        NodeKind::Wait { seconds } => {
            ui.horizontal(|ui| {
                ui.label("Wait");
                ui.add(egui::DragValue::new(seconds).speed(0.05).suffix(" s"));
            });
        }
        NodeKind::IfCompare {
            property,
            compare,
            value,
        } => {
            egui::ComboBox::from_id_salt(salt.with("property"))
                .selected_text(format!("{property:?}"))
                .show_ui(ui, |ui| {
                    for option in Property::ALL {
                        ui.selectable_value(property, option, format!("{option:?}"));
                    }
                });
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(salt.with("compare"))
                    .selected_text(format!("{compare:?}"))
                    .show_ui(ui, |ui| {
                        for option in Compare::ALL {
                            ui.selectable_value(compare, option, format!("{option:?}"));
                        }
                    });
                ui.add(egui::DragValue::new(value).speed(1.0));
            });
        }
        NodeKind::OnStart | NodeKind::EveryFrame | NodeKind::Destroy => {}
    }
}

fn xy_row(ui: &mut egui::Ui, x: &mut f32, y: &mut f32, speed: f64) {
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(x).prefix("X ").speed(speed));
        ui.add(egui::DragValue::new(y).prefix("Y ").speed(speed));
    });
}

// click the box then press the key to bind it and escape cancels
fn key_button(ui: &mut egui::Ui, salt: Id, key: &mut Key, capture: &mut KeyCapture) {
    let id = salt.with("key");
    let mut captured = false;

    // read the key before the button so the same press cannot also click it
    if capture.active == Some(id) {
        capture.seen = true;

        let pressed = ui.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key: pressed_key,
                    pressed: true,
                    repeat: false,
                    ..
                } => Some(*pressed_key),
                _ => None,
            })
        });

        if let Some(pressed) = pressed {
            if pressed != Key::Escape {
                *key = pressed;
            }
            capture.active = None;
            captured = true;
        }
    }

    let listening = capture.active == Some(id);
    let text = if listening {
        "Press a key...".to_string()
    } else {
        key.name().to_string()
    };

    let response = ui.add(egui::Button::new(text).selected(listening));

    if !captured {
        if response.clicked() {
            capture.active = if listening { None } else { Some(id) };
        } else if listening && response.clicked_elsewhere() {
            capture.active = None;
        }
    }

    // marked after the click is handled so the frame that starts listening counts too
    if capture.active == Some(id) {
        capture.seen = true;
    }
}

fn show_picker(ctx: &egui::Context, picker: &mut Picker) -> PickerResult {
    let mut result = PickerResult::Open;
    let only_connectable = picker.from.is_some();

    let area = egui::Area::new(Id::new("script_node_picker"))
        .fixed_pos(picker.at)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(230.0);

                let edit = ui.add(
                    egui::TextEdit::singleline(&mut picker.query)
                        .hint_text("Search nodes")
                        .desired_width(f32::INFINITY),
                );
                if picker.focus {
                    edit.request_focus();
                    picker.focus = false;
                }
                ui.separator();

                let query = picker.query.to_lowercase();
                let mut first: Option<NodeKind> = None;

                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for (category, kinds) in NodeKind::catalog() {
                            let matches: Vec<NodeKind> = kinds
                                .into_iter()
                                .filter(|kind| !only_connectable || kind.has_input())
                                .filter(|kind| {
                                    query.is_empty()
                                        || kind.title().to_lowercase().contains(&query)
                                })
                                .collect();

                            if matches.is_empty() {
                                continue;
                            }

                            ui.label(RichText::new(category).weak());
                            for kind in matches {
                                if first.is_none() {
                                    first = Some(kind.clone());
                                }
                                if ui.button(kind.title()).clicked() {
                                    result = PickerResult::Choose(kind);
                                }
                            }
                        }
                    });

                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    if let Some(kind) = first {
                        result = PickerResult::Choose(kind);
                    }
                }
            });
        });

    let outside_click = ctx.input(|i| i.pointer.any_pressed())
        && ctx
            .pointer_latest_pos()
            .is_some_and(|p| !area.response.rect.contains(p));

    if outside_click || ctx.input(|i| i.key_pressed(Key::Escape)) {
        result = PickerResult::Close;
    }

    result
}

fn add_menu(ui: &mut egui::Ui, graph: &mut ScriptGraph, at: Vec2) {
    for (category, kinds) in NodeKind::catalog() {
        ui.menu_button(category, |ui| {
            for kind in kinds {
                if ui.button(kind.title()).clicked() {
                    graph.add(kind, at);
                    ui.close_menu();
                }
            }
        });
    }
}

fn draw_grid(painter: &egui::Painter, rect: Rect, pan: Vec2) {
    let spacing = 32.0;
    let stroke = Stroke::new(1.0_f32, Color32::from_white_alpha(8));

    let mut x = rect.min.x + pan.x.rem_euclid(spacing);
    while x < rect.max.x {
        painter.line_segment([pos2(x, rect.min.y), pos2(x, rect.max.y)], stroke);
        x += spacing;
    }

    let mut y = rect.min.y + pan.y.rem_euclid(spacing);
    while y < rect.max.y {
        painter.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], stroke);
        y += spacing;
    }
}

fn bezier(a: Pos2, b: Pos2) -> Vec<Pos2> {
    let reach = ((b.x - a.x).abs() * 0.5).max(40.0);
    let c1 = pos2(a.x + reach, a.y);
    let c2 = pos2(b.x - reach, b.y);

    (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            let u = 1.0 - t;
            let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            pos2(
                w0 * a.x + w1 * c1.x + w2 * c2.x + w3 * b.x,
                w0 * a.y + w1 * c1.y + w2 * c2.y + w3 * b.y,
            )
        })
        .collect()
}
