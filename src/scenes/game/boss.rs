// ── boss.rs — Boss fight logic ────────────────────────────────────────────────
// All logic gated behind boss_active — zero impact on free-roam.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::images::solid;
use crate::state::*;
use super::bootstrap::hook_asteroid_anim_for_spawn;

pub fn tick_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    tick_boss_zone_entry(c, st);
    tick_boss_appearance(c, st);
    tick_boss_movement(c, st);
    tick_boss_asteroid_drift(c, st);
    tick_boss_shooting(c, st);
    tick_boss_bolts(c, st);
    tick_boss_bolt_player_collision(c, st);
    tick_boss_player_hits_boss(c, st);
    tick_boss_hud(c, st);
}

// ── Zone entry + arena clear ──────────────────────────────────────────────────

fn tick_boss_zone_entry(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Only active when the player selected Boss Mode from the menu.
    if !matches!(c.get_var("boss_mode_active"), Some(Value::Bool(true))) { return; }
    // Once this run's boss is defeated, do not re-enter boss flow.
    if matches!(c.get_var("boss_mode_cleared"), Some(Value::Bool(true))) { return; }
    let mut s = st.lock().unwrap();
    // Only fire in normal game, not space mode.
    if s.in_space_mode || s.space_launch_active { return; }
    if s.dead { return; }

    if !s.boss_active && s.px >= BOSS_THRESHOLD_X {
        s.boss_active = true;
        s.boss_cleared = false;
        s.boss_entry_ticks = 0;
        s.boss_phase = 0.0;
        s.boss_hp = BOSS_MAX_HP;
        s.boss_shoot_timer = BOSS_SHOOT_INTERVAL;
        drop(s);
        if let Some(obj) = c.get_game_object_mut("boss_hp_bar") {
            obj.visible = true;
        }
        return;
    }

    if !s.boss_active { return; }

    // One-shot arena clear: remove all hooks/pads/coins/turrets/bullets/etc.
    if !s.boss_cleared {
        s.boss_cleared = true;

        // --- hooks ---
        let hooks: Vec<String> = s.live_hooks.drain(..).collect();
        s.spawn_animations.retain(|a| !hooks.contains(&a.id));
        for id in &hooks { s.pool_free.push(id.clone()); }

        // --- pads (also hide thruster) ---
        let pads: Vec<String> = s.pad_live.drain(..).collect();
        for id in &pads { s.pad_free.push(id.clone()); }

        // --- spinners ---
        let spinners: Vec<String> = s.spinner_live.drain(..).collect();
        for id in &spinners { s.spinner_free.push(id.clone()); }

        // --- coins ---
        let coins: Vec<String> = s.coin_live.drain(..).collect();
        for id in &coins { s.coin_free.push(id.clone()); }

        // --- flips ---
        let flips: Vec<String> = s.flip_live.drain(..).collect();
        for id in &flips { s.flip_free.push(id.clone()); }

        // --- score x2 ---
        let sx2: Vec<String> = s.score_x2_live.drain(..).collect();
        for id in &sx2 { s.score_x2_free.push(id.clone()); }

        // --- zero-g ---
        let zg: Vec<String> = s.zero_g_live.drain(..).collect();
        for id in &zg { s.zero_g_free.push(id.clone()); }

        // --- gates ---
        let gates: Vec<String> = s.gate_live.drain(..).collect();
        for id in &gates { s.gate_free.push(id.clone()); }

        // --- gravity wells ---
        let gwells: Vec<String> = s.gwell_live.drain(..).collect();
        s.gwell_timers.retain(|(gid, _, _)| !gwells.contains(gid));
        for id in &gwells { s.gwell_free.push(id.clone()); }

        // --- turrets + bullets ---
        let turrets: Vec<String> = s.turret_live.drain(..).collect();
        s.turret_timers.retain(|(gid, _)| !turrets.contains(gid));
        for id in &turrets { s.turret_free.push(id.clone()); }
        let bullets: Vec<(String, f32, f32, u32)> = s.bullet_live.drain(..).collect();
        for (id, _, _, _) in &bullets { s.bullet_free.push(id.clone()); }

        // --- rocket pads ---
        let rpads: Vec<String> = s.rocket_pad_live.drain(..).collect();
        for id in &rpads { s.rocket_pad_free.push(id.clone()); }

        // --- floating world asteroids outside arena ---
        // Remove all existing floating asteroid GIFs when entering boss zone.
        let boss_asteroid_ids = s.boss_asteroids.clone();
        let mut world_asteroids: Vec<String> = s.space_asteroid_live.drain(..).collect();
        world_asteroids.retain(|id| !boss_asteroid_ids.contains(id));
        for id in &world_asteroids { s.space_asteroid_free.push(id.clone()); }

        // Register boss asteroids as live so collision systems treat them
        // exactly like regular floating asteroids.
        for id in &boss_asteroid_ids {
            if !s.space_asteroid_live.contains(id) {
                s.space_asteroid_live.push(id.clone());
            }
        }

        // Kill any in-flight spawn animations for all cleared objects so
        // tick_spawn_animations cannot make them visible again after the clear.
        s.spawn_animations.retain(|a| {
            !spinners.contains(&a.id)
                && !pads.contains(&a.id)
                && !coins.contains(&a.id)
                && !flips.contains(&a.id)
                && !sx2.contains(&a.id)
                && !zg.contains(&a.id)
                && !gates.contains(&a.id)
                && !gwells.contains(&a.id)
                && !turrets.contains(&a.id)
                && !rpads.contains(&a.id)
                && !world_asteroids.contains(&a.id)
        });

        // Collect all ids to hide.
        let mut all_hide: Vec<String> = hooks;
        all_hide.extend(pads.iter().cloned());
        // Also hide pad thrusters (named pad_X_thruster)
        let thr_ids: Vec<String> = pads.iter().map(|n| format!("{n}_thruster")).collect();
        all_hide.extend(thr_ids);
        all_hide.extend(spinners);
        all_hide.extend(coins);
        all_hide.extend(flips);
        all_hide.extend(sx2);
        all_hide.extend(zg);
        all_hide.extend(gates.iter().flat_map(|g| [format!("{g}_top"), format!("{g}_bot")]));
        all_hide.extend(gwells);
        all_hide.extend(turrets);
        all_hide.extend(bullets.iter().map(|(id, _, _, _)| id.clone()));
        all_hide.extend(rpads);
        all_hide.extend(world_asteroids.clone());

        drop(s);

        for id in &all_hide {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.visible = false;
                obj.position = (-3000.0, -3000.0);
                obj.momentum = (0.0, 0.0);
            }
        }

        // Apply reduced gravity to player for the boss zone.
        if let Some(obj) = c.get_game_object_mut("player") {
            // Preserve sign (flipped gravity support).
            let cur = obj.gravity;
            let sign = if cur < 0.0 { -1.0_f32 } else { 1.0_f32 };
            obj.gravity = sign * GRAVITY * BOSS_GRAVITY_SCALE;
        }

        // Spawn boss-zone asteroids immediately on entry.
        place_boss_asteroids(c, &boss_asteroid_ids);
        return;
    }

    // Clamp player inside boss zone while boss is alive.
    if s.boss_hp > 0 {
        let half = PLAYER_R;
        if s.px < BOSS_ZONE_X1 + half {
            s.px = BOSS_ZONE_X1 + half;
            s.vx = s.vx.max(0.0);
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                if obj.position.0 < BOSS_ZONE_X1 + half - PLAYER_R {
                    obj.position.0 = BOSS_ZONE_X1 + half - PLAYER_R;
                }
                if obj.momentum.0 < 0.0 { obj.momentum.0 = 0.0; }
            }
        } else if s.px > BOSS_ZONE_X2 - half {
            s.px = BOSS_ZONE_X2 - half;
            s.vx = s.vx.min(0.0);
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                if obj.position.0 > BOSS_ZONE_X2 - half - PLAYER_R {
                    obj.position.0 = BOSS_ZONE_X2 - half - PLAYER_R;
                }
                if obj.momentum.0 > 0.0 { obj.momentum.0 = 0.0; }
            }
        }
    }
}

