// ── scenes/game/space_zone.rs ─────────────────────────────────────────────────
// Space zone: camera control, oxygen, planet gravity, spawning/culling,
// entry/exit transitions, welcome text, and coin collection for the space zone.
//
// Camera strategy
// ────────────────
// The world is `VH` tall with the normal game in y = 0..VH. Space objects are
// at negative y (above the screen). The Quartz camera clamps `position.1 >= 0`
// inside `lerp_toward`, so we disable that by setting `cam.zoom_anchor = Some(…)`.
// This file then manually lerps `cam.position.1` each tick to follow the player
// into negative-y territory. On space exit the anchor is cleared and normal
// follow behaviour resumes.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::images::*;
use crate::objects::*;
use crate::state::*;
use super::helpers::*;

// ── Public entry point ────────────────────────────────────────────────────────

/// Called every tick from the main on_update closure.
pub fn tick_space_zone(c: &mut Canvas, st: &Arc<Mutex<State>>, frame: u32) {
    let in_space = st.lock().unwrap().in_space_mode;

    // ── Pre-entry: player was launched by a rocket pad ────────────────────────
    if !in_space {
        let (py, vy, launch_active) = {
            let s = st.lock().unwrap();
            (s.py, s.vy, s.space_launch_active)
        };

        if launch_active {
            // If player reversed (fell back below screen top), cancel the launch
            if vy > 8.0 && py > VH * 0.5 {
                st.lock().unwrap().space_launch_active = false;
                if let Some(cam) = c.camera_mut() { cam.zoom_anchor = None; }
                return;
            }
            // Allow camera to track above y=0 (normally clamped by lerp_toward)
            if let Some(cam) = c.camera_mut() {
                if cam.zoom_anchor.is_none() {
                    cam.zoom_anchor = Some((0.0, 0.0));
                }
            }
            // Scene change: player has risen far enough above the normal game world
            if py < SPACE_ENTRY_Y {
                enter_space(c, st);
            }
        }
        return;
    }

    // ── Inside space zone ────────────────────────────────────────────────────
    // No rotational drag: swing is perfectly elastic in space.
    // The hook-grab event always attaches with damping=0.001; zero it each tick.
    if st.lock().unwrap().hooked {
        if let Some(g) = c.get_grapple_mut("player") { g.damping = 0.0; }
    }

    tick_space_camera(c, st);
    tick_space_oxygen(c, st);
    tick_space_spawning(c, st, frame);
    tick_space_culling(c, st);
    tick_space_coin_collect(c, st);
    tick_space_welcome_text(c, st);
    tick_space_planet_pulse(c, st, frame);
    tick_space_settle(st);

    // Check return threshold: player drifted back below SPACE_RETURN_Y
    let py = st.lock().unwrap().py;
    if py > SPACE_RETURN_Y {
        exit_space(c, st, false);
    }
}

// ── Entry / Exit ─────────────────────────────────────────────────────────────

// ── Momentum settle ─────────────────────────────────────────────────────
// Fires exactly once per space visit. When the player reaches SPACE_SETTLE_Y
// (the depth where the catch planet lives) their momentum is zeroed so they
// float into the gravity well cleanly instead of rocketing through it.
fn tick_space_settle(st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.space_settle_done { return; }
    if s.py <= SPACE_SETTLE_Y {
        s.vx = 0.0;
        s.vy = 0.0;
        s.space_settle_done = true;
    }
}

