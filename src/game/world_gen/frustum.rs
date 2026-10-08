//! Frustum culling at chunk granularity, using the `collision` crate.

use cgmath::{Matrix4, Point3};
use collision::{Aabb3, Frustum, Relation};
use macroquad::{
    camera::{Camera, Camera3D},
    math::Mat4,
};

use crate::{constant::CHUNK_WIDTH_I32 as W, game::world_gen::world::ChunkKey};

#[derive(Clone, Copy)]
pub struct ViewFrustum(Frustum<f32>);

impl ViewFrustum {
    pub fn from_camera(camera: &Camera3D) -> Option<Self> {
        Self::from_matrix(camera.matrix())
    }

    /// `matrix` is projection * view. `None` if it is degenerate.
    pub fn from_matrix(matrix: Mat4) -> Option<Self> {
        Frustum::from_matrix4(Matrix4::from(matrix.to_cols_array_2d())).map(ViewFrustum)
    }

    /// False only when the chunk lies completely outside the frustum.
    pub fn sees_chunk(&self, key: ChunkKey) -> bool {
        let min = Point3::new((key.0 * W) as f32, (key.1 * W) as f32, (key.2 * W) as f32);
        let max = Point3::new(min.x + W as f32, min.y + W as f32, min.z + W as f32);
        self.0.contains(&Aabb3::new(min, max)) != Relation::Out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::{Vec3, Vec4, vec3};

    fn matrix(position: Vec3, yaw: f32, pitch: f32) -> Mat4 {
        let front = vec3(yaw.cos() * pitch.cos(), pitch.sin(), -yaw.sin() * pitch.cos());
        Mat4::perspective_rh_gl(0.8, 16.0 / 9.0, 0.01, 1000.0) * Mat4::look_at_rh(position, position + front, Vec3::Y)
    }

    fn looking_down_negative_z() -> ViewFrustum {
        ViewFrustum::from_matrix(matrix(Vec3::ZERO, std::f32::consts::FRAC_PI_2, 0.0)).unwrap()
    }

    fn in_clip_space(m: Mat4, p: Vec3) -> bool {
        let c = m * Vec4::new(p.x, p.y, p.z, 1.0);
        c.w > 0.0 && c.x.abs() <= c.w && c.y.abs() <= c.w && c.z.abs() <= c.w
    }

    /// Deterministic pseudo-random numbers in 0..1.
    fn random(state: &mut u64) -> f32 {
        *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*state >> 40) as f32 / (1u64 << 24) as f32
    }

    #[test]
    fn sees_chunks_in_front_but_not_behind_or_far_to_the_side() {
        let f = looking_down_negative_z();
        assert!(f.sees_chunk((0, 0, -3)));
        assert!(f.sees_chunk((-1, -1, -1))); // the chunk the camera is inside
        assert!(!f.sees_chunk((0, 0, 3)));
        assert!(!f.sees_chunk((40, 0, -2)));
        assert!(!f.sees_chunk((0, 40, -2)));
    }

    #[test]
    fn respects_the_far_plane() {
        let f = looking_down_negative_z();
        assert!(f.sees_chunk((0, 0, -20)));
        assert!(!f.sees_chunk((0, 0, -100))); // 100 chunks = 3200 blocks, far plane is 1000
    }

    #[test]
    fn never_culls_a_chunk_that_has_any_visible_point() {
        let mut seed = 42;
        let w = W as f32;
        for _ in 0..40 {
            let position = vec3(random(&mut seed) * 200.0 - 100.0, random(&mut seed) * 100.0, random(&mut seed) * 200.0 - 100.0);
            let m = matrix(position, random(&mut seed) * 6.28, random(&mut seed) * 2.0 - 1.0);
            let f = ViewFrustum::from_matrix(m).unwrap();
            for _ in 0..200 {
                let key = (
                    (position.x / w) as i32 + (random(&mut seed) * 12.0) as i32 - 6,
                    (position.y / w) as i32 + (random(&mut seed) * 12.0) as i32 - 6,
                    (position.z / w) as i32 + (random(&mut seed) * 12.0) as i32 - 6,
                );
                let origin = vec3(key.0 as f32, key.1 as f32, key.2 as f32) * w;
                let any_point_visible = (0..=4).any(|i| (0..=4).any(|j| (0..=4).any(|k| {
                    in_clip_space(m, origin + vec3(i as f32, j as f32, k as f32) * (w / 4.0))
                })));
                assert!(!any_point_visible || f.sees_chunk(key), "culled a chunk that is visible: {key:?}");
            }
        }
    }

    #[test]
    fn culls_most_of_the_surrounding_chunks() {
        let f = looking_down_negative_z();
        let total = 21 * 21 * 21;
        let seen = (-10..=10).flat_map(|x| (-10..=10).flat_map(move |y| (-10..=10).map(move |z| (x, y, z)))).filter(|&k| f.sees_chunk(k)).count();
        assert!(seen * 4 < total, "{seen} of {total} chunks survived culling");
    }

    #[test]
    fn degenerate_matrix_gives_no_frustum() {
        assert!(ViewFrustum::from_matrix(Mat4::ZERO).is_none());
    }
}