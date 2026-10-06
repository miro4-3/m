use noise::{NoiseFn, Perlin};

use crate::{constant::CHUNK_WIDTH, world_gen::chunk::Chunk};



pub fn generate_chunk(chunk_x: i32, chunk_y: i32, chunk_z: i32, id: u16) -> Chunk {
    let perlin: Perlin = Perlin::new(1);
    let mut new_chunk = Chunk::new();
    
    // Adjust these to tweak terrain shape
    let scale = 0.025;      // Frequency: Smaller = smoother/wider hills, Larger = spiky terrain
    let height_scale = 12.0; // Max amplitude/height of hills
    let base_height = 8.0;   // Baseline Y level (sea/ground level offset)

    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_WIDTH {
            // Compute global world coordinates for seamless chunk transitions
            let world_x = (chunk_x * CHUNK_WIDTH as i32 + x as i32) as f64;
            let world_z = (chunk_z * CHUNK_WIDTH as i32 + z as i32) as f64;
            
            // Sample 2D Perlin noise (returns float in range [-1.0, 1.0])
            let noise_value = perlin.get([world_x * scale, world_z * scale]);
            
            // Calculate height level for this (x, z) column
            let terrain_height = (base_height + noise_value * height_scale) as i32;
            
            // Fill blocks vertically up to local target height within this chunk
            // for y in 0..CHUNK_WIDTH {
            //     let world_y = chunk_y * CHUNK_WIDTH as i32 + y as i32;
                
            //     if world_y <= terrain_height {
            //         new_chunk.set_block(x, y, z, block_name);
            //     }
            // }
            let w = CHUNK_WIDTH as i32;
            let h = (terrain_height - chunk_y * w + 1).clamp(0, w);
            for y in 0..h as usize {
                new_chunk.set_block(x, y, z, id);
            }
        }
    }
    new_chunk
}