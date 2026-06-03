use quartz::*;
use std::sync::{Arc, Mutex};

use crate::achievements::*;
use crate::constants::*;
use crate::state::*;
use super::helpers::{find_collected_pickups, pad_thruster_id, sfx_vol};

pub fn tick_pickups(c: &mut Canvas, st: &Arc<Mutex<State>>, tech_bounce_img: &Image, tech_bounce_img_flipped: &Image, thruster_anim: Option<&AnimatedSprite>, thruster_anim_flipped: Option<&AnimatedSprite>) {
    tick_coin_magnet(c, st);
    tick_powerup_magnet(c, st);
    tick_coin_collect(c, st);
    tick_flip_collect(c, st, tech_bounce_img, tech_bounce_img_flipped, thruster_anim, thruster_anim_flipped);
    tick_score_x2_collect(c, st);
    tick_zero_g_collect(c, st);
    tick_flip_timer(c, st, tech_bounce_img, tech_bounce_img_flipped, thruster_anim, thruster_anim_flipped);
    tick_score_x2_timer(st);
    tick_zero_g_timer(c, st);
}

/// Park and hide collected objects at (park, park).
fn park_collected(c: &mut Canvas, collected: &[String], park: f32) {
    for name in collected {
        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; obj.position = (park, park); }
    }
}

/// Check which `live` objects within magnet_r of (px,py) are not already in `locked`.
fn detect_magnet_locks(c: &Canvas, live: &[String], locked: &[String], px: f32, py: f32, magnet_r: f32) -> Vec<String> {
    live.iter().filter(|name| {
        !locked.contains(name) && c.get_game_object(name).map_or(false, |obj| {
            let dx = px - (obj.position.0 + obj.size.0 * 0.5);
            let dy = py - (obj.position.1 + obj.size.1 * 0.5);
            dx * dx + dy * dy < magnet_r * magnet_r
        })
    }).cloned().collect()
}

/// Apply pull force toward (px, py) for all locked objects.
fn apply_magnet_pull(c: &mut Canvas, locked: &[String], px: f32, py: f32) {
    for name in locked {
        if let Some(obj) = c.get_game_object_mut(name) {
            let cx = obj.position.0 + obj.size.0 * 0.5;
            let cy = obj.position.1 + obj.size.1 * 0.5;
            let dx = px - cx; let dy = py - cy;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let pull = (POWERUP_MAGNET_PULL * dist).min(dist);
            obj.position.0 += dx / dist * pull;
            obj.position.1 += dy / dist * pull;
        }
    }
}

// ── Mirror all live obstacles around VH centre on gravity flip ──────────────

