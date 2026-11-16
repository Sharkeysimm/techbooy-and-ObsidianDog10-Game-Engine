use std::path::Path;

pub fn is_image(path: &Path) -> bool {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(extension.as_str(), "png" | "jpg" | "jpeg")
}

pub fn image_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy().replace('\\', "/"))
}
