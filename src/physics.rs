use eframe::egui::Vec2;
use rapier2d::geometry::{Group, InteractionGroups, InteractionTestMode};
use rapier2d::prelude::*;
use std::collections::HashMap;

use crate::game_object::{BodyType, BASE};
use crate::math::rotate;
use crate::scene::{Scene, WorldCollider, WorldShape};

const GRAVITY: f32 = 980.0;
const STEP: f32 = 1.0 / 60.0;
const HINGE_ITERATIONS: usize = 10;
const BAUMGARTE: f32 = 0.2;

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
    freeze_x: bool,
    freeze_y: bool,
    trigger: bool,
    bounciness: i32,
}

struct Item {
    spec: Spec,
    collider: WorldCollider,
    gravity_scale: f32,
    bounciness: f32,
}

#[derive(Clone, PartialEq, Eq)]
struct HingeSpec {
    a: u64,
    b: Option<u64>,
    x: i32,
    y: i32,
    limits: Option<(i32, i32)>,
    motor: Option<i32>,
}

struct HingeItem {
    spec: HingeSpec,
    local_a: Vec2,
    limits: Option<(f32, f32)>,
    motor: Option<f32>,
}

// a hinge as the solver sees it with anchors in each body's own frame
struct Hinge {
    a: u64,
    b: Option<u64>,
    local_a: Vec2,
    local_b: Vec2,
    fixed: Vec2,
    reference: f32,
    limits: Option<(f32, f32)>,
    motor: Option<f32>,
}

struct BodyInfo {
    handle: RigidBodyHandle,
    inv_mass: f32,
    inv_inertia: f32,
}

#[derive(Clone, Copy)]
struct BodyState {
    pos: Vec2,
    angle: f32,
    v: Vec2,
    w: f32,
    inv_mass: f32,
    inv_inertia: f32,
}

impl BodyState {
    fn world(point: Vec2) -> Self {
        Self {
            pos: point,
            angle: 0.0,
            v: Vec2::ZERO,
            w: 0.0,
            inv_mass: 0.0,
            inv_inertia: 0.0,
        }
    }
}

pub struct PhysicsSystem {
    world: PhysicsWorld,
    dynamic: Vec<(u64, RigidBodyHandle)>,
    bodies: HashMap<u64, BodyInfo>,
    hinges: Vec<Hinge>,
    specs: Vec<Spec>,
    hinge_specs: Vec<HingeSpec>,
}

