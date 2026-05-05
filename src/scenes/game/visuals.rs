use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::zone_index_for_distance;
use crate::images::*;
use crate::state::*;
use super::helpers::*;

/// Tick visual effects: glow flashes, nearest-hook highlight, zone palette
/// transitions, dark-mode fade, mover animations, zoom.
pub fn tick_visuals(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    prev_zone_idx: &mut usize,
    prev_nearest_hook: &mut String,
    dark_mode_prev: &mut bool,
    frame_counter: u32,
    tech_bounce_img: &Image,
    tech_bounce_anim_frames: &[Image],
    tech_bounce_img_flipped: &Image,
    tech_bounce_anim_frames_flipped: &[Image],
) {
    tick_pad_impact_animation(c, st, tech_bounce_img, tech_bounce_anim_frames, tech_bounce_img_flipped, tech_bounce_anim_frames_flipped);
    tick_glow_flashes(c, st, tech_bounce_img);
    tick_nearest_hook_highlight(c, st, prev_nearest_hook);
    tick_zone_palette(c, st, prev_zone_idx, tech_bounce_img, tech_bounce_img_flipped);
    tick_dark_mode(c, st, dark_mode_prev);
    tick_spinner_movers(c, st, frame_counter);
    tick_pad_movers(c, st, frame_counter);
    tick_pad_thrusters(c, st);
    tick_zoom(c, st);
    tick_debug_radii(c, st);
    tick_player_ball_animation(c, st);
}

fn tick_pad_impact_animation(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    tech_bounce_img: &Image,
    tech_bounce_anim_frames: &[Image],
    tech_bounce_img_flipped: &Image,
    tech_bounce_anim_frames_flipped: &[Image],
) {
    let gravity_dir = st.lock().unwrap().gravity_dir;
    let (frames, base_img) = if gravity_dir < 0.0 {
        (tech_bounce_anim_frames_flipped, tech_bounce_img_flipped)
    } else {
        (tech_bounce_anim_frames, tech_bounce_img)
    };
    if frames.is_empty() {
        let mut s = st.lock().unwrap();
        s.pad_bounce_anim.clear();
        return;
    }

    let frame_count = frames.len();
    if frame_count <= 1 {
        let mut s = st.lock().unwrap();
        s.pad_bounce_anim.clear();
        return;
    }

    let ticks_per_frame = (60.0 / TECH_BOUNCE_FPS.max(1.0)).round().max(1.0) as u32;
    let mut s = st.lock().unwrap();
    let mut keep: Vec<(String, usize, u32)> = Vec::with_capacity(s.pad_bounce_anim.len());
    let active = std::mem::take(&mut s.pad_bounce_anim);
    drop(s);

    for (name, mut frame_idx, mut ticks_left) in active {
        let mut finished = false;
        if let Some(obj) = c.get_game_object_mut(&name) {
            let idx = frame_idx.min(frame_count - 1);
            obj.animated_sprite = None;
            obj.set_image(frames[idx].clone());
        } else {
            finished = true;
        }

        if !finished {
            if ticks_left > 0 {
                ticks_left -= 1;
            }
            if ticks_left == 0 {
                frame_idx += 1;
                ticks_left = ticks_per_frame;
            }
            if frame_idx >= frame_count {
                if let Some(obj) = c.get_game_object_mut(&name) {
                    obj.animated_sprite = None;
                    obj.set_image(base_img.clone());
                }
            } else {
                keep.push((name, frame_idx, ticks_left));
            }
        }
    }

    let mut s = st.lock().unwrap();
    s.pad_bounce_anim = keep;
}

// ── Glow flash decay ────────────────────────────────────────────────────────

