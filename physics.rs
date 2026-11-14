use std::collections::HashMap;

use rapier2d::prelude::*;

use crate::game_object::{
    ColliderShape as GameColliderShape, GameObject, PhysicsBody, PhysicsConfig,
};

pub struct PhysicsSystem {
    world: PhysicsWorld,
    body_handles: HashMap<u64, RigidBodyHandle>,
    configs: HashMap<u64, Option<crate::game_object::PhysicsConfig>>,
}

impl PhysicsSystem {
pub fn teleport(&mut self, id: u64, x: f32, y: f32) {
    let Some(handle) = self.body_handles.get(&id) else {
        return;
    };

    if let Some(body) = self.world.bodies.get_mut(*handle) {
        body.set_translation(Vector::new(x, y), true);
    }
}

    pub fn sync_objects(&mut self, objects: &[GameObject]) {
        let current: HashMap<u64, Option<PhysicsConfig>> = objects
            .iter()
            .map(|object| (object.id, object.physics))
            .collect();

        if current == self.configs {
            return;
        }

        self.world = PhysicsWorld::new();
        self.world.gravity = Vector::new(0.0, 290.0);
        self.body_handles.clear();

        for object in objects {
            let Some(config) = object.physics else {
                continue;
            };

            let body_builder = match config.body {
                PhysicsBody::Dynamic => RigidBodyBuilder::dynamic().linear_damping(1.0),
                PhysicsBody::Fixed => RigidBodyBuilder::fixed(),
                PhysicsBody::Kinematic => RigidBodyBuilder::kinematic_position_based(),
            }
            .translation(Vector::new(object.x, object.y));

            let collider_builder = match config.collider {
                GameColliderShape::Box { half_width, half_height } => {
                    ColliderBuilder::cuboid(half_width, half_height)
                }
                GameColliderShape::Circle { radius } => ColliderBuilder::ball(radius),
            };

            let (body_handle, _) = self.world.insert(body_builder, collider_builder);
            self.body_handles.insert(object.id, body_handle);
        }

        self.configs = current;
    }

    pub fn step(&mut self, objects: &mut [GameObject]) {
        // Game code controls kinematic positions, so push them into Rapier first.
        for object in objects.iter() {
            let Some(handle) = self.body_handles.get(&object.id) else {
                continue;
            };

            if object
                .physics
                .is_some_and(|config| matches!(config.body, PhysicsBody::Kinematic))
            {
                if let Some(body) = self.world.bodies.get_mut(*handle) {
                    body.set_next_kinematic_translation(Vector::new(object.x, object.y));
                }
            }
        }

        self.world.step();

        // Rapier controls dynamic positions, so copy them back after stepping.
        for object in objects.iter_mut() {
            let Some(handle) = self.body_handles.get(&object.id) else {
                continue;
            };

            if object
                .physics
                .is_some_and(|config| matches!(config.body, PhysicsBody::Dynamic))
            {
                if let Some(body) = self.world.bodies.get(*handle) {
                    let position = body.translation();
                    object.x = position.x;
                    object.y = position.y;
                }
            }
        }
    }
}

