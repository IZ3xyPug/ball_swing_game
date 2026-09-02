use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use crate::images::circle_cached;
use crate::scenes::game::bootstrap::hook_asteroid_anim_for_spawn;
use crate::scenes::game::helpers::center_warp_on_player;
use crate::scenes::game::space_zone::wormhole2_template;
#[allow(unused_imports)]
use super::*;

/// Position the weakpoint marker rings on the boss body, visible only while the
/// boss is up, so players can see where to land buffed hits.
pub(crate) fn tick_boss_weakpoints(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let active = {
        let s = st.lock().unwrap();
        s.boss_active && s.boss_spawned && s.boss_hp > 0
    };
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    for (i, (ox, oy)) in BOSS_WEAKPOINT_OFFSETS.iter().enumerate() {
        let id = format!("boss_weak_{i}");
        let Some(obj) = c.get_game_object_mut(&id) else { continue; };
        if active {
            let r = BOSS_WEAKPOINT_R;
            obj.position = (bcx + ox - r, bcy + oy - r);
            obj.visible = true;
        } else {
            obj.visible = false;
        }
    }
}

/// The Sun Eater's darkness attack.
///
/// Runs the SAME night-mode post pass and player lamp chain the eclipse
/// approach uses (`eclipse::begin_night_mode`), at the `boss_dark` preset.
/// Previously this only dropped the ambient — but quartz lighting is
/// multiplicative, so an ambient of 0.06 with no light source in the world is
/// not a dark room, it is a blank screen: there is nothing for the lamp to
/// restore because there is no lamp. The attack was unreadable and unfair for
/// exactly that reason.
///
/// With the shared entry point the player becomes the light source for three
/// seconds, the node markers stay lit so the route is still findable, and the
/// vignette closes in — the attack takes away the room, not the game.
pub(crate) fn tick_boss_darkness(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 { return; }

    if s.boss_dark_active {
        s.boss_dark_ticks = s.boss_dark_ticks.saturating_sub(1);
        let ending = s.boss_dark_ticks == 0;
        if ending {
            s.boss_dark_active = false;
            s.boss_dark_cooldown = BOSS_DARK_INTERVAL;
        }
        drop(s);
        if ending {
            c.set_var("boss_darkness", false);
            crate::scenes::game::eclipse::end_night_mode(c);
            crate::scenes::game::eclipse::kill_node_lights(c);
            if c.has_lighting() {
                c.set_ambient(Color(255, 255, 255, 255), 1.0);
            }
        } else {
            // Hold the markers lit for the whole attack. They are attached
            // per pool slot, so this only has to match `enabled` to `visible`.
            crate::scenes::game::eclipse::drive_node_lights(c, ECLIPSE_NODE_LIGHT_INTENSITY);
        }
    } else {
        s.boss_dark_cooldown = s.boss_dark_cooldown.saturating_sub(1);
        let starting = s.boss_dark_cooldown == 0;
        if starting {
            s.boss_dark_active = true;
            s.boss_dark_ticks = BOSS_DARK_DURATION;
        }
        drop(s);
        if starting {
            c.set_var("boss_darkness", true);
            crate::scenes::game::eclipse::ensure_node_lights(c);
            crate::scenes::game::eclipse::begin_night_mode(c, crate::scenes::game::eclipse::NightPost::boss_dark());
            crate::scenes::game::eclipse::drive_node_lights(c, ECLIPSE_NODE_LIGHT_INTENSITY);
            if c.has_lighting() {
                c.set_ambient(Color(10, 10, 25, 255), BOSS_DARK_AMBIENT);
            }
        }
    }
}

