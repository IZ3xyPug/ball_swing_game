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
use std::thread;

use crate::constants::*;
use crate::images::*;
use crate::objects::*;
use crate::state::*;
use super::helpers::*;

fn solar_surface_ratio_from_gif_bytes(solar_bytes: &[u8]) -> f32 {
    use image::AnimationDecoder;
    use image::codecs::gif::GifDecoder;

    const LUMA_MIN: f32 = 120.0;
    const COVERAGE_THRESHOLD: f32 = 0.35;

    let cursor = std::io::Cursor::new(solar_bytes);
    let Ok(decoder) = GifDecoder::new(cursor) else {
        return SOLAR_SURFACE_RATIO_DEFAULT;
    };
    let Ok(frames) = decoder.into_frames().collect_frames() else {
        return SOLAR_SURFACE_RATIO_DEFAULT;
    };
    if frames.is_empty() {
        return SOLAR_SURFACE_RATIO_DEFAULT;
    }

    let h = frames[0].buffer().height() as usize;
    let w = frames[0].buffer().width() as usize;
    if h < 2 || w == 0 {
        return SOLAR_SURFACE_RATIO_DEFAULT;
    }

    let mut row_coverage = vec![0.0f32; h];
    for frame in &frames {
        let buf = frame.buffer();
        for y in 0..h {
            let mut covered = 0usize;
            for x in 0..w {
                let p = buf.get_pixel(x as u32, y as u32);
                let luma = 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32;
                if luma >= LUMA_MIN {
                    covered += 1;
                }
            }
            row_coverage[y] += covered as f32 / w as f32;
        }
    }
    let inv_n = 1.0 / frames.len() as f32;
    for v in &mut row_coverage {
        *v *= inv_n;
    }

    for y in (0..h).rev() {
        if row_coverage[y] >= COVERAGE_THRESHOLD {
            return (y as f32 / (h - 1) as f32).clamp(0.0, 1.0);
        }
    }

    SOLAR_SURFACE_RATIO_DEFAULT
}

fn solar_kill_y(s: &State) -> f32 {
    let surface_ratio = s.solar_surface_ratio.clamp(0.0, 1.0);
    SPACE_UPPER_LIMIT_Y + SPACE_SOLAR_H * surface_ratio
}

fn queue_solar_decode_if_needed(st: &Arc<Mutex<State>>) {
    let should_queue = {
        let s = st.lock().unwrap();
        !s.solar_anim_loaded && s.solar_anim_pending.is_none()
    };
    if !should_queue {
        return;
    }

    let pending = Arc::new(Mutex::new(None::<AnimatedSprite>));
    st.lock().unwrap().solar_anim_pending = Some(Arc::clone(&pending));
    let st_for_decode = Arc::clone(st);
    thread::spawn(move || {
        let solar_bytes: &[u8] = include_bytes!("../../../assets/corona_v5.gif");
        // Decode at 1/8 resolution — GPU upscales to full size at display time.
        // 4× fewer pixels per frame vs 1/4, so this completes well before space entry.
        let load_w = VW / 8.0;
        let load_h = SPACE_SOLAR_H / 8.0;
        let surface_ratio = solar_surface_ratio_from_gif_bytes(solar_bytes);
        if let Ok(anim) = AnimatedSprite::new(
            solar_bytes, (load_w, load_h), SOLAR_ANIM_FPS,
        ) {
            *pending.lock().unwrap() = Some(anim);
            st_for_decode.lock().unwrap().solar_surface_ratio = surface_ratio;
            return;
        }

        // Decode failed: clear pending so later ticks/re-entries can retry.
        st_for_decode.lock().unwrap().solar_anim_pending = None;
    });
}

