use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::zone_index_for_distance;
use crate::images::*;
use crate::objects::*;
use crate::state::*;
use super::helpers::*;

pub fn tick_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    tick_spinner_collision(c, st);
    tick_gate_collision(c, st);
    tick_pad_bounce(c, st);
}

/// Sets state fields for unhook + queues canvas ops (rope hide, gravity restore).
/// Caller must drop the lock before processing the returned commands.
struct UnhookOps {
    prev_hook: String,
    zone_idx: usize,
    gravity_val: f32,
}

fn begin_unhook(s: &mut State) -> Option<UnhookOps> {
    if !s.hooked { return None; }
    let prev = s.active_hook.clone();
    let zone_idx = zone_index_for_distance(s.distance);
    let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
    let gdir = s.gravity_dir;
    s.hooked = false;
    s.active_hook = String::new();
    Some(UnhookOps { prev_hook: prev, zone_idx, gravity_val: GRAVITY * gravity_scale * gdir })
}

fn apply_unhook(c: &mut Canvas, ops: &UnhookOps) {
    c.run(Action::Hide { target: Target::name("rope") });
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.gravity = ops.gravity_val;
    }
    // Restore hook to base colour.
    if !ops.prev_hook.is_empty() {
        if let Some(hobj) = c.get_game_object_mut(&ops.prev_hook) {
            let (r, g, b) = hook_base_for_zone(ops.zone_idx);
            hobj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
                image: circle_img(HOOK_R as u32, r, g, b).into(),
                color: None,
            });
            hobj.clear_glow();
        }
    }
}

// ── Spinning obstacle collision ──────────────────────────────────────────────

fn tick_spinner_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.spinners_enabled { return; }

    let live = s.spinner_live.clone();
    for name in live {
        let hit_info = {
            if let Some(obj) = c.get_game_object(&name) {
                circle_hits_obb(
                    (s.px, s.py), PLAYER_R + 4.0,
                    obj.position, obj.size, obj.rotation,
                )
            } else { None }
        };

        if let Some((push_x, push_y)) = hit_info {
            s.px += push_x;
            s.py += push_y;

            let push_len = (push_x * push_x + push_y * push_y).sqrt().max(0.001);
            let nx = push_x / push_len;
            let ny = push_y / push_len;

            let inward = -(s.vx * nx + s.vy * ny);
            if inward > 0.0 {
                s.vx += nx * inward;
                s.vy += ny * inward;
            }

            if s.spinner_hit_cooldown == 0 {
                let push_mag = (SPINNER_HIT_PUSH_X * SPINNER_HIT_PUSH_X
                    + SPINNER_HIT_PUSH_Y * SPINNER_HIT_PUSH_Y).sqrt();
                s.vx += nx * push_mag;
                s.vy += ny * push_mag;
                s.spinner_hit_cooldown = 6;
                s.glow_flashes.push((name.clone(), 10));

                let unhook_ops = begin_unhook(&mut s);
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&name) {
                    obj.set_glow(GlowConfig { color: Color(255, 100, 80, 220), width: 8.0 });
                }
                if let Some(ref ops) = unhook_ops {
                    apply_unhook(c, ops);
                }

                s = st.lock().unwrap();
            }
        }
    }
}

// ── Gate collision ──────────────────────────────────────────────────────────

fn tick_gate_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !GATES_ENABLED { return; }

    let live = s.gate_live.clone();
    for gate_id in live {
        let top_id = format!("{gate_id}_top");
        let bot_id = format!("{gate_id}_bot");
        for seg_id in [top_id, bot_id] {
            let hit_info = {
                if let Some(obj) = c.get_game_object(&seg_id) {
                    circle_hits_aabb(
                        (s.px, s.py), PLAYER_R + 2.0,
                        obj.position, obj.size,
                    )
                } else { None }
            };

            if let Some((push_x, push_y)) = hit_info {
                s.px += push_x;
                s.py += push_y;

                let push_len = (push_x * push_x + push_y * push_y).sqrt().max(0.001);
                let nx = push_x / push_len;
                let ny = push_y / push_len;
                let inward = -(s.vx * nx + s.vy * ny);
                if inward > 0.0 {
                    s.vx += nx * inward;
                    s.vy += ny * inward;
                }

                s.vx += nx * 4.0;
                s.vy += ny * 4.0;

                let unhook_ops = begin_unhook(&mut s);
                drop(s);
                if let Some(ref ops) = unhook_ops {
                    apply_unhook(c, ops);
                }
                s = st.lock().unwrap();
            }
        }
    }
}

// ── Pad bounce ──────────────────────────────────────────────────────────────

fn tick_pad_bounce(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let falling_down = s.gravity_dir > 0.0 && s.vy > 0.0;
    let falling_up   = s.gravity_dir < 0.0 && s.vy < 0.0;
    if !s.bounce_enabled || !(falling_down || falling_up) { return; }

    let player_bottom = s.py + PLAYER_R;
    let player_top    = s.py - PLAYER_R;
    let player_left   = s.px - PLAYER_R;
    let player_right  = s.px + PLAYER_R;
    let mut bounced_pad: Option<(String, f32, f32)> = None; // (name, pad_top, pad_bot)

    for name in &s.pad_live {
        if let Some(obj) = c.get_game_object(name) {
            let pad_top    = obj.position.1;
            let pad_bottom = obj.position.1 + PAD_H;
            let pad_left   = obj.position.0;
            let pad_right  = obj.position.0 + PAD_W;
            let overlap_x = player_right > pad_left && player_left < pad_right;
            let hit = if falling_down {
                overlap_x && player_bottom >= pad_top && player_bottom <= pad_top + PAD_H + s.vy.abs()
            } else {
                overlap_x && player_top <= pad_bottom && player_top >= pad_bottom - PAD_H - s.vy.abs()
            };
            if hit { bounced_pad = Some((name.clone(), pad_top, pad_bottom)); break; }
        }
    }

    if let Some((pad_name, pad_top, pad_bottom)) = bounced_pad {
        let bounce_factor = (1.0 - s.pad_bounce_count as f32 * PAD_BOUNCE_DECAY).max(PAD_BOUNCE_MIN_FACTOR);
        s.vy = PAD_BOUNCE_VY_START * bounce_factor * s.gravity_dir;
        s.pad_bounce_count = s.pad_bounce_count.saturating_add(1);

        if falling_down {
            s.py = pad_top - PLAYER_R;
        } else {
            s.py = pad_bottom + PLAYER_R;
        }

        let unhook_ops = begin_unhook(&mut s);
        let zone_idx = zone_index_for_distance(s.distance);
        s.glow_flashes.push((pad_name.clone(), 12));
        drop(s);

        if let Some(ref ops) = unhook_ops {
            apply_unhook(c, ops);
        }

        if let Some(obj) = c.get_game_object_mut(&pad_name) {
            let (pr, pg, pb) = pad_hit_for_zone(zone_idx);
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (PAD_W, PAD_H), 0.0),
                image: pad_img(PAD_W as u32, PAD_H as u32, pr, pg, pb).into(),
                color: None,
            });
            obj.set_glow(GlowConfig { color: Color(60, 200, 255, 220), width: 10.0 });
        }
    }
}
