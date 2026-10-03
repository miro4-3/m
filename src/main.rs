// Lib
use macroquad::prelude::*;

mod world_gen;
mod constant;
use noise::{NoiseFn, Perlin};
use world_gen::chunk::Chunk;
use world_gen::world::World;
use constant::CHUNK_WIDTH;

const LOOK_SPEED: f32 = 0.1;

fn generate_chunk(chunk_x: i32, chunk_y: i32, chunk_z: i32, block_name: u8) -> Chunk {
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
            for y in 0..CHUNK_WIDTH {
                let world_y = chunk_y * CHUNK_WIDTH as i32 + y as i32;
                
                if world_y <= terrain_height {
                    new_chunk.set_block(x, y, z, block_name);
                }
            }
        }
    }
    new_chunk
}

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
    let mut world = World::new();
    
    
    let mut player_position = vec3(-3.0, 4.0, -3.0);
    let mut yaw: f32 = 1.18;
    let mut pitch: f32 = -0.4;
    let up_vector = vec3(0.0, 1.0, 0.0);
    let mut last_mouse: Vec2 = mouse_position().into();
    
    for cx in 0..8 {
        for cz in 0..8 {
            for cy in -1..1 {
                world.chunks.insert((cx, cy, cz), generate_chunk(cx, cy, cz, 1));
            }
            
        }
    }
    loop {
        let dt = get_frame_time();
        clear_background(SKYBLUE);
        
        
        let mouse: Vec2 = mouse_position().into();
        let mouse_delta = mouse - last_mouse;
        last_mouse = mouse;
        yaw -= mouse_delta.x * dt * LOOK_SPEED;
        pitch -= mouse_delta.y * dt * LOOK_SPEED;
        pitch = pitch.clamp(-1.5, 1.5);
        
        let front = vec3(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            -yaw.sin() * pitch.cos(),
        ).normalize();
        let right = front.cross(up_vector).normalize();

        
        let mut mv = Vec3::ZERO;
        if is_key_down(KeyCode::W) { mv += front; }
        if is_key_down(KeyCode::S) { mv -= front; }
        if is_key_down(KeyCode::D) { mv += right; }
        if is_key_down(KeyCode::A) { mv -= right; }
        if is_key_down(KeyCode::Space) { mv += up_vector; }
        if is_key_down(KeyCode::LeftShift) { mv -= up_vector; }
        if is_key_down(KeyCode::Minus) {move_speed -= 1.0;}
        if is_key_down(KeyCode::Equal) {move_speed += 1.0;}
        player_position += mv * move_speed * dt;

        set_camera(&Camera3D {
            position: player_position,
            target: player_position + front,
            up: up_vector,
            ..Default::default()
        });

        world.draw();

        // 2D pass (HUD)
        set_default_camera();
        draw_text(&format!("{}", get_fps()), 10.0, 20.0, 24.0, WHITE);
        draw_text(&format!("pos: {:.1}, {:.1}, {:.1}", player_position.x, player_position.y, player_position.z), 10.0, 40.0, 24.0, WHITE);
        draw_text(&format!("yaw: {:.2}, pitch: {:.2}", yaw, pitch), 10.0, 60.0, 24.0, WHITE);
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