pub fn prewarm_solar_decode(st: &Arc<Mutex<State>>) {
    queue_solar_decode_if_needed(st);
}

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
            // Pre-warm solar gif decode the moment the rocket fires — gives the
            // full ascent time (several seconds) for the thread to finish before
            // the player arrives near the solar ceiling.
            queue_solar_decode_if_needed(st);
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
    // In space there is no pendulum: zero downward gravity while hooked so the
    // player swings at a constant rate. Restore space gravity when unhooked.
    {
        let (hooked, gdir) = { let s = st.lock().unwrap(); (s.hooked, s.gravity_dir) };
        if hooked {
            if let Some(g) = c.get_grapple_mut("player") { g.damping = 0.0; }
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.gravity = 0.0;                    // no pendulum: constant-rate swing
                obj.gravity_target = None;            // no planet pull while on rope
                obj.gravity_all_sources = false;      // disable ALL gravity sources while swinging
            }
        } else {
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.gravity = GRAVITY * SPACE_GRAVITY_SCALE * gdir;
                obj.gravity_target = Some("space_planet".to_string()); // planet pull when free
                obj.gravity_all_sources = false;      // only tagged "space_planet" planets pull
            }
        }
    }

    // Keep solar animation/device state current before evaluating death.
    tick_solar_pending(c, st);
    tick_solar_screen_pos(c, st);

    // Near-surface orbit capture for space planets. Keeps the player circling
    // just above the surface instead of being pulled into repeated impacts.
    tick_space_planet_orbit_capture(c, st);

    // ── Solar ceiling kill zone ───────────────────────────────────────────────
    let (py, kill_y) = {
        let s = st.lock().unwrap();
        (s.py, solar_kill_y(&s))
    };
    if py < kill_y {
        // Player has crossed the dense solar surface line — sun-death.
        c.set_var("died_to_sun", true);
        if let Some(cam) = c.camera_mut() {
            cam.flash_with(
                Color(255, 200, 50, 255),
                1.4,
                FlashMode::FadeOut,
                FlashEase::Smooth,
                1.0,
                0.3,
            );
        }
        exit_space(c, st, true); // forced: no exit stasis, no return
        return;
    }

    tick_space_camera(c, st);
    tick_space_oxygen(c, st);
    tick_space_gwells(c, st, frame);
    tick_space_spawning(c, st, frame);
    tick_space_culling(c, st);
    tick_space_coin_collect(c, st);
    tick_space_welcome_text(c, st);
    tick_space_planet_pulse(c, st, frame);

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
        s.space_orbit_locked_planet.clear();
        s.space_orbit_speed = 0.0;

        // Seed space object rightmosts from current player x
        let px = s.px;
        s.space_planet_rightmost    = px - VW * 0.5;
        s.space_hook_rightmost      = px - VW * 0.5;
        s.space_coin_rightmost      = px - VW * 0.5;
        s.space_blackhole_rightmost = px - VW * 2.0;
        s.space_asteroid_rightmost  = px - VW * 0.5;

        // Freeze background scale for parallax starfield effect
        s.space_entry_bg_scale = 1.0; // will be refined below after drop
        s.space_entry_px = s.px; // freeze X for return teleport
        // Return any leftover spent coins from a prior visit to the free pool
        let _sc: Vec<String> = s.space_coin_spent.drain(..).collect();
        s.space_coin_free.extend(_sc);
        let _src: Vec<String> = s.space_red_coin_spent.drain(..).collect();
        s.space_red_coin_free.extend(_src);
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
        if let Some(obj) = c.get_game_object_mut("player") {
            if !hooked { obj.gravity = GRAVITY * SPACE_GRAVITY_SCALE * gdir; }
            // Enable engine planet-gravity attraction toward "space_planet"-tagged objects
            obj.gravity_target        = Some("space_planet".to_string());
            obj.gravity_influence_mult = 3.0;  // field bounded to 3× planet_radius
            obj.gravity_all_sources   = false; // only respond to space_planet gravity
        }
    }

    // Show solar ceiling. Prewarm started at game load; tick_solar_pending
    // attaches the animation the tick the background thread finishes.
    // If already decoded (common on revisit), it is visible immediately.
    if let Some(obj) = c.get_game_object_mut("solar_ceiling") {
        if obj.animated_sprite.is_none() {
            queue_solar_decode_if_needed(st);
        } else {
            obj.visible = true;
        }
    }

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

    // ── Entry stasis: orbit a nearby hook before the player has control ───────
    // Pick a hook from the pool; place it just above entry, directly above player.
    let stasis_hook = {
        let mut s = st.lock().unwrap();
        s.space_hook_free.pop()
    };
    if let Some(hook_id) = stasis_hook {
        let (px, px_done) = {
            let s = st.lock().unwrap();
            (s.px, true)
        };
        // Place hook just above entry at a comfortable orbit depth
        let hx = px;
        let hy = SPACE_ENTRY_Y - STASIS_ORBIT_R * 2.5;

        if let Some(obj) = c.get_game_object_mut(&hook_id) {
            obj.position = (hx - HOOK_R, hy - HOOK_R);
            obj.size     = (HOOK_R * 2.0, HOOK_R * 2.0);
            obj.visible  = true;
            obj.set_image(hook_asteroid_img_for_id(&hook_id, AsteroidHookState::Base));
        }

        {
            let mut s = st.lock().unwrap();
            s.space_hook_live.push(hook_id.clone());
            if s.space_hook_rightmost < hx { s.space_hook_rightmost = hx; }
            // Position player at orbit start (top of circle)
            s.px = hx;
            s.py = hy - STASIS_ORBIT_R;
            s.vx = 0.0;
            s.vy = 0.0;
            // Stasis state
            s.space_stasis_active   = true;
            s.space_stasis_hook_id  = hook_id.clone();
            s.space_stasis_is_entry = true;
            s.space_settle_done     = true; // prevent old settle from firing
            // Orbit center for build_scene orbit animation
            s.hook_x = hx;
            s.hook_y = hy;
        }

        if let Some(obj) = c.get_game_object_mut("player") {
            obj.position = (px - PLAYER_R, SPACE_ENTRY_Y - STASIS_ORBIT_R - PLAYER_R);
            obj.momentum = (0.0, 0.0);
            obj.gravity  = 0.0;
        }

        // Activate soft pause: game_paused + start_prompt_active (orbit animation)
        c.set_var("game_paused", true);
        c.set_var("start_prompt_active", true);
        c.set_var("start_orbit_ticks", 0i32);
        let _ = px_done;

        // Update prompt text
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
            let scale = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    "HOLD SPACE TO EXPLORE",
                    &font,
                    52.0 * scale,
                    Color(235, 245, 255, 255),
                    1300.0 * scale,
                )));
                obj.visible = true;
            }
        }
    }
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
    // Body-only image: visual_r for both params, no ring padding
    let img = planet_img_cached(visual_r, visual_r, pr, pg, pb);
    let d   = visual_r * 2.0;
    if let Some(obj) = c.get_game_object_mut(&id) {
        obj.position      = (x - visual_r, y - visual_r);
        obj.size          = (d, d);
        obj.planet_radius  = Some(visual_r); // engine gravity + landing snap
        obj.collision_mode = CollisionMode::Solid(CollisionShape::Circle { radius: visual_r }); // solid surface at visual radius
        obj.visible        = true;
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
            obj.set_image(hook_asteroid_img_for_id(&hook_id, AsteroidHookState::Base));
        }
        s = st.lock().unwrap();
    }
}

