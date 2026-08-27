// ── boss.rs — Boss fight logic ────────────────────────────────────────────────
// All logic gated behind boss_active — zero impact on free-roam.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use crate::images::circle_cached;
use super::bootstrap::hook_asteroid_anim_for_spawn;
use super::helpers::center_warp_on_player;
use crate::scenes::game::space_zone::wormhole2_template;

pub fn tick_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Published every frame: several systems (distance tracking, the headless
    // harness, the HUD) need to know an arena is active, and deriving it from
    // boss HP is wrong during entry and victory stasis.
    {
        let active = st.lock().unwrap().boss_active;
        c.set_var("boss_active", active);
    }
    tick_boss_zone_entry(c, st);
    tick_boss_stasis(c, st);
    tick_boss_appearance(c, st);
    tick_boss_movement(c, st);
    tick_boss_asteroid_drift(c, st);
    tick_boss_shooting(c, st);
    tick_boss_darkness(c, st);
    tick_boss_weakpoints(c, st);
    tick_warp_flash(c, st);
    tick_boss_forcefield(c, st);
    tick_generators(c, st);
    tick_barrier(c, st);
    tick_desperation(c, st);
    tick_boss_bolts(c, st);
    tick_boss_bolt_player_collision(c, st);
    tick_boss_player_hits_boss(c, st);
    tick_boss_hud(c, st);
    tick_boss_indicators(c, st);
    tick_boss_lights(c, st);
    tick_buff_node_elec(c, st);
}

/// Push a small electricity effect over each buff tether node so they read as
/// distinct from the regular grab nodes (the same round electricity the player
/// gets while buffed).
fn tick_buff_node_elec(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let nodes: Vec<(f32, f32)> = {
        let s = st.lock().unwrap();
        s.live_hooks.iter().filter_map(|id| {
            let obj = c.get_game_object(id)?;
            if obj.tags.iter().any(|t| t == BUFF_HOOK_TAG) {
                Some((obj.position.0 + obj.size.0 * 0.5, obj.position.1 + obj.size.1 * 0.5))
            } else {
                None
            }
        }).collect()
    };
    if nodes.is_empty() { return; }
    for (cx, cy) in nodes {
        super::fx::push_electric_fx(c, (cx, cy), (HOOK_R * 2.6, HOOK_R * 2.6), (0.6, 0.95, 1.0, 0.7));
    }
}