fn tick_glow_flashes(c: &mut Canvas, st: &Arc<Mutex<State>>, _tech_bounce_img: &Image) {
    let mut s = st.lock().unwrap();
    let zone_idx = zone_index_for_distance(s.distance);
    let mut expired: Vec<String> = Vec::new();
    for (name, timer) in s.glow_flashes.iter_mut() {
        if *timer > 0 {
            *timer -= 1;
            if *timer == 0 { expired.push(name.clone()); }
        }
    }
    s.glow_flashes.retain(|(_, t)| *t > 0);
    drop(s);

    let asteroid_mode = matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
    for name in &expired {
        if let Some(obj) = c.get_game_object_mut(name) {
            if obj.tags.iter().any(|t| t == "hook") {
                if asteroid_mode {
                    obj.set_image(hook_asteroid_img_for_id(name, AsteroidHookState::Base));
                } else {
                    let (r, g, b) = hook_base_for_zone(zone_idx);
                    obj.set_image(hook_img(r, g, b));
                }
            }
            obj.clear_glow();
        }
    }
}

// ── Nearest hook highlight ──────────────────────────────────────────────────

fn tick_nearest_hook_highlight(c: &mut Canvas, st: &Arc<Mutex<State>>, prev_nearest: &mut String) {
    let s = st.lock().unwrap();
    if s.hooked { return; }
    let px = s.px;
    let py = s.py;
    let zone_idx = zone_index_for_distance(s.distance);
    let hooks = s.live_hooks.clone();
    drop(s);

    let max_r2 = ROPE_LEN_MAX * ROPE_LEN_MAX;
    let mut best_id: Option<String> = None;
    let mut best_dist = f32::INFINITY;
    let max_dist2 = ROPE_LEN_MAX * ROPE_LEN_MAX;
    for hid in &hooks {
        if let Some(obj) = c.get_game_object(hid) {
            let hcx = obj.position.0 + HOOK_R;
            let hcy = obj.position.1 + HOOK_R;
            let dx = px - hcx;
            let dy = py - hcy;
            let d2 = dx * dx + dy * dy;
            if d2 <= max_dist2 && d2 < best_dist {
                best_dist = d2;
                best_id = Some(hid.clone());
            }
        }
    }

    let nearest = best_id.unwrap_or_default();
    if nearest != *prev_nearest {
        let asteroid_mode = matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
        // Remove glow from old nearest.
        if !prev_nearest.is_empty() {
            if let Some(obj) = c.get_game_object_mut(prev_nearest) {
                if asteroid_mode {
                    obj.set_image(hook_asteroid_img_for_id(prev_nearest, AsteroidHookState::Base));
                } else {
                    let (r, g, b) = hook_base_for_zone(zone_idx);
                    obj.set_image(hook_img(r, g, b));
                }
                obj.clear_glow();
            }
        }
        // Glow new nearest.
        if !nearest.is_empty() {
            if let Some(obj) = c.get_game_object_mut(&nearest) {
                if asteroid_mode {
                    obj.set_image(hook_asteroid_img_for_id(&nearest, AsteroidHookState::Near));
                    obj.clear_glow();
                } else {
                    let (r, g, b) = hook_near_for_zone(zone_idx);
                    obj.set_image(hook_img(r, g, b));
                    obj.set_glow(GlowConfig { color: Color(255, 230, 140, 190), width: 13.0 });
                }
            }
        }
        *prev_nearest = nearest;
    }
}

// ── Zone-palette recolouring ────────────────────────────────────────────────

fn tick_zone_palette(c: &mut Canvas, st: &Arc<Mutex<State>>, prev_zone: &mut usize, tech_bounce_img: &Image, tech_bounce_img_flipped: &Image) {
    let s = st.lock().unwrap();
    let zone_idx = zone_index_for_distance(s.distance);
    let gravity_dir = s.gravity_dir;
    if zone_idx == *prev_zone { return; }
    let hooks = s.live_hooks.clone();
    let pads = s.pad_live.clone();
    let spinners = s.spinner_live.clone();
    drop(s);

    *prev_zone = zone_idx;

    let pad_img = if gravity_dir < 0.0 { tech_bounce_img_flipped } else { tech_bounce_img };
    let asteroid_mode = matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
    for hid in &hooks {
        if let Some(obj) = c.get_game_object_mut(hid) {
            if asteroid_mode {
                obj.set_image(hook_asteroid_img_for_id(hid, AsteroidHookState::Base));
                obj.clear_glow();
            } else {
                let (r, g, b) = hook_base_for_zone(zone_idx);
                obj.set_image(hook_img(r, g, b));
            }
        }
    }
    for pid in &pads {
        if let Some(obj) = c.get_game_object_mut(pid) {
            obj.animated_sprite = None;
            obj.set_image(pad_img.clone());
        }
    }
    for sid in &spinners {
        if let Some(obj) = c.get_game_object_mut(sid) {
            let (r, g, b) = spinner_for_zone(zone_idx);
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (SPINNER_W, SPINNER_H), 0.0),
                image: spinner_cached(SPINNER_W as u32, SPINNER_H as u32, r, g, b),
                color: None,
            });
        }
    }
}