impl PhysicsSystem {
    pub fn new() -> Self {
        Self {
            world: new_world(),
            dynamic: Vec::new(),
            bodies: HashMap::new(),
            hinges: Vec::new(),
            specs: Vec::new(),
            hinge_specs: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.world = new_world();
        self.dynamic.clear();
        self.bodies.clear();
        self.hinges.clear();
        self.specs.clear();
        self.hinge_specs.clear();
    }

    pub fn step(&mut self, scene: &mut Scene) {
        // kicks are speed changes queued by scripts
        let kicks: Vec<(u64, Vec2)> = scene.kicks.drain(..).collect();
        let velocity_sets: Vec<(u64, Option<f32>, Option<f32>)> =
            scene.velocity_sets.drain(..).collect();
        let angvel_sets: Vec<(u64, f32)> = scene.angvel_sets.drain(..).collect();

        let items = collect(scene);
        let hinge_items = collect_hinges(scene, &items);
        let specs: Vec<Spec> = items.iter().map(|i| i.spec.clone()).collect();
        let hinge_specs: Vec<HingeSpec> = hinge_items.iter().map(|h| h.spec.clone()).collect();

        if specs != self.specs || hinge_specs != self.hinge_specs {
            self.rebuild(&items, &hinge_items);
            self.specs = specs;
            self.hinge_specs = hinge_specs;
        }

        if items.is_empty() {
            return;
        }

        // anything moved from outside the simulation gets teleported
        for (id, handle) in &self.dynamic {
            let target = scene.world_affine(*id).position();
            if let Some(body) = self.world.bodies.get_mut(*handle) {
                let current = body.translation();
                if (current.x - target.x).abs() > 0.01 || (current.y - target.y).abs() > 0.01 {
                    body.set_translation(Vector::new(target.x, target.y), true);
                }
            }
        }

        // an impulse scaled by mass gives the same speed change for any size
        for (id, velocity) in kicks {
            let Some(entry) = self.dynamic.iter().find(|entry| entry.0 == id) else {
                continue;
            };
            let handle = entry.1;

            if let Some(body) = self.world.bodies.get_mut(handle) {
                let mass = body.mass();
                body.apply_impulse(Vector::new(velocity.x * mass, velocity.y * mass), true);
            }
        }

        // scripts can overwrite one axis of the speed and keep the other
        for (id, x, y) in velocity_sets {
            let Some(entry) = self.dynamic.iter().find(|entry| entry.0 == id) else {
                continue;
            };
            let handle = entry.1;

            if let Some(body) = self.world.bodies.get_mut(handle) {
                let current = body.linvel();
                let (current_x, current_y) = (current.x, current.y);
                body.set_linvel(
                    Vector::new(x.unwrap_or(current_x), y.unwrap_or(current_y)),
                    true,
                );
            }
        }

        for (id, spin) in angvel_sets {
            let Some(entry) = self.dynamic.iter().find(|entry| entry.0 == id) else {
                continue;
            };
            let handle = entry.1;

            if let Some(body) = self.world.bodies.get_mut(handle) {
                body.set_angvel(spin, true);
            }
        }

        self.solve_hinges();
        self.world.step();

        for (id, handle) in &self.dynamic {
            let Some(body) = self.world.bodies.get(*handle) else {
                continue;
            };

            let position = body.translation();
            let angle = body.rotation().angle();
            scene.set_world_pose(*id, Vec2::new(position.x, position.y), angle);
        }
    }

    fn rebuild(&mut self, items: &[Item], hinge_items: &[HingeItem]) {
        self.world = new_world();
        self.dynamic.clear();
        self.bodies.clear();
        self.hinges.clear();

        let groups = collision_groups(hinge_items);

        for item in items {
            let mut body = match item.spec.kind {
                Kind::Static => RigidBodyBuilder::fixed(),
                Kind::Dynamic => RigidBodyBuilder::dynamic().gravity_scale(item.gravity_scale),
                Kind::Kinematic => RigidBodyBuilder::kinematic_position_based(),
            };
            if item.spec.lock_rotation {
                body = body.lock_rotations();
            }
            if item.spec.freeze_x || item.spec.freeze_y {
                body = body.enabled_translations(!item.spec.freeze_x, !item.spec.freeze_y);
            }
            let body = body
                .translation(Vector::new(
                    item.collider.position.x,
                    item.collider.position.y,
                ))
                .rotation(item.collider.angle);

            let mut collider = match &item.collider.shape {
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
            .restitution(item.bounciness)
            .sensor(item.spec.trigger);

            // bodies joined by a hinge do not collide with each other
            if let Some((memberships, filter)) = groups.get(&item.spec.id) {
                collider = collider.collision_groups(InteractionGroups::new(
                    Group::from_bits_truncate(*memberships),
                    Group::from_bits_truncate(*filter),
                    InteractionTestMode::And,
                ));
            }

            let (handle, _) = self.world.insert(body, collider);

            let (mass, inertia) = mass_properties(&item.collider.shape);
            let (inv_mass, inv_inertia) = match item.spec.kind {
                Kind::Dynamic => (
                    1.0 / mass.max(1e-3),
                    if item.spec.lock_rotation {
                        0.0
                    } else {
                        1.0 / inertia.max(1e-3)
                    },
                ),
                _ => (0.0, 0.0),
            };
            self.bodies.insert(
                item.spec.id,
                BodyInfo {
                    handle,
                    inv_mass,
                    inv_inertia,
                },
            );

            if item.spec.kind == Kind::Dynamic {
                self.dynamic.push((item.spec.id, handle));
            }
        }

        for hinge in hinge_items {
            let Some(a) = items.iter().find(|i| i.spec.id == hinge.spec.a) else {
                continue;
            };
            let anchor = a.collider.position + rotate(hinge.local_a, a.collider.angle);

            // the other side is measured at build time so the joint starts with no error
            let (local_b, fixed, reference) = match hinge.spec.b {
                Some(b_id) => {
                    let Some(b) = items.iter().find(|i| i.spec.id == b_id) else {
                        continue;
                    };
                    (
                        rotate(anchor - b.collider.position, -b.collider.angle),
                        Vec2::ZERO,
                        a.collider.angle - b.collider.angle,
                    )
                }
                None => (Vec2::ZERO, anchor, a.collider.angle),
            };

            self.hinges.push(Hinge {
                a: hinge.spec.a,
                b: hinge.spec.b,
                local_a: hinge.local_a,
                local_b,
                fixed,
                reference,
                limits: hinge.limits,
                motor: hinge.motor,
            });
        }
    }

    // sequential impulses on the velocities right before rapier steps
    fn solve_hinges(&mut self) {
        if self.hinges.is_empty() {
            return;
        }

        let mut states: HashMap<u64, BodyState> = HashMap::new();
        for hinge in &self.hinges {
            for id in std::iter::once(hinge.a).chain(hinge.b) {
                if states.contains_key(&id) {
                    continue;
                }
                let Some(info) = self.bodies.get(&id) else {
                    continue;
                };
                let Some(body) = self.world.bodies.get(info.handle) else {
                    continue;
                };

                let translation = body.translation();
                let velocity = body.linvel();
                states.insert(
                    id,
                    BodyState {
                        pos: Vec2::new(translation.x, translation.y),
                        angle: body.rotation().angle(),
                        v: Vec2::new(velocity.x, velocity.y),
                        w: body.angvel(),
                        inv_mass: info.inv_mass,
                        inv_inertia: info.inv_inertia,
                    },
                );
            }
        }

        for _ in 0..HINGE_ITERATIONS {
            for hinge in &self.hinges {
                let _ = solve_hinge(hinge, &mut states);
            }
        }

        for (id, state) in &states {
            if state.inv_mass <= 0.0 && state.inv_inertia <= 0.0 {
                continue;
            }
            let Some(info) = self.bodies.get(id) else {
                continue;
            };

            if let Some(body) = self.world.bodies.get_mut(info.handle) {
                body.set_linvel(Vector::new(state.v.x, state.v.y), true);
                body.set_angvel(state.w, true);
            }
        }
    }
}

fn solve_hinge(hinge: &Hinge, states: &mut HashMap<u64, BodyState>) -> Option<()> {
    let mut a = *states.get(&hinge.a)?;
    let mut b = match hinge.b {
        Some(id) => *states.get(&id)?,
        None => BodyState::world(hinge.fixed),
    };

    let ra = rotate(hinge.local_a, a.angle);
    let rb = if hinge.b.is_some() {
        rotate(hinge.local_b, b.angle)
    } else {
        Vec2::ZERO
    };
    let error = (b.pos + rb) - (a.pos + ra);

    // the two anchor points must move together and drift is pulled back gently
    let anchor_a = a.v + Vec2::new(-a.w * ra.y, a.w * ra.x);
    let anchor_b = b.v + Vec2::new(-b.w * rb.y, b.w * rb.x);
    let rhs = -((anchor_b - anchor_a) + error * (BAUMGARTE / STEP));

    let k11 = a.inv_mass + b.inv_mass + a.inv_inertia * ra.y * ra.y + b.inv_inertia * rb.y * rb.y;
    let k12 = -a.inv_inertia * ra.x * ra.y - b.inv_inertia * rb.x * rb.y;
    let k22 = a.inv_mass + b.inv_mass + a.inv_inertia * ra.x * ra.x + b.inv_inertia * rb.x * rb.x;
    let det = k11 * k22 - k12 * k12;

    if det.abs() > 1e-18 {
        let p = Vec2::new(
            (k22 * rhs.x - k12 * rhs.y) / det,
            (k11 * rhs.y - k12 * rhs.x) / det,
        );

        a.v -= p * a.inv_mass;
        a.w -= a.inv_inertia * (ra.x * p.y - ra.y * p.x);
        b.v += p * b.inv_mass;
        b.w += b.inv_inertia * (rb.x * p.y - rb.y * p.x);
    }

    let spin_mass = a.inv_inertia + b.inv_inertia;
    if spin_mass > 0.0 {
        if let Some((lower, upper)) = hinge.limits {
            let theta = wrap(a.angle - b.angle - hinge.reference);
            let relative = a.w - b.w;

            if theta < lower {
                let target = ((lower - theta) * BAUMGARTE / STEP).max(0.0);
                if relative < target {
                    let impulse = (target - relative) / spin_mass;
                    a.w += a.inv_inertia * impulse;
                    b.w -= b.inv_inertia * impulse;
                }
            } else if theta > upper {
                let target = -((theta - upper) * BAUMGARTE / STEP).max(0.0);
                if relative > target {
                    let impulse = (target - relative) / spin_mass;
                    a.w += a.inv_inertia * impulse;
                    b.w -= b.inv_inertia * impulse;
                }
            }
        }

        if let Some(speed) = hinge.motor {
            let impulse = (speed - (a.w - b.w)) / spin_mass;
            a.w += a.inv_inertia * impulse;
            b.w -= b.inv_inertia * impulse;
        }
    }

    states.insert(hinge.a, a);
    if let Some(id) = hinge.b {
        states.insert(id, b);
    }
    Some(())
}

fn wrap(angle: f32) -> f32 {
    angle.sin().atan2(angle.cos())
}

// every body in a hinge pair gets its own group bit and skips its partner
fn collision_groups(hinges: &[HingeItem]) -> HashMap<u64, (u32, u32)> {
    let mut bits: HashMap<u64, u32> = HashMap::new();
    let mut next = 0u32;

    for hinge in hinges {
        let Some(b) = hinge.spec.b else {
            continue;
        };
        for id in [hinge.spec.a, b] {
            if !bits.contains_key(&id) && next < 31 {
                bits.insert(id, 1u32 << next);
                next += 1;
            }
        }
    }

    let mut groups = HashMap::new();
    for (&id, &bit) in &bits {
        let mut excluded = 0u32;

        for hinge in hinges {
            let Some(b) = hinge.spec.b else {
                continue;
            };
            let partner = if hinge.spec.a == id {
                Some(b)
            } else if b == id {
                Some(hinge.spec.a)
            } else {
                None
            };

            if let Some(partner_bit) = partner.and_then(|p| bits.get(&p)) {
                excluded |= partner_bit;
            }
        }

        groups.insert(id, (bit, !excluded));
    }

    groups
}

// density is 1 so mass is just the area
fn mass_properties(shape: &WorldShape) -> (f32, f32) {
    match shape {
        WorldShape::Box { half_x, half_y } => {
            let mass = 4.0 * half_x * half_y;
            (mass, mass * (half_x * half_x + half_y * half_y) / 3.0)
        }
        WorldShape::Circle { radius } => {
            let mass = std::f32::consts::PI * radius * radius;
            (mass, mass * radius * radius / 2.0)
        }
        WorldShape::Capsule {
            radius,
            half_segment,
        } => {
            let mass = std::f32::consts::PI * radius * radius + 4.0 * radius * half_segment;
            let reach = half_segment + radius;
            (mass, mass * (radius * radius + reach * reach) / 3.0)
        }
        WorldShape::Polygon { points } => {
            let mut area = 0.0;
            let (mut reach_x, mut reach_y) = (0.0_f32, 0.0_f32);

            for (index, point) in points.iter().enumerate() {
                let next = points[(index + 1) % points.len()];
                area += point.x * next.y - next.x * point.y;
                reach_x = reach_x.max(point.x.abs());
                reach_y = reach_y.max(point.y.abs());
            }

            let mass = (area / 2.0).abs();
            (mass, mass * (reach_x * reach_x + reach_y * reach_y) / 3.0 * 0.7)
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

            let (kind, gravity_scale, lock_rotation, freeze) = match object.rigidbody {
                None => (Kind::Static, 0.0, false, (false, false)),
                Some(rb) => {
                    let kind = match rb.body {
                        BodyType::Dynamic => Kind::Dynamic,
                        BodyType::Kinematic => Kind::Kinematic,
                        BodyType::Fixed => Kind::Static,
                    };
                    (
                        kind,
                        rb.gravity_scale,
                        rb.lock_rotation,
                        (rb.freeze_x, rb.freeze_y),
                    )
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
                    freeze_x: freeze.0,
                    freeze_y: freeze.1,
                    trigger: own.is_trigger,
                    bounciness: quantise(own.bounciness),
                },
                collider,
                gravity_scale,
                bounciness: own.bounciness,
            })
        })
        .collect()
}

// a hinge needs both bodies to exist in the simulation or it is skipped
fn collect_hinges(scene: &Scene, items: &[Item]) -> Vec<HingeItem> {
    let exists = |id: u64| items.iter().any(|i| i.spec.id == id);

    scene
        .objects
        .iter()
        .filter_map(|object| {
            let hinge = object.hinge?;
            if !exists(object.id) {
                return None;
            }
            if let Some(other) = hinge.connected {
                if !exists(other) {
                    return None;
                }
            }

            let scale = scene.world_affine(object.id).scale();
            let local = Vec2::new(hinge.anchor_x * BASE * scale.x, hinge.anchor_y * BASE * scale.y);

            // angles in the inspector are counterclockwise but physics runs the other way
            let limits = hinge.use_limits.then(|| {
                let low = -hinge.upper_angle.to_radians();
                let high = -hinge.lower_angle.to_radians();
                (low.min(high), low.max(high))
            });
            let motor = hinge.use_motor.then(|| -hinge.motor_speed.to_radians());

            Some(HingeItem {
                spec: HingeSpec {
                    a: object.id,
                    b: hinge.connected,
                    x: quantise(local.x),
                    y: quantise(local.y),
                    limits: limits.map(|(low, high)| (quantise(low), quantise(high))),
                    motor: motor.map(quantise),
                },
                local_a: local,
                limits,
                motor,
            })
        })
        .collect()
}
