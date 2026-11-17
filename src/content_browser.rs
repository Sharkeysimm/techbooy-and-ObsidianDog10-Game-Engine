use eframe::egui::{
    self, Align, Align2, Color32, FontId, Id, Key, LayerId, Layout, Order, Rect, Sense,
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::assets::{image_uri, is_image, is_prefab, is_scene, is_vscript};
use crate::drag::{AssetPayload, ObjectPayload};
use crate::script::ScriptGraph;
use crate::storage;

const ROOT: &str = "assets";
const ROOT_LABEL: &str = "Content";

struct Entry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

struct FolderNode {
    name: String,
    path: PathBuf,
    children: Vec<FolderNode>,
}

struct Rename {
    path: PathBuf,
    buffer: String,
    focused: bool,
}

enum RenameOutcome {
    Commit,
    Cancel,
}

pub struct ContentBrowser {
    root: PathBuf,
    current: PathBuf,
    selected: Option<PathBuf>,
    pending_delete: Option<PathBuf>,
    prefab_request: Option<u64>,
    script_open_request: Option<PathBuf>,
    scene_open_request: Option<PathBuf>,
    renaming: Option<Rename>,
    search: String,
    icon_size: f32,
    entries: Vec<Entry>,
    tree: FolderNode,
    loaders_installed: bool,
}

impl ContentBrowser {
    pub fn new() -> Self {
        let root = PathBuf::from(ROOT);
        let _ = fs::create_dir_all(&root);

        let mut browser = Self {
            current: root.clone(),
            tree: build_root(&root),
            root,
            selected: None,
            pending_delete: None,
            prefab_request: None,
            script_open_request: None,
            scene_open_request: None,
            renaming: None,
            search: String::new(),
            icon_size: 72.0,
            entries: Vec::new(),
            loaders_installed: false,
        };
        browser.refresh();
        browser
    }

    pub fn current_dir(&self) -> &Path {
        &self.current
    }

    pub fn take_scene_open_request(&mut self) -> Option<PathBuf> {
        self.scene_open_request.take()
    }

    pub fn take_script_open_request(&mut self) -> Option<PathBuf> {
        self.script_open_request.take()
    }

    pub fn take_prefab_request(&mut self) -> Option<u64> {
        self.prefab_request.take()
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.loaders_installed {
            egui_extras::install_image_loaders(ctx);
            self.loaders_installed = true;
        }

        self.import_dropped(ctx);

        let f2 = ctx.input(|i| i.key_pressed(Key::F2));
        if f2 && self.renaming.is_none() && !ctx.wants_keyboard_input() {
            if let Some(path) = self.selected.clone() {
                self.start_rename(path);
            }
        }

        let panel = egui::TopBottomPanel::bottom("content_browser")
            .resizable(true)
            .default_height(240.0)
            .min_height(120.0)
            .show(ctx, |ui| {
                self.toolbar(ui);
                ui.separator();

                let mut navigate: Option<PathBuf> = None;
                let mut delete: Option<PathBuf> = None;
                let mut rename_request: Option<PathBuf> = None;
                let mut rename_result: Option<RenameOutcome> = None;
                let mut new_folder = false;
                let mut new_script = false;
                let mut open_script: Option<PathBuf> = None;
                let mut open_scene: Option<PathBuf> = None;
                let mut new_scene = false;
                let mut refresh = false;
                let mut prefab_drop: Option<u64> = None;

                egui::SidePanel::left("folder_tree")
                    .resizable(true)
                    .default_width(160.0)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::both().show(ui, |ui| {
                            draw_tree(ui, &self.tree, &self.current, 0, &mut navigate);
                        });
                    });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    // background is added first so entries sit on top of it
                    let background =
                        ui.interact(ui.max_rect(), ui.id().with("background"), Sense::click());
                    background.context_menu(|ui| {
                        if ui.button("New Folder").clicked() {
                            new_folder = true;
                            ui.close_menu();
                        }
                        if ui.button("New Visual Script").clicked() {
                            new_script = true;
                            ui.close_menu();
                        }
                        if ui.button("New Scene").clicked() {
                            new_scene = true;
                            ui.close_menu();
                        }
                        if ui.button("Refresh").clicked() {
                            refresh = true;
                            ui.close_menu();
                        }
                    });

                    if background.dnd_hover_payload::<ObjectPayload>().is_some() {
                        let rect = ui.max_rect();
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            Color32::from_rgba_unmultiplied(70, 130, 220, 40),
                        );
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            "Drop to save as prefab",
                            FontId::proportional(16.0),
                            Color32::WHITE,
                        );
                    }
                    if let Some(payload) = background.dnd_release_payload::<ObjectPayload>() {
                        prefab_drop = Some(payload.id);
                    }

                    let filter = self.search.to_lowercase();

                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .drag_to_scroll(false)
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);

                                let visible = self.entries.iter().filter(|e| {
                                    filter.is_empty() || e.name.to_lowercase().contains(&filter)
                                });

                                for entry in visible {
                                    let is_selected = self.selected.as_ref() == Some(&entry.path);
                                    let rename = self
                                        .renaming
                                        .as_mut()
                                        .filter(|r| r.path == entry.path);

                                    let (response, outcome) =
                                        draw_entry(ui, entry, self.icon_size, is_selected, rename);

                                    if outcome.is_some() {
                                        rename_result = outcome;
                                    }
                                    if response.clicked() {
                                        self.selected = Some(entry.path.clone());
                                    }
                                    if is_image(&entry.path)
                                        || is_prefab(&entry.path)
                                        || is_vscript(&entry.path)
                                    {
                                        response.dnd_set_drag_payload(AssetPayload {
                                            path: entry.path.clone(),
                                        });
                                    }
                                    if response.double_clicked() {
                                        if entry.is_dir {
                                            navigate = Some(entry.path.clone());
                                        } else if is_vscript(&entry.path) {
                                            open_script = Some(entry.path.clone());
                                        } else if is_scene(&entry.path) {
                                            open_scene = Some(entry.path.clone());
                                        }
                                    }

                                    response.context_menu(|ui| {
                                        let openable = entry.is_dir
                                            || is_vscript(&entry.path)
                                            || is_scene(&entry.path);
                                        if openable && ui.button("Open").clicked() {
                                            if entry.is_dir {
                                                navigate = Some(entry.path.clone());
                                            } else if is_scene(&entry.path) {
                                                open_scene = Some(entry.path.clone());
                                            } else {
                                                open_script = Some(entry.path.clone());
                                            }
                                            ui.close_menu();
                                        }
                                        if ui.button("Rename").clicked() {
                                            rename_request = Some(entry.path.clone());
                                            ui.close_menu();
                                        }
                                        if ui.button("Delete").clicked() {
                                            delete = Some(entry.path.clone());
                                            ui.close_menu();
                                        }
                                    });
                                }
                            });
                        });
                });

                match rename_result {
                    Some(RenameOutcome::Commit) => self.finish_rename(true),
                    Some(RenameOutcome::Cancel) => self.finish_rename(false),
                    None => {}
                }
                if let Some(path) = navigate {
                    self.open(path);
                }
                if let Some(path) = rename_request {
                    self.start_rename(path);
                }
                if delete.is_some() {
                    self.pending_delete = delete;
                }
                if new_folder {
                    self.create_folder();
                }
                if new_script {
                    self.create_script();
                }
                if open_script.is_some() {
                    self.script_open_request = open_script;
                }
                if open_scene.is_some() {
                    self.scene_open_request = open_scene;
                }
                if new_scene {
                    self.create_scene();
                }
                if prefab_drop.is_some() {
                    self.prefab_request = prefab_drop;
                }
                if refresh {
                    ui.ctx().forget_all_images();
                    self.refresh();
                }
            });

        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let painter = ctx.layer_painter(LayerId::new(
                Order::Foreground,
                Id::new("browser_drop_overlay"),
            ));
            let rect = panel.response.rect;
            painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(70, 130, 220, 60));
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Drop files to import",
                FontId::proportional(16.0),
                Color32::WHITE,
            );
        }

        self.delete_dialog(ctx);
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let crumbs = self.breadcrumbs();

        ui.horizontal(|ui| {
            ui.menu_button("Create", |ui| {
                if ui.button("Folder").clicked() {
                    self.create_folder();
                    ui.close_menu();
                }
                if ui.button("Visual Script").clicked() {
                    self.create_script();
                    ui.close_menu();
                }
                if ui.button("Scene").clicked() {
                    self.create_scene();
                    ui.close_menu();
                }
            });

            if ui.button("Refresh").clicked() {
                ui.ctx().forget_all_images();
                self.refresh();
            }

            ui.separator();

            for (index, (name, path)) in crumbs.into_iter().enumerate() {
                if index > 0 {
                    ui.label("/");
                }
                if ui.small_button(name).clicked() {
                    self.open(path);
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(egui::Slider::new(&mut self.icon_size, 40.0..=128.0).show_value(false));
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Search")
                        .desired_width(140.0),
                );
            });
        });
    }

    fn breadcrumbs(&self) -> Vec<(String, PathBuf)> {
        let mut crumbs = vec![(self.tree.name.clone(), self.root.clone())];

        if let Ok(relative) = self.current.strip_prefix(&self.root) {
            let mut path = self.root.clone();
            for component in relative.components() {
                path.push(component);
                crumbs.push((
                    component.as_os_str().to_string_lossy().into_owned(),
                    path.clone(),
                ));
            }
        }

        crumbs
    }

    fn open(&mut self, path: PathBuf) {
        self.current = path;
        self.selected = None;
        self.renaming = None;
        self.refresh();
    }

    pub fn refresh(&mut self) {
        if !self.current.is_dir() {
            self.current = self.root.clone();
        }
        self.tree = build_root(&self.root);
        self.entries = read_entries(&self.current);
    }

    fn create_folder(&mut self) {
        let path = unique_path(&self.current, "New Folder", None);
        if fs::create_dir(&path).is_ok() {
            self.refresh();
            self.start_rename(path);
        }
    }

    fn create_scene(&mut self) {
        if let Ok(path) = storage::create_scene_file(&self.current) {
            self.refresh();
            self.start_rename(path);
        }
    }

    // new scripts start empty and go straight into rename mode
    fn create_script(&mut self) {
        if let Ok(path) = storage::save_script(&ScriptGraph::empty(), "New Script", &self.current) {
            self.refresh();
            self.start_rename(path);
        }
    }

    fn start_rename(&mut self, path: PathBuf) {
        let buffer = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        self.selected = Some(path.clone());
        self.renaming = Some(Rename {
            path,
            buffer,
            focused: false,
        });
    }

    fn finish_rename(&mut self, commit: bool) {
        let Some(rename) = self.renaming.take() else {
            return;
        };
        if !commit {
            return;
        }

        let name = rename.buffer.trim();
        let invalid = name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']);
        let Some(parent) = rename.path.parent() else {
            return;
        };
        if invalid {
            return;
        }

        let target = parent.join(name);
        if target == rename.path || target.exists() {
            return;
        }

        if fs::rename(&rename.path, &target).is_ok() {
            self.selected = Some(target);
            self.refresh();
        }
    }

    fn import_dropped(&mut self, ctx: &egui::Context) {
        let sources: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if sources.is_empty() {
            return;
        }

        for source in sources {
            let Some(name) = source.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let is_dir = source.is_dir();

            // a folder dropped into itself would copy forever
            if is_dir {
                if let (Ok(here), Ok(there)) = (self.current.canonicalize(), source.canonicalize())
                {
                    if here.starts_with(&there) {
                        continue;
                    }
                }
            }

            let (stem, extension) = if is_dir {
                (name, None)
            } else {
                let stem = source
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(name);
                let extension = source.extension().map(|e| e.to_string_lossy().into_owned());
                (stem, extension)
            };

            let target = unique_path(&self.current, &stem, extension.as_deref());
            let result = if is_dir {
                copy_dir(&source, &target)
            } else {
                fs::copy(&source, &target).map(|_| ())
            };

            if result.is_ok() {
                self.selected = Some(target);
            }
        }

        self.refresh();
    }

    fn delete_dialog(&mut self, ctx: &egui::Context) {
        let Some(path) = self.pending_delete.clone() else {
            return;
        };

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let mut confirm = false;
        let mut cancel = false;

        egui::Window::new("Delete")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("Delete {name} permanently"));
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        confirm = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });

        if confirm {
            ctx.forget_image(&image_uri(&path));
            let _ = if path.is_dir() {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            self.selected = None;
            self.refresh();
        }
        if confirm || cancel {
            self.pending_delete = None;
        }
    }
}

