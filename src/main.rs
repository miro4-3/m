// Lib
use macroquad::prelude::*;

mod game;
mod constant;
use std::ops::RangeInclusive;

use game::world_gen::loader::{ChunkLoader, default_workers};
use game::world_gen::world::{View, World};

use crate::game::player::settings::{RenderDistance, RenderSettings};
use crate::game::world_gen::voxel::Voxel;
use crate::game::player::{character};
use crate::game::world_gen::world_generation::generate_chunk;


const LOOK_SPEED: f32 = 0.0225;
/// Chunks meshed per frame while moving, so new terrain doesn't cause a stutter.
const MESHES_PER_FRAME: usize = 16;
/// Finished chunks taken from the worker threads per frame.
const CHUNKS_PER_FRAME: usize = 16;
/// Chunk layers that exist (cy). The world is unbounded sideways. Terrain peaks at y = 46.
const WORLD_HEIGHT: RangeInclusive<i32> = -1..=1;



fn window_conf() -> Conf {
    Conf {
        window_title: "m".to_owned(),
        platform: miniquad::conf::Platform {
            swap_interval: Some(1),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    set_cursor_grab(true);
    show_mouse(false);
    let mut move_speed: f32 = 5.0;
    let mut mouse_lock = true;
    let render_settings: RenderSettings = RenderSettings { render_distance: RenderDistance { x: 16,y: 16, z: 16 } };
    let mut world = World::streaming(WORLD_HEIGHT);
    let mut loader = ChunkLoader::new(default_workers(), |(cx, cy, cz)| generate_chunk(cx, cy, cz, Voxel::DIRT));
    
    let mut player_position = vec3(0.0, 0.0, 0.0);
    
    let mut yaw: f32 = 1.18;
    let mut pitch: f32 = -0.4;
    let up_vector = vec3(0.0, 1.0, 0.0);
    let mut last_mouse: Vec2 = mouse_position().into();
    
    loop {
        let dt = get_frame_time();
        clear_background(SKYBLUE);
        
        let camera = character::spectator_mode_start(mouse_lock, &mut move_speed, &mut last_mouse, &mut yaw, &mut pitch, dt, up_vector, &mut player_position);
        let view = View::new(player_position, &render_settings.render_distance).looking_through(&camera);
        loader.update(&mut world, &view, CHUNKS_PER_FRAME);
        
        // Drawing 3D
        unsafe {
            macroquad::miniquad::gl::glEnable(macroquad::miniquad::gl::GL_CULL_FACE);
            macroquad::miniquad::gl::glCullFace(macroquad::miniquad::gl::GL_BACK);
            macroquad::miniquad::gl::glFrontFace(macroquad::miniquad::gl::GL_CCW);
        } // cull backface
        
        world.rebuild_dirty(&view, MESHES_PER_FRAME);
        world.draw(&view);

        // 2D pass (HUD)
        set_default_camera();
        unsafe {macroquad::miniquad::gl::glDisable(macroquad::miniquad::gl::GL_CULL_FACE)} // disable culling
        draw_text(&format!("{}", get_fps()), 10.0, 20.0, 24.0, WHITE);
        draw_text(&format!("pos: {:.1}, {:.1}, {:.1}", player_position.x, player_position.y, player_position.z), 10.0, 40.0, 24.0, WHITE);
        let (cx, cy, cz) = view.center();
        draw_text(&format!("chunk: {}, {}, {}", cx, cy, cz), 10.0, 100.0, 24.0, WHITE);
        draw_text(&format!("yaw: {:.2}, pitch: {:.2}", yaw, pitch), 10.0, 60.0, 24.0, WHITE);
        draw_text(&format!("faces: {}", world.face_count(&view)), 10.0, 80.0, 24.0, WHITE);
        draw_text(&format!("chunks: {} loaded, {} loading", world.chunks.len(), loader.in_flight()), 10.0, 120.0, 24.0, WHITE);
        if is_key_pressed(KeyCode::G) {
            if mouse_lock {
                mouse_lock = false;
                set_cursor_grab(false);
                show_mouse(true);
            }
            else {
                mouse_lock = true;
                set_cursor_grab(true);
                show_mouse(false);
            }
        }
        if is_key_down(KeyCode::Escape) {break}
        next_frame().await
    }
}