// ── Dark-mode fade (zone 3+) ────────────────────────────────────────────────

fn tick_dark_mode(c: &mut Canvas, _st: &Arc<Mutex<State>>, prev_dark: &mut bool) {
    // Dark mode is driven by zone-palette already. If a dedicated
    // overlay is needed later, hook it here. For now, no-op placeholder.
    let _ = (c, prev_dark);
}

// ── Spinner vertical movers ─────────────────────────────────────────────────

fn tick_spinner_movers(c: &mut Canvas, st: &Arc<Mutex<State>>, frame: u32) {
    let s = st.lock().unwrap();
    let origins = s.spinner_origins.clone();
    // Collect IDs of objects still in a spawn animation (dormant or mid-drop).
    let animating: std::collections::HashSet<String> = s.spawn_animations.iter()
        .map(|a| a.id.clone())
        .collect();
    drop(s);

    for (id, origin_y, amp, speed, phase) in &origins {
        if *amp == 0.0 { continue; }
        if animating.contains(id) { continue; } // don't fight spawn anim
        let t = *phase + *speed * (frame as f32 / 60.0);
        let offset = amp * t.sin();
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.position.1 = origin_y + offset;
        }
    }
}

// ── Pad horizontal movers ───────────────────────────────────────────────────

fn tick_pad_movers(c: &mut Canvas, st: &Arc<Mutex<State>>, frame: u32) {
    let s = st.lock().unwrap();
    let origins = s.pad_origins.clone();
    // Collect IDs of objects still in a spawn animation (dormant or mid-drop).
    let animating: std::collections::HashSet<String> = s.spawn_animations.iter()
        .map(|a| a.id.clone())
        .collect();
    drop(s);

    for (id, origin_x, amp, speed, phase) in &origins {
        if *amp == 0.0 { continue; }
        if animating.contains(id) { continue; } // don't fight spawn anim
        // speed is stored as a small angular rate (radians per ~60 frames).
        // Divide frame by 60 to get a smooth time base so sin() changes gradually.
        let t = *phase + *speed * (frame as f32 / 60.0);
        let offset = amp * t.sin();
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.position.0 = origin_x + offset;
        }
    }
}

fn tick_pad_thrusters(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (pad_ids, flipped) = {
        let s = st.lock().unwrap();
        (s.pad_live.clone(), s.gravity_dir < 0.0)
    };

    let thruster_embed = PAD_THRUSTER_HIDE_TOP + PAD_THRUSTER_RAISE_Y;

    for pad_id in &pad_ids {
        let Some((px, py, vis, layer)) = c
            .get_game_object(pad_id)
            .map(|pad| (pad.position.0, pad.position.1, pad.visible, pad.layer))
        else {
            continue;
        };

        let thr_id = pad_thruster_id(pad_id);
        if let Some(thr) = c.get_game_object_mut(&thr_id) {
            thr.position.0 = px + (PAD_W - PAD_THRUSTER_W) * 0.5;
            thr.position.1 = if flipped {
                py - PAD_THRUSTER_H + thruster_embed
            } else {
                py + PAD_H - thruster_embed
            };
            thr.rotation = 0.0;
            thr.layer = layer - 1;
            thr.visible = vis;
        }
    }
}