fn draw_tree(
    ui: &mut egui::Ui,
    node: &FolderNode,
    current: &Path,
    depth: usize,
    navigate: &mut Option<PathBuf>,
) {
    let is_current = node.path.as_path() == current;

    if node.children.is_empty() {
        if ui.selectable_label(is_current, &node.name).clicked() {
            *navigate = Some(node.path.clone());
        }
        return;
    }

    let id = ui.make_persistent_id(&node.path);
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, depth == 0)
        .show_header(ui, |ui| {
            if ui.selectable_label(is_current, &node.name).clicked() {
                *navigate = Some(node.path.clone());
            }
        })
        .body(|ui| {
            for child in &node.children {
                draw_tree(ui, child, current, depth + 1, navigate);
            }
        });
}

fn draw_entry(
    ui: &mut egui::Ui,
    entry: &Entry,
    size: f32,
    selected: bool,
    rename: Option<&mut Rename>,
) -> (egui::Response, Option<RenameOutcome>) {
    let label_height = 20.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(size, size + label_height), Sense::click_and_drag());

    let text_color = ui.visuals().text_color();
    let painter = ui.painter().clone();

    if selected {
        painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(70, 130, 220, 90));
    } else if response.hovered() {
        painter.rect_filled(rect, 4.0, Color32::from_white_alpha(14));
    }

    let icon = Rect::from_min_size(rect.min, egui::vec2(size, size)).shrink(size * 0.15);

    if entry.is_dir {
        let tab = Rect::from_min_size(
            icon.min - egui::vec2(0.0, size * 0.07),
            egui::vec2(icon.width() * 0.4, size * 0.1),
        );
        let color = Color32::from_rgb(214, 170, 70);
        painter.rect_filled(tab, 2.0, color);
        painter.rect_filled(icon, 4.0, color);
    } else if is_image(Path::new(&entry.name)) {
        painter.rect_filled(icon, 4.0, Color32::from_gray(30));

        let image = egui::Image::new(image_uri(&entry.path)).fit_to_exact_size(icon.size());
        let fitted = image
            .load_and_calc_size(ui, icon.size())
            .unwrap_or(icon.size());
        image.paint_at(ui, Rect::from_center_size(icon.center(), fitted));
    } else {
        painter.rect_filled(icon, 4.0, tile_color(&entry.path));

        let extension = Path::new(&entry.name)
            .extension()
            .map(|e| e.to_string_lossy().to_uppercase())
            .unwrap_or_default();
        painter.text(
            icon.center(),
            Align2::CENTER_CENTER,
            extension,
            FontId::proportional(size * 0.2),
            Color32::WHITE,
        );
    }

    let label_rect = Rect::from_min_size(
        egui::pos2(rect.min.x, rect.min.y + size + 1.0),
        egui::vec2(size, label_height - 1.0),
    );

    let mut outcome = None;

    if let Some(rename) = rename {
        let edit = ui.put(
            label_rect,
            egui::TextEdit::singleline(&mut rename.buffer)
                .font(FontId::proportional(12.0))
                .desired_width(size),
        );

        if !rename.focused {
            edit.request_focus();
            rename.focused = true;
        } else if edit.lost_focus() {
            let escaped = ui.input(|i| i.key_pressed(Key::Escape));
            outcome = Some(if escaped {
                RenameOutcome::Cancel
            } else {
                RenameOutcome::Commit
            });
        }
    } else {
        let mut job = egui::text::LayoutJob::simple(
            entry.name.clone(),
            FontId::proportional(12.0),
            text_color,
            size,
        );
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;

        let galley = painter.layout_job(job);
        let pos = egui::pos2(
            rect.center().x - galley.size().x / 2.0,
            rect.min.y + size + 3.0,
        );
        painter.galley(pos, galley, text_color);
    }

    (response, outcome)
}

