pub struct GameObject {
    pub id: u64,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub visible: bool,
    pub parent: Option<u64>,
    pub texture: Option<String>,
}

impl GameObject {
    pub fn square() -> Self {
        Self {
            id: 0,
            name: "Square".into(),
            x: 0.0,
            y: 0.0,
            size: 50.0,
            visible: true,
            parent: None,
            texture: None,
        }
    }

    pub fn sprite(name: String, texture: String, x: f32, y: f32) -> Self {
        Self {
            name,
            x,
            y,
            size: 100.0,
            texture: Some(texture),
            ..Self::square()
        }
    }
}
