use eframe::egui::{vec2, Vec2};
use std::collections::HashSet;

use crate::math::{point_in_polygon, rotate};
use crate::scene::{Scene, WorldShape};

// a little slack so resting contacts still count as touching
const MARGIN: f32 = 1.5;

// every collider reduced to a point, a segment or a convex polygon plus a radius
struct Core {
    points: Vec<Vec2>,
    closed: bool,
    radius: f32,
}

fn core_of(scene: &Scene, id: u64) -> Option<Core> {
    let collider = scene.world_collider(id)?;
    let to_world = |p: Vec2| collider.position + rotate(p, collider.angle);

    Some(match &collider.shape {
        WorldShape::Box { half_x, half_y } => Core {
            points: [
                vec2(-half_x, -half_y),
                vec2(*half_x, -half_y),
                vec2(*half_x, *half_y),
                vec2(-half_x, *half_y),
            ]
            .iter()
            .map(|p| to_world(*p))
            .collect(),
            closed: true,
            radius: 0.0,
        },
        WorldShape::Circle { radius } => Core {
            points: vec![collider.position],
            closed: false,
            radius: *radius,
        },
        WorldShape::Capsule {
            radius,
            half_segment,
        } => Core {
            points: vec![
                to_world(vec2(0.0, -half_segment)),
                to_world(vec2(0.0, *half_segment)),
            ],
            closed: false,
            radius: *radius,
        },
        WorldShape::Polygon { points } => Core {
            points: points.iter().map(|p| to_world(*p)).collect(),
            closed: true,
            radius: 0.0,
        },
    })
}

fn edges(core: &Core) -> Vec<(Vec2, Vec2)> {
    let points = &core.points;

    match points.len() {
        0 => Vec::new(),
        1 => vec![(points[0], points[0])],
        2 => vec![(points[0], points[1])],
        n => (0..n).map(|i| (points[i], points[(i + 1) % n])).collect(),
    }
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn segments_intersect(p1: Vec2, p2: Vec2, q1: Vec2, q2: Vec2) -> bool {
    let d1 = cross(p2 - p1, q1 - p1);
    let d2 = cross(p2 - p1, q2 - p1);
    let d3 = cross(q2 - q1, p1 - q1);
    let d4 = cross(q2 - q1, p2 - q1);
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let length = ab.length_sq();
    let t = if length <= f32::EPSILON {
        0.0
    } else {
        ((p - a).dot(ab) / length).clamp(0.0, 1.0)
    };
    (p - (a + ab * t)).length()
}

fn segment_distance(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> f32 {
    if segments_intersect(a0, a1, b0, b1) {
        return 0.0;
    }

    point_segment_distance(a0, b0, b1)
        .min(point_segment_distance(a1, b0, b1))
        .min(point_segment_distance(b0, a0, a1))
        .min(point_segment_distance(b1, a0, a1))
}

fn distance(a: &Core, b: &Core) -> f32 {
    if a.closed && b.points.iter().any(|p| point_in_polygon(*p, &a.points)) {
        return 0.0;
    }
    if b.closed && a.points.iter().any(|p| point_in_polygon(*p, &b.points)) {
        return 0.0;
    }

    let mut best = f32::MAX;
    for (a0, a1) in edges(a) {
        for (b0, b1) in edges(b) {
            best = best.min(segment_distance(a0, a1, b0, b1));
        }
    }
    best
}

// pairs are stored smaller id first and at least one side needs a rigidbody
pub fn touching_pairs(scene: &Scene) -> HashSet<(u64, u64)> {
    let cores: Vec<(u64, Core, bool)> = scene
        .objects
        .iter()
        .filter(|o| o.collider.is_some() && scene.is_visible(o.id))
        .filter_map(|o| Some((o.id, core_of(scene, o.id)?, o.rigidbody.is_some())))
        .collect();

    let mut pairs = HashSet::new();

    for i in 0..cores.len() {
        for j in (i + 1)..cores.len() {
            let (a, b) = (&cores[i], &cores[j]);
            if !a.2 && !b.2 {
                continue;
            }

            if distance(&a.1, &b.1) <= a.1.radius + b.1.radius + MARGIN {
                pairs.insert((a.0.min(b.0), a.0.max(b.0)));
            }
        }
    }

    pairs
}
