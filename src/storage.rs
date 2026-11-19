use eframe::egui::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::assets::{is_image, is_prefab, is_scene, is_text_script, ROOT, TEXT_SCRIPT_EXT};
use crate::game_object::{GameObject, HingeJoint2D, Transform};
use crate::scene::Scene;
use crate::script::ScriptGraph;

#[derive(Serialize, Deserialize)]
pub struct Prefab {
    pub root: u64,
    pub objects: Vec<GameObject>,
}

fn clean_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "Object".into()
    } else {
        cleaned
    }
}

fn unique_path(dir: &Path, stem: &str, extension: &str) -> PathBuf {
    let mut n = 0;
    loop {
        let name = if n == 0 {
            format!("{stem}.{extension}")
        } else {
            format!("{stem} {n}.{extension}")
        };
        let path = dir.join(name);
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

// saves the object and everything under it
pub fn save_prefab(scene: &Scene, root: u64, dir: &Path) -> io::Result<PathBuf> {
    let object = scene
        .get(root)
        .ok_or_else(|| io::Error::other("object no longer exists"))?;

    let mut ids = vec![root];
    let mut index = 0;
    while index < ids.len() {
        let current = ids[index];
        ids.extend(scene.children_of(Some(current)));
        index += 1;
    }

    let mut objects: Vec<GameObject> = ids.iter().filter_map(|id| scene.get(*id).cloned()).collect();

    // the root is stored in world space so it does not depend on its old parent
    let (position, rotation, scale) = scene.world_affine(root).decompose();
    if let Some(first) = objects.first_mut() {
        first.parent = None;
        first.transform = Transform {
            x: position.x,
            y: position.y,
            rotation,
            scale_x: scale.x,
            scale_y: scale.y,
        };
    }

    let path = unique_path(dir, &clean_name(&object.name), "prefab");
    let json = serde_json::to_string_pretty(&Prefab { root, objects }).map_err(io::Error::other)?;
    fs::write(&path, json)?;
    Ok(path)
}

pub fn load_prefab(path: &Path) -> Option<Prefab> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

// copies the prefab into the scene with fresh ids and returns the new root
pub fn instantiate(
    scene: &mut Scene,
    prefab: &Prefab,
    parent: Option<u64>,
    position: Option<Vec2>,
) -> Option<u64> {
    let mut ids: HashMap<u64, u64> = HashMap::new();

    for object in &prefab.objects {
        let new_id = scene.add(object.clone());
        ids.insert(object.id, new_id);
    }

    for object in &prefab.objects {
        let new_id = ids[&object.id];
        let new_parent = if object.id == prefab.root {
            parent
        } else {
            object.parent.and_then(|p| ids.get(&p).copied())
        };

        if let Some(copy) = scene.get_mut(new_id) {
            copy.parent = new_parent;

            // a hinge to something outside the prefab is dropped instead of pinned to the world
            if let Some(hinge) = copy.hinge {
                if let Some(old) = hinge.connected {
                    copy.hinge = ids.get(&old).map(|new| HingeJoint2D {
                        connected: Some(*new),
                        ..hinge
                    });
                }
            }
        }
    }

    let root = *ids.get(&prefab.root)?;
    if let Some(position) = position {
        scene.set_world_position(root, position);
    }
    Some(root)
}

#[derive(Serialize)]
struct SceneRef<'a> {
    objects: &'a [GameObject],
    next_id: u64,
}

#[derive(Deserialize)]
struct SceneFile {
    objects: Vec<GameObject>,
    next_id: u64,
}

// compact form used to tell whether a scene changed since it was saved
pub fn scene_json(scene: &Scene) -> String {
    serde_json::to_string(&scene.objects).unwrap_or_default()
}

pub fn write_scene(scene: &Scene, path: &Path) -> io::Result<()> {
    let file = SceneRef {
        objects: &scene.objects,
        next_id: scene.next_id(),
    };
    let json = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
    fs::write(path, json)
}

pub fn save_scene_as(scene: &Scene, dir: &Path, name: &str) -> io::Result<PathBuf> {
    let path = unique_path(dir, &clean_name(name), "scene");
    write_scene(scene, &path)?;
    Ok(path)
}

pub fn create_scene_file(dir: &Path) -> io::Result<PathBuf> {
    save_scene_as(&Scene::blank(), dir, "New Scene")
}

pub fn load_scene(path: &Path) -> Option<Scene> {
    let text = fs::read_to_string(path).ok()?;
    let file: SceneFile = serde_json::from_str(&text).ok()?;
    Some(Scene::from_parts(file.objects, file.next_id))
}

// text scripts start empty because the language is not decided yet
pub fn create_text_script(dir: &Path) -> io::Result<PathBuf> {
    let path = unique_path(dir, "New Script", TEXT_SCRIPT_EXT);
    fs::write(&path, "")?;
    Ok(path)
}

pub fn list_text_scripts() -> Vec<String> {
    list_matching(is_text_script)
}

pub fn list_scenes() -> Vec<String> {
    list_matching(is_scene)
}

pub fn save_script(graph: &ScriptGraph, name: &str, dir: &Path) -> io::Result<PathBuf> {
    let path = unique_path(dir, &clean_name(name), "vscript");
    let json = serde_json::to_string_pretty(graph).map_err(io::Error::other)?;
    fs::write(&path, json)?;
    Ok(path)
}

pub fn load_script(path: &Path) -> Option<ScriptGraph> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn list_prefabs() -> Vec<String> {
    list_matching(is_prefab)
}

pub fn list_images() -> Vec<String> {
    list_matching(is_image)
}

fn list_matching(keep: fn(&Path) -> bool) -> Vec<String> {
    let mut found = Vec::new();
    walk(Path::new(ROOT), keep, &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, keep: fn(&Path) -> bool, found: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, keep, found);
        } else if keep(&path) {
            found.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
}
