// ── scenes/game/hearts.rs — run lives + auto-progress checkpoint respawn ──────
// Falling off-screen normally ends the run. With the hearts system it costs a
// heart (if any remain) and respawns the player at the last auto-progress saved
// grab-node, re-entering via the same orbit-in used at game start.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use crate::images::*;
use super::helpers::*;

/// Per-frame hearts bookkeeping: save checkpoints on block boundaries and keep
/// the HUD + `hearts`/`heart_losses` vars current (the headless harness reads
/// these vars).
pub fn tick_hearts(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    checkpoint_save(c, st);
    hearts_hud_update(c, st);
    tick_buff(c, st);
    tick_solar_flare(c, st);
}

/// Solar flare hazard: telegraphed, then erupts. If the player isn't within
/// `FLARE_SHIELD_RADIUS` of a shielded node on the eruption frame, they lose a
/// heart. Repeated flares run on a cooldown.
pub fn tick_solar_flare(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (in_space, boss, god) = {
        let s = st.lock().unwrap();
        (s.in_space_mode, s.boss_active, s.god_mode)
    };
    if in_space || boss || god {
        c.set_var("flare_warning", false);
        c.set_var("flare_active", false);
        return;
    }

    let mut s = st.lock().unwrap();
    if s.flare_warn > 0 {
        s.flare_warn = s.flare_warn.saturating_sub(1);
        if s.flare_warn == 0 {
            // Eruption frame: cost a heart if not near a shielded node.
            let px = s.px;
            let py = s.py;
            let shield_hooks: Vec<String> = s.live_hooks.clone();
            s.flare_active = true;
            s.flare_active_ticks = FLARE_ACTIVE_TICKS;
            drop(s);

            let protected = shield_hooks.iter().any(|id| {
                c.get_game_object(id)
                    .map(|o| {
                        let hx = o.position.0 + o.size.0 * 0.5;
                        let hy = o.position.1 + o.size.1 * 0.5;
                        (hx - px) * (hx - px) + (hy - py) * (hy - py)
                            < FLARE_SHIELD_RADIUS * FLARE_SHIELD_RADIUS
                    })
                    .unwrap_or(false)
            });
            if !protected {
                lose_heart(c, st);
            }
            c.set_var("flare_active", true);
            c.set_var("flare_warning", false);
            return;
        }
        let warn = s.flare_warn > 0;
        drop(s);
        c.set_var("flare_warning", warn);
        return;
    }

    if s.flare_active {
        s.flare_active_ticks = s.flare_active_ticks.saturating_sub(1);
        if s.flare_active_ticks == 0 {
            s.flare_active = false;
            s.flare_cooldown = FLARE_INTERVAL;
        }
        let active = s.flare_active;
        drop(s);
        c.set_var("flare_active", active);
        c.set_var("flare_warning", false);
        return;
    }

    s.flare_cooldown = s.flare_cooldown.saturating_sub(1);
    if s.flare_cooldown == 0 {
        s.flare_warn = FLARE_WARN_TICKS;
        s.flare_cooldown = FLARE_INTERVAL;
    }
    let warn = s.flare_warn > 0;
    drop(s);
    c.set_var("flare_warning", warn);
    c.set_var("flare_active", false);
}

/// Tick down an active buff and clear it (and the player glow) when it expires.
fn tick_buff(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Debug/test: a var can force a buff so the weakpoint check can be validated.
    // Safe read (get_bool panics if unset).
    let force = matches!(c.get_var("debug_force_buff"), Some(Value::Bool(true)));
    if force {
        {
            let mut s = st.lock().unwrap();
            s.player_buff = 1;
            s.buff_timer = 600;
        }
        c.set_var("debug_force_buff", false);
        if let Some(p) = c.get_game_object_mut("player") {
            p.set_glow(GlowConfig { color: Color(110, 230, 255, 255), width: 22.0 });
        }
    }
    let mut s = st.lock().unwrap();
    c.set_var("player_buff", Value::I32(s.player_buff as i32));
    c.set_var("buff_timer", Value::I32(s.buff_timer as i32));
    if s.buff_timer == 0 {
        return;
    }
    s.buff_timer -= 1;
    if s.buff_timer == 0 {
        s.player_buff = 0;
        drop(s);
        if let Some(p) = c.get_game_object_mut("player") {
            p.clear_glow();
        }
        return;
    }
    if s.buff_hit_flash > 0 {
        s.buff_hit_flash -= 1;
    }
}

