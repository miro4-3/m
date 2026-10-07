// Lib
use macroquad::prelude::*;

mod game;
mod constant;
use game::world_gen::world::World;

use crate::constant::CHUNK_WIDTH;
use crate::game::player::settings::{RenderDistance, RenderSettings};
use crate::game::world_gen::voxel::{self, Voxel};
use crate::game::player::{character};
use crate::game::world_gen::world_generation::generate_chunk;


const LOOK_SPEED: f32 = 0.1;



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
    let mut render_settings: RenderSettings = RenderSettings { render_distance: RenderDistance { x: 8,y: 8, z: 8 } };
    let mut world = World::new();
    
    let mut player_position = vec3(-3.0, 4.0, -3.0);
    
    let mut yaw: f32 = 1.18;
    let mut pitch: f32 = -0.4;
    let up_vector = vec3(0.0, 1.0, 0.0);
    let mut last_mouse: Vec2 = mouse_position().into();
    
    for cx in 0..16 {
            for cz in 0..16 {
            for cy in 0..1 {
                    world.insert((cx, cy, cz), generate_chunk(cx, cy, cz, Voxel::DIRT));
            }
            
        }
    }
   

    loop {
        let dt = get_frame_time();
        clear_background(SKYBLUE);
        
        character::spectator_mode_start(mouse_lock, &mut move_speed, &mut last_mouse, &mut yaw, &mut pitch, dt, up_vector, &mut player_position);
        let player_chunk_x = (player_position.x / CHUNK_WIDTH as f32).floor() as i32;
        let player_chunk_y = (player_position.y / CHUNK_WIDTH as f32).floor() as i32;
        let player_chunk_z = (player_position.z / CHUNK_WIDTH as f32).floor() as i32;
        
        // Drawing 3D
        unsafe {
            macroquad::miniquad::gl::glEnable(macroquad::miniquad::gl::GL_CULL_FACE);
            macroquad::miniquad::gl::glCullFace(macroquad::miniquad::gl::GL_BACK);
            macroquad::miniquad::gl::glFrontFace(macroquad::miniquad::gl::GL_CCW);
        } // cull backface
        
        world.rebuild_dirty();
        world.draw();

        // 2D pass (HUD)
        set_default_camera();
        unsafe {macroquad::miniquad::gl::glDisable(macroquad::miniquad::gl::GL_CULL_FACE)} // disable culling
        draw_text(&format!("{}", get_fps()), 10.0, 20.0, 24.0, WHITE);
        draw_text(&format!("pos: {:.1}, {:.1}, {:.1}", player_position.x, player_position.y, player_position.z), 10.0, 40.0, 24.0, WHITE);
                draw_text(&format!("pos: {:.1}, {:.1}, {:.1}", player_chunk_x, player_chunk_y, player_chunk_z), 10.0, 90.0, 24.0, WHITE);
        draw_text(&format!("yaw: {:.2}, pitch: {:.2}", yaw, pitch), 10.0, 60.0, 24.0, WHITE);
        draw_text(&format!("faces: {}", world.face_count()), 10.0, 85.0, 24.0, WHITE);
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