/// Position the weakpoint marker rings on the boss body, visible only while the
/// boss is up, so players can see where to land buffed hits.
fn tick_boss_weakpoints(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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

// ── Boss darkness attack (uses the quartz lighting system) ───────────────────

fn tick_boss_darkness(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 { return; }

    if s.boss_dark_active {
        s.boss_dark_ticks = s.boss_dark_ticks.saturating_sub(1);
        if s.boss_dark_ticks == 0 {
            s.boss_dark_active = false;
            s.boss_dark_cooldown = BOSS_DARK_INTERVAL;
            drop(s);
            c.set_var("boss_darkness", false);
            if c.has_lighting() {
                c.set_ambient(Color(255, 255, 255, 255), 1.0);
            }
        }
    } else {
        s.boss_dark_cooldown = s.boss_dark_cooldown.saturating_sub(1);
        if s.boss_dark_cooldown == 0 {
            s.boss_dark_active = true;
            s.boss_dark_ticks = BOSS_DARK_DURATION;
            drop(s);
            c.set_var("boss_darkness", true);
            if c.has_lighting() {
                c.set_ambient(Color(10, 10, 25, 255), 0.06);
            }
        }
    }
}

// ── Zone entry + arena clear ──────────────────────────────────────────────────

// ── Arena placement ──────────────────────────────────────────────────────────
//
// The arena used to be two constants, so a run could only ever hold one fight.
// It is now a per-fight region recorded on the canvas at entry, which is what
// lets `mode::boss_trigger_distance` schedule several. The player is warped in
// and out, so the region can sit anywhere — it does not have to be reachable by
// swinging, and successive arenas simply step further along the X axis.

/// Left and right walls of the arena for the fight currently being set up.
fn arena_bounds(c: &Canvas) -> (f32, f32) {
    let x1 = match c.get_var("boss_arena_x1") {
        Some(Value::F32(v)) => v,
        Some(Value::F64(v)) => v as f32,
        _ => BOSS_ZONE_X1,
    };
    let x2 = match c.get_var("boss_arena_x2") {
        Some(Value::F32(v)) => v,
        Some(Value::F64(v)) => v as f32,
        _ => BOSS_ZONE_X2,
    };
    (x1, x2)
}

/// Place the arena for fight `index`. Successive arenas are laid end to end
/// with a wide gap, far past anything the normal generator will ever reach.
fn place_arena(c: &mut Canvas, index: u32) {
    let stride = (BOSS_ZONE_X2 - BOSS_ZONE_X1) + BOSS_ARENA_GAP;
    let x1 = crate::mode::BOSS_ARENA_ORIGIN_X + stride * index as f32;
    let x2 = x1 + (BOSS_ZONE_X2 - BOSS_ZONE_X1);
    c.set_var("boss_arena_x1", Value::F32(x1));
    c.set_var("boss_arena_x2", Value::F32(x2));
}

fn tick_boss_zone_entry(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Which fights this run has, and where, is the mode's business.
    let mode = crate::mode::current_mode(c);
    // `boss_mode_active` remains an override so the debug/test harness can force
    // a fight in any mode.
    let forced_mode = matches!(c.get_var("boss_mode_active"), Some(Value::Bool(true)));
    if !mode.has_bosses() && !forced_mode { return; }

    let mut s = st.lock().unwrap();
    // The distance that triggers the NEXT fight. `None` means this run has no
    // fights left — the schedule is exhausted, so the boss flow is done.
    let threshold = match crate::mode::boss_trigger_distance(mode, s.boss_index) {
        Some(d) => SPAWN_X + d,
        None if forced_mode => BOSS_THRESHOLD_X,
        None => return,
    };
    // Only fire in normal game, not space mode.
    if s.in_space_mode || s.space_launch_active { return; }
    if s.dead { return; }

    // Spawn the pre-portal approach grapple nodes once, as the player nears the
    // boss threshold, so there is always a swing path up to the portal. Drop the
    // lock first because the spawner takes its own.
    if !s.boss_active && !s.boss_approach_nodes_spawned
        && s.px >= threshold - BOSS_APPROACH_RANGE
    {
        drop(s);
        spawn_boss_approach_nodes(c, st);
        s = st.lock().unwrap();
    }

    // Boss entry: reach the threshold, or be force-warped (debug/test harness).
    // Safe read: `get_bool` panics if the var is unset in a normal boss run.
    let force = matches!(c.get_var("force_boss_warp"), Some(Value::Bool(true)));
    if !s.boss_active && (s.px >= threshold || force) {
        // Remember where the level was left, so victory returns the player to
        // the run rather than stranding them in the arena's stretch of X.
        s.boss_return_x = s.px;
        s.boss_return_y = s.py;
        let index = s.boss_index;
        drop(s);
        place_arena(c, index);
        let mut s = st.lock().unwrap();
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

        // The player has crossed into the boss arena — hide the threshold marker.
        if let Some(obj) = c.get_game_object_mut("boss_threshold_marker") {
            obj.visible = false;
        }
        if let Some(obj) = c.get_game_object_mut("boss_marker_arrow") {
            obj.visible = false;
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
        // Spawn climbable tether nodes across the arena so the player can
        // swing up to reach the upper-sky boss.
        spawn_arena_tether_nodes(c, st);
        // Warp the player into the arena with a wormhole-style flash.
        warp_player_into_arena(c, st);
        // Last-boss set dressing: barrier + generators.
        spawn_generators_and_barrier(c, st);
        // Hold the player in a stasis orbit around a safe tether node so they
        // can get their bearings before the battle starts. Tether to a node to
        // begin (see `tick_boss_stasis`).
        enter_boss_stasis(c, st);
        return;
    }

    // Clamp player inside boss zone while boss is alive.
    if s.boss_hp > 0 {
        let (zx1, zx2) = arena_bounds(c);
        let half = PLAYER_R;
        if s.px < zx1 + half {
            s.px = zx1 + half;
            s.vx = s.vx.max(0.0);
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                if obj.position.0 < zx1 + half - PLAYER_R {
                    obj.position.0 = zx1 + half - PLAYER_R;
                }
                if obj.momentum.0 < 0.0 { obj.momentum.0 = 0.0; }
            }
        } else if s.px > zx2 - half {
            s.px = zx2 - half;
            s.vx = s.vx.min(0.0);
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                if obj.position.0 > zx2 - half - PLAYER_R {
                    obj.position.0 = zx2 - half - PLAYER_R;
                }
                if obj.momentum.0 > 0.0 { obj.momentum.0 = 0.0; }
            }
        }
    }
}