fn place_boss_asteroids(c: &mut Canvas, asteroid_ids: &[String]) {
    let anim = hook_asteroid_anim_for_spawn();
    let zone_w = BOSS_ZONE_X2 - BOSS_ZONE_X1;
    const Y_TOP:  f32 = -3500.0;
    const Y_BOT:  f32 =  1500.0;

    // Deterministic hash RNG for stable-but-arbitrary placement.
    fn hash01(mut x: u32) -> f32 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x as f32) / (u32::MAX as f32)
    }

    // Build a blue-noise-ish point set so spacing stays wide while looking random.
    let mut points: Vec<(f32, f32)> = Vec::new();
    let min_sep = 1120.0;
    let min_sep2 = min_sep * min_sep;
    let mut seed = 0xC0FFEE_u32;
    for _ in 0..600 {
        if points.len() >= asteroid_ids.len() { break; }
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let rx = hash01(seed);
        let ry = hash01(seed ^ 0x9E37_79B9);
        let x = BOSS_ZONE_X1 + rx * zone_w;
        let y = Y_TOP + ry * (Y_BOT - Y_TOP);
        if points.iter().all(|(px, py)| {
            let dx = x - *px;
            let dy = y - *py;
            dx * dx + dy * dy >= min_sep2
        }) {
            points.push((x, y));
        }
    }

    // Fallback: if rejection sampling under-fills, top up with sparse jittered rows.
    if points.len() < asteroid_ids.len() {
        let need = asteroid_ids.len() - points.len();
        for i in 0..need {
            let t = (i as f32 + 0.5) / need as f32;
            let x = BOSS_ZONE_X1 + zone_w * t + ((i as f32 * 173.0) % 500.0 - 250.0);
            let y = Y_TOP + (Y_BOT - Y_TOP) * ((i as f32 * 0.618_033_95) % 1.0)
                + ((i as f32 * 97.0) % 280.0 - 140.0);
            points.push((x.clamp(BOSS_ZONE_X1, BOSS_ZONE_X2), y.clamp(Y_TOP, Y_BOT)));
        }
    }

    for (i, id) in asteroid_ids.iter().enumerate() {
        let (ax, ay) = points[i.min(points.len().saturating_sub(1))];
        let dvx = ((i as f32 * 0.7 + 0.4) % 1.0 - 0.5) * 0.6;
        let dvy = ((i as f32 * 1.3 + 0.1) % 1.0 - 0.5) * 0.3;

        if let Some(obj) = c.get_game_object_mut(id) {
            obj.position = (ax - obj.size.0 * 0.5, ay - obj.size.1 * 0.5);
            obj.momentum = (dvx, dvy);
            obj.visible = true;
            if let Some(anim_ref) = &anim {
                obj.set_animation(anim_ref.clone());
            }
        }
    }
}

