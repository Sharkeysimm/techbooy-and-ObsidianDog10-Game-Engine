use eframe::egui::{self, Key};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::content_browser::ContentBrowser;
use crate::game_object::GameObject;
use crate::gizmo::Tool;
use crate::hierarchy::HierarchyState;
use crate::physics::PhysicsSystem;
use crate::scene::Scene;
use crate::storage;
use crate::script::{MouseButton, ScriptRuntime};
use crate::theme;
use crate::viewport::{ViewMode, ViewportState};
use crate::{drag, hierarchy, inspector, viewport};

enum SceneAction {
    New,
    Open(PathBuf),
}

pub struct Editor {
    scene: Scene,
    browser: ContentBrowser,
    physics: PhysicsSystem,
    scripts: ScriptRuntime,
    viewport: ViewportState,
    hierarchy: HierarchyState,
    snapshot: Option<(Vec<GameObject>, u64)>,
    scene_path: Option<PathBuf>,
    saved_json: String,
    pending_action: Option<SceneAction>,
    themed: bool,
    show_about: bool,
    last_frame: Instant,
    physics_accumulator: f32,
    is_playing: bool,
}

impl Editor {
    pub fn new() -> Self {
        let scene = Scene::new();
        let saved_json = storage::scene_json(&scene);

        Self {
            scene,
            browser: ContentBrowser::new(),
            physics: PhysicsSystem::new(),
            scripts: ScriptRuntime::new(),
            viewport: ViewportState::new(),
            hierarchy: HierarchyState::default(),
            snapshot: None,
            scene_path: None,
            saved_json,
            pending_action: None,
            themed: false,
            show_about: false,
            last_frame: Instant::now(),
            physics_accumulator: 0.0,
            is_playing: false,
        }
    }

