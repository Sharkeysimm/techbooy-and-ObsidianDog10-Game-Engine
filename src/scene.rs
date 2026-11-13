use eframe::egui::Vec2;

use crate::game_object::GameObject;

pub struct Scene {
    pub objects: Vec<GameObject>,
    pub selected: Option<u64>,
    next_id: u64,
}

impl Scene {
    pub fn new() -> Self {
        let mut scene = Self {
            objects: Vec::new(),
            selected: None,
            next_id: 1,
        };
        let id = scene.add(GameObject::square());
        scene.selected = Some(id);
        scene
    }

    pub fn add(&mut self, mut object: GameObject) -> u64 {
        object.id = self.next_id;
        self.next_id += 1;

        let id = object.id;
        self.objects.push(object);
        id
    }

    pub fn get(&self, id: u64) -> Option<&GameObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut GameObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    pub fn children_of(&self, parent: Option<u64>) -> Vec<u64> {
        self.objects
            .iter()
            .filter(|o| o.parent == parent)
            .map(|o| o.id)
            .collect()
    }

    // parents come before their children so children draw on top
    pub fn draw_order(&self) -> Vec<u64> {
        let mut order = Vec::new();
        self.collect(None, &mut order);
        order
    }

    fn collect(&self, parent: Option<u64>, order: &mut Vec<u64>) {
        for id in self.children_of(parent) {
            order.push(id);
            self.collect(Some(id), order);
        }
    }

    pub fn world_position(&self, id: u64) -> Vec2 {
        let mut position = Vec2::ZERO;
        let mut current = self.get(id);

        while let Some(object) = current {
            position += Vec2::new(object.x, object.y);
            current = object.parent.and_then(|p| self.get(p));
        }

        position
    }

    pub fn is_visible(&self, id: u64) -> bool {
        let mut current = self.get(id);

        while let Some(object) = current {
            if !object.visible {
                return false;
            }
            current = object.parent.and_then(|p| self.get(p));
        }

        true
    }

    pub fn is_same_or_descendant(&self, id: u64, ancestor: u64) -> bool {
        let mut current = self.get(id);

        while let Some(object) = current {
            if object.id == ancestor {
                return true;
            }
            current = object.parent.and_then(|p| self.get(p));
        }

        false
    }

    // keeps the world position so the object does not jump when reparented
    pub fn set_parent(&mut self, id: u64, parent: Option<u64>) {
        if let Some(parent) = parent {
            if self.is_same_or_descendant(parent, id) {
                return;
            }
        }
        match self.get(id) {
            Some(object) if object.parent != parent => {}
            _ => return,
        }

        let world = self.world_position(id);
        let parent_world = parent.map(|p| self.world_position(p)).unwrap_or(Vec2::ZERO);

        if let Some(object) = self.get_mut(id) {
            object.parent = parent;
            object.x = world.x - parent_world.x;
            object.y = world.y - parent_world.y;
        }
    }

    pub fn remove(&mut self, id: u64) {
        let mut doomed = vec![id];
        let mut index = 0;

        while index < doomed.len() {
            let current = doomed[index];
            doomed.extend(self.children_of(Some(current)));
            index += 1;
        }

        self.objects.retain(|o| !doomed.contains(&o.id));

        if self.selected.is_some_and(|s| doomed.contains(&s)) {
            self.selected = None;
        }
    }
}
