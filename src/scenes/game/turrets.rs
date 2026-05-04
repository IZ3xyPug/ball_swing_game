use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::zone_index_for_distance;
use crate::state::*;
use super::helpers::*;

pub fn tick_turrets(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    tick_turret_aim(c, st);
    tick_turret_shoot(c, st);
    tick_bullets(c, st);
    tick_bullet_collision(c, st);
}

#[inline]
fn turret_phase_for_x(x: f32) -> u8 {
    if x >= TURRET_PHASE_3_X {
        3
    } else if x >= TURRET_PHASE_2_X {
        2
    } else {
        1
    }
}

#[inline]
fn turret_target_point(
    phase: u8,
    turret_center: (f32, f32),
    player_pos: (f32, f32),
    player_vel: (f32, f32),
    hooked: bool,
    hook_pos: (f32, f32),
    rope_len: f32,
    gravity_dir: f32,
) -> (f32, f32) {
    if phase < 3 {
        return player_pos;
    }
    let (tcx, tcy) = turret_center;
    let (px, py) = player_pos;
    let (vx, vy) = player_vel;

    if hooked {
        // Pendulum-aware intercept: simulate the rope constraint physics forward
        // tick by tick and stop when the bullet can reach that position.
        let (hx, hy) = hook_pos;
        let (mut sim_px, mut sim_py) = (px, py);
        let (mut sim_vx, mut sim_vy) = (vx, vy);

        for t in 1..=(TURRET_PREDICT_MAX_T as usize) {
            // ── Rope constraint (mirrors tick_rope_constraint in physics.rs) ──
            let dx = sim_px - hx;
            let dy = sim_py - hy;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let nx = dx / dist;
            let ny = dy / dist;
            let tx = -ny;
            let ty =  nx;

            let radial_v  = sim_vx * nx + sim_vy * ny;
            let tangent_v = sim_vx * tx + sim_vy * ty;

            sim_px = hx + nx * rope_len;
            sim_py = hy + ny * rope_len;

            let tv = (tangent_v + GRAVITY * gravity_dir * ty) * SWING_DRAG;
            sim_vx = tx * tv;
            sim_vy = ty * tv;
            let _ = radial_v; // stripped from velocity above

            // ── Position advances by velocity ──
            sim_px += sim_vx;
            sim_py += sim_vy;

            // ── Intercept check ──
            let bdx = sim_px - tcx;
            let bdy = sim_py - tcy;
            let bullet_dist = BULLET_SPEED * t as f32;
            if bdx * bdx + bdy * bdy <= bullet_dist * bullet_dist {
                return (sim_px, sim_py);
            }
        }
        // No intercept found — return last simulated position
        return (sim_px, sim_py);
    }

    // ── Free-flight (not hooked): two-iteration linear lead at 65% ───────────
    let dx0 = px - tcx;
    let dy0 = py - tcy;
    let dist0 = (dx0 * dx0 + dy0 * dy0).sqrt().max(1.0);
    let t0 = dist0 / BULLET_SPEED;
    let p1x = px + vx * t0;
    let p1y = py + vy * t0;
    let dx1 = p1x - tcx;
    let dy1 = p1y - tcy;
    let dist1 = (dx1 * dx1 + dy1 * dy1).sqrt().max(1.0);
    let lead_t = (dist1 / BULLET_SPEED * 0.65).clamp(0.0, TURRET_PREDICT_MAX_T);
    (px + vx * lead_t, py + vy * lead_t)
}

// ── Aim barrels toward player ────────────────────────────────────────────────

fn tick_turret_aim(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let s = st.lock().unwrap();
    let px = s.px;
    let py = s.py;
    let pvx = s.vx;
    let pvy = s.vy;
    let hooked = s.hooked;
    let hook_pos = (s.hook_x, s.hook_y);
    let rope_len = s.rope_len;
    let gravity_dir = s.gravity_dir;
    let live = s.turret_live.clone();
    drop(s);

    for name in &live {
        if let Some(obj) = c.get_game_object_mut(name) {
            let tcx = obj.position.0 + obj.size.0 * 0.5;
            let tcy = obj.position.1 + obj.size.1 * 0.5;
            let phase = turret_phase_for_x(tcx);
            let (tx, ty) = turret_target_point(
                phase, (tcx, tcy), (px, py), (pvx, pvy),
                hooked, hook_pos, rope_len, gravity_dir,
            );
            let angle = (ty - tcy).atan2(tx - tcx).to_degrees();
            obj.rotation = angle;
        }
    }
}

// ── Shoot on interval ────────────────────────────────────────────────────────