/// Position the barrier and generator nodes across the arena.
pub(crate) fn spawn_generators_and_barrier(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (zx1, zx2) = arena_bounds(c);
    let zone_w = zx2 - zx1;
    {
        let mut s = st.lock().unwrap();
        s.boss_generators.clear();
        s.boss_generator_hp.clear();
        for _ in 0..BOSS_GENERATOR_COUNT {
            s.boss_generators.push(String::new());
            s.boss_generator_hp.push(BOSS_GENERATOR_HP);
        }
        s.boss_barrier_up = true;
        s.boss_final_phase = false;
        s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH;
        s.boss_lunge_ticks = 0;
        s.boss_lunge_target = (0.0, 0.0);
    }
    for i in 0..BOSS_GENERATOR_COUNT {
        let id = format!("boss_gen_{i}");
        let frac = i as f32 / (BOSS_GENERATOR_COUNT - 1).max(1) as f32;
        let gx = zx1 + zone_w * (0.15 + frac * 0.7);
        let gy = -700.0 - frac * 2200.0;
        {
            let mut s = st.lock().unwrap();
            if i < s.boss_generators.len() {
                s.boss_generators[i] = id.clone();
            }
        }
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.visible = true;
            obj.position = (gx - BOSS_GENERATOR_R, gy - BOSS_GENERATOR_R);
            obj.momentum = (0.0, 0.0);
            obj.rotation_momentum = 0.0;
        }
    }
    if let Some(obj) = c.get_game_object_mut("boss_barrier") {
        obj.position = (zx1, BOSS_BARRIER_Y);
        obj.visible = true;
    }
}

/// Damage generators (buffed player hits, or the boss crashing into them), and
/// drop the barrier once all are down (entering the final bait-and-bail phase).
/// Show the boss forcefield ring while the generators are still up, and the
/// arena boundary forcefield while the boss fight is active.
pub(crate) fn tick_boss_forcefield(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (active, spawned, generators_up, hp) = {
        let s = st.lock().unwrap();
        let gens_up = !s.boss_generator_hp.is_empty() && s.boss_generator_hp.iter().any(|&hp| hp > 0);
        (s.boss_active, s.boss_spawned, gens_up, s.boss_hp)
    };
    let fighting = active && hp > 0;

    for id in ["boss_boundary_b", "boss_boundary_t", "boss_boundary_l", "boss_boundary_r"] {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = fighting;
        }
    }

    let boss_center = c.get_game_object("boss")
        .map(|o| (o.position.0 + BOSS_SIZE * 0.5, o.position.1 + BOSS_SIZE * 0.5));

    if let Some(obj) = c.get_game_object_mut("boss_forcefield") {
        if fighting && spawned && generators_up {
            if let Some((bcx, bcy)) = boss_center {
                let d = BOSS_SIZE * 1.5;
                obj.position = (bcx - d * 0.5, bcy - d * 0.5);
                obj.size = (d, d);
                obj.update_image_shape();
                obj.visible = true;
            } else {
                obj.visible = false;
            }
        } else {
            obj.visible = false;
        }
    }
}

pub(crate) fn tick_generators(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (px, py, buffed, boss_pos) = {
        let s = st.lock().unwrap();
        if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 {
            return;
        }
        (
            s.px,
            s.py,
            s.player_buff > 0,
            c.get_game_object("boss").map(|o| o.position).unwrap_or((-9999.0, -9999.0)),
        )
    };
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;

    let gens: Vec<String> = st.lock().unwrap().boss_generators.clone();
    let mut damaged: Vec<usize> = Vec::new();
    for (i, id) in gens.iter().enumerate() {
        let Some(obj) = c.get_game_object(id) else { continue; };
        if !obj.visible {
            continue;
        }
        let gx = obj.position.0 + obj.size.0 * 0.5;
        let gy = obj.position.1 + obj.size.1 * 0.5;
        if buffed {
            let dx = px - gx;
            let dy = py - gy;
            let r = PLAYER_R + BOSS_GENERATOR_R;
            if dx * dx + dy * dy < r * r {
                damaged.push(i);
                continue;
            }
        }
        // Boss crashing into the generator damages it (the "lure" path).
        let dx = bcx - gx;
        let dy = bcy - gy;
        let r = BOSS_SIZE * 0.5 + BOSS_GENERATOR_R;
        if dx * dx + dy * dy < r * r {
            damaged.push(i);
        }
    }
    if damaged.is_empty() {
        return;
    }
    {
        let mut s = st.lock().unwrap();
        for i in damaged {
            if i < s.boss_generator_hp.len() {
                s.boss_generator_hp[i] -= 1;
            }
        }
    }
    let mut all_down = true;
    {
        let mut s = st.lock().unwrap();
        for i in 0..s.boss_generator_hp.len() {
            if s.boss_generator_hp[i] <= 0 {
                if let Some(id) = s.boss_generators.get(i).cloned() {
                    if let Some(obj) = c.get_game_object_mut(&id) {
                        obj.visible = false;
                    }
                }
            } else {
                all_down = false;
            }
        }
    }
    if all_down {
        let mut s = st.lock().unwrap();
        s.boss_barrier_up = false;
        s.boss_final_phase = true;
        drop(s);
        if let Some(obj) = c.get_game_object_mut("boss_barrier") {
            obj.visible = false;
        }
    }
}