pub fn exit_space(c: &mut Canvas, st: &Arc<Mutex<State>>, forced: bool) {
    {
        let mut s = st.lock().unwrap();
        if !s.in_space_mode { return; }
        s.in_space_mode = false;
        s.space_stasis_active = false;
        s.space_gwell_timers.clear();
        s.space_orbit_locked_planet.clear();
        s.space_orbit_speed = 0.0;
    }

    // Hide solar ceiling
    if let Some(obj) = c.get_game_object_mut("solar_ceiling") {
        obj.visible = false;
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
    c.set_var("space_exit_zoom_reset", true);

    // Restore normal swing damping
    if let Some(g) = c.get_grapple_mut("player") { g.damping = 0.001; }

    // Restore normal gravity; clear space-planet attraction, snap X to entry position
    {
        let s = st.lock().unwrap();
        let gdir = s.gravity_dir;
        let hooked = s.hooked;
        let entry_px = s.space_entry_px;
        drop(s);
        if let Some(obj) = c.get_game_object_mut("player") {
            if !hooked { obj.gravity = GRAVITY * gdir; }
            obj.gravity_target        = None;
            obj.gravity_influence_mult = 3.0;
            obj.gravity_all_sources   = true; // restore normal game gravity-well behavior
            obj.position.0 = entry_px - PLAYER_R; // snap X to pre-space position
        }
        st.lock().unwrap().px = entry_px;
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

    // ── Exit stasis: orbit a hook in normal space before player resumes ───────
    // Only for voluntary returns (not forced oxygen/sun death).
    if !forced {
        // Use a hook from the normal pool placed in comfortable normal-world height.
        let exit_hook = {
            let mut s = st.lock().unwrap();
            s.pool_free.pop()
        };
        if let Some(hook_id) = exit_hook {
            let px = st.lock().unwrap().px;
            let hx = px;
            let hy = VH * 0.28; // well within normal play zone, near top third

            if let Some(obj) = c.get_game_object_mut(&hook_id) {
                obj.position = (hx - HOOK_R, hy - HOOK_R);
                obj.size     = (HOOK_R * 2.0, HOOK_R * 2.0);
                obj.visible  = true;
            }

            {
                let mut s = st.lock().unwrap();
                s.live_hooks.push(hook_id.clone());
                // Position player at orbit top
                s.px = hx;
                s.py = hy - STASIS_ORBIT_R;
                s.vx = 0.0;
                s.vy = 0.0;
                s.hooked = false;
                // Stasis state
                s.space_stasis_active   = true;
                s.space_stasis_hook_id  = hook_id.clone();
                s.space_stasis_is_entry = false;
                // Orbit center for build_scene orbit animation
                s.hook_x = hx;
                s.hook_y = hy;
            }

            if let Some(obj) = c.get_game_object_mut("player") {
                obj.position = (px - PLAYER_R, hy - STASIS_ORBIT_R - PLAYER_R);
                obj.momentum = (0.0, 0.0);
                obj.gravity  = 0.0;
            }
            if let Some(obj) = c.get_game_object_mut("rope") {
                obj.visible = false;
            }

            // Snap camera to normal zone, then activate stasis
            if let Some(cam) = c.camera_mut() {
                cam.zoom_anchor = Some((hx, hy));
                cam.zoom_lerp_speed = 0.06;
                cam.smooth_zoom(1.30);
            }

            c.set_var("game_paused", true);
            c.set_var("start_prompt_active", true);
            c.set_var("start_orbit_ticks", 0i32);

            if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
                let scale = c.virtual_scale();
                if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
                    obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                        "HOLD SPACE TO CONTINUE",
                        &font,
                        52.0 * scale,
                        Color(235, 245, 255, 255),
                        1300.0 * scale,
                    )));
                    obj.visible = true;
                }
            }
        }
    }
}

// ── Camera ────────────────────────────────────────────────────────────────────

/// Poll the solar ceiling async decode. When the background thread has finished
/// decoding the AnimatedSprite, swap it onto the solar_ceiling object and clear
/// the pending handle from state.
fn tick_solar_pending(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Clone the Arc handle without holding the state lock during try_lock.
    let pending_arc = st.lock().unwrap().solar_anim_pending.clone();
    if let Some(arc) = pending_arc {
        if let Ok(mut guard) = arc.try_lock() {
            if let Some(mut anim) = guard.take() {
                if let Some(obj) = c.get_game_object_mut("solar_ceiling") {
                    obj.size = (VW, SPACE_SOLAR_H);
                    obj.set_image(anim.get_current_image());
                    obj.set_animation(anim);
                    obj.visible = true;
                }
                let mut s = st.lock().unwrap();
                s.solar_anim_pending = None;
                s.solar_anim_loaded = true;
            }
        }
    }
}

fn tick_solar_screen_pos(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Distance-based reveal: keep native resolution and only slide vertically
    // from off-screen as the player nears the killline.
    let (py, kill_y) = {
        let s = st.lock().unwrap();
        (s.py, solar_kill_y(&s))
    };
    let dist_to_kill = (py - kill_y).max(0.0);
    let t = (1.0 - dist_to_kill / SPACE_SOLAR_REVEAL_DIST).clamp(0.0, 1.0);
    let scale = SPACE_SOLAR_FAR_SCALE + (1.0 - SPACE_SOLAR_FAR_SCALE) * t;
    let w = VW * scale;
    let h = SPACE_SOLAR_H * scale;
    let bottom_y = SPACE_SOLAR_FAR_BOTTOM_OFFSET * (1.0 - t) + SPACE_SOLAR_NEAR_BOTTOM_Y * t;
    let screen_x = -(w - VW) * 0.5;
    let screen_y = bottom_y - h;
    if let Some(obj) = c.get_game_object_mut("solar_ceiling") {
        obj.size = (w, h);
        obj.position = (screen_x, screen_y);
    }
}

pub(super) fn tick_space_camera_pub(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    tick_space_camera(c, st);
}

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
        // Don't drain oxygen during entry/exit stasis.
        if s.space_stasis_active {
            return;
        }
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
        let (dist, coins) = {
            let mut s = st.lock().unwrap();
            s.dead = true;
            (s.distance, s.coin_count as i32)
        };
        c.set_var("last_distance", dist);
        c.set_var("last_coins", coins.max(0));
        c.set_var("died_to_oxygen", true);
        c.set_var("died_to_sun", false);
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.visible = false;
        }
        if let Some(obj) = c.get_game_object_mut("rope") {
            obj.visible = false;
        }
        c.load_scene("gameover");
        return;
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

