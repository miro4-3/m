use noise::{NoiseFn, Perlin};

use crate::{
    constant::CHUNK_WIDTH,
    game::world_gen::{chunk::Chunk, voxel::Voxel},
};

/// Builds the terrain for one chunk: a Perlin-noise heightmap filled with `voxel`.
pub fn generate_chunk(chunk_x: i32, chunk_y: i32, chunk_z: i32, voxel: Voxel) -> Chunk {
    let perlin: Perlin = Perlin::new(1);
    let mut new_chunk = Chunk::new();

    // Adjust these to tweak terrain shape
    let scale = 0.0125;       // Frequency: Smaller = smoother/wider hills, Larger = spiky terrain
    let height_scale = 30.0; // Max amplitude/height of hills
    let base_height = 16.0;   // Baseline Y level (sea/ground level offset)

    let w = CHUNK_WIDTH as i32;
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_WIDTH {
            // Compute global world coordinates for seamless chunk transitions
            let world_x = (chunk_x * w + x as i32) as f64;
            let world_z = (chunk_z * w + z as i32) as f64;

            // Sample 2D Perlin noise (returns float in range [-1.0, 1.0])
            let noise_value = perlin.get([world_x * scale, world_z * scale]);

            // Height level for this (x, z) column
            let terrain_height = (base_height + noise_value * height_scale) as i32;

            // How many voxels of this column fall inside this chunk (0..=w)
            let h = (terrain_height - chunk_y * w + 1).clamp(0, w);
            for y in 0..h as usize {
                new_chunk.set_voxel(x, y, z, voxel);
            }
        }
    }
    new_chunk
}