fn enter_space(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let mut s = st.lock().unwrap();
        s.in_space_mode       = true;
        s.space_launch_active  = false; // consumed — cannot re-enter without another pad
        s.space_settle_done    = false; // arm the momentum-zero trigger for this visit
        s.space_oxygen        = SPACE_OXYGEN_TICKS;
        s.space_welcome_ticks = SPACE_WELCOME_TICKS;
        s.space_return_delay  = 0;

        // Seed space object rightmosts from current player x
        let px = s.px;
        s.space_planet_rightmost    = px - VW * 0.5;
        s.space_hook_rightmost      = px - VW * 0.5;
        s.space_coin_rightmost      = px - VW * 0.5;
        s.space_blackhole_rightmost = px - VW * 2.0;

        // Freeze background scale for parallax starfield effect
        s.space_entry_bg_scale = 1.0; // will be refined below after drop

        // Set near-zero gravity
        drop(s);
    }

    // Freeze the current bg scale and apply space gravity to player
    let bg_scale = c.get_var("bg_scale_current")
        .and_then(|v| if let Value::F32(f) = v { Some(f) } else if let Value::F64(f) = v { Some(f as f32) } else { None })
        .unwrap_or(1.0);
    {
        let mut s = st.lock().unwrap();
        s.space_entry_bg_scale = bg_scale;
        let gdir = s.gravity_dir;
        let hooked = s.hooked;
        drop(s);
        if !hooked {
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.gravity = GRAVITY * SPACE_GRAVITY_SCALE * gdir;
            }
        }
    }

    // Guarantee a planet at entry so the player has an immediate gravity anchor.
    spawn_catch_planet(c, st);

    // Camera: disable auto Y lerp by setting a dummy zoom_anchor
    if let Some(cam) = c.camera_mut() {
        // Store current camera Y in space state (will be updated each tick)
        cam.zoom_anchor   = Some((0.0, 0.0)); // disables Y lerp in lerp_toward
        cam.lerp_speed    = SPACE_CAM_LERP_IN;
        cam.smooth_zoom(SPACE_CAM_ZOOM_IN);
        cam.zoom_lerp_speed = 0.04;
    }

    // Store initial camera Y
    let cam_y = c.camera().map(|cam| cam.position.1).unwrap_or(0.0);
    st.lock().unwrap().space_cam_y = cam_y;

    // Entry flash: screen goes fully white IMMEDIATELY (FadeOut starts at peak).
    // The scene spawns while the screen is opaque; flash fades to reveal space.
    if let Some(cam) = c.camera_mut() {
        cam.flash_with(
            Color(220, 240, 255, 255), // full white-blue, immediately opaque
            1.6,                        // total fade duration after hold
            FlashMode::FadeOut,         // peak brightness on tick 0
            FlashEase::Smooth,
            1.0,
            0.35,                       // hold full brightness 0.35s — covers spawn lag
        );
    }

    // Show welcome text
    if let Some(obj) = c.get_game_object_mut("space_welcome_text") {
        obj.visible = true;
    }

    // Show oxygen bar, hide distance bar
    if let Some(obj) = c.get_game_object_mut("dist_bar") {
        obj.visible = false;
    }
    if let Some(obj) = c.get_game_object_mut("oxygen_bar") {
        obj.visible = true;
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (OXYGEN_BAR_W, OXYGEN_BAR_H), 0.0),
            image: oxygen_bar_img(1.0, OXYGEN_BAR_W as u32, OXYGEN_BAR_H as u32).into(),
            color: None,
        });
    }

    // Signal background module to switch to deep-space look
    c.set_var("in_space_mode", true);
}

