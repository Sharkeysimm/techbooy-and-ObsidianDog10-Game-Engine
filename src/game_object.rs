use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::script::ScriptGraph;

// size in pixels of a shape at scale 1
pub const BASE: f32 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Square,
    Circle,
    Capsule,
    Triangle,
    Diamond,
    Pentagon,
    Hexagon,
}

impl Shape {
    pub const ALL: [Shape; 7] = [
        Shape::Square,
        Shape::Circle,
        Shape::Capsule,
        Shape::Triangle,
        Shape::Diamond,
        Shape::Pentagon,
        Shape::Hexagon,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TextureFilter {
    Nearest,
    Linear,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BodyType {
    Dynamic,
    Kinematic,
    Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ColliderShape {
    Box { size_x: f32, size_y: f32 },
    Circle { radius: f32 },
    Capsule { width: f32, height: f32 },
    Polygon { sides: u32, radius: f32 },
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Transform {
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct ShapeRenderer {
    pub shape: Shape,
    #[serde(with = "crate::serde_util::color")]
    pub color: Color32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SpriteRenderer {
    pub texture: String,
    pub filter: TextureFilter,
    #[serde(with = "crate::serde_util::color")]
    pub tint: Color32,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Camera2D {
    pub size: f32,
    #[serde(with = "crate::serde_util::color")]
    pub background: Color32,
}

impl Default for Camera2D {
    fn default() -> Self {
        Self {
            size: 4.0,
            background: Color32::from_rgb(30, 34, 44),
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Rigidbody2D {
    pub body: BodyType,
    pub gravity_scale: f32,
    #[serde(default)]
    pub freeze_x: bool,
    #[serde(default)]
    pub freeze_y: bool,
    pub lock_rotation: bool,
}

impl Default for Rigidbody2D {
    fn default() -> Self {
        Self {
            body: BodyType::Dynamic,
            gravity_scale: 1.0,
            freeze_x: false,
            freeze_y: false,
            lock_rotation: false,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Collider2D {
    pub shape: ColliderShape,
    pub bounciness: f32,
    #[serde(default)]
    pub is_trigger: bool,
}

impl Collider2D {
    pub fn with(shape: ColliderShape) -> Self {
        Self {
            shape,
            bounciness: 0.0,
            is_trigger: false,
        }
    }

    pub fn boxed() -> Self {
        Self::with(ColliderShape::Box {
            size_x: 1.0,
            size_y: 1.0,
        })
    }

    pub fn circle() -> Self {
        Self::with(ColliderShape::Circle { radius: 0.5 })
    }

    pub fn capsule() -> Self {
        Self::with(ColliderShape::Capsule {
            width: 0.5,
            height: 1.0,
        })
    }

    pub fn polygon(sides: u32) -> Self {
        Self::with(ColliderShape::Polygon {
            sides,
            radius: 0.5,
        })
    }

    // collider that matches the outline of a renderer shape
    pub fn for_shape(shape: Shape) -> Self {
        match shape {
            Shape::Square => Self::boxed(),
            Shape::Circle => Self::circle(),
            Shape::Capsule => Self::capsule(),
            Shape::Triangle => Self::polygon(3),
            Shape::Diamond => Self::polygon(4),
            Shape::Pentagon => Self::polygon(5),
            Shape::Hexagon => Self::polygon(6),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GameObject {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub parent: Option<u64>,
    pub transform: Transform,
    pub shape_renderer: Option<ShapeRenderer>,
    pub sprite_renderer: Option<SpriteRenderer>,
    pub camera: Option<Camera2D>,
    pub rigidbody: Option<Rigidbody2D>,
    pub collider: Option<Collider2D>,
    pub script: Option<ScriptGraph>,
}

impl GameObject {
    pub fn empty() -> Self {
        Self {
            id: 0,
            name: "GameObject".into(),
            visible: true,
            parent: None,
            transform: Transform::default(),
            shape_renderer: None,
            sprite_renderer: None,
            camera: None,
            rigidbody: None,
            collider: None,
            script: None,
        }
    }

    pub fn from_shape(shape: Shape) -> Self {
        let color = match shape {
            Shape::Circle => Color32::from_rgb(140, 200, 255),
            _ => Color32::WHITE,
        };

        Self {
            name: format!("{shape:?}"),
            shape_renderer: Some(ShapeRenderer { shape, color }),
            ..Self::empty()
        }
    }

    pub fn camera() -> Self {
        Self {
            name: "Camera".into(),
            camera: Some(Camera2D::default()),
            ..Self::empty()
        }
    }

    pub fn sprite(name: String, texture: String, x: f32, y: f32) -> Self {
        Self {
            name,
            transform: Transform {
                x,
                y,
                ..Transform::default()
            },
            sprite_renderer: Some(SpriteRenderer {
                texture,
                filter: TextureFilter::Nearest,
                tint: Color32::WHITE,
            }),
            ..Self::empty()
        }
    }
}
