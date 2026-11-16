use eframe::egui::Vec2;

use crate::game_object::{Camera2D, ColliderShape, GameObject, Shape, Transform, BASE};
use crate::math::{regular_polygon, Affine};

pub enum WorldShape {
    Box { half_x: f32, half_y: f32 },
    Circle { radius: f32 },
    Capsule { radius: f32, half_segment: f32 },
    Polygon { points: Vec<Vec2> },
}

pub struct WorldCollider {
    pub shape: WorldShape,
    pub position: Vec2,
    pub angle: f32,
}

pub struct Scene {
    pub objects: Vec<GameObject>,
    pub selected: Option<u64>,
    next_id: u64,
}

impl Scene {
    pub fn new() -> Self {
        let mut scene = Self {
            objects: Vec::new(),
            selected: None,
            next_id: 1,
        };
        let mut camera = GameObject::camera();
        camera.name = "Main Camera".into();
        scene.add(camera);

        let id = scene.add(GameObject::from_shape(Shape::Square));
        scene.selected = Some(id);
        scene
    }

    pub fn add(&mut self, mut object: GameObject) -> u64 {
        object.id = self.next_id;
        self.next_id += 1;

        let id = object.id;
        self.objects.push(object);
        id
    }

    pub fn get(&self, id: u64) -> Option<&GameObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut GameObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    pub fn children_of(&self, parent: Option<u64>) -> Vec<u64> {
        self.objects
            .iter()
            .filter(|o| o.parent == parent)
            .map(|o| o.id)
            .collect()
    }

    // parents come before their children so children draw on top
    pub fn draw_order(&self) -> Vec<u64> {
        let mut order = Vec::new();
        self.collect(None, &mut order);
        order
    }

    fn collect(&self, parent: Option<u64>, order: &mut Vec<u64>) {
        for id in self.children_of(parent) {
            order.push(id);
            self.collect(Some(id), order);
        }
    }

    pub fn world_affine(&self, id: u64) -> Affine {
        let mut chain = Vec::new();
        let mut current = self.get(id);

        while let Some(object) = current {
            chain.push(object.transform);
            current = object.parent.and_then(|p| self.get(p));
        }

        chain
            .iter()
            .rev()
            .fold(Affine::IDENTITY, |acc, t| acc.mul(&local_affine(t)))
    }

    fn parent_affine(&self, id: u64) -> Affine {
        self.get(id)
            .and_then(|o| o.parent)
            .map(|p| self.world_affine(p))
            .unwrap_or(Affine::IDENTITY)
    }

    pub fn set_world_position(&mut self, id: u64, position: Vec2) {
        let local = self.parent_affine(id).inverse().apply(position);

        if let Some(object) = self.get_mut(id) {
            object.transform.x = local.x;
            object.transform.y = local.y;
        }
    }

    pub fn set_world_pose(&mut self, id: u64, position: Vec2, angle: f32) {
        let parent = self.parent_affine(id);
        let local = parent.inverse().apply(position);
        let rotation = -(angle - parent.angle()).to_degrees();

        if let Some(object) = self.get_mut(id) {
            object.transform.x = local.x;
            object.transform.y = local.y;
            object.transform.rotation = rotation;
        }
    }

    pub fn world_collider(&self, id: u64) -> Option<WorldCollider> {
        let object = self.get(id)?;
        let collider = object.collider?;
        let affine = self.world_affine(id);
        let scale = affine.scale();

        let shape = match collider.shape {
            ColliderShape::Box { size_x, size_y } => WorldShape::Box {
                half_x: (size_x * BASE * scale.x.abs() * 0.5).max(0.5),
                half_y: (size_y * BASE * scale.y.abs() * 0.5).max(0.5),
            },
            ColliderShape::Circle { radius } => WorldShape::Circle {
                radius: (radius * BASE * scale.x.abs().max(scale.y.abs())).max(0.5),
            },
            ColliderShape::Capsule { width, height } => {
                let radius = (width * BASE * scale.x.abs() * 0.5).max(0.5);
                let total = height * BASE * scale.y.abs();
                WorldShape::Capsule {
                    radius,
                    half_segment: (total * 0.5 - radius).max(0.0),
                }
            }
            ColliderShape::Polygon { sides, radius } => WorldShape::Polygon {
                points: regular_polygon(sides, radius * BASE)
                    .into_iter()
                    .map(|p| Vec2::new(p.x * scale.x.abs(), p.y * scale.y.abs()))
                    .collect(),
            },
        };

        Some(WorldCollider {
            shape,
            position: affine.position(),
            angle: affine.angle(),
        })
    }

    // first active camera in hierarchy order
    pub fn main_camera(&self) -> Option<(Camera2D, Vec2)> {
        self.draw_order().into_iter().find_map(|id| {
            let camera = self.get(id)?.camera?;
            if !self.is_visible(id) {
                return None;
            }
            Some((camera, self.world_affine(id).position()))
        })
    }

    pub fn is_visible(&self, id: u64) -> bool {
        let mut current = self.get(id);

        while let Some(object) = current {
            if !object.visible {
                return false;
            }
            current = object.parent.and_then(|p| self.get(p));
        }

        true
    }

    pub fn is_same_or_descendant(&self, id: u64, ancestor: u64) -> bool {
        let mut current = self.get(id);

        while let Some(object) = current {
            if object.id == ancestor {
                return true;
            }
            current = object.parent.and_then(|p| self.get(p));
        }

        false
    }

    // keeps the world transform so the object does not jump when reparented
    pub fn set_parent(&mut self, id: u64, parent: Option<u64>) {
        if let Some(parent) = parent {
            if self.is_same_or_descendant(parent, id) {
                return;
            }
        }
        match self.get(id) {
            Some(object) if object.parent != parent => {}
            _ => return,
        }

        let world = self.world_affine(id);
        let parent_affine = parent
            .map(|p| self.world_affine(p))
            .unwrap_or(Affine::IDENTITY);
        let (position, rotation, scale) = parent_affine.inverse().mul(&world).decompose();

        if let Some(object) = self.get_mut(id) {
            object.parent = parent;
            object.transform = Transform {
                x: position.x,
                y: position.y,
                rotation,
                scale_x: scale.x,
                scale_y: scale.y,
            };
        }
    }

    pub fn remove(&mut self, id: u64) {
        let mut doomed = vec![id];
        let mut index = 0;

        while index < doomed.len() {
            let current = doomed[index];
            doomed.extend(self.children_of(Some(current)));
            index += 1;
        }

        self.objects.retain(|o| !doomed.contains(&o.id));

        if self.selected.is_some_and(|s| doomed.contains(&s)) {
            self.selected = None;
        }
    }
}

fn local_affine(t: &Transform) -> Affine {
    Affine::from_trs(Vec2::new(t.x, t.y), t.rotation, Vec2::new(t.scale_x, t.scale_y))
}
