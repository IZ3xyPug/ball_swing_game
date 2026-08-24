// ── build_scene.rs — Thin dispatcher ──────────────────────────────────────
// All game logic lives in sibling modules. This file creates the scene,
// wires up callbacks, and dispatches the per-frame tick in order.

use quartz::*;
use quartz::plugin::terrain_collision::TerrainCollisionPlugin;
use std::sync::{Arc, Mutex, OnceLock};

use crate::achievements::*;
use crate::audio_state;
use crate::constants::*;
use crate::state::gen_hook_batch;
use crate::images::*;
use crate::objects::{ui_text_left_spec, ui_text_spec};
use crate::state::*;
use crate::shop::{SHOP_ROPE_COLORS, SHOP_TRAIL_COLORS, SHOP_BG_COLORS};

// ── Lazily-computed background images ────────────────────────────────────────
// Computed in a background thread at app start so the tutorial renders
// immediately. By the time the player navigates to the game scene the
// images are guaranteed to be ready.

struct BgImages {
    vivid:         image::RgbaImage,
    vivid_flip:    image::RgbaImage,
    palettes:      Vec<image::RgbaImage>,
    palettes_flip: Vec<image::RgbaImage>,
    space:         Arc<image::RgbaImage>,
    transparent_stars: Arc<image::RgbaImage>,
}

// SAFETY: image::RgbaImage and Arc<image::RgbaImage> are Send + Sync.
unsafe impl Send for BgImages {}
unsafe impl Sync for BgImages {}

static BG_IMAGES: OnceLock<BgImages> = OnceLock::new();

fn compute_bg_images() -> BgImages {
    let bg_w = VW as u32;
    let bg_h = VH as u32;
    let starfield = star_field(bg_w, bg_h, STARFIELD_STAR_COUNT, 0xCAFE_BABE);
    // Explicit type annotation lets Rust auto-deref Arc<RgbaImage> → &RgbaImage.
    let sf: &image::RgbaImage = &starfield.image;

    // Aurora image decoded once; Triangle filter is imperceptible for a bg
    // and is ~10× faster than Lanczos3.
    let grad_start = {
        let aurora = image::load_from_memory(include_bytes!("../../../assets/aurora_earth.gif"))
            .expect("aurora_earth.gif decode")
            .to_rgba8();
        image::imageops::resize(&aurora, bg_w, bg_h, image::imageops::FilterType::Triangle)
    };

    let grad_vivid = gradient_rect(bg_w, bg_h, (8, 26, 74), (104, 194, 255));
    let blend_h = bg_h / 8;

    let vivid = composite_starfield_gradient(sf, &grad_vivid, bg_w, bg_h, blend_h);

    let palettes: Vec<image::RgbaImage> = SHOP_BG_COLORS.iter().map(|&(pr, pg, pb)| {
        let mut tinted = grad_start.clone();
        for px in tinted.pixels_mut() {
            px[0] = (px[0] as f32 * 0.55 + pr as f32 * 0.45).min(255.0) as u8;
            px[1] = (px[1] as f32 * 0.55 + pg as f32 * 0.45).min(255.0) as u8;
            px[2] = (px[2] as f32 * 0.55 + pb as f32 * 0.45).min(255.0) as u8;
        }
        composite_starfield_gradient(sf, &tinted, bg_w, bg_h, blend_h)
    }).collect();
    let palettes_flip: Vec<image::RgbaImage> =
        palettes.iter().map(|img| flip_image_vertical(img)).collect();

    let space = starfield.image.clone();

    let transparent_stars = {
        let mut img: image::RgbaImage = (*sf).clone();
        for pixel in img.pixels_mut() {
            if pixel[0] < 20 && pixel[1] < 20 && pixel[2] < 25 {
                pixel[3] = 0;
            }
        }
        Arc::new(img)
    };

    let vivid_flip = flip_image_vertical(&vivid);

    BgImages { vivid, vivid_flip, palettes, palettes_flip, space, transparent_stars }
}

fn cached_coin_icon_anim() -> Option<AnimatedSprite> {
    static CACHE: OnceLock<Option<AnimatedSprite>> = OnceLock::new();
    CACHE.get_or_init(|| {
        AnimatedSprite::new(
            include_bytes!("../../../assets/catcoingold.gif"),
            (112.0, 112.0),
            12.0,
        ).ok()
    }).clone()
}

fn cached_score_x2_icon_anim() -> Option<AnimatedSprite> {
    static CACHE: OnceLock<Option<AnimatedSprite>> = OnceLock::new();
    CACHE.get_or_init(|| {
        AnimatedSprite::new(
            include_bytes!("../../../assets/2x.gif"),
            (120.0, 120.0),
            12.0,
        ).ok()
    }).clone()
}
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
use super::turrets;
use super::gravity_cannon;
use super::boss;
use super::helpers::*;
const PAUSE_MENU_ANIM_FRAMES: i32 = 14;
const PLAYER_TRAIL_EMITTER_NAME: &str = "player_trail";
const PLAYER_TRAIL_MID_NAME:  &str = "player_trail_b";

fn mid_trail_color(near: (u8, u8, u8)) -> (u8, u8, u8) {
    const ANCHOR: (f32, f32, f32) = (100.0, 120.0, 255.0);
    let b = |c: u8, a: f32| (c as f32 * 0.35 + a * 0.65).round() as u8;
    (b(near.0, ANCHOR.0), b(near.1, ANCHOR.1), b(near.2, ANCHOR.2))
}

/// Remove existing trail emitters, build fresh ones from `trail_color`, and attach to "player".
fn rebuild_player_trail(c: &mut Canvas, trail_color: (u8, u8, u8)) {
    let mid_color = mid_trail_color(trail_color);
    c.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
    c.remove_emitter(PLAYER_TRAIL_MID_NAME);
    let trail_near = EmitterBuilder::new(PLAYER_TRAIL_EMITTER_NAME)
        .rate(350.0).lifetime(0.18).velocity(0.0, 0.0)
        .spread(10.0, 10.0).size(45.0)
        .color(trail_color.0, trail_color.1, trail_color.2, 240)
        .color_end(trail_color.0, trail_color.1, trail_color.2, 0)
        .size_end(12.0).shape(ParticleShape::Circle)
        .interpolate_position(true)
        .render_layer(3).gravity_scale(0.0)
        .collision(CollisionResponse::None).build();
    let trail_mid = EmitterBuilder::new(PLAYER_TRAIL_MID_NAME)
        .rate(280.0).lifetime(0.40).velocity(0.0, 0.0)
        .spread(20.0, 20.0).size(32.0)
        .color(mid_color.0, mid_color.1, mid_color.2, 200)
        .color_end(mid_color.0, mid_color.1, mid_color.2, 0)
        .size_end(10.0).shape(ParticleShape::Circle)
        .interpolate_position(true)
        .render_layer(2).gravity_scale(0.0)
        .collision(CollisionResponse::None).build();
    c.add_emitter(trail_near);
    c.add_emitter(trail_mid);
    c.attach_emitter_to(PLAYER_TRAIL_EMITTER_NAME, "player");
    c.attach_emitter_to(PLAYER_TRAIL_MID_NAME, "player");
}

fn selected_trail_color(c: &Canvas) -> (u8, u8, u8) {
    let idx = match c.get_var("player_trail_selected") {
        Some(Value::I32(v)) => (v.max(0) as usize).min(SHOP_TRAIL_COLORS.len() - 1),
        _ => 0,
    };
    SHOP_TRAIL_COLORS[idx]
}
// Slider layout constants (must match bootstrap.rs SLIDER_Y / SLIDER_TRACK_W).
const SLIDER_TRACK_W: f32 = 1400.0;
const SLIDER_THUMB_W: f32 = 60.0;
const SLIDER_THUMB_H: f32 = 80.0;
const SLIDER_TRACK_H: f32 = 24.0;
const SLIDER_TRACK_X: f32 = (VW - SLIDER_TRACK_W) / 2.0;
const SLIDER_Y: [f32; 3] = [820.0, 1120.0, 1420.0];
const SLIDER_VARS:   [&str; 3] = ["vol_master", "vol_music", "vol_sound"];
const SLIDER_THUMBS: [&str; 3] = ["slider_master_thumb", "slider_music_thumb", "slider_sound_thumb"];
const SLIDER_TRACKS: [&str; 3] = ["slider_master_track", "slider_music_track", "slider_sound_track"];

fn position_slider_thumbs(c: &mut Canvas) {
    position_volume_sliders(c, SLIDER_THUMBS);
}

fn update_bgm_volume(c: &Canvas) {
    let base = c.get_f32("bgm_base_vol");
    if base > 0.0 {
        audio_state::set_game_bgm_volume(music_volume(c, base));
    }
}

fn update_settings_text(c: &mut Canvas) {
    update_volume_labels(c, ["settings_label_0", "settings_label_1", "settings_label_2"]);
}

fn hide_pause_ui(c: &mut Canvas) {
    for name in ["pause_overlay", "pause_title",
                 "pause_resume_btn", "pause_restart_btn",
                 "pause_settings_btn", "pause_menu_btn",
                 "start_prompt_text",
                 "settings_label_0", "settings_label_1", "settings_label_2",
                 "settings_back_btn",
                 "slider_master_track", "slider_master_thumb",
                 "slider_music_track",  "slider_music_thumb",
                 "slider_sound_track",  "slider_sound_thumb"] {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.clear_highlight();
        }
    }
}

