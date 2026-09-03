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
    super::solar::tick_solar(c, st);
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
            s.buff_absorbs = crate::constants::BUFF_ABSORB_MAX;
        }
        c.set_var("debug_force_buff", false);
        // Buff is shown by the round electricity mega-shader effect (not a glow).
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
        // An attached effect lives on the object until released.
        super::fx::clear_object_fx(c, "player");
        return;
    }
    // Buff is still active: render the "electricity ball" mega-shader effect over
    // the player while the boss-damage buff is in effect.
    if s.player_buff > 0 {
        // ATTACHED to the player rather than pushed, so it rides the player's
        // own transform instead of a separately reconstructed one.
        let d = c
            .get_game_object("player")
            .map(|p| p.size.0.max(p.size.1).max(PLAYER_R * 2.0))
            .unwrap_or(PLAYER_R * 2.0);
        super::fx::attach_electric_fx(
            c, "player",
            (d * BUFF_PLAYER_FX_SCALE, d * BUFF_PLAYER_FX_SCALE),
            BUFF_PLAYER_FX_TINT,
        );
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
    // SECOND WIND spends a free respawn instead of a heart. Power-ups are still
    // lost — the upgrade buys the life, not the run state, so a rank never
    // makes a mistake free.
    {
        let mut s = st.lock().unwrap();
        if s.free_respawns_left > 0 {
            s.free_respawns_left -= 1;
            s.player_buff = 0;
            s.buff_timer = 0;
            s.buff_hit_flash = 0;
            s.zero_g_timer = 0;
            s.score_x2_timer = 0;
            let left = s.free_respawns_left as i32;
            drop(s);
            c.set_var("free_respawns_left", Value::I32(left));
            hearts_hud_update(c, st);
            if let Some(cam) = c.camera_mut() {
                // Green flash rather than red: a saved life should not read as
                // damage taken.
                cam.flash_with(Color(70, 220, 150, 110), 0.35, FlashMode::Pulse,
                               FlashEase::Sharp, 0.8, 0.0);
            }
            return false;
        }
    }

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
    // Each pad has a companion thruster (`pad_X_thruster`) that is only
    // positioned/visible via `tick_pad_thrusters` while the pad stays in
    // `pad_live`. After recycling the pad we must hide its thruster too,
    // otherwise it is left floating alone (no pad) after a respawn.
    let pad_thr_ids: Vec<String> = pad_ids.iter().map(|n| format!("{n}_thruster")).collect();
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
        .chain(pad_ids.iter()).chain(pad_thr_ids.iter()).chain(spinner_ids.iter())
        .chain(coin_ids.iter())
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
    s.cannon_rightmost = s.cannon_rightmost.min(backfill_x);
    s.rocket_pad_rightmost = s.rocket_pad_rightmost.min(backfill_x);

    // Drop hooks that were pre-generated for the (now cleared) world ahead —
    // otherwise the spawner places them at their old far-ahead positions and
    // the content near the checkpoint never regenerates. Regenerate from the
    // checkpoint X so hooks/pads/etc. follow the player back.
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

fn place_checkpoint_hook(c: &mut Canvas, st: &Arc<Mutex<State>>, cx: f32, cy: f32) {
    let mut s = st.lock().unwrap();
    let Some(id) = s.pool_free.pop() else { return; };
    s.live_hooks.push(id.clone());
    if cx > s.rightmost_x {
        s.rightmost_x = cx;
    }
    drop(s);

    let asteroid_mode = c.get_bool("asteroid_hooks_on");
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
        if asteroid_mode {
            // Match the standard artifact hook look (same as the starting hook)
            // so the respawn orbit node doesn't appear as a plain coloured circle.
            obj.set_animation(hook_artifact_anim());
            obj.size = (HOOK_ARTIFACT_R * 2.0, HOOK_ARTIFACT_R * 2.0);
            obj.clear_glow();
        } else {
            obj.set_image(hook_img(C_HOOK.0, C_HOOK.1, C_HOOK.2));
            obj.clear_glow();
        }
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
