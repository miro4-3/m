use macroquad::{
    color::{BROWN, Color, GRAY, GREEN, MAGENTA}, math::vec3, models::{Mesh, Vertex, draw_mesh},
};



use crate::constant::{CHUNK_WIDTH, CHUNK_WIDTH_I32 as W, FACES, MAX_FACES_PER_MESH};

pub const AIR: u16 = 0;


fn voxel_color(id: u16) -> Color {
    match id {
        1 => BROWN,
        2 => GREEN,
        3 => GRAY,
        _ => MAGENTA, // unknown id
    }
}

pub struct Chunk {
    blocks: Vec<u16>,
    meshes: Vec<Mesh>,
    dirty: bool,
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            blocks: vec![AIR; CHUNK_WIDTH * CHUNK_WIDTH * CHUNK_WIDTH],
            meshes: Vec::new(),
            dirty: true,
        }
    }

    fn index(x: usize, y: usize, z: usize) -> usize {
        debug_assert!(x < CHUNK_WIDTH && y < CHUNK_WIDTH && z < CHUNK_WIDTH);
        x + z * CHUNK_WIDTH + y * CHUNK_WIDTH * CHUNK_WIDTH
    }

    fn in_bounds(x: i32, y: i32, z: i32) -> bool {
        (0..W).contains(&x) && (0..W).contains(&y) && (0..W).contains(&z)
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> u16 {
        self.blocks[Self::index(x, y, z)]
    }

    /// Sets a block and flags the meshes as stale if anything changed.
    pub fn set_block(&mut self, x: usize, y: usize, z: usize, id: u16) {
        let i = Self::index(x, y, z);
        if self.blocks[i] != id {
            self.blocks[i] = id;
            self.dirty = true;
        }
    }

    /// True if (x, y, z) is inside this chunk and not air.
    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        Self::in_bounds(x, y, z) && self.get_block(x as usize, y as usize, z as usize) != AIR
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn set_meshes(&mut self, meshes: Vec<Mesh>) {
        self.meshes = meshes;
        self.dirty = false;
    }

    pub fn face_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 6).sum()
    }

    pub fn draw(&self) {
        for mesh in &self.meshes {
            draw_mesh(mesh);
        }
    }

    pub fn build_meshes(
        &self,
        key: (i32, i32, i32),
        solid_outside: impl Fn(i32, i32, i32) -> bool,
    ) -> Vec<Mesh> {
        let (bx, by, bz) = (key.0 * W, key.1 * W, key.2 * W);
        let origin = vec3(bx as f32, by as f32, bz as f32);

        let mut meshes = Vec::new();
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();

        for y in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    let id = self.get_block(x, y, z);
                    if id == AIR {
                        continue;
                    }
                    let base_color = voxel_color(id);

                    for (n, corners, shade) in FACES.iter() {
                        let (nx, ny, nz) = (x as i32 + n[0], y as i32 + n[1], z as i32 + n[2]);
                        let covered = if Self::in_bounds(nx, ny, nz) {
                            self.get_block(nx as usize, ny as usize, nz as usize) != AIR
                        } else {
                            solid_outside(bx + nx, by + ny, bz + nz)
                        };
                        if covered {
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
                        let color = Color::new(
                            base_color.r * shade,
                            base_color.g * shade,
                            base_color.b * shade,
                            1.0,
                        );
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(c: &Chunk) -> (usize, usize) {
        let m = c.build_meshes((0, 0, 0), |_, _, _| false);
        (
            m.iter().map(|m| m.vertices.len()).sum(),
            m.iter().map(|m| m.indices.len()).sum(),
        )
    }

    #[test]
    fn index_is_unique_for_every_cell() {
        let mut seen = vec![false; CHUNK_WIDTH.pow(3)];
        for y in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    let i = Chunk::index(x, y, z);
                    assert!(!seen[i]);
                    seen[i] = true;
                }
            }
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn single_block_has_six_faces() {
        let mut c = Chunk::new();
        c.set_block(5, 5, 5, 1);
        assert_eq!(counts(&c), (24, 36));
    }

    #[test]
    fn adjacent_blocks_hide_shared_face() {
        let mut c = Chunk::new();
        c.set_block(5, 5, 5, 1);
        c.set_block(6, 5, 5, 1);
        assert_eq!(counts(&c), (40, 60)); // 10 faces
    }

    #[test]
    fn outside_neighbour_hides_border_face() {
        let mut c = Chunk::new();
        c.set_block(15, 0, 0, 1);
        let open = c.build_meshes((0, 0, 0), |_, _, _| false);
        let blocked = c.build_meshes((0, 0, 0), |x, _, _| x == 16);
        let faces = |m: &[Mesh]| m.iter().map(|m| m.indices.len() / 6).sum::<usize>();
        assert_eq!((faces(&open), faces(&blocked)), (6, 5));
    }

    #[test]
    fn full_chunk_splits_into_multiple_meshes() {
        let mut c = Chunk::new();
        for y in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_WIDTH {
                for x in 0..CHUNK_WIDTH {
                    c.set_block(x, y, z, 1);
                }
            }
        }
        let m = c.build_meshes((0, 0, 0), |_, _, _| false);
        assert_eq!(m.len(), 2); // 1536 faces, 800 per mesh
        assert!(m.iter().all(|m| m.indices.len() / 6 <= MAX_FACES_PER_MESH));
    }

    #[test]
    fn set_block_marks_dirty_only_on_change() {
        let mut c = Chunk::new();
        c.set_meshes(Vec::new());
        assert!(!c.is_dirty());
        c.set_block(0, 0, 0, AIR); // no change
        assert!(!c.is_dirty());
        c.set_block(0, 0, 0, 1);
        assert!(c.is_dirty());
    }

    #[test]
    fn meshes_are_placed_at_chunk_origin() {
        let mut c = Chunk::new();
        c.set_block(0, 0, 0, 1);
        let m = c.build_meshes((2, -1, 3), |_, _, _| false);
        let min_x = m[0].vertices.iter().map(|v| v.position.x).fold(f32::MAX, f32::min);
        let min_y = m[0].vertices.iter().map(|v| v.position.y).fold(f32::MAX, f32::min);
        let min_z = m[0].vertices.iter().map(|v| v.position.z).fold(f32::MAX, f32::min);
        assert_eq!((min_x, min_y, min_z), (32.0, -16.0, 48.0));
    }
}