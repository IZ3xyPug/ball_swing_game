// ── build_scene.rs — Thin dispatcher ──────────────────────────────────────
// All game logic lives in sibling modules. This file creates the scene,
// wires up callbacks, and dispatches the per-frame tick in order.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::audio_state;
use crate::constants::*;
use crate::gameplay::zone_index_for_distance;
use crate::state::gen_hook_batch;
use crate::images::*;
use crate::objects::ui_text_spec;
use crate::state::*;
use crate::shop::{SHOP_ROPE_COLORS, SHOP_TRAIL_COLORS, SHOP_BG_COLORS};
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
use super::helpers::*;

const PAUSE_MENU_ANIM_FRAMES: i32 = 14;
const PLAYER_TRAIL_EMITTER_NAME: &str = "player_trail";

fn volume_value(c: &Canvas, var: &str, default: f32) -> f32 {
    match c.get_var(var) {
        Some(Value::F32(v)) => v.clamp(0.0, 1.0),
        _ => default,
    }
}

fn set_volume_value(c: &mut Canvas, var: &str, v: f32) {
    c.set_var(var, v.clamp(0.0, 1.0));
}

fn game_music_volume(c: &Canvas, base: f32) -> f32 {
    let master = volume_value(c, "vol_master", 1.0);
    let music = volume_value(c, "vol_music", 1.0);
    (base * master * music).clamp(0.0, 1.0)
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
    for i in 0..3 {
        let vol = volume_value(c, SLIDER_VARS[i], 1.0);
        let thumb_x = SLIDER_TRACK_X + vol * (SLIDER_TRACK_W - SLIDER_THUMB_W);
        let thumb_y = SLIDER_Y[i] - (SLIDER_THUMB_H - SLIDER_TRACK_H) / 2.0;
        if let Some(obj) = c.get_game_object_mut(SLIDER_THUMBS[i]) {
            obj.position = (thumb_x, thumb_y);
        }
    }
    // Engine may be hard-paused — sync offsets so the renderer sees new positions.
    c.sync_ignore_zoom_offsets();
}

fn update_bgm_volume(c: &Canvas) {
    let base = c.get_f32("bgm_base_vol");
    if base > 0.0 {
        audio_state::set_game_bgm_volume(game_music_volume(c, base));
    }
}

/// Returns a cached copy of the UI font — parses the TTF once, clones cheaply after.
fn settings_font() -> Option<Font> {
    use std::sync::OnceLock;
    static CACHED: OnceLock<Font> = OnceLock::new();
    CACHED.get_or_init(||
        Font::from_bytes(include_bytes!("../../../assets/font.ttf"))
            .expect("font.ttf must be valid")
    ).clone().into()
}

fn update_settings_text(c: &mut Canvas) {
    let master = volume_value(c, "vol_master", 1.0);
    let music  = volume_value(c, "vol_music",  1.0);
    let sound  = volume_value(c, "vol_sound",  1.0);
    let labels = [
        format!("MASTER VOLUME   {:>3}%", (master * 100.0).round() as i32),
        format!("MUSIC VOLUME    {:>3}%",  (music  * 100.0).round() as i32),
        format!("SOUND VOLUME    {:>3}%",  (sound  * 100.0).round() as i32),
    ];
    let names = ["settings_label_0", "settings_label_1", "settings_label_2"];
    if let Some(font) = settings_font() {
        let s = c.virtual_scale();
        for i in 0..3 {
            if let Some(obj) = c.get_game_object_mut(names[i]) {
                obj.set_drawable(Box::new(ui_text_spec(
                    &labels[i], &font, 38.0 * s, Color(235, 245, 255, 255), 1500.0 * s,
                )));
            }
        }
    }
}

