
use macroquad::{color::{BROWN, Color}, math::{Vec3, vec3}, models::{Mesh, Vertex, draw_mesh}};

use crate::constant::{CHUNK_WIDTH, FACES, MAX_FACES_PER_MESH};


pub struct Chunk {
    pub blocks: Vec<u8>,
    meshes: Option<Vec<Mesh>>,
}

impl Chunk {
    fn get_index(x: usize, y: usize, z:usize) -> usize {
        x + y * CHUNK_WIDTH * CHUNK_WIDTH + z * CHUNK_WIDTH
    }
    fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        let w = CHUNK_WIDTH as i32;
        if x < 0 || y < 0 || z < 0 || x >= w || y >= w || z >= w {return false;}
        self.get_block(x as usize, y as usize, z as usize) != 0
    }
    // pub
    pub fn new() -> Self {
        Chunk {blocks: vec![0; CHUNK_WIDTH *CHUNK_WIDTH * CHUNK_WIDTH], meshes: None}
    }
    // the coordinate system is: x: north - south, y: up - down, z: west - east
    pub fn get_block(&self, x: usize, y: usize, z: usize) -> u8 {
        self.blocks[Self::get_index(x, y, z)]
    }
    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block_name: u8) -> () {
        self.blocks[Self::get_index(x, y, z)] = block_name;
    }
    
    fn build_meshes(&self, origin: Vec3) -> Vec<Mesh> {
    let mut meshes = Vec::new();
    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    for y in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_WIDTH {
            for x in 0..CHUNK_WIDTH {
                if self.get_block(x, y, z) == 0 { continue; }

                for (n, corners, shade) in FACES.iter() {
                    if self.is_solid(x as i32 + n[0], y as i32 + n[1], z as i32 + n[2]) {
                        continue;
                    }

                    // flush the current mesh when it's full
                    if indices.len() / 6 >= MAX_FACES_PER_MESH {
                        meshes.push(Mesh {
                            vertices: std::mem::take(&mut vertices),
                            indices: std::mem::take(&mut indices),
                            texture: None,
                        });
                    }

                    let base = vertices.len() as u16;
                    let color = Color::new(BROWN.r * shade, BROWN.g * shade, BROWN.b * shade, 1.0);

                    for c in corners {
                        let p = origin + vec3(x as f32 + c[0], y as f32 + c[1], z as f32 + c[2]);
                        vertices.push(Vertex::new(p.x, p.y, p.z, 0.0, 0.0, color));
                    }
                    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                }
            }
        }
    }

    if !indices.is_empty() {
        meshes.push(Mesh { vertices, indices, texture: None });
    }
    meshes
}

pub fn draw(&mut self, origin: Vec3) {
    if self.meshes.is_none() {
        self.meshes = Some(self.build_meshes(origin));
    }
    for mesh in self.meshes.as_ref().unwrap() {
        draw_mesh(mesh);
    }
}
}