/// Save a checkpoint on the nearest live grab-node whenever the player crosses
/// into a new `CHECKPOINT_INTERVAL` block. Skipped in space and boss arenas.
pub fn checkpoint_save(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (block, cb) = {
        let s = st.lock().unwrap();
        if s.in_space_mode || s.boss_active {
            return;
        }
        ((s.px / CHECKPOINT_INTERVAL).floor() as i32, s.checkpoint_block)
    };
    if block <= cb {
        return;
    }

    let (px, py, hooks) = {
        let s = st.lock().unwrap();
        (s.px, s.py, s.live_hooks.clone())
    };
    let mut best: Option<(f32, f32, f32)> = None;
    for id in &hooks {
        if let Some(obj) = c.get_game_object(id) {
            let hx = obj.position.0 + obj.size.0 * 0.5;
            let hy = obj.position.1 + obj.size.1 * 0.5;
            let d = (hx - px) * (hx - px) + (hy - py) * (hy - py);
            if best.map_or(true, |(_, _, bd)| d < bd) {
                best = Some((hx, hy, d));
            }
        }
    }

    let mut s = st.lock().unwrap();
    s.checkpoint_block = block;
    if let Some((hx, hy, _)) = best {
        s.checkpoint_x = hx;
        s.checkpoint_y = hy;
    }
}

/// Remove one heart (and any active buff/power-ups). Returns true if the run is
/// over (no hearts left). Keeps the `hearts`/`heart_losses` vars current.
pub fn lose_heart(c: &mut Canvas, st: &Arc<Mutex<State>>) -> bool {
    let mut s = st.lock().unwrap();
    s.hearts -= 1;
    s.player_buff = 0;
    s.buff_timer = 0;
    s.buff_hit_flash = 0;
    s.zero_g_timer = 0;
    s.score_x2_timer = 0;
    let over = s.hearts <= 0;
    drop(s);

    let losses = match c.get_var("heart_losses") {
        Some(Value::I32(n)) => n.saturating_add(1),
        Some(Value::F64(n)) => (n as i32).saturating_add(1),
        _ => 1,
    };
    c.set_var("heart_losses", Value::I32(losses));
    hearts_hud_update(c, st);
    // Brief red flash so a heart loss is clearly signalled.
    if let Some(cam) = c.camera_mut() {
        cam.flash_with(Color(200, 40, 40, 110), 0.35, FlashMode::Pulse, FlashEase::Sharp, 0.8, 0.0);
    }
    over
}

/// Respawn the player at the last checkpoint via the orbit-in animation. Rewinds
/// the spawn frontiers so the world repopulates from the checkpoint, and drops a
/// temporary checkpoint node there so there is always something to grab.
pub fn respawn(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (cx, cy) = {
        let s = st.lock().unwrap();
        (s.checkpoint_x, s.checkpoint_y)
    };

    {
        let mut s = st.lock().unwrap();
        s.respawn_active = true;
        s.respawn_ticks = 0;
        s.hooked = false;
        s.active_hook = String::new();
        s.vx = 0.0;
        s.vy = 0.0;
        s.rope_len = RESPAWN_ORBIT_R;
        s.gravity_dir = 1.0;
        s.hook_x = cx;
        s.hook_y = cy;
        s.px = cx;
        s.py = cy - RESPAWN_ORBIT_R;
        s.in_space_mode = false;
        s.space_launch_active = false;
        s.space_settle_done = false;
        s.cannon_captured = false;
    }

    clear_world_for_respawn(c, st);
    rewind_frontiers(st, cx);
    place_checkpoint_hook(c, st, cx, cy);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (cx - PLAYER_R, (cy - RESPAWN_ORBIT_R) - PLAYER_R);
        obj.momentum = (0.0, 0.0);
        obj.gravity = 0.0;
        obj.visible = true;
    }
    c.run(Action::Hide { target: Target::name("rope") });
    c.set_var("rope_visible_at_pause", false);
    c.set_var("start_prompt_active", true);
    c.set_var("game_paused", true);
    c.set_var("start_orbit_ticks", 0i32);
    c.set_var("input_needs_edge_reset", true);
    // Center the camera on the respawn node so the standby orbit is framed.
    if let Some(cam) = c.camera_mut() {
        cam.position = (cx - VW * 0.5, cy - VH * 0.5);
        cam.snap_zoom(1.0);
        cam.zoom_anchor = Some((cx, cy));
    }
}