// ── Boss appearance after delay ───────────────────────────────────────────────

fn tick_boss_appearance(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || s.boss_spawned { return; }

    s.boss_entry_ticks = s.boss_entry_ticks.saturating_add(1);
    if s.boss_entry_ticks < BOSS_ENTRY_DELAY_TICKS { return; }

    s.boss_spawned = true;

    // Spawn phase starts from the top of the lissajous sweep.
    s.boss_phase = std::f32::consts::FRAC_PI_2;
    drop(s);

    // Place boss at its initial lissajous position (top-center).
    let spawn_x = BOSS_ARENA_CENTER_X - BOSS_SIZE * 0.5;
    let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.position = (spawn_x, spawn_y);
        obj.visible = true;
    }
}

// ── Boss movement — lissajous figure-8 ───────────────────────────────────────
// Horizontal: A·sin(phase)         → sweeps full arena width
// Vertical:   B·sin(2·phase + π/4) → two vertical cycles per horizontal sweep
// The asymmetric phase offset makes it feel less mechanical.

fn tick_boss_movement(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (cur_x, cur_y) = if let Some(obj) = c.get_game_object("boss") {
        (obj.position.0 + BOSS_SIZE * 0.5, obj.position.1 + BOSS_SIZE * 0.5)
    } else {
        (BOSS_ARENA_CENTER_X, BOSS_Y_CENTER)
    };

    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 { return; }

    let px = s.px;
    let py = s.py;

    // Smooth Lissajous 3:2 figure — natural, coherent looping pattern.
    s.boss_phase += BOSS_PHASE_X_SPEED * 3.2;
    let phase = s.boss_phase;

    // Lissajous curve: x = sin(3*phase), y = sin(2*phase + offset)
    // This creates a smooth 3-loop pattern that visits all quadrants naturally
    let x_liss = (phase * 3.0).sin();
    let y_liss = (phase * 2.0 + 0.5).sin();

    // Map to arena bounds: X = [-1,1] → [20000, 27000], Y = [-1,1] → [-4000, 1200]
    let tx_base = BOSS_ARENA_CENTER_X + x_liss * BOSS_ARENA_HALF_W * 0.95;
    let y_min = -4000.0;
    let y_max = 1200.0;
    let y_center = (y_min + y_max) * 0.5;
    let y_half_range = (y_max - y_min) * 0.5;
    let mut ty_base = y_center + y_liss * y_half_range * 0.92;

    // ── Player proximity avoidance: dynamic steering away when threatened ──
    let pdx = cur_x - px;
    let pdy = cur_y - py;
    let player_dist2 = pdx * pdx + pdy * pdy;
    let danger_radius = 1200.0; // activation distance
    let danger_radius2 = danger_radius * danger_radius;

    let (tx_final, ty_final) = if player_dist2 < danger_radius2 {
        // Player too close: steer boss away with proportional force
        let threat_factor = (1.0 - (player_dist2.sqrt() / danger_radius)).max(0.0).powi(2);
        let threat_mul = 0.7 * threat_factor; // max 70% steering influence
        
        // Escape direction: away from player
        let escape_dist = player_dist2.sqrt().max(1.0);
        let escape_dx = pdx / escape_dist;
        let escape_dy = pdy / escape_dist;
        
        // Guide to opposite corner/edge
        let opposite_corner_x = if escape_dx > 0.0 {
            BOSS_ZONE_X1 + BOSS_SIZE * 0.5
        } else {
            BOSS_ZONE_X2 - BOSS_SIZE * 0.5
        };
        let opposite_corner_y = if escape_dy > 0.0 {
            y_min
        } else {
            y_max
        };
        
        // Blend base pattern with escape direction
        let tx_escape = opposite_corner_x * threat_mul + tx_base * (1.0 - threat_mul);
        let ty_escape = opposite_corner_y * threat_mul + ty_base * (1.0 - threat_mul);
        (tx_escape, ty_escape)
    } else {
        (tx_base, ty_base)
    };

    let dx = tx_final - cur_x;
    let dy = ty_final - cur_y;
    let d = (dx * dx + dy * dy).sqrt().max(1.0);
    
    // Smooth speed modulation based on phase — faster in straights, slower at turns
    let speed_mod = 0.3 + 0.7 * (phase * 0.5).cos().abs();
    let seek_speed = 22.0 * speed_mod; // smooth variable speed
    let desired_vx = dx / d * seek_speed;
    let desired_vy = dy / d * seek_speed;

    // Very smooth velocity transitions to eliminate jerky direction changes
    s.boss_vx += (desired_vx - s.boss_vx) * 0.18;
    s.boss_vy += (desired_vy - s.boss_vy) * 0.18;

    let max_speed = 38.0; // moderate speed for smooth curves
    let sp = (s.boss_vx * s.boss_vx + s.boss_vy * s.boss_vy).sqrt();
    if sp > max_speed {
        let k = max_speed / sp;
        s.boss_vx *= k;
        s.boss_vy *= k;
    }

    let mut nx = cur_x + s.boss_vx;
    let mut ny = cur_y + s.boss_vy;

    let x_min = BOSS_ZONE_X1 + BOSS_SIZE * 0.5;
    let x_max = BOSS_ZONE_X2 - BOSS_SIZE * 0.5;
    if nx < x_min {
        nx = x_min;
        s.boss_vx = s.boss_vx.abs() * 0.65;
    } else if nx > x_max {
        nx = x_max;
        s.boss_vx = -s.boss_vx.abs() * 0.65;
    }

    let boundary_y_min = -4000.0;
    let boundary_y_max = 1200.0;
    if ny < boundary_y_min {
        ny = boundary_y_min;
        s.boss_vy = s.boss_vy.abs() * 0.75;
    } else if ny > boundary_y_max {
        ny = boundary_y_max;
        s.boss_vy = -s.boss_vy.abs() * 0.9;
    }
    drop(s);

    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.position = (nx - BOSS_SIZE * 0.5, ny - BOSS_SIZE * 0.5);
    }
}

