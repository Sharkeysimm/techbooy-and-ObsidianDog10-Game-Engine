use eframe::egui;
use std::time::Instant;

use crate::content_browser::ContentBrowser;
use crate::physics::PhysicsSystem;
use crate::scene::Scene;
use crate::{drag, hierarchy, inspector, viewport};

pub struct Editor {
    scene: Scene,
    browser: ContentBrowser,
    physics: PhysicsSystem,
    last_frame: Instant,
    physics_accumulator: f32, 
    is_playing: bool, 
}

impl Editor {
    pub fn new() -> Self {
        Self {
            scene: Scene::new(),
            browser: ContentBrowser::new(),
            physics: PhysicsSystem::new(),
            last_frame: Instant::now(),
            physics_accumulator: 0.0,
            is_playing: false,
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            if ui
                .button(if self.is_playing { "Stop" } else { "Play" })
                .clicked()
            {
                self.is_playing = !self.is_playing;
                self.physics_accumulator = 0.0;
            }
        });

        self.browser.show(ctx);
        hierarchy::show(ctx, &mut self.scene);
        inspector::show(ctx, &mut self.scene, &mut self.physics);

        // Make sure physics knows about scene changes before it steps.
        self.physics.sync_objects(&self.scene.objects);

        let now = Instant::now();
        let frame_time = (now - self.last_frame).as_secs_f32().min(0.25);
        self.last_frame = now;

        const FIXED_STEP: f32 = 1.0 / 60.0;
        const MAX_STEPS_PER_FRAME: usize = 5;

        if self.is_playing {
            self.physics_accumulator += frame_time;

            let mut steps = 0;
            while self.physics_accumulator >= FIXED_STEP && steps < MAX_STEPS_PER_FRAME {
                self.physics.step(&mut self.scene.objects);
                self.physics_accumulator -= FIXED_STEP;
                steps += 1;
            }
        }

        viewport::show(ctx, &mut self.scene);
        drag::paint_ghost(ctx, &self.scene);
        ctx.request_repaint();
    }
}
