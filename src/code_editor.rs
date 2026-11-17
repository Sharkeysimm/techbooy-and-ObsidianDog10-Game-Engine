use eframe::egui::{self, Align, Color32, FontId, Key, Layout, RichText};
use std::fs;
use std::path::PathBuf;

const AUTOSAVE_DELAY: f64 = 0.8;

struct OpenFile {
    path: PathBuf,
    text: String,
    saved: String,
    last_edit: f64,
}

impl OpenFile {
    fn dirty(&self) -> bool {
        self.text != self.saved
    }

    fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    // a file deleted from the content browser is not recreated
    fn save(&mut self) {
        if self.path.exists() && fs::write(&self.path, &self.text).is_ok() {
            self.saved = self.text.clone();
        }
    }
}

pub struct CodeEditor {
    files: Vec<OpenFile>,
    active: usize,
    font_size: f32,
    cursor: (usize, usize),
}

impl Default for CodeEditor {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            active: 0,
            font_size: 14.0,
            cursor: (1, 1),
        }
    }
}

impl CodeEditor {
    pub fn open(&mut self, path: PathBuf) -> bool {
        if let Some(index) = self.files.iter().position(|f| f.path == path) {
            self.active = index;
            return true;
        }

        match fs::read_to_string(&path) {
            Ok(text) => {
                self.files.push(OpenFile {
                    path,
                    saved: text.clone(),
                    text,
                    last_edit: 0.0,
                });
                self.active = self.files.len() - 1;
                true
            }
            Err(_) => false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let now = ui.input(|i| i.time);

        if self.files.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    "Double-click a script in the content browser to open it here.\n\
                     Right-click the content browser and choose New Script to make one.",
                );
            });
            return;
        }

        self.active = self.active.min(self.files.len() - 1);

        if ui.input(|i| i.modifiers.command && i.key_pressed(Key::S)) {
            self.files[self.active].save();
        }

        let mut close: Option<usize> = None;
        ui.horizontal(|ui| {
            for (index, file) in self.files.iter().enumerate() {
                let label = if file.dirty() {
                    format!("{} *", file.name())
                } else {
                    file.name()
                };

                if ui.selectable_label(index == self.active, label).clicked() {
                    self.active = index;
                }
                if ui.small_button("x").clicked() {
                    close = Some(index);
                }
                ui.separator();
            }
        });
        ui.separator();

        if let Some(index) = close {
            let mut file = self.files.remove(index);
            file.save();

            if self.files.is_empty() {
                return;
            }
            self.active = self.active.min(self.files.len() - 1);
        }

        let (path_text, lines, dirty) = {
            let file = &self.files[self.active];
            (
                file.path.display().to_string(),
                file.text.split('\n').count(),
                file.dirty(),
            )
        };
        let cursor = self.cursor;

        egui::TopBottomPanel::bottom("code_status").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(path_text);
                ui.separator();
                ui.label(format!("Ln {}, Col {}", cursor.0, cursor.1));
                ui.label(format!("{lines} lines"));

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("A+").clicked() {
                        self.font_size = (self.font_size + 1.0).min(32.0);
                    }
                    if ui.small_button("A-").clicked() {
                        self.font_size = (self.font_size - 1.0).max(8.0);
                    }
                    ui.label(if dirty { "unsaved" } else { "saved" });
                });
            });
        });

        let mut edited = false;

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let font = FontId::monospace(self.font_size);
            let file = &mut self.files[self.active];

            let line_count = file.text.split('\n').count().max(1);
            let width = line_count.to_string().len();
            let gutter: String = (1..=line_count)
                .map(|n| format!("{n:>width$}"))
                .collect::<Vec<_>>()
                .join("\n");

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.label(
                            RichText::new(gutter)
                                .font(font.clone())
                                .color(Color32::from_white_alpha(70)),
                        );
                        ui.add_space(8.0);

                        let output = egui::TextEdit::multiline(&mut file.text)
                            .code_editor()
                            .font(font.clone())
                            .frame(false)
                            .margin(egui::Margin::ZERO)
                            .desired_width(f32::INFINITY)
                            .desired_rows(24)
                            .show(ui);

                        if output.response.changed() {
                            edited = true;
                        }

                        if let Some(range) = output.cursor_range {
                            let before: String = file.text.chars().take(range.primary.ccursor.index).collect();
                            let row = before.matches('\n').count() + 1;
                            let column = before
                                .rsplit('\n')
                                .next()
                                .map(|line| line.chars().count())
                                .unwrap_or(0)
                                + 1;
                            self.cursor = (row, column);
                        }
                    });
                });
        });

        // saved a moment after typing stops
        if edited {
            self.files[self.active].last_edit = now;
        }
        for file in &mut self.files {
            if file.dirty() && now - file.last_edit > AUTOSAVE_DELAY {
                file.save();
            }
        }
    }
}