// ── Entry catch planet ─────────────────────────────────────────────────────────
// Spawns one planet just above the entry threshold when the player enters space.
// The engine's handle_planet_landings + gravity_well do the rest — no custom
// collision code needed.
fn spawn_catch_planet(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let Some(id) = s.space_planet_free.pop() else { return; };
    let px        = s.px;
    let color_idx = (s.seed >> 3) as usize;
    // Large planet guarantees a wide gravity catchment area.
    // The engine's gravity_influence_mult (3×) extends the field to
    // visual_r * 3 = 460*3 = 1380px from center.
    // Planet center placed at SPACE_SETTLE_Y.
    // Player at SPACE_ENTRY_Y = -(VH*0.35) ≈ -756.
    // Distance from entry to settle = (VH*1.1 - VH*0.35) = VH*0.75 ≈ 1620px.
    // 1620 > 1380: player enters the gravity well before they reach the settle
    // depth and is being pulled while the momentum zero fires at SPACE_SETTLE_Y.
    let visual_r  = SPACE_PLANET_RADIUS_LG_MAX;  // 460 — largest available
    let gravity_r = visual_r * SPACE_PLANET_GRAV_R_MULT;
    let x = px;   // directly above entry X — always above the player
    let y = SPACE_SETTLE_Y;
    s.space_planet_live.push(id.clone());
    s.space_planet_data.push((id.clone(), gravity_r, SPACE_PLANET_GRAV_STRENGTH));
    // Advance the spawner rightmost past this planet so it doesn't double-spawn here.
    if s.space_planet_rightmost < x { s.space_planet_rightmost = x; }
    // Pre-populate some hook points in a band around the catch planet so the
    // player has immediate grapple options when they arrive.
    let hook_ids: Vec<String> = (0..4).filter_map(|_| s.space_hook_free.pop()).collect();
    drop(s);

    let (pr, pg, pb) = C_SPACE_PLANET[color_idx % C_SPACE_PLANET.len()];
    let img = planet_img_cached(visual_r, gravity_r, pr, pg, pb);
    let d   = gravity_r * 2.0;
    if let Some(obj) = c.get_game_object_mut(&id) {
        obj.position      = (x - gravity_r, y - gravity_r);
        obj.size          = (d, d);
        obj.planet_radius = Some(visual_r); // engine collision + gravity at visual surface
        obj.visible       = true;
        obj.set_image(Image {
            shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
            image: img,
            color: None,
        });
        obj.set_glow(GlowConfig { color: Color(pr, pg, pb, 110), width: 28.0 });
    }

    // Place hooks evenly spaced to the left, right, above, and below the catch planet.
    // Offsets keep them just outside the visual surface so they're reachable.
    let offsets: [(f32, f32); 4] = [
        (-(visual_r + 320.0), 0.0),     // left
        ( (visual_r + 320.0), 0.0),     // right
        (0.0, -(visual_r + 320.0)),     // above
        (0.0,   visual_r + 320.0 ),     // below
    ];
    let mut s = st.lock().unwrap();
    for (hook_id, (ox, oy)) in hook_ids.into_iter().zip(offsets.iter()) {
        let hx = x + ox;
        let hy = y + oy;
        s.space_hook_live.push(hook_id.clone());
        if s.space_hook_rightmost < hx { s.space_hook_rightmost = hx; }
        drop(s);
        if let Some(obj) = c.get_game_object_mut(&hook_id) {
            obj.position = (hx - HOOK_R, hy - HOOK_R);
            obj.size     = (HOOK_R * 2.0, HOOK_R * 2.0);
            obj.visible  = true;
            obj.set_image(hook_img(C_SPACE_HOOK.0, C_SPACE_HOOK.1, C_SPACE_HOOK.2));
        }
        s = st.lock().unwrap();
    }
}

pub fn exit_space(c: &mut Canvas, st: &Arc<Mutex<State>>, forced: bool) {
    {
        let mut s = st.lock().unwrap();
        if !s.in_space_mode { return; }
        s.in_space_mode = false;
    }

    // Hide all space objects and return them to pools
    cull_all_space_objects(c, st);

    // Hide welcome text
    if let Some(obj) = c.get_game_object_mut("space_welcome_text") {
        obj.visible = false;
    }

    // Show distance bar, hide oxygen bar
    if let Some(obj) = c.get_game_object_mut("dist_bar") {
        obj.visible = true;
    }
    if let Some(obj) = c.get_game_object_mut("oxygen_bar") {
        obj.visible = false;
    }

    // Restore camera: screen is fully covered by the exit flash — teleport everything
    // back to valid world-space values instantly. tick_zoom takes over next tick.
    if let Some(cam) = c.camera_mut() {
        cam.zoom_anchor     = None;              // re-enable lerp_toward Y clamping
        cam.position.1      = 0.0;              // snap from deep-negative space Y to world top
        cam.lerp_speed      = 0.10;
        cam.zoom_lerp_speed = ZOOM_OUT_LERP;
        cam.zoom            = 1.0 / ZOOM_MAX;   // start fully zoomed out (arc-peak look)
        cam.zoom_target     = 1.0 / ZOOM_MAX;   // tick_zoom lerps back as player falls
    }

    // Restore normal swing damping
    if let Some(g) = c.get_grapple_mut("player") { g.damping = 0.001; }

    // Restore normal gravity
    {
        let s = st.lock().unwrap();
        let gdir = s.gravity_dir;
        let hooked = s.hooked;
        drop(s);
        if !hooked {
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.gravity = GRAVITY * gdir;
            }
        }
    }

    // Apply a strong downward push to send player back to normal zone
    {
        let mut s = st.lock().unwrap();
        if forced {
            s.vy += SPACE_RETURN_FORCE_VY;
        }
    }

    // Re-entry flash: immediately full red-orange while camera snaps and objects cull,
    // then fades to reveal normal zone. Player never sees the teleport.
    if let Some(cam) = c.camera_mut() {
        cam.flash_with(
            Color(255, 60, 20, 255),  // full hot red, immediately opaque
            1.8,                       // total fade duration after hold
            FlashMode::FadeOut,        // peak brightness on tick 0
            FlashEase::Smooth,
            1.0,
            0.4,                       // hold full 0.4s — covers cull lag + camera snap
        );
        cam.shake(40.0, 0.5);
    }

    c.set_var("in_space_mode", false);
    c.set_var("bg_force_refresh", true);
}

