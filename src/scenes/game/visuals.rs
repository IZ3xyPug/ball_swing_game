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
) {
    tick_glow_flashes(c, st);
    tick_nearest_hook_highlight(c, st, prev_nearest_hook);
    tick_zone_palette(c, st, prev_zone_idx);
    tick_dark_mode(c, st, dark_mode_prev);
    tick_spinner_movers(c, st, frame_counter);
    tick_pad_movers(c, st, frame_counter);
    tick_zoom(c, st);
}

// ── Glow flash decay ────────────────────────────────────────────────────────

fn tick_glow_flashes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
            if obj.tags.iter().any(|t| t == "pad") {
                let (r, g, b) = pad_for_zone(zone_idx);
                let corner_r = pad_corner_radius();
                obj.set_image(Image {
                    shape: ShapeType::RoundedRectangle(0.0, (PAD_W, PAD_H), 0.0, corner_r),
                    image: pad_cached(PAD_W as u32, PAD_H as u32, r, g, b),
                    color: None,
                });
            } else if obj.tags.iter().any(|t| t == "hook") {
                if asteroid_mode {
                    obj.set_image(hook_asteroid_img());
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
                    obj.set_image(hook_asteroid_img());
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
                    obj.set_image(hook_asteroid_img());
                } else {
                    let (r, g, b) = hook_near_for_zone(zone_idx);
                    obj.set_image(hook_img(r, g, b));
                }
                obj.set_glow(GlowConfig { color: Color(255, 230, 140, 190), width: 13.0 });
            }
        }
        *prev_nearest = nearest;
    }
}

// ── Zone-palette recolouring ────────────────────────────────────────────────

fn tick_zone_palette(c: &mut Canvas, st: &Arc<Mutex<State>>, prev_zone: &mut usize) {
    let s = st.lock().unwrap();
    let zone_idx = zone_index_for_distance(s.distance);
    if zone_idx == *prev_zone { return; }
    let hooks = s.live_hooks.clone();
    let pads = s.pad_live.clone();
    let spinners = s.spinner_live.clone();
    drop(s);

    *prev_zone = zone_idx;

    let asteroid_mode = matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
    for hid in &hooks {
        if let Some(obj) = c.get_game_object_mut(hid) {
            if asteroid_mode {
                obj.set_image(hook_asteroid_img());
            } else {
                let (r, g, b) = hook_base_for_zone(zone_idx);
                obj.set_image(hook_img(r, g, b));
            }
        }
    }
    for pid in &pads {
        if let Some(obj) = c.get_game_object_mut(pid) {
            let (r, g, b) = pad_for_zone(zone_idx);
            let corner_r = pad_corner_radius();
            obj.set_image(Image {
                shape: ShapeType::RoundedRectangle(0.0, (PAD_W, PAD_H), 0.0, corner_r),
                image: pad_cached(PAD_W as u32, PAD_H as u32, r, g, b),
                color: None,
            });
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
    drop(s);

    for (id, origin_y, amp, speed, phase) in &origins {
        if *amp == 0.0 { continue; }
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
    drop(s);

    for (id, origin_x, amp, speed, phase) in &origins {
        if *amp == 0.0 { continue; }
        // speed is stored as a small angular rate (radians per ~60 frames).
        // Divide frame by 60 to get a smooth time base so sin() changes gradually.
        let t = *phase + *speed * (frame as f32 / 60.0);
        let offset = amp * t.sin();
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.position.0 = origin_x + offset;
        }
    }
}

// ── Zoom (Dune-style: zoom out when player goes high) ───────────────────────

fn tick_zoom(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let s = st.lock().unwrap();
    // Space mode and rocket ascent manage their own camera/zoom — don't interfere.
    if s.in_space_mode || s.space_launch_active { return; }
    let flipped = s.gravity_dir < 0.0;
    let anchor_y = if flipped { 0.0 } else { VH };

    let target_zoom = if flipped {
        let effective_y = s.py + s.vy.max(0.0) * ZOOM_LOOKAHEAD_T;
        ((VH - ZOOM_TOP_MARGIN) / effective_y.abs().max(1.0)).clamp(1.0 / ZOOM_MAX, 1.0)
    } else {
        let effective_y = s.py + s.vy.min(0.0) * ZOOM_LOOKAHEAD_T;
        ((VH - ZOOM_TOP_MARGIN) / (VH - effective_y).abs().max(1.0)).clamp(1.0 / ZOOM_MAX, 1.0)
    };

    let px = s.px;
    drop(s);

    if let Some(cam) = c.camera_mut() {
        cam.zoom_lerp_speed = if target_zoom < cam.zoom { ZOOM_OUT_LERP } else { ZOOM_IN_LERP };
        cam.zoom_anchor = Some((px, anchor_y));
        cam.smooth_zoom(target_zoom);
    }
}