fn place_boss_asteroids(c: &mut Canvas, asteroid_ids: &[String]) {
    let (zx1, zx2) = arena_bounds(c);
    let anim = hook_asteroid_anim_for_spawn();
    let zone_w = zx2 - zx1;
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
        let x = zx1 + rx * zone_w;
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
            let x = zx1 + zone_w * t + ((i as f32 * 173.0) % 500.0 - 250.0);
            let y = Y_TOP + (Y_BOT - Y_TOP) * ((i as f32 * 0.618_033_95) % 1.0)
                + ((i as f32 * 97.0) % 280.0 - 140.0);
            points.push((x.clamp(zx1, zx2), y.clamp(Y_TOP, Y_BOT)));
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

/// Activate a single arena tether node (image, tags, glow) and register it live.
fn activate_arena_tether_node(c: &mut Canvas, s: &mut State, id: String, hx: f32, hy: f32, is_buff: bool) {
    s.live_hooks.push(id.clone());
    if let Some(obj) = c.get_game_object_mut(&id) {
        obj.visible = true;
        obj.position = (hx - HOOK_R, hy - HOOK_R);
        obj.size = (HOOK_R * 2.0, HOOK_R * 2.0);
        obj.gravity = 0.0;
        obj.momentum = (0.0, 0.0);
        obj.rotation_momentum = 0.0;
        obj.collision_mode = CollisionMode::NonPlatform;
        obj.tags.retain(|t| t != "arena_node" && t != BUFF_HOOK_TAG);
        obj.tags.push("arena_node".into());
        if !obj.tags.iter().any(|t| t == "hook") {
            obj.tags.push("hook".into());
        }
        if is_buff {
            obj.tags.push(BUFF_HOOK_TAG.into());
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
                image: circle_cached(HOOK_R as u32, C_BUFF_HOOK.0, C_BUFF_HOOK.1, C_BUFF_HOOK.2),
                color: None,
            });
            obj.set_glow(GlowConfig { color: Color(110, 230, 255, 255), width: 16.0 });
        } else {
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
                image: circle_cached(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2),
                color: None,
            });
            obj.clear_glow();
        }
        obj.clear_highlight();
    } else {
        s.live_hooks.retain(|n| n != &id);
        s.pool_free.push(id);
    }
}

/// Spawn a small grid of climbable grab nodes spanning the arena X and the
/// upper-sky boss Y band (+300 down to −3600), so the player can swing up to
/// reach the boss at y ≈ −2500. Adds staggered gap-fill nodes between the main
/// columns so there are no large horizontal gaps.
fn spawn_arena_tether_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (zx1, zx2) = arena_bounds(c);
    let mut s = st.lock().unwrap();
    let zone_w = zx2 - zx1;
    let cols = 5;
    let rows = 6;
    let total = cols * rows;
    for i in 0..total {
        let Some(id) = s.pool_free.pop() else { break; };
        let col = i % cols;
        let row = i / cols;
        let frac = row as f32 / (rows - 1).max(1) as f32;
        let hy = 300.0 - frac * 4400.0; // +300 … −4100
        let hx = zx1 + zone_w * (0.5 + (col as f32 - (cols as f32 - 1.0) * 0.5) * 0.22);
        // Every third node is a buff node so the player can get a buff mid-fight.
        activate_arena_tether_node(c, &mut *s, id, hx, hy, i % 3 == 0);
    }

    // Staggered gap-fill nodes in the wide gaps between the main columns:
    // one near the bottom, one at mid height, one nearer the top.
    let gap_fracs = [0.17, 0.39, 0.61, 0.83];
    let gap_ys = [-3600.0, -2000.0, 500.0];
    for gf in gap_fracs {
        let gx = zx1 + zone_w * gf;
        for &gy in &gap_ys {
            let Some(id) = s.pool_free.pop() else { break; };
            activate_arena_tether_node(c, &mut *s, id, gx, gy, false);
        }
    }
}

