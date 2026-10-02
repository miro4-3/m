// Lib
use macroquad::prelude::*;

mod world_gen;
mod constant;
use world_gen::chunk::Chunk;
use world_gen::world::World;
use constant::CHUNK_WIDTH;

const MAGIC_NUMBER_1: usize = 1;
const LOOK_SPEED: f32 = 0.1;
const MOVE_SPEED: f32 = 5.0;

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

    let mut world = World::new();
    world.chunks.insert((0, 0), generate_chunk(1));

    let mut player_position = vec3(-3.0, 4.0, -3.0);
    let mut yaw: f32 = 1.18;
    let mut pitch: f32 = -0.4;
    let up_vector = vec3(0.0, 1.0, 0.0);
    let mut last_mouse: Vec2 = mouse_position().into();

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
        player_position += mv * MOVE_SPEED * dt;

        set_camera(&Camera3D {
            position: player_position,
            target: player_position + front,
            up: up_vector,
            ..Default::default()
        });

        draw_grid(20, 1.0, BLACK, GRAY);

        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_WIDTH {

                draw_cube(
                    vec3(x as f32 + 0.5, 1.5, z as f32 + 0.5),
                    vec3(1.0, 1.0, 1.0),
                    None,
                    BROWN,
                );
            }
        }

        // 2D pass (HUD)
        set_default_camera();
        draw_text(&format!("{}", get_fps()), 10.0, 20.0, 24.0, WHITE);
        draw_text(&format!("pos: {:.1}, {:.1}, {:.1}", player_position.x, player_position.y, player_position.z), 10.0, 40.0, 24.0, WHITE);
        draw_text(&format!("yaw: {:.2}, pitch: {:.2}", yaw, pitch), 10.0, 60.0, 24.0, WHITE);

        next_frame().await
    }
}