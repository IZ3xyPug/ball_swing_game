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
    // Evict Poisson-disk points that have scrolled far behind the player.
    {
        let mut s = st.lock().unwrap();
        let evict_x = s.px - 15_000.0;
        s.world_sampler.evict_before(evict_x);
    }
    spawn_hooks(c, st);
    if matches!(c.get_var("spawn_pads_on"), Some(Value::Bool(true)) | None) {
        spawn_pads(c, st);
    }
    if matches!(c.get_var("spawn_spinners_on"), Some(Value::Bool(true)) | None) {
        spawn_spinners(c, st);
    }
    if matches!(c.get_var("spawn_coins_on"), Some(Value::Bool(true)) | None) {
        spawn_coins(c, st, coin_spawn_image, coin_spawn_anim);
    }
    if matches!(c.get_var("spawn_flips_on"), Some(Value::Bool(true)) | None) {
        spawn_flips(c, st);
    }
    if matches!(c.get_var("spawn_score_x2_on"), Some(Value::Bool(true)) | None) {
        spawn_score_x2(c, st);
    }
    if matches!(c.get_var("spawn_zero_g_on"), Some(Value::Bool(true)) | None) {
        spawn_zero_g(c, st);
    }
    spawn_gates(c, st);
    if matches!(c.get_var("spawn_gwells_on"), Some(Value::Bool(true)) | None) {
        spawn_gravity_wells(c, st);
    }
    if matches!(c.get_var("spawn_turrets_on"), Some(Value::Bool(true)) | None) {
        spawn_turrets(c, st);
    }
    spawn_rocket_pads(c, st);
    spawn_main_asteroids(c, st);
}

fn circle_overlaps_aabb(cx: f32, cy: f32, r: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    let closest_x = cx.clamp(x, x + w);
    let closest_y = cy.clamp(y, y + h);
    let dx = cx - closest_x;
    let dy = cy - closest_y;
    dx * dx + dy * dy <= r * r
}

fn hook_overlaps_hazards(c: &Canvas, s: &State, hx: f32, hy: f32) -> bool {
    let r = HOOK_R;

    for pad_name in &s.pad_live {
        if let Some(pad) = c.get_game_object(pad_name) {
            if circle_overlaps_aabb(hx, hy, r, pad.position.0, pad.position.1, PAD_W, PAD_H) {
                return true;
            }
        }
    }

    for spinner_name in &s.spinner_live {
        if let Some(spinner) = c.get_game_object(spinner_name) {
            if circle_overlaps_aabb(hx, hy, r, spinner.position.0, spinner.position.1, SPINNER_W, SPINNER_H) {
                return true;
            }
        }
    }

    for gwell_name in &s.gwell_live {
        if let Some(gwell) = c.get_game_object(gwell_name) {
            let gcx = gwell.position.0 + gwell.size.0 * 0.5;
            let gcy = gwell.position.1 + gwell.size.1 * 0.5;
            let gr = gwell.size.0.min(gwell.size.1) * 0.5;
            let dx = hx - gcx;
            let dy = hy - gcy;
            let rr = r + gr;
            if dx * dx + dy * dy <= rr * rr {
                return true;
            }
        }
    }

    false
}

