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

// ── Aim barrels toward player ────────────────────────────────────────────────

fn tick_turret_aim(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let s = st.lock().unwrap();
    let px = s.px;
    let py = s.py;
    let live = s.turret_live.clone();
    drop(s);

    for name in &live {
        if let Some(obj) = c.get_game_object_mut(name) {
            let tcx = obj.position.0 + obj.size.0 * 0.5;
            let tcy = obj.position.1 + obj.size.1 * 0.5;
            let angle = (py - tcy).atan2(px - tcx).to_degrees();
            obj.rotation = angle;
        }
    }
}

// ── Shoot on interval ────────────────────────────────────────────────────────

fn tick_turret_shoot(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let px = s.px;
    let py = s.py;

    let mut ready: Vec<(String, usize)> = Vec::new();
    for (i, timer) in s.turret_timers.iter_mut().enumerate() {
        if timer.1 > 0 {
            timer.1 -= 1;
        } else {
            ready.push((timer.0.clone(), i));
        }
    }

    let mut shots: Vec<(String, f32, f32, f32, f32)> = Vec::new();

    for (turret_id, timer_idx) in &ready {
        if let Some(obj) = c.get_game_object(turret_id) {
            let tcx = obj.position.0 + obj.size.0 * 0.5;
            let tcy = obj.position.1 + obj.size.1 * 0.5;
            let dx = px - tcx;
            let dy = py - tcy;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            if dist > TURRET_DETECT_RADIUS {
                continue;
            }
            let vx = BULLET_SPEED * dx / dist;
            let vy = BULLET_SPEED * dy / dist;
            let tip_dist = TURRET_R + TURRET_BARREL_LEN;
            let bx = tcx + tip_dist * dx / dist - BULLET_W * 0.5;
            let by = tcy + tip_dist * dy / dist - BULLET_H * 0.5;

            if let Some(bullet_id) = s.bullet_free.pop() {
                s.bullet_live.push((bullet_id.clone(), vx, vy, BULLET_LIFETIME_TICKS));
                shots.push((bullet_id, bx, by, vx, vy));
                if let Some((_, ticks)) = s.turret_timers.get_mut(*timer_idx) {
                    *ticks = TURRET_SHOOT_INTERVAL;
                }
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
        }
        if *ttl > 0 {
            *ttl -= 1;
        }
        if *ttl == 0 {
            recycle_ids.push(name.clone());
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

// ── Bullet ↔ player collision ─────────────────────────────────────────────────

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

    for id in &hit_ids {
        s.bullet_live.retain(|(n, _, _, _)| n != id);
        s.bullet_free.push(id.clone());
    }

    // Disconnect player from hook if currently hooked.
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
            if let Some(hobj) = c.get_game_object_mut(&prev) {
                let (r, g, b) = hook_base_for_zone(zone_idx);
                hobj.set_image(hook_img(r, g, b));
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
