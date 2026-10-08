use std::{collections::HashMap, ops::RangeInclusive};

use macroquad::{camera::Camera3D, math::Vec3};

use crate::{
    constant::{CHUNK_WIDTH_I32 as W, FACES},
    game::{
        player::settings::RenderDistance,
        world_gen::{chunk::Chunk, frustum::ViewFrustum, voxel::Voxel},
    },
};

pub type ChunkKey = (i32, i32, i32);

/// The chunk that contains a world position.
pub fn chunk_of(position: Vec3) -> ChunkKey {
    let chunk = |v: f32| (v / W as f32).floor() as i32;
    (chunk(position.x), chunk(position.y), chunk(position.z))
}

/// What the player can see: a box of chunks that reaches `RenderDistance` out along each
/// axis, optionally narrowed to the camera's view frustum.
#[derive(Clone, Copy)]
pub struct View {
    center: ChunkKey,
    radius: [i32; 3],
    frustum: Option<ViewFrustum>,
}

impl View {
    pub fn new(position: Vec3, distance: &RenderDistance) -> Self {
        View {
            center: chunk_of(position),
            radius: [distance.x as i32, distance.y as i32, distance.z as i32],
            frustum: None,
        }
    }

    /// Also skips drawing chunks outside what `camera` can see.
    pub fn looking_through(mut self, camera: &Camera3D) -> Self {
        self.frustum = ViewFrustum::from_camera(camera);
        self
    }

    pub fn center(&self) -> ChunkKey {
        self.center
    }

    fn offset(&self, key: ChunkKey) -> [i32; 3] {
        [key.0 - self.center.0, key.1 - self.center.1, key.2 - self.center.2]
    }

    /// Inside the render distance box, widened by `margin` chunks on every side?
    pub fn within(&self, key: ChunkKey, margin: i32) -> bool {
        let offset = self.offset(key);
        (0..3).all(|i| offset[i].abs() <= self.radius[i] + margin)
    }

    fn contains(&self, key: ChunkKey) -> bool {
        self.within(key, 0)
    }

    /// In range and not outside the camera's frustum: worth drawing.
    fn sees(&self, key: ChunkKey) -> bool {
        self.contains(key) && self.frustum.is_none_or(|f| f.sees_chunk(key))
    }

    /// Every chunk position in the box, widened by `margin`.
    pub fn keys(&self, margin: i32) -> impl Iterator<Item = ChunkKey> + use<> {
        let (c, r) = (self.center, self.radius.map(|r| r + margin));
        (c.0 - r[0]..=c.0 + r[0]).flat_map(move |x| {
            (c.1 - r[1]..=c.1 + r[1]).flat_map(move |y| (c.2 - r[2]..=c.2 + r[2]).map(move |z| (x, y, z)))
        })
    }

    pub fn distance_sq(&self, key: ChunkKey) -> i32 {
        self.offset(key).iter().map(|d| d * d).sum()
    }
}