fn find_safe_hook_position(c: &Canvas, s: &State, base_x: f32, base_y: f32) -> Option<(f32, f32)> {
    let candidates: &[(f32, f32)] = &[
        (0.0, 0.0),
        (0.0, -100.0),
        (0.0, 100.0),
        (0.0, -220.0),
        (0.0, 220.0),
        (0.0, -300.0),
        (0.0, 300.0),
        (0.0, -420.0),
        (0.0, 420.0),
        (0.0, -500.0),
        (0.0, 500.0),
        (0.0, -620.0),
        (0.0, 620.0),
    ];

    for (dx, dy) in candidates {
        let hx = base_x + dx;
        let hy = (base_y + dy).clamp(HOOK_Y_MIN, HOOK_Y_MAX);
        if !hook_overlaps_hazards(c, s, hx, hy) {
            return Some((hx, hy));
        }
    }
    None
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
        let mut hy = spec.y;

        // Collect spinner and pad positions before the relocation loops so we
        // can draw from s.seed (mut) without holding borrow conflicts.
        let spinner_positions: Vec<(f32, f32)> = s.spinner_live.iter()
            .filter_map(|name| c.get_game_object(name)
                .map(|o| (o.position.0 + SPINNER_W * 0.5,
                          o.position.1 + SPINNER_H * 0.5)))  // (cx, center_y)
            .collect();

        let pad_positions: Vec<(f32, f32)> = s.pad_live.iter()
            .filter_map(|name| c.get_game_object(name)
                .map(|o| (o.position.0 + PAD_W * 0.5,   // pad centre x
                          o.position.1)))               // pad top y
            .collect();

        // If a grab node falls within 2x the spinner proximity radius,
        // push it above the spinner centre. This applies from any direction
        // (left/right/above/below) because the check is Euclidean.
        for (scx, scenter_y) in &spinner_positions {
            let dx = hx - scx;
            let dy = hy - scenter_y;
            let prox_r = HOOK_SPINNER_PROX_R * 2.0;
            if dx * dx + dy * dy < prox_r * prox_r {
                hy = (scenter_y - HOOK_SPINNER_Y_OFFSET).clamp(HOOK_Y_MIN, HOOK_Y_MAX);
            }
        }

        // If a grab node is too close to a bounce pad, push it HOOK_PAD_CLEAR_Y (800 px)
        // above the pad's top edge.
        for (pad_cx, pad_top) in &pad_positions {
            if (hx - pad_cx).abs() < PAD_W && hy > pad_top - HOOK_PAD_CLEAR_Y {
                hy = (pad_top - HOOK_PAD_CLEAR_Y).clamp(HOOK_Y_MIN, HOOK_Y_MAX);
            }
        }

        if let Some((safe_hx, safe_hy)) = find_safe_hook_position(c, &s, hx, hy) {
            hx = safe_hx;
            hy = safe_hy;
        }

        // Hard rule: no gap between grab nodes. Instead of discarding a hook
        // that lands too close in Y to the previous, push its Y far enough away.
        if (hy - s.last_hook_y).abs() < HOOK_CLOSE_Y_THRESHOLD {
            let above = s.last_hook_y - HOOK_CLOSE_Y_THRESHOLD;
            let below = s.last_hook_y + HOOK_CLOSE_Y_THRESHOLD;
            hy = if above >= HOOK_Y_MIN { above } else { below.min(HOOK_Y_MAX) };
        }
        hy = hy.clamp(HOOK_Y_MIN, HOOK_Y_MAX);

        s.last_hook_y = hy;
        s.live_hooks.push(id.clone());
        if hx > s.rightmost_x { s.rightmost_x = hx; }
        hooks_spawned += 1;
        // Register hook centre in the Poisson sampler so future pads/spinners
        // naturally avoid this Y position.
        s.world_sampler.add(hx, hy);

        let zone_idx = zone_index_for_distance(s.distance);
        drop(s);

        let asteroid_mode = matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position = (hx - HOOK_R, hy - HOOK_R);
            obj.size = (HOOK_R * 2.0, HOOK_R * 2.0);
            obj.visible = true;
            if asteroid_mode {
                obj.set_image(hook_asteroid_img_for_id(&id, AsteroidHookState::Base));
            } else {
                let (r, g, b) = hook_base_for_zone(zone_idx);
                obj.set_image(hook_img(r, g, b));
            }
            obj.clear_highlight();
        }

        s = st.lock().unwrap();

        // Generate more hooks when pending queue runs low.
        if s.pending.len() < 3 {
            let from_x = s.rightmost_x;
            let distance = s.distance;
            let mut seed = s.seed;
            let mut gen_head_x = s.gen_head_x;
            let mut gen_head_y = s.gen_head_y;
            let batch = gen_hook_batch(&mut seed, from_x, &mut gen_head_x, &mut gen_head_y, distance);
            s.seed = seed;
            s.gen_head_x = gen_head_x;
            s.gen_head_y = gen_head_y;
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
        let raw_y = {
            let mut seed = s.seed;
            let y = s.world_sampler.sample_y(&mut seed, x, PAD_Y_MIN, 1850.0, 20);
            s.seed = seed;
            y
        };
        let mut y = if s.gravity_dir < 0.0 { VH - raw_y - PAD_H } else { raw_y };

        // Pads must be at least PAD_BELOW_HOOK_Y_GAP below any nearby grab node.
        // The critical check is against s.pending (hooks queued but not yet
        // placed) because pads are generated far ahead — live_hooks only holds
        // hooks near the player's current position, which are too far behind to
        // be spatially relevant here.  We check both just to be safe.
        let pad_cx = x + PAD_W * 0.5;
        let mut min_pad_top: Option<f32> = None;

        for hook_name in &s.live_hooks {
            if let Some(hook_obj) = c.get_game_object(hook_name) {
                let hook_cx = hook_obj.position.0 + HOOK_R;
                if (pad_cx - hook_cx).abs() <= PAD_HOOK_NEAR_X {
                    let hook_cy = hook_obj.position.1 + HOOK_R;
                    let req = hook_cy + PAD_BELOW_HOOK_Y_GAP;
                    min_pad_top = Some(min_pad_top.map_or(req, |m: f32| m.max(req)));
                }
            }
        }
        for spec in &s.pending {
            if (pad_cx - spec.x).abs() <= PAD_HOOK_NEAR_X {
                let req = spec.y + PAD_BELOW_HOOK_Y_GAP;
                min_pad_top = Some(min_pad_top.map_or(req, |m: f32| m.max(req)));
            }
        }
        if let Some(req_top) = min_pad_top {
            y = y.max(req_top);
        }

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
            let corner_r = pad_corner_radius();
            obj.position = (x, y);
            obj.visible = true;
            obj.set_image(Image {
                shape: ShapeType::RoundedRectangle(0.0, (PAD_W, PAD_H), 0.0, corner_r),
                image: pad_cached(PAD_W as u32, PAD_H as u32, r, g, b),
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
        let y = {
            let mut seed = s.seed;
            let sampled = s.world_sampler.sample_y(&mut seed, x, -50.0, 1300.0, 20);
            s.seed = seed;
            sampled
        };
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
                image: spinner_cached(SPINNER_W as u32, SPINNER_H as u32, r, g, b),
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
        let roll = lcg(&mut s.seed);
        let spawn_grid  = s.coin_free.len() >= COIN_GRID_COLS * COIN_GRID_ROWS
            && roll < COIN_GRID_CHANCE;
        let spawn_array = !spawn_grid
            && s.coin_free.len() >= COIN_ARRAY_COUNT
            && roll < COIN_GRID_CHANCE + COIN_ARRAY_CHANCE;
        let mut spawn_batch: Vec<(String, f32, f32, usize)> = Vec::new();
        let mut spawned_start_x = desired_start_x;
        let coin_anim_frames = coin_spawn_anim.as_ref().map(|a| a.frame_count().max(1)).unwrap_or(1);
        let array_phase_frame = (lcg(&mut s.seed) * coin_anim_frames as f32) as usize;

        if spawn_grid {
            let center_min = (COIN_ARRAY_Y_MIN + COIN_CURVE_RISE).min(COIN_ARRAY_Y_MAX);
            let center_raw_y = lcg_range(&mut s.seed, center_min, COIN_ARRAY_Y_MAX);
            let half_rows = (COIN_GRID_ROWS as f32 - 1.0) * 0.5;
            'grid: for gr in 0..COIN_GRID_ROWS {
                for gc in 0..COIN_GRID_COLS {
                    let x = spawned_start_x + gc as f32 * COIN_GRID_SPACING_X;
                    let row_offset = (gr as f32 - half_rows) * COIN_GRID_SPACING_Y;
                    let raw_y = center_raw_y + row_offset;
                    let y = if s.gravity_dir < 0.0 { VH - raw_y } else { raw_y };
                    let Some(id) = s.coin_free.pop() else { break 'grid; };
                    s.coin_live.push(id.clone());
                    spawn_batch.push((id, x, y, array_phase_frame.min(coin_anim_frames - 1)));
                }
            }
        } else if spawn_array {
            let mut best_anchor: Option<(f32, f32)> = None;
            let mut best_score = f32::INFINITY;
            let hook_ids = s.live_hooks.clone();
            for hid in &hook_ids {
                if let Some(hook_obj) = c.get_game_object(hid) {
                    let hcx = hook_obj.position.0 + HOOK_R;
                    let hcy = hook_obj.position.1 + HOOK_R;
                    let raw_y = if s.gravity_dir < 0.0 { VH - hcy } else { hcy };
                    let candidate_x = hcx + COIN_ARRAY_HOOK_DX;
                    if candidate_x < desired_start_x {
                        continue;
                    }
                    let score = (candidate_x - desired_start_x).abs();
                    if score < best_score {
                        best_score = score;
                        best_anchor = Some((hcx, raw_y));
                    }
                }
            }

            let center_raw_y = if let Some((hook_cx, raw_y)) = best_anchor {
                spawned_start_x = (hook_cx + COIN_ARRAY_HOOK_DX).max(desired_start_x);
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
            // No pattern this slot — advance rightmost and continue.
            s.coin_rightmost = desired_start_x;
            batches += 1;
            drop(s);
            s = st.lock().unwrap();
            continue;
        }

        if spawn_batch.is_empty() { break; }

        s.coin_rightmost = if spawn_grid {
            spawned_start_x + (COIN_GRID_COLS as f32 - 1.0) * COIN_GRID_SPACING_X
        } else {
            spawned_start_x + (COIN_ARRAY_COUNT as f32 - 1.0) * COIN_ARRAY_SPACING
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
        let x = (s.flip_rightmost + gap)
            .max(s.score_x2_rightmost + 3000.0)
            .max(s.zero_g_rightmost + 3000.0);
        let raw_y = lcg_range(&mut s.seed, -750.0, 850.0);
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
        let x = (s.score_x2_rightmost + gap)
            .max(s.flip_rightmost + 3000.0)
            .max(s.zero_g_rightmost + 3000.0);
        let raw_y = lcg_range(&mut s.seed, -750.0, 850.0);
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
        let x = (s.zero_g_rightmost + gap)
            .max(s.flip_rightmost + 3000.0)
            .max(s.score_x2_rightmost + 3000.0);
        let raw_y = lcg_range(&mut s.seed, -750.0, 850.0);
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
                if let Some((safe_hx, safe_hy)) = find_safe_hook_position(c, &s, hx, hy) {
                    s.live_hooks.push(hook_id.clone());
                    Some((hook_id, safe_hx, safe_hy))
                } else {
                    s.pool_free.push(hook_id);
                    None
                }
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
    // Gravity wells only appear in zone 2 (the hardest difficulty band).
    if zone_index_for_distance(s.distance) < 2 {
        let z2_start_x = SPAWN_X + 2.0 * ZONE_DISTANCE_STEP;
        if s.gwell_rightmost < z2_start_x { s.gwell_rightmost = z2_start_x; }
        return;
    }
    let mut spawned = 0usize;
    while spawned < GWELL_SPAWN_BUDGET
        && s.gwell_rightmost < s.px + GEN_AHEAD
        && !s.gwell_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, GWELL_GAP_MIN, GWELL_GAP_MAX);
        let x = s.gwell_rightmost + gap;
        // Dual Y-band: 0–500 (top) or 1000–1500 (bottom).
        let y = if lcg(&mut s.seed) < 0.5 {
            lcg_range(&mut s.seed, 0.0, 500.0)
        } else {
            lcg_range(&mut s.seed, 1000.0, 1500.0)
        };
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

// ── Turrets ───────────────────────────────────────────────────────────────────

fn spawn_turrets(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    // Turrets only appear in zone 2 (the third difficulty band).
    if zone_index_for_distance(s.distance) < 2 {
        let z2_start_x = SPAWN_X + 2.0 * ZONE_DISTANCE_STEP;
        if s.turret_rightmost < z2_start_x { s.turret_rightmost = z2_start_x; }
        return;
    }
    let mut spawned = 0usize;
    while spawned < TURRET_SPAWN_BUDGET
        && s.turret_rightmost < s.px + GEN_AHEAD
        && !s.turret_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, TURRET_GAP_MIN, TURRET_GAP_MAX);
        let x = s.turret_rightmost + gap;
        // Dual Y-band: 100–250 (top) or 1400–1850 (bottom).
        let y = if lcg(&mut s.seed) < 0.5 {
            lcg_range(&mut s.seed, 100.0, 250.0)
        } else {
            lcg_range(&mut s.seed, 1400.0, 1850.0)
        };
        let Some(id) = s.turret_free.pop() else { break; };
        s.turret_live.push(id.clone());
        s.turret_rightmost = x;
        s.turret_timers.push((id.clone(), TURRET_SHOOT_INTERVAL));
        spawned += 1;
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            let half = TURRET_FULL_SIZE * 0.5;
            obj.position = (x - half, y - half);
            obj.size = (TURRET_FULL_SIZE, TURRET_FULL_SIZE);
            obj.visible = true;
        }

        s = st.lock().unwrap();
    }
}

// ── Rocket pads ───────────────────────────────────────────────────────────────
// Very rare: only spawn one if the RNG roll passes ROCKET_PAD_SPAWN_CHANCE,
// so they feel special. Rocket pads do NOT spawn while inside space mode.

fn spawn_rocket_pads(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.in_space_mode { return; }

    let mut spawned = 0usize;
    while spawned < 1
        && s.rocket_pad_rightmost < s.px + GEN_AHEAD
        && !s.rocket_pad_free.is_empty()
    {
        let gap = lcg_range(&mut s.seed, ROCKET_PAD_GAP_MIN, ROCKET_PAD_GAP_MAX);
        let x = s.rocket_pad_rightmost + gap;
        let raw_y = lcg_range(&mut s.seed, VH * 0.42, VH - ROCKET_PAD_H - 60.0);
        let y = if s.gravity_dir < 0.0 { VH - raw_y - ROCKET_PAD_H } else { raw_y };
        // Advance the rightmost regardless of spawn so the window keeps moving
        s.rocket_pad_rightmost = x;

        if lcg(&mut s.seed) < ROCKET_PAD_SPAWN_CHANCE {
            let Some(id) = s.rocket_pad_free.pop() else { break; };
            s.rocket_pad_live.push(id.clone());
            spawned += 1;
            drop(s);

            if let Some(obj) = c.get_game_object_mut(&id) {
                obj.position = (x, y);
                obj.visible = true;
            }

            s = st.lock().unwrap();
        }
    }
}

fn spawn_main_asteroids(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.in_space_mode { return; }

    let mut spawned = 0usize;
    while spawned < SPACE_ASTEROID_SPAWN_BUDGET
        && !s.space_asteroid_free.is_empty()
        && s.space_asteroid_rightmost < s.px + GEN_AHEAD
    {
        let gap = lcg_range(&mut s.seed, SPACE_ASTEROID_GAP_MIN, SPACE_ASTEROID_GAP_MAX);
        let x = s.space_asteroid_rightmost + gap;
        let size = lcg_range(&mut s.seed, SPACE_ASTEROID_SIZE_MIN, SPACE_ASTEROID_SIZE_MAX);

        // Blend Y range by size: small → near hook zone, large → high up (visible zoomed out).
        let size_t = (size - SPACE_ASTEROID_SIZE_MIN)
            / (SPACE_ASTEROID_SIZE_MAX - SPACE_ASTEROID_SIZE_MIN);
        let y_min = SPACE_ASTEROID_Y_NEAR_MIN
            + (SPACE_ASTEROID_Y_FAR_MIN - SPACE_ASTEROID_Y_NEAR_MIN) * size_t;
        let y_max = SPACE_ASTEROID_Y_NEAR_MAX
            + (SPACE_ASTEROID_Y_FAR_MAX - SPACE_ASTEROID_Y_NEAR_MAX) * size_t;
        let y = lcg_range(&mut s.seed, y_min, y_max);

        // Slightly stronger spin than before so rotation reads clearly in play.
        let spin_mag = lcg_range(&mut s.seed, 0.45, 0.95);
        let spin = if lcg(&mut s.seed) < 0.5 { -spin_mag } else { spin_mag };

        let Some(id) = s.space_asteroid_free.pop() else { break; };
        s.space_asteroid_live.push(id.clone());
        s.space_asteroid_rightmost = x;
        spawned += 1;
        let drift = lcg_range(&mut s.seed, 1.2, 3.5);
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position = (x - size * 0.5, y - size * 0.5);
            obj.size = (size, size);
            obj.visible = true;
            obj.rotation_momentum = spin;
            obj.momentum = (-drift, 0.0);
            obj.gravity = 0.0;
            // Dynamic asteroid mass: small asteroids are lighter, large are heavier.
            // Wide range so large asteroids feel notably heavier than small ones.
            let density = 0.3 + size_t * 6.0;
            obj.material.density = density;
            // Tighter circle collider to closely match the visible sprite shape.
            let hit_r = size * 0.38;
            obj.collision_mode = CollisionMode::Solid(CollisionShape::Circle { radius: hit_r });
            obj.is_platform = false;
            obj.collision_layer = ASTEROID_COLLISION_LAYER;
            obj.collision_mask = ASTEROID_COLLISION_LAYER;
            obj.update_image_shape();
            if let Some(a) = obj.animated_sprite.as_mut() {
                let frames = a.frame_count().max(1);
                let phase = ((x * 0.003).abs() as usize) % frames;
                a.set_frame(phase);
            }
        }

        s = st.lock().unwrap();
    }
}