/// While the barrier is up, clamp the player (and boss) from crossing into the
/// sun side of the arena.
pub(crate) fn tick_barrier(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_barrier_up {
        return;
    }
    if s.py < BOSS_BARRIER_Y {
        s.py = BOSS_BARRIER_Y;
        if s.vy < 0.0 {
            s.vy = 0.0;
        }
        drop(s);
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.position.1 = BOSS_BARRIER_Y - PLAYER_R;
            if obj.momentum.1 < 0.0 {
                obj.momentum.1 = 0.0;
            }
        }
        return;
    }
    // Keep the boss on the safe side of the barrier too.
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((0.0, 0.0));
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    if bcy < BOSS_BARRIER_Y {
        drop(s);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position.1 = BOSS_BARRIER_Y - BOSS_SIZE * 0.5;
        }
    }
}

/// Final phase: the boss periodically lunges at where the player *was* (a
/// telegraphed bait), which is a dodge test and a window to counter-attack.
///
/// The lunge used to KILL the boss outright if it carried past the sun line —
/// the "bait-and-bail" finisher from the original design. That is gone: the sun
/// is no longer part of this fight, so the only way to end it is to bring the
/// barrier down by destroying both generators and then damage the boss with a
/// buffed weakpoint hit. The lunge is now purely an attack the player dodges.
pub(crate) fn tick_desperation(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (final_phase, active, spawned, hp) = {
        let s = st.lock().unwrap();
        (s.boss_final_phase, s.boss_active, s.boss_spawned, s.boss_hp)
    };
    if !final_phase || !active || !spawned || hp <= 0 {
        return;
    }

    let mut s = st.lock().unwrap();
    if s.boss_lunge_telegraph > 0 {
        s.boss_lunge_telegraph -= 1;
        if s.boss_lunge_telegraph == 0 {
            // Lock target to the player's current position (the bait).
            s.boss_lunge_target = (s.px, s.py);
            s.boss_lunge_ticks = 90;
        }
        drop(s);
        return;
    }
    if s.boss_lunge_ticks > 0 {
        s.boss_lunge_ticks -= 1;
        let (tx, ty) = s.boss_lunge_target;
        let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((0.0, 0.0));
        let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
        let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
        let dx = tx - bcx;
        let dy = ty - bcy;
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let spd = 42.0;
        let nvx = dx / d * spd;
        let nvy = dy / d * spd;
        let nx = bcx + nvx;
        let ny = bcy + nvy;
        s.boss_vx = nvx;
        s.boss_vy = nvy;
        let done = s.boss_lunge_ticks == 0;
        drop(s);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (nx - BOSS_SIZE * 0.5, ny - BOSS_SIZE * 0.5);
        }
        // NOTE: no sun-line kill here any more. The boss overshooting the top
        // of the arena used to end the fight instantly, which short-circuited
        // the generators-then-weakpoint loop that is now the whole fight.
        // Clamp instead, so a lunge cannot carry it out of the arena.
        let ny = ny.max(BOSS_ARENA_TOP_Y);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position.1 = ny - BOSS_SIZE * 0.5;
        }
        if done {
            let mut s = st.lock().unwrap();
            s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH;
        }
        return;
    }
    s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH.max(s.boss_lunge_telegraph);
}