    // stopping restores the scene to how it was when play was pressed
    fn scene_name(&self) -> String {
        self.scene_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    fn is_dirty(&self) -> bool {
        storage::scene_json(&self.scene) != self.saved_json
    }

    fn save_scene(&mut self) {
        if self.is_playing {
            return;
        }

        let result = match &self.scene_path {
            Some(path) => storage::write_scene(&self.scene, path).map(|_| path.clone()),
            None => storage::save_scene_as(&self.scene, self.browser.current_dir(), "New Scene"),
        };

        match result {
            Ok(path) => {
                self.scene.log(format!("saved {}", path.display()));
                self.scene_path = Some(path);
                self.saved_json = storage::scene_json(&self.scene);
                self.browser.refresh();
            }
            Err(error) => self.scene.log(format!("could not save scene: {error}")),
        }
    }

    // switching scenes asks first when there are unsaved changes
    fn request_scene(&mut self, action: SceneAction) {
        if self.is_playing {
            return;
        }

        if self.is_dirty() {
            self.pending_action = Some(action);
        } else {
            self.perform_scene(action);
        }
    }

    fn perform_scene(&mut self, action: SceneAction) {
        match action {
            SceneAction::New => {
                self.scene = Scene::blank();
                self.scene_path = None;
            }
            SceneAction::Open(path) => match storage::load_scene(&path) {
                Some(scene) => {
                    self.scene = scene;
                    self.scene_path = Some(path);
                }
                None => {
                    self.scene
                        .log(format!("could not open {}", path.display()));
                    return;
                }
            },
        }

        self.saved_json = storage::scene_json(&self.scene);
        self.physics.reset();
        self.scripts.reset();
    }

    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(action) = self.pending_action.take() else {
            return;
        };

        let name = self.scene_name();
        let mut choice: Option<u8> = None;

        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Save changes to {name}?"));
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        choice = Some(0);
                    }
                    if ui.button("Don't Save").clicked() {
                        choice = Some(1);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(2);
                    }
                });
            });

        match choice {
            Some(0) => {
                self.save_scene();
                self.perform_scene(action);
            }
            Some(1) => self.perform_scene(action),
            Some(_) => {}
            None => self.pending_action = Some(action),
        }
    }

    fn toggle_play(&mut self) {
        self.physics.reset();
        self.scripts.reset();
        self.scene.kicks.clear();
        self.physics_accumulator = 0.0;

        if self.is_playing {
            self.is_playing = false;

            if let Some((objects, next_id)) = self.snapshot.take() {
                self.scene.restore(objects, next_id);
            }
            if let Some(id) = self.scene.selected {
                if self.scene.get(id).is_none() {
                    self.scene.selected = None;
                }
            }
        } else {
            self.snapshot = Some((self.scene.objects.clone(), self.scene.next_id()));
            self.scene.log.clear();
            self.is_playing = true;
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        // only the scene and game views use these keys
        let in_view = matches!(self.viewport.mode, ViewMode::Scene | ViewMode::Both);
        if !in_view
            || self.is_playing
            || self.viewport.capturing_key()
            || ctx.wants_keyboard_input()
        {
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

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;

        egui::Window::new("About Sprocket")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new("Sprocket")
                        .size(22.0)
                        .strong()
                        .color(theme::ACCENT),
                );
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("A small 2D game engine written in Rust.");
            });

        self.show_about = open;
    }

    fn open_code(&mut self, path: PathBuf) {
        if !self.viewport.open_code_file(path.clone()) {
            self.scene
                .log(format!("could not open {}", path.display()));
        }
    }

    pub fn ui(&mut self, ctx: &egui::Context) {
        if !self.themed {
            theme::apply(ctx);
            self.themed = true;
        }

        if ctx.input(|i| i.key_pressed(Key::F11)) {
            let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!fullscreen));
        }

        // the code editor uses its own ctrl s for the open file
        let code_open = self.viewport.mode == ViewMode::Code;
        if !code_open && ctx.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            self.save_scene();
        }

        self.shortcuts(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("SPROCKET").strong().color(theme::ACCENT));
                ui.separator();

                ui.menu_button("File", |ui| {
                    ui.add_enabled_ui(!self.is_playing, |ui| {
                        if ui.button("New Scene").clicked() {
                            self.request_scene(SceneAction::New);
                            ui.close_menu();
                        }
                        if ui.button("Save Scene (Ctrl+S)").clicked() {
                            self.save_scene();
                            ui.close_menu();
                        }
                        if ui.button("Save Scene As").clicked() {
                            self.scene_path = None;
                            self.save_scene();
                            ui.close_menu();
                        }
                    });
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About Sprocket").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
                ui.label(self.scene_name());
                ui.separator();

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
        if let Some(path) = self.browser.take_code_open_request() {
            self.open_code(path);
        }
        if let Some(path) = self.scene.open_text.take() {
            self.open_code(path);
        }
        self.about_window(ctx);
        if let Some(path) = self.browser.take_scene_open_request() {
            self.request_scene(SceneAction::Open(path));
        }
        self.unsaved_dialog(ctx);
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
            // clicks only count when the pointer is over a view
            let mouse: HashSet<MouseButton> = if self.scene.mouse_in_view {
                ctx.input(|i| {
                    let mut buttons = HashSet::new();
                    if i.pointer.primary_down() {
                        buttons.insert(MouseButton::Left);
                    }
                    if i.pointer.secondary_down() {
                        buttons.insert(MouseButton::Right);
                    }
                    if i.pointer.middle_down() {
                        buttons.insert(MouseButton::Middle);
                    }
                    buttons
                })
            } else {
                HashSet::new()
            };
            self.scripts.update(&mut self.scene, held, mouse, frame_time);

            // a script asked to change scene so swap it in and start over
            if let Some(path) = self.scene.pending_scene.take() {
                match storage::load_scene(Path::new(&path)) {
                    Some(next) => {
                        let log = std::mem::take(&mut self.scene.log);
                        self.scene = next;
                        self.scene.log = log;
                        self.physics.reset();
                        self.scripts.reset();
                        self.physics_accumulator = 0.0;
                    }
                    None => self.scene.log(format!("could not load scene '{path}'")),
                }
            }

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
