use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

pub fn tick_pickups(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    tick_coin_magnet(c, st);
    tick_coin_collect(c, st);
    tick_flip_collect(c, st);
    tick_score_x2_collect(c, st);
    tick_zero_g_collect(c, st);
    tick_flip_timer(c, st);
    tick_score_x2_timer(st);
    tick_zero_g_timer(c, st);
}

// ── Mirror all live obstacles around VH centre on gravity flip ──────────────

fn flip_all_live_objects(c: &mut Canvas, s: &State) {
    // Mirror helper: new_y = VH - old_y - height
    // Hooks
    for name in &s.live_hooks {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Pads
    for name in &s.pad_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Spinners
    for name in &s.spinner_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Coins
    for name in &s.coin_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Flip pickups
    for name in &s.flip_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Score x2 pickups
    for name in &s.score_x2_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Zero-g pickups
    for name in &s.zero_g_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Gate segments
    for gate_id in &s.gate_live {
        let top_id = format!("{gate_id}_top");
        let bot_id = format!("{gate_id}_bot");
        for seg_id in [top_id, bot_id] {
            if let Some(obj) = c.get_game_object_mut(&seg_id) {
                obj.position.1 = VH - obj.position.1 - obj.size.1;
            }
        }
    }
    // Gravity wells
    for name in &s.gwell_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Turrets
    for name in &s.turret_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Bullets (position only; vy is negated in apply_flip_transform)
    for (name, _, _, _) in &s.bullet_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
}

/// Also mirror the mover origin Y values so animated movers stay in sync.
fn flip_mover_origins(s: &mut State) {
    // Spinner origins: (id, origin_y, amp, speed, phase)
    for entry in s.spinner_origins.iter_mut() {
        entry.1 = VH - entry.1 - SPINNER_H;
    }
    // Pad origins: (id, origin_x, amp, speed, phase) — pads move horizontally,
    // but their Y is set by position so we don't need to flip origin_x.
    // However pad positions are already flipped above, so nothing extra needed.
}

fn mirror_player_for_flip(s: &mut State) {
    s.vy = -s.vy;
    s.py = (VH - s.py).clamp(PLAYER_R, VH - PLAYER_R);
    if s.hooked {
        s.hook_y = (VH - s.hook_y).clamp(HOOK_R, VH - HOOK_R);
    }
}

fn apply_flip_transform(c: &mut Canvas, s: &mut State) {
    mirror_player_for_flip(s);
    flip_all_live_objects(c, s);
    flip_mover_origins(s);
    // Negate bullet vertical velocities so they keep flying in the right direction.
    for (_, _, vy, _) in s.bullet_live.iter_mut() {
        *vy = -*vy;
    }
}

pub fn trigger_flip(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    s.gravity_dir *= -1.0;
    s.flip_timer = FLIP_DURATION;
    apply_flip_transform(c, &mut s);
    let gdir = s.gravity_dir;
    let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
    let hooked = s.hooked;
    drop(s);

    // Snap camera Y so the zoom-anchor switch (VH ↔ 0) doesn't strand the
    // viewport at the old baseline.  Without this, lerp_toward skips Y
    // (because zoom_anchor is Some) and the camera never catches up.
    if let Some(cam) = c.camera_mut() {
        if gdir < 0.0 {
            cam.position.1 = 0.0;
        } else {
            cam.position.1 = VH - VH / cam.zoom;
        }
    }

    if !hooked {
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * gravity_scale * gdir;
        }
    }
}

// ── Coin magnet pull ────────────────────────────────────────────────────────

fn tick_coin_magnet(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let magnet_r = COIN_MAGNET_RADIUS;
    let live = s.coin_live.clone();
    let mut newly_locked: Vec<String> = Vec::new();

    for name in &live {
        if s.coin_magnet_locked.contains(name) { continue; }
        if let Some(obj) = c.get_game_object(name) {
            let cx = obj.position.0 + COIN_R;
            let cy = obj.position.1 + COIN_R;
            let dx = s.px - cx;
            let dy = s.py - cy;
            if dx * dx + dy * dy < magnet_r * magnet_r {
                newly_locked.push(name.clone());
            }
        }
    }

    for name in &newly_locked {
        s.coin_magnet_locked.push(name.clone());
    }
    drop(s);

    let s = st.lock().unwrap();
    let locked = s.coin_magnet_locked.clone();
    let px = s.px;
    let py = s.py;
    drop(s);

    for name in &locked {
        if let Some(obj) = c.get_game_object_mut(name) {
            let cx = obj.position.0 + COIN_R;
            let cy = obj.position.1 + COIN_R;
            let dx = px - cx;
            let dy = py - cy;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let pull = (COIN_MAGNET_PULL * dist).min(dist);
            obj.position.0 += dx / dist * pull;
            obj.position.1 += dy / dist * pull;
        }
    }
}

// ── Coin collect ────────────────────────────────────────────────────────────

fn tick_coin_collect(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let collect_r = PLAYER_R + COIN_R + 10.0;
    let live = s.coin_live.clone();
    let mut collected: Vec<String> = Vec::new();

    for name in &live {
        if let Some(obj) = c.get_game_object(name) {
            let cx = obj.position.0 + COIN_R;
            let cy = obj.position.1 + COIN_R;
            let dx = s.px - cx;
            let dy = s.py - cy;
            if dx * dx + dy * dy < collect_r * collect_r {
                collected.push(name.clone());
            }
        }
    }

    let coin_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
    for name in &collected {
        s.coin_count += coin_mult;
        s.coin_live.retain(|n| n != name);
        s.coin_magnet_locked.retain(|n| n != name);
        s.coin_free.push(name.clone());
    }
    drop(s);

    for name in &collected {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-3700.0, -3700.0);
        }
    }

    if !collected.is_empty() {
        let sfx_path = match c.get_i32("coin_sfx_index") {
            1 => ASSET_COIN_SFX_1,
            2 => ASSET_COIN_SFX_2,
            3 => ASSET_COIN_SFX_4,
            _ => ASSET_COIN_SFX_3,
        };
        c.play_sound_with(sfx_path, SoundOptions::new().volume(0.2));
    }
}

