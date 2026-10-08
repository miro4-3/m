use std::collections::HashMap;

use macroquad::math::Vec3;

use crate::{
    constant::{CHUNK_WIDTH_I32 as W, FACES},
    game::{
        player::settings::RenderDistance,
        world_gen::{chunk::Chunk, voxel::Voxel},
    },
};

pub type ChunkKey = (i32, i32, i32);

/// The chunk that contains a world position.
pub fn chunk_of(position: Vec3) -> ChunkKey {
    let chunk = |v: f32| (v / W as f32).floor() as i32;
    (chunk(position.x), chunk(position.y), chunk(position.z))
}

/// The chunks around the viewer that should be meshed and drawn: a box that reaches
/// `RenderDistance` chunks out along each axis.
#[derive(Clone, Copy)]
pub struct View {
    center: ChunkKey,
    radius: [i32; 3],
}

impl View {
    pub fn new(position: Vec3, distance: &RenderDistance) -> Self {
        View {
            center: chunk_of(position),
            radius: [distance.x as i32, distance.y as i32, distance.z as i32],
        }
    }

    pub fn center(&self) -> ChunkKey {
        self.center
    }

    fn offset(&self, key: ChunkKey) -> [i32; 3] {
        [key.0 - self.center.0, key.1 - self.center.1, key.2 - self.center.2]
    }

    fn contains(&self, key: ChunkKey) -> bool {
        let offset = self.offset(key);
        (0..3).all(|i| offset[i].abs() <= self.radius[i])
    }

    fn distance_sq(&self, key: ChunkKey) -> i32 {
        self.offset(key).iter().map(|d| d * d).sum()
    }
}

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

    /// Re-meshes up to `budget` changed chunks inside `view`, nearest first.
    /// Chunks outside the view stay dirty until they come into range.
    pub fn rebuild_dirty(&mut self, view: &View, budget: usize) {
        let mut keys: Vec<ChunkKey> = self
            .chunks
            .iter()
            .filter(|(key, chunk)| chunk.is_dirty() && view.contains(**key))
            .map(|(key, _)| *key)
            .collect();
        keys.sort_unstable_by_key(|&key| view.distance_sq(key));

        for key in keys.into_iter().take(budget) {
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

    fn visible_chunks<'a>(&'a self, view: &'a View) -> impl Iterator<Item = &'a Chunk> {
        self.chunks
            .iter()
            .filter(|(key, _)| view.contains(**key))
            .map(|(_, chunk)| chunk)
    }

    pub fn draw(&self, view: &View) {
        for chunk in self.visible_chunks(view) {
            chunk.draw();
        }
    }

    /// Quads currently drawn, i.e. in chunks inside `view`.
    pub fn face_count(&self, view: &View) -> usize {
        self.visible_chunks(view).map(|c| c.face_count()).sum()
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

    fn distance(x: usize, y: usize, z: usize) -> RenderDistance {
        RenderDistance { x, y, z }
    }

    fn everywhere() -> View {
        View::new(Vec3::ZERO, &distance(100, 100, 100))
    }

    fn chunk_with_one_voxel() -> Chunk {
        let mut c = Chunk::new();
        c.set_voxel(0, 0, 0, Voxel::DIRT);
        c
    }

    /// Chunk (0,0,0)'s last voxel touches chunk (1,0,0)'s first voxel.
    fn world_with_two_voxels_across_a_border() -> World {
        let mut a = Chunk::new();
        a.set_voxel(LAST, 0, 0, Voxel::DIRT);
        let mut b = Chunk::new();
        b.set_voxel(0, 0, 0, Voxel::DIRT);
        let mut w = World::new();
        w.insert((0, 0, 0), a);
        w.insert((1, 0, 0), b);
        w.rebuild_dirty(&everywhere(), usize::MAX);
        w
    }

    #[test]
    fn border_faces_are_culled_between_chunks() {
        let w = world_with_two_voxels_across_a_border();
        assert_eq!(w.face_count(&everywhere()), 10); // 12 minus the 2 touching faces
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
        w.rebuild_dirty(&everywhere(), usize::MAX);
        assert_eq!(w.face_count(&everywhere()), 6); // B's voxel is fully exposed again
    }

    #[test]
    fn rebuild_clears_dirty_flags() {
        let mut w = world_with_two_voxels_across_a_border();
        w.rebuild_dirty(&everywhere(), usize::MAX);
        assert!(w.chunks.values().all(|c| !c.is_dirty()));
    }

    #[test]
    fn chunk_of_rounds_down_including_negatives() {
        let w = W as f32;
        assert_eq!(chunk_of(Vec3::new(0.0, 0.0, 0.0)), (0, 0, 0));
        assert_eq!(chunk_of(Vec3::new(w - 0.1, 0.0, 0.0)), (0, 0, 0));
        assert_eq!(chunk_of(Vec3::new(w, 0.0, 0.0)), (1, 0, 0));
        assert_eq!(chunk_of(Vec3::new(-0.1, -w, -w - 0.1)), (-1, -1, -2));
    }

    #[test]
    fn view_is_a_box_with_independent_radii() {
        let view = View::new(Vec3::ZERO, &distance(2, 1, 0));
        assert!(view.contains((2, 1, 0)) && view.contains((-2, -1, 0)));
        assert!(!view.contains((3, 0, 0)) && !view.contains((0, 2, 0)) && !view.contains((0, 0, 1)));
    }

    #[test]
    fn only_chunks_in_view_are_visible() {
        let mut w = World::new();
        for x in 0..10 {
            w.insert((x, 0, 0), chunk_with_one_voxel());
        }
        let view = View::new(Vec3::ZERO, &distance(3, 0, 0)); // x = 0..=3
        assert_eq!(w.visible_chunks(&view).count(), 4);
    }

    #[test]
    fn far_chunks_stay_dirty_until_they_are_in_range() {
        let mut w = World::new();
        w.insert((0, 0, 0), chunk_with_one_voxel());
        w.insert((9, 0, 0), chunk_with_one_voxel());
        w.rebuild_dirty(&View::new(Vec3::ZERO, &distance(2, 2, 2)), usize::MAX);
        assert!(!w.chunks[&(0, 0, 0)].is_dirty());
        assert!(w.chunks[&(9, 0, 0)].is_dirty());

        let moved = View::new(Vec3::new(9.0 * W as f32, 0.0, 0.0), &distance(2, 2, 2));
        w.rebuild_dirty(&moved, usize::MAX);
        assert!(!w.chunks[&(9, 0, 0)].is_dirty());
    }

    #[test]
    fn budget_rebuilds_the_nearest_chunks_first() {
        let mut w = World::new();
        for x in [3, 1, 2] {
            w.insert((x, 0, 0), chunk_with_one_voxel());
        }
        w.rebuild_dirty(&View::new(Vec3::ZERO, &distance(5, 5, 5)), 2);
        let dirty: Vec<_> = (1..=3).filter(|&x| w.chunks[&(x, 0, 0)].is_dirty()).collect();
        assert_eq!(dirty, vec![3]);
    }

    #[test]
    fn face_count_only_counts_chunks_in_view() {
        let mut w = World::new();
        w.insert((0, 0, 0), chunk_with_one_voxel());
        w.insert((9, 0, 0), chunk_with_one_voxel());
        w.rebuild_dirty(&everywhere(), usize::MAX);
        assert_eq!(w.face_count(&everywhere()), 12);
        assert_eq!(w.face_count(&View::new(Vec3::ZERO, &distance(2, 2, 2))), 6);
    }
}