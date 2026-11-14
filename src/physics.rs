use rapier2d::prelude::*;

use crate::game_object::BodyType;
use crate::scene::{Scene, WorldCollider, WorldShape};

const GRAVITY: f32 = 980.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Static,
    Dynamic,
    Kinematic,
}

#[derive(Clone, PartialEq, Eq)]
enum SpecShape {
    Box(i32, i32),
    Circle(i32),
    Capsule(i32, i32),
    Polygon(Vec<(i32, i32)>),
}

// quantised so float noise does not trigger rebuilds
#[derive(Clone, PartialEq, Eq)]
struct Spec {
    id: u64,
    kind: Kind,
    shape: SpecShape,
    gravity: i32,
    lock_rotation: bool,
    bounciness: i32,
}

struct Item {
    spec: Spec,
    collider: WorldCollider,
    gravity_scale: f32,
    bounciness: f32,
}

pub struct PhysicsSystem {
    world: PhysicsWorld,
    dynamic: Vec<(u64, RigidBodyHandle)>,
    specs: Vec<Spec>,
}

impl PhysicsSystem {
    pub fn new() -> Self {
        Self {
            world: new_world(),
            dynamic: Vec::new(),
            specs: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.world = new_world();
        self.dynamic.clear();
        self.specs.clear();
    }

    pub fn step(&mut self, scene: &mut Scene) {
        let items = collect(scene);
        let specs: Vec<Spec> = items.iter().map(|i| i.spec.clone()).collect();

        if specs != self.specs {
            self.rebuild(&items);
            self.specs = specs;
        }

        if items.is_empty() {
            return;
        }

        self.world.step();

        for (id, handle) in &self.dynamic {
            let Some(body) = self.world.bodies.get(*handle) else {
                continue;
            };

            let position = body.translation();
            let angle = body.rotation().angle();
            scene.set_world_pose(*id, eframe::egui::Vec2::new(position.x, position.y), angle);
        }
    }

    fn rebuild(&mut self, items: &[Item]) {
        self.world = new_world();
        self.dynamic.clear();

        for item in items {
            let mut body = match item.spec.kind {
                Kind::Static => RigidBodyBuilder::fixed(),
                Kind::Dynamic => RigidBodyBuilder::dynamic().gravity_scale(item.gravity_scale),
                Kind::Kinematic => RigidBodyBuilder::kinematic_position_based(),
            };
            if item.spec.lock_rotation {
                body = body.lock_rotations();
            }
            let body = body
                .translation(Vector::new(
                    item.collider.position.x,
                    item.collider.position.y,
                ))
                .rotation(item.collider.angle);

            let collider = match &item.collider.shape {
                WorldShape::Box { half_x, half_y } => ColliderBuilder::cuboid(*half_x, *half_y),
                WorldShape::Circle { radius } => ColliderBuilder::ball(*radius),
                WorldShape::Capsule {
                    radius,
                    half_segment,
                } => ColliderBuilder::capsule_y(*half_segment, *radius),
                WorldShape::Polygon { points } => {
                    let hull: Vec<_> = points
                        .iter()
                        .map(|p| Vector::new(p.x, p.y).into())
                        .collect();
                    ColliderBuilder::convex_hull(&hull)
                        .unwrap_or_else(|| ColliderBuilder::ball(1.0))
                }
            }
            .restitution(item.bounciness);

            let (handle, _) = self.world.insert(body, collider);

            if item.spec.kind == Kind::Dynamic {
                self.dynamic.push((item.spec.id, handle));
            }
        }
    }
}

fn new_world() -> PhysicsWorld {
    let mut world = PhysicsWorld::new();
    world.gravity = Vector::new(0.0, GRAVITY);
    world
}

fn quantise(value: f32) -> i32 {
    (value * 100.0).round() as i32
}

// a collider alone is static and a rigidbody needs a collider to simulate
fn collect(scene: &Scene) -> Vec<Item> {
    scene
        .objects
        .iter()
        .filter(|o| scene.is_visible(o.id))
        .filter_map(|object| {
            let own = object.collider?;
            let collider = scene.world_collider(object.id)?;

            let (kind, gravity_scale, lock_rotation) = match object.rigidbody {
                None => (Kind::Static, 0.0, false),
                Some(rb) => {
                    let kind = match rb.body {
                        BodyType::Dynamic => Kind::Dynamic,
                        BodyType::Kinematic => Kind::Kinematic,
                        BodyType::Fixed => Kind::Static,
                    };
                    (kind, rb.gravity_scale, rb.lock_rotation)
                }
            };

            let shape = match &collider.shape {
                WorldShape::Box { half_x, half_y } => {
                    SpecShape::Box(quantise(*half_x), quantise(*half_y))
                }
                WorldShape::Circle { radius } => SpecShape::Circle(quantise(*radius)),
                WorldShape::Capsule {
                    radius,
                    half_segment,
                } => SpecShape::Capsule(quantise(*radius), quantise(*half_segment)),
                WorldShape::Polygon { points } => SpecShape::Polygon(
                    points
                        .iter()
                        .map(|p| (quantise(p.x), quantise(p.y)))
                        .collect(),
                ),
            };

            Some(Item {
                spec: Spec {
                    id: object.id,
                    kind,
                    shape,
                    gravity: quantise(gravity_scale),
                    lock_rotation,
                    bounciness: quantise(own.bounciness),
                },
                collider,
                gravity_scale,
                bounciness: own.bounciness,
            })
        })
        .collect()
}
