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

/// Update the rope visual when hooked. The actual constraint physics are
/// handled by the engine's GrappleConstraint (via crystalline).
pub fn tick_rope_visual(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let s = st.lock().unwrap();
    if !s.hooked { return; }

    // Read current player position from engine (crystalline may have moved it).
    let (px, py) = if let Some(obj) = c.get_game_object("player") {
        (obj.position.0 + PLAYER_R, obj.position.1 + PLAYER_R)
    } else {
        (s.px, s.py)
    };

    let (rdx, rdy, hx, hy) = (px - s.hook_x, py - s.hook_y, s.hook_x, s.hook_y);
    drop(s);

    let rope_len = (rdx * rdx + rdy * rdy).sqrt().max(1.0);
    let rope_ang = rdy.atan2(rdx).to_degrees();
    let rope_mid_x = hx + rdx * 0.5;
    let rope_mid_y = hy + rdy * 0.5;

    if let Some(rope_obj) = c.get_game_object_mut("rope") {
        rope_obj.size = (rope_len, ROPE_THICKNESS);
        rope_obj.position = (rope_mid_x - rope_len * 0.5, rope_mid_y - ROPE_THICKNESS * 0.5);
        rope_obj.rotation = rope_ang;
    }
}

/// Manage engine gravity. Gravity is always active — the GrappleConstraint
/// works with gravity to produce natural pendulum swing.
pub fn sync_engine_gravity(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let s = st.lock().unwrap();
    let g_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
    let target_gravity = GRAVITY * g_scale * s.gravity_dir;
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.gravity = target_gravity;
    }
}

/// Clamp player momentum to MOMENTUM_CAP and write state back to engine.
/// When hooked, position is managed by the GrappleConstraint — only cap
/// and write momentum.
pub fn cap_momentum_and_write_back(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();

    let speed = (s.vx*s.vx + s.vy*s.vy).sqrt();
    if speed > MOMENTUM_CAP {
        s.vx = s.vx / speed * MOMENTUM_CAP;
        s.vy = s.vy / speed * MOMENTUM_CAP;
    }

    let (px, py, vx, vy, hooked) = (s.px, s.py, s.vx, s.vy, s.hooked);
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        // When hooked, crystalline manages position via GrappleConstraint.
        // Only write position back when free-falling.
        if !hooked {
            obj.position = (px - PLAYER_R, py - PLAYER_R);
        }
        obj.momentum = (vx, vy);
    }
}