// ── Drift boss arena asteroids ────────────────────────────────────────────────

fn tick_boss_asteroid_drift(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let s = st.lock().unwrap();
    if !s.boss_active { return; }
    let ids = s.boss_asteroids.clone();
    drop(s);

    // The engine applies momentum to position automatically.
    // This function only bounces asteroids off the arena boundaries.
    let y_min = -3500.0;
    let y_max =  1500.0;

    for id in &ids {
        if let Some(obj) = c.get_game_object_mut(id) {
            // Bounce off arena X walls.
            if obj.position.0 < BOSS_ZONE_X1 {
                obj.momentum.0 = obj.momentum.0.abs();
                obj.position.0 = BOSS_ZONE_X1;
            } else if obj.position.0 + obj.size.0 > BOSS_ZONE_X2 {
                obj.momentum.0 = -obj.momentum.0.abs();
                obj.position.0 = BOSS_ZONE_X2 - obj.size.0;
            }
            // Bounce off Y limits.
            if obj.position.1 < y_min {
                obj.momentum.1 = obj.momentum.1.abs();
                obj.position.1 = y_min;
            } else if obj.position.1 + obj.size.1 > y_max {
                obj.momentum.1 = -obj.momentum.1.abs();
                obj.position.1 = y_max - obj.size.1;
            }
        }
    }
}

