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

/// Reveal the arena's boundary walls (left/right) so the player can see the
/// play-area limits of the fight. No floor — you can fall to your doom below.
/// The walls extend from high above the highest possible launch up to the bottom
/// of the visible play area, so it never looks like you can hop over them.
pub(crate) fn tick_arena_walls(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let active = { let s = st.lock().unwrap(); s.boss_active && !s.dead };
    let (x1, x2) = arena_bounds(c);
    let top = -9000.0;
    let bottom = 600.0;
    let wall_h = bottom - top;
    let cy = (top + bottom) * 0.5;
    if active {
        let half = ARENA_WALL_THICKNESS * 0.5;
        if let Some(obj) = c.get_game_object_mut("arena_wall_l") {
            obj.size = (ARENA_WALL_THICKNESS, wall_h);
            obj.position = (x1 - half, cy - wall_h * 0.5);
            obj.visible = true;
        }
        if let Some(obj) = c.get_game_object_mut("arena_wall_r") {
            obj.size = (ARENA_WALL_THICKNESS, wall_h);
            obj.position = (x2 - half, cy - wall_h * 0.5);
            obj.visible = true;
        }
        bounce_player_off_walls(c, st, x1, x2);
    } else {
        for name in ["arena_wall_l", "arena_wall_r"] {
            if let Some(obj) = c.get_game_object_mut(name) {
                obj.visible = false;
            }
        }
    }
}

/// Push the player back off an arena wall, and make sure they LEAVE it.
///
/// Called from `tick_arena_walls`, which runs after `tick_rope_constraint` and
/// before `cap_momentum_and_write_back` — the only window in the tick where
/// both the position and the velocity can be corrected and survive to the
/// engine. Earlier, the rope solve would overwrite the position; later, the
/// write-back has already happened.
///
/// The two states need different answers, because the rope OWNS the player's
/// position while they are hooked:
///
///  * Free: reflect the horizontal velocity and place them clear of the face.
///  * Hooked: `tick_rope_constraint` re-projects `px` onto the arc every frame,
///    so a position correction is erased before it is ever drawn. What can be
///    changed is the direction of travel ALONG the arc, so the swing reverses
///    and carries them away from the wall under its own momentum. That is why
///    this reflects the TANGENT and not `vx` — reflecting `vx` on a hooked
///    player is undone by the next frame's projection, which is exactly the
///    "stuck swinging against the wall".
///
/// A minimum separation speed matters more than the restitution: a player
/// arriving almost parallel to the wall reflects to almost nothing and grinds
/// along it, which is the sticking this exists to prevent.
/// What a wall does to the player this frame.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum WallBounce {
    /// Clear of both walls.
    None,
    /// Free flight: move to `px` and leave with `vx`.
    Free { px: f32, vx: f32 },
    /// On the rope: keep the position (the arc owns it) and swing back.
    Swing { vx: f32, vy: f32 },
}

/// The wall response, as a pure function of the player's state.
///
/// Split out from the tick so it can be tested against the arithmetic that
/// actually runs, rather than against a description of it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn arena_wall_bounce(
    px: f32, py: f32, vx: f32, vy: f32,
    hooked: bool, hook: (f32, f32),
    x1: f32, x2: f32,
) -> WallBounce {
    // The wall objects are CENTRED on the bound, so the inner face is half a
    // thickness inside it.
    let inner_l = x1 + ARENA_WALL_THICKNESS * 0.5 + PLAYER_R;
    let inner_r = x2 - ARENA_WALL_THICKNESS * 0.5 - PLAYER_R;
    if inner_r <= inner_l { return WallBounce::None; }

    // `dir` is the direction OUT of the wall: +1 off the left wall, -1 off the
    // right one.
    let (dir, face) = if px < inner_l {
        (1.0_f32, inner_l)
    } else if px > inner_r {
        (-1.0_f32, inner_r)
    } else {
        return WallBounce::None;
    };

    if hooked {
        // On the arc: reverse the swing if it is still heading into the wall.
        let dx = px - hook.0;
        let dy = py - hook.1;
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let (tx, ty) = (-dy / d, dx / d);
        let tangent = vx * tx + vy * ty;
        // Which way along the arc moves the player out of the wall.
        let out = if tx * dir >= 0.0 { 1.0 } else { -1.0 };
        // Already swinging out: leave the swing alone rather than pumping it.
        if tangent * out > 0.0 { return WallBounce::None; }
        let speed = (tangent.abs() * ARENA_WALL_RESTITUTION).max(ARENA_WALL_MIN_BOUNCE) * out;
        return WallBounce::Swing { vx: tx * speed, vy: ty * speed };
    }

    // Free: put them on the clear side of the face and send them away from it.
    let away = (vx.abs() * ARENA_WALL_RESTITUTION).max(ARENA_WALL_MIN_BOUNCE) * dir;
    WallBounce::Free { px: face, vx: away }
}

