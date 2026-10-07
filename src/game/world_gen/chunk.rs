use macroquad::models::{Mesh, draw_mesh};

use crate::constant::{CHUNK_WIDTH, CHUNK_WIDTH_I32 as W};
use crate::game::world_gen::{
    meshing::{Neighbours, greedy_mesh},
    voxel::Voxel,
};

/// Flat-index step along x, y, z.
pub const STRIDE: [usize; 3] = [1, CHUNK_WIDTH * CHUNK_WIDTH, CHUNK_WIDTH];

/// A cube of voxels (`CHUNK_WIDTH` per side) plus its cached render meshes.
///
/// Coordinates: x: north-south, y: up-down, z: west-east. Voxels are stored
/// flat with x varying fastest. Meshes are built by `World::rebuild_dirty`
/// (so border faces can see neighbouring chunks) and stored via `set_meshes`.
pub struct Chunk {
    voxels: Vec<Voxel>,
    meshes: Vec<Mesh>,
    pub dirty: bool,
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            voxels: vec![Voxel::AIR; CHUNK_WIDTH * CHUNK_WIDTH * CHUNK_WIDTH],
            meshes: Vec::new(),
            dirty: true,
        }
    }

    fn index(x: usize, y: usize, z: usize) -> usize {
        debug_assert!(x < CHUNK_WIDTH && y < CHUNK_WIDTH && z < CHUNK_WIDTH);
        x * STRIDE[0] + y * STRIDE[1] + z * STRIDE[2]
    }

    fn in_bounds(x: i32, y: i32, z: i32) -> bool {
        (0..W).contains(&x) && (0..W).contains(&y) && (0..W).contains(&z)
    }

    pub fn get_voxel(&self, x: usize, y: usize, z: usize) -> Voxel {
        self.voxels[Self::index(x, y, z)]
    }

    /// Sets a voxel and flags the meshes as stale if anything changed.
    pub fn set_voxel(&mut self, x: usize, y: usize, z: usize, voxel: Voxel) {
        let i = Self::index(x, y, z);
        if self.voxels[i] != voxel {
            self.voxels[i] = voxel;
            self.dirty = true;
        }
    }

    /// True if (x, y, z) is inside this chunk and holds a solid voxel.
    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        Self::in_bounds(x, y, z) && self.get_voxel(x as usize, y as usize, z as usize).is_solid()
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

    pub fn voxels(&self) -> &[Voxel] {
        &self.voxels
    }

    /// Builds world-space meshes for the chunk at chunk coordinates `key`.
    pub fn build_meshes(&self, key: (i32, i32, i32), neighbours: Neighbours) -> Vec<Mesh> {
        greedy_mesh(&self.voxels, key, neighbours)
    }

    
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn stride_is_the_index_step_along_each_axis() {
        let origin = Chunk::index(0, 0, 0);
        assert_eq!(Chunk::index(1, 0, 0) - origin, STRIDE[0]);
        assert_eq!(Chunk::index(0, 1, 0) - origin, STRIDE[1]);
        assert_eq!(Chunk::index(0, 0, 1) - origin, STRIDE[2]);
    }

    #[test]
    fn set_voxel_marks_dirty_only_on_change() {
        let mut c = Chunk::new();
        c.set_meshes(Vec::new());
        assert!(!c.is_dirty());
        c.set_voxel(0, 0, 0, Voxel::AIR); // no change
        assert!(!c.is_dirty());
        c.set_voxel(0, 0, 0, Voxel::DIRT);
        assert!(c.is_dirty());
    }
}