// ── Planet orbit capture (space mode) ───────────────────────────────────────
fn tick_space_planet_orbit_capture(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.in_space_mode || s.space_stasis_active {
        s.space_orbit_locked_planet.clear();
        s.space_orbit_speed = 0.0;
        return;
    }
    if s.hooked {
        // Hook grab explicitly breaks orbit lock so grapple can pull free.
        s.space_orbit_locked_planet.clear();
        s.space_orbit_speed = 0.0;
        return;
    }

    let (px, py, vx, vy) = (s.px, s.py, s.vx, s.vy);
    let planet_ids: Vec<String> = s.space_planet_live.clone();
    let locked_id = s.space_orbit_locked_planet.clone();
    let locked_speed = s.space_orbit_speed;
    drop(s);

    // If already locked, stay on that orbit every tick for a smooth perfect ring.
    if !locked_id.is_empty() {
        if let Some(obj) = c.get_game_object(&locked_id) {
            if obj.visible {
                let surface_r = obj.planet_radius.unwrap_or(obj.size.0 * 0.5);
                let cx = obj.position.0 + surface_r;
                let cy = obj.position.1 + surface_r;
                let dx = px - cx;
                let dy = py - cy;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let nx = dx / dist;
                let ny = dy / dist;
                let tx = -ny;
                let ty = nx;
                let mut tangential_v = if locked_speed.abs() >= SPACE_PLANET_ORBIT_MIN_TANGENTIAL {
                    locked_speed
                } else {
                    let cross = dx * vy - dy * vx;
                    let sign = if cross.abs() > 0.001 { cross.signum() } else { 1.0 };
                    SPACE_PLANET_ORBIT_MIN_TANGENTIAL * sign
                };
                tangential_v = tangential_v.clamp(
                    -SPACE_PLANET_ORBIT_MAX_TANGENTIAL,
                    SPACE_PLANET_ORBIT_MAX_TANGENTIAL,
                );

                let orbit_r = surface_r + PLAYER_R + SPACE_PLANET_ORBIT_ALT_PAD;
                let mut s = st.lock().unwrap();
                s.px = cx + nx * orbit_r;
                s.py = cy + ny * orbit_r;
                s.vx = tx * tangential_v;
                s.vy = ty * tangential_v;
                s.space_orbit_speed = tangential_v;
                return;
            }
        }

        // Locked planet went away; clear lock and allow reacquire.
        let mut s = st.lock().unwrap();
        s.space_orbit_locked_planet.clear();
        s.space_orbit_speed = 0.0;
    }

    // Acquire lock only when entering near a planet surface.
    let mut best: Option<(String, f32, f32, f32, f32)> = None; // (id, dist, cx, cy, surface_r)
    for id in &planet_ids {
        let Some(obj) = c.get_game_object(id) else { continue; };
        if !obj.visible {
            continue;
        }
        let surface_r = obj.planet_radius.unwrap_or(obj.size.0 * 0.5);
        let cx = obj.position.0 + surface_r;
        let cy = obj.position.1 + surface_r;
        let dx = px - cx;
        let dy = py - cy;
        let dist = (dx * dx + dy * dy).sqrt();
        let capture_r = surface_r + PLAYER_R + SPACE_PLANET_ORBIT_CAPTURE_PAD;
        if dist > capture_r {
            continue;
        }

        match &best {
            Some((_, best_dist, _, _, _)) if dist >= *best_dist => {}
            _ => best = Some((id.clone(), dist, cx, cy, surface_r)),
        }
    }

    let Some((planet_id, dist, cx, cy, surface_r)) = best else { return; };
    let safe_dist = dist.max(1.0);
    let nx = (px - cx) / safe_dist;
    let ny = (py - cy) / safe_dist;
    let tx = -ny;
    let ty = nx;

    let mut tangential_v = vx * tx + vy * ty;
    if tangential_v.abs() < SPACE_PLANET_ORBIT_MIN_TANGENTIAL {
        let cross = (px - cx) * vy - (py - cy) * vx;
        let sign = if cross.abs() > 0.001 {
            cross.signum()
        } else if tangential_v >= 0.0 {
            1.0
        } else {
            -1.0
        };
        tangential_v = SPACE_PLANET_ORBIT_MIN_TANGENTIAL * sign;
    }
    tangential_v = tangential_v.clamp(
        -SPACE_PLANET_ORBIT_MAX_TANGENTIAL,
        SPACE_PLANET_ORBIT_MAX_TANGENTIAL,
    );

    let orbit_r = surface_r + PLAYER_R + SPACE_PLANET_ORBIT_ALT_PAD;
    let mut s = st.lock().unwrap();
    s.px = cx + nx * orbit_r;
    s.py = cy + ny * orbit_r;
    s.vx = tx * tangential_v;
    s.vy = ty * tangential_v;
    s.space_orbit_locked_planet = planet_id;
    s.space_orbit_speed = tangential_v;
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
    spawn_space_sun_bonus_clusters(c, st);
    spawn_space_red_coins(c, st);
    spawn_space_blackholes(c, st);
    spawn_space_asteroids(c, st);
}