fn tick_turret_shoot(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let px = s.px;
    let py = s.py;
    let pvx = s.vx;
    let pvy = s.vy;
    let hooked = s.hooked;
    let hook_pos = (s.hook_x, s.hook_y);
    let rope_len = s.rope_len;
    let gravity_dir = s.gravity_dir;

    // Collect turrets that are ready to shoot.
    let mut ready: Vec<(String, usize)> = Vec::new(); // (turret_id, timer_index)
    for (i, timer) in s.turret_timers.iter_mut().enumerate() {
        if timer.1 > 0 {
            timer.1 -= 1;
        } else {
            ready.push((timer.0.clone(), i));
        }
    }

    let mut shots: Vec<(String, f32, f32, f32, f32)> = Vec::new(); // (bullet_id, bx, by, vx, vy)

    for (turret_id, timer_idx) in &ready {
        if let Some(obj) = c.get_game_object(turret_id) {
            let tcx = obj.position.0 + obj.size.0 * 0.5;
            let tcy = obj.position.1 + obj.size.1 * 0.5;
            let phase = turret_phase_for_x(tcx);
            let (tx, ty) = turret_target_point(
                phase, (tcx, tcy), (px, py), (pvx, pvy),
                hooked, hook_pos, rope_len, gravity_dir,
            );
            let dx = tx - tcx;
            let dy = ty - tcy;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            if dist > TURRET_DETECT_RADIUS {
                continue;
            }
            let vx = BULLET_SPEED * dx / dist;
            let vy = BULLET_SPEED * dy / dist;
            let tip_dist = TURRET_R + TURRET_BARREL_LEN;
            let muzzle_cx = tcx + tip_dist * dx / dist;
            let muzzle_cy = tcy + tip_dist * dy / dist;
            let shot_count = if phase >= 2 { 2usize } else { 1usize };
            if s.bullet_free.len() < shot_count {
                continue;
            }

            if shot_count == 1 {
                if let Some(bullet_id) = s.bullet_free.pop() {
                    let bx = muzzle_cx - BULLET_W * 0.5;
                    let by = muzzle_cy - BULLET_H * 0.5;
                    s.bullet_live.push((bullet_id.clone(), vx, vy, BULLET_LIFETIME_TICKS));
                    shots.push((bullet_id, bx, by, vx, vy));
                }
            } else {
                // Phase 2+: two bullets in succession along the fire axis.
                // Bullet 1 at muzzle; bullet 2 behind it by TURRET_SUCCESSIVE_GAP px.
                // Both travel at the same velocity so they stay spread apart in flight.
                let fx = dx / dist;
                let fy = dy / dist;
                for offset in [0.0f32, -TURRET_SUCCESSIVE_GAP] {
                    if let Some(bullet_id) = s.bullet_free.pop() {
                        let cx = muzzle_cx + fx * offset;
                        let cy = muzzle_cy + fy * offset;
                        let bx = cx - BULLET_W * 0.5;
                        let by = cy - BULLET_H * 0.5;
                        s.bullet_live.push((bullet_id.clone(), vx, vy, BULLET_LIFETIME_TICKS));
                        shots.push((bullet_id, bx, by, vx, vy));
                    }
                }
            }

            let reset_ticks = if phase >= 2 { TURRET_SHOOT_INTERVAL_P2 } else { TURRET_SHOOT_INTERVAL_FAST };
            if let Some((_, ticks)) = s.turret_timers.get_mut(*timer_idx) {
                *ticks = reset_ticks;
            }
        }
    }
    drop(s);

    for (id, bx, by, vx, vy) in &shots {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.position = (*bx, *by);
            obj.rotation = vy.atan2(*vx).to_degrees();
            obj.visible = true;
        }
    }
}

// ── Move bullets ─────────────────────────────────────────────────────────────

fn tick_bullets(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut recycle_ids: Vec<String> = Vec::new();

    for (name, vx, vy, ttl) in &mut s.bullet_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.0 += *vx;
            obj.position.1 += *vy;

            if *ttl > 0 {
                *ttl -= 1;
            }
            if *ttl == 0 {
                recycle_ids.push(name.clone());
            }
        }
    }

    if recycle_ids.is_empty() {
        return;
    }

    for id in &recycle_ids {
        s.bullet_live.retain(|(n, _, _, _)| n != id);
        s.bullet_free.push(id.clone());
    }
    drop(s);

    for id in &recycle_ids {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-5000.0, -5000.0);
        }
    }
}

// ── Bullet ↔ player collision ────────────────────────────────────────────────

fn tick_bullet_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let collect_r = PLAYER_R + BULLET_W.max(BULLET_H) * 0.5 + 4.0;
    let px = s.px;
    let py = s.py;

    let mut hit_ids: Vec<String> = Vec::new();
    for (name, _, _, _) in &s.bullet_live {
        if let Some(obj) = c.get_game_object(name) {
            let bcx = obj.position.0 + BULLET_W * 0.5;
            let bcy = obj.position.1 + BULLET_H * 0.5;
            let dx = px - bcx;
            let dy = py - bcy;
            if dx * dx + dy * dy < collect_r * collect_r {
                hit_ids.push(name.clone());
            }
        }
    }

    if hit_ids.is_empty() { return; }

    // Recycle hit bullets.
    for id in &hit_ids {
        s.bullet_live.retain(|(n, _, _, _)| n != id);
        s.bullet_free.push(id.clone());
    }

    // Disconnect player from hook if hooked.
    if s.hooked {
        let prev = s.active_hook.clone();
        let zone_idx = zone_index_for_distance(s.distance);
        let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
        let gdir = s.gravity_dir;
        s.hooked = false;
        s.active_hook = String::new();
        drop(s);

        c.run(Action::Hide { target: Target::name("rope") });
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * gravity_scale * gdir;
        }
        if !prev.is_empty() {
            let asteroid_mode = matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
            if let Some(hobj) = c.get_game_object_mut(&prev) {
                if asteroid_mode {
                    hobj.set_image(hook_asteroid_img_for_id(&prev, AsteroidHookState::Base));
                } else {
                    let (r, g, b) = hook_base_for_zone(zone_idx);
                    hobj.set_image(hook_img(r, g, b));
                }
                hobj.clear_glow();
            }
        }
    } else {
        drop(s);
    }

    for id in &hit_ids {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-5000.0, -5000.0);
        }
    }
}
