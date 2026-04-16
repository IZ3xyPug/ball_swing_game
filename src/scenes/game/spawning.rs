use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::*;
use crate::images::*;
use crate::state::*;
use super::helpers::*;

pub fn tick_spawning(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    coin_spawn_image: &Image,
    coin_spawn_anim: &Option<AnimatedSprite>,
) {
    spawn_hooks(c, st);
    spawn_pads(c, st);
    spawn_spinners(c, st);
    spawn_coins(c, st, coin_spawn_image, coin_spawn_anim);
    spawn_flips(c, st);
    spawn_score_x2(c, st);
    spawn_zero_g(c, st);
    spawn_gates(c, st);
    spawn_gravity_wells(c, st);
}

// ── Hooks ─────────────────────────────────────────────────────────────────────

fn spawn_hooks(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut hooks_spawned = 0usize;
    while hooks_spawned < HOOKS_SPAWN_BUDGET_PER_TICK
        && !s.pending.is_empty()
        && !s.pool_free.is_empty()
        && s.rightmost_x < s.px + GEN_AHEAD
    {
        let spec = s.pending.pop_front().unwrap();
        let Some(id) = s.pool_free.pop() else { break; };

        let mut hx = spec.x;
        let hy = spec.y;

        // Push away from nearby spinners.
        let hook_spinner_min_x_gap = HOOK_SPINNER_MIN_X_GAP;
        for spinner_name in &s.spinner_live {
            if let Some(spinner_obj) = c.get_game_object(spinner_name) {
                let scx = spinner_obj.position.0 + SPINNER_W * 0.5;
                let dx = hx - scx;
                if dx.abs() < hook_spinner_min_x_gap {
                    let dir = if dx >= 0.0 { 1.0 } else { -1.0 };
                    hx += dir * HOOK_SPINNER_PUSH_X;
                }
            }
        }

        s.live_hooks.push(id.clone());
        if spec.x > s.rightmost_x { s.rightmost_x = spec.x; }
        hooks_spawned += 1;

        let zone_idx = zone_index_for_distance(s.distance);
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            let (r, g, b) = hook_base_for_zone(zone_idx);
            obj.position = (hx - HOOK_R, hy - HOOK_R);
            obj.size = (HOOK_R * 2.0, HOOK_R * 2.0);
            obj.visible = true;
            obj.set_image(hook_img(r, g, b));
            obj.clear_highlight();
        }

        s = st.lock().unwrap();

        // Generate more hooks when pending queue runs low.
        if s.pending.len() < 3 {
            let from_x = s.rightmost_x + 620.0;
            let difficulty = (s.distance / 10000.0).min(3.0);
            let mut seed = s.seed;
            let mut gen_y = s.gen_y;
            let batch = gen_hook_batch(&mut seed, from_x, &mut gen_y, difficulty);
            s.seed = seed;
            s.gen_y = gen_y;
            s.pending.extend(batch);
        }
    }
}

// ── Pads ──────────────────────────────────────────────────────────────────────

fn spawn_pads(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut pads_spawned = 0usize;
    while pads_spawned < PADS_SPAWN_BUDGET_PER_TICK
        && s.pad_rightmost < s.px + GEN_AHEAD
        && !s.pad_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, PAD_GAP_MIN, PAD_GAP_MAX);
        let x = s.pad_rightmost + gap;
        let raw_y = lcg_range(&mut s.seed, VH * 0.38, VH - PAD_H - 40.0);
        let y = if s.gravity_dir < 0.0 { VH - raw_y - PAD_H } else { raw_y };
        let Some(id) = s.pad_free.pop() else { break; };
        s.pad_live.push(id.clone());
        s.pad_rightmost = x;
        pads_spawned += 1;

        let is_mover = lcg(&mut s.seed) < 0.35;
        let (origin_x, amp, speed, phase) = if is_mover {
            let a = lcg_range(&mut s.seed, PAD_MOVE_RANGE * 0.3, PAD_MOVE_RANGE);
            let sp = lcg_range(&mut s.seed, PAD_MOVE_SPEED * 0.5, PAD_MOVE_SPEED * 1.5);
            let ph = lcg(&mut s.seed) * std::f32::consts::TAU;
            (x, a, sp, ph)
        } else {
            (x, 0.0, 0.0, 0.0)
        };
        s.pad_origins.push((id.clone(), origin_x, amp, speed, phase));

        let zone_idx = zone_index_for_distance(s.distance);
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            let (r, g, b) = pad_for_zone(zone_idx);
            obj.position = (x, y);
            obj.visible = true;
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (PAD_W, PAD_H), 0.0),
                image: pad_img(PAD_W as u32, PAD_H as u32, r, g, b).into(),
                color: None,
            });
        }

        s = st.lock().unwrap();
    }
}