fn spawn_space_coin_pick(c: &mut Canvas, st: &Arc<Mutex<State>>, x: f32, y: f32, high_chance: f32) {
    let (id, is_high) = {
        let mut s = st.lock().unwrap();
        let roll = lcg(&mut s.seed);
        let pick_high = roll < high_chance && !s.space_red_coin_free.is_empty();

        if pick_high {
            let Some(id) = s.space_red_coin_free.pop() else { return; };
            s.space_red_coin_live.push(id.clone());
            (id, true)
        } else if let Some(id) = s.space_coin_free.pop() {
            s.space_coin_live.push(id.clone());
            if s.space_coin_rightmost < x {
                s.space_coin_rightmost = x;
            }
            (id, false)
        } else if let Some(id) = s.space_red_coin_free.pop() {
            s.space_red_coin_live.push(id.clone());
            (id, true)
        } else {
            return;
        }
    };

    if let Some(obj) = c.get_game_object_mut(&id) {
        if is_high {
            obj.position = (x - SPACE_RED_COIN_R, y - SPACE_RED_COIN_R);
            obj.size = (SPACE_RED_COIN_R * 2.0, SPACE_RED_COIN_R * 2.0);
            obj.visible = true;
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (SPACE_RED_COIN_R * 2.0, SPACE_RED_COIN_R * 2.0), 0.0),
                image: red_coin_img_cached(SPACE_RED_COIN_R as u32),
                color: None,
            });
            obj.set_glow(GlowConfig { color: Color(C_SPACE_COIN_HIGH.0, C_SPACE_COIN_HIGH.1, C_SPACE_COIN_HIGH.2, 170), width: 18.0 });
        } else {
            obj.position = (x - SPACE_COIN_R, y - SPACE_COIN_R);
            obj.size = (SPACE_COIN_R * 2.0, SPACE_COIN_R * 2.0);
            obj.visible = true;
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (SPACE_COIN_R * 2.0, SPACE_COIN_R * 2.0), 0.0),
                image: space_coin_img_cached(SPACE_COIN_R as u32),
                color: None,
            });
            obj.set_glow(GlowConfig { color: Color(C_SPACE_COIN.0, C_SPACE_COIN.1, C_SPACE_COIN.2, 140), width: 14.0 });
        }
    }
}

fn spawn_space_sun_bonus_clusters(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (spawn_cluster, center_x, center_y, count) = {
        let mut s = st.lock().unwrap();
        if lcg(&mut s.seed) > SPACE_SUN_BONUS_CLUSTER_CHANCE {
            return;
        }
        let count = SPACE_SUN_BONUS_CLUSTER_COINS_MIN
            + ((lcg(&mut s.seed) * (SPACE_SUN_BONUS_CLUSTER_COINS_MAX - SPACE_SUN_BONUS_CLUSTER_COINS_MIN + 1) as f32) as usize)
                .min(SPACE_SUN_BONUS_CLUSTER_COINS_MAX - SPACE_SUN_BONUS_CLUSTER_COINS_MIN);
        let center_x = s.px + GEN_AHEAD * (0.58 + lcg(&mut s.seed) * 0.30);
        let y_min = SPACE_HOOK_SUN_SAFETY_BAND_MIN.max(SPACE_HOOK_SUN_ZONE_Y_MIN);
        let y_max = SPACE_HOOK_SUN_SAFETY_BAND_MAX.min(SPACE_HOOK_SUN_ZONE_Y_MAX);
        let center_y = lcg_range(&mut s.seed, y_min, y_max);
        (true, center_x, center_y, count)
    };

    if !spawn_cluster {
        return;
    }

    let start_x = center_x - SPACE_SUN_BONUS_CLUSTER_SPACING * (count as f32 - 1.0) * 0.5;
    for i in 0..count {
        let x = start_x + i as f32 * SPACE_SUN_BONUS_CLUSTER_SPACING;
        let y = center_y + ((i as f32 * 1.7).sin() * 55.0);
        spawn_space_coin_pick(c, st, x, y, SPACE_SUN_BONUS_RED_CHANCE);
    }
}