pub fn build_game_scene(ctx: &mut Context) -> Scene {
    // Pre-compute background gradient images (small tile, stretched by GPU).
    // Generate the starfield once, then composite it into the upper half of each gradient.
    let bg_w = VW as u32;
    let bg_h = VH as u32;
    let starfield_quartz = star_field(bg_w, bg_h, STARFIELD_STAR_COUNT, 0xCAFE_BABE);
    let starfield_rgba: &image::RgbaImage = &starfield_quartz.image;

    let grad_start = {
        let aurora_src = image::load_from_memory(include_bytes!("../../../assets/aurora_earth.gif"))
            .expect("aurora_earth.gif decode failed")
            .to_rgba8();
        image::imageops::resize(&aurora_src, bg_w, bg_h, image::imageops::FilterType::Lanczos3)
    };
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

    // Per-palette aurora backgrounds for the background shop category.
    // Each entry is the aurora_earth tinted toward the corresponding SHOP_BG_COLORS palette.
    let bg_zone_start_palettes: Arc<Vec<image::RgbaImage>> = Arc::new(
        SHOP_BG_COLORS.iter().map(|&(pr, pg, pb)| {
            let mut tinted = grad_start.clone();
            for px in tinted.pixels_mut() {
                px[0] = (px[0] as f32 * 0.55 + pr as f32 * 0.45).min(255.0) as u8;
                px[1] = (px[1] as f32 * 0.55 + pg as f32 * 0.45).min(255.0) as u8;
                px[2] = (px[2] as f32 * 0.55 + pb as f32 * 0.45).min(255.0) as u8;
            }
            composite_starfield_gradient(starfield_rgba, &tinted, bg_w, bg_h, blend_h)
        }).collect()
    );
    let bg_zone_start_palettes_flip: Arc<Vec<image::RgbaImage>> = Arc::new(
        bg_zone_start_palettes.iter().map(|img| flip_image_vertical(img)).collect()
    );

    // Reuse the existing VW×VH starfield Arc for space mode (opaque).
    let bg_space_img_arc = starfield_quartz.image.clone(); // Arc<RgbaImage>, no copy

    // Transparent-background starfield for the normal-mode scroll overlay.
    // Post-process the opaque starfield: make the near-black sky pixels alpha=0.
    // Stars (brighter pixels) remain fully opaque. The transparent background ensures
    // both panels are seamless at any seam position — no aurora-mismatch artifact.
    let transparent_star_arc: std::sync::Arc<image::RgbaImage> = {
        let mut img: image::RgbaImage = (*starfield_rgba).clone();
        for pixel in img.pixels_mut() {
            if pixel[0] < 20 && pixel[1] < 20 && pixel[2] < 25 {
                pixel[3] = 0;
            }
        }
        std::sync::Arc::new(img)
    };
    // Tiny 1-px placeholder images so tick_background's signature is unchanged.
    let bg_zone_start_space        = solid(5, 5, 15, 255);
    let bg_zone_purple_space       = solid(5, 5, 15, 255);
    let bg_zone_black_space        = solid(5, 5, 15, 255);
    let bg_zone_start_vivid_space  = solid(5, 5, 15, 255);
    let bg_zone_purple_vivid_space = solid(5, 5, 15, 255);
    let bg_zone_black_vivid_space  = solid(5, 5, 15, 255);

    // Pre-compute vertically flipped backgrounds for reverse gravity.
    // (bg_zone_start_flip is handled per-palette via bg_zone_start_palettes_flip)
    let bg_zone_purple_flip = flip_image_vertical(&bg_zone_purple);
    let bg_zone_black_flip = flip_image_vertical(&bg_zone_black);
    let bg_zone_start_vivid_flip = flip_image_vertical(&bg_zone_start_vivid);
    let bg_zone_purple_vivid_flip = flip_image_vertical(&bg_zone_purple_vivid);
    let bg_zone_black_vivid_flip = flip_image_vertical(&bg_zone_black_vivid);
    let bg_zone_start_space_flip = flip_image_vertical(&bg_zone_start_space);
    let bg_zone_purple_space_flip = flip_image_vertical(&bg_zone_purple_space);
    let bg_zone_black_space_flip = flip_image_vertical(&bg_zone_black_space);
    let bg_zone_start_vivid_space_flip = flip_image_vertical(&bg_zone_start_vivid_space);
    let bg_zone_purple_vivid_space_flip = flip_image_vertical(&bg_zone_purple_vivid_space);
    let bg_zone_black_vivid_space_flip = flip_image_vertical(&bg_zone_black_vivid_space);

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
        turret_free,
        bullet_free,
        coin_static_sprite,
        coin_anim_template,
        score_x2_anim_template: _,
        tech_bounce_static_img,
        tech_bounce_static_img_flipped,
        tech_bounce_anim_frames,
        tech_bounce_anim_frames_flipped,
        pad_thruster_static_img,
        pad_thruster_anim_template,
        pad_thruster_anim_template_flipped,
        rocket_pad_free,
        space_planet_free,
        space_hook_free,
        space_coin_free,
        space_blue_coin_free,
        space_bh_free,
        space_asteroid_free,
        space_red_coin_free,
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

            // ── Player particle trail ────────────────────────────────────
            // Determine selected trail colour first so it can be used here and on resume.
            let trail_color = {
                let trail_val = match canvas.get_var("player_trail_selected") {
                    Some(Value::I32(v)) => v.max(0) as usize,
                    _ => 0,
                }.min(SHOP_TRAIL_COLORS.len() - 1);
                SHOP_TRAIL_COLORS[trail_val]
            };
            canvas.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
            let player_trail = EmitterBuilder::new(PLAYER_TRAIL_EMITTER_NAME)
                .rate(72.0)
                .lifetime(0.68)
                .velocity(-2.0, 8.0)
                .spread(6.0, 6.0)
                .size(9.0)
                .color(trail_color.0, trail_color.1, trail_color.2, 255)
                .render_layer(2)
                .gravity_scale(0.0)
                .collision(CollisionResponse::None)
                .build();
            canvas.add_emitter(player_trail);
            canvas.attach_emitter_to(PLAYER_TRAIL_EMITTER_NAME, "player");

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
                        obj.set_image(Image {
                            shape: ShapeType::Ellipse(0.0, (PLAYER_R * 2.0, PLAYER_R * 2.0), 0.0),
                            image: circle_cached(PLAYER_R as u32, cr, cg, cb),
                            color: None,
                        });
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
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (4.0, 4.0), 0.0),
                        image: solid(rr, rg, rb, 255).into(),
                        color: None,
                    });
                }
            }

            // ── Background music (looped, switchable) ───────────────────
            if !audio_state::has_game_bgm() {
                let handle = canvas.play_sound_with(
                    ASSET_BGM_TRACK_1,
                    SoundOptions::new().volume(game_music_volume(canvas, 0.084)).looping(true),
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
                        let game_paused = c.is_paused()
                            || matches!(c.get_var("game_paused"), Some(Value::Bool(true)));
                        if game_paused {
                            return;
                        }

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
                        let game_paused = c.is_paused()
                            || matches!(c.get_var("game_paused"), Some(Value::Bool(true)));
                        if !game_paused {
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
                        let game_paused = c.is_paused()
                            || matches!(c.get_var("game_paused"), Some(Value::Bool(true)));
                        if game_paused { return; }
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

                    if *key == Key::Character("7".into()) {
                        if c.get_i32("bgm_track_index") != 1 {
                            let handle = c.play_sound_with(
                                ASSET_BGM_TRACK_2,
                                SoundOptions::new().volume(game_music_volume(c, 0.167)).looping(true),
                            );
                            audio_state::replace_game_bgm(handle);
                            c.set_var("bgm_track_index", 1);
                            c.set_var("bgm_base_vol", 0.167_f32);
                        }
                        return;
                    }

                    if *key == Key::Character("8".into()) {
                        if c.get_i32("bgm_track_index") != 2 {
                            let handle = c.play_sound_with(
                                ASSET_BGM_TRACK_3,
                                SoundOptions::new().volume(game_music_volume(c, 0.5)).looping(true),
                            );
                            audio_state::replace_game_bgm(handle);
                            c.set_var("bgm_track_index", 2);
                            c.set_var("bgm_base_vol", 0.5_f32);
                        }
                        return;
                    }

                    if *key == Key::Character("9".into()) {
                        if c.get_i32("bgm_track_index") != 3 {
                            let handle = c.play_sound_with(
                                ASSET_MENU_BGM_2,
                                SoundOptions::new().volume(game_music_volume(c, 0.18)).looping(true),
                            );
                            audio_state::replace_game_bgm(handle);
                            c.set_var("bgm_track_index", 3);
                            c.set_var("bgm_base_vol", 0.18_f32);
                        }
                        return;
                    }

                    // ── God mode toggle (key '0') ────────────────────────
                    if *key == Key::Character("0".into()) {
                        let game_paused = c.is_paused()
                            || matches!(c.get_var("game_paused"), Some(Value::Bool(true)));
                        if game_paused { return; }
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
                    if matches!(c.get_var("settings_open"), Some(Value::Bool(true))) {
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
                    if !is_pause && !is_space { return; }

                    let game_paused = c.is_paused()
                        || matches!(c.get_var("game_paused"), Some(Value::Bool(true)));

                    if game_paused {
                        // Check before clearing the var whether this is an orbit-launch.
                        let is_orbit_launch = matches!(
                            c.get_var("start_prompt_active"),
                            Some(Value::Bool(true))
                        );
                        c.resume();
                        c.set_var("pause_animating", false);
                        c.set_var("pause_anim_frames", 0);
                        c.set_var("game_paused", false);
                        c.set_var("start_prompt_active", false);
                        let tc = {
                            let tv = match c.get_var("player_trail_selected") {
                                Some(Value::I32(v)) => v.max(0) as usize,
                                _ => 0,
                            }.min(SHOP_TRAIL_COLORS.len() - 1);
                            SHOP_TRAIL_COLORS[tv]
                        };
                        let trail = EmitterBuilder::new(PLAYER_TRAIL_EMITTER_NAME)
                            .rate(72.0)
                            .lifetime(0.68)
                            .velocity(-2.0, 8.0)
                            .spread(6.0, 6.0)
                            .size(9.0)
                            .color(tc.0, tc.1, tc.2, 255)
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
                            ("pause_resume_btn", (VW - 700.0) / 2.0, 780.0),
                            ("pause_restart_btn", (VW - 700.0) / 2.0, 1000.0),
                            ("pause_settings_btn", (VW - 700.0) / 2.0, 1220.0),
                            ("pause_menu_btn", (VW - 700.0) / 2.0, 1440.0),
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

            if canvas.get_var("vol_master").is_none() {
                canvas.set_var("vol_master", 1.0f32);
            }
            if canvas.get_var("vol_music").is_none() {
                canvas.set_var("vol_music", 1.0f32);
            }
            if canvas.get_var("vol_sound").is_none() {
                canvas.set_var("vol_sound", 1.0f32);
            }

            // Spawn toggles (all on by default; toggled via settings panel).
            canvas.set_var("spawn_pads_on", true);
            canvas.set_var("spawn_spinners_on", true);
            canvas.set_var("spawn_coins_on", true);
            canvas.set_var("spawn_flips_on", true);
            canvas.set_var("spawn_score_x2_on", true);
            canvas.set_var("spawn_zero_g_on", true);
            canvas.set_var("spawn_gwells_on", true);
            canvas.set_var("spawn_turrets_on", true);

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
            const ORBIT_OMEGA: f32 = 0.038; // rad/frame, CCW visual (Y-down)
            let start_px = start_hook.0;
            let start_py = (start_hook.1 - ORBIT_R).clamp(PLAYER_R, VH - PLAYER_R);
            let start_rope_len = ORBIT_R;

            let coin_spawn_anim = coin_anim_template.clone();
            let coin_spawn_image = coin_static_sprite.clone();

            let fresh_state = State {
                px: start_px,
                py: start_py,
                vx: 0.0,
                vy: 0.0,
                hooked: false,
                hook_x: start_hook.0,
                hook_y: start_hook.1,
                rope_len: start_rope_len,
                active_hook: "hook_0".into(),
                distance: 0.0,
                score: 0,
                coin_count: 0,
                gravity_dir: 1.0,
                score_time_awards: 0,
                score_distance_awards: 0,
                seed,
                pending: first_batch,
                live_hooks: starter_names.clone(),
                pool_free: pool_free.clone(),
                gen_y,
                rightmost_x,
                gen_head_x,
                gen_head_y,
                last_hook_y: f32::NEG_INFINITY,
                world_sampler: crate::poisson::PoissonSampler::new(600.0),
                dead: false,
                ticks: 0,
                pad_live: Vec::new(),
                pad_free: pad_free.clone(),
                pad_rightmost: SPAWN_X,
                pad_origins: Vec::new(),
                spinner_live: Vec::new(),
                spinner_free: spinner_free.clone(),
                spinner_rightmost: SPAWN_X + VW * 0.65,
                spinner_origins: Vec::new(),
                // Temporarily disable spinner collisions/behavior.
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
                turret_live: Vec::new(),
                turret_free: turret_free.clone(),
                turret_rightmost: SPAWN_X + 2000.0,
                turret_timers: Vec::new(),
                bullet_live: Vec::new(),
                bullet_free: bullet_free.clone(),
                dark_mode: false,
                god_mode: false,
                glow_flashes: Vec::new(),
                pad_bounce_anim: Vec::new(),
                spawn_animations: Vec::new(),

                hud_last_dist_fill:    u32::MAX,
                hud_last_coins:        u32::MAX,
                hud_last_momentum:     u32::MAX,
                hud_last_gravity_flip: false,
                hud_last_py:           i32::MAX,
                hud_last_px:           i32::MAX,
                hud_last_flip_timer:   u32::MAX,
                hud_last_zero_g_timer: u32::MAX,
                hud_last_score_x2_timer: u32::MAX,
                hud_last_score:        u32::MAX,
                hud_coin_fade_ticks:   u32::MAX,
                hud_coin_alpha:        0,
                hud_last_coin_alpha:   0,
                hud_coin_base_img:     None,

                // Space zone
                in_space_mode:            false,
                space_launch_active:      false,
                space_settle_done:        false,
                space_welcome_ticks:      0,
                space_oxygen:             SPACE_OXYGEN_TICKS,
                space_return_delay:       0,
                space_cam_y:              0.0,
                space_entry_bg_scale:     1.0,

                rocket_pad_live:          Vec::new(),
                rocket_pad_free:          rocket_pad_free.clone(),
                rocket_pad_rightmost:     SPAWN_X,

                space_planet_live:        Vec::new(),
                space_planet_free:        space_planet_free.clone(),
                space_planet_rightmost:   SPAWN_X,
                space_planet_data:        Vec::new(),

                space_hook_live:          Vec::new(),
                space_hook_free:          space_hook_free.clone(),
                space_hook_rightmost:     SPAWN_X,

                space_coin_live:          Vec::new(),
                space_coin_free:          space_coin_free.clone(),
                space_coin_rightmost:     SPAWN_X,

                space_blue_coin_live:   Vec::new(),
                space_blue_coin_free:   space_blue_coin_free.clone(),

                space_blackhole_live:     Vec::new(),
                space_blackhole_free:     space_bh_free.clone(),
                space_blackhole_rightmost: SPAWN_X,
                space_blackhole_data:     Vec::new(),

                space_asteroid_live:      Vec::new(),
                space_asteroid_free:      space_asteroid_free.clone(),
                space_asteroid_rightmost: SPAWN_X,

                hud_last_oxygen:          u32::MAX,

                space_stasis_active:    false,
                space_stasis_hook_id:   String::new(),
                space_stasis_is_entry:  false,

                space_red_coin_live:    Vec::new(),
                space_red_coin_free:    space_red_coin_free.clone(),

                space_gwell_timers:     Vec::new(),
                space_bh_teleport_fx:   Vec::new(),
                space_orbit_locked_planet: String::new(),
                space_orbit_speed:       0.0,
                space_entry_px:         0.0,
                space_coin_spent:       Vec::new(),
                space_blue_coin_spent:  Vec::new(),
                space_red_coin_spent:   Vec::new(),
                solar_surface_ratio:    SOLAR_SURFACE_RATIO_DEFAULT,
                solar_anim_loaded:      false,
                solar_anim_pending:     None,

                score_active_block: i32::MIN,
                score_block_ticks:  0,
                score_dead_blocks:  std::collections::HashSet::new(),

                player_ball_frame:       0,
                player_ball_hit_rewind:  false,
                player_ball_frame_timer: 0,
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
            let asteroid_mode_reset = matches!(canvas.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
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
            }
            if let Some(obj) = canvas.get_game_object_mut("pause_overlay") {
                obj.position = (-400.0, 0.0);
                obj.visible = false;
            }

            // Set background image AND apply the proper overscan/raise size so
            // the background fills the screen correctly from the first frame.
            {
                let bg_sel = match canvas.get_var("player_bg_selected") {
                    Some(Value::I32(v)) => v.max(0) as usize,
                    _ => 0,
                }.min(bg_zone_start_palettes.len().saturating_sub(1));
                if let Some(obj) = canvas.get_game_object_mut("bg") {
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
                        image: bg_zone_start_palettes[bg_sel].clone().into(),
                        color: None,
                    });
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
                        obj.collision_mode = CollisionMode::solid_circle(HOOK_ARTIFACT_R);
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
                    c.set_var("pause_hover_idx", -1);
                    c.set_var("settings_open", false);
                    c.set_var("game_paused", false);
                    let tc2 = {
                        let tv = match c.get_var("player_trail_selected") {
                            Some(Value::I32(v)) => v.max(0) as usize,
                            _ => 0,
                        }.min(SHOP_TRAIL_COLORS.len() - 1);
                        SHOP_TRAIL_COLORS[tv]
                    };
                    let trail = EmitterBuilder::new(PLAYER_TRAIL_EMITTER_NAME)
                        .rate(72.0).lifetime(0.68).velocity(-2.0, 8.0)
                        .spread(6.0, 6.0).size(9.0).color(tc2.0, tc2.1, tc2.2, 255)
                        .render_layer(2).gravity_scale(0.0)
                        .collision(CollisionResponse::None).build();
                    c.add_emitter(trail);
                    c.attach_emitter_to(PLAYER_TRAIL_EMITTER_NAME, "player");
                    if let Some(obj) = c.get_game_object_mut("player") { obj.visible = true; }
                    let was_hooked = matches!(c.get_var("rope_visible_at_pause"), Some(Value::Bool(true)));
                    if let Some(obj) = c.get_game_object_mut("rope") { obj.visible = was_hooked; }
                    for name in ["pause_overlay", "pause_title",
                                 "pause_resume_btn", "pause_restart_btn",
                                 "pause_settings_btn", "pause_menu_btn",
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
                    c.set_var("settings_open", false);
                    c.set_var("settings_dragging", -1i32);
                });
                canvas.register_custom_event("pause_restart_click".into(), |c| {
                    if !matches!(c.get_var("game_paused"), Some(Value::Bool(true))) { return; }
                    c.resume();
                    c.set_var("pause_animating", false);
                    c.set_var("pause_anim_frames", 0);
                    c.set_var("pause_hover_idx", -1);
                    c.set_var("settings_open", false);
                    c.set_var("settings_dragging", -1i32);
                    c.set_var("game_paused", false);
                    for name in ["pause_overlay", "pause_title",
                                 "pause_resume_btn", "pause_restart_btn",
                                 "pause_settings_btn", "pause_menu_btn",
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
                    let next = c.get_i32("level_nonce").saturating_add(1);
                    c.set_var("level_nonce", next);
                    c.load_scene("game");
                });
                canvas.register_custom_event("pause_menu_click".into(), |c| {
                    if !matches!(c.get_var("game_paused"), Some(Value::Bool(true))) { return; }
                    c.resume();
                    c.set_var("pause_animating", false);
                    c.set_var("pause_anim_frames", 0);
                    c.set_var("pause_hover_idx", -1);
                    c.set_var("settings_open", false);
                    c.set_var("settings_dragging", -1i32);
                    c.set_var("game_paused", false);
                    for name in ["pause_overlay", "pause_title",
                                 "pause_resume_btn", "pause_restart_btn",
                                 "pause_settings_btn", "pause_menu_btn",
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
                    // Reset camera zoom before loading menu so there's no
                    // visual pop from a zoomed-in game camera transitioning
                    // to the menu's unzoomed camera.
                    if let Some(cam) = c.camera_mut() {
                        cam.snap_zoom(1.0);
                        cam.zoom_anchor = None;
                    }
                    c.load_scene("menu");
                });
                canvas.register_custom_event("pause_settings_click".into(), |c| {
                    if !matches!(c.get_var("game_paused"), Some(Value::Bool(true))) { return; }
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
                    let btn_layout: &[(&str, f32, f32)] = &[
                        ("pause_title", (VW - 650.0) / 2.0, VH * 0.20),
                        ("pause_resume_btn", (VW - 700.0) / 2.0, 780.0),
                        ("pause_restart_btn", (VW - 700.0) / 2.0, 1000.0),
                        ("pause_settings_btn", (VW - 700.0) / 2.0, 1220.0),
                        ("pause_menu_btn", (VW - 700.0) / 2.0, 1440.0),
                    ];
                    for &(name, bx, by) in btn_layout {
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
                            if !matches!(c.get_var("game_paused"), Some(Value::Bool(true))) {
                                return;
                            }

                            // pos is world-space; multiply by zoom to get virtual-screen space
                            // so ignore_zoom UI hit tests are correct at any camera zoom level.
                            let zoom = c.camera().map(|cam| cam.zoom).unwrap_or(1.0);

                            // If dragging a settings slider, update its position/value.
                            let dragging = c.get_i32("settings_dragging");
                            if dragging >= 0 && matches!(c.get_var("settings_open"), Some(Value::Bool(true))) {
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
                                    c.sync_ignore_zoom_offsets();
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
                            if let Some(obj) = c.get_game_object_mut("pause_resume_btn") {
                                if over_resume { obj.set_tint(hover_tint); } else { obj.clear_highlight(); }
                            }
                            if let Some(obj) = c.get_game_object_mut("pause_restart_btn") {
                                if over_restart { obj.set_tint(hover_tint); } else { obj.clear_highlight(); }
                            }
                            if let Some(obj) = c.get_game_object_mut("pause_settings_btn") {
                                if over_settings { obj.set_tint(hover_tint); } else { obj.clear_highlight(); }
                            }
                            if let Some(obj) = c.get_game_object_mut("pause_menu_btn") {
                                if over_menu { obj.set_tint(hover_tint); } else { obj.clear_highlight(); }
                            }
                            if let Some(obj) = c.get_game_object_mut("settings_back_btn") {
                                if over_back { obj.set_tint(hover_tint); } else { obj.clear_highlight(); }
                            }
                        }
                    });

                    canvas.on_mouse_press(move |c, btn, pos| {
                        if btn != MouseButton::Left { return; }
                        if !c.is_scene("game") { return; }
                        if !matches!(c.get_var("game_paused"), Some(Value::Bool(true))) {
                            return;
                        }

                        // pos is world-space (divided by zoom); ignore_zoom UI lives in
                        // virtual-screen space (0..VW, 0..VH). Multiply by zoom to convert.
                        let zoom = c.camera().map(|cam| cam.zoom).unwrap_or(1.0);
                        let ux = pos.0 * zoom;
                        let uy = pos.1 * zoom;

                        // If settings panel is open, check for slider track hits first.
                        if matches!(c.get_var("settings_open"), Some(Value::Bool(true))) {
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
                                        c.sync_ignore_zoom_offsets();
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

                let bg_p = bg_zone_purple.clone();
                let bg_b = bg_zone_black.clone();
                let bg_sv = bg_zone_start_vivid.clone();
                let bg_pv = bg_zone_purple_vivid.clone();
                let bg_bv = bg_zone_black_vivid.clone();
                let bg_pf = bg_zone_purple_flip.clone();
                let bg_bf = bg_zone_black_flip.clone();
                let bg_svf = bg_zone_start_vivid_flip.clone();
                let bg_pvf = bg_zone_purple_vivid_flip.clone();
                let bg_bvf = bg_zone_black_vivid_flip.clone();
                let bg_palettes_s = Arc::clone(&bg_zone_start_palettes);
                let bg_palettes_sf = Arc::clone(&bg_zone_start_palettes_flip);
                let bg_ss = bg_zone_start_space.clone();
                let bg_ps = bg_zone_purple_space.clone();
                let bg_bs = bg_zone_black_space.clone();
                let bg_svs = bg_zone_start_vivid_space.clone();
                let bg_pvs = bg_zone_purple_vivid_space.clone();
                let bg_bvs = bg_zone_black_vivid_space.clone();
                let bg_ssf = bg_zone_start_space_flip.clone();
                let bg_psf = bg_zone_purple_space_flip.clone();
                let bg_bsf = bg_zone_black_space_flip.clone();
                let bg_svsf = bg_zone_start_vivid_space_flip.clone();
                let bg_pvsf = bg_zone_purple_vivid_space_flip.clone();
                let bg_bvsf = bg_zone_black_vivid_space_flip.clone();
                let bg_space_arc = bg_space_img_arc.clone();
                let transparent_star_arc = transparent_star_arc.clone();
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
                            let btn_layout: &[(&str, f32, f32)] = &[
                                ("pause_title", (VW - 650.0) / 2.0, VH * 0.20),
                                ("pause_resume_btn", (VW - 700.0) / 2.0, 780.0),
                                ("pause_restart_btn", (VW - 700.0) / 2.0, 1000.0),
                                ("pause_settings_btn", (VW - 700.0) / 2.0, 1220.0),
                                ("pause_menu_btn", (VW - 700.0) / 2.0, 1440.0),
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
                                    obj.position = (-400.0, 0.0);
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
                        // ── Orbit animation while waiting for "hold space to begin" ──
                        if matches!(c.get_var("start_prompt_active"), Some(Value::Bool(true))) {
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
                        let trail_rgb = {
                            let idx = match c.get_var("player_trail_selected") {
                                Some(Value::I32(v)) => v.max(0) as usize,
                                _ => 0,
                            }
                            .min(SHOP_TRAIL_COLORS.len() - 1);
                            SHOP_TRAIL_COLORS[idx]
                        };
                        c.run(Action::set_emitter_color(
                            PLAYER_TRAIL_EMITTER_NAME,
                            trail_rgb.0,
                            trail_rgb.1,
                            trail_rgb.2,
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
                    if matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true))) {
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
                    if matches!(c.get_var("manual_flip_queued"), Some(Value::Bool(true))) {
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
                    if matches!(c.get_var("bg_force_refresh"), Some(Value::Bool(true))) {
                        prev_bg_theme = None;
                        prev_palette_zone = usize::MAX;
                        prev_nearest_hook.clear();
                        scroll_init = false; // re-apply star panel images after respawn
                        c.set_var("bg_force_refresh", false);
                    }
                    background::tick_background(
                        c,
                        &st,
                        &mut prev_bg_theme,
                        &mut bg_scale_smooth,
                        {
                            let bg_sel = match c.get_var("player_bg_selected") {
                                Some(Value::I32(v)) => v.max(0) as usize,
                                _ => 0,
                            }.min(bg_palettes_s.len().saturating_sub(1));
                            &bg_palettes_s[bg_sel]
                        },
                        &bg_p,
                        &bg_b,
                        &bg_sv,
                        &bg_pv,
                        &bg_bv,
                        {
                            let bg_sel = match c.get_var("player_bg_selected") {
                                Some(Value::I32(v)) => v.max(0) as usize,
                                _ => 0,
                            }.min(bg_palettes_sf.len().saturating_sub(1));
                            &bg_palettes_sf[bg_sel]
                        },
                        &bg_pf,
                        &bg_bf,
                        &bg_svf,
                        &bg_pvf,
                        &bg_bvf,
                        &bg_ss,
                        &bg_ps,
                        &bg_bs,
                        &bg_svs,
                        &bg_pvs,
                        &bg_bvs,
                        &bg_ssf,
                        &bg_psf,
                        &bg_bsf,
                        &bg_svsf,
                        &bg_pvsf,
                        &bg_bvsf,
                    );

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
                            let img = if in_space {
                                Image {
                                    shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
                                    image: bg_space_arc.clone(),
                                    color: None,
                                }
                            } else {
                                Image {
                                    shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
                                    image: transparent_star_arc.clone(),
                                    color: None,
                                }
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
                    let died_to_sun = matches!(c.get_var("died_to_sun"), Some(Value::Bool(true)));
                    let died_to_oxygen = matches!(c.get_var("died_to_oxygen"), Some(Value::Bool(true)));
                    let dead_now = !s.god_mode && (died_to_sun || died_to_oxygen || (s.gravity_dir > 0.0
                        && s.py > VH + 150.0)
                        || (s.gravity_dir < 0.0 && s.py < -150.0));
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
            canvas.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
        })
}