// ── Boss shoots bolts at player ───────────────────────────────────────────────

fn tick_boss_shooting(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 { return; }

    if s.boss_shoot_timer > 0 {
        s.boss_shoot_timer -= 1;
        return;
    }
    s.boss_shoot_timer = BOSS_SHOOT_INTERVAL;

    let bolt_id = match s.boss_bolt_free.pop() {
        Some(id) => id,
        None => return,
    };

    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-9999.0, -9999.0));
    let boss_cx = boss_pos.0 + BOSS_SIZE * 0.5;
    let boss_cy = boss_pos.1 + BOSS_SIZE * 0.5;

    let px = s.px;
    let py = s.py;
    let dx = px - boss_cx;
    let dy = py - boss_cy;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let vx = dx / len * BOSS_BOLT_SPEED;
    let vy = dy / len * BOSS_BOLT_SPEED;

    s.boss_bolt_live.push((bolt_id.clone(), vx, vy, BOSS_BOLT_LIFETIME));
    drop(s);

    if let Some(obj) = c.get_game_object_mut(&bolt_id) {
        obj.position = (boss_cx - BOSS_BOLT_W * 0.5, boss_cy - BOSS_BOLT_H * 0.5);
        obj.visible = true;
    }
}

// ── Move boss bolts ───────────────────────────────────────────────────────────