/// Recycle every live world object back to its pool and hide it, so a respawn
/// regenerates cleanly from the checkpoint instead of duplicating content that
/// was already spawned ahead of the fall.
fn clear_world_for_respawn(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let hook_ids = std::mem::take(&mut s.live_hooks);
    let pad_ids = std::mem::take(&mut s.pad_live);
    let spinner_ids = std::mem::take(&mut s.spinner_live);
    let coin_ids = std::mem::take(&mut s.coin_live);
    let flip_ids = std::mem::take(&mut s.flip_live);
    let sx2_ids = std::mem::take(&mut s.score_x2_live);
    let zg_ids = std::mem::take(&mut s.zero_g_live);
    let gate_ids = std::mem::take(&mut s.gate_live);
    let gwell_ids = std::mem::take(&mut s.gwell_live);
    let turret_ids = std::mem::take(&mut s.turret_live);
    let bullet_ids: Vec<String> = s.bullet_live.drain(..).map(|(id, _, _, _)| id).collect();
    let rpad_ids = std::mem::take(&mut s.rocket_pad_live);
    let cannon_ids = std::mem::take(&mut s.cannon_live);

    for id in hook_ids.iter()
        .chain(pad_ids.iter()).chain(spinner_ids.iter()).chain(coin_ids.iter())
        .chain(flip_ids.iter()).chain(sx2_ids.iter()).chain(zg_ids.iter())
        .chain(gate_ids.iter()).chain(gwell_ids.iter()).chain(turret_ids.iter())
        .chain(bullet_ids.iter()).chain(rpad_ids.iter()).chain(cannon_ids.iter())
    {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
            obj.momentum = (0.0, 0.0);
        }
    }

    s.pool_free.extend(hook_ids);
    s.pad_free.extend(pad_ids);
    s.spinner_free.extend(spinner_ids);
    s.coin_free.extend(coin_ids);
    s.flip_free.extend(flip_ids);
    s.score_x2_free.extend(sx2_ids);
    s.zero_g_free.extend(zg_ids);
    s.gate_free.extend(gate_ids);
    s.gwell_free.extend(gwell_ids);
    s.turret_free.extend(turret_ids);
    s.bullet_free.extend(bullet_ids);
    s.rocket_pad_free.extend(rpad_ids);
    s.cannon_free.extend(cannon_ids);
    s.pad_origins.clear();
    s.spinner_origins.clear();
    s.gwell_timers.clear();
    s.turret_timers.clear();
    s.cannon_phases.clear();
    s.spawn_animations.clear();
}

fn rewind_frontiers(st: &Arc<Mutex<State>>, cx: f32) {
    let mut s = st.lock().unwrap();
    let backfill_x = cx - GEN_AHEAD * 0.3;
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

fn place_checkpoint_hook(c: &mut Canvas, st: &Arc<Mutex<State>>, cx: f32, cy: f32) {
    let mut s = st.lock().unwrap();
    let Some(id) = s.pool_free.pop() else { return; };
    s.live_hooks.push(id.clone());
    if cx > s.rightmost_x {
        s.rightmost_x = cx;
    }
    drop(s);

    if let Some(obj) = c.get_game_object_mut(&id) {
        obj.visible = true;
        obj.position = (cx - HOOK_R, cy - HOOK_R);
        obj.size = (HOOK_R * 2.0, HOOK_R * 2.0);
        obj.gravity = 0.0;
        obj.momentum = (0.0, 0.0);
        obj.rotation_momentum = 0.0;
        obj.collision_mode = CollisionMode::NonPlatform;
        obj.tags.retain(|t| t != "checkpoint_node" && t != "hook");
        obj.tags.push("checkpoint_node".into());
        obj.tags.push("hook".into());
        obj.set_image(Image {
            shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
            image: circle_cached(HOOK_R as u32, 90, 220, 255),
            color: None,
        });
        obj.set_glow(GlowConfig { color: Color(120, 220, 255, 255), width: 20.0 });
    } else {
        let mut s = st.lock().unwrap();
        s.live_hooks.retain(|n| n != &id);
        s.pool_free.push(id);
    }
}

fn hearts_hud_update(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (hearts, max) = {
        let s = st.lock().unwrap();
        (s.hearts.max(0), s.max_hearts.max(1))
    };
    c.set_var("hearts", Value::I32(hearts));
    if let Some(obj) = c.get_game_object_mut("hearts_hud") {
        let total_w = HEART_W * max as f32 + HEART_GAP * (max as f32 - 1.0);
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (total_w, HEART_H), 0.0),
            image: hearts_img(hearts as u32, max as u32).into(),
            color: None,
        });
        obj.size = (total_w, HEART_H);
        obj.visible = true;
        obj.update_image_shape();
    }
}