fn clear_pause_state(c: &mut Canvas) {
    if !c.get_bool("game_paused") { return; }
    c.resume();
    c.set_var("pause_animating", false);
    c.set_var("pause_anim_frames", 0);
    c.set_var("pause_hover_idx", -1);
    c.set_var("settings_open", false);
    c.set_var("settings_dragging", -1i32);
    c.set_var("game_paused", false);
    hide_pause_ui(c);
}

const PAUSE_BTN_LAYOUT: [(&str, f32, f32); 5] = [
    ("pause_title",        (VW - 650.0) / 2.0, VH * 0.20),
    ("pause_resume_btn",   (VW - 700.0) / 2.0, 780.0),
    ("pause_restart_btn",  (VW - 700.0) / 2.0, 1000.0),
    ("pause_settings_btn", (VW - 700.0) / 2.0, 1220.0),
    ("pause_menu_btn",     (VW - 700.0) / 2.0, 1440.0),
];

fn switch_game_bgm(c: &mut Canvas, track_idx: i32, asset: &str, base_vol: f32) {
    if c.get_i32("bgm_track_index") != track_idx {
        let handle = c.play_sound_with(asset, SoundOptions::new().volume(music_volume(c, base_vol)).looping(true));
        audio_state::replace_game_bgm(handle);
        c.set_var("bgm_track_index", track_idx);
        c.set_var("bgm_base_vol", base_vol);
    }
}