// ── Camera ────────────────────────────────────────────────────────────────────

fn tick_space_camera(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let s = st.lock().unwrap();
    let py = s.py;
    let space_cam_y = s.space_cam_y;
    drop(s);

    // Visible height at current zoom
    let zoom = c.camera().map(|cam| cam.zoom).unwrap_or(1.0).max(0.01);
    let visible_h = VH / zoom;

    // Target camera Y: center player in view, offset up by lead amount
    let target_cam_y = py - visible_h * 0.5 - SPACE_CAM_Y_LEAD;

    // Manual lerp (not clamped to 0 like the engine's lerp_toward)
    let new_cam_y = space_cam_y + (target_cam_y - space_cam_y) * SPACE_CAM_LERP_IN;

    st.lock().unwrap().space_cam_y = new_cam_y;

    if let Some(cam) = c.camera_mut() {
        cam.position.1 = new_cam_y;
    }
}

// ── Oxygen ────────────────────────────────────────────────────────────────────

fn tick_space_oxygen(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (oxygen, needs_return) = {
        let mut s = st.lock().unwrap();
        if s.space_oxygen > 0 {
            s.space_oxygen -= 1;
            (s.space_oxygen, false)
        } else {
            // Oxygen empty: tick the grace/return delay
            let needs = if s.space_return_delay == 0 {
                true
            } else {
                s.space_return_delay -= 1;
                false
            };
            (0, needs)
        }
    };

    // Update HUD bar every 6 ticks (10 times/sec) to reduce redraws
    let q_oxy = (oxygen as f32 / SPACE_OXYGEN_TICKS as f32 * 1000.0) as u32;
    let last_q = st.lock().unwrap().hud_last_oxygen;
    if q_oxy != last_q {
        st.lock().unwrap().hud_last_oxygen = q_oxy;
        let fill = oxygen as f32 / SPACE_OXYGEN_TICKS as f32;
        if let Some(obj) = c.get_game_object_mut("oxygen_bar") {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (OXYGEN_BAR_W, OXYGEN_BAR_H), 0.0),
                image: oxygen_bar_img(fill, OXYGEN_BAR_W as u32, OXYGEN_BAR_H as u32).into(),
                color: None,
            });
        }

        // Low oxygen flash warning
        if oxygen == 0 {
            if let Some(cam) = c.camera_mut() {
                cam.flash_with(Color(200, 40, 40, 80), 0.4, FlashMode::Pulse, FlashEase::Sharp, 0.7, 0.0);
            }
        }
    }

    if needs_return {
        exit_space(c, st, true);
    }
}

// ── Welcome text ──────────────────────────────────────────────────────────────

fn tick_space_welcome_text(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let ticks = {
        let mut s = st.lock().unwrap();
        if s.space_welcome_ticks > 0 {
            s.space_welcome_ticks -= 1;
        }
        s.space_welcome_ticks
    };

    if ticks == 0 {
        if let Some(obj) = c.get_game_object_mut("space_welcome_text") {
            obj.visible = false;
        }
    }
}

// ── Planet pulse ──────────────────────────────────────────────────────────────