// ── Spinners ──────────────────────────────────────────────────────────────────

fn spawn_spinners(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spinners_spawned = 0usize;
    while spinners_spawned < SPINNERS_SPAWN_BUDGET_PER_TICK
        && s.spinner_rightmost < s.px + GEN_AHEAD
        && !s.spinner_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, SPINNER_GAP_MIN, SPINNER_GAP_MAX);
        let x = s.spinner_rightmost + gap;
        let y = lcg_range(&mut s.seed, VH * 0.12, VH - SPINNER_H - 60.0);
        let Some(id) = s.spinner_free.pop() else { break; };
        s.spinner_live.push(id.clone());
        s.spinner_rightmost = x;
        spinners_spawned += 1;

        let zone_idx = zone_index_for_distance(s.distance);
        let spin_dir = if lcg(&mut s.seed) < 0.5 { 1.0 } else { -1.0 };
        let rot_speed = spinner_speed_for_zone(zone_idx) * spin_dir;

        // Zone 2+ spinners can move vertically.
        let is_mover = zone_idx >= 2 && lcg(&mut s.seed) < 0.5;
        let (origin_y, amp, speed, phase) = if is_mover {
            let a = lcg_range(&mut s.seed, SPINNER_BLACK_MOVE_AMP_MIN, SPINNER_BLACK_MOVE_AMP_MAX);
            let sp = lcg_range(&mut s.seed, SPINNER_BLACK_MOVE_SPEED_MIN, SPINNER_BLACK_MOVE_SPEED_MAX);
            let ph = lcg(&mut s.seed) * std::f32::consts::TAU;
            (y, a, sp, ph)
        } else {
            (y, 0.0, 0.0, 0.0)
        };
        s.spinner_origins.push((id.clone(), origin_y, amp, speed, phase));

        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            let (r, g, b) = spinner_for_zone(zone_idx);
            obj.position = (x, y);
            obj.visible = true;
            obj.rotation_momentum = rot_speed;
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (SPINNER_W, SPINNER_H), 0.0),
                image: spinner_img(SPINNER_W as u32, SPINNER_H as u32, r, g, b).into(),
                color: None,
            });
        }

        s = st.lock().unwrap();
    }
}

// ── Coins ─────────────────────────────────────────────────────────────────────