pub fn build_game_scene(ctx: &mut Context) -> Scene {
    // Kick off background image computation immediately so the tutorial scene
    // renders without delay. By the time the player navigates to the game scene
    // the images are ready. The OnceLock guarantees exactly-once computation.
    std::thread::spawn(|| { BG_IMAGES.get_or_init(compute_bg_images); });

    // Build all game objects and pool structures.
    let (scene, pools) = bootstrap::build_scene_objects(ctx);

    let bootstrap::PoolSets {
        starter_names, pool_free, pad_free, spinner_free,
        coin_free, flip_free, score_x2_free, zero_g_free,
        gate_free, gwell_free, turret_free, bullet_free,
        coin_static_sprite, coin_anim_template, score_x2_anim_template: _,
        tech_bounce_static_img, tech_bounce_static_img_flipped,
        tech_bounce_anim_frames, tech_bounce_anim_frames_flipped,
        pad_thruster_static_img, pad_thruster_anim_template, pad_thruster_anim_template_flipped,
        rocket_pad_free, space_planet_free, space_hook_free,
        space_coin_free, space_blue_coin_free, space_bh_free,
        space_asteroid_free, space_red_coin_free, cannon_free,
        boss_bolt_free, boss_asteroid_ids, comet_free, warn_free,
    } = pools;

    // Starter hook positions (must match bootstrap.rs).
    let starter_hooks: &[(f32, f32)] = &[
        (START_HOOK_X,                              START_HOOK_Y),
        (START_HOOK_X + HOOK_FIXED_X_GAP,           VH * 0.30),
        (START_HOOK_X + HOOK_FIXED_X_GAP * 2.0,    VH * 0.46),
        (START_HOOK_X + HOOK_FIXED_X_GAP * 3.0,    VH * 0.34),
        (START_HOOK_X + HOOK_FIXED_X_GAP * 4.0,    VH * 0.52),
    ];

    // Persistent state arc — created on first enter, reused on respawns.
    let persistent_state: Arc<Mutex<Option<Arc<Mutex<State>>>>> =
        Arc::new(Mutex::new(None));
    scene
        .on_enter(move |canvas| {
            // ── Crystalline renderer ─────────────────────────────────────
            // Re-create the physics world on every game entry so there is no
            // stale solver state or leftover particles from a previous run.
            canvas.enable_crystalline();
            canvas.set_var("crystalline_ready", true);

            // ── Terrain collision plugin (dynamic outline support) ──────
            let terrain_collision_registered = matches!(
                canvas.get_var("terrain_collision_registered"),
                Some(Value::Bool(true))
            );
            if !terrain_collision_registered {
                canvas.add_plugin(TerrainCollisionPlugin::new());
                canvas.set_var("terrain_collision_registered", true);
            }
            // Pre-warm comet GIF + warning image OnceLocks in a background
            // thread so the first J-press has zero decode lag.
            std::thread::spawn(spawning::preload_comet_assets);

            // ── Player particle trail ────────────────────────────────────
            let trail_color = selected_trail_color(canvas);
            rebuild_player_trail(canvas, trail_color);

            // ── Camera ───────────────────────────────────────────────────
            let mut cam = Camera::new((1_000_000_000.0, VH), (VW, VH));
            cam.follow(Some(Target::name("player")));
            cam.lerp_speed = 0.10;
            canvas.set_camera(cam);
            if let Some(cam) = canvas.camera_mut() {
                cam.snap_zoom(1.0);
                cam.zoom_anchor = None;
            }
            canvas.set_var("coin_sfx_index", 0);
            canvas.set_var("space_zoom_mode", 3);
            canvas.set_var("asteroid_hooks_on", true);
            canvas.set_var("boss_mode_cleared", false);
            canvas.set_var("start_orbit_ticks", 0i32);
            canvas.set_var("start_follow_force_ticks", 0i32);
            canvas.set_var("start_zoom_recover_ticks", 0i32);
            canvas.set_var("zoom_anchor_y", VH);

            // ── Apply selected character to player ───────────────────────
            {
                let char_val = match canvas.get_var("player_char_selected") {
                    Some(Value::I32(v)) => v.max(0),
                    _ => 0,
                };
                let char_idx = (char_val as usize).min(PLAYER_CHAR_COLORS.len() - 1);
                if let Some(obj) = canvas.get_game_object_mut("player") {
                    if char_idx == 0 {
                        // Calico cat — keep (or restore) the animated sprite.
                        if obj.animated_sprite.is_none() {
                            if let Ok(mut calico) = AnimatedSprite::new(
                                include_bytes!("../../../assets/calicoball.gif"),
                                (PLAYER_R * 2.0, PLAYER_R * 2.0),
                                CALICO_FPS,
                            ) {
                                calico.set_fps(0.0);
                                obj.set_animation(calico);
                            }
                        }
                    } else {
                        // Solid colour circle — clear animation so it doesn't override drawable.
                        obj.animated_sprite = None;
                        let (cr, cg, cb) = PLAYER_CHAR_COLORS[char_idx];
                        obj.set_image(Image { shape: ShapeType::Ellipse(0.0, (PLAYER_R * 2.0, PLAYER_R * 2.0), 0.0), image: circle_cached(PLAYER_R as u32, cr, cg, cb), color: None });
                    }
                }
            }

            // ── Apply selected rope colour ───────────────────────────────
            {
                let rope_val = match canvas.get_var("player_rope_selected") {
                    Some(Value::I32(v)) => v.max(0) as usize,
                    _ => 0,
                }.min(SHOP_ROPE_COLORS.len() - 1);
                let (rr, rg, rb) = SHOP_ROPE_COLORS[rope_val];
                if let Some(obj) = canvas.get_game_object_mut("rope") {
                    obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (4.0, 4.0), 0.0), image: solid(rr, rg, rb, 255).into(), color: None });
                }
            }

            // ── Background music (looped, switchable) ───────────────────
            if !audio_state::has_game_bgm() {
                let handle = canvas.play_sound_with(
                    ASSET_BGM_TRACK_1,
                    SoundOptions::new().volume(music_volume(canvas, 0.084)).looping(true),
                );
                audio_state::replace_game_bgm(handle);
                canvas.set_var("bgm_track_index", 0);
                canvas.set_var("bgm_base_vol", 0.084_f32);
            }
            // Stop menu music when starting the game.
            audio_state::stop_menu_bgm();

            // ── Pause key (register once globally) ───────────────────────
            let pause_key_registered = matches!(
                canvas.get_var("pause_key_registered"),
                Some(Value::Bool(true))
            );
            if !pause_key_registered {
                let persistent_state_key = Arc::clone(&persistent_state);
                canvas.on_key_press(move |c, key| {
                    if !c.is_scene("game") { return; }

                    if *key == Key::Character("1".into()) {
                        if is_game_paused(c) { return; }

                        let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                        if let Some(state_arc) = state_opt {
                            let mut s = state_arc.lock().unwrap();
                            s.zero_g_timer = ZERO_G_DURATION;
                            let gdir = s.gravity_dir;
                            let hooked = s.hooked;
                            drop(s);

                            if !hooked {
                                if let Some(obj) = c.get_game_object_mut("player") {
                                    obj.gravity = GRAVITY * ZERO_G_GRAVITY_SCALE * gdir;
                                }
                            }
                        }
                        return;
                    }

                    if *key == Key::Character("2".into()) {
                        if !is_game_paused(c) {
                            c.set_var("manual_flip_queued", true);
                        }
                        return;
                    }

                    if *key == Key::Character("4".into()) {
                        c.set_var("death_sound_mode", 0i32); // 0 = man (default)
                        // Reduced space background zoom amount.
                        c.set_var("space_zoom_mode", 4);
                        return;
                    }

                    // Key '5': spawn a rocket pad just ahead of the player for testing.
                    if *key == Key::Character("5".into()) {
                        if is_game_paused(c) { return; }
                        let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                        if let Some(state_arc) = state_opt {
                            let mut s = state_arc.lock().unwrap();
                            if let Some(id) = s.rocket_pad_free.pop() {
                                let spawn_x = s.px + VW * 0.28;
                                let spawn_y = s.py + PLAYER_R * 2.0 + 10.0;
                                s.rocket_pad_live.push(id.clone());
                                drop(s);
                                if let Some(obj) = c.get_game_object_mut(&id) {
                                    obj.position = (spawn_x - ROCKET_PAD_W * 0.5, spawn_y);
                                    obj.visible = true;
                                }
                            }
                        }
                        return;
                    }

                    // Debug-spawn keys: o=special hook, k=extended hook, j=comet
                    if !is_game_paused(c) {
                        let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                        if let Some(state_arc) = state_opt {
                            match key {
                                Key::Character(k) if k == "o" => { let _ = spawning::spawn_debug_special_hook(c, &state_arc);  return; }
                                Key::Character(k) if k == "k" => { let _ = spawning::spawn_debug_extended_hook(c, &state_arc); return; }
                                Key::Character(k) if k == "j" => { let _ = spawning::spawn_debug_comet(c, &state_arc);         return; }
                                _ => {}
                            }
                        }
                    }

                    if *key == Key::Character("7".into()) { switch_game_bgm(c, 1, ASSET_BGM_TRACK_2, 0.167); return; }
                    if *key == Key::Character("8".into()) { switch_game_bgm(c, 2, ASSET_BGM_TRACK_3, 0.5);   return; }
                    if *key == Key::Character("9".into()) { switch_game_bgm(c, 3, ASSET_MENU_BGM_2,  0.18);  return; }

                    // ── God mode toggle (key '0') ────────────────────────
                    if *key == Key::Character("0".into()) {
                        if is_game_paused(c) { return; }
                        let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                        if let Some(state_arc) = state_opt {
                            let mut s = state_arc.lock().unwrap();
                            s.god_mode = !s.god_mode;
                            let gm = s.god_mode;
                            if gm {
                                s.hooked = false;
                                s.vx = 0.0;
                                s.vy = 0.0;
                            }
                            drop(s);
                            if gm {
                                if let Some(obj) = c.get_game_object_mut("player") {
                                    obj.momentum = (0.0, 0.0);
                                    obj.gravity = 0.0;
                                }
                                if let Some(obj) = c.get_game_object_mut("rope") {
                                    obj.visible = false;
                                }
                                if let Some(cam) = c.camera_mut() {
                                    cam.flash_with(Color(255, 220, 0, 160), 0.3, FlashMode::Pulse, FlashEase::Sharp, 0.7, 0.02);
                                }
                            }
                        }
                        return;
                    }

                    // ── Switch to arcade death sound (key '3') ──
                    if *key == Key::Character("3".into()) {
                        c.set_var("death_sound_mode", 1i32); // 1 = arcade
                        return;
                    }

                    // ── Settings toggle keys (only when settings panel is open) ──
                    if c.get_bool("settings_open") {
                        let adjust = match key {
                            Key::Character(ch) if ch == "a" => Some(("vol_master", -0.05f32)),
                            Key::Character(ch) if ch == "d" => Some(("vol_master",  0.05f32)),
                            Key::Character(ch) if ch == "j" => Some(("vol_music",  -0.05f32)),
                            Key::Character(ch) if ch == "l" => Some(("vol_music",   0.05f32)),
                            Key::Character(ch) if ch == "n" => Some(("vol_sound",  -0.05f32)),
                            Key::Character(ch) if ch == "m" => Some(("vol_sound",   0.05f32)),
                            _ => None
                        };
                        if let Some((var, delta)) = adjust {
                            let cur = volume_value(c, var, 1.0);
                            set_volume_value(c, var, cur + delta);
                            update_settings_text(c);
                            position_slider_thumbs(c);
                            update_bgm_volume(c);
                            return;
                        }
                    }

                    let is_pause = *key == Key::Character("p".into());
                    let is_space = *key == Key::Named(NamedKey::Space);

                    // ── Debug: G = spawn a gravity cannon ahead of the player ──
                    if *key == Key::Character("g".into()) {
                        if !is_game_paused(c) {
                            let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                            if let Some(state_arc) = state_opt {
                                gravity_cannon::debug_spawn_cannon(c, &state_arc);
                            }
                        }
                        return;
                    }

                    if !is_pause && !is_space { return; }

                    if is_game_paused(c) {
                        // Check before clearing the var whether this is an orbit-launch.
                        let is_orbit_launch = c.get_bool("start_prompt_active");
                        c.resume();
                        c.set_var("pause_animating", false);
                        c.set_var("pause_anim_frames", 0);
                        c.set_var("game_paused", false);
                        c.set_var("start_prompt_active", false);
                        rebuild_player_trail(c, selected_trail_color(c));
                        if let Some(obj) = c.get_game_object_mut("player") {
                            obj.visible = true;
                        }
                        // Only restore rope if the player was hooked when pause started.
                        let was_hooked = c.get_bool("rope_visible_at_pause");
                        if let Some(obj) = c.get_game_object_mut("rope") {
                            obj.visible = was_hooked;
                        }
                        // Hide pause overlay and buttons
                        hide_pause_ui(c);
                        c.set_var("settings_open", false);
                        c.set_var("settings_dragging", -1i32);

                        // If launching from orbit start, give the ball its tangential velocity
                        // and release the intro zoom so tick_zoom takes over naturally.
                        if is_orbit_launch {
                            let ticks = c.get_i32("start_orbit_ticks").max(0) as f32;
                            const ORBIT_R: f32 = 240.0;
                            const ORBIT_OMEGA: f32 = 0.038;
                            let theta = -std::f32::consts::FRAC_PI_2 - ORBIT_OMEGA * ticks;
                            // CCW visual in Y-down: vx = r*ω*sin(θ), vy = -r*ω*cos(θ)
                            let vx = ORBIT_R * ORBIT_OMEGA * theta.sin();
                            let vy = -(ORBIT_R * ORBIT_OMEGA * theta.cos());
                            let in_space;
                            let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                            if let Some(state_arc) = state_opt {
                                let mut s = state_arc.lock().unwrap();
                                s.vx = vx;
                                s.vy = vy;
                                s.hooked = false;
                                let gdir = s.gravity_dir;
                                in_space = s.in_space_mode;
                                s.space_stasis_active = false;
                                drop(s);
                                if let Some(obj) = c.get_game_object_mut("player") {
                                    obj.momentum = (vx, vy);
                                    obj.gravity = GRAVITY * gdir;
                                }
                            } else {
                                in_space = false;
                            }
                            if let Some(obj) = c.get_game_object_mut("rope") {
                                obj.visible = false;
                            }
                            if !in_space {
                                // Release intro zoom anchor; tick_zoom will lerp back to normal.
                                if let Some(cam) = c.camera_mut() {
                                    cam.zoom_anchor = None;
                                    cam.follow(Some(Target::name("player")));
                                    cam.snap_zoom(1.0);
                                }
                                // Force follow briefly to avoid any intro camera target desync.
                                c.set_var("start_follow_force_ticks", 180i32);
                                // Slow zoom recovery so the handoff feels smooth instead of abrupt.
                                c.set_var("start_zoom_recover_ticks", 0i32);
                            }
                            // In space stasis: space_zone::tick_space_camera manages the camera.
                        }
                    } else if is_pause {
                        let animating = matches!(
                            c.get_var("pause_animating"),
                            Some(Value::Bool(true))
                        );
                        if animating { return; }

                        c.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
                        c.remove_emitter(PLAYER_TRAIL_MID_NAME);
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
                        for &(name, bx, by) in PAUSE_BTN_LAYOUT.iter() {
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
            canvas.set_var("start_prompt_active", false);
            canvas.set_var("manual_flip_queued", false);
            canvas.set_var("mouse_grab_queued", false);
            canvas.set_var("mouse_release_queued", false);
            canvas.set_var("mouse_grab_x", 0.0f32);
            canvas.set_var("mouse_grab_y", 0.0f32);
            canvas.set_var("grab_from_mouse", false);
            canvas.set_var("bg_vivid", false);
            canvas.set_var("bg_force_refresh", true);
            canvas.set_var("pause_hover_idx", -1);
            canvas.set_var("settings_open", false);
            canvas.set_var("settings_dragging", -1i32);

            for var in ["vol_master", "vol_music", "vol_sound"] {
                if canvas.get_var(var).is_none() { canvas.set_var(var, 1.0f32); }
            }

            // Spawn toggles (all on by default; toggled via settings panel).
            for var in ["spawn_pads_on","spawn_spinners_on","spawn_coins_on","spawn_flips_on",
                        "spawn_score_x2_on","spawn_zero_g_on","spawn_gwells_on","spawn_turrets_on"] {
                canvas.set_var(var, true);
            }

            if canvas.get_var("level_nonce").is_none() {
                canvas.set_var("level_nonce", 0i32);
            }

            // ── Fresh game state ─────────────────────────────────────────
            let mut seed: u64 = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0xDEAD_BEEF);
            let level_nonce = canvas.get_i32("level_nonce").max(0) as u64;
            seed ^= level_nonce.wrapping_mul(0x9E37_79B9_7F4A_7C15);

            let gen_y = starter_hooks
                .last()
                .map(|(_, y)| *y)
                .unwrap_or(SPAWN_Y);
            let first_from = starter_hooks
                .last()
                .map(|(x, _)| *x)
                .unwrap_or(SPAWN_X + 2000.0);
            let mut gen_head_x = first_from;
            let mut gen_head_y = gen_y;
            let first_batch = gen_hook_batch(&mut seed, first_from, &mut gen_head_x, &mut gen_head_y, 0.0);
            let rightmost_x = starter_hooks
                .last()
                .map(|(x, _)| *x)
                .unwrap_or(SPAWN_X);

            let start_hook = starter_hooks
                .first()
                .copied()
                .unwrap_or((START_HOOK_X, START_HOOK_Y));
            // Ball starts in a counterclockwise orbit above the first grab node.
            const ORBIT_R: f32 = 240.0;
            let start_px = start_hook.0;
            let start_py = (start_hook.1 - ORBIT_R).clamp(PLAYER_R, VH - PLAYER_R);
            let start_rope_len = ORBIT_R;

            let coin_spawn_anim = coin_anim_template.clone();
            let coin_spawn_image = coin_static_sprite.clone();

            let fresh_state = State {
                px: start_px, py: start_py,
                vx: 0.0, vy: 0.0,
                hooked: false, hook_x: start_hook.0,
                hook_y: start_hook.1, rope_len: start_rope_len,
                active_hook: "hook_0".into(), distance: 0.0,
                score: 0, coin_count: 0,
                gravity_dir: 1.0, score_time_awards: 0,
                score_distance_awards: 0,
                seed,
                pending: first_batch, live_hooks: starter_names.clone(),
                pool_free: pool_free.clone(),
                rightmost_x,
                gen_head_x,
                gen_head_y,
                last_hook_y: f32::NEG_INFINITY,
                world_sampler: crate::poisson::PoissonSampler::new(600.0), dead: false,
                ticks: 0, pad_live: Vec::new(),
                pad_free: pad_free.clone(), pad_rightmost: SPAWN_X,
                pad_origins: Vec::new(), spinner_live: Vec::new(),
                spinner_free: spinner_free.clone(), spinner_rightmost: SPAWN_X + VW * 0.65,
                spinner_origins: Vec::new(),
                // Temporarily disable spinner collisions/behavior.
                spinners_enabled: true, spinner_spin_enabled: true,
                spinner_hit_cooldown: 0, coin_live: Vec::new(),
                coin_free: coin_free.clone(), coin_rightmost: SPAWN_X,
                coin_magnet_locked: Vec::new(), magnet_debug: false,
                flip_live: Vec::new(), flip_free: flip_free.clone(),
                flip_rightmost: SPAWN_X + VW * 1.1, flip_timer: 0,
                flip_magnet_locked: Vec::new(), score_x2_live: Vec::new(),
                score_x2_free: score_x2_free.clone(), score_x2_rightmost: SPAWN_X + VW * 1.35,
                score_x2_timer: 0, score_x2_magnet_locked: Vec::new(),
                zero_g_live: Vec::new(), zero_g_free: zero_g_free.clone(),
                zero_g_rightmost: SPAWN_X + VW * 1.6, zero_g_timer: 0,
                zero_g_magnet_locked: Vec::new(), gate_live: Vec::new(),
                gate_free: gate_free.clone(), gate_rightmost: SPAWN_X + VW * 1.0,
                gwell_live: Vec::new(), gwell_free: gwell_free.clone(),
                gwell_rightmost: SPAWN_X + VW * 2.0, gwell_timers: Vec::new(),
                turret_live: Vec::new(), turret_free: turret_free.clone(),
                turret_rightmost: SPAWN_X + 2000.0, turret_timers: Vec::new(),
                bullet_live: Vec::new(), bullet_free: bullet_free.clone(),
                dark_mode: false, god_mode: false,
                glow_flashes: Vec::new(), pad_bounce_anim: Vec::new(),
                spawn_animations: Vec::new(),

                hud_last_dist_fill:    u32::MAX, hud_last_coins:        u32::MAX,
                hud_last_py:           i32::MAX, hud_last_px:           i32::MAX,
                hud_last_flip_timer:   u32::MAX, hud_last_zero_g_timer: u32::MAX,
                hud_last_score_x2_timer: u32::MAX, hud_last_score:        u32::MAX,
                hud_coin_fade_ticks:   u32::MAX, hud_coin_alpha:        0,
                hud_last_coin_alpha:   0, hud_coin_base_img:     None,

                // Space zone
                in_space_mode:            false, space_launch_active:      false,
                space_settle_done:        false, space_welcome_ticks:      0,
                space_oxygen:             SPACE_OXYGEN_TICKS, space_return_delay:       0,
                space_cam_y:              0.0, space_entry_bg_scale:     1.0,

                rocket_pad_live:          Vec::new(),
                rocket_pad_free:          rocket_pad_free.clone(),
                rocket_pad_rightmost:     SPAWN_X,

                space_planet_live:        Vec::new(),
                space_planet_free:        space_planet_free.clone(),
                space_planet_rightmost:   SPAWN_X, space_planet_data:        Vec::new(),

                space_hook_live:          Vec::new(),
                space_hook_free:          space_hook_free.clone(),
                space_hook_rightmost:     SPAWN_X,

                space_coin_live:          Vec::new(),
                space_coin_free:          space_coin_free.clone(),
                space_coin_rightmost:     SPAWN_X,

                space_blue_coin_live:   Vec::new(),
                space_blue_coin_free:   space_blue_coin_free.clone(),

                space_blackhole_live:     Vec::new(),
                space_blackhole_free:     space_bh_free.clone(), space_blackhole_rightmost: SPAWN_X,
                space_blackhole_data:     Vec::new(),

                space_asteroid_live:      Vec::new(),
                space_asteroid_free:      space_asteroid_free.clone(),
                space_asteroid_rightmost: SPAWN_X,

                hud_last_oxygen:          u32::MAX,

                space_stasis_active:    false, space_stasis_hook_id:   String::new(),
                space_stasis_is_entry:  false,

                space_red_coin_live:    Vec::new(),
                space_red_coin_free:    space_red_coin_free.clone(),

                space_gwell_timers:     Vec::new(), space_bh_teleport_fx:   Vec::new(),
                space_orbit_locked_planet: String::new(), space_orbit_speed:       0.0,
                space_entry_px:         0.0, space_coin_spent:       Vec::new(),
                space_blue_coin_spent:  Vec::new(), space_red_coin_spent:   Vec::new(),
                solar_surface_ratio:    SOLAR_SURFACE_RATIO_DEFAULT, solar_anim_loaded:      false,
                solar_anim_pending:     None,

                score_active_block: i32::MIN, score_block_ticks:  0,
                score_dead_blocks:  std::collections::HashSet::new(),

                player_ball_frame:       0, player_ball_hit_rewind:  false,
                player_ball_frame_timer: 0,

                cannon_live:       Vec::new(), cannon_free:       cannon_free.clone(),
                cannon_rightmost:  SPAWN_X, cannon_phases:     Vec::new(),
                cannon_captured:   false, cannon_capture_id: String::new(),
                cannon_damp_timer: 0, boss_active:       false,
                boss_entry_ticks:  0, boss_spawned:      false,
                boss_cleared:      false, boss_hp:           crate::constants::BOSS_MAX_HP,
                boss_phase:        0.0, boss_vx:           0.0,
                boss_vy:           0.0, boss_shoot_timer:  crate::constants::BOSS_SHOOT_INTERVAL,
                boss_bolt_live:    Vec::new(), boss_bolt_free:    boss_bolt_free.clone(),
                boss_asteroids:    boss_asteroid_ids.clone(), hud_last_boss_hp:  -999,
                boss_dark_cooldown: BOSS_DARK_INTERVAL, boss_dark_ticks: 0,
                boss_dark_active:   false,

                comet_live:        Vec::new(), comet_free:        comet_free.clone(),

                comet_warn_live:   Vec::new(), warn_free:         warn_free.clone(),
                comet_spawn_timer: COMET_SPAWN_INTERVAL,

                hearts:            crate::constants::MAX_HEARTS,
                max_hearts:        crate::constants::MAX_HEARTS,
                checkpoint_x:      start_hook.0,
                checkpoint_y:      start_hook.1,
                checkpoint_block:  0,
                respawn_active:    false,
                respawn_ticks:     0,
                player_buff:       0,
                buff_timer:        0,
                buff_hit_flash:    0,
                flare_cooldown:    FLARE_INTERVAL,
                flare_warn:        0,
                flare_active:      false,
                flare_active_ticks: 0,
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

            if let Some(obj) = canvas.get_game_object_mut("player") {
                obj.position = (start_px - PLAYER_R, start_py - PLAYER_R);
                obj.momentum = (0.0, 0.0);
                obj.gravity = 0.0;
                obj.visible = true;
            }
            if let Some(rope_obj) = canvas.get_game_object_mut("rope") {
                rope_obj.visible = false;
            }
            canvas.set_var("rope_visible_at_pause", false);

            // Reset starter hooks to their canonical positions — they may have
            // been culled (hidden + moved off-screen) during a previous run.
            let asteroid_mode_reset = canvas.get_bool("asteroid_hooks_on");
            let hook_half_reset = if asteroid_mode_reset { HOOK_ARTIFACT_R } else { HOOK_R };
            for (i, &(hx, hy)) in starter_hooks.iter().enumerate() {
                let id = format!("hook_{i}");
                if let Some(obj) = canvas.get_game_object_mut(&id) {
                    obj.position = (hx - hook_half_reset, hy - hook_half_reset);
                    obj.visible = true;
                    obj.momentum = (0.0, 0.0);
                }
            }

            // Hide all asteroid objects from the previous run so they don't
            // appear as ghosts before the new run's spawner places them.
            for i in 0..SPACE_ASTEROID_POOL_SIZE {
                let id = format!("space_asteroid_{i}");
                if let Some(obj) = canvas.get_game_object_mut(&id) {
                    obj.visible = false;
                    obj.position = (-9800.0, -9800.0);
                    obj.momentum = (0.0, 0.0);
                    obj.rotation = 0.0;
                    obj.rotation_momentum = 0.0;
                    obj.gravity = 0.0;
                }
            }

            // Start paused with only tint + "hold space to begin".
            if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
                let s = canvas.virtual_scale();
                if let Some(obj) = canvas.get_game_object_mut("start_prompt_text") {
                    obj.set_drawable(Box::new(ui_text_spec(
                        "HOLD SPACE TO BEGIN",
                        &font,
                        52.0 * s,
                        Color(235, 245, 255, 255),
                        1300.0 * s,
                    )));
                    obj.visible = true;
                }

                if let Some(obj) = canvas.get_game_object_mut(GOLD_MASTER_TOAST_TITLE_NAME) {
                    obj.set_drawable(Box::new(ui_text_left_spec(
                        GOLD_MASTER_TITLE,
                        &font,
                        46.0 * s,
                        Color(250, 225, 120, 255),
                        1080.0 * s,
                    )));
                    obj.visible = false;
                }
                if let Some(obj) = canvas.get_game_object_mut(GOLD_MASTER_TOAST_DESC_NAME) {
                    obj.set_drawable(Box::new(ui_text_left_spec(
                        GOLD_MASTER_DESCRIPTION,
                        &font,
                        28.0 * s,
                        Color(210, 220, 235, 230),
                        1080.0 * s,
                    )));
                    obj.visible = false;
                }
                if let Some(obj) = canvas.get_game_object_mut(GOLD_MASTER_TOAST_CHECK_NAME) {
                    obj.set_drawable(Box::new(ui_text_spec(
                        "✓",
                        &font,
                        52.0 * s,
                        Color(130, 255, 165, 255),
                        120.0 * s,
                    )));
                    obj.visible = false;
                }
            }
            if let Some(obj) = canvas.get_game_object_mut("pause_overlay") {
                obj.position = (-400.0, 0.0);
                obj.visible = false;
            }

            clear_gold_master_toast(canvas);

            // Set background image AND apply the proper overscan/raise size so
            // the background fills the screen correctly from the first frame.
            {
                let bg_imgs = BG_IMAGES.get_or_init(compute_bg_images);
                let bg_sel = match canvas.get_var("player_bg_selected") {
                    Some(Value::I32(v)) => v.max(0) as usize,
                    _ => 0,
                }.min(bg_imgs.palettes.len().saturating_sub(1));
                if let Some(obj) = canvas.get_game_object_mut("bg") {
                    obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: bg_imgs.palettes[bg_sel].clone().into(), color: None });
                    const OVERSCAN: f32 = 200.0;
                    const BG_RAISE: f32 = 150.0;
                    let w = VW + OVERSCAN * 2.0;
                    let h = VH + BG_RAISE;
                    obj.size = (w, h);
                    obj.position = (-OVERSCAN, -BG_RAISE);
                    obj.update_image_shape();
                    obj.visible = true;
                }
            }
            for name in ["pause_title", "pause_resume_btn", "pause_restart_btn", "pause_settings_btn", "pause_menu_btn"] {
                if let Some(obj) = canvas.get_game_object_mut(name) {
                    obj.visible = false;
                }
            }

            // Pre-populate world objects so startup pause shows the full scene,
            // not just initial grab nodes.
            spawning::tick_spawning(
                canvas,
                &state,
                &coin_spawn_image,
                &coin_spawn_anim,
                &tech_bounce_static_img,
                &tech_bounce_static_img_flipped,
                &pad_thruster_static_img,
                pad_thruster_anim_template.as_ref(),
                pad_thruster_anim_template_flipped.as_ref(),
            );

            // Set up all live hooks for asteroid mode.
            {
                let hooks = state.lock().unwrap().live_hooks.clone();
                for hid in &hooks {
                    if let Some(obj) = canvas.get_game_object_mut(hid) {
                        obj.set_animation(hook_artifact_anim());
                        obj.size = (HOOK_ARTIFACT_R * 2.0, HOOK_ARTIFACT_R * 2.0);
                        obj.collision_mode = CollisionMode::NonPlatform;
                        obj.gravity = 0.0;
                        obj.momentum = (0.0, 0.0);
                        obj.rotation_momentum = 0.0;
                    }
                }
            }

            // Snap intro zoom in on hook_0 for the title screen orbit animation.
            if let Some(cam) = canvas.camera_mut() {
                cam.snap_zoom(1.30);
                cam.zoom_anchor = Some((start_hook.0, start_hook.1));
            }

            // Reset solar death flag and ceiling visibility for fresh run.
            canvas.set_var("died_to_sun", false);
            canvas.set_var("died_to_oxygen", false);
            canvas.set_var("heart_losses", 0i32);
            if let Some(obj) = canvas.get_game_object_mut("solar_ceiling") {
                obj.visible = false;
            }

            canvas.set_var("start_prompt_active", true);
            canvas.set_var("game_paused", true);
            // Do not hard-pause the engine here: hard-pause skips
            // apply_camera_transform, which can leave stale zoom from
            // the previous scene on screen. We gate gameplay with
            // `game_paused`/`start_prompt_active` instead.
            canvas.resume();

            // ── Pre-warm rope texture cache (background thread) ──────────
            physics::prewarm_rope_fx_cache();
            // Pre-warm solar GIF decode so corona is ready before space approach.
            super::space_zone::prewarm_solar_decode(&state);
            // Pre-warm catcoin GIF decode so first space coin spawn does not hitch.
            super::space_zone::prewarm_space_coin_decode();
            // Pre-warm artifact hook GIF decode (background thread) to avoid
            // per-spawn disk read and decode stalls during gameplay.
            std::thread::spawn(|| { super::helpers::prewarm_hook_artifact(); });
            std::thread::spawn(|| { super::helpers::prewarm_hook_artifact_green(); });
            std::thread::spawn(|| { super::helpers::prewarm_zero_g_overlay(); });

            // Assign overlay animations once so they're ready to play on demand.
            if let Some(obj) = canvas.get_game_object_mut("zero_g_overlay") {
                obj.set_animation(super::helpers::zero_g_overlay_anim());
            }
            // Animated catcoingold icon in the coin counter slot.
            if let Some(anim) = cached_coin_icon_anim() {
                if let Some(obj) = canvas.get_game_object_mut("coin_icon_anim") {
                    obj.set_animation(anim);
                }
            }
            // Ability icon animations (ZeroG.gif and 2x.gif shown in HUD near scoreboard).
            if let Some(obj) = canvas.get_game_object_mut("zero_g_icon") {
                obj.set_animation(super::helpers::zero_g_overlay_anim());
            }
            if let Some(anim) = cached_score_x2_icon_anim() {
                if let Some(obj) = canvas.get_game_object_mut("score_x2_icon") {
                    obj.set_animation(anim);
                }
            }
            if let Some(anim) = crate::images::space_rip_flip_anim_cached() {
                if let Some(obj) = canvas.get_game_object_mut("flip_icon") {
                    obj.set_animation(anim);
                }
            }

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
                    if !c.get_bool("game_paused") { return; }
                    clear_pause_state(c);
                    rebuild_player_trail(c, selected_trail_color(c));
                    if let Some(obj) = c.get_game_object_mut("player") { obj.visible = true; }
                    let was_hooked = c.get_bool("rope_visible_at_pause");
                    if let Some(obj) = c.get_game_object_mut("rope") { obj.visible = was_hooked; }
                });
                canvas.register_custom_event("pause_restart_click".into(), |c| {
                    if !c.get_bool("game_paused") { return; }
                    clear_pause_state(c);
                    let next = c.get_i32("level_nonce").saturating_add(1);
                    c.set_var("level_nonce", next);
                    c.load_scene("game");
                });
                canvas.register_custom_event("pause_menu_click".into(), |c| {
                    if !c.get_bool("game_paused") { return; }
                    clear_pause_state(c);
                    if let Some(cam) = c.camera_mut() {
                        cam.snap_zoom(1.0);
                        cam.zoom_anchor = None;
                    }
                    c.load_scene("menu");
                });
                canvas.register_custom_event("pause_settings_click".into(), |c| {
                    if !c.get_bool("game_paused") { return; }
                    // Hide pause menu buttons, show settings panel.
                    for name in ["pause_title", "pause_resume_btn", "pause_restart_btn",
                                 "pause_settings_btn", "pause_menu_btn"] {
                        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; }
                    }
                    c.set_var("settings_open", true);
                    c.set_var("settings_dragging", -1i32);
                    // Render label text (percentages only)
                    update_settings_text(c);
                    for name in ["settings_label_0", "settings_label_1", "settings_label_2"] {
                        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = true; }
                    }
                    if let Some(obj) = c.get_game_object_mut("settings_back_btn") {
                        obj.position = ((VW - 700.0) / 2.0, 1660.0);
                        obj.visible = true;
                    }
                    // Show slider tracks and thumbs at positions matching current vols
                    position_slider_thumbs(c);
                    for name in SLIDER_TRACKS.iter().chain(SLIDER_THUMBS.iter()) {
                        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = true; }
                    }
                });
                canvas.register_custom_event("settings_back_click".into(), |c| {
                    c.set_var("settings_open", false);
                    c.set_var("settings_dragging", -1i32);
                    for name in ["settings_label_0", "settings_label_1", "settings_label_2",
                                 "settings_back_btn"] {
                        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; }
                    }
                    for name in SLIDER_TRACKS.iter().chain(SLIDER_THUMBS.iter()) {
                        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; }
                    }
                    // Re-show pause menu.
                    for &(name, bx, by) in PAUSE_BTN_LAYOUT.iter() {
                        if let Some(obj) = c.get_game_object_mut(name) {
                            obj.position = (bx, by);
                            obj.visible = true;
                        }
                    }
                });

                // Pause UI uses ignore_zoom objects, so mouse hit-tests must
                // compensate for camera zoom (input pos is in world virtual space).

                let pause_ui_mouse_registered = matches!(
                    canvas.get_var("pause_ui_mouse_registered"),
                    Some(Value::Bool(true))
                );
                if !pause_ui_mouse_registered {
                    canvas.on_mouse_move({
                        move |c, pos| {
                            if !c.is_scene("game") { return; }
                            if !c.get_bool("game_paused") {
                                return;
                            }

                            // pos is world-space; multiply by zoom to get virtual-screen space
                            // so ignore_zoom UI hit tests are correct at any camera zoom level.
                            let zoom = c.camera().map(|cam| cam.zoom).unwrap_or(1.0);

                            // If dragging a settings slider, update its position/value.
                            let dragging = c.get_i32("settings_dragging");
                            if dragging >= 0 && c.get_bool("settings_open") {
                                let idx = dragging as usize;
                                if idx < 3 {
                                    let vol = ((pos.0 * zoom - SLIDER_TRACK_X) / SLIDER_TRACK_W).clamp(0.0, 1.0);
                                    set_volume_value(c, SLIDER_VARS[idx], vol);
                                    let thumb_x = SLIDER_TRACK_X + vol * (SLIDER_TRACK_W - SLIDER_THUMB_W);
                                    let thumb_y = SLIDER_Y[idx] - (SLIDER_THUMB_H - SLIDER_TRACK_H) / 2.0;
                                    if let Some(obj) = c.get_game_object_mut(SLIDER_THUMBS[idx]) {
                                        obj.position = (thumb_x, thumb_y);
                                    }
                                    // Sync so the renderer sees the new thumb position
                                    // while the engine is hard-paused.
                                    update_settings_text(c);
                                    update_bgm_volume(c);
                                }
                                return;
                            }

                            let ux = pos.0 * zoom;
                            let uy = pos.1 * zoom;
                            let bx = (VW - 700.0) / 2.0;

                            let over_resume = ux >= bx && ux <= bx + 700.0 && uy >= 780.0 && uy <= 950.0;
                            let over_restart = ux >= bx && ux <= bx + 700.0 && uy >= 1000.0 && uy <= 1170.0;
                            let over_settings = ux >= bx && ux <= bx + 700.0 && uy >= 1220.0 && uy <= 1390.0;
                            let over_menu = ux >= bx && ux <= bx + 700.0 && uy >= 1440.0 && uy <= 1610.0;
                            let over_back = ux >= bx && ux <= bx + 700.0 && uy >= 1660.0 && uy <= 1830.0;

                            let hover_idx = if over_resume {
                                0
                            } else if over_restart {
                                1
                            } else if over_settings {
                                2
                            } else if over_menu {
                                3
                            } else if over_back {
                                4
                            } else {
                                -1
                            };

                            let prev_idx = c.get_i32("pause_hover_idx");
                            if hover_idx == prev_idx {
                                return;
                            }
                            c.set_var("pause_hover_idx", hover_idx);

                            // Subtle but visible lighter hover state.
                            let hover_tint = Color(255, 255, 255, 92);
                            for (name, over) in [
                                ("pause_resume_btn",    over_resume),
                                ("pause_restart_btn",   over_restart),
                                ("pause_settings_btn",  over_settings),
                                ("pause_menu_btn",      over_menu),
                                ("settings_back_btn",   over_back),
                            ] {
                                if let Some(obj) = c.get_game_object_mut(name) {
                                    if over { obj.set_tint(hover_tint); } else { obj.clear_highlight(); }
                                }
                            }
                        }
                    });

                    canvas.on_mouse_press(move |c, btn, pos| {
                        if btn != MouseButton::Left { return; }
                        if !c.is_scene("game") { return; }
                        if !c.get_bool("game_paused") {
                            return;
                        }

                        // pos is world-space (divided by zoom); ignore_zoom UI lives in
                        // virtual-screen space (0..VW, 0..VH). Multiply by zoom to convert.
                        let zoom = c.camera().map(|cam| cam.zoom).unwrap_or(1.0);
                        let ux = pos.0 * zoom;
                        let uy = pos.1 * zoom;

                        // If settings panel is open, check for slider track hits first.
                        if c.get_bool("settings_open") {
                            if ux >= SLIDER_TRACK_X && ux <= SLIDER_TRACK_X + SLIDER_TRACK_W {
                                for idx in 0..3usize {
                                    if uy >= SLIDER_Y[idx] - 40.0 && uy <= SLIDER_Y[idx] + 64.0 {
                                        let vol = ((ux - SLIDER_TRACK_X) / SLIDER_TRACK_W).clamp(0.0, 1.0);
                                        set_volume_value(c, SLIDER_VARS[idx], vol);
                                        let thumb_x = SLIDER_TRACK_X + vol * (SLIDER_TRACK_W - SLIDER_THUMB_W);
                                        let thumb_y = SLIDER_Y[idx] - (SLIDER_THUMB_H - SLIDER_TRACK_H) / 2.0;
                                        if let Some(obj) = c.get_game_object_mut(SLIDER_THUMBS[idx]) {
                                            obj.position = (thumb_x, thumb_y);
                                        }
                                        update_settings_text(c);
                                        update_bgm_volume(c);
                                        c.set_var("settings_dragging", idx as i32);
                                        return;
                                    }
                                }
                            }
                        }

                        let bx = (VW - 700.0) / 2.0;

                        if ux >= bx && ux <= bx + 700.0 {
                            if uy >= 780.0 && uy <= 950.0 {
                                c.run(Action::Custom { name: "pause_resume_click".into() });
                            } else if uy >= 1000.0 && uy <= 1170.0 {
                                c.run(Action::Custom { name: "pause_restart_click".into() });
                            } else if uy >= 1220.0 && uy <= 1390.0 {
                                c.run(Action::Custom { name: "pause_settings_click".into() });
                            } else if uy >= 1440.0 && uy <= 1610.0 {
                                c.run(Action::Custom { name: "pause_menu_click".into() });
                            } else if uy >= 1660.0 && uy <= 1830.0 {
                                c.run(Action::Custom { name: "settings_back_click".into() });
                            }
                        }
                    });

                    canvas.on_mouse_release(move |c, _btn, _pos| {
                        if !c.is_scene("game") { return; }
                        c.set_var("settings_dragging", -1i32);
                    });

                    canvas.set_var("pause_ui_mouse_registered", true);
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
                let mut z_was_down = false;
                let mut prev_nearest_hook = String::new();
                let mut dark_mode_prev = false;
                let mut prev_bg_theme: Option<(bool, usize, bool, bool, bool)> = None;
                let mut prev_palette_zone: usize = usize::MAX;
                let mut frame_counter: u32 = 0;
                let mut bg_scale_smooth: f32 = 1.0;
                let mut prev_god_mode: bool = false;

                let mut star_shift: f32 = 0.0;
                let mut star_auto_scroll = true;
                let mut m_was_down = false;
                let mut n_was_down = false;
                let mut scroll_init = false;
                let mut prev_scroll_in_space: Option<bool> = None;
                let mut prev_player_center: Option<(f32, f32)> = None;

                canvas.on_update(move |c: &mut Canvas| {
                    if !c.is_scene("game") { return; }
                    let (px, py, vx, vy) = if let Some(player) = c.get_game_object("player") {
                        (
                            player.position.0 + player.size.0 * 0.5,
                            player.position.1 + player.size.1 * 0.5,
                            player.momentum.0,
                            player.momentum.1,
                        )
                    } else {
                        return;
                    };

                    let speed = (vx * vx + vy * vy).sqrt();
                    let Some(shield) = c.get_game_object_mut("airshield") else {
                        return;
                    };

                    if speed < AIRSHIELD_SPEED_THRESHOLD {
                        shield.visible = false;
                        prev_player_center = Some((px, py));
                        return;
                    }

                    // Direction source: post-crystalline net movement from solved
                    // position delta this frame. Momentum is fallback only.
                    let (mdx, mdy) = if let Some((lx, ly)) = prev_player_center {
                        (px - lx, py - ly)
                    } else {
                        (vx, vy)
                    };
                    prev_player_center = Some((px, py));

                    let motion_len = (mdx * mdx + mdy * mdy).sqrt();
                    let (dx, dy) = if motion_len > 0.001 {
                        (mdx / motion_len, mdy / motion_len)
                    } else if speed > f32::EPSILON {
                        (vx / speed, vy / speed)
                    } else {
                        (1.0, 0.0)
                    };

                    // Anchor the shield by its right-middle point (x=1.0, y=0.5),
                    // then rotate so that point always lies in front of net motion.
                    let ahead = PLAYER_R + AIRSHIELD_AHEAD_OFFSET;
                    let ax = px + dx * ahead;
                    let ay = py + dy * ahead;
                    let cx = ax - dx * (shield.size.0 * 0.5);
                    let cy = ay - dy * (shield.size.0 * 0.5);

                    shield.position = (cx - shield.size.0 * 0.5, cy - shield.size.1 * 0.5);
                    shield.rotation = dy.atan2(dx).to_degrees();
                    shield.visible = true;
                });
                let tech_bounce_img = tech_bounce_static_img.clone();
                let tech_bounce_img_flipped = tech_bounce_static_img_flipped.clone();
                let tech_bounce_anim = tech_bounce_anim_frames.clone();
                let tech_bounce_anim_flipped = tech_bounce_anim_frames_flipped.clone();
                let pad_thruster_static_img = pad_thruster_static_img.clone();
                let pad_thruster_anim_template = pad_thruster_anim_template.clone();
                let pad_thruster_anim_template_flipped = pad_thruster_anim_template_flipped.clone();

                canvas.on_update(move |c| {
                    if !c.is_scene("game") { return; }
                    // ── Dead check ───────────────────────────────────────
                    {
                        let s = st.lock().unwrap();
                        if s.dead {
                            return;
                        }
                    }

                    // ── Hearts / checkpoint bookkeeping ──────────────────
                    super::hearts::tick_hearts(c, &st);

                    // ── Camera-anchored UI ───────────────────────────────
                    let cam_x = c
                        .camera()
                        .map(|cam| cam.position.0)
                        .unwrap_or(0.0);
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
                                obj.position = (-400.0, y);
                                obj.visible = true;
                            }
                            // Animate buttons alongside the overlay
                            for &(name, bx, by) in PAUSE_BTN_LAYOUT.iter() {
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
                                    obj.position = (-400.0, 0.0);
                                }
                                for &(name, bx, by) in PAUSE_BTN_LAYOUT.iter() {
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

                    if is_game_paused(c) {
                        // ── Orbit animation while waiting for "hold space to begin" ──
                        if c.get_bool("start_prompt_active") {
                            let ticks = c.get_i32("start_orbit_ticks").max(0) as f32;
                            const ORBIT_R: f32 = 240.0;
                            const ORBIT_OMEGA: f32 = 0.038;
                            const INTRO_ZOOM: f32 = 1.30;
                            // Start at top (-π/2) and sweep CCW visually (decreasing θ in Y-down)
                            let theta = -std::f32::consts::FRAC_PI_2 - ORBIT_OMEGA * ticks;
                            let (hx, hy) = {
                                let s = st.lock().unwrap();
                                (s.hook_x, s.hook_y)
                            };
                            let px = hx + ORBIT_R * theta.cos();
                            let py = hy + ORBIT_R * theta.sin();
                            {
                                let mut s = st.lock().unwrap();
                                s.px = px;
                                s.py = py;
                            }
                            if let Some(obj) = c.get_game_object_mut("player") {
                                obj.position = (px - PLAYER_R, py - PLAYER_R);
                                obj.momentum = (0.0, 0.0);
                                obj.gravity = 0.0;
                            }
                            let in_space = { st.lock().unwrap().in_space_mode };
                            if in_space {
                                // Space stasis: keep space camera tracking the player.
                                super::space_zone::tick_space_camera_pub(c, &st);
                            } else {
                                // Normal intro orbit: maintain zoom anchored on hook.
                                if let Some(cam) = c.camera_mut() {
                                    cam.zoom_lerp_speed = 0.06;
                                    cam.zoom_anchor = Some((hx, hy));
                                    cam.smooth_zoom(INTRO_ZOOM);
                                }
                            }
                            c.set_var("start_orbit_ticks", ticks as i32 + 1);

                            // Keep asteroid-mode hooks frozen on the start screen.
                            // The physics engine still runs (soft pause only), so we
                            // zero every live hook's momentum each frame to prevent drift.
                            {
                                let hooks = st.lock().unwrap().live_hooks.clone();
                                for hid in &hooks {
                                    if let Some(obj) = c.get_game_object_mut(hid) {
                                        obj.momentum = (0.0, 0.0);
                                        obj.rotation_momentum = 0.0;
                                    }
                                }
                            }
                        }
                        if let Some(obj) =
                            c.get_game_object_mut("pause_overlay")
                        {
                            obj.position.0 = cam_x - 400.0;
                        }
                        return;
                    }

                    // ── Intro follow recovery window ────────────────────
                    let follow_force = c.get_i32("start_follow_force_ticks").max(0);
                    if follow_force > 0 {
                        if let Some(cam) = c.camera_mut() {
                            cam.follow(Some(Target::name("player")));
                            cam.lerp_speed = 0.10;
                        }
                        c.set_var("start_follow_force_ticks", follow_force - 1);
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
                    // After orbit launch, force "was" state to false so the held
                    // space press is seen as a fresh rising edge on the first gameplay
                    // frame — giving an immediate grab on that same space click.
                    if matches!(c.get_var("input_needs_edge_reset"), Some(Value::Bool(true))) {
                        space_was_down = false;
                        mouse_was_down = false;
                        c.set_var("input_needs_edge_reset", false);
                    }
                    let action_was = space_was_down || mouse_was_down;
                    if action_now && !action_was {
                        c.run(Action::Custom { name: "do_grab".into() });
                    } else if !action_now && action_was {
                        c.run(Action::Custom { name: "do_release".into() });
                    }
                    space_was_down = space_now;
                    mouse_was_down = mouse_now;

                    // ── Speed-reactive trail (rate only; size/lifetime/spread set at build time) ─
                    let speed = {
                        let s = st.lock().unwrap();
                        (s.vx * s.vx + s.vy * s.vy).sqrt()
                    };
                    let off = speed < 3.0;
                    c.run(Action::set_emitter_rate(PLAYER_TRAIL_EMITTER_NAME,
                        if off { 0.0 } else { (350.0 + speed * 5.0).clamp(0.0, 900.0) }));
                    c.run(Action::set_emitter_rate(PLAYER_TRAIL_MID_NAME,
                        if off { 0.0 } else { (280.0 + speed * 4.0).clamp(0.0, 720.0) }));
                    
                    // ── Tick counters ────────────────────────────────────
                    {
                        let mut s = st.lock().unwrap();
                        s.ticks += 1;
                        if s.spinner_hit_cooldown > 0 {
                            s.spinner_hit_cooldown -= 1;
                        }
                    }
                    frame_counter = frame_counter.wrapping_add(1);

                    // ── Read player state from engine ────────────────────
                    {
                        let mut s = st.lock().unwrap();
                        physics::read_player_from_engine(c, &mut s);
                    }

                    // ── Rope constraint (before spawning/collision) ──────
                    if !st.lock().unwrap().god_mode {
                        physics::tick_rope_constraint(c, &st);
                    }

                    // ── Spawning ─────────────────────────────────────────
                    spawning::tick_spawning(
                        c,
                        &st,
                        &coin_spawn_image,
                        &coin_spawn_anim,
                        &tech_bounce_img,
                        &tech_bounce_img_flipped,
                        &pad_thruster_static_img,
                        pad_thruster_anim_template.as_ref(),
                        pad_thruster_anim_template_flipped.as_ref(),
                    );

                    // ── Culling ──────────────────────────────────────────
                    culling::tick_culling(c, &st);

                    // ── Collision ────────────────────────────────────────
                    collision::tick_collision(c, &st);

                    // ── Asteroid-hook Y clamp ─────────────────────────────
                    // Prevent asteroid-mode hooks from drifting above y = -600.
                    if c.get_bool("asteroid_hooks_on") {
                        let live = st.lock().unwrap().live_hooks.clone();
                        for hid in &live {
                            if let Some(obj) = c.get_game_object_mut(hid) {
                                if obj.position.1 < -600.0 {
                                    obj.position.1 = -600.0;
                                    if obj.momentum.1 < 0.0 {
                                        obj.momentum.1 = 0.0;
                                    }
                                }
                            }
                        }
                    }

                    // ── Pickups ──────────────────────────────────────────
                    pickups::tick_pickups(c, &st, &tech_bounce_img, &tech_bounce_img_flipped, pad_thruster_anim_template.as_ref(), pad_thruster_anim_template_flipped.as_ref());

                    // ── Manual gravity flip (key '2') ───────────────────
                    if c.get_bool("manual_flip_queued") {
                        pickups::trigger_flip(c, &st, &tech_bounce_img, &tech_bounce_img_flipped, pad_thruster_anim_template.as_ref(), pad_thruster_anim_template_flipped.as_ref());
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
                        c.set_var("manual_flip_queued", false);
                    }

                    // ── Gravity wells ────────────────────────────────────
                    gravity_wells::tick_gravity_wells(
                        c,
                        &st,
                        frame_counter,
                    );

                    // ── Turrets ──────────────────────────────────────────
                    turrets::tick_turrets(c, &st);

                    // ── Gravity cannons ───────────────────────────────────
                    gravity_cannon::tick_cannons(c, &st);
                    // ── Boss fight ────────────────────────────────────────
                    boss::tick_boss(c, &st);

                    // ── Space zone ────────────────────────────────────────
                    super::space_zone::tick_space_zone(c, &st, frame_counter);

                    // ── Distance tracking ────────────────────────────────
                    {
                        let mut s = st.lock().unwrap();
                        if !s.in_space_mode && !s.space_launch_active {
                            let travelled = (s.px - SPAWN_X).max(0.0);
                            if travelled > s.distance {
                                s.distance = travelled;
                            }
                        }

                        // ── Dead-block passive score guard ───────────────
                        // Track how long the player has been in the same
                        // 5000-px block. After 12 s the block is "dead" and
                        // no longer yields passive time-score (even on return).
                        let current_block = (s.px / PASSIVE_SCORE_BLOCK_SIZE).floor() as i32;
                        if current_block == s.score_active_block {
                            s.score_block_ticks += 1;
                            if s.score_block_ticks >= PASSIVE_SCORE_DEAD_TICKS {
                                s.score_dead_blocks.insert(current_block);
                            }
                        } else {
                            s.score_active_block = current_block;
                            s.score_block_ticks = 0;
                        }
                        let block_is_dead = s.score_dead_blocks.contains(&current_block);

                        let time_awards = s.ticks / 60;
                        if time_awards > s.score_time_awards {
                            let gained = time_awards - s.score_time_awards;
                            s.score_time_awards = time_awards;
                            if !block_is_dead {
                                let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
                                s.score = s
                                    .score
                                    .saturating_add(gained.saturating_mul(10).saturating_mul(score_mult));
                            }
                        }

                        let distance_awards = (s.distance / 5000.0).floor() as u32;
                        if distance_awards > s.score_distance_awards {
                            let gained = distance_awards - s.score_distance_awards;
                            let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
                            s.score = s
                                .score
                                .saturating_add(gained.saturating_mul(200).saturating_mul(score_mult));
                            s.score_distance_awards = distance_awards;
                        }
                    }

                    // ── Write physics back to engine ─────────────────────
                    physics::cap_momentum_and_write_back(c, &st);

                    // ── God mode: free-fly override ───────────────────────
                    {
                        let gm = st.lock().unwrap().god_mode;
                        if gm {
                            // Z key toggles 4× speed boost (rising edge)
                            let z_now = c.key("z");
                            if z_now && !z_was_down {
                                let cur = matches!(c.get_var("god_mode_boost"), Some(Value::Bool(true)));
                                c.set_var("god_mode_boost", !cur);
                            }
                            z_was_down = z_now;
                            let boost = matches!(c.get_var("god_mode_boost"), Some(Value::Bool(true)));
                            const GOD_SPEED: f32 = 30.0;
                            let speed = if boost { GOD_SPEED * 4.0 } else { GOD_SPEED };
                            let dx = if c.key("d") { speed } else if c.key("a") { -speed } else { 0.0 };
                            let dy = if c.key("s") { speed } else if c.key("w") { -speed } else { 0.0 };
                            let mut s = st.lock().unwrap();
                            s.px += dx;
                            s.py += dy;
                            s.vx = 0.0;
                            s.vy = 0.0;
                            s.hooked = false;
                            let (px, py) = (s.px, s.py);
                            drop(s);
                            if let Some(obj) = c.get_game_object_mut("player") {
                                obj.position = (px - PLAYER_R, py - PLAYER_R);
                                obj.momentum = (0.0, 0.0);
                                obj.gravity = 0.0;
                            }
                            if let Some(obj) = c.get_game_object_mut("rope") {
                                obj.visible = false;
                            }
                        } else if prev_god_mode {
                            // God mode just turned OFF — restore engine physics immediately
                            // so the player doesn't stay frozen for an extra frame.
                            let s = st.lock().unwrap();
                            let gdir = s.gravity_dir;
                            let hooked = s.hooked;
                            drop(s);
                            let target_g = if hooked { 0.0 } else { GRAVITY * gdir };
                            if let Some(obj) = c.get_game_object_mut("player") {
                                obj.gravity = target_g;
                                // Give a tiny nudge so the engine's momentum integrator
                                // picks up on the change immediately.
                                obj.momentum = (0.0, GRAVITY * gdir * 0.5);
                            }
                        }
                        prev_god_mode = gm;
                    }

                    // ── Sync engine gravity ───────────────────────────────
                    if !st.lock().unwrap().god_mode {
                        physics::sync_engine_gravity(c, &st);
                    }

                    // ── Visuals ──────────────────────────────────────────
                    visuals::tick_visuals(
                        c,
                        &st,
                        &mut prev_palette_zone,
                        &mut prev_nearest_hook,
                        &mut dark_mode_prev,
                        frame_counter,
                        &tech_bounce_img,
                        &tech_bounce_anim,
                        &tech_bounce_img_flipped,
                        &tech_bounce_anim_flipped,
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

                    // ── Background ───────────────────────────────────────
                    if c.get_bool("bg_force_refresh") {
                        prev_bg_theme = None;
                        prev_palette_zone = usize::MAX;
                        prev_nearest_hook.clear();
                        scroll_init = false; // re-apply star panel images after respawn
                        c.set_var("bg_force_refresh", false);
                    }
                    {
                        let imgs = BG_IMAGES.get_or_init(compute_bg_images);
                        let bg_sel = match c.get_var("player_bg_selected") {
                            Some(Value::I32(v)) => v.max(0) as usize,
                            _ => 0,
                        }.min(imgs.palettes.len().saturating_sub(1));
                        background::tick_background(
                            c, &st, &mut prev_bg_theme, &mut bg_scale_smooth,
                            &imgs.palettes[bg_sel], &imgs.vivid,
                            &imgs.palettes_flip[bg_sel], &imgs.vivid_flip,
                        );
                    }

                    // ── Background star parallax ──────────────────────────
                    // Panels track bg's live size/position so stars scale exactly
                    // with the zoom effect. Modulus stays fixed at VW (constant)
                    // to prevent offset jumps; the result is then scaled to bg_w.
                    {
                        let in_space = { st.lock().unwrap().in_space_mode };
                        let m_now = c.key("m");
                        let n_now = c.key("n");
                        if m_now && !m_was_down { star_auto_scroll = true; }
                        if n_now && !n_was_down { star_auto_scroll = false; }
                        m_was_down = m_now;
                        n_was_down = n_now;

                        if star_auto_scroll {
                            star_shift += 0.75;
                        }

                        // Re-init when mode changes (space⇔normal) or on first run.
                        if prev_scroll_in_space != Some(in_space) {
                            scroll_init = false;
                            prev_scroll_in_space = Some(in_space);
                        }

                        // Read bg's current size/position (driven by bg_scale_smooth + gravity flip).
                        // In normal mode this expands as the player rises; in space it's VW+400.
                        let (bg_w, bg_h, bg_x, bg_y) = c.get_game_object("bg")
                            .map(|o| (o.size.0, o.size.1, o.position.0, o.position.1))
                            .unwrap_or((VW + 400.0, VH + 150.0, -200.0, -150.0));

                        // rem_euclid(VW) is stable (VW is a literal constant — never changes).
                        // Scaling by bg_w/VW maps the normalized offset into bg-space.
                        let offset = star_shift.rem_euclid(VW) * (bg_w / VW);

                        if !scroll_init {
                            // Space: opaque panels (same-image seamless, stars clearly visible).
                            // Normal: transparent overlay over aurora gradient.
                            let imgs = BG_IMAGES.get_or_init(compute_bg_images);
                            let img = if in_space {
                                Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: imgs.space.clone(), color: None }
                            } else {
                                Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: imgs.transparent_stars.clone(), color: None }
                            };
                            if let Some(obj) = c.get_game_object_mut("bg_space") {
                                obj.set_image(img.clone());
                            }
                            if let Some(obj) = c.get_game_object_mut("bg_stars_b") {
                                obj.set_image(img);
                            }
                            scroll_init = true;
                        }

                        // Panels sit at bg's x-anchor minus the scroll offset.
                        // Panel B is exactly one bg_w to the right — seamless wrap.
                        // Positions are rounded to whole pixels to prevent sub-pixel
                        // jitter / blur that is most visible during lag spikes.
                        let px = (bg_x - offset).round();
                        // Y offset pushes moving star panels lower on screen.
                        let py = (bg_y + 200.0).round();
                        if let Some(obj) = c.get_game_object_mut("bg_space") {
                            obj.size = (bg_w, bg_h);
                            obj.position = (px, py);
                            obj.visible = true;
                        }
                        if let Some(obj) = c.get_game_object_mut("bg_stars_b") {
                            obj.size = (bg_w, bg_h);
                            obj.position = (px + bg_w, py);
                            obj.visible = true;
                        }
                    }

                    // ── Death check ──────────────────────────────────────
                    let mut s = st.lock().unwrap();
                    // Solar death: set by tick_space_zone when player reaches solar ceiling.
                    let died_to_sun = c.get_bool("died_to_sun");
                    let died_to_oxygen = c.get_bool("died_to_oxygen");
                    let dead_now = !s.god_mode && (died_to_sun || died_to_oxygen || s.hearts <= 0
                        || (s.gravity_dir > 0.0
                        && s.py > VH + 150.0)
                        || (s.gravity_dir < 0.0 && s.py < -150.0));
                    if dead_now {
                        // ── Falls can be survived with hearts remaining ──
                        let is_fall = !died_to_sun && !died_to_oxygen;
                        if is_fall {
                            drop(s);
                            let over = super::hearts::lose_heart(c, &st);
                            if !over {
                                // Respawn at the last auto-progress checkpoint.
                                super::hearts::respawn(c, &st);
                                return;
                            }
                            // Last heart spent → fall through to game over.
                            s = st.lock().unwrap();
                        }

                        c.set_var("last_distance", s.distance);
                        c.set_var("last_score", s.score as i32);
                        c.set_var("last_coins", s.coin_count as i32);
                        s.dead = true;
                        drop(s);
                        if let Some(cam) = c.camera_mut() {
                            cam.snap_zoom(1.0);
                        }
                        c.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
                        c.remove_emitter(PLAYER_TRAIL_MID_NAME);
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
                        if died_to_sun {
                            c.set_var("died_to_sun", false);
                            c.set_var("died_to_oxygen", false);
                            play_death_sound(c);
                            c.load_scene("gameover_sun");
                        } else if died_to_oxygen {
                            c.set_var("died_to_oxygen", false);
                            play_death_sound(c);
                            c.load_scene("gameover_oxygen");
                        } else {
                            if !died_to_oxygen {
                                c.set_var("died_to_oxygen", false);
                            }
                            play_death_sound(c);
                            c.load_scene("gameover");
                        }
                    }
                });
                canvas.set_var("game_tick_registered", true);
            }
        })
        .on_exit(move |canvas| {
            canvas.run(Action::DetachEmitter {
                emitter_name: PLAYER_TRAIL_EMITTER_NAME.to_string(),
            });
            canvas.run(Action::DetachEmitter {
                emitter_name: PLAYER_TRAIL_MID_NAME.to_string(),
            });
            canvas.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
            canvas.remove_emitter(PLAYER_TRAIL_MID_NAME);
        })
}