pub(crate) fn tick_boss_appearance(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || s.boss_spawned || s.boss_stasis_active { return; }

    s.boss_entry_ticks = s.boss_entry_ticks.saturating_add(1);
    if s.boss_entry_ticks < BOSS_ENTRY_DELAY_TICKS { return; }

    s.boss_spawned = true;

    // Spawn phase starts from the top of the lissajous sweep.
    s.boss_phase = std::f32::consts::FRAC_PI_2;
    let boss_name = s.boss_kind.name();
    drop(s);

    // Show this boss's name on the banner (set once, at spawn).
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../../assets/font.ttf")) {
        let sc = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("boss_name_text") {
            obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                boss_name, &font, 42.0 * sc, Color(200, 60, 220, 255), 1000.0 * sc,
            )));
        }
    }

    // Place boss at its initial lissajous position (top-center).
    let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
    let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.position = (spawn_x, spawn_y);
        obj.visible = true;
    }
}

pub(crate) fn tick_boss_movement(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (zx1, zx2) = arena_bounds(c);
    let (cur_x, cur_y) = if let Some(obj) = c.get_game_object("boss") {
        (obj.position.0 + BOSS_SIZE * 0.5, obj.position.1 + BOSS_SIZE * 0.5)
    } else {
        (arena_center_x(c), BOSS_Y_CENTER)
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

    // Map to arena bounds: X = [-1,1] → [20000, 34000], Y = [-1,1] → [-6000, 2400]
    let tx_base = arena_center_x(c) + x_liss * BOSS_ARENA_HALF_W * 0.95;
    let y_min = -6000.0;
    let y_max = 2400.0;
    let y_center = (y_min + y_max) * 0.5;
    let y_half_range = (y_max - y_min) * 0.5;
    let ty_base = y_center + y_liss * y_half_range * 0.92;

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
            zx1 + BOSS_SIZE * 0.5
        } else {
            zx2 - BOSS_SIZE * 0.5
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

    let x_min = zx1 + BOSS_SIZE * 0.5;
    let x_max = zx2 - BOSS_SIZE * 0.5;
    if nx < x_min {
        nx = x_min;
        s.boss_vx = s.boss_vx.abs() * 0.65;
    } else if nx > x_max {
        nx = x_max;
        s.boss_vx = -s.boss_vx.abs() * 0.65;
    }

    let boundary_y_min = -6000.0;
    let boundary_y_max = 2400.0;
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

pub(crate) fn tick_boss_asteroid_drift(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (zx1, zx2) = arena_bounds(c);
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
            if obj.position.0 < zx1 {
                obj.momentum.0 = obj.momentum.0.abs();
                obj.position.0 = zx1;
            } else if obj.position.0 + obj.size.0 > zx2 {
                obj.momentum.0 = -obj.momentum.0.abs();
                obj.position.0 = zx2 - obj.size.0;
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

pub(crate) fn tick_boss_shooting(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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

pub(crate) fn tick_boss_bolts(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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

pub(crate) fn tick_boss_bolt_player_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-7000.0, -7000.0);
        }
    }

    // A boss bolt normally breaks the tether and costs a heart. While buffed,
    // the buff absorbs up to BUFF_ABSORB_MAX hits with no damage; after the last
    // one it ends early. Refreshing the buff resets the absorb count.
    let absorb = s.player_buff > 0 && s.buff_absorbs > 0;
    if absorb {
        s.buff_absorbs -= 1;
        s.buff_hit_flash = 6;
        if s.buff_absorbs == 0 {
            s.player_buff = 0;
            s.buff_timer = 0;
        }
        drop(s);
        // Brief cyan flash so the absorb reads clearly.
        if let Some(cam) = c.camera_mut() {
            cam.flash_with(Color(110, 230, 255, 200), 0.25, FlashMode::Pulse, FlashEase::Sharp, 0.7, 0.0);
        }
        return;
    }

    // A boss bolt breaks the tether and costs a heart.
    let hooked = s.hooked;
    if hooked {
        s.hooked = false;
        s.active_hook = String::new();
    }
    drop(s);
    if hooked {
        c.run(Action::Hide { target: Target::name("rope") });
    }
    let _over = crate::scenes::game::hearts::lose_heart(c, st);
    // Restore light boss gravity after the hit.
    if let Some(obj) = c.get_game_object_mut("player") {
        let gdir = st.lock().unwrap().gravity_dir;
        obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE * gdir;
    }
}

pub(crate) fn tick_boss_player_hits_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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

    // ── Buffed weakpoint hit damages the boss; unprotected body contact ─────
    // knocks the player back and disconnects the tether (no boss damage).
    let buffed = s.player_buff > 0;
    let near_weakpoint = BOSS_WEAKPOINT_OFFSETS.iter().any(|(wx, wy)| {
        let wx = bcx + wx;
        let wy = bcy + wy;
        let ddx = px - wx;
        let ddy = py - wy;
        ddx * ddx + ddy * ddy < BOSS_WEAKPOINT_R * BOSS_WEAKPOINT_R
    });

    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let nx = dx / len;
    let ny = dy / len;

    // Boss is forcefield-protected until ALL generators are destroyed.
    // (A debug var can force it down for headless weakpoint validation.)
    let forcefield_debug = matches!(c.get_var("debug_boss_forcefield_down"), Some(Value::Bool(true)));
    let generators_down = forcefield_debug
        || (!s.boss_generator_hp.is_empty() && s.boss_generator_hp.iter().all(|&hp| hp <= 0));

    let mut contact_damage = false;
    let (nwx, nwy, did_unhook) = if buffed && near_weakpoint && generators_down {
        // Buffed weakpoint hit with the forcefield down: damage the boss.
        s.boss_hp -= 1;
        s.buff_hit_flash = 20;
        (nx * 26.0, ny * 26.0, false)
    } else {
        // Unbuffed contact with the boss body COSTS a heart, it does not merely
        // bounce. Touching the thing you are fighting has to be a mistake, or
        // the buff is only a damage tool and never a defensive decision — and
        // the fight degenerates into riding the boss for free.
        let unhook = s.hooked;
        if unhook {
            s.hooked = false;
            s.active_hook = String::new();
        }
        if !buffed {
            contact_damage = true;
        }
        (nx * 34.0, ny * 34.0, unhook)
    };
    s.vx = nwx;
    s.vy = nwy;

    let hp = s.boss_hp;
    let asteroid_ids = s.boss_asteroids.clone();
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.momentum.0 = nwx;
        obj.momentum.1 = nwy;
    }
    if did_unhook {
        c.run(Action::Hide { target: Target::name("rope") });
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE;
        }
    }

    if contact_damage {
        // Applied after the State lock is dropped — `lose_heart` takes it too.
        crate::scenes::game::hearts::lose_heart(c, st);
    }

    if hp <= 0 {
        // Recycle the objects the fight owns, then hand off to the real victory
        // flow.
        //
        // This used to be an inline copy of the end-of-fight teardown, and it
        // had drifted from `finish_boss`: it awarded no meta currency, showed no
        // victory stasis, and never advanced `boss_index` — so with the sun-line
        // finisher removed, killing the boss by weakpoint damage (the only way
        // now, and the intended one) would have paid nothing and re-armed the
        // SAME fight at the same threshold.
        let asteroid_ids2 = asteroid_ids.clone();
        {
            let mut s2 = st.lock().unwrap();
            let live: Vec<String> = s2.boss_bolt_live.iter().map(|(n, _, _, _)| n.clone()).collect();
            for id in &live {
                s2.boss_bolt_live.retain(|(n, _, _, _)| n != id);
                s2.boss_bolt_free.push(id.clone());
            }
            s2.space_asteroid_live.retain(|id| !asteroid_ids2.contains(id));
            s2.boss_dark_active = false;
            s2.boss_dark_ticks = 0;
            s2.boss_dark_cooldown = BOSS_DARK_INTERVAL;
            drop(s2);
            for id in &live {
                if let Some(obj) = c.get_game_object_mut(id) {
                    obj.visible = false;
                    obj.position = (-7000.0, -7000.0);
                }
            }
        }
        for id in &asteroid_ids {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.visible = false;
                obj.position = (-8000.0, -8000.0);
                obj.momentum = (0.0, 0.0);
            }
        }
        // Restore player gravity (the arena runs at a reduced scale).
        if let Some(obj) = c.get_game_object_mut("player") {
            let cur = obj.gravity;
            let sign = if cur < 0.0 { -1.0_f32 } else { 1.0_f32 };
            obj.gravity = sign * GRAVITY;
        }

        // Meta reward, victory stasis, and — via `complete_boss_finish` when the
        // player tethers out — the frontier rewind and the schedule advance that
        // arms the next fight.
        finish_boss(c, st);
    }
}