fn spawn_space_hooks(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < SPACE_HOOK_SPAWN_BUDGET
        && !s.space_hook_free.is_empty()
        && s.space_hook_rightmost < s.px + GEN_AHEAD
    {
        // 4 vertical bands, biased toward the solar safety zone for late recoveries.
        let roll = lcg(&mut s.seed);
        let band = if roll < 0.22 {
            0
        } else if roll < 0.46 {
            1
        } else if roll < 0.66 {
            2
        } else {
            3
        };
        let gap = match band {
            3 => lcg_range(&mut s.seed, SPACE_HOOK_SUN_GAP_MIN, SPACE_HOOK_SUN_GAP_MAX),
            _ => lcg_range(&mut s.seed, SPACE_HOOK_GAP_MIN, SPACE_HOOK_GAP_MAX),
        };
        let x   = s.space_hook_rightmost + gap;
        // Distribute across 4 vertical bands so hooks exist everywhere, with
        // extra density near the solar ceiling where the player needs them most.
        let y = match band {
            0 => lcg_range(&mut s.seed, SPACE_HOOK_Y_SHALLOW_MIN, SPACE_HOOK_Y_SHALLOW_MAX),
            1 => lcg_range(&mut s.seed, SPACE_HOOK_Y_MID_MIN,     SPACE_HOOK_Y_MID_MAX),
            2 => lcg_range(&mut s.seed, SPACE_HOOK_Y_DEEP_MIN,    SPACE_HOOK_Y_DEEP_MAX),
            _ => lcg_range(&mut s.seed, SPACE_HOOK_SUN_SAFETY_BAND_MIN, SPACE_HOOK_SUN_SAFETY_BAND_MAX),
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
            obj.set_image(hook_asteroid_img_for_id(&id, AsteroidHookState::Base));
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

        // Collect coin arc IDs (mix of normal and red) and nearby hook IDs
        let arc_count = SPACE_COIN_ARC_COUNT;
        let red_count = (arc_count as f32 * SPACE_COIN_ARC_RED_FRAC).floor() as usize;
        let normal_count = arc_count - red_count;
        let arc_coin_ids: Vec<(String, bool)> = {
            let mut ids: Vec<(String, bool)> = Vec::new();
            for _ in 0..normal_count {
                if let Some(cid) = s.space_coin_free.pop() {
                    ids.push((cid, false));
                }
            }
            for _ in 0..red_count {
                if let Some(cid) = s.space_red_coin_free.pop() {
                    ids.push((cid, true));
                }
            }
            ids
        };
        let hook_ids: Vec<String> = (0..SPACE_PLANET_NEARBY_HOOKS)
            .filter_map(|_| s.space_hook_free.pop())
            .collect();
        drop(s);

        // Rebuild planet
        let (pr, pg, pb) = C_SPACE_PLANET[color_idx % C_SPACE_PLANET.len()];
        // Body-only image: visual_r for both params, no ring padding
        let img = planet_img_cached(visual_r, visual_r, pr, pg, pb);
        let d   = visual_r * 2.0;
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position      = (x - visual_r, y - visual_r);
            obj.size          = (d, d);
            obj.planet_radius  = Some(visual_r);
            obj.collision_mode = CollisionMode::Solid(CollisionShape::Circle { radius: visual_r }); // match current visual size
            obj.visible        = true;
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                image: img,
                color: None,
            });
            obj.set_glow(GlowConfig { color: Color(pr, pg, pb, 55), width: 18.0 });
        }

        // Place coin arc around planet
        let arc_r = visual_r * SPACE_COIN_ARC_RADIUS_MULT;
        let n_arc = arc_coin_ids.len();
        let step = if n_arc > 0 { std::f32::consts::TAU / n_arc as f32 } else { 0.0 };
        let phase_offset: f32 = std::f32::consts::FRAC_PI_2; // start at top
        for (i, (cid, is_red)) in arc_coin_ids.iter().enumerate() {
            let theta = phase_offset + step * i as f32;
            let cx = x + arc_r * theta.cos();
            let cy = y + arc_r * theta.sin();
            if *is_red {
                if let Some(obj) = c.get_game_object_mut(cid) {
                    obj.position = (cx - SPACE_RED_COIN_R, cy - SPACE_RED_COIN_R);
                    obj.size     = (SPACE_RED_COIN_R * 2.0, SPACE_RED_COIN_R * 2.0);
                    obj.visible  = true;
                    obj.set_image(Image {
                        shape: ShapeType::Ellipse(0.0, (SPACE_RED_COIN_R * 2.0, SPACE_RED_COIN_R * 2.0), 0.0),
                        image: red_coin_img_cached(SPACE_RED_COIN_R as u32),
                        color: None,
                    });
                    obj.set_glow(GlowConfig { color: Color(255, 60, 20, 160), width: 18.0 });
                }
            } else {
                if let Some(obj) = c.get_game_object_mut(cid) {
                    obj.position = (cx - SPACE_COIN_R, cy - SPACE_COIN_R);
                    obj.size     = (SPACE_COIN_R * 2.0, SPACE_COIN_R * 2.0);
                    obj.visible  = true;
                    obj.set_image(Image {
                        shape: ShapeType::Ellipse(0.0, (SPACE_COIN_R * 2.0, SPACE_COIN_R * 2.0), 0.0),
                        image: space_coin_img_cached(SPACE_COIN_R as u32),
                        color: None,
                    });
                    obj.set_glow(GlowConfig { color: Color(C_SPACE_COIN.0, C_SPACE_COIN.1, 60, 140), width: 14.0 });
                }
            }
        }
        // Register arc coins
        {
            let mut s = st.lock().unwrap();
            for (cid, is_red) in arc_coin_ids {
                if is_red {
                    s.space_red_coin_live.push(cid);
                } else {
                    s.space_coin_live.push(cid);
                    if s.space_coin_rightmost < x { s.space_coin_rightmost = x; }
                }
            }
        }

        // Place hooks at cardinal offsets from planet
        let offsets: &[(f32, f32)] = &[
            (-(visual_r + SPACE_PLANET_HOOK_OFFSET), 0.0),
            ( visual_r + SPACE_PLANET_HOOK_OFFSET,  0.0),
            (0.0, -(visual_r + SPACE_PLANET_HOOK_OFFSET)),
        ];
        let mut hook_points: Vec<(f32, f32)> = Vec::new();
        {
            let mut s = st.lock().unwrap();
            for (hook_id, &(ox, oy)) in hook_ids.iter().zip(offsets.iter()) {
                let hx = x + ox;
                let hy = y + oy;
            hook_points.push((hx, hy));
                s.space_hook_live.push(hook_id.clone());
                if s.space_hook_rightmost < hx { s.space_hook_rightmost = hx; }
                drop(s);
                if let Some(obj) = c.get_game_object_mut(hook_id) {
                    obj.position = (hx - HOOK_R, hy - HOOK_R);
                    obj.size     = (HOOK_R * 2.0, HOOK_R * 2.0);
                    obj.visible  = true;
                    obj.set_image(hook_asteroid_img_for_id(hook_id, AsteroidHookState::Base));
                }
                s = st.lock().unwrap();
            }
        }

        // Partial guide lines from one nearby hook toward another, so players
        // can visually follow routes across hook nodes around each planet.
        for pair in hook_points.windows(2) {
            let (ax, ay) = pair[0];
            let (bx, by) = pair[1];
            for i in 0..SPACE_PLANET_HOOK_GUIDE_COINS {
                let denom = (SPACE_PLANET_HOOK_GUIDE_COINS - 1).max(1) as f32;
                let t01 = i as f32 / denom;
                let t = SPACE_PLANET_HOOK_GUIDE_T_MIN
                    + (SPACE_PLANET_HOOK_GUIDE_T_MAX - SPACE_PLANET_HOOK_GUIDE_T_MIN) * t01;
                let cx = ax + (bx - ax) * t;
                let cy = ay + (by - ay) * t;
                spawn_space_coin_pick(c, st, cx, cy, SPACE_PLANET_HOOK_GUIDE_RED_CHANCE);
            }
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
        // Start gwell active with a random remaining time so they don't all sync
        let initial_ticks = (lcg(&mut s.seed) * GWELL_ON_TICKS as f32) as u32 + 1;
        s.space_gwell_timers.push((id.clone(), initial_ticks, true));
        spawned += 1;

        let hook_ids: Vec<String> = (0..SPACE_GWELL_NEARBY_HOOKS)
            .filter_map(|_| s.space_hook_free.pop())
            .collect();
        drop(s);

        // Use gravity well ring visual (semi-transparent, visible but subtle)
        let visual_r = radius * 3.0; // display rings at 3× gravity radius
        let d = visual_r * 2.0;
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position      = (x - visual_r, y - visual_r);
            obj.size          = (d, d);
            obj.planet_radius = Some(radius); // starts active
            if !obj.tags.iter().any(|t| t == "space_planet") {
                obj.tags.push("space_planet".to_string());
            }
            obj.visible       = true;
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                image: gwell_ring_cached(visual_r, C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2, GWELL_RING_COUNT, 200.0),
                color: None,
            });
            obj.set_glow(GlowConfig {
                color: Color(C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2, 160),
                width: 12.0,
            });
        }

        // Place nearby hooks so the player can escape the gravity pull
        let hook_offsets: &[(f32, f32)] = &[
            (-(visual_r + SPACE_GWELL_HOOK_OFFSET), 0.0),
            ( visual_r + SPACE_GWELL_HOOK_OFFSET,   0.0),
        ];
        {
            let mut s = st.lock().unwrap();
            for (hook_id, &(ox, oy)) in hook_ids.iter().zip(hook_offsets.iter()) {
                let hx = x + ox;
                let hy = y + oy;
                s.space_hook_live.push(hook_id.clone());
                if s.space_hook_rightmost < hx { s.space_hook_rightmost = hx; }
                drop(s);
                if let Some(obj) = c.get_game_object_mut(hook_id) {
                    obj.position = (hx - HOOK_R, hy - HOOK_R);
                    obj.size     = (HOOK_R * 2.0, HOOK_R * 2.0);
                    obj.visible  = true;
                    obj.set_image(hook_asteroid_img_for_id(hook_id, AsteroidHookState::Base));
                }
                s = st.lock().unwrap();
            }
        }

        s = st.lock().unwrap();
    }
}

