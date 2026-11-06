use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct Portraits {
    folder: PathBuf,
    loaded: HashMap<String, Option<TextureHandle>>,
}

impl Portraits {
    pub fn new(folder: PathBuf) -> Portraits {
        Portraits { folder, loaded: HashMap::new() }
    }

    pub fn reload(&mut self) {
        self.loaded.clear();
    }

    pub fn get(&mut self, ctx: &egui::Context, id: &str) -> Option<TextureHandle> {
        let file = self.folder.join(format!("{id}.png"));
        self.loaded
            .entry(id.to_string())
            .or_insert_with(|| {
                let image = image::load_from_memory(&fs::read(file).ok()?).ok()?.thumbnail(96, 96).to_rgba8();
                let size = [image.width() as usize, image.height() as usize];
                Some(ctx.load_texture(format!("portrait-{id}"), ColorImage::from_rgba_unmultiplied(size, image.as_raw()), TextureOptions::LINEAR))
            })
            .clone()
    }

    pub fn delete(&mut self, id: &str) {
        let _ = fs::remove_file(self.folder.join(format!("{id}.png")));
        self.loaded.remove(id);
    }
}