/// Reveal the boss threshold marker once the player approaches the portal. The
/// procedural hooks now cover the approach densely (after the stride tightening
/// in constants.rs), so no extra approach nodes are placed here.
fn spawn_boss_approach_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.boss_approach_nodes_spawned {
        return;
    }
    s.boss_approach_nodes_spawned = true;
    drop(s);

    // Reveal the huge black-hole threshold marker so the player knows they are
    // heading into something special.
    if let Some(obj) = c.get_game_object_mut("boss_threshold_marker") {
        obj.visible = true;
    }
    if let Some(obj) = c.get_game_object_mut("boss_marker_arrow") {
        obj.visible = true;
    }
}

/// Warp the player into the boss arena (wormhole-style flash) at the bottom of
/// the tether grid, so the fight is entered cleanly rather than the player
/// having to climb from the normal zone.
fn warp_player_into_arena(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let warp_x = BOSS_ARENA_CENTER_X;
    let warp_y = 200.0;
    {
        let mut s = st.lock().unwrap();
        s.px = warp_x;
        s.py = warp_y;
        s.vx = 0.0;
        s.vy = 0.0;
        s.hooked = false;
        s.active_hook = String::new();
        s.hook_x = warp_x;
        s.hook_y = warp_y;
        s.rope_len = RESPAWN_ORBIT_R;
        s.cannon_captured = false;
        s.in_space_mode = false;
    }
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (warp_x - PLAYER_R, warp_y - PLAYER_R);
        obj.momentum = (0.0, 0.0);
        obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE;
        obj.visible = true;
    }
    c.run(Action::Hide { target: Target::name("rope") });
    c.set_var("rope_visible_at_pause", false);
    // Show the wormhole gif over the player during the warp — a defined-size
    // portal so the opening reads as a visible wormhole (not an invisible
    // full-screen smear). tick_warp_flash re-centres it on the player.
    if let Some(obj) = c.get_game_object_mut("warp_flash") {
        obj.set_animation(wormhole2_template());
        obj.size = (BOSS_WORMHOLE_D, BOSS_WORMHOLE_D);
        obj.position = (warp_x - BOSS_WORMHOLE_D * 0.5, warp_y - BOSS_WORMHOLE_D * 0.5);
        obj.visible = true;
    }
    c.set_var("warp_flash_ticks", 40i32);
    if let Some(cam) = c.camera_mut() {
        cam.flash_with(
            Color(160, 160, 255, 130),
            0.40,
            FlashMode::Pulse,
            FlashEase::Sharp,
            0.9,
            0.0,
        );
    }
}

/// Enter a stasis orbit around a safe tether node after teleporting into the
/// boss arena. The battle stays inactive while the player orbits so they can
/// get their bearings; tethering to a node (grabbing it) ends the stasis and
/// starts the fight (see `tick_boss_stasis`).
fn enter_boss_stasis(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (warp_x, warp_y) = (BOSS_ARENA_CENTER_X, 200.0);
    let mut s = st.lock().unwrap();
    s.boss_stasis_active = true;
    s.boss_stasis_ticks = 0;
    s.boss_stasis_hook = String::new();

    // Pick the tether node nearest the warp point as the "safe node".
    let mut best: Option<(f32, f32, f32, String)> = None;
    for id in &s.live_hooks {
        if let Some(obj) = c.get_game_object(id) {
            let hcx = obj.position.0 + obj.size.0 * 0.5;
            let hcy = obj.position.1 + obj.size.1 * 0.5;
            let d2 = (hcx - warp_x).powi(2) + (hcy - warp_y).powi(2);
            if best.as_ref().map(|(bd, _, _, _)| d2 < *bd).unwrap_or(true) {
                best = Some((d2, hcx, hcy, id.clone()));
            }
        }
    }
    let (_, hx, hy, hook_id) = best.unwrap_or((0.0, warp_x, warp_y, String::new()));
    s.boss_stasis_hook = hook_id;

    // Position the player at the top of the orbit around the safe node.
    s.hook_x = hx;
    s.hook_y = hy;
    s.px = hx;
    s.py = hy - RESPAWN_ORBIT_R;
    s.vx = 0.0;
    s.vy = 0.0;
    s.hooked = false;
    s.active_hook = String::new();
    s.rope_len = RESPAWN_ORBIT_R;
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (hx - PLAYER_R, (hy - RESPAWN_ORBIT_R) - PLAYER_R);
        obj.momentum = (0.0, 0.0);
        obj.visible = true;
    }

    // Show a prompt telling the player to tether to begin.
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
        let scale = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
            obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                "TETHER TO A NODE TO BEGIN",
                &font,
                52.0 * scale,
                Color(200, 235, 255, 255),
                1300.0 * scale,
            )));
            obj.visible = true;
        }
    }
}

