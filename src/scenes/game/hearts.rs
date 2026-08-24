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
}

/// Tick down an active buff and clear it (and the player glow) when it expires.
fn tick_buff(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
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
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (0.0, 0.0), 0.0),
            image: hearts_img(hearts as u32, max as u32).into(),
            color: None,
        });
    }
}