fn tick_boss_bolts(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let recycle: Vec<String>;
    let move_list: Vec<(String, f32, f32)>;
    {
        let mut s = st.lock().unwrap();
        if !s.boss_active { return; }

        let mut rc: Vec<String> = Vec::new();
        let mut mv: Vec<(String, f32, f32)> = Vec::new();
        for (name, vx, vy, ttl) in &mut s.boss_bolt_live {
            mv.push((name.clone(), *vx, *vy));
            if *ttl > 0 { *ttl -= 1; }
            if *ttl == 0 { rc.push(name.clone()); }
        }
        for id in &rc {
            s.boss_bolt_live.retain(|(n, _, _, _)| n != id);
            s.boss_bolt_free.push(id.clone());
        }
        recycle = rc;
        move_list = mv;
    }

    for (name, vx, vy) in &move_list {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.0 += vx;
            obj.position.1 += vy;
        }
    }

    for id in &recycle {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-7000.0, -7000.0);
        }
    }
}

// ── Boss bolt hits player ─────────────────────────────────────────────────────

fn tick_boss_bolt_player_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || s.dead { return; }

    let px = s.px;
    let py = s.py;
    let hit_r = PLAYER_R + BOSS_BOLT_W.max(BOSS_BOLT_H) * 0.5 + 4.0;

    let live_snapshot: Vec<String> = s.boss_bolt_live.iter().map(|(n, _, _, _)| n.clone()).collect();
    let mut hit_ids: Vec<String> = Vec::new();

    for name in &live_snapshot {
        if let Some(obj) = c.get_game_object(name) {
            let bcx = obj.position.0 + BOSS_BOLT_W * 0.5;
            let bcy = obj.position.1 + BOSS_BOLT_H * 0.5;
            let dx = px - bcx;
            let dy = py - bcy;
            if dx * dx + dy * dy < hit_r * hit_r {
                hit_ids.push(name.clone());
            }
        }
    }

    if hit_ids.is_empty() { return; }

    for id in &hit_ids {
        s.boss_bolt_live.retain(|(n, _, _, _)| n != id);
        s.boss_bolt_free.push(id.clone());
    }

    let unhook = s.hooked;
    let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
    let gdir = s.gravity_dir;
    if unhook {
        s.hooked = false;
        s.active_hook = String::new();
    }
    drop(s);

    if unhook {
        c.run(Action::Hide { target: Target::name("rope") });
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * gravity_scale * gdir * BOSS_GRAVITY_SCALE;
        }
    }

    for id in &hit_ids {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-7000.0, -7000.0);
        }
    }
}

// ── Player hits boss body ─────────────────────────────────────────────────────