fn flip_all_live_objects(c: &mut Canvas, s: &State, tech_bounce_img: &Image, tech_bounce_img_flipped: &Image, thruster_anim: Option<&AnimatedSprite>, thruster_anim_flipped: Option<&AnimatedSprite>) {
    let flipped = s.gravity_dir < 0.0;
    let pad_img = if flipped { tech_bounce_img_flipped } else { tech_bounce_img };
    // Mirror helper: new_y = VH - old_y - height
    // Simple pools: just flip Y
    for name in s.live_hooks.iter()
        .chain(&s.spinner_live)
        .chain(&s.coin_live)
        .chain(&s.flip_live)
        .chain(&s.score_x2_live)
        .chain(&s.zero_g_live)
        .chain(&s.gwell_live)
        .chain(&s.turret_live)
        .chain(&s.space_asteroid_live)
    {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    for (bname, _, _, _) in &s.bullet_live {
        if let Some(obj) = c.get_game_object_mut(bname) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
        }
    }
    // Pads
    for name in &s.pad_live {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.1 = VH - obj.position.1 - obj.size.1;
            obj.rotation = 0.0;
            obj.animated_sprite = None;
            obj.set_image(pad_img.clone());
        }
        let thr_id = pad_thruster_id(name);
        if let Some(thr) = c.get_game_object_mut(&thr_id) {
            thr.position.1 = VH - thr.position.1 - thr.size.1;
            thr.rotation = 0.0;
            let thr_tmpl = if flipped { thruster_anim_flipped } else { thruster_anim };
            if let Some(anim) = thr_tmpl {
                thr.set_animation(anim.clone());
            }
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
}

/// Also mirror the mover origin Y values so animated movers stay in sync.
fn flip_mover_origins(c: &mut Canvas, s: &mut State) {
    // Spinner origins: (id, origin_y, amp, speed, phase)
    for entry in s.spinner_origins.iter_mut() {
        entry.1 = VH - entry.1 - SPINNER_H;
    }
    // Pad origins: (id, origin_x, amp, speed, phase) — pads move horizontally,
    // but their Y is set by position so we don't need to flip origin_x.
    // However pad positions are already flipped above, so nothing extra needed.

    // SpawnAnim: gravity_dir is already the NEW direction when this is called.
    // For STARTED animations: snap immediately to the flipped target so there is
    // no push-down artifact from mid-flight interpolation fighting the flip.
    // For NOT-YET-STARTED animations: flip the target and recompute start_y so
    // the drop-in comes from the correct off-screen side for the new gravity.
    let drop_sign: f32 = if s.gravity_dir < 0.0 { -1.0 } else { 1.0 };
    for anim in s.spawn_animations.iter_mut() {
        let h = c.get_game_object(&anim.id).map(|o| o.size.1).unwrap_or(0.0);
        let new_target = VH - anim.target_y - h;
        if anim.started {
            // Snap: set position directly and mark animation complete.
            anim.target_y = new_target;
            anim.start_y  = new_target;
            anim.elapsed  = anim.total;
            if let Some(obj) = c.get_game_object_mut(&anim.id) {
                obj.position.1 = new_target;
            }
        } else {
            // Not started: flip target, recompute start so entry comes from
            // the correct off-screen side (above in normal, below in flipped).
            anim.target_y = new_target;
            anim.start_y  = new_target - drop_sign * SPAWN_ANIM_DROP;
        }
    }
}

fn mirror_player_for_flip(s: &mut State) {
    s.vy = -s.vy;
    s.py = VH - s.py;
    if s.hooked {
        s.hook_y = VH - s.hook_y;
    }
}

fn apply_flip_transform(c: &mut Canvas, s: &mut State, tech_bounce_img: &Image, tech_bounce_img_flipped: &Image, thruster_anim: Option<&AnimatedSprite>, thruster_anim_flipped: Option<&AnimatedSprite>) {
    mirror_player_for_flip(s);
    flip_all_live_objects(c, s, tech_bounce_img, tech_bounce_img_flipped, thruster_anim, thruster_anim_flipped);
    flip_mover_origins(c, s);
    // Negate bullet vertical velocities so they keep flying in the right direction.
    for (_, _, vy, _) in s.bullet_live.iter_mut() {
        *vy = -*vy;
    }
}

pub fn trigger_flip(c: &mut Canvas, st: &Arc<Mutex<State>>, tech_bounce_img: &Image, tech_bounce_img_flipped: &Image, thruster_anim: Option<&AnimatedSprite>, thruster_anim_flipped: Option<&AnimatedSprite>) {
    let mut s = st.lock().unwrap();
    s.gravity_dir *= -1.0;
    s.flip_timer = FLIP_DURATION;
    apply_flip_transform(c, &mut s, tech_bounce_img, tech_bounce_img_flipped, thruster_anim, thruster_anim_flipped);
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

// ── Powerup magnet pull ──────────────────────────────────────────────────────

fn tick_powerup_magnet(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let (px, py) = (s.px, s.py);
    let magnet_r = POWERUP_MAGNET_RADIUS;

    let new_flip  = detect_magnet_locks(c, &s.flip_live.clone(),     &s.flip_magnet_locked,     px, py, magnet_r);
    let new_x2    = detect_magnet_locks(c, &s.score_x2_live.clone(), &s.score_x2_magnet_locked, px, py, magnet_r);
    let new_zg    = detect_magnet_locks(c, &s.zero_g_live.clone(),   &s.zero_g_magnet_locked,   px, py, magnet_r);
    s.flip_magnet_locked.extend(new_flip);
    s.score_x2_magnet_locked.extend(new_x2);
    s.zero_g_magnet_locked.extend(new_zg);

    let flip_locked   = s.flip_magnet_locked.clone();
    let x2_locked     = s.score_x2_magnet_locked.clone();
    let zg_locked     = s.zero_g_magnet_locked.clone();
    drop(s);

    apply_magnet_pull(c, &flip_locked, px, py);
    apply_magnet_pull(c, &x2_locked,   px, py);
    apply_magnet_pull(c, &zg_locked,   px, py);
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

    if !collected.is_empty() {
        let current_total = match c.get_var(TOTAL_COINS_COLLECTED_VAR) {
            Some(Value::I32(v)) => v.max(0),
            _ => 0,
        };
        let gained = (collected.len() as i32).saturating_mul(coin_mult as i32);
        let new_total = current_total.saturating_add(gained);
        c.set_var(TOTAL_COINS_COLLECTED_VAR, new_total);
        let _ = maybe_unlock_gold_master(c, new_total);
    }

    for name in &collected {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-3700.0, -3700.0);
        }
    }

    if !collected.is_empty() {
        c.play_sound_with(ASSET_COIN_SFX_3, SoundOptions::new().volume(sfx_vol(c, 0.2)));
    }
}