fn tile_color(path: &Path) -> Color32 {
    if is_prefab(path) {
        Color32::from_rgb(140, 95, 210)
    } else if is_vscript(path) {
        Color32::from_rgb(70, 160, 105)
    } else if is_scene(path) {
        Color32::from_rgb(60, 150, 190)
    } else {
        Color32::from_rgb(90, 96, 108)
    }
}

fn build_root(root: &Path) -> FolderNode {
    let mut node = build_tree(root);
    node.name = ROOT_LABEL.into();
    node
}

fn build_tree(path: &Path) -> FolderNode {
    let mut children: Vec<FolderNode> = fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| build_tree(&e.path()))
        .collect();
    children.sort_by_key(|c| c.name.to_lowercase());

    FolderNode {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| ROOT.into()),
        path: path.to_path_buf(),
        children,
    }
}

fn read_entries(dir: &Path) -> Vec<Entry> {
    let mut entries: Vec<Entry> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| {
            let path = e.path();
            Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                is_dir: path.is_dir(),
                path,
            }
        })
        .collect();

    // folders first then alphabetical
    entries.sort_by_key(|e| (!e.is_dir, e.name.to_lowercase()));
    entries
}

fn unique_path(dir: &Path, base: &str, extension: Option<&str>) -> PathBuf {
    let name = |n: u32| {
        let stem = if n == 0 {
            base.to_string()
        } else {
            format!("{base} {n}")
        };
        match extension {
            Some(ext) => format!("{stem}.{ext}"),
            None => stem,
        }
    };

    let mut n = 0;
    while dir.join(name(n)).exists() {
        n += 1;
    }
    dir.join(name(n))
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;

    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());

        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }

    Ok(())
}
