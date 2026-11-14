use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::path::PathBuf;

use crate::assets::{image_uri, is_image};
use crate::drag::AssetPayload;
use crate::game_object::GameObject;
use crate::scene::Scene;

pub fn show(ctx: &egui::Context, scene: &mut Scene) {
    let mut spawn: Option<(PathBuf, Vec2)> = None;

    egui::CentralPanel::default().show(ctx, |ui| {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::hover());
        let center = response.rect.center();

        painter.rect_filled(response.rect, 0.0, Color32::BLACK);

        for id in scene.draw_order() {
            let Some(object) = scene.get(id) else {
                continue;
            };
            if !scene.is_visible(id) {
                continue;
            }

            let pos = center + scene.world_position(id);
            let size = egui::vec2(object.size, object.size);

            let bounds = match &object.texture {
                Some(uri) => draw_texture(ui, uri, pos, size),
                None => {
                    let rect = Rect::from_center_size(pos, size);
                    painter.rect_filled(rect, 0.0, Color32::WHITE);
                    rect
                }
            };

            if scene.selected == Some(id) {
                let outline = vec![
                    bounds.left_top(),
                    bounds.right_top(),
                    bounds.right_bottom(),
                    bounds.left_bottom(),
                ];
                painter.add(egui::Shape::closed_line(
                    outline,
                    Stroke::new(1.5_f32, Color32::from_rgb(255, 200, 60)),
                ));
            }
        }

        let hovering_image = response
            .dnd_hover_payload::<AssetPayload>()
            .is_some_and(|p| is_image(&p.path));
        if hovering_image {
            painter.rect_filled(
                response.rect,
                0.0,
                Color32::from_rgba_unmultiplied(70, 130, 220, 40),
            );
        }

        if let Some(payload) = response.dnd_release_payload::<AssetPayload>() {
            let pointer = ui.input(|i| i.pointer.latest_pos());
            if let (true, Some(pointer)) = (is_image(&payload.path), pointer) {
                spawn = Some((payload.path.clone(), pointer - center));
            }
        }
    });

    if let Some((path, offset)) = spawn {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Sprite".into());

        let id = scene.add(GameObject::sprite(name, image_uri(&path), offset.x, offset.y));
        scene.selected = Some(id);
    }
}

fn draw_texture(ui: &egui::Ui, uri: &str, pos: Pos2, size: Vec2) -> Rect {
    let image = egui::Image::new(uri).fit_to_exact_size(size);
    let fitted = image.load_and_calc_size(ui, size).unwrap_or(size);
    let rect = Rect::from_center_size(pos, fitted);

    image.paint_at(ui, rect);
    rect
}