fn spawn_coins(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    coin_spawn_image: &Image,
    coin_spawn_anim: &Option<AnimatedSprite>,
) {
    let mut s = st.lock().unwrap();
    let mut batches = 0usize;
    while batches < COIN_BATCHES_BUDGET_PER_TICK
        && s.coin_rightmost < s.px + GEN_AHEAD
        && !s.coin_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, COIN_GAP_MIN, COIN_GAP_MAX);
        let desired_start_x = s.coin_rightmost + gap;
        let spawn_array = s.coin_free.len() >= COIN_ARRAY_COUNT && lcg(&mut s.seed) < COIN_ARRAY_CHANCE;
        let mut spawn_batch: Vec<(String, f32, f32, usize)> = Vec::new();
        let mut spawned_start_x = desired_start_x;
        let coin_anim_frames = coin_spawn_anim.as_ref().map(|a| a.frame_count().max(1)).unwrap_or(1);
        let array_phase_frame = (lcg(&mut s.seed) * coin_anim_frames as f32) as usize;

        if spawn_array {
            let mut best_anchor: Option<(f32, f32)> = None;
            let mut best_score = f32::INFINITY;
            let hook_ids = s.live_hooks.clone();
            for hid in &hook_ids {
                if let Some(hook_obj) = c.get_game_object(hid) {
                    let hcx = hook_obj.position.0 + HOOK_R;
                    let hcy = hook_obj.position.1 + HOOK_R;
                    let raw_y = if s.gravity_dir < 0.0 { VH - hcy } else { hcy };
                    let candidate_x = hcx + COIN_ARRAY_HOOK_DX;
                    let score = (candidate_x - desired_start_x).abs();
                    if score < best_score {
                        best_score = score;
                        best_anchor = Some((hcx, raw_y));
                    }
                }
            }

            let center_raw_y = if let Some((hook_cx, raw_y)) = best_anchor {
                spawned_start_x = hook_cx + COIN_ARRAY_HOOK_DX;
                raw_y + COIN_ARRAY_HOOK_DY
            } else {
                let center_min = (COIN_ARRAY_Y_MIN + COIN_CURVE_RISE).min(COIN_ARRAY_Y_MAX);
                lcg_range(&mut s.seed, center_min, COIN_ARRAY_Y_MAX)
            };

            let half = (COIN_ARRAY_COUNT as f32 - 1.0) * 0.5;
            for i in 0..COIN_ARRAY_COUNT {
                let x = spawned_start_x + i as f32 * COIN_ARRAY_SPACING;
                let t = i as f32 - half;
                let norm = if half > 0.0 { (t.abs() / half).clamp(0.0, 1.0) } else { 0.0 };
                let arch = 1.0 - norm * norm;
                let raw_y = center_raw_y - arch * COIN_CURVE_RISE;
                let y = if s.gravity_dir < 0.0 { VH - raw_y } else { raw_y };
                let Some(id) = s.coin_free.pop() else { break; };
                s.coin_live.push(id.clone());
                spawn_batch.push((id, x, y, array_phase_frame.min(coin_anim_frames - 1)));
            }
        } else {
            let raw_y = lcg_range(&mut s.seed, COIN_SINGLE_Y_MIN, COIN_SINGLE_Y_MAX);
            let y = if s.gravity_dir < 0.0 { VH - raw_y } else { raw_y };
            if let Some(id) = s.coin_free.pop() {
                s.coin_live.push(id.clone());
                let single_phase = ((lcg(&mut s.seed) * coin_anim_frames as f32) as usize).min(coin_anim_frames - 1);
                spawn_batch.push((id, desired_start_x, y, single_phase));
            }
        }

        if spawn_batch.is_empty() { break; }

        s.coin_rightmost = if spawn_array {
            spawned_start_x + (COIN_ARRAY_COUNT as f32 - 1.0) * COIN_ARRAY_SPACING
        } else {
            desired_start_x
        };
        batches += 1;
        drop(s);

        for (id, cx, cy, phase) in &spawn_batch {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.position = (*cx - COIN_R, *cy - COIN_R);
                obj.visible = true;
                obj.set_image(coin_spawn_image.clone());
                if let Some(anim) = coin_spawn_anim {
                    if obj.animated_sprite.is_none() {
                        obj.set_animation(anim.clone());
                    }
                    if let Some(a) = obj.animated_sprite.as_mut() {
                        a.set_frame(*phase);
                    }
                }
            }
        }

        s = st.lock().unwrap();
    }
}

// ── Flips ─────────────────────────────────────────────────────────────────────

