use std::collections::HashMap;

use macroquad::math::vec3;

use crate::{constant::CHUNK_WIDTH, world_gen::chunk::Chunk};


pub struct World {
    pub chunks: HashMap<(i32, i32), Chunk>,
}

impl World {
    pub fn new() -> Self {
        World {chunks: HashMap::new()}
    }
    pub fn draw(&self) {
        for (&(cx, cz), chunk) in &self.chunks {
            let origin = vec3((cx * CHUNK_WIDTH as i32) as f32, 0.0, (cz * CHUNK_WIDTH as i32) as f32);
            chunk.draw(origin);
        }
    }
}