fn tick_boss_player_hits_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 { return; }

    let px = s.px;
    let py = s.py;

    let boss_pos = c.get_game_object("boss").map(|o| o.position);
    let Some(bpos) = boss_pos else { return };

    let bcx = bpos.0 + BOSS_SIZE * 0.5;
    let bcy = bpos.1 + BOSS_SIZE * 0.5;
    let hit_r = PLAYER_R + BOSS_SIZE * 0.5;

    let dx = px - bcx;
    let dy = py - bcy;
    if dx * dx + dy * dy >= hit_r * hit_r { return; }

    s.boss_hp -= 1;

    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let nx = dx / len;
    let ny = dy / len;
    s.vx = nx * 22.0;
    s.vy = ny * 22.0;

    let hp = s.boss_hp;
    let asteroid_ids = s.boss_asteroids.clone();
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.momentum.0 = nx * 22.0;
        obj.momentum.1 = ny * 22.0;
    }

    if hp <= 0 {
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
        }
        if let Some(obj) = c.get_game_object_mut("boss_hp_bar") {
            obj.visible = false;
        }
        // Hide boss asteroids and remove from space_asteroid_live.
        for id in &asteroid_ids {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.visible = false;
                obj.position = (-8000.0, -8000.0);
                obj.momentum = (0.0, 0.0);
            }
        }
        // Restore player gravity.
        if let Some(obj) = c.get_game_object_mut("player") {
            let cur = obj.gravity;
            let sign = if cur < 0.0 { -1.0_f32 } else { 1.0_f32 };
            obj.gravity = sign * GRAVITY;
        }
        // Recycle live bolts, remove boss asteroids from live list, and
        // resume normal world spawning immediately after the fight.
        let mut s2 = st.lock().unwrap();
        let live: Vec<String> = s2.boss_bolt_live.iter().map(|(n, _, _, _)| n.clone()).collect();
        for id in &live {
            s2.boss_bolt_live.retain(|(n, _, _, _)| n != id);
            s2.boss_bolt_free.push(id.clone());
        }
        s2.space_asteroid_live.retain(|id| !asteroid_ids.contains(id));

        // Exit boss mode so tick_spawning/tick_culling run again.
        s2.boss_active = false;
        s2.boss_spawned = false;
        s2.boss_cleared = false;
        s2.boss_entry_ticks = 0;
        s2.boss_shoot_timer = BOSS_SHOOT_INTERVAL;

        // Rewind spawn frontiers behind the player so content repopulates now,
        // not only after travelling far past old rightmost markers.
        let backfill_x = s2.px - GEN_AHEAD * 0.35;
        s2.rightmost_x = s2.rightmost_x.min(backfill_x);
        s2.pad_rightmost = s2.pad_rightmost.min(backfill_x);
        s2.spinner_rightmost = s2.spinner_rightmost.min(backfill_x);
        s2.coin_rightmost = s2.coin_rightmost.min(backfill_x);
        s2.flip_rightmost = s2.flip_rightmost.min(backfill_x);
        s2.score_x2_rightmost = s2.score_x2_rightmost.min(backfill_x);
        s2.zero_g_rightmost = s2.zero_g_rightmost.min(backfill_x);
        s2.gate_rightmost = s2.gate_rightmost.min(backfill_x);
        s2.gwell_rightmost = s2.gwell_rightmost.min(backfill_x);
        s2.turret_rightmost = s2.turret_rightmost.min(backfill_x);
        s2.rocket_pad_rightmost = s2.rocket_pad_rightmost.min(backfill_x);
        s2.space_asteroid_rightmost = s2.space_asteroid_rightmost.min(backfill_x);

        // Ensure hook spawning can restart even if pending queue was exhausted.
        if s2.pending.is_empty() {
            let mut seed = s2.seed;
            let mut gen_head_x = s2.gen_head_x;
            let mut gen_head_y = s2.gen_head_y;
            let batch = gen_hook_batch(&mut seed, backfill_x, &mut gen_head_x, &mut gen_head_y, s2.distance);
            s2.seed = seed;
            s2.gen_head_x = gen_head_x;
            s2.gen_head_y = gen_head_y;
            s2.pending.extend(batch);
        }
        drop(s2);

        // Mark boss as cleared for this run to prevent immediate re-entry at same X.
        c.set_var("boss_mode_cleared", true);

        for id in &live {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.visible = false;
                obj.position = (-7000.0, -7000.0);
            }
        }
    }
}

// ── Boss HP bar HUD update ────────────────────────────────────────────────────

fn tick_boss_hud(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }

    let hp    = s.boss_hp;
    let dirty = hp != s.hud_last_boss_hp;
    if !dirty { return; }
    s.hud_last_boss_hp = hp;
    drop(s);

    let fill = (hp as f32 / BOSS_MAX_HP as f32).clamp(0.0, 1.0);
    let w = BOSS_HP_BAR_W as u32;
    let h = BOSS_HP_BAR_H as u32;
    let fill_px = (fill * w as f32).round() as u32;

    let mut img = image::RgbaImage::new(w, h);
    for row in 0..h {
        for col in 0..w {
            let color = if col < fill_px {
                image::Rgba([C_BOSS_HP_FILL.0, C_BOSS_HP_FILL.1, C_BOSS_HP_FILL.2, 255])
            } else {
                image::Rgba([C_BOSS_HP_BG.0, C_BOSS_HP_BG.1, C_BOSS_HP_BG.2, 200])
            };
            img.put_pixel(col, row, color);
        }
    }

    if let Some(obj) = c.get_game_object_mut("boss_hp_bar") {
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (BOSS_HP_BAR_W, BOSS_HP_BAR_H), 0.0),
            image: img.into(),
            color: None,
        });
        obj.visible = fill > 0.0;
    }
}