fn spawn_flips(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut flips_spawned = 0usize;
    while flips_spawned < FLIPS_SPAWN_BUDGET_PER_TICK
        && s.flip_rightmost < s.px + GEN_AHEAD
        && !s.flip_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, FLIP_GAP_MIN, FLIP_GAP_MAX);
        let x = s.flip_rightmost + gap;
        let raw_y = lcg_range(&mut s.seed, VH * 0.12, VH * 0.70);
        let y = if s.gravity_dir < 0.0 { VH - raw_y - FLIP_H } else { raw_y };
        let Some(id) = s.flip_free.pop() else { break; };
        s.flip_live.push(id.clone());
        s.flip_rightmost = x;
        flips_spawned += 1;
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position = (x, y);
            obj.visible = true;
        }

        s = st.lock().unwrap();
    }
}

// ── Score x2 ──────────────────────────────────────────────────────────────────

fn spawn_score_x2(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < 1
        && s.score_x2_rightmost < s.px + GEN_AHEAD
        && !s.score_x2_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, SCORE_X2_GAP_MIN, SCORE_X2_GAP_MAX);
        let x = s.score_x2_rightmost + gap;
        let raw_y = lcg_range(&mut s.seed, VH * 0.15, VH * 0.65);
        let y = if s.gravity_dir < 0.0 { VH - raw_y - SCORE_X2_H } else { raw_y };
        let Some(id) = s.score_x2_free.pop() else { break; };
        s.score_x2_live.push(id.clone());
        s.score_x2_rightmost = x;
        spawned += 1;
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position = (x, y);
            obj.visible = true;
        }

        s = st.lock().unwrap();
    }
}

// ── Zero-g ────────────────────────────────────────────────────────────────────

fn spawn_zero_g(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < ZERO_G_SPAWN_BUDGET_PER_TICK
        && s.zero_g_rightmost < s.px + GEN_AHEAD
        && !s.zero_g_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, ZERO_G_GAP_MIN, ZERO_G_GAP_MAX);
        let x = s.zero_g_rightmost + gap;
        let raw_y = lcg_range(&mut s.seed, VH * 0.15, VH * 0.65);
        let y = if s.gravity_dir < 0.0 { VH - raw_y - ZERO_G_H } else { raw_y };
        let Some(id) = s.zero_g_free.pop() else { break; };
        s.zero_g_live.push(id.clone());
        s.zero_g_rightmost = x;
        spawned += 1;
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position = (x, y);
            obj.visible = true;
        }

        s = st.lock().unwrap();
    }
}

// ── Gates ─────────────────────────────────────────────────────────────────────