fn bounce_player_off_walls(c: &mut Canvas, st: &Arc<Mutex<State>>, x1: f32, x2: f32) {
    let bounce = {
        let s = st.lock().unwrap();
        if s.dead { return; }
        arena_wall_bounce(
            s.px, s.py, s.vx, s.vy, s.hooked, (s.hook_x, s.hook_y), x1, x2,
        )
    };
    match bounce {
        WallBounce::None => {}
        WallBounce::Swing { vx, vy } => {
            let mut s = st.lock().unwrap();
            s.vx = vx;
            s.vy = vy;
        }
        WallBounce::Free { px, vx } => {
            let (px, py, vx, vy) = {
                let mut s = st.lock().unwrap();
                s.px = px;
                s.vx = vx;
                (s.px, s.py, s.vx, s.vy)
            };
            // The engine integrates `momentum` into `position` after the tick,
            // so a stale inward momentum would walk the player straight back
            // into the wall on the very frame they were pushed out of it.
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.position = (px - PLAYER_R, py - PLAYER_R);
                obj.momentum = (vx, vy);
            }
        }
    }
}

/// Mark a few of the arena tether nodes as shielded so they are shelter during
/// the Flare Titan's flares (timed-release, like the world's flare system).
pub(crate) fn ensure_arena_shelter_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let ids: Vec<String> = st.lock().unwrap().live_hooks.clone();
    for (i, id) in ids.iter().enumerate() {
        if i % 4 == 0 {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.tags.retain(|t| t != SHIELD_HOOK_TAG);
                obj.tags.push(SHIELD_HOOK_TAG.into());
                obj.set_glow(GlowConfig { color: Color(255, 200, 80, 255), width: 14.0 });
            }
        }
    }
}

/// Push a small electricity effect over each buff tether node so they read as
/// distinct from the regular grab nodes (the same round electricity the player
/// gets while buffed).
/// The electric aura on a buff node.
///
/// ATTACHED to each node rather than pushed as a screen overlay. A pushed mega
/// sprite is drawn in a pass that runs after every other renderer, so it sat in
/// front of the arena asteroids even when an asteroid was between the camera
/// and the node — the aura read as being in a different plane from the node it
/// belongs to. Attaching it makes the sprite part of the node's own draw, so it
/// takes the node's layer and depth and is occluded by exactly what occludes
/// the node.
///
/// The player's own buff dome stays a pushed overlay (see `solar.rs`): that one
/// genuinely must never be hidden by scenery.
///
/// Attached sprites persist until cleared, so every id touched last frame that
/// is no longer a buff node has to be cleared explicitly — a recycled pool slot
/// would otherwise keep wearing an aura for something it is no longer.
pub(crate) fn tick_buff_node_elec(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (candidates, previous) = {
        let s = st.lock().unwrap();
        (s.live_hooks.clone(), s.buff_fx_attached.clone())
    };

    let mut attached: Vec<String> = Vec::new();
    for id in &candidates {
        let Some(obj) = c.get_game_object(id) else { continue; };
        if !obj.visible || !obj.tags.iter().any(|t| t == BUFF_HOOK_TAG) { continue; }
        crate::scenes::game::fx::attach_electric_fx(
            c, id,
            (HOOK_R * BUFF_NODE_FX_SCALE, HOOK_R * BUFF_NODE_FX_SCALE),
            BUFF_NODE_FX_TINT,
        );
        attached.push(id.clone());
    }

    for id in &previous {
        if !attached.contains(id) {
            crate::scenes::game::fx::clear_object_fx(c, id);
        }
    }

    st.lock().unwrap().buff_fx_attached = attached;
}