fn tick_space_planet_pulse(c: &mut Canvas, st: &Arc<Mutex<State>>, frame: u32) {
    let s = st.lock().unwrap();
    let planet_data = s.space_planet_data.clone();
    let bh_data     = s.space_blackhole_data.clone();
    drop(s);

    for (id, _gravity_r, _strength) in &planet_data {
        if let Some(obj) = c.get_game_object_mut(id) {
            let t = frame as f32 * 0.018;
            let pulse = 0.82 + 0.18 * ((t.sin() + 1.0) * 0.5);
            // Animate glow width; safe to just re-set glow with updated params.
            let pr = C_SPACE_PLANET[0].0;
            let pg = C_SPACE_PLANET[0].1;
            let pb = C_SPACE_PLANET[0].2;
            let w = 18.0 * pulse;
            let a = (55.0 * pulse) as u8;
            obj.set_glow(GlowConfig { color: Color(pr, pg, pb, a), width: w });
        }
    }

    // Black holes: subtle swirl — slightly rotate their position (visual-only spin)
    for (id, _gravity_r, _strength) in &bh_data {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.rotation = (obj.rotation + 0.12).rem_euclid(360.0);
        }
    }
}

// ── Space object spawning ─────────────────────────────────────────────────────

fn tick_space_spawning(c: &mut Canvas, st: &Arc<Mutex<State>>, _frame: u32) {
    spawn_space_hooks(c, st);
    spawn_space_planets(c, st);
    spawn_space_coins(c, st);
    spawn_space_blackholes(c, st);
}

fn spawn_space_hooks(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < SPACE_HOOK_SPAWN_BUDGET
        && !s.space_hook_free.is_empty()
        && s.space_hook_rightmost < s.px + GEN_AHEAD
    {
        let gap = lcg_range(&mut s.seed, SPACE_HOOK_GAP_MIN, SPACE_HOOK_GAP_MAX);
        let x   = s.space_hook_rightmost + gap;
        // Distribute across 3 vertical bands so hooks exist everywhere in space.
        let y = match (lcg(&mut s.seed) * 3.0) as u32 {
            0 => lcg_range(&mut s.seed, SPACE_HOOK_Y_SHALLOW_MIN, SPACE_HOOK_Y_SHALLOW_MAX),
            1 => lcg_range(&mut s.seed, SPACE_HOOK_Y_MID_MIN,     SPACE_HOOK_Y_MID_MAX),
            _ => lcg_range(&mut s.seed, SPACE_HOOK_Y_DEEP_MIN,    SPACE_HOOK_Y_DEEP_MAX),
        };
        let Some(id) = s.space_hook_free.pop() else { break; };
        s.space_hook_live.push(id.clone());
        s.space_hook_rightmost = x;
        spawned += 1;
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position = (x - HOOK_R, y - HOOK_R);
            obj.size     = (HOOK_R * 2.0, HOOK_R * 2.0);
            obj.visible  = true;
            obj.set_image(hook_img(C_SPACE_HOOK.0, C_SPACE_HOOK.1, C_SPACE_HOOK.2));
        }

        s = st.lock().unwrap();
    }
}

fn spawn_space_planets(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < SPACE_PLANET_SPAWN_BUDGET
        && !s.space_planet_free.is_empty()
        && s.space_planet_rightmost < s.px + GEN_AHEAD
    {
        let gap  = lcg_range(&mut s.seed, SPACE_PLANET_GAP_MIN, SPACE_PLANET_GAP_MAX);
        let x    = s.space_planet_rightmost + gap;
        let y    = lcg_range(&mut s.seed, SPACE_PLANET_Y_MIN, SPACE_PLANET_Y_MAX);
        let large = lcg(&mut s.seed) < 0.35;
        let visual_r = if large {
            lcg_range(&mut s.seed, SPACE_PLANET_RADIUS_LG_MIN, SPACE_PLANET_RADIUS_LG_MAX)
        } else {
            lcg_range(&mut s.seed, SPACE_PLANET_RADIUS_SM_MIN, SPACE_PLANET_RADIUS_SM_MAX)
        };
        let gravity_r = visual_r * SPACE_PLANET_GRAV_R_MULT;
        let color_idx = (lcg(&mut s.seed) * C_SPACE_PLANET.len() as f32) as usize;
        let Some(id) = s.space_planet_free.pop() else { break; };
        s.space_planet_live.push(id.clone());
        s.space_planet_data.push((id.clone(), gravity_r, SPACE_PLANET_GRAV_STRENGTH));
        s.space_planet_rightmost = x;
        spawned += 1;
        drop(s);

        // Rebuild planet at new position and size
        let (pr, pg, pb) = C_SPACE_PLANET[color_idx % C_SPACE_PLANET.len()];
        let img = planet_img_cached(visual_r, gravity_r, pr, pg, pb);
        let d   = gravity_r * 2.0;
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position     = (x - gravity_r, y - gravity_r);
            obj.size         = (d, d);
            // Engine handles surface collision + slide at visual_r.
            // gravity_influence_mult (default 3×) extends the field beyond visual_r.
            obj.planet_radius = Some(visual_r);
            obj.visible      = true;
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                image: img,
                color: None,
            });
            obj.set_glow(GlowConfig {
                color: Color(pr, pg, pb, 55),
                width: 18.0,
            });
        }

        s = st.lock().unwrap();
    }
}

