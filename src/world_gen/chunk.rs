use macroquad::color::*;
use macroquad::math::Vec3;
use macroquad::math::vec3;
use macroquad::models::draw_cube;
use macroquad::models::draw_cube_wires;

use crate::constant::CHUNK_HEIGHT;
use crate::constant::CHUNK_WIDTH;


pub struct Chunk {
    pub blocks: Vec<u8>
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {blocks: vec![0; CHUNK_WIDTH *CHUNK_WIDTH * CHUNK_WIDTH * CHUNK_HEIGHT]}
    }
    // the coordinate system is: x: north - south, y: up - down, z: west - east
    fn get_index(x: usize, y: usize, z:usize) -> usize {
        x + y * CHUNK_WIDTH * CHUNK_HEIGHT + z * CHUNK_WIDTH
    }
    pub fn get_block(&self, x: usize, y: usize, z: usize) -> u8 {
        self.blocks[Self::get_index(x, y, z)]
    }
    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block_name: u8) -> () {
        self.blocks[Self::get_index(x, y, z)] = block_name;
    }
     pub fn draw(&self, origin: Vec3) {
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    let id = self.get_block(x, y, z);
                    if id == 0 { continue; }

                    let pos = origin + vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                    draw_cube(pos, Vec3::ONE, None, BROWN);
                    draw_cube_wires(pos, Vec3::ONE, DARKBROWN);
                }
            }
        }
    }
}