/// Drive the boss stasis orbit each tick. While the player is in stasis, keep
/// them circling the safe node so they can survey the arena; the moment they
/// tether (grab a node) the stasis ends and the battle starts. A debug var
/// (`debug_boss_stasis_down`) bypasses the tether for headless validation.
fn tick_boss_stasis(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_stasis_active { return; }

    // The player tethered to a node (or a debug bypass): end stasis, start.
    let force_down = matches!(c.get_var("debug_boss_stasis_down"), Some(Value::Bool(true)));
    if s.hooked || force_down {
        let victory = s.boss_hp <= 0;
        s.boss_stasis_active = false;
        s.boss_stasis_hook.clear();
        if !victory {
            // Make the boss spawn on the next appearance tick.
            s.boss_entry_ticks = BOSS_ENTRY_DELAY_TICKS;
        }
        drop(s);
        if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
            obj.visible = false;
        }
        if victory {
            complete_boss_finish(c, st);
        }
        return;
    }

    // Otherwise hold the player in a slow counter-clockwise orbit around the
    // safe node so they can look around before deciding to tether.
    let ticks = s.boss_stasis_ticks;
    s.boss_stasis_ticks = ticks + 1;
    let (hx, hy) = (s.hook_x, s.hook_y);
    const ORBIT_OMEGA: f32 = 0.038;
    let theta = -std::f32::consts::FRAC_PI_2 - ORBIT_OMEGA * ticks as f32;
    let px = hx + RESPAWN_ORBIT_R * theta.cos();
    let py = hy + RESPAWN_ORBIT_R * theta.sin();
    s.px = px;
    s.py = py;
    s.vx = 0.0;
    s.vy = 0.0;
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (px - PLAYER_R, py - PLAYER_R);
        obj.momentum = (0.0, 0.0);
    }
}

/// After a fall in the boss arena, reset the player into the stasis orbit while
/// preserving boss progress (HP, generators, final phase). The boss is unspawned
/// so it stops attacking during the orbit; it re-appears once the player tethers
/// to resume the fight.
pub fn reset_boss_after_fall(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let mut s = st.lock().unwrap();
        s.boss_spawned = false;
        s.boss_stasis_active = true;
        // Clear any active darkness so the stasis orbit is visible.
        s.boss_dark_active = false;
        s.boss_dark_ticks = 0;
    }
    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.visible = false;
        obj.position = (-6000.0, -6000.0);
        obj.momentum = (0.0, 0.0);
    }
    if c.has_lighting() {
        c.set_ambient(Color(255, 255, 255, 255), 1.0);
    }
    // Re-enter the stasis orbit (positions/orbits the player, shows the prompt).
    enter_boss_stasis(c, st);
}

/// Keep the boss, its bolts, and the arena tether nodes lit so they remain
/// faintly visible (and add ambience) during the darkness phase. Lights are
/// attached to their objects so they follow automatically; bolts are only lit
/// while active, and the boss light only while it is spawned.
fn tick_boss_lights(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    if !c.has_lighting() { return; }
    let (active, spawned) = {
        let s = st.lock().unwrap();
        (s.boss_active, s.boss_spawned)
    };
    if !active {
        return;
    }

    // Boss: faint purple light (no shadow cast — small ambient fill).
    if c.get_light("boss_light").is_none() {
        let mut ls = LightSource::new("boss_light", (0.0, 0.0), Color(170, 90, 230, 255), 1500.0, 0.32);
        ls.casts_shadows = false;
        c.add_light(ls);
        c.attach_light("boss_light", "boss", (0.0, 0.0));
    }
    c.set_light_enabled("boss_light", spawned);

    // Bolts: faint orange light, lit only while the bolt is visible.
    for i in 0..BOSS_BOLT_POOL_SIZE {
        let lid = format!("boss_bolt_light_{i}");
        if c.get_light(&lid).is_none() {
            let mut ls = LightSource::new(lid.clone(), (0.0, 0.0), Color(255, 120, 30, 255), 620.0, 0.5);
            ls.casts_shadows = false;
            c.add_light(ls);
            c.attach_light(&lid, &format!("boss_bolt_{i}"), (0.0, 0.0));
        }
        let vis = c.get_game_object(&format!("boss_bolt_{i}")).map(|o| o.visible).unwrap_or(false);
        c.set_light_enabled(&lid, vis);
    }

    // Arena tether nodes: faint cyan light so the player can navigate in the dark.
    let hooks: Vec<String> = {
        let s = st.lock().unwrap();
        s.live_hooks.clone()
    };
    for id in &hooks {
        let lid = format!("boss_node_light_{id}");
        if c.get_light(&lid).is_none() {
            let mut ls = LightSource::new(lid.clone(), (0.0, 0.0), Color(110, 230, 255, 255), 520.0, 0.45);
            ls.casts_shadows = false;
            c.add_light(ls);
            c.attach_light(&lid, id, (0.0, 0.0));
        }
        c.set_light_enabled(&lid, true);
    }
}

