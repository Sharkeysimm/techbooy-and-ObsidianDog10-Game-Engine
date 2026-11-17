use eframe::egui::{Key, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::collision;
use crate::scene::Scene;
use crate::storage;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Property {
    PosX,
    PosY,
    Rotation,
    ScaleX,
    ScaleY,
}

impl Property {
    pub const ALL: [Property; 5] = [
        Property::PosX,
        Property::PosY,
        Property::Rotation,
        Property::ScaleX,
        Property::ScaleY,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Compare {
    Less,
    Greater,
}

impl Compare {
    pub const ALL: [Compare; 2] = [Compare::Less, Compare::Greater];
}

#[derive(Clone, Copy, PartialEq)]
pub enum Category {
    Event,
    Action,
    Flow,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    OnStart,
    EveryFrame,
    WhileKey {
        #[serde(with = "crate::serde_util::key")]
        key: Key,
    },
    OnKeyPressed {
        #[serde(with = "crate::serde_util::key")]
        key: Key,
    },
    OnKeyReleased {
        #[serde(with = "crate::serde_util::key")]
        key: Key,
    },
    OnCollide { with: String },
    WhileColliding { with: String },
    OnStopColliding { with: String },
    Move { x: f32, y: f32 },
    Rotate { degrees: f32 },
    SetPosition { x: f32, y: f32 },
    SetScale { x: f32, y: f32 },
    SetVisible { visible: bool },
    AddVelocity { x: f32, y: f32 },
    SetVelocityX { value: f32 },
    SetVelocityY { value: f32 },
    Print { text: String },
    Wait { seconds: f32 },
    SpawnPrefab { prefab: String },
    LoadScene { scene: String },
    DestroyOther,
    Destroy,
    IfKey {
        #[serde(with = "crate::serde_util::key")]
        key: Key,
    },
    IfCompare {
        property: Property,
        compare: Compare,
        value: f32,
    },
}

impl NodeKind {
    pub fn title(&self) -> &'static str {
        match self {
            NodeKind::OnStart => "On Start",
            NodeKind::EveryFrame => "Every Frame",
            NodeKind::WhileKey { .. } => "While Key Held",
            NodeKind::OnKeyPressed { .. } => "On Key Pressed",
            NodeKind::OnKeyReleased { .. } => "On Key Released",
            NodeKind::OnCollide { .. } => "On Collide",
            NodeKind::WhileColliding { .. } => "While Colliding",
            NodeKind::OnStopColliding { .. } => "On Stop Colliding",
            NodeKind::Move { .. } => "Move",
            NodeKind::Rotate { .. } => "Rotate",
            NodeKind::SetPosition { .. } => "Set Position",
            NodeKind::SetScale { .. } => "Set Scale",
            NodeKind::SetVisible { .. } => "Set Visible",
            NodeKind::AddVelocity { .. } => "Add Velocity",
            NodeKind::SetVelocityX { .. } => "Set Velocity X",
            NodeKind::SetVelocityY { .. } => "Set Velocity Y",
            NodeKind::Print { .. } => "Print",
            NodeKind::Wait { .. } => "Wait",
            NodeKind::SpawnPrefab { .. } => "Spawn Prefab",
            NodeKind::LoadScene { .. } => "Load Scene",
            NodeKind::DestroyOther => "Destroy Other",
            NodeKind::Destroy => "Destroy Self",
            NodeKind::IfKey { .. } => "If Key Held",
            NodeKind::IfCompare { .. } => "If Compare",
        }
    }

    pub fn category(&self) -> Category {
        match self {
            NodeKind::OnStart
            | NodeKind::EveryFrame
            | NodeKind::WhileKey { .. }
            | NodeKind::OnKeyPressed { .. }
            | NodeKind::OnKeyReleased { .. }
            | NodeKind::OnCollide { .. }
            | NodeKind::WhileColliding { .. }
            | NodeKind::OnStopColliding { .. } => Category::Event,
            NodeKind::IfKey { .. } | NodeKind::IfCompare { .. } => Category::Flow,
            _ => Category::Action,
        }
    }

    pub fn has_input(&self) -> bool {
        self.category() != Category::Event
    }

    // an empty label means one plain output
    pub fn outputs(&self) -> &'static [&'static str] {
        match self {
            NodeKind::Destroy | NodeKind::LoadScene { .. } => &[],
            NodeKind::IfKey { .. } | NodeKind::IfCompare { .. } => &["True", "False"],
            _ => &[""],
        }
    }

    pub fn catalog() -> Vec<(&'static str, Vec<NodeKind>)> {
        vec![
            (
                "Events",
                vec![
                    NodeKind::OnStart,
                    NodeKind::EveryFrame,
                    NodeKind::WhileKey { key: Key::Space },
                    NodeKind::OnKeyPressed { key: Key::Space },
                    NodeKind::OnKeyReleased { key: Key::Space },
                    NodeKind::OnCollide {
                        with: String::new(),
                    },
                    NodeKind::WhileColliding {
                        with: String::new(),
                    },
                    NodeKind::OnStopColliding {
                        with: String::new(),
                    },
                ],
            ),
            (
                "Actions",
                vec![
                    NodeKind::Move { x: 100.0, y: 0.0 },
                    NodeKind::Rotate { degrees: 90.0 },
                    NodeKind::SetPosition { x: 0.0, y: 0.0 },
                    NodeKind::SetScale { x: 1.0, y: 1.0 },
                    NodeKind::SetVisible { visible: true },
                    NodeKind::AddVelocity { x: 0.0, y: -400.0 },
                    NodeKind::SetVelocityX { value: 200.0 },
                    NodeKind::SetVelocityY { value: 0.0 },
                    NodeKind::Print {
                        text: "Hello".into(),
                    },
                    NodeKind::Wait { seconds: 1.0 },
                    NodeKind::SpawnPrefab {
                        prefab: String::new(),
                    },
                    NodeKind::LoadScene {
                        scene: String::new(),
                    },
                    NodeKind::DestroyOther,
                    NodeKind::Destroy,
                ],
            ),
            (
                "Flow",
                vec![
                    NodeKind::IfKey { key: Key::Space },
                    NodeKind::IfCompare {
                        property: Property::PosY,
                        compare: Compare::Greater,
                        value: 0.0,
                    },
                ],
            ),
        ]
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: u32,
    pub kind: NodeKind,
    #[serde(with = "crate::serde_util::vec2")]
    pub pos: Vec2,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Link {
    pub from: u32,
    pub port: usize,
    pub to: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ScriptGraph {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    next_id: u32,
}

impl ScriptGraph {
    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            links: Vec::new(),
            next_id: 1,
        }
    }

    // small example so a new graph already does something
    pub fn starter() -> Self {
        let mut graph = Self::empty();

        let start = graph.add(NodeKind::OnStart, Vec2::new(30.0, 30.0));
        let print = graph.add(
            NodeKind::Print {
                text: "Hello".into(),
            },
            Vec2::new(280.0, 30.0),
        );
        graph.connect(start, 0, print);

        let held = graph.add(
            NodeKind::WhileKey {
                key: Key::ArrowRight,
            },
            Vec2::new(30.0, 170.0),
        );
        let mover = graph.add(NodeKind::Move { x: 200.0, y: 0.0 }, Vec2::new(280.0, 170.0));
        graph.connect(held, 0, mover);

        graph
    }

    pub fn add(&mut self, kind: NodeKind, pos: Vec2) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(Node { id, kind, pos });
        id
    }

    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn target(&self, from: u32, port: usize) -> Option<u32> {
        self.links
            .iter()
            .find(|l| l.from == from && l.port == port)
            .map(|l| l.to)
    }

    pub fn connect(&mut self, from: u32, port: usize, to: u32) {
        if from == to {
            return;
        }
        let Some(source) = self.node(from) else {
            return;
        };
        let Some(destination) = self.node(to) else {
            return;
        };
        if port >= source.kind.outputs().len() || !destination.kind.has_input() {
            return;
        }

        self.disconnect_output(from, port);
        self.links.push(Link { from, port, to });
    }

    pub fn disconnect_output(&mut self, from: u32, port: usize) {
        self.links.retain(|l| !(l.from == from && l.port == port));
    }

    pub fn disconnect_inputs(&mut self, to: u32) {
        self.links.retain(|l| l.to != to);
    }

    pub fn disconnect_all(&mut self, id: u32) {
        self.links.retain(|l| l.from != id && l.to != id);
    }

    pub fn remove(&mut self, id: u32) {
        self.nodes.retain(|n| n.id != id);
        self.disconnect_all(id);
    }
}

struct Input {
    held: HashSet<Key>,
    pressed: HashSet<Key>,
    released: HashSet<Key>,
}

// pairs are (this object, other object) and both directions are listed
#[derive(Default)]
struct Collisions {
    enter: Vec<(u64, u64)>,
    stay: Vec<(u64, u64)>,
    exit: Vec<(u64, u64)>,
}

struct Pending {
    object: u64,
    node: u32,
    other: Option<u64>,
    remaining: f32,
}

pub struct ScriptRuntime {
    started: HashSet<u64>,
    previous: HashSet<Key>,
    touching: HashSet<(u64, u64)>,
    pending: Vec<Pending>,
}

const MAX_STEPS: usize = 500;

impl ScriptRuntime {
    pub fn new() -> Self {
        Self {
            started: HashSet::new(),
            previous: HashSet::new(),
            touching: HashSet::new(),
            pending: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.started.clear();
        self.previous.clear();
        self.touching.clear();
        self.pending.clear();
    }

    pub fn update(&mut self, scene: &mut Scene, held: HashSet<Key>, dt: f32) {
        let pressed: HashSet<Key> = held.difference(&self.previous).copied().collect();
        let released: HashSet<Key> = self.previous.difference(&held).copied().collect();
        self.previous = held.clone();
        let input = Input {
            held,
            pressed,
            released,
        };

        let now = collision::touching_pairs(scene);
        let mut collisions = Collisions::default();

        for &(a, b) in &now {
            let is_new = !self.touching.contains(&(a, b));
            for pair in [(a, b), (b, a)] {
                collisions.stay.push(pair);
                if is_new {
                    collisions.enter.push(pair);
                }
            }
        }
        for &(a, b) in &self.touching {
            if !now.contains(&(a, b)) {
                collisions.exit.push((a, b));
                collisions.exit.push((b, a));
            }
        }
        self.touching = now;

        let ids: Vec<u64> = scene
            .objects
            .iter()
            .filter(|o| o.script.is_some() && scene.is_visible(o.id))
            .map(|o| o.id)
            .collect();

        for id in ids {
            self.run_object(scene, &input, &collisions, id, dt);
        }

        // waits belonging to destroyed objects are dropped
        self.pending.retain(|p| scene.get(p.object).is_some());
    }

    fn run_object(
        &mut self,
        scene: &mut Scene,
        input: &Input,
        collisions: &Collisions,
        id: u64,
        dt: f32,
    ) {
        // graph is taken out while it runs so the scene can be changed freely
        let Some(graph) = scene.get_mut(id).and_then(|o| o.script.take()) else {
            return;
        };
        let mut alive = true;

        let mut due: Vec<(u32, Option<u64>)> = Vec::new();
        self.pending.retain_mut(|p| {
            if p.object != id {
                return true;
            }
            p.remaining -= dt;
            if p.remaining <= 0.0 {
                due.push((p.node, p.other));
                false
            } else {
                true
            }
        });
        for (node, other) in due {
            if !alive {
                break;
            }
            let other = other.filter(|o| scene.get(*o).is_some());
            alive = self.run_chain(scene, input, &graph, id, Some(node), other, dt);
        }

        let first_frame = self.started.insert(id);
        for node in &graph.nodes {
            if !alive {
                break;
            }

            let triggers: Vec<Option<u64>> = match &node.kind {
                NodeKind::OnStart => {
                    if first_frame {
                        vec![None]
                    } else {
                        Vec::new()
                    }
                }
                NodeKind::EveryFrame => vec![None],
                NodeKind::WhileKey { key } => flag(input.held.contains(key)),
                NodeKind::OnKeyPressed { key } => flag(input.pressed.contains(key)),
                NodeKind::OnKeyReleased { key } => flag(input.released.contains(key)),
                NodeKind::OnCollide { with } => others(scene, &collisions.enter, id, with),
                NodeKind::WhileColliding { with } => others(scene, &collisions.stay, id, with),
                NodeKind::OnStopColliding { with } => others(scene, &collisions.exit, id, with),
                _ => Vec::new(),
            };

            let first = graph.target(node.id, 0);
            for other in triggers {
                if !alive {
                    break;
                }
                alive = self.run_chain(scene, input, &graph, id, first, other, dt);
            }
        }

        if alive {
            if let Some(object) = scene.get_mut(id) {
                object.script = Some(graph);
            }
        }
    }

    // returns false when the object destroyed itself
    #[allow(clippy::too_many_arguments)]
    fn run_chain(
        &mut self,
        scene: &mut Scene,
        input: &Input,
        graph: &ScriptGraph,
        id: u64,
        start: Option<u32>,
        other: Option<u64>,
        dt: f32,
    ) -> bool {
        let mut current = start;
        let mut steps = 0;

        while let Some(node_id) = current {
            steps += 1;
            if steps > MAX_STEPS {
                scene.log("script stopped because it looped without a Wait".into());
                break;
            }
            let Some(node) = graph.node(node_id) else {
                break;
            };

            let mut port = 0;

            match &node.kind {
                NodeKind::Move { x, y } => {
                    if let Some(object) = scene.get_mut(id) {
                        object.transform.x += x * dt;
                        object.transform.y += y * dt;
                    }
                }
                NodeKind::Rotate { degrees } => {
                    if let Some(object) = scene.get_mut(id) {
                        object.transform.rotation += degrees * dt;
                    }
                }
                NodeKind::SetPosition { x, y } => {
                    if let Some(object) = scene.get_mut(id) {
                        object.transform.x = *x;
                        object.transform.y = *y;
                    }
                }
                NodeKind::SetScale { x, y } => {
                    if let Some(object) = scene.get_mut(id) {
                        object.transform.scale_x = *x;
                        object.transform.scale_y = *y;
                    }
                }
                NodeKind::SetVisible { visible } => {
                    if let Some(object) = scene.get_mut(id) {
                        object.visible = *visible;
                    }
                }
                NodeKind::AddVelocity { x, y } => scene.kicks.push((id, Vec2::new(*x, *y))),
                NodeKind::SetVelocityX { value } => {
                    scene.velocity_sets.push((id, Some(*value), None))
                }
                NodeKind::SetVelocityY { value } => {
                    scene.velocity_sets.push((id, None, Some(*value)))
                }
                NodeKind::Print { text } => scene.log(text.clone()),
                NodeKind::Wait { seconds } => {
                    if let Some(next) = graph.target(node_id, 0) {
                        self.pending.push(Pending {
                            object: id,
                            node: next,
                            other,
                            remaining: seconds.max(0.0),
                        });
                    }
                    break;
                }
                NodeKind::SpawnPrefab { prefab } => {
                    let position = scene.world_affine(id).position();
                    match storage::load_prefab(Path::new(prefab)) {
                        Some(data) => {
                            storage::instantiate(scene, &data, None, Some(position));
                        }
                        None => scene.log(format!("could not load prefab '{prefab}'")),
                    }
                }
                NodeKind::LoadScene { scene: target } => {
                    if target.is_empty() {
                        scene.log("pick a scene in the Load Scene node".into());
                    } else {
                        scene.pending_scene = Some(target.clone());
                    }
                    break;
                }
                NodeKind::DestroyOther => {
                    if let Some(target) = other {
                        if target != id {
                            scene.remove(target);
                        }
                    }
                    // removing a parent also removes this object
                    if scene.get(id).is_none() {
                        return false;
                    }
                }
                NodeKind::Destroy => {
                    scene.remove(id);
                    return false;
                }
                NodeKind::IfKey { key } => {
                    port = if input.held.contains(key) { 0 } else { 1 };
                }
                NodeKind::IfCompare {
                    property,
                    compare,
                    value,
                } => {
                    let Some(object) = scene.get(id) else {
                        break;
                    };
                    let t = object.transform;
                    let current_value = match property {
                        Property::PosX => t.x,
                        Property::PosY => t.y,
                        Property::Rotation => t.rotation,
                        Property::ScaleX => t.scale_x,
                        Property::ScaleY => t.scale_y,
                    };
                    let passed = match compare {
                        Compare::Less => current_value < *value,
                        Compare::Greater => current_value > *value,
                    };
                    port = if passed { 0 } else { 1 };
                }
                NodeKind::OnStart
                | NodeKind::EveryFrame
                | NodeKind::WhileKey { .. }
                | NodeKind::OnKeyPressed { .. }
                | NodeKind::OnKeyReleased { .. }
                | NodeKind::OnCollide { .. }
                | NodeKind::WhileColliding { .. }
                | NodeKind::OnStopColliding { .. } => {}
            }

            current = graph.target(node_id, port);
        }

        true
    }
}

fn flag(condition: bool) -> Vec<Option<u64>> {
    if condition {
        vec![None]
    } else {
        Vec::new()
    }
}

// other objects in the list for this object whose name matches the filter
fn others(scene: &Scene, list: &[(u64, u64)], id: u64, with: &str) -> Vec<Option<u64>> {
    let wanted = with.trim().to_lowercase();

    list.iter()
        .filter(|pair| pair.0 == id)
        .map(|pair| pair.1)
        .filter(|other| {
            wanted.is_empty()
                || scene
                    .get(*other)
                    .is_some_and(|o| o.name.to_lowercase() == wanted)
        })
        .map(Some)
        .collect()
}