pub struct World {
    pub chunks: HashMap<ChunkKey, Chunk>,
    height: Option<RangeInclusive<i32>>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        World { chunks: HashMap::new(), height: None }
    }

    /// A world that exists only in the chunk layers `height`, and is unbounded sideways.
    /// A chunk is meshed once all its neighbours inside that range have loaded, so chunks
    /// streaming in are never meshed more than once.
    pub fn streaming(height: RangeInclusive<i32>) -> Self {
        World { chunks: HashMap::new(), height: Some(height) }
    }

    pub fn height(&self) -> Option<&RangeInclusive<i32>> {
        self.height.as_ref()
    }

    /// Adds (or replaces) a chunk. Its neighbours are re-meshed too, since faces along the
    /// shared border may now be hidden.
    pub fn insert(&mut self, key: ChunkKey, chunk: Chunk) {
        self.chunks.insert(key, chunk);
        self.mark_neighbours_dirty(key);
    }

    pub fn remove(&mut self, key: ChunkKey) {
        if self.chunks.remove(&key).is_some() {
            self.mark_neighbours_dirty(key);
        }
    }

    /// Drops every chunk outside `view` widened by `margin`.
    pub fn unload_beyond(&mut self, view: &View, margin: i32) {
        let far: Vec<ChunkKey> = self.chunks.keys().filter(|key| !view.within(**key, margin)).copied().collect();
        for key in far {
            self.remove(key);
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
            .filter(|(key, chunk)| chunk.is_dirty() && view.contains(**key) && self.neighbours_loaded(**key))
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
            .filter(|(key, _)| view.sees(**key))
            .map(|(_, chunk)| chunk)
    }

    pub fn draw(&self, view: &View) {
        for chunk in self.visible_chunks(view) {
            chunk.draw();
        }
    }

    /// Quads currently drawn, i.e. in chunks `view` can see.
    pub fn face_count(&self, view: &View) -> usize {
        self.visible_chunks(view).map(|c| c.face_count()).sum()
    }

    fn mark_neighbours_dirty(&mut self, key: ChunkKey) {
        for (n, _, _) in FACES.iter() {
            self.mark_dirty((key.0 + n[0], key.1 + n[1], key.2 + n[2]));
        }
    }

    /// True unless this is a streaming world and a neighbour that should exist hasn't loaded yet.
    fn neighbours_loaded(&self, key: ChunkKey) -> bool {
        let Some(height) = &self.height else { return true };
        FACES.iter().all(|(n, _, _)| {
            let at = (key.0 + n[0], key.1 + n[1], key.2 + n[2]);
            !height.contains(&at.1) || self.chunks.contains_key(&at)
        })
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

    fn looking_down_negative_z() -> ViewFrustum {
        use macroquad::math::{Mat4, vec3};
        let projection = Mat4::perspective_rh_gl(0.8, 16.0 / 9.0, 0.01, 1000.0);
        ViewFrustum::from_matrix(projection * Mat4::look_at_rh(Vec3::ZERO, vec3(0.0, 0.0, -1.0), Vec3::Y)).unwrap()
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

    #[test]
    fn draw_skips_chunks_outside_the_frustum() {
        let mut w = World::new();
        for key in [(0, 0, -3), (0, 0, 3), (30, 0, -3)] {
            w.insert(key, chunk_with_one_voxel());
        }
        let mut view = everywhere();
        assert_eq!(w.visible_chunks(&view).count(), 3);
        view.frustum = Some(looking_down_negative_z());
        assert_eq!(w.visible_chunks(&view).count(), 1);
    }

    #[test]
    fn frustum_does_not_limit_which_chunks_get_meshed() {
        let mut w = World::new();
        w.insert((0, 0, 3), chunk_with_one_voxel()); // behind the camera
        let mut view = everywhere();
        view.frustum = Some(looking_down_negative_z());
        w.rebuild_dirty(&view, usize::MAX);
        assert!(!w.chunks[&(0, 0, 3)].is_dirty()); // meshed, so turning around shows no pop-in
    }

    #[test]
    fn view_keys_cover_the_widened_box() {
        let view = View::new(Vec3::ZERO, &distance(1, 0, 2));
        let keys: Vec<_> = view.keys(1).collect();
        assert_eq!(keys.len(), 5 * 3 * 7);
        assert!(keys.iter().all(|&k| view.within(k, 1)));
        assert!(!keys.iter().any(|&k| view.within(k, 0) != (k.0.abs() <= 1 && k.1 == 0 && k.2.abs() <= 2)));
    }

    #[test]
    fn streaming_chunks_wait_for_their_neighbours_before_meshing() {
        let mut w = World::streaming(0..=0);
        w.insert((0, 0, 0), chunk_with_one_voxel());
        w.rebuild_dirty(&everywhere(), usize::MAX);
        assert!(w.chunks[&(0, 0, 0)].is_dirty()); // 4 sideways neighbours still missing

        for key in [(1, 0, 0), (-1, 0, 0), (0, 0, 1)] {
            w.insert(key, chunk_with_one_voxel());
        }
        w.rebuild_dirty(&everywhere(), usize::MAX);
        assert!(w.chunks[&(0, 0, 0)].is_dirty()); // one left

        w.insert((0, 0, -1), chunk_with_one_voxel());
        w.rebuild_dirty(&everywhere(), usize::MAX);
        assert!(!w.chunks[&(0, 0, 0)].is_dirty()); // layers above and below don't exist, so don't count
    }

    #[test]
    fn plain_worlds_mesh_chunks_immediately() {
        let mut w = World::new();
        w.insert((0, 0, 0), chunk_with_one_voxel());
        w.rebuild_dirty(&everywhere(), usize::MAX);
        assert!(!w.chunks[&(0, 0, 0)].is_dirty());
    }

    #[test]
    fn removing_a_chunk_re_meshes_its_neighbours() {
        let mut w = world_with_two_voxels_across_a_border();
        w.remove((1, 0, 0));
        assert!(w.chunks[&(0, 0, 0)].is_dirty());
        w.rebuild_dirty(&everywhere(), usize::MAX);
        assert_eq!(w.face_count(&everywhere()), 6); // nothing hides it any more
    }

    #[test]
    fn unload_beyond_keeps_chunks_within_the_margin() {
        let mut w = World::new();
        for x in 0..10 {
            w.insert((x, 0, 0), chunk_with_one_voxel());
        }
        w.unload_beyond(&View::new(Vec3::ZERO, &distance(2, 2, 2)), 1); // keeps x = 0..=3
        let mut kept: Vec<_> = w.chunks.keys().map(|k| k.0).collect();
        kept.sort();
        assert_eq!(kept, vec![0, 1, 2, 3]);
    }
}