// ── Zoom (Dune-style: zoom out when player goes high) ───────────────────────

fn tick_zoom(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let intro_zoom_recover = c.get_i32("start_zoom_recover_ticks").max(0);
    if intro_zoom_recover > 0 {
        if let Some(cam) = c.camera_mut() {
            cam.zoom_lerp_speed = 0.02;
            cam.zoom_anchor = None;
            cam.follow(Some(Target::name("player")));
            cam.smooth_zoom(1.0);
        }
        c.set_var("start_zoom_recover_ticks", intro_zoom_recover - 1);
        return;
    }

    let s = st.lock().unwrap();
    // Space mode and rocket ascent manage their own camera/zoom — don't interfere.
    if s.in_space_mode || s.space_launch_active { return; }

    let pending_space_exit_reset = matches!(
        c.get_var("space_exit_zoom_reset"),
        Some(Value::Bool(true))
    );

    let flipped = s.gravity_dir < 0.0;
    let target_anchor_y: f32 = if flipped { 0.0 } else { VH };
    let cur_anchor_y = match c.get_var("zoom_anchor_y") {
        Some(Value::F32(v)) => v,
        _ => target_anchor_y,
    };
    let new_anchor_y = cur_anchor_y + (target_anchor_y - cur_anchor_y) * 0.06;
    c.set_var("zoom_anchor_y", new_anchor_y);
    let anchor_y = new_anchor_y;

    let target_zoom = if flipped {
        let effective_y = s.py + s.vy.max(0.0) * ZOOM_LOOKAHEAD_T;
        ((VH - ZOOM_TOP_MARGIN) / effective_y.abs().max(1.0)).clamp(1.0 / ZOOM_MAX, 1.0)
    } else {
        let effective_y = s.py + s.vy.min(0.0) * ZOOM_LOOKAHEAD_T;
        ((VH - ZOOM_TOP_MARGIN) / (VH - effective_y).abs().max(1.0)).clamp(1.0 / ZOOM_MAX, 1.0)
    };

    let px = s.px;
    drop(s);

    if pending_space_exit_reset {
        c.set_var("zoom_anchor_y", target_anchor_y);
        if let Some(cam) = c.camera_mut() {
            cam.follow(Some(Target::name("player")));
            cam.zoom_lerp_speed = ZOOM_OUT_LERP;
            cam.zoom_anchor = Some((px, target_anchor_y));
            cam.snap_zoom(target_zoom);
            // Keep the post-space handoff from inheriting stale negative-space Y.
            cam.position.1 = if flipped {
                (VH - VH / cam.zoom).max(0.0)
            } else {
                0.0
            };
        }
        c.set_var("space_exit_zoom_reset", false);
        return;
    }

    if let Some(cam) = c.camera_mut() {
        cam.zoom_lerp_speed = if target_zoom < cam.zoom { ZOOM_OUT_LERP } else { ZOOM_IN_LERP };
        cam.zoom_anchor = Some((px, anchor_y));
        cam.smooth_zoom(target_zoom);
    }
}

// ── Debug radius visualizer (hold X = 1× radius, hold C = 2× radius) ───────