// ── Flip collect ────────────────────────────────────────────────────────────

fn tick_flip_collect(c: &mut Canvas, st: &Arc<Mutex<State>>, tech_bounce_img: &Image, tech_bounce_img_flipped: &Image, thruster_anim: Option<&AnimatedSprite>, thruster_anim_flipped: Option<&AnimatedSprite>) {
    let mut s = st.lock().unwrap();
    let collected = find_collected_pickups(c, &s.flip_live.clone(), FLIP_W, FLIP_H);
    for name in &collected {
        s.flip_live.retain(|n| n != name); s.flip_magnet_locked.retain(|n| n != name); s.flip_free.push(name.clone());
        let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
        s.score = s.score.saturating_add(50u32.saturating_mul(score_mult));
    }
    drop(s);
    park_collected(c, &collected, -3800.0);
    if !collected.is_empty() {
        trigger_flip(c, st, tech_bounce_img, tech_bounce_img_flipped, thruster_anim, thruster_anim_flipped);
        c.play_sound_with(ASSET_COIN_SFX_2, SoundOptions::new().volume(sfx_vol(c, 0.2)));
        if let Some(cam) = c.camera_mut() {
            cam.flash_with(Color(160, 50, 220, 200), 0.50, FlashMode::Pulse, FlashEase::Sharp, 0.85, 0.02);
            cam.shake(60.0, 0.60);
        }
    }
}

fn tick_score_x2_collect(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let collected = find_collected_pickups(c, &s.score_x2_live.clone(), SCORE_X2_W, SCORE_X2_H);
    for name in &collected {
        s.score_x2_live.retain(|n| n != name); s.score_x2_magnet_locked.retain(|n| n != name);
        s.score_x2_free.push(name.clone()); s.score_x2_timer = SCORE_X2_DURATION;
    }
    drop(s);
    park_collected(c, &collected, -3850.0);
}

fn tick_zero_g_collect(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let collected = find_collected_pickups(c, &s.zero_g_live.clone(), ZERO_G_W, ZERO_G_H);
    for name in &collected {
        s.zero_g_live.retain(|n| n != name); s.zero_g_magnet_locked.retain(|n| n != name);
        s.zero_g_free.push(name.clone()); s.zero_g_timer = ZERO_G_DURATION;
        let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
        s.score = s.score.saturating_add(50u32.saturating_mul(score_mult));
    }
    drop(s);
    park_collected(c, &collected, -3875.0);
    if !collected.is_empty() { c.play_sound_with(ASSET_COIN_SFX_2, SoundOptions::new().volume(sfx_vol(c, 0.2))); }
}

// ── Flip timer ──────────────────────────────────────────────────────────────

fn tick_flip_timer(c: &mut Canvas, st: &Arc<Mutex<State>>, tech_bounce_img: &Image, tech_bounce_img_flipped: &Image, thruster_anim: Option<&AnimatedSprite>, thruster_anim_flipped: Option<&AnimatedSprite>) {
    let mut s = st.lock().unwrap();
    if s.flip_timer > 0 {
        s.flip_timer -= 1;
        if s.flip_timer == 0 {
            // Gravity reverts.
            s.gravity_dir *= -1.0;
            apply_flip_transform(c, &mut s, tech_bounce_img, tech_bounce_img_flipped, thruster_anim, thruster_anim_flipped);
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
