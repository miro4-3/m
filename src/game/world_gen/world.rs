use std::collections::HashMap;

use crate::{
    constant::{CHUNK_WIDTH_I32 as W, FACES}, game::{world_gen::{chunk::Chunk, voxel::Voxel}},
};

type ChunkKey = (i32, i32, i32);

pub struct World {
    pub chunks: HashMap<ChunkKey, Chunk>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        World { chunks: HashMap::new() }
    }

    pub fn insert(&mut self, key: ChunkKey, chunk: Chunk) {
        self.chunks.insert(key, chunk);
        for (n, _, _) in FACES.iter() {
            self.mark_dirty((key.0 + n[0], key.1 + n[1], key.2 + n[2]));
        }
    }
    /// Edits one voxel by world coordinates. Does nothing if the chunk isn't loaded.
    #[allow(dead_code)] // editing API, not used by the game yet
    pub fn set_voxel(&mut self, wx: i32, wy: i32, wz: i32, voxel: Voxel) {
        let key = (wx.div_euclid(W), wy.div_euclid(W), wz.div_euclid(W));
        let (lx, ly, lz) = (wx.rem_euclid(W), wy.rem_euclid(W), wz.rem_euclid(W));
        let Some(chunk) = self.chunks.get_mut(&key) else { return };
        chunk.set_voxel(lx as usize, ly as usize, lz as usize, voxel);

        // an edit on a chunk border changes the neighbour's visible faces too
        for (n, _, _) in FACES.iter() {
            let (nx, ny, nz) = (lx + n[0], ly + n[1], lz + n[2]);
            if !(0..W).contains(&nx) || !(0..W).contains(&ny) || !(0..W).contains(&nz) {
                self.mark_dirty((key.0 + n[0], key.1 + n[1], key.2 + n[2]));
            }
        }
    }

    /// True if the voxel at these world coordinates is solid. Unloaded chunks count as air.
    #[allow(dead_code)] // for collision and raycasts later
    pub fn is_solid(&self, wx: i32, wy: i32, wz: i32) -> bool {
        let key = (wx.div_euclid(W), wy.div_euclid(W), wz.div_euclid(W));
        self.chunks.get(&key).is_some_and(|c| {
            c.is_solid(wx.rem_euclid(W), wy.rem_euclid(W), wz.rem_euclid(W))
        })
    }

    /// Re-meshes every chunk whose blocks (or neighbours) changed. Cheap when nothing is dirty.
    pub fn rebuild_dirty(&mut self) {
        let keys: Vec<ChunkKey> = self
            .chunks
            .iter()
            .filter(|(_, c)| c.is_dirty())
            .map(|(k, _)| *k)
            .collect();
        for key in keys {
            let neighbours = FACES.map(|(n, _, _)| {
                let at = (key.0 + n[0], key.1 + n[1], key.2 + n[2]);
                self.chunks.get(&at).map(Chunk::voxels)
            });
            let meshes = self.chunks[&key].build_meshes(key, neighbours);
            if let Some(chunk) = self.chunks.get_mut(&key) {
                chunk.set_meshes(meshes);
            }
        }
    }

    pub fn draw(&self) {
        for chunk in self.chunks.values() {
            chunk.draw();
        }
    }

    pub fn face_count(&self) -> usize {
        self.chunks.values().map(|c| c.face_count()).sum()
    }

    fn mark_dirty(&mut self, key: ChunkKey) {
        if let Some(chunk) = self.chunks.get_mut(&key) {
            chunk.mark_dirty();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constant::CHUNK_WIDTH;

    const LAST: usize = CHUNK_WIDTH - 1; // index of the last voxel along an axis

    /// Chunk (0,0,0)'s last voxel touches chunk (1,0,0)'s first voxel.
    fn world_with_two_voxels_across_a_border() -> World {
        let mut a = Chunk::new();
        a.set_voxel(LAST, 0, 0, Voxel::DIRT);
        let mut b = Chunk::new();
        b.set_voxel(0, 0, 0, Voxel::DIRT);
        let mut w = World::new();
        w.insert((0, 0, 0), a);
        w.insert((1, 0, 0), b);
        w.rebuild_dirty();
        w
    }

    #[test]
    fn border_faces_are_culled_between_chunks() {
        let w = world_with_two_voxels_across_a_border();
        assert_eq!(w.face_count(), 10); // 12 minus the 2 touching faces
    }

    #[test]
    fn is_solid_handles_negative_coordinates() {
        let mut c = Chunk::new();
        c.set_voxel(LAST, LAST, LAST, Voxel::DIRT);
        let mut w = World::new();
        w.insert((-1, -1, -1), c);
        assert!(w.is_solid(-1, -1, -1));
        assert!(!w.is_solid(0, 0, 0));
        assert!(!w.is_solid(-W - 1, 0, 0)); // chunk -2 isn't loaded = air
    }

    #[test]
    fn editing_a_border_voxel_updates_the_neighbour() {
        let mut w = world_with_two_voxels_across_a_border();
        w.set_voxel(LAST as i32, 0, 0, Voxel::AIR); // remove A's voxel
        w.rebuild_dirty();
        assert_eq!(w.face_count(), 6); // B's voxel is fully exposed again
    }

    #[test]
    fn rebuild_clears_dirty_flags() {
        let mut w = world_with_two_voxels_across_a_border();
        w.rebuild_dirty();
        assert!(w.chunks.values().all(|c| !c.is_dirty()));
    }
}