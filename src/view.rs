use eframe::egui::{Pos2, Vec2};

// maps world pixels to screen and back
#[derive(Clone, Copy)]
pub struct View {
    pub origin: Pos2,
    pub zoom: f32,
}

impl View {
    pub fn new(screen_center: Pos2, world_center: Vec2, zoom: f32) -> Self {
        Self {
            origin: screen_center - world_center * zoom,
            zoom,
        }
    }

    pub fn to_screen(&self, world: Vec2) -> Pos2 {
        self.origin + world * self.zoom
    }

    pub fn to_world(&self, screen: Pos2) -> Vec2 {
        (screen - self.origin) / self.zoom
    }
}
