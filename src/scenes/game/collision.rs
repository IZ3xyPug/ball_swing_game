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
    tick_rocket_pad_collision(c, st);
    tick_asteroid_collision(c, st);
    tick_asteroid_asteroid_collision(c, st);
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
    // Restore hook to base colour (or asteroid skin if mode is on).
    if !ops.prev_hook.is_empty() {
        let asteroid_mode = matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
        if let Some(hobj) = c.get_game_object_mut(&ops.prev_hook) {
            if asteroid_mode {
                hobj.set_image(hook_asteroid_img_for_id(&ops.prev_hook, AsteroidHookState::Base));
            } else {
                let (r, g, b) = hook_base_for_zone(ops.zone_idx);
                hobj.set_image(hook_img(r, g, b));
            }
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
        s.vy = PAD_BOUNCE_VY_START * PAD_BOUNCE_VERTICAL_BOOST * bounce_factor * s.gravity_dir;
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
            obj.set_glow(GlowConfig {
                color: Color(pr, pg, pb, 220),
                width: 10.0,
            });
        }
    }
}

// ── Rocket pad launch ─────────────────────────────────────────────────────────
// Circle-AABB hit test: player ball landing on top of a rocket pad launches
// the player upward with a big velocity boost and unhooks them.

pub fn tick_rocket_pad_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    // Rocket pads only work in normal mode (not in space already)
    if s.in_space_mode { return; }

    let player_bottom = s.py + PLAYER_R;
    let player_left   = s.px - PLAYER_R;
    let player_right  = s.px + PLAYER_R;
    let approaching   = s.gravity_dir > 0.0 && s.vy > 0.0;

    if !approaching { return; }

    let live = s.rocket_pad_live.clone();
    let mut hit_pad: Option<String> = None;
    for name in &live {
        if let Some(obj) = c.get_game_object(name) {
            let pad_top   = obj.position.1;
            let pad_left  = obj.position.0;
            let pad_right = obj.position.0 + ROCKET_PAD_W;
            let overlap_x = player_right > pad_left && player_left < pad_right;
            if overlap_x && player_bottom >= pad_top && player_bottom <= pad_top + ROCKET_PAD_H + s.vy.abs() {
                hit_pad = Some(name.clone());
                break;
            }
        }
    }

    if let Some(ref pad_name) = hit_pad {
        // Apply launch velocity — strong enough to clear the entire normal zone
        s.vy = ROCKET_PAD_LAUNCH_VY;  // large negative = upward
        s.vx += ROCKET_PAD_LAUNCH_VX;
        s.py = s.py - PLAYER_R * 0.5; // nudge up to avoid re-trigger
        // This is the ONLY place that unlocks the space zone entry check.
        s.space_launch_active = true;

        let unhook_ops = begin_unhook(&mut s);
        drop(s);

        if let Some(ref ops) = unhook_ops {
            apply_unhook(c, ops);
        }

        // Camera shake + flash
        if let Some(cam) = c.camera_mut() {
            cam.shake(55.0, 0.5);
            cam.flash_with(Color(C_ROCKET_PAD_GLOW.0, C_ROCKET_PAD_GLOW.1, C_ROCKET_PAD_GLOW.2, 160),
                0.5, FlashMode::Pulse, FlashEase::Sharp, 0.85, 0.0);
        }

        // Glowing highlight on the pad
        if let Some(obj) = c.get_game_object_mut(pad_name) {
            obj.set_glow(GlowConfig {
                color: Color(C_ROCKET_PAD.0, C_ROCKET_PAD.1, C_ROCKET_PAD.2, 240),
                width: 18.0,
            });
        }
    }
}

// ── Floating asteroid collision ──────────────────────────────────────────────

