// ── build_scene.rs — Thin dispatcher ──────────────────────────────────────
// All game logic lives in sibling modules. This file creates the scene,
// wires up callbacks, and dispatches the per-frame tick in order.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::gen_hook_batch;
use crate::images::*;
use crate::state::*;
use super::bootstrap;
use super::events;
use super::physics;
use super::spawning;
use super::culling;
use super::collision;
use super::pickups;
use super::visuals;
use super::hud_update;
use super::background;
use super::gravity_wells;
use super::helpers::*;
use quartz::Timer;

const PAUSE_MENU_ANIM_FRAMES: i32 = 14;
const PLAYER_TRAIL_EMITTER_NAME: &str = "player_trail";

pub fn build_game_scene(ctx: &mut Context) -> Scene {
    // Pre-compute background gradient images (small tile, stretched by GPU).
    // Generate the starfield once, then composite it into the upper half of each gradient.
    let bg_w = VW as u32;
    let bg_h = VH as u32;
    let starfield_quartz = star_field(bg_w, bg_h, STARFIELD_STAR_COUNT, 0xCAFE_BABE);
    let starfield_rgba: &image::RgbaImage = &starfield_quartz.image;

    let grad_start = gradient_rect(bg_w, bg_h, C_SKY_TOP, C_SKY_BOT);
    let grad_purple = gradient_rect(bg_w, bg_h, C_ZONE_PURPLE_TOP, C_ZONE_PURPLE_BOT);
    let grad_black = gradient_rect(bg_w, bg_h, C_ZONE_BLACK_TOP, C_ZONE_BLACK_BOT);
    let grad_start_vivid = gradient_rect(bg_w, bg_h, (8, 26, 74), (104, 194, 255));
    let grad_purple_vivid = gradient_rect(bg_w, bg_h, (56, 18, 94), (165, 78, 230));
    let grad_black_vivid = gradient_rect(bg_w, bg_h, (212, 142, 28), (255, 236, 120));

    let blend_h = bg_h / 8; // smooth transition zone
    let bg_zone_start = composite_starfield_gradient(starfield_rgba, &grad_start, bg_w, bg_h, blend_h);
    let bg_zone_purple = composite_starfield_gradient(starfield_rgba, &grad_purple, bg_w, bg_h, blend_h);
    let bg_zone_black = composite_starfield_gradient(starfield_rgba, &grad_black, bg_w, bg_h, blend_h);
    let bg_zone_start_vivid = composite_starfield_gradient(starfield_rgba, &grad_start_vivid, bg_w, bg_h, blend_h);
    let bg_zone_purple_vivid = composite_starfield_gradient(starfield_rgba, &grad_purple_vivid, bg_w, bg_h, blend_h);
    let bg_zone_black_vivid = composite_starfield_gradient(starfield_rgba, &grad_black_vivid, bg_w, bg_h, blend_h);

    // Pre-compute vertically flipped backgrounds for reverse gravity.
    let bg_zone_start_flip = flip_image_vertical(&bg_zone_start);
    let bg_zone_purple_flip = flip_image_vertical(&bg_zone_purple);
    let bg_zone_black_flip = flip_image_vertical(&bg_zone_black);
    let bg_zone_start_vivid_flip = flip_image_vertical(&bg_zone_start_vivid);
    let bg_zone_purple_vivid_flip = flip_image_vertical(&bg_zone_purple_vivid);
    let bg_zone_black_vivid_flip = flip_image_vertical(&bg_zone_black_vivid);

    // Build all game objects and pool structures.
    let (scene, pools) = bootstrap::build_scene_objects(ctx);

    let bootstrap::PoolSets {
        starter_names,
        pool_free,
        pad_free,
        spinner_free,
        coin_free,
        flip_free,
        score_x2_free,
        zero_g_free,
        gate_free,
        gwell_free,
        coin_static_sprite,
        coin_anim_template,
        score_x2_anim_template: _,
    } = pools;

    // Starter hook positions (must match bootstrap.rs).
    let starter_hooks: &[(f32, f32)] = &[
        (START_HOOK_X, START_HOOK_Y),
        (SPAWN_X + 1060.0, VH * 0.30),
        (SPAWN_X + 1860.0, VH * 0.46),
        (SPAWN_X + 2760.0, VH * 0.34),
        (SPAWN_X + 3720.0, VH * 0.52),
    ];

    // Persistent state arc — created on first enter, reused on respawns.
    let persistent_state: Arc<Mutex<Option<Arc<Mutex<State>>>>> =
        Arc::new(Mutex::new(None));

    scene
        .on_enter(move |canvas| {
            // ── Crystalline renderer ─────────────────────────────────────
            let crystalline_ready = matches!(
                canvas.get_var("crystalline_ready"),
                Some(Value::Bool(true))
            );
            if !crystalline_ready {
                canvas.enable_crystalline();
                canvas.enable_bloom(BloomSettings { threshold: 0.35, strength: 1.5 });
                canvas.set_var("crystalline_ready", true);

                // Give the player a persistent bright glow for bloom testing.
                if let Some(obj) = canvas.get_game_object_mut("player") {
                    obj.set_glow(GlowConfig {
                        color: Color(180, 255, 220, 180),
                        width: 14.0,
                    });
                }
            }

            // ── Night-mode restoration on re-enter ───────────────────────
            // Scene objects are cloned from bootstrap (unlit=false). If night
            // mode was active before death/scene switch, re-apply unlit and
            // re-add player lights.
            let night_active = matches!(
                canvas.get_var("night_mode_active"),
                Some(Value::Bool(true))
            );
            if night_active {
                // Re-apply lighting system if it was lost
                if !canvas.has_lighting() {
                    canvas.enable_lighting(LightingConfig::night());
                    canvas.enable_night_mode_shader(
                        0.30, 1.2, 0.55, 0.45, 0.35, 2.0,
                    );
                }

                // Mark all HUD objects as unlit
                for name in canvas.get_names_by_tag("hud") {
                    if let Some(obj) = canvas.get_game_object_mut(&name) {
                        obj.unlit = true;
                    }
                }

                // Re-add player lights (they were removed with the scene objects)
                let has_player_light = canvas.get_light("player_light").is_some();
                if !has_player_light {
                    let light = LightSource::new(
                        "player_light",
                        (0.0, 0.0),
                        Color(180, 255, 220, 255),
                        1200.0,
                        4.0,
                    );
                    canvas.add_light(light);
                    canvas.attach_light("player_light", "player", (0.0, 0.0));

                    let trail_light = LightSource::new(
                        "trail_light",
                        (0.0, 0.0),
                        Color(170, 255, 170, 200),
                        420.0,
                        2.2,
                    ).with_effect(LightEffect::Pulse {
                        min_intensity: 1.7,
                        max_intensity: 2.7,
                        speed: 3.0,
                    });
                    canvas.add_light(trail_light);
                    canvas.attach_light("trail_light", "player", (-60.0, 0.0));

                    // Particle trail segment lights — point lights at 1/4 the
                    // radius of player_light (300 = 1200 / 4).
                    for (i, (offset_x, intensity)) in [
                        (-50.0_f32, 2.0_f32),
                        (-120.0, 1.5),
                        (-200.0, 1.0),
                    ].iter().enumerate() {
                        let pid = format!("trail_p{}_light", i);
                        let tl = LightSource::new(
                            pid.clone(),
                            (0.0, 0.0),
                            Color(170, 255, 170, 200),
                            300.0,
                            *intensity,
                        );
                        canvas.add_light(tl);
                        canvas.attach_light(&pid, "player", (*offset_x, 0.0));
                    }
                }
            }

            // ── Player particle trail ────────────────────────────────────
            canvas.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
            let player_trail = EmitterBuilder::new(PLAYER_TRAIL_EMITTER_NAME)
                .rate(72.0)
                .lifetime(0.68)
                .velocity(-2.0, 8.0)
                .spread(6.0, 6.0)
                .size(9.0)
                .color(170, 255, 170, 255)
                .render_layer(2)
                .gravity_scale(0.0)
                .collision(CollisionResponse::None)
                .build();
            canvas.add_emitter(player_trail);
            canvas.attach_emitter_to(PLAYER_TRAIL_EMITTER_NAME, "player");

            // ── Camera ───────────────────────────────────────────────────
            let mut cam = Camera::new((VW * 80.0, VH), (VW, VH));
            cam.follow(Some(Target::name("player")));
            cam.lerp_speed = 0.10;
            canvas.set_camera(cam);

            // ── Pause key (register once globally) ───────────────────────
            let pause_key_registered = matches!(
                canvas.get_var("pause_key_registered"),
                Some(Value::Bool(true))
            );
            if !pause_key_registered {
                canvas.on_key_press(|c, key| {
                    if !c.is_scene("game") { return; }

                    if *key == Key::Character("1".into()) {
                        let vivid_now = matches!(
                            c.get_var("bg_vivid"),
                            Some(Value::Bool(true))
                        );
                        c.set_var("bg_vivid", !vivid_now);
                        return;
                    }

                    // Toggle bloom on/off for A/B comparison testing.
                    if *key == Key::Character("2".into()) {
                        if c.bloom_enabled() {
                            c.disable_bloom();
                        } else {
                            c.enable_bloom(BloomSettings { threshold: 0.35, strength: 1.5 });
                        }
                        return;
                    }

                    // Toggle night mode — dark ambient, player light with shadows,
                    // combined post shader (bloom + vignette + chromatic aberration).
                    if *key == Key::Character("3".into()) {
                        if c.has_lighting() {
                            c.disable_lighting();
                            c.clear_post_override();
                            c.set_var("night_mode_active", false);
                            // Restore HUD objects to lit pipeline
                            for name in c.get_names_by_tag("hud") {
                                if let Some(obj) = c.get_game_object_mut(&name) {
                                    obj.unlit = false;
                                }
                            }
                        } else {
                            c.enable_lighting(LightingConfig::night());
                            c.set_var("night_mode_active", true);

                            // Combined night-mode post shader
                            c.enable_night_mode_shader(
                                0.30,   // bloom threshold
                                1.2,    // bloom strength
                                0.55,   // vignette strength
                                0.45,   // vignette radius
                                0.35,   // vignette softness
                                2.0,    // chromatic aberration intensity
                            );

                            // Mark all HUD objects as unlit so they render at full brightness
                            for name in c.get_names_by_tag("hud") {
                                if let Some(obj) = c.get_game_object_mut(&name) {
                                    obj.unlit = true;
                                }
                            }

                            // Main player light — large radius, bright
                            let light = LightSource::new(
                                "player_light",
                                (0.0, 0.0),
                                Color(180, 255, 220, 255),
                                1200.0,
                                4.0,
                            );
                            c.add_light(light);
                            c.attach_light("player_light", "player", (0.0, 0.0));

                            // Trailing glow behind the player (proportionate to player light)
                            let trail_light = LightSource::new(
                                "trail_light",
                                (0.0, 0.0),
                                Color(170, 255, 170, 200),
                                420.0,
                                2.2,
                            ).with_effect(LightEffect::Pulse {
                                min_intensity: 1.7,
                                max_intensity: 2.7,
                                speed: 3.0,
                            });
                            c.add_light(trail_light);
                            c.attach_light("trail_light", "player", (-60.0, 0.0));

                            // Add lights to all currently visible coins
                            for name in c.get_names_by_tag("coin") {
                                if let Some(obj) = c.get_game_object(&name) {
                                    if obj.visible {
                                        let light_id = format!("coin_light_{}", name);
                                        let coin_light = LightSource::new(
                                            light_id.clone(),
                                            (0.0, 0.0),
                                            Color(255, 220, 80, 255),
                                            300.0,
                                            1.0,
                                        ).with_shadows(false).with_effect(LightEffect::Pulse {
                                            min_intensity: 0.75,
                                            max_intensity: 1.25,
                                            speed: 2.0,
                                        });
                                        c.add_light(coin_light);
                                        c.attach_light(&light_id, &name, (0.0, 0.0));
                                    }
                                }
                            }

                            // Trail particle lights — 1/4 the radius of player_light (300 = 1200 / 4)
                            for (i, (offset_x, intensity)) in [
                                (-50.0_f32, 2.0_f32),
                                (-120.0, 1.5),
                                (-200.0, 1.0),
                            ].iter().enumerate() {
                                let pid = format!("trail_p{}_light", i);
                                let tl = LightSource::new(
                                    pid.clone(),
                                    (0.0, 0.0),
                                    Color(170, 255, 170, 200),
                                    300.0,
                                    *intensity,
                                );
                                c.add_light(tl);
                                c.attach_light(&pid, "player", (*offset_x, 0.0));
                            }

                            // Spotlights aiming down from all currently visible spinners
                            for name in c.get_names_by_tag("spinner") {
                                if let Some(obj) = c.get_game_object(&name) {
                                    if obj.visible {
                                        let light_id = format!("spinner_light_{}", name);
                                        let mut spot = LightSource::new(
                                            light_id.clone(),
                                            (0.0, 0.0),
                                            Color(180, 180, 255, 200),
                                            480.0,
                                            3.5,
                                        );
                                        spot.light_type = LightType::Spot {
                                            direction: std::f32::consts::FRAC_PI_2,
                                            cone_angle: 1.0,
                                        };
                                        spot.casts_shadows = false;
                                        c.add_light(spot);
                                        c.attach_light(&light_id, &name, (0.0, 0.0));
                                    }
                                }
                            }
                        }
                        return;
                    }

                    // Toggle vignette on/off.
                    if *key == Key::Character("4".into()) {
                        let vig_on = matches!(c.get_var("vignette_active"), Some(Value::Bool(true)));
                        if vig_on {
                            c.clear_post_override();
                            c.set_var("vignette_active", false);
                        } else {
                            c.enable_vignette(0.6, 0.4, 0.35);
                            c.set_var("vignette_active", true);
                        }
                        return;
                    }

                    // Toggle chromatic aberration on/off.
                    if *key == Key::Character("5".into()) {
                        let ca_on = matches!(c.get_var("ca_active"), Some(Value::Bool(true)));
                        if ca_on {
                            c.clear_post_override();
                            c.set_var("ca_active", false);
                        } else {
                            c.enable_chromatic_aberration(3.0);
                            c.set_var("ca_active", true);
                        }
                        return;
                    }

                    let is_pause = *key == Key::Character("p".into());
                    let is_space = *key == Key::Named(NamedKey::Space);
                    if !is_pause && !is_space { return; }

                    let game_paused = c.is_paused()
                        || matches!(c.get_var("game_paused"), Some(Value::Bool(true)));

                    if game_paused {
                        c.resume();
                        c.set_var("pause_animating", false);
                        c.set_var("pause_anim_frames", 0);
                        c.set_var("game_paused", false);
                        let trail = EmitterBuilder::new(PLAYER_TRAIL_EMITTER_NAME)
                            .rate(72.0)
                            .lifetime(0.68)
                            .velocity(-2.0, 8.0)
                            .spread(6.0, 6.0)
                            .size(9.0)
                            .color(170, 255, 170, 255)
                            .render_layer(2)
                            .gravity_scale(0.0)
                            .collision(CollisionResponse::None)
                            .build();
                        c.add_emitter(trail);
                        c.attach_emitter_to(PLAYER_TRAIL_EMITTER_NAME, "player");
                        if let Some(obj) = c.get_game_object_mut("player") {
                            obj.visible = true;
                        }
                        // Only restore rope if the player was hooked when pause started.
                        let was_hooked = matches!(
                            c.get_var("rope_visible_at_pause"),
                            Some(Value::Bool(true))
                        );
                        if let Some(obj) = c.get_game_object_mut("rope") {
                            obj.visible = was_hooked;
                        }
                        // Hide pause overlay and buttons
                        for name in ["pause_overlay", "pause_title",
                                     "pause_resume_btn", "pause_respawn_btn",
                                     "pause_settings_btn", "pause_menu_btn"] {
                            if let Some(obj) = c.get_game_object_mut(name) {
                                obj.visible = false;
                            }
                        }
                    } else if is_pause {
                        let animating = matches!(
                            c.get_var("pause_animating"),
                            Some(Value::Bool(true))
                        );
                        if animating { return; }

                        c.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
                        if let Some(obj) = c.get_game_object_mut("player") {
                            obj.visible = false;
                        }
                        // Remember rope state before hiding so unpause can restore it.
                        let rope_was_visible = c
                            .get_game_object("rope")
                            .map_or(false, |o| o.visible);
                        c.set_var("rope_visible_at_pause", rope_was_visible);
                        if let Some(obj) = c.get_game_object_mut("rope") {
                            obj.visible = false;
                        }
                        // Start overlay + buttons off-screen for slide animation
                        if let Some(obj) = c.get_game_object_mut("pause_overlay") {
                            obj.position = (0.0, -VH);
                            obj.visible = true;
                        }
                        // Buttons also start off-screen (shifted up by VH)
                        let btn_layout: &[(&str, f32, f32)] = &[
                            ("pause_title", (VW - 650.0) / 2.0, VH * 0.20),
                            ("pause_resume_btn", (VW - 700.0) / 2.0, 820.0),
                            ("pause_respawn_btn", (VW - 700.0) / 2.0, 980.0),
                            ("pause_settings_btn", (VW - 700.0) / 2.0, 1140.0),
                            ("pause_menu_btn", (VW - 700.0) / 2.0, 1300.0),
                        ];
                        for &(name, bx, by) in btn_layout {
                            if let Some(obj) = c.get_game_object_mut(name) {
                                obj.position = (bx, by - VH);
                                obj.visible = true;
                            }
                        }
                        c.set_var("pause_anim_total", PAUSE_MENU_ANIM_FRAMES);
                        c.set_var("pause_anim_frames", PAUSE_MENU_ANIM_FRAMES);
                        c.set_var("pause_animating", true);
                    }
                });
                canvas.set_var("pause_key_registered", true);
            }

            canvas.set_var("pause_anim_frames", 0);
            canvas.set_var("pause_anim_total", PAUSE_MENU_ANIM_FRAMES);
            canvas.set_var("pause_animating", false);
            canvas.set_var("game_paused", false);
            canvas.set_var("mouse_grab_queued", false);
            canvas.set_var("mouse_release_queued", false);
            canvas.set_var("mouse_grab_x", 0.0f32);
            canvas.set_var("mouse_grab_y", 0.0f32);
            canvas.set_var("grab_from_mouse", false);
            canvas.set_var("bg_vivid", false);
            canvas.set_var("bg_force_refresh", true);

            // ── Fresh game state ─────────────────────────────────────────
            let mut seed: u64 = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0xDEAD_BEEF);

            let mut gen_y = starter_hooks
                .last()
                .map(|(_, y)| *y)
                .unwrap_or(SPAWN_Y);
            let first_from = starter_hooks
                .last()
                .map(|(x, _)| *x + 620.0)
                .unwrap_or(SPAWN_X + 2000.0);
            let first_batch = gen_hook_batch(&mut seed, first_from, &mut gen_y, 0.0);
            let rightmost_x = starter_hooks
                .last()
                .map(|(x, _)| *x)
                .unwrap_or(SPAWN_X);

            let start_hook = starter_hooks
                .first()
                .copied()
                .unwrap_or((START_HOOK_X, START_HOOK_Y));
            let start_rope_len = ((SPAWN_X - start_hook.0).powi(2)
                + (SPAWN_Y - start_hook.1).powi(2))
            .sqrt();

            let coin_spawn_anim = coin_anim_template.clone();
            let coin_spawn_image = coin_static_sprite.clone();

            let fresh_state = State {
                px: SPAWN_X,
                py: SPAWN_Y,
                vx: 13.0,
                vy: 0.0,
                hooked: true,
                hook_x: start_hook.0,
                hook_y: start_hook.1,
                rope_len: start_rope_len,
                active_hook: "hook_0".into(),
                distance: 0.0,
                score: 0,
                coin_count: 0,
                gravity_dir: 1.0,
                seed,
                pending: first_batch,
                live_hooks: starter_names.clone(),
                pool_free: pool_free.clone(),
                gen_y,
                rightmost_x,
                dead: false,
                ticks: 0,
                pad_live: Vec::new(),
                pad_free: pad_free.clone(),
                pad_rightmost: SPAWN_X,
                pad_origins: Vec::new(),
                pad_bounce_count: 0,
                spinner_live: Vec::new(),
                spinner_free: spinner_free.clone(),
                spinner_rightmost: SPAWN_X + VW * 0.65,
                spinner_origins: Vec::new(),
                spinners_enabled: true,
                spinner_spin_enabled: true,
                spinner_hit_cooldown: 0,
                coin_live: Vec::new(),
                coin_free: coin_free.clone(),
                coin_rightmost: SPAWN_X,
                coin_magnet_locked: Vec::new(),
                magnet_debug: false,
                flip_live: Vec::new(),
                flip_free: flip_free.clone(),
                flip_rightmost: SPAWN_X + VW * 1.1,
                flip_timer: 0,
                score_x2_live: Vec::new(),
                score_x2_free: score_x2_free.clone(),
                score_x2_rightmost: SPAWN_X + VW * 1.35,
                score_x2_timer: 0,
                zero_g_live: Vec::new(),
                zero_g_free: zero_g_free.clone(),
                zero_g_rightmost: SPAWN_X + VW * 1.6,
                zero_g_timer: 0,
                gate_live: Vec::new(),
                gate_free: gate_free.clone(),
                gate_rightmost: SPAWN_X + VW * 1.0,
                gwell_live: Vec::new(),
                gwell_free: gwell_free.clone(),
                gwell_rightmost: SPAWN_X + VW * 2.0,
                gwell_timers: Vec::new(),
                bounce_enabled: true,
                dark_mode: false,
                glow_flashes: Vec::new(),

                shake_cooldown: Timer::new(0.001),
                flash_cooldown: Timer::new(0.001),

                hud_last_dist_fill:    u32::MAX,
                hud_last_coins:        u32::MAX,
                hud_last_momentum:     u32::MAX,
                hud_last_gravity_flip: false,
                hud_last_py:           i32::MAX,
                hud_last_px:           i32::MAX,
                hud_last_flip_timer:   u32::MAX,
                hud_last_zero_g_timer: u32::MAX,
                burst_emitters: Vec::new(),
                burst_counter: 0,
            };

            // Reuse persistent Arc across respawns.
            {
                let mut slot = persistent_state.lock().unwrap();
                if let Some(existing) = slot.as_ref() {
                    *existing.lock().unwrap() = fresh_state;
                } else {
                    *slot = Some(Arc::new(Mutex::new(fresh_state)));
                }
            }
            let state =
                persistent_state.lock().unwrap().as_ref().unwrap().clone();

            // Start hooked to hook_0—highlight it.
            if let Some(obj) = canvas.get_game_object_mut("hook_0") {
                let (r, g, b) = hook_on_for_zone(0);
                obj.set_image(hook_img(r, g, b));
            }
            canvas.run(Action::Show {
                target: Target::name("rope"),
            });

            // Attach the initial grapple constraint so crystalline enforces
            // the rope from the very first frame (matches State.hooked = true).
            let initial_grapple = GrappleConstraint::at_point(start_hook, start_rope_len)
                .with_stiffness(1.0)
                .with_damping(0.001);
            canvas.attach_grapple("player", initial_grapple);

            // ── Register grab/release events + mouse handlers ────────────
            events::register_events(canvas, &state);

            // ── Pause menu button handlers (register once) ───────────────
            let pause_btns_registered = matches!(
                canvas.get_var("pause_btns_registered"),
                Some(Value::Bool(true))
            );
            if !pause_btns_registered {
                // Click handlers
                canvas.register_custom_event("pause_resume_click".into(), |c| {
                    if !matches!(c.get_var("game_paused"), Some(Value::Bool(true))) { return; }
                    // Trigger resume via synthetic "p" press logic
                    c.resume();
                    c.set_var("pause_animating", false);
                    c.set_var("pause_anim_frames", 0);
                    c.set_var("game_paused", false);
                    let trail = EmitterBuilder::new(PLAYER_TRAIL_EMITTER_NAME)
                        .rate(72.0).lifetime(0.68).velocity(-2.0, 8.0)
                        .spread(6.0, 6.0).size(9.0).color(170, 255, 170, 255)
                        .render_layer(2).gravity_scale(0.0)
                        .collision(CollisionResponse::None).build();
                    c.add_emitter(trail);
                    c.attach_emitter_to(PLAYER_TRAIL_EMITTER_NAME, "player");
                    if let Some(obj) = c.get_game_object_mut("player") { obj.visible = true; }
                    let was_hooked = matches!(c.get_var("rope_visible_at_pause"), Some(Value::Bool(true)));
                    if let Some(obj) = c.get_game_object_mut("rope") { obj.visible = was_hooked; }
                    for name in ["pause_overlay", "pause_title",
                                 "pause_resume_btn", "pause_respawn_btn",
                                 "pause_settings_btn", "pause_menu_btn"] {
                        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; }
                    }
                });
                canvas.register_custom_event("pause_respawn_click".into(), |c| {
                    if !matches!(c.get_var("game_paused"), Some(Value::Bool(true))) { return; }
                    c.resume();
                    c.set_var("game_paused", false);
                    c.load_scene("game");
                });
                canvas.register_custom_event("pause_menu_click".into(), |c| {
                    if !matches!(c.get_var("game_paused"), Some(Value::Bool(true))) { return; }
                    c.resume();
                    c.set_var("game_paused", false);
                    c.load_scene("menu");
                });
                canvas.register_custom_event("pause_settings_click".into(), |_c| {
                    // Settings: no-op for now
                });

                // Hover highlight handlers
                struct BtnStyle { name: &'static str, r: u8, g: u8, b: u8, label: &'static str }
                let styles: &[BtnStyle] = &[
                    BtnStyle { name: "pause_resume_btn",   r: 50,  g: 160, b: 90,  label: "RESUME" },
                    BtnStyle { name: "pause_respawn_btn",  r: 60,  g: 120, b: 200, label: "RESPAWN" },
                    BtnStyle { name: "pause_settings_btn", r: 80,  g: 80,  b: 100, label: "SETTINGS" },
                    BtnStyle { name: "pause_menu_btn",     r: 170, g: 65,  b: 65,  label: "MENU" },
                ];
                for s in styles {
                    let btn_name: String = s.name.into();
                    let (hr, hg, hb) = (s.r.saturating_add(40), s.g.saturating_add(40), s.b.saturating_add(40));
                    let hover_img = pause_btn_img(700, 120, hr, hg, hb, s.label);
                    let normal_img = pause_btn_img(700, 120, s.r, s.g, s.b, s.label);
                    let enter_name = format!("{}_enter", s.name);
                    let leave_name = format!("{}_leave", s.name);

                    let btn_enter = btn_name.clone();
                    canvas.register_custom_event(enter_name, move |c| {
                        if let Some(obj) = c.get_game_object_mut(&btn_enter) {
                            obj.set_image(Image {
                                shape: ShapeType::Rectangle(0.0, (700.0, 120.0), 0.0),
                                image: hover_img.clone().into(),
                                color: None,
                            });
                        }
                    });
                    let btn_leave = btn_name.clone();
                    canvas.register_custom_event(leave_name, move |c| {
                        if let Some(obj) = c.get_game_object_mut(&btn_leave) {
                            obj.set_image(Image {
                                shape: ShapeType::Rectangle(0.0, (700.0, 120.0), 0.0),
                                image: normal_img.clone().into(),
                                color: None,
                            });
                        }
                    });
                }
                canvas.set_var("pause_btns_registered", true);
            }

            // ── Main tick (register once) ────────────────────────────────
            let tick_registered = matches!(
                canvas.get_var("game_tick_registered"),
                Some(Value::Bool(true))
            );
            if !tick_registered {
                let st = state.clone();
                let mut space_was_down = false;
                let mut mouse_was_down = false;
                let mut prev_nearest_hook = String::new();
                let mut dark_mode_prev = false;
                let mut prev_bg_theme: Option<(bool, usize, bool, bool)> = None;
                let mut prev_palette_zone: usize = usize::MAX;
                let mut frame_counter: u32 = 0;

                let bg_s = bg_zone_start.clone();
                let bg_p = bg_zone_purple.clone();
                let bg_b = bg_zone_black.clone();
                let bg_sv = bg_zone_start_vivid.clone();
                let bg_pv = bg_zone_purple_vivid.clone();
                let bg_bv = bg_zone_black_vivid.clone();
                let bg_sf = bg_zone_start_flip.clone();
                let bg_pf = bg_zone_purple_flip.clone();
                let bg_bf = bg_zone_black_flip.clone();
                let bg_svf = bg_zone_start_vivid_flip.clone();
                let bg_pvf = bg_zone_purple_vivid_flip.clone();
                let bg_bvf = bg_zone_black_vivid_flip.clone();

                canvas.on_update(move |c| {
                    // ── Dead check ───────────────────────────────────────
                    {
                        let s = st.lock().unwrap();
                        if s.dead {
                            return;
                        }
                    }

                    // ── Camera-anchored UI ───────────────────────────────
                    let cam_x = c
                        .camera()
                        .map(|cam| cam.position.0)
                        .unwrap_or(0.0);
                    if let Some(obj) = c.get_game_object_mut("bg") {
                        obj.position = (0.0, 0.0);
                    }
                    let floor_y = {
                        let s = st.lock().unwrap();
                        if s.gravity_dir < 0.0 { 0.0 } else { VH - 28.0 }
                    };
                    if let Some(obj) = c.get_game_object_mut("danger_floor") {
                        obj.position = (0.0, floor_y);
                    }

                    // ── Pause entrance animation ─────────────────────────
                    if matches!(
                        c.get_var("pause_animating"),
                        Some(Value::Bool(true))
                    ) {
                        let mut remaining =
                            c.get_i32("pause_anim_frames").max(0);
                        let total = c.get_i32("pause_anim_total").max(1);

                        if remaining > 0 {
                            remaining -= 1;
                            let t =
                                1.0 - (remaining as f32 / total as f32);
                            let ease = 1.0 - (1.0 - t).powi(3);
                            let y = -VH + VH * ease;

                            if let Some(obj) =
                                c.get_game_object_mut("pause_overlay")
                            {
                                obj.position = (0.0, y);
                                obj.visible = true;
                            }
                            // Animate buttons alongside the overlay
                            let btn_layout: &[(&str, f32, f32)] = &[
                                ("pause_title", (VW - 650.0) / 2.0, VH * 0.20),
                                ("pause_resume_btn", (VW - 700.0) / 2.0, 820.0),
                                ("pause_respawn_btn", (VW - 700.0) / 2.0, 980.0),
                                ("pause_settings_btn", (VW - 700.0) / 2.0, 1140.0),
                                ("pause_menu_btn", (VW - 700.0) / 2.0, 1300.0),
                            ];
                            for &(name, bx, by) in btn_layout {
                                if let Some(obj) = c.get_game_object_mut(name) {
                                    obj.position = (bx, by + y);
                                    obj.visible = true;
                                }
                            }
                            c.set_var("pause_anim_frames", remaining);
                            if remaining == 0 {
                                if let Some(obj) =
                                    c.get_game_object_mut("pause_overlay")
                                {
                                    obj.position = (0.0, 0.0);
                                }
                                for &(name, bx, by) in btn_layout {
                                    if let Some(obj) = c.get_game_object_mut(name) {
                                        obj.position = (bx, by);
                                    }
                                }
                                c.set_var("pause_animating", false);
                                c.set_var("game_paused", true);
                                c.pause();
                            }
                            return;
                        }
                        c.set_var("pause_animating", false);
                    }

                    if c.is_paused()
                        || matches!(
                            c.get_var("game_paused"),
                            Some(Value::Bool(true))
                        )
                    {
                        if let Some(obj) =
                            c.get_game_object_mut("pause_overlay")
                        {
                            obj.position.0 = cam_x;
                        }
                        return;
                    }

                    // ── Input (grab / release) ──────────────────────────
                    // Spacebar and mouse are both polled here so they
                    // trigger at the same point in the frame.
                    let space_now = c.key("space");
                    let mouse_now = matches!(
                        c.get_var("mouse_left_held"),
                        Some(Value::Bool(true))
                    );
                    let action_now = space_now || mouse_now;
                    let action_was = space_was_down || mouse_was_down;
                    if action_now && !action_was {
                        c.run(Action::Custom {
                            name: "do_grab".into(),
                        });
                    } else if !action_now && action_was {
                        c.run(Action::Custom {
                            name: "do_release".into(),
                        });
                    }
                    space_was_down = space_now;
                    mouse_was_down = mouse_now;

                    // ── Speed-reactive trail ─────────────────────────────
                    {
                        let s = st.lock().unwrap();
                        let speed =
                            (s.vx * s.vx + s.vy * s.vy).sqrt();
                        let rate =
                            (62.0 + speed * 1.6).clamp(62.0, 150.0);
                        let life =
                            (0.62 + speed * 0.010).clamp(0.62, 0.95);
                        let size =
                            (8.0 + speed * 0.06).clamp(8.0, 12.0);
                        let spread =
                            (5.0 + speed * 0.05).clamp(5.0, 9.5);
                        let evx =
                            (-s.vx * 0.55).clamp(-34.0, 34.0);
                        let evy =
                            (-s.vy * 0.55).clamp(-34.0, 34.0);
                        drop(s);

                        c.run(Action::set_emitter_rate(
                            PLAYER_TRAIL_EMITTER_NAME,
                            rate,
                        ));
                        c.run(Action::set_emitter_lifetime(
                            PLAYER_TRAIL_EMITTER_NAME,
                            life,
                        ));
                        c.run(Action::set_emitter_size(
                            PLAYER_TRAIL_EMITTER_NAME,
                            size,
                        ));
                        c.run(Action::set_emitter_spread(
                            PLAYER_TRAIL_EMITTER_NAME,
                            spread,
                            spread,
                        ));
                        c.run(Action::set_emitter_velocity(
                            PLAYER_TRAIL_EMITTER_NAME,
                            evx,
                            evy,
                        ));
                        c.run(Action::set_emitter_color(
                            PLAYER_TRAIL_EMITTER_NAME,
                            170,
                            255,
                            170,
                            255,
                        ));
                    }

                    // ── Tick counters ────────────────────────────────────
                    {
                        let mut s = st.lock().unwrap();
                        s.ticks += 1;
                        if s.spinner_hit_cooldown > 0 {
                            s.spinner_hit_cooldown -= 1;
                        }
                        // Advance camera effect cooldown timers (1/60s per frame).
                        s.shake_cooldown.tick(1.0 / 60.0);
                        s.flash_cooldown.tick(1.0 / 60.0);
                    }
                    frame_counter = frame_counter.wrapping_add(1);

                    // ── Read player state from engine ────────────────────
                    {
                        let mut s = st.lock().unwrap();
                        physics::read_player_from_engine(c, &mut s);
                    }

                    // ── Rope visual (before spawning/collision) ─────
                    physics::tick_rope_visual(c, &st);

                    // ── Spawning ─────────────────────────────────────────
                    spawning::tick_spawning(
                        c,
                        &st,
                        &coin_spawn_image,
                        &coin_spawn_anim,
                    );

                    // ── Culling ──────────────────────────────────────────
                    culling::tick_culling(c, &st);

                    // ── Collision ────────────────────────────────────────
                    collision::tick_collision(c, &st);

                    // ── Pickups ──────────────────────────────────────────
                    pickups::tick_pickups(c, &st);

                    // ── Gravity wells ────────────────────────────────────
                    gravity_wells::tick_gravity_wells(
                        c,
                        &st,
                        frame_counter,
                    );

                    // ── Distance tracking ────────────────────────────────
                    {
                        let mut s = st.lock().unwrap();
                        let travelled = (s.px - SPAWN_X).max(0.0);
                        if travelled > s.distance {
                            s.distance = travelled;
                        }
                    }

                    // ── Write physics back to engine ─────────────────────
                    physics::cap_momentum_and_write_back(c, &st);
                    physics::sync_engine_gravity(c, &st);

                    // ── Visuals ──────────────────────────────────────────
                    visuals::tick_visuals(
                        c,
                        &st,
                        &mut prev_palette_zone,
                        &mut prev_nearest_hook,
                        &mut dark_mode_prev,
                        frame_counter,
                    );

                    // ── Coin magnet radius debug visual ──────────────────
                    {
                        let s = st.lock().unwrap();
                        let (px, py, debug) =
                            (s.px, s.py, s.magnet_debug);
                        drop(s);
                        if let Some(obj) =
                            c.get_game_object_mut("coin_magnet_radius")
                        {
                            obj.position = (
                                px - COIN_MAGNET_RADIUS,
                                py - COIN_MAGNET_RADIUS,
                            );
                            obj.visible = debug;
                        }
                    }

                    // ── HUD ──────────────────────────────────────────────
                    hud_update::tick_hud(c, &st);

                    // ── Camera flash overlay ─────────────────────────────
                    {
                        let flash_info = c.camera_mut().and_then(|cam| {
                            cam.effects.flash_color()
                        });
                        if let Some(obj) = c.get_game_object_mut("camera_flash_overlay") {
                            if let Some((color, alpha)) = flash_info {
                                let a = (color.3 as f32 * alpha).min(255.0) as u8;
                                let mut img = image::RgbaImage::new(1, 1);
                                img.put_pixel(0, 0, image::Rgba([color.0, color.1, color.2, a]));
                                obj.set_image(Image {
                                    shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
                                    image: img.into(),
                                    color: None,
                                });
                                obj.visible = true;
                            } else {
                                obj.visible = false;
                            }
                        }
                    }

                    // ── Background ───────────────────────────────────────
                    if matches!(c.get_var("bg_force_refresh"), Some(Value::Bool(true))) {
                        prev_bg_theme = None;
                        prev_palette_zone = usize::MAX;
                        prev_nearest_hook.clear();
                        c.set_var("bg_force_refresh", false);
                    }
                    background::tick_background(
                        c,
                        &st,
                        &mut prev_bg_theme,
                        &bg_s,
                        &bg_p,
                        &bg_b,
                        &bg_sv,
                        &bg_pv,
                        &bg_bv,
                        &bg_sf,
                        &bg_pf,
                        &bg_bf,
                        &bg_svf,
                        &bg_pvf,
                        &bg_bvf,
                    );

                    // ── Death check ──────────────────────────────────────
                    let mut s = st.lock().unwrap();
                    let dead_now = (s.gravity_dir > 0.0
                        && s.py > VH + 150.0)
                        || (s.gravity_dir < 0.0 && s.py < -150.0);
                    if dead_now {
                        c.set_var("last_distance", s.distance);
                        c.set_var("last_coins", s.coin_count as i32);
                        s.dead = true;
                        drop(s);
                        if let Some(cam) = c.camera_mut() {
                            cam.snap_zoom(1.0);
                        }
                        c.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
                        if let Some(obj) =
                            c.get_game_object_mut("player")
                        {
                            obj.visible = false;
                        }
                        if let Some(obj) =
                            c.get_game_object_mut("rope")
                        {
                            obj.visible = false;
                        }
                        c.load_scene("gameover");
                    }
                });
                canvas.set_var("game_tick_registered", true);
            }
        })
        .on_exit(|canvas| {
            canvas.run(Action::DetachEmitter {
                emitter_name: PLAYER_TRAIL_EMITTER_NAME.to_string(),
            });
            canvas.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
        })
}