fn spawn_gates(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < GATES_SPAWN_BUDGET_PER_TICK
        && GATES_ENABLED
        && s.gate_rightmost < s.px + GEN_AHEAD
        && !s.gate_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, GATE_GAP_MIN, GATE_GAP_MAX);
        let base_x = s.gate_rightmost + gap.max(GATE_MIN_CLUSTER_SEPARATION);
        let gaps_in_cluster = 2 + ((lcg(&mut s.seed) * 3.0) as usize);
        let cluster_spacing = GATE_MIN_CLUSTER_SEPARATION;
        let mut batch: Vec<(String, String, f32, Option<(String, f32, f32)>)> = Vec::new();

        for i in 0..gaps_in_cluster {
            let Some(gid) = s.gate_free.pop() else { break; };
            let gate_x = base_x + i as f32 * cluster_spacing;
            s.gate_live.push(gid.clone());
            let top_id = format!("{gid}_top");
            let bot_id = format!("{gid}_bot");

            let hook_spawn = if let Some(hook_id) = s.pool_free.pop() {
                let mut hx = gate_x - 450.0;
                for spinner_name in &s.spinner_live {
                    if let Some(so) = c.get_game_object(spinner_name) {
                        let scx = so.position.0 + SPINNER_W * 0.5;
                        let dx = hx - scx;
                        if dx.abs() < HOOK_SPINNER_MIN_X_GAP {
                            let dir = if dx >= 0.0 { 1.0 } else { -1.0 };
                            hx += dir * HOOK_SPINNER_PUSH_X;
                        }
                    }
                }
                let hy = 650.0;
                s.live_hooks.push(hook_id.clone());
                Some((hook_id, hx, hy))
            } else {
                None
            };

            batch.push((top_id, bot_id, gate_x, hook_spawn));
        }

        if batch.is_empty() { break; }

        let last_x = batch.last().map(|(_, _, x, _)| *x).unwrap_or(base_x);
        s.gate_rightmost = last_x;
        spawned += 1;
        let spinner_ids = s.spinner_live.clone();
        let zone_idx = zone_index_for_distance(s.distance);
        drop(s);

        for (top_id, bot_id, gate_x, hook_spawn) in &batch {
            if let Some(obj) = c.get_game_object_mut(top_id) {
                obj.position = (*gate_x, -GATE_VERTICAL_OVERFLOW);
                obj.size = (GATE_W, GATE_TOP_SEG_H);
                obj.visible = true;
            }
            if let Some(obj) = c.get_game_object_mut(bot_id) {
                obj.position = (*gate_x, GATE_TOP_BASE_H + GATE_GAP_H);
                obj.size = (GATE_W, GATE_BOT_SEG_H);
                obj.visible = true;
            }
            for sid in &spinner_ids {
                if let Some(sp) = c.get_game_object_mut(sid) {
                    let overlaps = sp.position.0 + SPINNER_W > *gate_x - 80.0
                        && sp.position.0 < *gate_x + GATE_W + 80.0;
                    if overlaps {
                        sp.position.0 = *gate_x + GATE_W + 240.0;
                    }
                }
            }
            if let Some((hook_id, hx, hy)) = hook_spawn {
                if let Some(obj) = c.get_game_object_mut(hook_id) {
                    let (r, g, b) = hook_base_for_zone(zone_idx);
                    obj.position = (*hx - HOOK_R, *hy - HOOK_R);
                    obj.visible = true;
                    obj.set_image(hook_img(r, g, b));
                }
            }
        }

        s = st.lock().unwrap();
    }
}

// ── Gravity wells ─────────────────────────────────────────────────────────────

fn spawn_gravity_wells(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < GWELL_SPAWN_BUDGET
        && s.gwell_rightmost < s.px + GEN_AHEAD
        && !s.gwell_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, GWELL_GAP_MIN, GWELL_GAP_MAX);
        let x = s.gwell_rightmost + gap;
        let y = lcg_range(&mut s.seed, GWELL_Y_MIN, GWELL_Y_MAX);
        let radius = lcg_range(&mut s.seed, GWELL_RADIUS_MIN, GWELL_RADIUS_MAX);
        let strength = lcg_range(&mut s.seed, GWELL_STRENGTH_MIN, GWELL_STRENGTH_MAX);
        let visual_scale = lcg_range(&mut s.seed, GWELL_VISUAL_SCALE_MIN, GWELL_VISUAL_SCALE_MAX);
        let visual_r = PLAYER_R * visual_scale;

        let Some(id) = s.gwell_free.pop() else { break; };
        s.gwell_live.push(id.clone());
        s.gwell_rightmost = x;
        s.gwell_timers.push((id.clone(), GWELL_ON_TICKS, true)); // starts active
        spawned += 1;
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            let d = visual_r * 2.0;
            obj.position = (x - visual_r, y - visual_r);
            obj.size = (d, d);
            obj.visible = true;
            obj.planet_radius = Some(radius);
            obj.gravity_strength = strength;
            // Set the stepped-alpha ring image
            let ring_img = gwell_ring_cached(
                visual_r,
                C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2,
                GWELL_RING_COUNT, 200.0,
            );
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                image: ring_img,
                color: None,
            });
            obj.set_glow(GlowConfig {
                color: Color(C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2, 200),
                width: 14.0,
            });
        }

        s = st.lock().unwrap();
    }
}