// ── Gravity well tick (space zone) ───────────────────────────────────────────

fn tick_space_gwells(c: &mut Canvas, st: &Arc<Mutex<State>>, frame: u32) {
    let mut s = st.lock().unwrap();
    let mut toggle_ids: Vec<(String, bool)> = Vec::new();

    for (id, remaining, active) in s.space_gwell_timers.iter_mut() {
        if *remaining > 0 { *remaining -= 1; }
        if *remaining == 0 {
            *active = !*active;
            *remaining = if *active { GWELL_ON_TICKS } else { GWELL_OFF_TICKS };
            toggle_ids.push((id.clone(), *active));
        }
    }

    let timers = s.space_gwell_timers.clone();
    drop(s);

    for (id, now_active) in &toggle_ids {
        if let Some(obj) = c.get_game_object_mut(id) {
            let visual_r = obj.size.0 * 0.5;
            let d = visual_r * 2.0;
            if *now_active {
                obj.planet_radius = Some(obj.planet_radius.unwrap_or(SPACE_BLACKHOLE_RADIUS_MIN));
                obj.set_image(Image {
                    shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                    image: gwell_ring_cached(visual_r, C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2, GWELL_RING_COUNT, 200.0),
                    color: None,
                });
                obj.set_glow(GlowConfig { color: Color(C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2, 200), width: 14.0 });
            } else {
                obj.planet_radius = None;
                obj.set_image(Image {
                    shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                    image: gwell_ring_cached(visual_r, C_GWELL_DORMANT.0, C_GWELL_DORMANT.1, C_GWELL_DORMANT.2, GWELL_RING_COUNT, 80.0),
                    color: None,
                });
                obj.set_glow(GlowConfig { color: Color(C_GWELL_DORMANT.0, C_GWELL_DORMANT.1, C_GWELL_DORMANT.2, 60), width: 6.0 });
            }
        }
    }

    // Pulse active wells
    for (id, _, active) in &timers {
        if !active { continue; }
        if let Some(obj) = c.get_game_object_mut(id) {
            let t = frame as f32 * GWELL_PULSE_SPEED;
            let pulse = GWELL_PULSE_MIN + (1.0 - GWELL_PULSE_MIN) * ((t.sin() + 1.0) * 0.5);
            obj.set_glow(GlowConfig {
                color: Color(C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2, (200.0 * pulse) as u8),
                width: 14.0 * pulse,
            });
        }
    }
}

// ── Spawn: space asteroids ────────────────────────────────────────────────────

fn spawn_space_asteroids(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < SPACE_ASTEROID_SPAWN_BUDGET
        && !s.space_asteroid_free.is_empty()
        && s.space_asteroid_rightmost < s.px + GEN_AHEAD
    {
        let gap   = lcg_range(&mut s.seed, SPACE_ASTEROID_GAP_MIN, SPACE_ASTEROID_GAP_MAX);
        let x     = s.space_asteroid_rightmost + gap;
        let large = lcg(&mut s.seed) < 0.4;
        let y     = if large {
            lcg_range(&mut s.seed, SPACE_ASTEROID_Y_FAR_MIN, SPACE_ASTEROID_Y_FAR_MAX)
        } else {
            lcg_range(&mut s.seed, SPACE_ASTEROID_Y_NEAR_MIN, SPACE_ASTEROID_Y_NEAR_MAX)
        };
        let size  = lcg_range(&mut s.seed, SPACE_ASTEROID_SIZE_MIN, SPACE_ASTEROID_SIZE_MAX);
        let drift_vx = lcg_range(&mut s.seed, SPACE_ASTEROID_VX_MIN, SPACE_ASTEROID_VX_MAX);
        let drift_vy = lcg_range(&mut s.seed, SPACE_ASTEROID_VY_MIN, SPACE_ASTEROID_VY_MAX);
        let rot_mom  = (lcg(&mut s.seed) - 0.5) * 0.02; // gentle spin

        let Some(id) = s.space_asteroid_free.pop() else { break; };
        s.space_asteroid_live.push(id.clone());
        s.space_asteroid_rightmost = x;
        spawned += 1;
        drop(s);

        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.position          = (x, y);
            obj.size              = (size, size);
            obj.momentum          = (drift_vx, drift_vy);
            obj.rotation_momentum = rot_mom;
            obj.gravity           = 0.0;
            obj.visible           = true;
            // Image already loaded in bootstrap; just resize the shape on the existing image.
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (size, size), 0.0),
                image: asteroid_hook_image_cached(),
                color: None,
            });
        }

        s = st.lock().unwrap();
    }
}