/// Count down and hide the boss-arena wormhole warp overlay. The cannon
/// fast-travel warp manages its own two-phase overlay via `cannon_warp_phase`,
/// so skip it here.
fn tick_warp_flash(c: &mut Canvas, _st: &Arc<Mutex<State>>) {
    if matches!(c.get_var("cannon_warp_phase"), Some(Value::I32(v)) if v > 0) {
        return;
    }
    let t = match c.get_var("warp_flash_ticks") {
        Some(Value::I32(v)) => v,
        _ => 0,
    };
    if t <= 0 {
        return;
    }
    // Keep the warp origin centred on the player so the speed lines converge on
    // the ball, not the middle of the screen.
    center_warp_on_player(c);
    c.set_var("warp_flash_ticks", t - 1);
    if t - 1 <= 0 {
        if let Some(obj) = c.get_game_object_mut("warp_flash") {
            obj.visible = false;
        }
    }
}

// ── Last-boss: barrier + generators + bait-and-bail ──────────────────────────

/// Position the barrier and generator nodes across the arena.
fn spawn_generators_and_barrier(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
fn tick_boss_forcefield(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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

fn tick_generators(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
fn tick_barrier(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
/// telegraphed bait). If the player dodges, the lunge carries the boss past the
/// open sun line and it falls in.
fn tick_desperation(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
        // The lunge carried the boss past the sun line → it falls in.
        if ny < BOSS_SUN_KILL_Y {
            finish_boss(c, st);
            return;
        }
        if done {
            let mut s = st.lock().unwrap();
            s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH;
        }
        return;
    }
    s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH.max(s.boss_lunge_telegraph);
}

/// Boss defeated: award a chunk of meta currency, hide the boss/generators, and
/// drop the player back into a stasis orbit with a congratulations message. The
/// actual end-of-fight cleanup (rewind frontiers, `boss_active=false`) happens
/// once the player tethers out of the victory stasis (`complete_boss_finish`).
fn finish_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let generator_ids;
    {
        let mut s = st.lock().unwrap();
        s.boss_hp = 0;
        s.boss_spawned = false;
        s.boss_stasis_active = true;
        s.boss_stasis_hook.clear();
        s.boss_entry_ticks = 0;
        s.boss_barrier_up = false;
        s.boss_final_phase = false;
        s.boss_lunge_ticks = 0;
        s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH;
        s.boss_killed = true;
        generator_ids = s.boss_generators.clone();
    }

    // Award meta currency for the permanent-roguelike upgrade pool.
    crate::profile::award_meta_currency(META_BOSS_REWARD);

    for id in generator_ids {
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
        }
    }
    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.visible = false;
        obj.position = (-6000.0, -6000.0);
        obj.momentum = (0.0, 0.0);
    }
    if let Some(obj) = c.get_game_object_mut("boss_barrier") {
        obj.visible = false;
    }
    if c.has_lighting() {
        c.set_ambient(Color(255, 255, 255, 255), 1.0);
    }

    // Drop the player into the victory stasis orbit, then set the congrats text.
    enter_boss_stasis(c, st);
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
        let scale = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
            obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                &format!("CONGRATULATIONS! You defeated {BOSS_NAME}!  +{META_BOSS_REWARD} META"),
                &font,
                46.0 * scale,
                Color(255, 230, 140, 255),
                1400.0 * scale,
            )));
            obj.visible = true;
        }
    }
}

