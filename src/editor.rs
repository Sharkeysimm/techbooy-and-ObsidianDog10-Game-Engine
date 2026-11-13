use eframe::egui;

use crate::content_browser::ContentBrowser;
use crate::scene::Scene;
use crate::{drag, hierarchy, inspector, viewport};

pub struct Editor {
    scene: Scene,
    browser: ContentBrowser,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            scene: Scene::new(),
            browser: ContentBrowser::new(),
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        // bottom panel goes first so it spans the full window width
        self.browser.show(ctx);
        hierarchy::show(ctx, &mut self.scene);
        inspector::show(ctx, &mut self.scene);
        viewport::show(ctx, &mut self.scene);
        drag::paint_ghost(ctx, &self.scene);
    }
}
