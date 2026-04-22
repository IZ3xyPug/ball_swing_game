use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

/// Sync player position/velocity from engine object into State.
/// Call at the start of each tick before any game logic.
pub fn read_player_from_engine(c: &mut Canvas, s: &mut State) {
    if let Some(obj) = c.get_game_object("player") {
        s.px = obj.position.0 + PLAYER_R;
        s.py = obj.position.1 + PLAYER_R;
        s.vx = obj.momentum.0;
        s.vy = obj.momentum.1;
    }
}

/// Apply rope constraint when hooked. Modifies State velocity/position and
/// updates the rope visual. Also sets engine gravity to 0 (tangential gravity
/// is applied manually inside the constraint).
pub fn tick_rope_constraint(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.hooked { return; }

    let dx   = s.px - s.hook_x;
    let dy   = s.py - s.hook_y;
    let dist = (dx*dx + dy*dy).sqrt().max(1.0);
    let nx = dx / dist;
    let ny = dy / dist;
    let tx = -ny;
    let ty = nx;

    let radial_v = s.vx * nx + s.vy * ny;
    let mut tangent_v = s.vx * tx + s.vy * ty;
    let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };

    // Project position onto arc and strip radial velocity.
    s.px = s.hook_x + nx * s.rope_len;
    s.py = s.hook_y + ny * s.rope_len;
    s.vx -= radial_v * nx * SWING_TENSION;
    s.vy -= radial_v * ny * SWING_TENSION;

    // Apply tangential gravity + swing drag.
    tangent_v += GRAVITY * gravity_scale * s.gravity_dir * ty;
    tangent_v *= SWING_DRAG;
    s.vx = tx * tangent_v;
    s.vy = ty * tangent_v;

    // Update rope visual.
    let (rdx, rdy, hx, hy) = (s.px - s.hook_x, s.py - s.hook_y, s.hook_x, s.hook_y);
    let rope_len = (rdx * rdx + rdy * rdy).sqrt().max(1.0);
    let rope_ang = rdy.atan2(rdx).to_degrees();
    let rope_mid_x = hx + rdx * 0.5;
    let rope_mid_y = hy + rdy * 0.5;
    drop(s);

    if let Some(rope_obj) = c.get_game_object_mut("rope") {
        rope_obj.size = (rope_len, ROPE_THICKNESS);
        rope_obj.position = (rope_mid_x - rope_len * 0.5, rope_mid_y - ROPE_THICKNESS * 0.5);
        rope_obj.rotation = rope_ang;
    }
}

/// Manage engine gravity. When hooked: gravity = 0 (rope handles it).
/// When free: gravity = GRAVITY * direction * zero-g scale.
/// During rocket launch (space_launch_active) and while in space: near-zero gravity.
pub fn sync_engine_gravity(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let s = st.lock().unwrap();
    let target_gravity = if s.hooked {
        0.0
    } else if s.in_space_mode || s.space_launch_active {
        // Space / ascent: effectively no global gravity — planet wells do the work.
        GRAVITY * SPACE_GRAVITY_SCALE * s.gravity_dir
    } else {
        let g_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
        GRAVITY * g_scale * s.gravity_dir
    };
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.gravity = target_gravity;
    }
}

/// Clamp player momentum to MOMENTUM_CAP and write state back to engine.
/// The cap is bypassed while `space_launch_active` is true — the rocket pad
/// intentionally launches the player far beyond normal play speeds.
pub fn cap_momentum_and_write_back(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();

    if !s.space_launch_active {
        let speed = (s.vx*s.vx + s.vy*s.vy).sqrt();
        if speed > MOMENTUM_CAP {
            s.vx = s.vx / speed * MOMENTUM_CAP;
            s.vy = s.vy / speed * MOMENTUM_CAP;
        }
    }

    let (px, py, vx, vy) = (s.px, s.py, s.vx, s.vy);
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (px - PLAYER_R, py - PLAYER_R);
        obj.momentum = (vx, vy);
    }
}