/// Complete the end-of-fight cleanup once the player tethers out of the victory
/// stasis: rewind spawn frontiers and hand control back to normal play.
fn complete_boss_finish(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Put the player back where the level was left. Without this they stay at
    // the arena's own stretch of X — fine when a run held exactly one fight and
    // ended there, fatal now that a run holds several.
    let (rx, ry) = {
        let s = st.lock().unwrap();
        (s.boss_return_x, s.boss_return_y)
    };
    {
        let mut s = st.lock().unwrap();
        s.px = rx;
        s.py = ry;
        s.vx = 0.0;
        s.vy = 0.0;
        s.hooked = false;
        s.active_hook.clear();
        s.hook_x = rx;
        s.hook_y = ry;
    }
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (rx - PLAYER_R, ry - PLAYER_R);
        obj.momentum = (0.0, 0.0);
    }
    if let Some(cam) = c.camera_mut() {
        cam.position = (rx - VW * 0.5, ry - VH * 0.5);
        cam.snap_zoom(1.0);
    }

    {
        let mut s = st.lock().unwrap();
        s.boss_active = false;
        s.boss_spawned = false;
        s.boss_cleared = false;
        let backfill_x = s.px - GEN_AHEAD * 0.35;
        s.rightmost_x = s.rightmost_x.min(backfill_x);
        s.pad_rightmost = s.pad_rightmost.min(backfill_x);
        s.spinner_rightmost = s.spinner_rightmost.min(backfill_x);
        s.coin_rightmost = s.coin_rightmost.min(backfill_x);
        s.flip_rightmost = s.flip_rightmost.min(backfill_x);
        s.score_x2_rightmost = s.score_x2_rightmost.min(backfill_x);
        s.zero_g_rightmost = s.zero_g_rightmost.min(backfill_x);
        s.gate_rightmost = s.gate_rightmost.min(backfill_x);
        s.gwell_rightmost = s.gwell_rightmost.min(backfill_x);
        s.turret_rightmost = s.turret_rightmost.min(backfill_x);
        s.rocket_pad_rightmost = s.rocket_pad_rightmost.min(backfill_x);
        if s.pending.is_empty() {
            let mut seed = s.seed;
            let mut gen_head_x = s.gen_head_x.min(backfill_x);
            let mut gen_head_y = s.gen_head_y;
            let batch = gen_hook_batch(&mut seed, backfill_x, &mut gen_head_x, &mut gen_head_y, s.distance);
            s.seed = seed;
            s.gen_head_x = gen_head_x;
            s.gen_head_y = gen_head_y;
            s.pending.extend(batch);
        }
    }
    if let Some(obj) = c.get_game_object_mut("boss_hp_bar") {
        obj.visible = false;
    }
    if c.has_lighting() {
        c.set_light_enabled("boss_light", false);
        for i in 0..BOSS_BOLT_POOL_SIZE {
            c.set_light_enabled(&format!("boss_bolt_light_{i}"), false);
        }
    }

    // Advance the schedule. A run holds several fights now, so clearing one
    // arms the next rather than ending the boss flow for good — the schedule
    // running out is what ends it (`boss_trigger_distance` returns None).
    let mode = crate::mode::current_mode(c);
    let (index, was_final) = {
        let mut s = st.lock().unwrap();
        let was_final = crate::mode::is_final_boss(mode, s.boss_index);
        s.boss_index = s.boss_index.saturating_add(1);
        (s.boss_index, was_final)
    };
    c.set_var("boss_index", Value::I32(index as i32));
    c.set_var("boss_mode_cleared", was_final);
    c.set_var("bosses_defeated", Value::I32(index as i32));
}

// ── Boss appearance after delay ───────────────────────────────────────────────

fn tick_boss_appearance(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || s.boss_spawned || s.boss_stasis_active { return; }

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
    let (zx1, zx2) = arena_bounds(c);
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

    // Map to arena bounds: X = [-1,1] → [20000, 34000], Y = [-1,1] → [-6000, 2400]
    let tx_base = BOSS_ARENA_CENTER_X + x_liss * BOSS_ARENA_HALF_W * 0.95;
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

// ── Drift boss arena asteroids ────────────────────────────────────────────────

fn tick_boss_asteroid_drift(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
    let _over = super::hearts::lose_heart(c, st);
    // Restore light boss gravity after the hit.
    if let Some(obj) = c.get_game_object_mut("player") {
        let gdir = st.lock().unwrap().gravity_dir;
        obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE * gdir;
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

    let (nwx, nwy, did_unhook) = if buffed && near_weakpoint && generators_down {
        // Buffed weakpoint hit with the forcefield down: damage the boss.
        s.boss_hp -= 1;
        s.buff_hit_flash = 20;
        (nx * 26.0, ny * 26.0, false)
    } else {
        // Forcefield still up (or unprotected contact): knockback, no damage.
        let unhook = s.hooked;
        if unhook {
            s.hooked = false;
            s.active_hook = String::new();
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
        s2.boss_dark_active = false;
        s2.boss_dark_ticks = 0;
        s2.boss_dark_cooldown = BOSS_DARK_INTERVAL;

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
    c.set_var("boss_hp", Value::I32(hp));
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
        obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (BOSS_HP_BAR_W, BOSS_HP_BAR_H), 0.0), image: img.into(), color: None });
        obj.visible = fill > 0.0;
    }
}

