//Lib

use macroquad::prelude::*;
// Mod

mod world_gen;
mod constant;
use world_gen::chunk::Chunk;
use world_gen::world::World;

// Const

use constant::CHUNK_WIDTH;

// magic numbers
const MAGIC_NUMBER_1: usize = 1;


//
const BLOCK_TYPE: [(&str, u8); 2] = [
    ("air", 0),
    ("dirt", 1)
];




fn generate_chunk(block_name: u8) -> Chunk {
    let mut new_chunk = Chunk::new();
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_WIDTH {
            new_chunk.set_block(x, MAGIC_NUMBER_1, z, block_name);
        }
    }
    new_chunk
}

fn window_conf() -> Conf {
    Conf {
        window_title: "m".to_owned(),
        platform: miniquad::conf::Platform {
            swap_interval: Some(1), // 0 = vsync off, 1 = on
            ..Default::default()
        },
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = World::new();
    let mut x_coord = 0.0;
    let mut y_coord = 0.0;
    let mut z_coord = 0.0;
    world.chunks.insert((0, 0), generate_chunk(1));

    loop {
        clear_background(SKYBLUE);
        let fps:i32 = get_fps();
        draw_text(format!("{}", fps), 10.0, 20.0, 24.0, BLACK);
        draw_text(format!("x: {},y: {},z: {}", x_coord, y_coord, z_coord), 10.0, 40.0, 24.0, WHITE);
        next_frame().await
    }
}
