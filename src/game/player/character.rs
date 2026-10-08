use macroquad::{camera::{Camera3D, set_camera}, input::{is_key_down, mouse_position}, math::{Vec2, Vec3, vec3}};
use macroquad::input::KeyCode;
use crate::LOOK_SPEED;

pub fn spectator_mode_start(
    mouse_lock: bool,
    move_speed: &mut f32,
    last_mouse: &mut Vec2,
    yaw: &mut f32,
    pitch: &mut f32,
    dt: f32,
    up_vector: Vec3,
    player_position: &mut Vec3,
) -> Camera3D {
    if mouse_lock {
        let mouse: Vec2 = mouse_position().into();
        let mouse_delta = mouse - *last_mouse;
        *last_mouse = mouse;

        *yaw -= mouse_delta.x * dt * LOOK_SPEED;
        *pitch -= mouse_delta.y * dt * LOOK_SPEED;
        *pitch = pitch.clamp(-1.5, 1.5);
    }

    let front = vec3(
        yaw.cos() * pitch.cos(),
        pitch.sin(),
        -yaw.sin() * pitch.cos(),
    )
    .normalize();

    let right = front.cross(up_vector).normalize();

    let mut mv = Vec3::ZERO;
    if is_key_down(KeyCode::W) { mv += front; }
    if is_key_down(KeyCode::S) { mv -= front; }
    if is_key_down(KeyCode::D) { mv += right; }
    if is_key_down(KeyCode::A) { mv -= right; }
    if is_key_down(KeyCode::Space) { mv += up_vector; }
    if is_key_down(KeyCode::LeftShift) { mv -= up_vector; }

    if is_key_down(KeyCode::Minus) { *move_speed = (*move_speed - 1.0).max(0.1); }
    if is_key_down(KeyCode::Equal) { *move_speed += 1.0; }

    *player_position += mv * *move_speed * dt;

    let camera = Camera3D {
        position: *player_position,
        target: *player_position + front,
        up: up_vector,
        ..Default::default()
    };
    set_camera(&camera);
    camera
}