fn spawn_space_coins(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < SPACE_COIN_SPAWN_BUDGET
        && !s.space_coin_free.is_empty()
        && s.space_coin_rightmost < s.px + GEN_AHEAD
    {
        let gap = lcg_range(&mut s.seed, SPACE_COIN_GAP_MIN, SPACE_COIN_GAP_MAX);
        let x   = s.space_coin_rightmost + gap;
        // Place coins near existing planets or at random space y
        let y = lcg_range(&mut s.seed, SPACE_PLANET_Y_MAX * 0.95, SPACE_PLANET_Y_MAX * 0.10);
        let Some(id) = s.space_coin_free.pop() else { break; };
        s.space_coin_live.push(id.clone());
        s.space_coin_rightmost = x;
        spawned += 1;
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position = (x - SPACE_COIN_R, y - SPACE_COIN_R);
            obj.size     = (SPACE_COIN_R * 2.0, SPACE_COIN_R * 2.0);
            obj.visible  = true;
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (SPACE_COIN_R * 2.0, SPACE_COIN_R * 2.0), 0.0),
                image: space_coin_img_cached(SPACE_COIN_R as u32),
                color: None,
            });
            obj.set_glow(GlowConfig {
                color: Color(C_SPACE_COIN.0, C_SPACE_COIN.1, 60, 140),
                width: 14.0,
            });
        }

        s = st.lock().unwrap();
    }
}

fn spawn_space_blackholes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < SPACE_BLACKHOLE_SPAWN_BUDGET
        && !s.space_blackhole_free.is_empty()
        && s.space_blackhole_rightmost < s.px + GEN_AHEAD
    {
        let gap    = lcg_range(&mut s.seed, SPACE_BLACKHOLE_GAP_MIN, SPACE_BLACKHOLE_GAP_MAX);
        let x      = s.space_blackhole_rightmost + gap;
        let y      = lcg_range(&mut s.seed, SPACE_BLACKHOLE_Y_MIN, SPACE_BLACKHOLE_Y_MAX);
        let radius = lcg_range(&mut s.seed, SPACE_BLACKHOLE_RADIUS_MIN, SPACE_BLACKHOLE_RADIUS_MAX);
        let Some(id) = s.space_blackhole_free.pop() else { break; };
        s.space_blackhole_live.push(id.clone());
        s.space_blackhole_data.push((id.clone(), radius, SPACE_BLACKHOLE_GRAV_STRENGTH));
        s.space_blackhole_rightmost = x;
        spawned += 1;
        drop(s);

        let img = black_hole_img_cached(radius);
        let d   = radius * 2.0;
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position     = (x - radius, y - radius);
            obj.size         = (d, d);
            obj.planet_radius = Some(radius);
            obj.visible      = true;
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                image: img,
                color: None,
            });
        }

        s = st.lock().unwrap();
    }
}

// ── Culling ───────────────────────────────────────────────────────────────────

fn tick_space_culling(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    cull_space_hooks(c, st);
    cull_space_planets(c, st);
    cull_space_coins(c, st);
    cull_space_blackholes(c, st);
}

fn cull_space_hooks(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.space_hook_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + HOOK_R * 2.0 < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-9000.0, -9000.0);
        }
    }
    let active_culled = s.hooked && to_remove.iter().any(|n| *n == s.active_hook);
    s.space_hook_live.retain(|n| !to_remove.contains(n));
    for name in to_remove { s.space_hook_free.push(name); }
    if active_culled {
        let gdir   = s.gravity_dir;
        let gscale = SPACE_GRAVITY_SCALE;
        s.hooked       = false;
        s.active_hook  = String::new();
        drop(s);
        c.run(Action::Hide { target: Target::name("rope") });
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * gscale * gdir;
        }
    }
}