// ── Flip collect ────────────────────────────────────────────────────────────

fn tick_flip_collect(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let collect_r = PLAYER_R + (FLIP_W.min(FLIP_H)) * 0.5 + 10.0;
    let live = s.flip_live.clone();
    let mut collected: Vec<String> = Vec::new();

    for name in &live {
        if let Some(obj) = c.get_game_object(name) {
            let cx = obj.position.0 + FLIP_W * 0.5;
            let cy = obj.position.1 + FLIP_H * 0.5;
            let dx = s.px - cx;
            let dy = s.py - cy;
            if dx * dx + dy * dy < collect_r * collect_r {
                collected.push(name.clone());
            }
        }
    }

    for name in &collected {
        s.flip_live.retain(|n| n != name);
        s.flip_free.push(name.clone());
        let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
        s.score = s.score.saturating_add(50u32.saturating_mul(score_mult));
    }
    drop(s);

    for name in &collected {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-3800.0, -3800.0);
        }
    }

    if !collected.is_empty() {
        trigger_flip(c, st);

        // coin_up SFX on gravity flip collect
        c.play_sound_with(ASSET_COIN_SFX_2, SoundOptions::new().volume(0.2));

        // Purple flash + screen shake on gravity flip collect
        if let Some(cam) = c.camera_mut() {
            cam.flash_with(
                Color(160, 50, 220, 200),
                0.50,
                FlashMode::Pulse,
                FlashEase::Sharp,
                0.85,
                0.02,
            );
            cam.shake(60.0, 0.60);
        }
    }
}

// ── Score x2 collect ────────────────────────────────────────────────────────

fn tick_score_x2_collect(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let collect_r = PLAYER_R + (SCORE_X2_W.min(SCORE_X2_H)) * 0.5 + 10.0;
    let live = s.score_x2_live.clone();
    let mut collected: Vec<String> = Vec::new();

    for name in &live {
        if let Some(obj) = c.get_game_object(name) {
            let cx = obj.position.0 + SCORE_X2_W * 0.5;
            let cy = obj.position.1 + SCORE_X2_H * 0.5;
            let dx = s.px - cx;
            let dy = s.py - cy;
            if dx * dx + dy * dy < collect_r * collect_r {
                collected.push(name.clone());
            }
        }
    }

    for name in &collected {
        s.score_x2_live.retain(|n| n != name);
        s.score_x2_free.push(name.clone());
        s.score_x2_timer = SCORE_X2_DURATION;
    }
    drop(s);

    for name in &collected {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-3850.0, -3850.0);
        }
    }
}

// ── Zero-g collect ──────────────────────────────────────────────────────────

fn tick_zero_g_collect(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let collect_r = PLAYER_R + (ZERO_G_W.min(ZERO_G_H)) * 0.5 + 10.0;
    let live = s.zero_g_live.clone();
    let mut collected: Vec<String> = Vec::new();

    for name in &live {
        if let Some(obj) = c.get_game_object(name) {
            let cx = obj.position.0 + ZERO_G_W * 0.5;
            let cy = obj.position.1 + ZERO_G_H * 0.5;
            let dx = s.px - cx;
            let dy = s.py - cy;
            if dx * dx + dy * dy < collect_r * collect_r {
                collected.push(name.clone());
            }
        }
    }

    for name in &collected {
        s.zero_g_live.retain(|n| n != name);
        s.zero_g_free.push(name.clone());
        s.zero_g_timer = ZERO_G_DURATION;
        let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
        s.score = s.score.saturating_add(50u32.saturating_mul(score_mult));
    }
    drop(s);

    for name in &collected {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-3875.0, -3875.0);
        }
    }

    if !collected.is_empty() {
        c.play_sound_with(ASSET_COIN_SFX_2, SoundOptions::new().volume(0.2));
    }
}

// ── Flip timer ──────────────────────────────────────────────────────────────

fn tick_flip_timer(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.flip_timer > 0 {
        s.flip_timer -= 1;
        if s.flip_timer == 0 {
            // Gravity reverts.
            s.gravity_dir *= -1.0;
            apply_flip_transform(c, &mut s);
            let gdir = s.gravity_dir;
            let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
            let hooked = s.hooked;
            drop(s);

            // Snap camera Y for the new anchor baseline (same as trigger_flip).
            if let Some(cam) = c.camera_mut() {
                if gdir < 0.0 {
                    cam.position.1 = 0.0;
                } else {
                    cam.position.1 = VH - VH / cam.zoom;
                }
            }

            if !hooked {
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.gravity = GRAVITY * gravity_scale * gdir;
                }
            }
            return;
        }
    }
}

// ── Score x2 timer ──────────────────────────────────────────────────────────

fn tick_score_x2_timer(st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.score_x2_timer > 0 { s.score_x2_timer -= 1; }
}

// ── Zero-g timer ────────────────────────────────────────────────────────────

fn tick_zero_g_timer(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.zero_g_timer > 0 {
        s.zero_g_timer -= 1;
        if s.zero_g_timer == 0 && !s.hooked {
            let gdir = s.gravity_dir;
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.gravity = GRAVITY * gdir;
            }
            return;
        }
    }
}