/// Centre of the arena for the fight currently being set up.
///
/// `BOSS_ARENA_CENTER_X` is derived from the ORIGINAL fixed arena constants and
/// is now wrong for every fight: when arenas became relocatable the walls moved
/// and this did not, so the warp destination, the boss spawn and the boss's
/// movement centre all stayed at the old 27 000 while the walls sat millions of
/// pixels away. Use this instead.
pub(crate) fn arena_center_x(c: &Canvas) -> f32 {
    let (x1, x2) = arena_bounds(c);
    (x1 + x2) * 0.5
}

/// Left and right walls of the arena for the fight currently being set up.
pub(crate) fn arena_bounds(c: &Canvas) -> (f32, f32) {
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
pub(crate) fn place_arena(c: &mut Canvas, index: u32) {
    let stride = (BOSS_ZONE_X2 - BOSS_ZONE_X1) + BOSS_ARENA_GAP;
    let x1 = crate::mode::BOSS_ARENA_ORIGIN_X + stride * index as f32;
    let x2 = x1 + (BOSS_ZONE_X2 - BOSS_ZONE_X1);
    c.set_var("boss_arena_x1", Value::F32(x1));
    c.set_var("boss_arena_x2", Value::F32(x2));
}

pub(crate) fn tick_boss_zone_entry(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
        spawn_boss_approach_nodes(c, st, threshold);
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
        s.boss_approach_nodes_spawned = false; // re-arm for the next fight
        drop(s);
        c.set_var("boss_defeated_this_fight", false);
        let mut s = st.lock().unwrap();
        let index = s.boss_index;
        drop(s);
        place_arena(c, index);
        let mut s = st.lock().unwrap();
        s.boss_active = true;
        // A debug override lets the headless harness validate the existing
        // Sun Devourer fight regardless of which roster slot is being warped to
        // (once the new bosses land, index 0 is the Colossus).
        s.boss_kind = if matches!(c.get_var("debug_boss_kind_sundev"), Some(Value::Bool(true))) {
            crate::constants::BossKind::SunDevourer
        } else {
            crate::constants::boss_kind_for_index(index)
        };
        s.boss_parts = crate::constants::boss_parts_for_kind(s.boss_kind);
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
        // Barrier + generators are the Sun Devourer's finale set-dressing only;
        // no other roster boss gets them.
        if st.lock().unwrap().boss_kind == crate::constants::BossKind::SunDevourer {
            spawn_generators_and_barrier(c, st);
        }
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

pub(crate) fn place_boss_asteroids(c: &mut Canvas, asteroid_ids: &[String]) {
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
pub(crate) fn activate_arena_tether_node(c: &mut Canvas, s: &mut State, id: String, hx: f32, hy: f32, is_buff: bool) {
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
pub(crate) fn spawn_arena_tether_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
pub(crate) fn spawn_boss_approach_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>, threshold: f32) {
    let mut s = st.lock().unwrap();
    if s.boss_approach_nodes_spawned {
        return;
    }
    s.boss_approach_nodes_spawned = true;
    drop(s);

    // Reveal the huge black-hole threshold marker so the player knows they are
    // heading into something special — and MOVE IT to the fight that is
    // actually coming.
    //
    // Both objects were built once at `BOSS_THRESHOLD_X` (20 000), which was
    // the trigger back when a run held exactly one fight. The trigger is now
    // scheduled, so by the time this revealed them they were hundreds of
    // thousands of pixels behind the player and the teleport arrived with no
    // warning at all.
    let d = BOSS_MARKER_D;
    if let Some(obj) = c.get_game_object_mut("boss_threshold_marker") {
        obj.position = (threshold - d * 0.5, BOSS_MARKER_Y - d * 0.5);
        obj.visible = true;
    }
    let asz = BOSS_MARKER_ARROW_D;
    if let Some(obj) = c.get_game_object_mut("boss_marker_arrow") {
        obj.position = (threshold - asz * 0.5, (BOSS_MARKER_Y - 900.0) - asz * 0.5);
        obj.visible = true;
    }
}

/// Warp the player into the boss arena (wormhole-style flash) at the bottom of
/// the tether grid, so the fight is entered cleanly rather than the player
/// having to climb from the normal zone.
pub(crate) fn warp_player_into_arena(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let warp_x = arena_center_x(c);
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
pub(crate) fn enter_boss_stasis(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (warp_x, warp_y) = (arena_center_x(c), 200.0);
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
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../../assets/font.ttf")) {
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
pub(crate) fn tick_boss_stasis(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
pub(crate) fn tick_boss_lights(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
pub(crate) fn tick_warp_flash(c: &mut Canvas, _st: &Arc<Mutex<State>>) {
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

/// Boss defeated: award a chunk of meta currency, hide the boss/generators, and
/// drop the player back into a stasis orbit with a congratulations message. The
/// actual end-of-fight cleanup (rewind frontiers, `boss_active=false`) happens
/// once the player tethers out of the victory stasis (`complete_boss_finish`).
pub(crate) fn finish_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
        // A darkness attack in flight when the boss dies would otherwise leave
        // the night-mode post pass, the lamp and the dark ambient running for
        // the rest of the run — the post override is a single global slot and
        // nothing else would ever clear it.
        s.boss_dark_active = false;
        s.boss_dark_ticks = 0;
        s.boss_dark_cooldown = BOSS_DARK_INTERVAL;
        generator_ids = s.boss_generators.clone();
    }
    c.set_var("boss_darkness", false);
    crate::scenes::game::eclipse::end_night_mode(c);
    crate::scenes::game::eclipse::kill_node_lights(c);
    if c.has_lighting() {
        c.set_ambient(Color(255, 255, 255, 255), 1.0);
    }

    // Award meta currency for the permanent-roguelike upgrade pool, and coins
    // (on-hand) so a boss kill funds an in-run upgrade in the next link section.
    crate::profile::award_meta_currency(META_BOSS_REWARD);
    crate::profile::record_boss_defeated();
    {
        let mut s = st.lock().unwrap();
        s.coin_count = s.coin_count.saturating_add(BOSS_COIN_REWARD);
    }
    // Direct defeat signal. `boss_mode_cleared` now means "no fights left this
    // run", which is only true of the last one, so it is the wrong thing for a
    // per-fight check to read.
    c.set_var("boss_defeated_this_fight", true);

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
    let boss_name = st.lock().unwrap().boss_kind.name();
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../../assets/font.ttf")) {
        let scale = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
            obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                &format!("CONGRATULATIONS! You defeated {boss_name}!  +{META_BOSS_REWARD} META"),
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
pub(crate) fn complete_boss_finish(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
        s.upgrade_rightmost = s.upgrade_rightmost.min(backfill_x);
        // ALWAYS rebuild the pending hook queue from the backfilled position.
        // Leftover pre-arena hooks (when `pending` wasn't empty) sit at the
        // arena's stretch of X, which left a barren link section after the boss
        // that the player couldn't swing through until they died once.
        s.pending.clear();
        let mut seed = s.seed;
        let mut gen_head_x = s.gen_head_x.min(backfill_x);
        let mut gen_head_y = s.gen_head_y;
        let batch = gen_hook_batch(&mut seed, backfill_x, &mut gen_head_x, &mut gen_head_y, s.distance);
        s.seed = seed;
        s.gen_head_x = gen_head_x;
        s.gen_head_y = gen_head_y;
        s.pending.extend(batch);
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

pub(crate) fn tick_boss_hud(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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

/// Place a HUD arrow at the screen edge pointing at a world position. If the
/// world position is on screen the arrow is hidden. `cleft`/`ctop`/`zoom` are
/// the camera's world-space top-left and zoom.
pub(crate) fn place_off_arrow(c: &mut Canvas, id: &str, wx: f32, wy: f32, cleft: f32, ctop: f32, zoom: f32) {
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
pub(crate) fn tick_boss_indicators(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