fn cull_space_planets(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 2.0;
    let to_remove: Vec<String> = s.space_planet_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + o.size.0 < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.planet_radius = None;
            obj.position = (-12000.0, -12000.0);
        }
    }
    s.space_planet_live.retain(|n| !to_remove.contains(n));
    s.space_planet_data.retain(|(n, _, _)| !to_remove.contains(n));
    for name in to_remove { s.space_planet_free.push(name); }
}

fn cull_space_coins(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.space_coin_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + SPACE_COIN_R * 2.0 < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-9500.0, -9500.0);
        }
    }
    s.space_coin_live.retain(|n| !to_remove.contains(n));
    for name in to_remove { s.space_coin_free.push(name); }
}

fn cull_space_blackholes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 2.0;
    let to_remove: Vec<String> = s.space_blackhole_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + o.size.0 < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.planet_radius = None;
            obj.position = (-12000.0, -12000.0);
        }
    }
    s.space_blackhole_live.retain(|n| !to_remove.contains(n));
    s.space_blackhole_data.retain(|(n, _, _)| !to_remove.contains(n));
    for name in to_remove { s.space_blackhole_free.push(name); }
}

/// Return ALL space objects to their free pools and hide them. Used on exit.
fn cull_all_space_objects(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();

    let hooks: Vec<String> = s.space_hook_live.drain(..).collect();
    for n in &hooks {
        if let Some(obj) = c.get_game_object_mut(n) { obj.visible = false; obj.position = (-9000.0, -9000.0); }
        s.space_hook_free.push(n.clone());
    }

    let planets: Vec<String> = s.space_planet_live.drain(..).collect();
    for n in &planets {
        if let Some(obj) = c.get_game_object_mut(n) {
            obj.visible = false;
            obj.planet_radius = None;
            obj.position = (-12000.0, -12000.0);
        }
        s.space_planet_free.push(n.clone());
    }
    s.space_planet_data.clear();

    let coins: Vec<String> = s.space_coin_live.drain(..).collect();
    for n in &coins {
        if let Some(obj) = c.get_game_object_mut(n) { obj.visible = false; obj.position = (-9500.0, -9500.0); }
        s.space_coin_free.push(n.clone());
    }

    let bhs: Vec<String> = s.space_blackhole_live.drain(..).collect();
    for n in &bhs {
        if let Some(obj) = c.get_game_object_mut(n) {
            obj.visible = false;
            obj.planet_radius = None;
            obj.position = (-12000.0, -12000.0);
        }
        s.space_blackhole_free.push(n.clone());
    }
    s.space_blackhole_data.clear();
}

// ── Space coin collection ─────────────────────────────────────────────────────

fn tick_space_coin_collect(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let collect_r = PLAYER_R + SPACE_COIN_R + 10.0;
    let live = s.space_coin_live.clone();
    let mut collected: Vec<String> = Vec::new();

    for name in &live {
        if let Some(obj) = c.get_game_object(name) {
            let cx = obj.position.0 + SPACE_COIN_R;
            let cy = obj.position.1 + SPACE_COIN_R;
            let dx = s.px - cx;
            let dy = s.py - cy;
            if dx * dx + dy * dy < collect_r * collect_r {
                collected.push(name.clone());
            }
        }
    }

    let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
    for name in &collected {
        s.score = s.score.saturating_add(SPACE_COIN_SCORE * score_mult);
        s.space_coin_live.retain(|n| n != name);
        s.space_coin_free.push(name.clone());
    }
    drop(s);

    for name in &collected {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-9500.0, -9500.0);
        }
        // Gold flash per collect
        if let Some(cam) = c.camera_mut() {
            cam.flash_with(Color(255, 220, 80, 60), 0.25, FlashMode::Pulse, FlashEase::Sharp, 0.9, 0.0);
        }
    }

    if !collected.is_empty() {
        c.play_sound_with(
            crate::constants::ASSET_COIN_SFX_2,
            SoundOptions::new().volume(0.28),
        );
    }
}
