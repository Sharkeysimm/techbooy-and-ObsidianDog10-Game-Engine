use eframe::egui::{self, Key};
use std::collections::HashSet;
use std::time::Instant;

use crate::content_browser::ContentBrowser;
use crate::game_object::GameObject;
use crate::gizmo::Tool;
use crate::hierarchy::HierarchyState;
use crate::physics::PhysicsSystem;
use crate::scene::Scene;
use crate::storage;
use crate::script::ScriptRuntime;
use crate::viewport::ViewportState;
use crate::{drag, hierarchy, inspector, viewport};

pub struct Editor {
    scene: Scene,
    browser: ContentBrowser,
    physics: PhysicsSystem,
    scripts: ScriptRuntime,
    viewport: ViewportState,
    hierarchy: HierarchyState,
    snapshot: Option<Vec<GameObject>>,
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
            scripts: ScriptRuntime::new(),
            viewport: ViewportState::new(),
            hierarchy: HierarchyState::default(),
            snapshot: None,
            last_frame: Instant::now(),
            physics_accumulator: 0.0,
            is_playing: false,
        }
    }

    // stopping restores the scene to how it was when play was pressed
    fn toggle_play(&mut self) {
        self.physics.reset();
        self.scripts.reset();
        self.scene.kicks.clear();
        self.physics_accumulator = 0.0;

        if self.is_playing {
            self.is_playing = false;

            if let Some(objects) = self.snapshot.take() {
                self.scene.objects = objects;
            }
            if let Some(id) = self.scene.selected {
                if self.scene.get(id).is_none() {
                    self.scene.selected = None;
                }
            }
        } else {
            self.snapshot = Some(self.scene.objects.clone());
            self.scene.log.clear();
            self.is_playing = true;
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.is_playing || self.viewport.capturing_key() || ctx.wants_keyboard_input() {
            return;
        }

        let (w, e, r, delete) = ctx.input(|i| {
            (
                i.key_pressed(Key::W),
                i.key_pressed(Key::E),
                i.key_pressed(Key::R),
                i.key_pressed(Key::Delete),
            )
        });

        if w {
            self.viewport.tool = Tool::Move;
        }
        if e {
            self.viewport.tool = Tool::Rotate;
        }
        if r {
            self.viewport.tool = Tool::Scale;
        }
        if delete && !self.is_playing {
            if let Some(id) = self.scene.selected {
                self.scene.remove(id);
            }
        }
    }

    // saving needs the scene and the folder the content browser is showing
    fn handle_saves(&mut self) {
        if let Some(id) = self.browser.take_prefab_request() {
            let result = storage::save_prefab(&self.scene, id, self.browser.current_dir());
            self.report_save(result);
        }

        if self.viewport.take_script_save() {
            let saved = self
                .scene
                .selected
                .and_then(|id| self.scene.get(id))
                .and_then(|object| {
                    object.script.as_ref().map(|graph| {
                        storage::save_script(graph, &object.name, self.browser.current_dir())
                    })
                });

            if let Some(result) = saved {
                self.report_save(result);
            }
        }
    }

    fn report_save(&mut self, result: std::io::Result<std::path::PathBuf>) {
        match result {
            Ok(path) => self.scene.log(format!("saved {}", path.display())),
            Err(error) => self.scene.log(format!("could not save: {error}")),
        }
        self.browser.refresh();
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(Key::F11)) {
            let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }

        self.shortcuts(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let label = if self.is_playing { "Stop" } else { "Play" };
                if ui.button(label).clicked() {
                    self.toggle_play();
                }

                ui.separator();
                ui.selectable_value(&mut self.viewport.tool, Tool::Move, "Move (W)");
                ui.selectable_value(&mut self.viewport.tool, Tool::Rotate, "Rotate (E)");
                ui.selectable_value(&mut self.viewport.tool, Tool::Scale, "Scale (R)");

                ui.menu_button("Snap (hold Ctrl)", |ui| {
                    let snap = &mut self.viewport.snap;
                    ui.horizontal(|ui| {
                        ui.label("Move");
                        ui.add(egui::DragValue::new(&mut snap.move_step).speed(1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Rotate");
                        ui.add(egui::DragValue::new(&mut snap.rotate_step).speed(0.5));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Scale");
                        ui.add(egui::DragValue::new(&mut snap.scale_step).speed(0.01));
                    });
                });

                ui.separator();
                ui.checkbox(&mut self.viewport.show_grid, "Grid");
                ui.checkbox(&mut self.viewport.show_colliders, "Colliders");
            });
        });

        self.browser.show(ctx);
        if let Some(path) = self.browser.take_script_open_request() {
            match storage::load_script(&path) {
                Some(graph) => self.viewport.open_script_asset(path, graph),
                None => self.scene.log(format!("could not open {}", path.display())),
            }
        }
        hierarchy::show(ctx, &mut self.scene, &mut self.hierarchy);
        inspector::show(ctx, &mut self.scene, &mut self.viewport.mode);

        let now = Instant::now();
        let frame_time = (now - self.last_frame).as_secs_f32().min(0.25);
        self.last_frame = now;

        const FIXED_STEP: f32 = 1.0 / 60.0;
        const MAX_STEPS_PER_FRAME: usize = 5;

        if self.is_playing {
            // typing in a text field should not drive the game
            let held = if ctx.wants_keyboard_input() {
                HashSet::new()
            } else {
                ctx.input(|i| i.keys_down.clone())
            };
            self.scripts.update(&mut self.scene, held, frame_time);

            self.physics_accumulator += frame_time;

            let mut steps = 0;
            while self.physics_accumulator >= FIXED_STEP && steps < MAX_STEPS_PER_FRAME {
                self.physics.step(&mut self.scene);
                self.physics_accumulator -= FIXED_STEP;
                steps += 1;
            }
        }

        viewport::show(ctx, &mut self.scene, &mut self.viewport, self.is_playing);
        self.handle_saves();
        drag::paint_ghost(ctx, &self.scene);
        ctx.request_repaint();
    }
}
