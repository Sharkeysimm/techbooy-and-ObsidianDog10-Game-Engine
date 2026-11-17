use eframe::egui::{self, Color32};

pub const ACCENT: Color32 = Color32::from_rgb(232, 126, 38);

pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    visuals.selection.bg_fill = Color32::from_rgb(168, 92, 28);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
    visuals.hyperlink_color = ACCENT;

    ctx.set_visuals(visuals);
}
