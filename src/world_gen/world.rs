use std::collections::HashMap;

use macroquad::math::vec3;

use crate::{constant::CHUNK_WIDTH, world_gen::chunk::Chunk};


pub struct World {
    pub chunks: HashMap<(i32, i32, i32), Chunk>,
}

impl World {
    pub fn new() -> Self {
        World {chunks: HashMap::new()}
    }
    pub fn draw(&mut self) {
    for (&(cx, cy, cz), chunk) in self.chunks.iter_mut() {
        let w = CHUNK_WIDTH as f32;
        chunk.draw(vec3(cx as f32 * w, cy as f32 * w, cz as f32 * w));
    }
}
}