fn tick_asteroid_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let live = s.space_asteroid_live.clone();

    for name in live {
        // Circle-circle overlap: treat each asteroid as a circle of radius = half its size.
        let hit_info: Option<(f32, f32)> = {
            if let Some(obj) = c.get_game_object(&name) {
                let ax = obj.position.0 + obj.size.0 * 0.5;
                let ay = obj.position.1 + obj.size.1 * 0.5;
                let asteroid_r = obj.size.0 * 0.38;
                let min_dist = PLAYER_R + 6.0 + asteroid_r;
                let dx = s.px - ax;
                let dy = s.py - ay;
                let dist2 = dx * dx + dy * dy;
                if dist2 < min_dist * min_dist {
                    let dist = dist2.sqrt().max(0.001);
                    let push = min_dist - dist;
                    Some((dx / dist * push, dy / dist * push))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((push_x, push_y)) = hit_info {
            // Push player out of overlap.
            s.px += push_x;
            s.py += push_y;

            let push_len = (push_x * push_x + push_y * push_y).sqrt().max(0.001);
            let nx = push_x / push_len;
            let ny = push_y / push_len;

            // Strip inward velocity component (elastic deflect).
            let inward = -(s.vx * nx + s.vy * ny);
            if inward > 0.0 {
                s.vx += nx * inward;
                s.vy += ny * inward;
            }

            // Tangential component of player velocity drives asteroid spin.
            let tx = -ny;
            let ty = nx;
            let tang = s.vx * tx + s.vy * ty;
            let spin_impulse = tang * 0.006;

            drop(s);

            if let Some(obj) = c.get_game_object_mut(&name) {
                obj.rotation_momentum += spin_impulse;
                // Nudge the asteroid away from the player on impact.
                obj.momentum.0 += -nx * 1.5;
                obj.momentum.1 += -ny * 1.5;
            }

            s = st.lock().unwrap();
        }
    }
}

// ── Asteroid–Asteroid Collision ─────────────────────────────────────────────
// Manual circle-circle check run each game tick.  Crystalline's dynamic-dynamic
// path is correct but asteroids spawn 1300-2800 px apart and close too slowly
// to collide during normal play.  This check works directly on live object state
// so collisions are detected and resolved the moment any two circles overlap.

fn tick_asteroid_asteroid_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let live = {
        let s = st.lock().unwrap();
        s.space_asteroid_live.clone()
    };
    if live.len() < 2 { return; }

    // Snapshot: all values needed for contact math, read before any mutation.
    struct Snap { name: String, cx: f32, cy: f32, r: f32, mass: f32, mx: f32, my: f32 }
    let snaps: Vec<Snap> = live.iter().filter_map(|name| {
        let obj = c.get_game_object(name)?;
        if !obj.visible { return None; }
        Some(Snap {
            name: name.clone(),
            cx:   obj.position.0 + obj.size.0 * 0.5,
            cy:   obj.position.1 + obj.size.1 * 0.5,
            r:    obj.size.0 * 0.38,
            mass: (obj.material.density * obj.size.0 * obj.size.1).max(0.001),
            mx:   obj.momentum.0,
            my:   obj.momentum.1,
        })
    }).collect();

    let n = snaps.len();
    if n < 2 { return; }

    // Accumulated corrections per asteroid: (pos_dx, pos_dy, mom_dx, mom_dy)
    let mut corr: Vec<(f32, f32, f32, f32)> = vec![(0.0, 0.0, 0.0, 0.0); n];

    for i in 0..n {
        for j in (i + 1)..n {
            let a = &snaps[i];
            let b = &snaps[j];
            let dx = a.cx - b.cx;
            let dy = a.cy - b.cy;
            let dist2 = dx * dx + dy * dy;
            let min_dist = a.r + b.r;
            if dist2 >= min_dist * min_dist { continue; }

            let dist = dist2.sqrt().max(0.001);
            let pen  = min_dist - dist;
            // nx,ny: unit normal pointing from B's centre toward A's centre.
            let nx = dx / dist;
            let ny = dy / dist;

            // Position correction split by opposite-mass ratio so heavy bodies
            // barely move and light bodies are pushed away.
            let total = a.mass + b.mass;
            let ra = b.mass / total; // fraction A moves
            let rb = a.mass / total; // fraction B moves
            corr[i].0 += nx * pen * ra;
            corr[i].1 += ny * pen * ra;
            corr[j].0 -= nx * pen * rb;
            corr[j].1 -= ny * pen * rb;

            // Velocity impulse — only when approaching (rel velocity toward each other).
            // approach = (va - vb) · (direction from A to B) = dot with (-nx,-ny).
            let rel_vn = (a.mx - b.mx) * (-nx) + (a.my - b.my) * (-ny);
            if rel_vn > 0.0 {
                let inv_a = 1.0 / a.mass;
                let inv_b = 1.0 / b.mass;
                // e = 0.5 (partially elastic rock-on-rock).
                let j_imp = rel_vn * 1.5 / (inv_a + inv_b);
                // Push A away from B (+nx direction) and B away from A (-nx direction).
                corr[i].2 += nx * j_imp * inv_a;
                corr[i].3 += ny * j_imp * inv_a;
                corr[j].2 -= nx * j_imp * inv_b;
                corr[j].3 -= ny * j_imp * inv_b;
            }
        }
    }

    // Write accumulated corrections back to each asteroid object.
    for (snap, (pdx, pdy, mdx, mdy)) in snaps.iter().zip(corr.iter()) {
        if pdx.abs() < 0.001 && pdy.abs() < 0.001
            && mdx.abs() < 0.001 && mdy.abs() < 0.001
        {
            continue;
        }
        if let Some(obj) = c.get_game_object_mut(&snap.name) {
            obj.position.0 += pdx;
            obj.position.1 += pdy;
            obj.momentum.0 += mdx;
            obj.momentum.1 += mdy;
        }
    }
}
