use eframe::egui::{self, Color32, DragAndDrop, FontId, Id, LayerId, Order, Rect};
use std::path::PathBuf;

use crate::scene::Scene;

pub struct AssetPayload {
    pub path: PathBuf,
}

pub struct ObjectPayload {
    pub id: u64,
}

pub fn paint_ghost(ctx: &egui::Context, scene: &Scene) {
    let label = if let Some(payload) = DragAndDrop::payload::<AssetPayload>(ctx) {
        payload
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
    } else if let Some(payload) = DragAndDrop::payload::<ObjectPayload>(ctx) {
        scene.get(payload.id).map(|o| o.name.clone())
    } else {
        None
    };

    let (Some(label), Some(pointer)) = (label, ctx.pointer_latest_pos()) else {
        return;
    };

    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);

    let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("drag_ghost")));
    let galley = painter.layout_no_wrap(label, FontId::proportional(13.0), Color32::WHITE);
    let rect = Rect::from_min_size(pointer + egui::vec2(14.0, 14.0), galley.size()).expand(4.0);

    painter.rect_filled(rect, 4.0, Color32::from_black_alpha(200));
    painter.galley(rect.min + egui::vec2(4.0, 4.0), galley, Color32::WHITE);
}