fn tick_debug_radii(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Clear previous frame's debug rings.
    let prev_count = match c.get_var("debug_ring_count") {
        Some(Value::I32(v)) => v.max(0) as usize,
        _ => 0,
    };
    for i in 0..prev_count {
        c.remove_game_object(&format!("dbg_ring_{i}"));
    }
    c.set_var("debug_ring_count", 0i32);

    let show_x = c.key("x");
    let show_c = c.key("c");
    if !show_x && !show_c { return; }
    let multiplier: f32 = if show_c { 2.0 } else { 1.0 };

    let (spinner_names, gwell_names) = {
        let s = st.lock().unwrap();
        (s.spinner_live.clone(), s.gwell_live.clone())
    };

    let mut idx = 0usize;

    // Spinner rings — yellow, radius = SPINNER_W / 2.
    let spinner_data: Vec<(f32, f32)> = spinner_names.iter().filter_map(|name| {
        c.get_game_object(name).map(|o| (
            o.position.0 + SPINNER_W * 0.5,
            o.position.1 + SPINNER_H * 0.5,
        ))
    }).collect();

    for (cx, cy) in spinner_data {
        let r = (SPINNER_W * 0.5 * multiplier) as u32;
        let rf = r as f32;
        let img = ring_outline_img(r, 255, 230, 0);
        let mut go = GameObject::build(format!("dbg_ring_{idx}"))
            .position(cx - rf, cy - rf)
            .size(rf * 2.0, rf * 2.0)
            .layer(9990)
            .finish();
        go.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (rf * 2.0, rf * 2.0), 0.0),
            image: Arc::new(img),
            color: None,
        });
        c.add_game_object(format!("dbg_ring_{idx}"), go);
        idx += 1;
    }

    // GWell rings — cyan, radius = planet_radius.
    let gwell_data: Vec<(f32, f32, f32)> = gwell_names.iter().filter_map(|name| {
        c.get_game_object(name).map(|o| (
            o.position.0 + o.size.0 * 0.5,
            o.position.1 + o.size.1 * 0.5,
            o.planet_radius.unwrap_or(GWELL_RADIUS_MIN),
        ))
    }).collect();

    for (cx, cy, planet_r) in gwell_data {
        let r = (planet_r * multiplier) as u32;
        let rf = r as f32;
        let img = ring_outline_img(r, 0, 200, 255);
        let mut go = GameObject::build(format!("dbg_ring_{idx}"))
            .position(cx - rf, cy - rf)
            .size(rf * 2.0, rf * 2.0)
            .layer(9990)
            .finish();
        go.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (rf * 2.0, rf * 2.0), 0.0),
            image: Arc::new(img),
            color: None,
        });
        c.add_game_object(format!("dbg_ring_{idx}"), go);
        idx += 1;
    }

    c.set_var("debug_ring_count", idx as i32);
}

fn tick_player_ball_animation(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let frame_count = c.get_game_object("player")
        .and_then(|p| p.animated_sprite.as_ref())
        .map(|a| a.frame_count())
        .unwrap_or(1);
    if frame_count <= 1 { return; }
    let last_frame = frame_count - 1;

    let (vy, gravity_dir, hooked, in_space) = {
        let s = st.lock().unwrap();
        (s.vy, s.gravity_dir, s.hooked, s.in_space_mode)
    };

    let mut s = st.lock().unwrap();
    s.player_ball_frame_timer = s.player_ball_frame_timer.saturating_add(1);

    const FRAME_STEP_TICKS: u32 = 3;
    let step_this_tick = s.player_ball_frame_timer % FRAME_STEP_TICKS == 0;

    if step_this_tick {
        if hooked {
            // Hooked to a node: fast-rewind to frame 0 and stay there.
            s.player_ball_hit_rewind = false;
            if s.player_ball_frame > 0 {
                s.player_ball_frame = s.player_ball_frame.saturating_sub(2);
            }
        } else if s.player_ball_hit_rewind {
            // Pad-bounce fast rewind.
            if s.player_ball_frame == 0 {
                s.player_ball_hit_rewind = false;
            } else {
                s.player_ball_frame = s.player_ball_frame.saturating_sub(2);
                if s.player_ball_frame == 0 {
                    s.player_ball_hit_rewind = false;
                }
            }
        } else {
            // Free flight: in space always advance; normal zone advance only while rising.
            let should_advance = in_space || (vy * gravity_dir < 0.0);
            if should_advance {
                if s.player_ball_frame < last_frame { s.player_ball_frame += 1; }
            } else {
                if s.player_ball_frame > 0 { s.player_ball_frame -= 1; }
            }
        }
    }

    let target_frame = s.player_ball_frame;
    drop(s);

    if let Some(player) = c.get_game_object_mut("player") {
        if let Some(anim) = player.animated_sprite.as_mut() {
            anim.set_frame(target_frame);
        }
    }
}
