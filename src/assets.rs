use std::path::Path;

pub const ROOT: &str = "assets";

// extension for text scripts and the one place to change it
pub const TEXT_SCRIPT_EXT: &str = "spt";

fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn is_image(path: &Path) -> bool {
    matches!(extension(path).as_str(), "png" | "jpg" | "jpeg")
}

pub fn is_prefab(path: &Path) -> bool {
    extension(path) == "prefab"
}

pub fn is_text_script(path: &Path) -> bool {
    extension(path) == TEXT_SCRIPT_EXT
}

pub fn is_scene(path: &Path) -> bool {
    extension(path) == "scene"
}

pub fn is_vscript(path: &Path) -> bool {
    extension(path) == "vscript"
}

pub fn image_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy().replace('\\', "/"))
}