// ── Off-screen objective indicators ──────────────────────────────────────────

/// Place a HUD arrow at the screen edge pointing at a world position. If the
/// world position is on screen the arrow is hidden. `cleft`/`ctop`/`zoom` are
/// the camera's world-space top-left and zoom.
fn place_off_arrow(c: &mut Canvas, id: &str, wx: f32, wy: f32, cleft: f32, ctop: f32, zoom: f32) {
    let sx = (wx - cleft) * zoom;
    let sy = (wy - ctop) * zoom;
    let on_screen = sx >= 0.0 && sx <= VW && sy >= 0.0 && sy <= VH;
    let Some(obj) = c.get_game_object_mut(id) else { return; };
    if on_screen {
        obj.visible = false;
        return;
    }
    // Aim from screen centre toward the target.
    let dx = sx - VW * 0.5;
    let dy = sy - VH * 0.5;
    obj.rotation = dy.atan2(dx).to_degrees();
    // Clamp to the viewport rim so the arrow sits at the edge.
    let margin = 90.0;
    obj.position = (sx.clamp(margin, VW - margin), sy.clamp(margin, VH - margin));
    obj.visible = true;
}

/// Off-screen indicators during the fight: the boss-name banner, an edge arrow
/// pointing at the boss, and (only while the player is buffed) edge arrows
/// pointing at any off-screen generators that still have HP.
fn tick_boss_indicators(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (active, spawned, buff, gens_down) = {
        let s = st.lock().unwrap();
        let gens_down = !s.boss_generator_hp.is_empty() && s.boss_generator_hp.iter().all(|&hp| hp <= 0);
        (s.boss_active, s.boss_spawned, s.player_buff, gens_down)
    };

    // Hide everything when the fight is not running.
    if !active || !spawned {
        if let Some(obj) = c.get_game_object_mut("boss_name_text") { obj.visible = false; }
        if let Some(obj) = c.get_game_object_mut("boss_off_arrow") { obj.visible = false; }
        for i in 0..BOSS_GENERATOR_COUNT {
            if let Some(obj) = c.get_game_object_mut(&format!("gen_arrow_{i}")) { obj.visible = false; }
        }
        return;
    }

    let (cleft, ctop, zoom) = c.camera()
        .map(|cam| (cam.position.0, cam.position.1, cam.zoom.max(0.1)))
        .unwrap_or((0.0, 0.0, 1.0));

    // Boss-name banner: visible for the length of the fight.
    if let Some(obj) = c.get_game_object_mut("boss_name_text") { obj.visible = true; }

    // Arrow pointing at the boss when it is off-screen.
    if let Some(boss) = c.get_game_object("boss") {
        let bx = boss.position.0 + BOSS_SIZE * 0.5;
        let by = boss.position.1 + BOSS_SIZE * 0.5;
        place_off_arrow(c, "boss_off_arrow", bx, by, cleft, ctop, zoom);
    } else {
        if let Some(obj) = c.get_game_object_mut("boss_off_arrow") { obj.visible = false; }
    }

    // Generator objective arrows, only while the player is buffed and any
    // generator is still alive to destroy.
    let show_gens = buff > 0 && !gens_down;
    for i in 0..BOSS_GENERATOR_COUNT {
        let id = format!("gen_arrow_{i}");
        if show_gens {
            if let Some(gen) = c.get_game_object(&format!("boss_gen_{i}")) {
                let gw = BOSS_GENERATOR_R * 2.0;
                place_off_arrow(c, &id, gen.position.0 + gw * 0.5, gen.position.1 + gw * 0.5, cleft, ctop, zoom);
            } else if let Some(obj) = c.get_game_object_mut(&id) {
                obj.visible = false;
            }
        } else if let Some(obj) = c.get_game_object_mut(&id) {
            obj.visible = false;
        }
    }
}