// ── Spawn: isolated space red coins ──────────────────────────────────────────

fn spawn_space_red_coins(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    // Only spawn isolated red coins if no recent planet spawned them.
    // We gate on a low probability roll per tick to keep them rare.
    // Actual heavy red-coin spawn happens in spawn_space_planets (coin arcs).
    // Here we add the occasional lonely red coin in open space.
    let roll = lcg(&mut s.seed);
    if roll > 0.004 { return; } // ~0.4% per tick = one every ~25 sec at 60fps

    let Some(id) = s.space_red_coin_free.pop() else { return; };

    let x = s.px + GEN_AHEAD * (0.6 + lcg(&mut s.seed) * 0.4);
    let y = lcg_range(&mut s.seed, SPACE_PLANET_Y_MIN, SPACE_PLANET_Y_MAX);
    s.space_red_coin_live.push(id.clone());
    drop(s);

    if let Some(obj) = c.get_game_object_mut(&id) {
        obj.position = (x - SPACE_RED_COIN_R, y - SPACE_RED_COIN_R);
        obj.size     = (SPACE_RED_COIN_R * 2.0, SPACE_RED_COIN_R * 2.0);
        obj.visible  = true;
        obj.set_image(Image {
            shape: ShapeType::Ellipse(0.0, (SPACE_RED_COIN_R * 2.0, SPACE_RED_COIN_R * 2.0), 0.0),
            image: red_coin_img_cached(SPACE_RED_COIN_R as u32),
            color: None,
        });
        obj.set_glow(GlowConfig { color: Color(255, 60, 20, 160), width: 18.0 });
    }
}

// ── Culling ───────────────────────────────────────────────────────────────────

fn tick_space_culling(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    cull_space_hooks(c, st);
    cull_space_planets(c, st);
    cull_space_coins(c, st);
    cull_space_red_coins(c, st);
    cull_space_blackholes(c, st);
    cull_space_asteroids(c, st);
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
    s.space_gwell_timers.retain(|(id, _, _)| !to_remove.contains(id));
    for name in to_remove { s.space_blackhole_free.push(name); }
}

fn cull_space_red_coins(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.space_red_coin_live.iter()
        .filter(|n| c.get_game_object(n)
            .map(|o| o.position.0 + SPACE_RED_COIN_R * 2.0 < cutoff)
            .unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible   = false;
            obj.position  = (-9500.0, -9500.0);
        }
    }
    s.space_red_coin_live.retain(|n| !to_remove.contains(n));
    for name in to_remove { s.space_red_coin_free.push(name); }
}

fn cull_space_asteroids(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 2.0;
    let to_remove: Vec<String> = s.space_asteroid_live.iter()
        .filter(|n| c.get_game_object(n)
            .map(|o| o.position.0 + o.size.0 < cutoff)
            .unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible  = false;
            obj.momentum = (0.0, 0.0);
            obj.position = (-12000.0, -12000.0);
        }
    }
    s.space_asteroid_live.retain(|n| !to_remove.contains(n));
    for name in to_remove { s.space_asteroid_free.push(name); }
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
    s.space_gwell_timers.clear();

    let red_coins: Vec<String> = s.space_red_coin_live.drain(..).collect();
    for n in &red_coins {
        if let Some(obj) = c.get_game_object_mut(n) { obj.visible = false; obj.position = (-9500.0, -9500.0); }
        s.space_red_coin_free.push(n.clone());
    }

    let asteroids: Vec<String> = s.space_asteroid_live.drain(..).collect();
    for n in &asteroids {
        if let Some(obj) = c.get_game_object_mut(n) {
            obj.visible  = false;
            obj.momentum = (0.0, 0.0);
            obj.position = (-12000.0, -12000.0);
        }
        s.space_asteroid_free.push(n.clone());
    }
    // Return spent coins to free pool (can respawn on next space visit)
    let spent: Vec<String> = s.space_coin_spent.drain(..).collect();
    for n in spent { s.space_coin_free.push(n); }
    let spent_red: Vec<String> = s.space_red_coin_spent.drain(..).collect();
    for n in spent_red { s.space_red_coin_free.push(n); }
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
        s.space_coin_spent.push(name.clone()); // won't respawn until next space entry
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

    // ── Red coin collection ───────────────────────────────────────────────────
    {
        let mut s = st.lock().unwrap();
        let collect_r = PLAYER_R + SPACE_RED_COIN_R + 10.0;
        let live = s.space_red_coin_live.clone();
        let mut red_collected: Vec<String> = Vec::new();
        for name in &live {
            if let Some(obj) = c.get_game_object(name) {
                let cx = obj.position.0 + SPACE_RED_COIN_R;
                let cy = obj.position.1 + SPACE_RED_COIN_R;
                let dx = s.px - cx;
                let dy = s.py - cy;
                if dx * dx + dy * dy < collect_r * collect_r {
                    red_collected.push(name.clone());
                }
            }
        }
        let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
        for name in &red_collected {
            s.score = s.score.saturating_add(SPACE_RED_COIN_SCORE * score_mult);
            s.space_red_coin_live.retain(|n| n != name);
            s.space_red_coin_spent.push(name.clone()); // won't respawn until next space entry
        }
        drop(s);
        for name in &red_collected {
            if let Some(obj) = c.get_game_object_mut(name) {
                obj.visible = false;
                obj.position = (-9500.0, -9500.0);
            }
            if let Some(cam) = c.camera_mut() {
                cam.flash_with(Color(255, 80, 20, 100), 0.3, FlashMode::Pulse, FlashEase::Sharp, 0.9, 0.0);
            }
        }
        if !red_collected.is_empty() {
            c.play_sound_with(
                crate::constants::ASSET_COIN_SFX_2,
                SoundOptions::new().volume(0.45),
            );
        }
    }
}
