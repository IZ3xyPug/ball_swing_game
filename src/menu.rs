use quartz::*;
use std::sync::{Arc, Mutex};
use crate::achievements::*;
use crate::constants::*;
use crate::audio_state;
use crate::images::*;
use crate::objects::{ui_text_left_spec, ui_text_spec};
use crate::scenes::game::helpers::{volume_value, set_volume_value, music_volume, ui_font,
                                   position_volume_sliders, update_volume_labels};
use crate::shop;

const MENU_TRACKS: [&[u8]; 3] = [ASSET_MENU_BGM, ASSET_MENU_BGM_2, ASSET_MENU_BGM_3];

fn play_menu_track(c: &mut Canvas, idx: usize) {
    let track_idx = idx % MENU_TRACKS.len();
    let handle = c.play_sound_bytes_with(
        MENU_TRACKS[track_idx],
        SoundOptions::new().volume(music_volume(c, 0.18)).looping(false),
    );
    audio_state::replace_menu_bgm(handle);
    c.set_var("menu_bgm_track_index", track_idx as i32);
}


/// Which arrow of the mode selector a tap landed on: -1 for the left, +1 for
/// the right, `None` for a miss.
///
/// The zones are DELIBERATELY much larger than the arrows drawn inside the
/// widget. The selector is 140 world units tall, which is about eight
/// millimetres on a phone — under the ~9mm a thumb can reliably hit, and that
/// is before asking the player to land in the outer third of it. A tap that
/// misses gives no feedback at all, so it reads as "swiping doesn't work"
/// rather than "you missed by 4mm".
///
/// So the zones extend well above and below the widget and outward past its
/// edges, and they split it down the MIDDLE rather than into thirds: there is
/// nothing in the centre of the selector worth protecting from a stray tap, and
/// a dead band between the halves is just another way to miss.
fn mode_arrow_hit(c: &Canvas, world: (f32, f32)) -> Option<i32> {
    let obj = c.get_game_object("menu_mode_selector")?;
    let (x, y) = obj.position;
    let (w, h) = obj.size;
    // Grown by half the widget's height on every side.
    let pad = h * 0.5;
    if world.0 < x - pad || world.0 > x + w + pad
        || world.1 < y - pad || world.1 > y + h + pad
    {
        return None;
    }
    Some(if world.0 < x + w * 0.5 { -1 } else { 1 })
}

/// Move the game-mode selection by `delta` and redraw its labels.
///
/// The index lives in `game_mode_idx` on the canvas and NOWHERE ELSE. It used
/// to live in an `Arc<Mutex<usize>>` captured by the key handler as well, which
/// meant the tap path and the key path could disagree — and it forced anything
/// that wanted to change the mode to be registered where that Arc was in scope,
/// which is the menu's `on_enter`, which is precisely where mouse-press
/// handlers do not survive.
pub(crate) fn step_game_mode(c: &mut Canvas, delta: i32) {
    let n = GAME_MODES.len() as i32;
    let idx = ((c.get_i32("game_mode_idx") + delta).rem_euclid(n)) as usize;
    c.set_var("game_mode_idx", idx as i32);
    crate::menu_look::draw_menu_text(c);
}

/// Open what a menu button leads to. Shared by a click and by Enter on the
/// lit list item, so the two can never disagree.
///
/// Scenes are loaded through `queue_scene` rather than directly, because the
/// custom-event dispatch wasn't reliably firing for these buttons from inside
/// the mouse-press handler. START (sets the mode) and SHOP (a sub-view of this
/// scene) keep their custom events.
pub(crate) fn activate_menu_button(c: &mut Canvas, name: &str) {
    match name {
        "start_btn" => c.run(Action::Custom { name: "goto_game".into() }),
        "menu_shop_btn" => c.run(Action::Custom { name: "goto_shop".into() }),
        "menu_settings_btn" => c.set_var("queue_scene", "menu_settings"),
        "menu_achievements_btn" => c.set_var("queue_scene", "achievements"),
        "menu_stats_btn" => c.set_var("queue_scene", "stats"),
        "menu_daily_btn" => c.set_var("queue_scene", "daily_reward"),
        "menu_profile_btn" => c.set_var("queue_scene", "profile"),
        "menu_boss_order_btn" => c.set_var("queue_scene", "boss_order"),
        _ => {}
    }
}

/// The menu lives in the upper half of a 2×VH world (camera at MENU_Y), but the
/// engine's mouse hit-testing maps clicks to *virtual* coords while checking
/// *world* boxes — so menu buttons (at world y > VH) can't be clicked. Register
/// a camera-aware click handler that converts the click to world coords and
/// fires the button's custom event.
pub(crate) fn push_menu_press_handler(canvas: &mut Canvas) {
    let registered = matches!(canvas.get_var("menu_press_handler_registered"), Some(Value::Bool(true)));
    if registered {
        return;
    }
    canvas.on_mouse_press(move |c, _btn, pos| {
        if !c.is_scene("menu") {
            return;
        }
        if matches!(c.get_var("menu_in_shop"), Some(Value::Bool(true))) {
            return;
        }
        let world = c.screen_to_world(pos);

        // Click the planet and it pops: swells, springs back (see
        // `menu_look::tick_menu_look`). Cosmetic; the click goes nowhere else.
        if crate::menu_look::planet_hit(c, world) {
            let t = c.get_i32("menu_t");
            c.set_var("planet_pop_t", t);
            return;
        }

        // The mode selector's arrows.
        //
        // Handled HERE, in the handler registered at app start, and not in the
        // menu's `on_enter` — a mouse-press callback registered there does not
        // survive the scene load, which is the reason this function exists at
        // all (see its call site in `lib.rs`). Every attempt to add tap or
        // gesture handling in `on_enter` was dead code that never received an
        // event.
        if let Some(d) = mode_arrow_hit(c, world) {
            step_game_mode(c, d);
            return;
        }

        const BTNS: &[(&str, &str)] = &[
            ("start_btn", "goto_game"),
            ("menu_shop_btn", "goto_shop"),
            ("menu_settings_btn", "goto_menu_settings"),
            ("menu_achievements_btn", "goto_achievements"),
            ("menu_stats_btn", "goto_stats"),
            ("menu_daily_btn", "goto_daily_reward"),
            ("menu_profile_btn", "goto_profile"),
            ("menu_boss_order_btn", "goto_boss_order"),
        ];
        for (name, event) in BTNS {
            if let Some(obj) = c.get_game_object(name) {
                let hit = world.0 >= obj.position.0
                    && world.0 <= obj.position.0 + obj.size.0
                    && world.1 >= obj.position.1
                    && world.1 <= obj.position.1 + obj.size.1;
                if hit {
                    let _ = event;
                    activate_menu_button(c, name);
                    return;
                }
            }
        }
    });
    canvas.set_var("menu_press_handler_registered", true);
}

pub(crate) fn gameover_step_countup(c: &mut Canvas, value_key: &str, velocity_key: &str, target: i32) -> i32 {
    if target <= 0 {
        c.set_var(value_key, 0i32);
        c.set_var(velocity_key, 0i32);
        return 0;
    }

    let current = c.get_i32(value_key).max(0);
    if current >= target {
        c.set_var(value_key, target);
        c.set_var(velocity_key, 0i32);
        return target;
    }

    if current == 0 {
        let seed = 1.min(target);
        c.set_var(value_key, seed);
        c.set_var(velocity_key, 1i32);
        return seed;
    }

    let remaining = target - current;
    let prev_velocity = c.get_i32(velocity_key).max(0);
    let boosted_velocity = if prev_velocity <= 0 {
        2
    } else {
        ((prev_velocity as f32) * 1.08).ceil() as i32 + 1
    };
    let min_step = if current < 10 {
        1
    } else if current < 100 {
        2
    } else if current < 1_000 {
        4
    } else {
        8
    };
    let catchup_cap = (remaining / 18).max(min_step);
    let step = boosted_velocity.min(catchup_cap.max(min_step)).max(min_step);
    let next = (current + step).min(target);

    c.set_var(value_key, next);
    c.set_var(velocity_key, step);
    next
}

fn update_gameover_stats_text(
    c: &mut Canvas,
    object_id: &str,
    width: f32,
    font: &Font,
    scale: f32,
    distance: i32,
    score: i32,
    coins: i32,
) {
    if let Some(obj) = c.get_game_object_mut(object_id) {
        let stats_line = format!(
            "DISTANCE  {}\nSCORE  {}\nCOINS  {}",
            distance.max(0),
            score.max(0),
            coins.max(0),
        );
        obj.set_drawable(Box::new(ui_text_spec(
            &stats_line,
            font,
            46.0 * scale,
            Color(255, 255, 255, 255),
            width * scale,
        )));
    }
}

fn init_gameover_countup(c: &mut Canvas, stats_object_id: &str, width: f32) {
    let Some(font) = ui_font() else { return; };

    let scale = c.virtual_scale();
    
    // SAFE retrieval: use get_var and match instead of panicking get_i32/get_f32
    c.set_var("go_count_distance", 0i32);
    c.set_var("go_count_distance_vel", 0i32);
    c.set_var("go_count_score", 0i32);
    c.set_var("go_count_score_vel", 0i32);
    c.set_var("go_count_coins", 0i32);
    c.set_var("go_count_coins_vel", 0i32);

    update_gameover_stats_text(c, stats_object_id, width, &font, scale, 0, 0, 0);

    // Use a per-scene flag instead of a global one so each scene gets its own callback
    let flag_key = format!("go_countup_registered_{}", stats_object_id);
    let registered = matches!(c.get_var(&flag_key), Some(Value::Bool(true)));
    if registered { return; }

    c.on_update(|canvas| {
        let active = if canvas.is_scene("gameover") {
            Some("go_stats_text")
        } else if canvas.is_scene("gameover_sun") {
            Some("sun_go_stats_text")
        } else if canvas.is_scene("gameover_oxygen") {
            Some("oxy_go_stats_text")
        } else {
            None
        };
        
        let Some(stats_object_id) = active else { return; };

        let Some(font) = ui_font() else { return; };
        
        // SAFE retrieval: use get_var and match
        let target_distance = match canvas.get_var("last_distance") {
            Some(Value::F32(v)) => v.round() as i32,
            _ => 0
        };
        let target_score = match canvas.get_var("last_score") {
            Some(Value::I32(v)) => v.max(0),
            _ => 0
        };
        let target_coins = match canvas.get_var("last_coins") {
            Some(Value::I32(v)) => v.max(0),
            _ => 0
        };
        
        let distance = gameover_step_countup(
            canvas,
            "go_count_distance",
            "go_count_distance_vel",
            target_distance,
        );
        let score = gameover_step_countup(canvas, "go_count_score", "go_count_score_vel", target_score);
        let coins = gameover_step_countup(canvas, "go_count_coins", "go_count_coins_vel", target_coins);
        
        update_gameover_stats_text(
            canvas,
            stats_object_id,
            1000.0,
            &font,
            canvas.virtual_scale(),
            distance,
            score,
            coins,
        );
    });
    
    // Set per-scene flag so this scene won't register the callback again on next entry
    c.set_var(&flag_key, true);
}

/// Menu lives at y = MENU_Y..MENU_Y+VH; shop lives at y = 0..VH.
/// Camera pans between these two regions (from MENU_Y up to 0) for the transition.
///
/// The gap between them is sky. A window taller than 16:9 shows more than VH
/// of the world, and without the gap the shop's bottom edge (its MENU button,
/// the galaxy) peeked over the top of the menu.
pub(crate) const MENU_Y: f32 = VH + MENU_GAP;
const MENU_GAP: f32 = 600.0;

/// The shop's buttons only take clicks once the camera has arrived. The
/// engine hit-tests them in virtual coordinates, which match the world only
/// with the camera at 0 — so the very click on SHOP, still on the menu, used
/// to land on the BACKGROUNDS card sitting at the same spot and open it.
fn shop_ready(c: &Canvas) -> bool {
    c.get_bool("menu_in_shop") && c.camera().map_or(true, |cam| cam.position.1.abs() < 2.0)
}

/// How long the camera takes to climb into the shop or come back down.
const MENU_CLIMB_TICKS: i32 = 54;

/// How strongly the full-screen scene tints are applied, as a multiplier on
/// their authored alpha.
///
/// One knob rather than nine scattered `Color(...)` literals, because these are
/// tuned by eye against a background and always want adjusting together.
///
/// NOTE: turning this down is a content change, not a rendering fix. The
/// backgrounds looking washed out on Android was first blamed on these, on the
/// theory that alpha-to-coverage had been quietly discarding most of the tint
/// and real alpha blending now applies it in full. That theory is wrong: at 4x
/// coverage a 59% alpha writes ~59% of samples and resolves to the same colour
/// alpha blending produces directly. Whatever changed the backgrounds is still
/// unidentified — do not treat this constant as having fixed it.
const MENU_TINT_SCALE: f32 = 0.85;

/// A full-screen scene tint at the shared strength.
fn scene_tint(w: f32, h: f32, c: Color) -> Image {
    let Color(r, g, b, a) = c;
    tint_overlay(w, h, Color(r, g, b, (a as f32 * MENU_TINT_SCALE).round() as u8))
}

/// Widest a menu background is ever rasterised, whatever size the quad is.
///
/// 1920 still exceeds the pixel width of any phone this is meant to run on and
/// is half the linear size — a quarter of the texels — of the 4640 the quads
/// ask for.
pub(crate) const MENU_BG_MAX_W: f32 = 1920.0;

/// Cached aurora earth background — decoded and resized once, then Arc-shared.
static AURORA_BG_CACHE: std::sync::OnceLock<Arc<image::RgbaImage>> = std::sync::OnceLock::new();

/// Load aurora_earth.gif at the given dimensions using Lanczos3 for high-quality resize.
/// Result is cached so subsequent calls clone an Arc pointer instead of re-decoding.
fn bright_background_2(w: f32, h: f32) -> Image {
    let arc = AURORA_BG_CACHE.get_or_init(|| {
        let aurora_src = image::load_from_memory(include_bytes!("../assets/aurora_earth.gif"))
            .expect("aurora_earth.gif decode failed")
            .to_rgba8();
        // Rasterised at a BOUNDED size and stretched to `w` x `h`, not at the
        // full quad size.
        //
        // Callers ask for `VW + 800` x `VH` = 4640 x 2160, which is ten million
        // texels of a soft, out-of-focus photograph — over a phone's 4096
        // texture limit (so the engine had to downscale it again at upload) and
        // a large amount of sampling bandwidth for every frame that shows a
        // background. Two of them are on screen at once during the shop slide.
        //
        // The source GIF is nowhere near this detailed to begin with, so the
        // cap costs nothing visible on any display.
        let scale = (MENU_BG_MAX_W / w).min(1.0);
        let (tw, th) = (
            (w * scale).round().max(1.0) as u32,
            (h * scale).round().max(1.0) as u32,
        );
        let pixels = image::imageops::resize(
            &aurora_src, tw, th, image::imageops::FilterType::Triangle,
        );
        Arc::new(pixels)
    });
    Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: Arc::clone(arc), color: None }
}

fn tutorial_apply_page(c: &mut Canvas, page: i32) {
    let page = page.clamp(0, 2);
    c.set_var("tutorial_page", page);

    const TITLES: [&str; 3] = [
        "TUTORIAL  1/3",
        "TUTORIAL  2/3",
        "TUTORIAL  3/3",
    ];
    const BODIES: [&str; 3] = [
        "SWING BETWEEN HOOKS TO BUILD SPEED AND DISTANCE.",
        "GRAB POWERUPS TO IMPROVE SURVIVAL AND MOMENTUM.",
        "WATCH FLOOR DANGER, THEN CHAIN MOVES FOR LONG RUNS.",
    ];

    for i in 0..3 {
        if let Some(obj) = c.get_game_object_mut(&format!("tutorial_window_{i}")) {
            obj.visible = i == page;
        }
    }

    // Scale by virtual_scale() so the text tracks the canvas size and stays
    // aligned with the (already virtual-scale) tutorial boxes on any resize.
    if let Some(font) = ui_font() {
        let s = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("tutorial_title_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                TITLES[page as usize],
                &font,
                66.0 * s,
                Color(220, 238, 255, 255),
                1500.0 * s,
            )));
        }

        if let Some(obj) = c.get_game_object_mut("tutorial_body_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                BODIES[page as usize],
                &font,
                48.0 * s,
                Color(200, 218, 236, 235),
                1800.0 * s,
            )));
        }

        if let Some(obj) = c.get_game_object_mut("tutorial_next_text") {
            let label = if page >= 2 { "FINISH" } else { "NEXT" };
            obj.set_drawable(Box::new(ui_text_spec(
                label,
                &font,
                36.0 * s,
                Color(245, 250, 255, 255),
                180.0 * s,
            )));
        }
    }
}

pub fn build_tutorial_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(ctx, "tutorial_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    let bg_tint = GameObject::new_rect(ctx, "tutorial_bg_tint".into(),
        Some(scene_tint(VW + 800.0, VH, Color(40, 70, 130, 165))),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);

    let mut scene = Scene::new("tutorial")
        .with_object("tutorial_bg", bg)
        .with_object("tutorial_bg_tint", bg_tint);

    let win_w = 2960.0;
    let win_h = 1680.0;
    let win_x = VW * 0.5 - win_w * 0.5;
    let win_y = VH * 0.09;

    for i in 0..3 {
        let mut img = image::RgbaImage::new(win_w as u32, win_h as u32);
        for py in 0..img.height() {
            for px in 0..img.width() {
                let border = px < 5 || px >= img.width() - 5 || py < 5 || py >= img.height() - 5;
                let glow = ((py as f32 / img.height() as f32) * 24.0) as u8;
                img.put_pixel(
                    px,
                    py,
                    image::Rgba([
                        if border { 90 + i as u8 * 16 } else { 15 + glow / 4 },
                        if border { 150 + i as u8 * 8 } else { 20 + glow / 3 },
                        if border { 230 } else { 44 + glow / 2 },
                        if border { 255 } else { 235 },
                    ]),
                );
            }
        }
        let mut win = GameObject::new_rect(ctx, format!("tutorial_window_{i}").into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (win_w, win_h), 0.0), image: img.into(), color: None }),
            (win_w, win_h), (win_x, win_y),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        win.visible = i == 0;
        scene = scene.with_object(format!("tutorial_window_{i}"), win);
    }

    let title_w = 1600.0;
    let body_w = 1600.0;

    let title_text = GameObject::build("tutorial_title_text")
        .size(title_w, 120.0)
        .position(VW * 0.5 - title_w * 0.5, win_y + 120.0)
        .tag("ui")
        .build(ctx);
    let body_text = GameObject::build("tutorial_body_text")
        .size(body_w, 120.0)
        .position(VW * 0.5 - body_w * 0.5, win_y + 330.0)
        .tag("ui")
        .build(ctx);

    let next_btn_w = 300.0;
    let next_btn_h = 105.0;
    let next_btn_x = win_x + win_w - next_btn_w - 60.0;
    let next_btn_y = win_y + win_h - next_btn_h - 60.0;
    let next_btn = {
        let mut img = image::RgbaImage::new(next_btn_w as u32, next_btn_h as u32);
        for py in 0..img.height() {
            for px in 0..img.width() {
                let border = px < 3 || px >= img.width() - 3 || py < 3 || py >= img.height() - 3;
                img.put_pixel(px, py, image::Rgba([55, if border { 170 } else { 120 }, 85, 235]));
            }
        }
        GameObject::new_rect(ctx, "tutorial_next_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (next_btn_w, next_btn_h), 0.0), image: img.into(), color: None }),
            (next_btn_w, next_btn_h), (next_btn_x, next_btn_y),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    let next_text = GameObject::build("tutorial_next_text")
        .size(next_btn_w, next_btn_h)
        .position(next_btn_x, next_btn_y + 8.0)
        .tag("ui")
        .build(ctx);

    scene
        .with_object("tutorial_title_text", title_text)
        .with_object("tutorial_body_text", body_text)
        .with_object("tutorial_next_btn", next_btn)
        .with_object("tutorial_next_text", next_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "tutorial_next".into() },
                target: Target::name("tutorial_next_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("tutorial_next_btn"),
        )
        .on_enter(|canvas| {
            // If this profile already finished the tutorial, skip straight to
            // the menu.
            if crate::profile::profile().lock().unwrap().tutorial_done {
                canvas.load_scene("menu");
                return;
            }
            // The tutorial is played, not read: a coached run (see
            // `scenes::game::tutorial`).
            canvas.set_var(crate::scenes::game::tutorial::TUTORIAL_PENDING_VAR, true);
            canvas.set_var("boss_mode_active", false);
            canvas.set_var("game_intro_t", crate::constants::GAME_INTRO_TICKS);
            canvas.load_scene("game");
            return;
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);
            tutorial_apply_page(canvas, 0);

            // Re-apply the tutorial page whenever the canvas scale changes
            // (window resize), so the text stays aligned with the boxes.
            if !matches!(canvas.get_var("tutorial_update_registered"), Some(Value::Bool(true))) {
                canvas.on_update(|c| {
                    if !c.is_scene("tutorial") { return; }
                    let cur_scale = c.virtual_scale();
                    let last_scale = match c.get_var("tutorial_text_last_scale") { Some(Value::F32(v)) => v, _ => -1.0 };
                    if (last_scale - cur_scale).abs() >= 0.0001 {
                        c.set_var("tutorial_text_last_scale", cur_scale);
                        tutorial_apply_page(c, c.get_i32("tutorial_page"));
                    }
                });
                canvas.set_var("tutorial_update_registered", true);
            }

            canvas.register_custom_event("tutorial_next".into(), |c| {
                let page = c.get_i32("tutorial_page");
                if page >= 2 {
                    // Mark the tutorial complete on the active profile.
                    {
                        let p = crate::profile::profile();
                        let mut g = p.lock().unwrap();
                        g.tutorial_done = true;
                    }
                    crate::profile::save_profile();
                    c.load_scene("menu");
                } else {
                    tutorial_apply_page(c, page + 1);
                }
            });
        })
}

/// The selectable modes, in `mode::GameMode::index()` order so the selector
/// index IS the mode index — no lookup table to drift.
pub static GAME_MODES: &[(&str, &str)] = &[
    (crate::mode::GameMode::Casual.name(), crate::mode::GameMode::Casual.blurb()),
    (crate::mode::GameMode::Normal.name(), crate::mode::GameMode::Normal.blurb()),
    (crate::mode::GameMode::BossRush.name(), crate::mode::GameMode::BossRush.blurb()),
];

/// Select a profile slot, apply its persistent state to the run vars, and then
/// continue to the tutorial (if not yet done) or straight to the menu.
pub fn select_profile_and_continue(canvas: &mut Canvas, idx: usize) {
    crate::profile::select_profile(idx);
    let p = crate::profile::profile();
    let (tutorial_done, extra_hearts, c_char, c_rope, c_bg, c_trail, c_rope_style) = {
        let g = p.lock().unwrap();
        (g.tutorial_done, g.permanent_extra_hearts, g.cosmetic_char, g.cosmetic_rope, g.cosmetic_bg,
         g.cosmetic_trail, g.cosmetic_rope_style)
    };
    // Permanent extra hearts are read into the run on game start; push them now
    // so the first run already benefits.
    canvas.set_var(crate::constants::META_EXTRA_HEARTS_VAR, extra_hearts as i32);
    // Apply the profile's saved cosmetics.
    canvas.set_var("player_char_selected", c_char as i32);
    canvas.set_var("player_rope_selected", c_rope as i32);
    canvas.set_var("player_bg_selected", c_bg as i32);
    canvas.set_var("player_trail_selected", c_trail as i32);
    canvas.set_var("player_rope_style_selected", c_rope_style as i32);
    // Pre-mark already-unlocked achievements so they aren't re-triggered.
    if tutorial_done {
        canvas.load_scene("menu");
    } else {
        canvas.load_scene("tutorial");
    }
}

/// Apply the profile title + slot labels using the canvas's current virtual
/// scale. Called on enter AND re-applied every frame while `profile_text_dirty`
/// is set, because at boot the canvas isn't sized yet — `virtual_scale()` returns
/// 1.0 on the first on_enter, which makes the labels render far too large and
/// misaligned. Re-applying after layout has sized the canvas fixes the layout.
pub(crate) fn apply_profile_text(canvas: &mut Canvas) {
    let Some(font) = ui_font() else { return; };
    let s = canvas.virtual_scale();
    let profiles = crate::profile::all_profiles();
    for i in 0..crate::profile::SLOT_COUNT {
        let p = profiles.get(i).cloned().unwrap_or_default();
        let tut = if p.tutorial_done { "Tutorial done" } else { "New" };
        let label = format!("{}   \u{2022}   Meta {}   \u{2022}   {}", p.name, p.meta_currency, tut);
        if let Some(obj) = canvas.get_game_object_mut(&format!("profile_slot_text_{i}")) {
            obj.set_drawable(Box::new(ui_text_spec(&label, &font, 32.0 * s, Color(210, 225, 245, 255), 1080.0 * s)));
        }
        if let Some(obj) = canvas.get_game_object_mut(&format!("profile_delete_text_{i}")) {
            obj.set_drawable(Box::new(ui_text_spec("DELETE", &font, 26.0 * s, Color(255, 215, 215, 255), 200.0 * s)));
        }
    }
}

/// Show/hide the delete-confirmation modal and refresh its text to name the
/// slot under `profile_delete_pending`. Call whenever the pending var changes.
fn update_profile_delete_confirm(canvas: &mut Canvas) {
    let pending = match canvas.get_var("profile_delete_pending") {
        Some(Value::I32(v)) => v,
        _ => -1,
    };
    let show = pending >= 0;
    for name in [
        "profile_delete_dim",
        "profile_delete_panel", "profile_delete_title", "profile_delete_note",
        "profile_delete_confirm", "profile_delete_confirm_text",
        "profile_delete_cancel", "profile_delete_cancel_text",
    ] {
        if let Some(obj) = canvas.get_game_object_mut(name) {
            obj.visible = show;
        }
    }
    if !show { return; }
    let Some(font) = ui_font() else { return; };
    let s = canvas.virtual_scale();
    let (name, meta) = {
        let profiles = crate::profile::all_profiles();
        let p = profiles.get(pending as usize).cloned().unwrap_or_default();
        (p.name, p.meta_currency)
    };
    if let Some(obj) = canvas.get_game_object_mut("profile_delete_title") {
        obj.set_drawable(Box::new(ui_text_spec("DELETE THIS PROFILE?", &font, 46.0 * s, Color(255, 120, 120, 255), 1220.0 * s)));
    }
    if let Some(obj) = canvas.get_game_object_mut("profile_delete_note") {
        obj.set_drawable(Box::new(ui_text_spec(&format!("{name}   \u{2022}   Meta {meta}   \u{2014}   this cannot be undone."), &font, 30.0 * s, Color(220, 230, 245, 245), 1200.0 * s)));
    }
    if let Some(obj) = canvas.get_game_object_mut("profile_delete_confirm_text") {
        obj.set_drawable(Box::new(ui_text_spec("CONFIRM", &font, 34.0 * s, Color(255, 235, 235, 255), 380.0 * s)));
    }
    if let Some(obj) = canvas.get_game_object_mut("profile_delete_cancel_text") {
        obj.set_drawable(Box::new(ui_text_spec("CANCEL", &font, 34.0 * s, Color(225, 235, 250, 255), 380.0 * s)));
    }
}

/// Profile selection screen shown before the menu. Lists the save slots; picking
/// one loads it (meta / permanent upgrades / achievements / tutorial state) and
/// continues to the tutorial or the menu.
pub fn build_profile_scene(ctx: &mut Context) -> Scene {
    let mut scene = crate::menu_look::with_page_look(ctx, Scene::new("profile"), "SELECT PROFILE");

    // Slot buttons + text + a per-slot DELETE button on the right.
    for i in 0..crate::profile::SLOT_COUNT {
        let id = format!("profile_slot_{i}");
        let txt_id = format!("profile_slot_text_{i}");
        let del_id = format!("profile_delete_btn_{i}");
        let del_txt_id = format!("profile_delete_text_{i}");
        let y = VH * (0.26 + i as f32 * 0.15);
        let btn = GameObject::new_rect(ctx, id.clone().into(),
            Some(crate::menu_look::glass(1080.0, 120.0, crate::menu_look::GLASS_LILAC, false)),
            (1080.0, 120.0), ((VW - 1080.0) * 0.5, y),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        scene = scene.with_object(&id, btn);
        let txt = GameObject::build(&txt_id)
            .size(1080.0, 120.0)
            .position((VW - 1080.0) * 0.5, y + (120.0 - 40.0) * 0.5)
            .tag("ui")
            .build(ctx);
        scene = scene.with_object(&txt_id, txt);
        // Delete button sits to the right of the slot band.
        let del_x = VW * 0.5 + 570.0;
        let del_btn = GameObject::new_rect(ctx, del_id.clone().into(),
            Some(crate::menu_look::glass(200.0, 80.0, crate::menu_look::GLASS_ROSE, false)),
            (200.0, 80.0), (del_x, y + 20.0),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        scene = scene.with_object(&del_id, del_btn);
        let del_txt = GameObject::build(&del_txt_id)
            .size(200.0, 80.0)
            .position(del_x, y + 20.0 + (80.0 - 30.0) * 0.5)
            .tag("ui")
            .build(ctx);
        scene = scene.with_object(&del_txt_id, del_txt);
    }

    // ── Delete-confirmation modal (self-contained dialog) ──────────────────
    // NOTE: `new_rect`'s last param is GRAVITY (not layer). Pass 0.0 so the
    // dialog doesn't fall, then set `.layer` on the returned object to stack it
    // above the slot rows.
    let confirm_w = 1300.0;
    let confirm_h = 460.0;
    // Full-screen dim backdrop so the dialog clearly sits on top.
    let mut dim = GameObject::new_rect(ctx, "profile_delete_dim".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (VW + 800.0, VH), 0.0), image: solid(4, 2, 14, 170).into(), color: None }),
        (VW + 800.0, VH), (-400.0, 0.0),
        vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    dim.layer = 29;
    let mut confirm_bg = GameObject::new_rect(ctx, "profile_delete_panel".into(),
        Some(crate::menu_look::glass(confirm_w, confirm_h, crate::menu_look::GLASS_ROSE, true)),
        (confirm_w, confirm_h), (VW * 0.5 - confirm_w * 0.5, VH * 0.5 - confirm_h * 0.5),
        vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    confirm_bg.layer = 30;
    let confirm_title = GameObject::build("profile_delete_title")
        .size(confirm_w, 80.0)
        .position(VW * 0.5 - confirm_w * 0.5 + 40.0, VH * 0.5 - 150.0)
        .tag("ui")
        .layer(32)
        .build(ctx);
    let confirm_note = GameObject::build("profile_delete_note")
        .size(confirm_w, 60.0)
        .position(VW * 0.5 - confirm_w * 0.5 + 40.0, VH * 0.5 - 40.0)
        .tag("ui")
        .layer(32)
        .build(ctx);
    let mut confirm_btn = GameObject::new_rect(ctx, "profile_delete_confirm".into(),
        Some(crate::menu_look::glass(420.0, 100.0, crate::menu_look::GLASS_ROSE, true)),
        (420.0, 100.0), (VW * 0.5 - 460.0, VH * 0.5 + 120.0),
        vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    confirm_btn.layer = 31;
    let confirm_btn_text = GameObject::build("profile_delete_confirm_text")
        .size(420.0, 100.0)
        .position(VW * 0.5 - 460.0, VH * 0.5 + 120.0 + (100.0 - 36.0) * 0.5)
        .tag("ui")
        .layer(32)
        .build(ctx);
    let mut cancel_btn = GameObject::new_rect(ctx, "profile_delete_cancel".into(),
        Some(crate::menu_look::glass(420.0, 100.0, crate::menu_look::GLASS_LILAC, true)),
        (420.0, 100.0), (VW * 0.5 + 40.0, VH * 0.5 + 120.0),
        vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    cancel_btn.layer = 31;
    let cancel_btn_text = GameObject::build("profile_delete_cancel_text")
        .size(420.0, 100.0)
        .position(VW * 0.5 + 40.0, VH * 0.5 + 120.0 + (100.0 - 36.0) * 0.5)
        .tag("ui")
        .layer(32)
        .build(ctx);
    for (n, o) in [
        ("profile_delete_dim", dim),
        ("profile_delete_panel", confirm_bg),
        ("profile_delete_title", confirm_title),
        ("profile_delete_note", confirm_note),
        ("profile_delete_confirm", confirm_btn),
        ("profile_delete_confirm_text", confirm_btn_text),
        ("profile_delete_cancel", cancel_btn),
        ("profile_delete_cancel_text", cancel_btn_text),
    ] {
        scene = scene.with_object(n, o);
    }

    let mut scene = scene.on_enter(|canvas| {
        // Reset the camera to a known zoom-1.0 state so the slot bands and their
        // labels render in the same coordinate space (mirrors the gameover scene;
        // inheriting a leftover camera from the menu can shift UI text sideways).
        canvas.set_camera(Camera::new((VW, VH), (VW, VH)));
        canvas.set_var("profile_delete_pending", -1i32);
        crate::menu_look::register_page_tick(canvas);
        // Register the click handler once (guard against re-registration).
        if !matches!(canvas.get_var("profile_events_registered"), Some(Value::Bool(true))) {
            canvas.on_mouse_press(move |c, btn, pos| {
                if !c.is_scene("profile") || btn != MouseButton::Left {
                    return;
                }
                let world = c.screen_to_world(pos);
                let hit = |obj: &GameObject| {
                    world.0 >= obj.position.0 && world.0 <= obj.position.0 + obj.size.0
                        && world.1 >= obj.position.1 && world.1 <= obj.position.1 + obj.size.1
                };
                // If a delete-confirmation modal is showing, only its buttons are active.
                let pending = match c.get_var("profile_delete_pending") {
                    Some(Value::I32(v)) => v,
                    _ => -1,
                };
                if pending >= 0 {
                    if let Some(obj) = c.get_game_object("profile_delete_confirm") {
                        if hit(obj) {
                            crate::profile::delete_profile(pending as usize);
                            c.set_var("profile_delete_pending", -1i32);
                            update_profile_delete_confirm(c);
                            apply_profile_text(c);
                            return;
                        }
                    }
                    if let Some(obj) = c.get_game_object("profile_delete_cancel") {
                        if hit(obj) {
                            c.set_var("profile_delete_pending", -1i32);
                            update_profile_delete_confirm(c);
                            return;
                        }
                    }
                    return; // modal blocks everything else
                }
                // Per-slot DELETE buttons.
                for i in 0..crate::profile::SLOT_COUNT {
                    if let Some(obj) = c.get_game_object(&format!("profile_delete_btn_{i}")) {
                        if hit(obj) {
                            c.set_var("profile_delete_pending", i as i32);
                            update_profile_delete_confirm(c);
                            return;
                        }
                    }
                }
                // Slot buttons.
                for i in 0..crate::profile::SLOT_COUNT {
                    if let Some(obj) = c.get_game_object(&format!("profile_slot_{i}")) {
                        if hit(obj) {
                            select_profile_and_continue(c, i);
                            return;
                        }
                    }
                }
            });
            canvas.set_var("profile_events_registered", true);
        }

        // Title + slot labels. Applied here AND re-applied via on_update while
        // profile_text_dirty is set, so the boot scene gets the correct scale.
        apply_profile_text(canvas);
        update_profile_delete_confirm(canvas); // ensure the modal is hidden on entry
        canvas.set_var("profile_text_dirty", true);

        // Re-apply labels once the canvas has been sized (fixes the boot scene,
        // where virtual_scale() is still 1.0 when on_enter runs) or resized
        // (the font size / width bake in virtual_scale() at set time, so they
        // must be re-applied when the scale changes to keep the labels aligned
        // with the virtual-coordinate bands). Tracks the last-applied scale in
        // a canvas var (no captured Cell) so it stays register-once safe.
        if !matches!(canvas.get_var("profile_text_update_registered"), Some(Value::Bool(true))) {
            canvas.on_update(move |c| {
                if !c.is_scene("profile") { return; }
                let s = c.virtual_scale();
                let dirty = matches!(c.get_var("profile_text_dirty"), Some(Value::Bool(true)));
                let last = match c.get_var("profile_text_last_scale") {
                    Some(Value::F32(v)) => v,
                    _ => -1.0f32,
                };
                if !dirty && (last - s).abs() < 0.0001 { return; }
                c.set_var("profile_text_last_scale", s);
                apply_profile_text(c);
                update_profile_delete_confirm(c);
                c.set_var("profile_text_dirty", false);
            });
            canvas.set_var("profile_text_update_registered", true);
        }
    });

    scene
}

pub fn build_menu_scene(ctx: &mut Context) -> Scene {
    // Frame-rate readout (Android only).
    //
    // Added because "is it choppy?" has needed answering repeatedly and the
    // platform will not tell us: `SurfaceFlinger --latency` reports nothing for
    // this surface and perf counters are locked down on retail devices, so
    // every previous answer was inferred from CPU time rather than measured.
    let mut menu_fps_text = GameObject::build("menu_fps_text")
        .size(420.0, 60.0)
        .position(40.0, MENU_Y + 30.0)
        .build(ctx);
    menu_fps_text.visible = mobile_controls_enabled();

    // The look lives in `menu_look`: the living sky, the hero swing, the logo
    // and the list (its hit areas keep the names wired below).
    let mut scene = Scene::new("menu");
    for (id, obj) in crate::menu_look::menu_objects(ctx) {
        scene = scene.with_object(&id, obj);
    }
    let scene = scene.with_object("menu_fps_text", menu_fps_text);
    // The META counter: top right, fixed to the screen (it stays put while
    // the menu pans up into the shop), drawn like the coin counter.
    let mut meta_back = crate::menu_look::glass_obj(ctx, "menu_meta_back", (700.0, 176.0), (VW - 760.0, 28.0),
        [170, 140, 255], true, vec!["ui".into()]);
    meta_back.ignore_zoom = true;
    meta_back.layer = 12_099;
    let mut meta_badge = GameObject::new_rect(ctx, "menu_meta_badge".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (640.0, 168.0), 0.0),
            image: std::sync::Arc::new(crate::hud::meta_counter_img(0)), color: None }),
        (640.0, 168.0), (VW - 740.0, 32.0), vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    meta_badge.ignore_zoom = true;
    meta_badge.layer = 12_100;
    let scene = scene.with_object("menu_meta_back", meta_back).with_object("menu_meta_badge", meta_badge);

    // Embed shop objects at y=0..VH (camera pans from VH to 0 to reveal them)
    shop::extend_with_shop(ctx, scene)
        .with_event(
            GameEvent::KeyPress {
                key: Key::Named(NamedKey::Space),
                action: Action::Custom { name: "goto_game".into() },
                target: Target::name("start_btn"),
                modifiers: None,
            },
            Target::name("start_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_game".into() },
                target: Target::name("start_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("start_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_shop".into() },
                target: Target::name("menu_shop_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_shop_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_menu_settings".into() },
                target: Target::name("menu_settings_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_settings_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_achievements".into() },
                target: Target::name("menu_achievements_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_achievements_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_stats".into() },
                target: Target::name("menu_stats_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_stats_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_daily_reward".into() },
                target: Target::name("menu_daily_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_daily_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "goto_profile".into() },
                target: Target::name("menu_profile_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("menu_profile_btn"),
        )
        .on_enter(|canvas| {
            // Returning to main menu is the only transition that stops in-game music.
            audio_state::stop_game_bgm();
            // Only start a new menu track if none is currently playing.
            // This prevents music from restarting when backing out of Settings/Shop/etc.
            if !audio_state::has_menu_bgm() || audio_state::menu_bgm_finished() {
                let random_idx = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos() as usize)
                    .unwrap_or(0)) % MENU_TRACKS.len();
                play_menu_track(canvas, random_idx);
            }
            // World is 2×VH tall: shop at y=0..VH, menu at y=VH..2VH.
            // Camera starts pointing at menu (y=MENU_Y). goto_shop lerps y to 0.
            let mut cam = Camera::new((VW, MENU_Y + VH), (VW, VH));
            cam.position = (0.0, MENU_Y);
            canvas.set_camera(cam);
            canvas.set_var("menu_cam_target_y", MENU_Y);
            canvas.set_var("menu_cam_last_target", MENU_Y);
            canvas.set_var("menu_cam_from_y", MENU_Y);
            canvas.set_var("menu_cam_t", MENU_CLIMB_TICKS);
            canvas.set_var("menu_in_shop", false);

            push_menu_press_handler(canvas);

            canvas.set_var("menu_text_dirty", true);
            // The list starts on PLAY; the marker snaps to it.
            canvas.set_var("menu_sel", 0i32);
            // No launch carried over; the UI slides in (`menu_look::tick_launch`).
            canvas.set_var("menu_launch_t", 0i32);
            canvas.set_var("menu_intro_t", 1i32);
            canvas.set_var("menu_ui_base_taken", false);
            canvas.set_var("menu_marker_y", 0.0f32);
            canvas.set_var("menu_link_hover", -1i32);
            crate::menu_look::apply_menu_cat(canvas);
            // Normal is the core mode; Casual sits to its left, Boss Rush to its right.
            canvas.set_var("game_mode_idx", crate::mode::GameMode::Normal.index());


            let menu_key_registered = matches!(canvas.get_var("menu_key_registered"), Some(Value::Bool(true)));
            if !menu_key_registered {
                canvas.on_key_press({
                    move |c, key| {
                        if !c.is_scene("menu") { return; }
                        // In shop: route all keystrokes to the carousel handler.
                        if c.get_bool("menu_in_shop") {
                            shop::handle_shop_key(c, key);
                            return;
                        }
                        // B key: the boss-order testing menu.
                        //
                        // Main menu only, deliberately. The roster is read at
                        // arena entry once per fight, so an order set here
                        // applies from the next teleport — it never needs to be
                        // reachable mid-fight, and a modal opened during one is
                        // the shape that caused the upgrade-node soft-lock.
                        if *key == Key::Character("b".into()) {
                            c.load_scene("boss_order");
                            return;
                        }
                        // L key: skip to the next menu track.
                        if *key == Key::Character("l".into()) {
                            let cur = c.get_i32("menu_bgm_track_index").max(0) as usize;
                            let next = (cur + 1) % MENU_TRACKS.len();
                            play_menu_track(c, next);
                            return;
                        }
                        // Up and down move the lit item; Enter opens it.
                        match key {
                            Key::Named(NamedKey::ArrowUp) => { crate::menu_look::step_selection(c, -1); return; }
                            Key::Named(NamedKey::ArrowDown) => { crate::menu_look::step_selection(c, 1); return; }
                            Key::Named(NamedKey::Enter) => {
                                let name = crate::menu_look::selected_button(c);
                                activate_menu_button(c, name);
                                return;
                            }
                            _ => {}
                        }
                        let delta = match key {
                            Key::Named(NamedKey::ArrowLeft)  => -1,
                            Key::Named(NamedKey::ArrowRight) =>  1,
                            _ => 0,
                        };
                        if delta != 0 {
                            step_game_mode(c, delta);
                        }
                    }
                });

                canvas.on_key_release(|c, key| {
                    if !c.is_scene("menu") { return; }
                    if c.get_bool("menu_in_shop") {
                        shop::handle_shop_key_release(c, key);
                    }
                });
                canvas.set_var("menu_key_registered", true);
            }

            let menu_anim_registered = matches!(canvas.get_var("menu_anim_registered"), Some(Value::Bool(true)));
            if !menu_anim_registered {
                canvas.on_update(|c| {
                    if !c.is_scene("menu") { return; }
                    // The META counter follows the profile (bought, earned, K).
                    let meta = crate::profile::profile().lock().unwrap().meta_currency;
                    if c.get_i32("menu_meta_shown") != meta as i32 {
                        c.set_var("menu_meta_shown", meta as i32);
                        if let Some(o) = c.get_game_object_mut("menu_meta_badge") {
                            o.set_image(Image { shape: ShapeType::Rectangle(0.0, (640.0, 168.0), 0.0),
                                image: std::sync::Arc::new(crate::hud::meta_counter_img(meta)), color: None });
                        }
                    }

                    // Consume a queued scene switch. load_scene cannot be
                    // called reliably from inside a mouse-press callback, so
                    // the press handler queues the target here.
                    match c.get_var("queue_scene") {
                        Some(Value::Str(s)) if !s.is_empty() => {
                            let scene = s.clone();
                            c.set_var("queue_scene", "");
                            crate::transition::go(c, &scene);
                            return;
                        }
                        _ => {}
                    }

                    // Re-apply menu text whenever the canvas scale changes
                    // (window resize), so labels stay aligned with their button
                    // hit-boxes. `menu_text_dirty` is also set on scene entry.
                    let cur_scale = c.virtual_scale();
                    let last_scale = match c.get_var("menu_text_last_scale") { Some(Value::F32(v)) => v, _ => -1.0 };
                    if (last_scale - cur_scale).abs() >= 0.0001 {
                        c.set_var("menu_text_last_scale", cur_scale);
                        c.set_var("menu_text_dirty", true);
                    }

                    // ── Frame-rate readout ──────────────────────────────
                    if mobile_controls_enabled() {
                        let n = c.get_i32("fps_frames") + 1;
                        c.set_var("fps_frames", n);
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        let last = match c.get_var("fps_last_ms") {
                            Some(Value::I32(v)) => v as i64,
                            _ => 0,
                        };
                        if last == 0 {
                            c.set_var("fps_last_ms", (now & 0x3fff_ffff) as i32);
                        } else {
                            let elapsed = (now & 0x3fff_ffff) - last;
                            if elapsed >= 1000 {
                                let fps = (n as f32) * 1000.0 / elapsed as f32;
                                c.set_var("fps_frames", 0i32);
                                c.set_var("fps_last_ms", (now & 0x3fff_ffff) as i32);
                                if let Some(font) = ui_font() {
                                    let sc = c.virtual_scale();
                                    let msg = format!("{fps:.0} fps");
                                    if let Some(obj) = c.get_game_object_mut("menu_fps_text") {
                                        obj.set_drawable(Box::new(ui_text_spec(
                                            &msg, &font, 26.0 * sc,
                                            Color(255, 230, 130, 220), 400.0 * sc)));
                                    }
                                }
                            }
                        }
                    }

                    c.profile_stage("m_fps");
                    // ── Camera climb (shop ↔ menu) ──────────────────────
                    // An eased glide over a fixed time, not a chase: it starts
                    // gently, carries through the middle and settles, which is
                    // what makes the shop read as the sky above the menu rather
                    // than a cut. A new target restarts from wherever it is.
                    let target_y = c.get_f32("menu_cam_target_y");
                    if c.get_f32("menu_cam_last_target") != target_y {
                        let from = c.camera().map_or(target_y, |cam| cam.position.1);
                        c.set_var("menu_cam_last_target", target_y);
                        c.set_var("menu_cam_from_y", from);
                        c.set_var("menu_cam_t", 0i32);
                    }
                    let t = (c.get_i32("menu_cam_t") + 1).min(MENU_CLIMB_TICKS);
                    c.set_var("menu_cam_t", t);
                    let from = c.get_f32("menu_cam_from_y");
                    let k = t as f32 / MENU_CLIMB_TICKS as f32;
                    let e = if k < 0.5 { 4.0 * k * k * k } else { 1.0 - (-2.0 * k + 2.0).powi(3) * 0.5 };
                    if let Some(cam) = c.camera_mut() {
                        cam.position.1 = from + (target_y - from) * e;
                    }

                    c.profile_stage("m_cam");
                    // ── Shop carousel tick ──────────────────────────────
                    if c.get_bool("menu_in_shop") {
                        shop::tick_shop(c);
                        shop::tick_shop_preview(c);
                    }
                    // Shown only on the UPGRADES page; hides itself elsewhere.
                    crate::shop_demos::tick_demo(c);

                    c.profile_stage("m_shop");
                    // ── Menu BGM alternation ───────────────────────────
                    // `is_playable` guards the chain: with no audio device a
                    // handle is finished the moment it exists, and without this
                    // that means a fresh sound every frame forever.
                    if audio_state::menu_bgm_playable() && audio_state::menu_bgm_finished() {
                        let cur = c.get_i32("menu_bgm_track_index").max(0) as usize;
                        let next = (cur + 1) % MENU_TRACKS.len();
                        play_menu_track(c, next);
                    }

                    c.profile_stage("m_bgm");
                    if c.get_bool("menu_text_dirty") {
                        crate::menu_look::draw_menu_text(c);
                        c.set_var("menu_text_dirty", false);
                    }

                    c.profile_stage("m_text");
                    // The living sky and the swing (always: they are on screen
                    // through the climb), hover and the marker (menu only).
                    crate::menu_look::tick_menu_look(c);
                    c.profile_stage("m_look");
                });
                canvas.set_var("menu_anim_registered", true);
            }

            // PLAY: the menu takes itself apart and the camera dives into the
            // swinging cat (`menu_look::tick_launch`), which then calls
            // `start_game`; the run opens zoomed in on the player and pulls out.
            canvas.register_custom_event("goto_game".into(), |c| {
                if c.get_i32("menu_launch_t") > 0 || crate::transition::busy(c) { return; }
                c.set_var("menu_launch_t", 1i32);
            });
            canvas.register_custom_event("start_game_now".into(), |c| {
                let idx = c.get_i32("game_mode_idx").max(0);
                let mode = crate::mode::GameMode::from_index(idx);
                crate::mode::set_mode(c, mode);
                // PLAY from the menu is never the daily challenge.
                c.set_var(crate::daily::DAILY_PENDING_VAR, false);
                // `boss_mode_active` is now only the debug/test override, so it
                // must NOT be set from the mode picker — `mode.has_bosses()`
                // decides whether fights happen.
                c.set_var("boss_mode_active", false);
                c.set_var("menu_launch_t", 0i32);
                c.set_var("game_intro_t", crate::constants::GAME_INTRO_TICKS);
                c.load_scene("game");
                // The cut, covered: the run's camera opens on a lilac flash
                // that fades as it pulls out of the player.
                if let Some(cam) = c.camera_mut() {
                    cam.flash_with(Color(235, 220, 255, 230), 0.45, FlashMode::FadeOut, FlashEase::Smooth, 1.0, 0.05);
                }
            });
            // Pan camera upward into the shop region (no scene switch).
            canvas.register_custom_event("goto_shop".into(), |c| {
                c.set_var("menu_cam_target_y", 0.0f32);
                c.set_var("menu_in_shop", true);
                shop::init_shop(c);
            });
            // Back: if on carousel screen return to categories; otherwise return to menu.
            canvas.register_custom_event("shop_back".into(), |c| {
                if !shop_ready(c) { return; }
                if c.get_i32("shop_screen") == 1 {
                    shop::show_categories(c);
                } else {
                    c.set_var("menu_cam_target_y", MENU_Y);
                    c.set_var("menu_in_shop", false);
                }
            });
            // Legacy single-purchase event, kept so any older binding still
            // does something sensible; the upgrade carousel is the real path.
            canvas.register_custom_event("buy_perm_heart".into(), |c| {
                if c.get_i32("shop_active_category") != 4 { return; }
                shop::buy_selected_upgrade(c);
            });
            // Confirm selection for the active category. Cosmetic categories
            // equip and return to the grid; the upgrade category buys a rank
            // and stays put, because buying ranks is a repeated action.
            canvas.register_custom_event("shop_select".into(), |c| {
                if !shop_ready(c) { return; }
                let sel = c.get_i32("shop_selected");
                let cat = c.get_i32("shop_active_category");
                if shop::is_ability_category(cat) {
                    shop::select_ability(c, sel.max(0) as usize);
                    return;
                }
                if shop::is_upgrade_category(cat) {
                    shop::buy_selected_upgrade(c);
                    return;
                }
                // A cat or a rope style has to be OWNED to be equipped:
                // SELECT buys it first when it is not (or says why not, and
                // stays on the carousel).
                if (cat == 0 || cat == 1) && !shop::select_look(c, cat, sel.max(0) as usize) {
                    return;
                }
                match cat {
                    0 => c.set_var("player_char_selected", sel),
                    // Ropes: the carousel picks the STYLE; its colour is the
                    // swatch row, saved as it is tapped.
                    1 => {
                        c.set_var("player_rope_style_selected", sel);
                        crate::profile::save_rope_style(sel.max(0) as u32);
                    }
                    2 => c.set_var("player_bg_selected",   sel),
                    3 => c.set_var("player_trail_selected", sel),
                    _ => {}
                }
                // Persist the chosen cosmetics to the active profile.
                let (ch, rp, bg, tr) = (
                    c.get_i32("player_char_selected").max(0) as u32,
                    c.get_i32("player_rope_selected").max(0) as u32,
                    c.get_i32("player_bg_selected").max(0) as u32,
                    c.get_i32("player_trail_selected").max(0) as u32,
                );
                crate::profile::save_cosmetics(ch, rp, bg, tr);
                // The menu's swinging cat wears it straight away.
                crate::menu_look::apply_menu_cat(c);
                shop::show_categories(c);
            });
            // Category button events — each opens the carousel for that category.
            // Carousel arrows. Route through the same function the arrow keys
            // use, so a tap and a key can never disagree about what "next" means.
            canvas.register_custom_event("shop_prev".into(), |c| {
                if !shop_ready(c) { return; }
                shop::handle_shop_key(c, &Key::Named(NamedKey::ArrowLeft));
                shop::handle_shop_key_release(c, &Key::Named(NamedKey::ArrowLeft));
            });
            canvas.register_custom_event("shop_next".into(), |c| {
                if !shop_ready(c) { return; }
                shop::handle_shop_key(c, &Key::Named(NamedKey::ArrowRight));
                shop::handle_shop_key_release(c, &Key::Named(NamedKey::ArrowRight));
            });
            for i in 0..crate::shop::SHOP_ROPE_COLORS.len() {
                canvas.register_custom_event(format!("shop_rope_color_{i}"), move |c| {
                    if !shop_ready(c) { return; }
                    shop::select_rope_color(c, i);
                });
            }
            canvas.register_custom_event("shop_cat_0".into(), |c| { if shop_ready(c) { shop::show_carousel(c, 0); } });
            canvas.register_custom_event("shop_cat_1".into(), |c| { if shop_ready(c) { shop::show_carousel(c, 1); } });
            canvas.register_custom_event("shop_cat_2".into(), |c| { if shop_ready(c) { shop::show_carousel(c, 2); } });
            canvas.register_custom_event("shop_cat_3".into(), |c| { if shop_ready(c) { shop::show_carousel(c, 3); } });
            canvas.register_custom_event("shop_cat_4".into(), |c| { if shop_ready(c) { shop::show_carousel(c, 4); } });
            canvas.register_custom_event("shop_cat_5".into(), |c| { if shop_ready(c) { shop::show_carousel(c, 5); } });
            canvas.register_custom_event("goto_menu_settings".into(), |c| crate::transition::go(c, "menu_settings"));
            canvas.register_custom_event("goto_achievements".into(), |c| crate::transition::go(c, "achievements"));
            canvas.register_custom_event("goto_stats".into(), |c| crate::transition::go(c, "stats"));
            canvas.register_custom_event("goto_daily_reward".into(), |c| crate::transition::go(c, "daily_reward"));
            canvas.register_custom_event("goto_profile".into(), |c| crate::transition::go(c, "profile"));
        })
}

// ── Stand-alone Settings scene (accessible from main menu) ──────────────────
// Mirrors the in-game settings panel: slider tracks + thumbs, same layout.

// Layout constants — identical to bootstrap.rs / build_scene.rs.
const MS_SLIDER_TRACK_W: f32 = 1400.0;
const MS_SLIDER_TRACK_H: f32 = 24.0;
const MS_SLIDER_THUMB_W: f32 = 60.0;
const MS_SLIDER_THUMB_H: f32 = 80.0;
const MS_SLIDER_TRACK_X: f32 = (VW - MS_SLIDER_TRACK_W) / 2.0;
const MS_SLIDER_Y: [f32; 3] = [760.0, 990.0, 1220.0];
const MS_SLIDER_VARS:   [&str; 3] = ["vol_master", "vol_music", "vol_sound"];
const MS_SLIDER_THUMBS: [&str; 3] = ["ms_slider_master_thumb", "ms_slider_music_thumb", "ms_slider_sound_thumb"];
const MS_SLIDER_TRACKS: [&str; 3] = ["ms_slider_master_track", "ms_slider_music_track", "ms_slider_sound_track"];
const MS_LABEL_NAMES:   [&str; 3] = ["ms_settings_label_0", "ms_settings_label_1", "ms_settings_label_2"];

fn ms_position_slider_thumbs(c: &mut Canvas) {
    // This page's own slider heights (the in-game panel keeps its own).
    for i in 0..3 {
        let vol = volume_value(c, MS_SLIDER_VARS[i], 1.0);
        let x = MS_SLIDER_TRACK_X + vol * (MS_SLIDER_TRACK_W - MS_SLIDER_THUMB_W);
        let y = MS_SLIDER_Y[i] - (MS_SLIDER_THUMB_H - MS_SLIDER_TRACK_H) / 2.0;
        if let Some(o) = c.get_game_object_mut(MS_SLIDER_THUMBS[i]) { o.position = (x, y); }
    }
}

fn menu_settings_update_labels(c: &mut Canvas) {
    update_volume_labels(c, MS_LABEL_NAMES);
}

/// The graphics row under the sliders: a label and LOW / MEDIUM / HIGH.
const MS_GFX_Y: f32 = 1440.0;
/// The effect switches under it.
const MS_FX_Y: f32 = 1660.0;
const MS_FX_IDS: [&str; 3] = ["ms_fx_0", "ms_fx_1", "ms_fx_2"];
const MS_GFX_BTN: (f32, f32) = (420.0, 120.0);
const MS_GFX_IDS: [&str; 3] = ["ms_gfx_low", "ms_gfx_med", "ms_gfx_high"];

fn ms_gfx_x(i: usize) -> f32 {
    let row = 3.0 * MS_GFX_BTN.0 + 2.0 * 50.0;
    (VW - row) * 0.5 + i as f32 * (MS_GFX_BTN.0 + 50.0)
}

/// Draw the graphics row: the chosen quality lit gold.
fn ms_gfx_refresh(c: &mut Canvas) {
    let current = crate::graphics::quality();
    let Some(font) = ui_font() else { return };
    let s = c.virtual_scale();
    if let Some(o) = c.get_game_object_mut("ms_gfx_label") {
        o.set_drawable(Box::new(ui_text_spec("GRAPHICS", &font, 44.0 * s, Color(230, 222, 255, 255), MS_SLIDER_TRACK_W * s)));
    }
    for (i, q) in crate::graphics::Quality::ALL.iter().enumerate() {
        let on = *q == current;
        if let Some(o) = c.get_game_object_mut(MS_GFX_IDS[i]) {
            o.set_image(crate::menu_look::glass(MS_GFX_BTN.0, MS_GFX_BTN.1,
                if on { crate::menu_look::GLASS_GOLD } else { crate::menu_look::GLASS_LILAC }, on));
        }
        if let Some(o) = c.get_game_object_mut(&format!("{}_text", MS_GFX_IDS[i])) {
            o.set_drawable(Box::new(ui_text_spec(q.label(), &font, 38.0 * s,
                if on { Color(255, 236, 170, 255) } else { Color(215, 205, 245, 220) }, MS_GFX_BTN.0 * s)));
        }
    }
}

/// Draw the effect switches: lit gold when on.
fn ms_fx_refresh(c: &mut Canvas) {
    let Some(font) = ui_font() else { return };
    let s = c.virtual_scale();
    if let Some(o) = c.get_game_object_mut("ms_fx_label") {
        o.set_drawable(Box::new(ui_text_spec("EFFECTS", &font, 44.0 * s, Color(230, 222, 255, 255), MS_SLIDER_TRACK_W * s)));
    }
    for (i, (bit, name)) in crate::graphics::FX_ALL.iter().enumerate() {
        let on = crate::graphics::fx_on(*bit);
        if let Some(o) = c.get_game_object_mut(MS_FX_IDS[i]) {
            o.set_image(crate::menu_look::glass(MS_GFX_BTN.0, MS_GFX_BTN.1,
                if on { crate::menu_look::GLASS_GOLD } else { crate::menu_look::GLASS_LILAC }, on));
        }
        if let Some(o) = c.get_game_object_mut(&format!("{}_text", MS_FX_IDS[i])) {
            o.set_drawable(Box::new(ui_text_spec(&format!("{name}  {}", if on { "ON" } else { "OFF" }), &font, 36.0 * s,
                if on { Color(255, 236, 170, 255) } else { Color(175, 175, 205, 200) }, MS_GFX_BTN.0 * s)));
        }
    }
}

fn ms_update_menu_bgm(c: &Canvas) {
    audio_state::set_menu_bgm_volume(music_volume(c, 0.18));
}

pub fn build_menu_settings_scene(ctx: &mut Context) -> Scene {
    // The sliders sit on one glass card.
    let panel = crate::menu_look::glass_obj(ctx, "ms_panel", (MS_SLIDER_TRACK_W + 400.0, 1380.0),
        (MS_SLIDER_TRACK_X - 200.0, MS_SLIDER_Y[0] - 260.0), crate::menu_look::GLASS_LILAC, false, vec!["ui".into()]);

    // Three label objects — one above each slider track.
    let make_ms_label = |ctx: &mut Context, id: &str, y: f32| {
        let mut obj = GameObject::build(id)
            .size(MS_SLIDER_TRACK_W, 80.0)
            .position(MS_SLIDER_TRACK_X, y)
            .tag("ui")
            .build(ctx);
        obj.ignore_zoom = true;
        obj.layer = 12_004;
        obj
    };
    let ms_label_0 = make_ms_label(ctx, MS_LABEL_NAMES[0], MS_SLIDER_Y[0] - 100.0);
    let ms_label_1 = make_ms_label(ctx, MS_LABEL_NAMES[1], MS_SLIDER_Y[1] - 100.0);
    let ms_label_2 = make_ms_label(ctx, MS_LABEL_NAMES[2], MS_SLIDER_Y[2] - 100.0);

    // Slider tracks
    let make_ms_track = |ctx: &mut Context, id: &str, y: f32| {
        GameObject::new_rect(ctx, id.into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (MS_SLIDER_TRACK_W, MS_SLIDER_TRACK_H), 0.0), image: solid(110, 160, 235, 130).into(), color: None }),
            (MS_SLIDER_TRACK_W, MS_SLIDER_TRACK_H), (MS_SLIDER_TRACK_X, y),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    let ms_track_0 = make_ms_track(ctx, MS_SLIDER_TRACKS[0], MS_SLIDER_Y[0]);
    let ms_track_1 = make_ms_track(ctx, MS_SLIDER_TRACKS[1], MS_SLIDER_Y[1]);
    let ms_track_2 = make_ms_track(ctx, MS_SLIDER_TRACKS[2], MS_SLIDER_Y[2]);

    // Slider thumbs
    let make_ms_thumb = |ctx: &mut Context, id: &str, y: f32| {
        GameObject::new_rect(ctx, id.into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (MS_SLIDER_THUMB_W, MS_SLIDER_THUMB_H), 0.0), image: solid(238, 228, 255, 255).into(), color: None }),
            (MS_SLIDER_THUMB_W, MS_SLIDER_THUMB_H), (MS_SLIDER_TRACK_X, y - (MS_SLIDER_THUMB_H - MS_SLIDER_TRACK_H) / 2.0),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    let ms_thumb_0 = make_ms_thumb(ctx, MS_SLIDER_THUMBS[0], MS_SLIDER_Y[0]);
    let ms_thumb_1 = make_ms_thumb(ctx, MS_SLIDER_THUMBS[1], MS_SLIDER_Y[1]);
    let ms_thumb_2 = make_ms_thumb(ctx, MS_SLIDER_THUMBS[2], MS_SLIDER_Y[2]);

    // Back button
    let back_btn = crate::menu_look::page_back_btn(ctx, "ms_back_btn");
    let back_text_obj = crate::menu_look::page_back_text(ctx, "ms_back_text");

    let mut gfx: Vec<(String, GameObject)> = vec![(
        "ms_gfx_label".into(),
        GameObject::build("ms_gfx_label").size(MS_SLIDER_TRACK_W, 70.0)
            .position(MS_SLIDER_TRACK_X, MS_GFX_Y - 100.0).tag("ui").build(ctx),
    )];
    for (i, id) in MS_GFX_IDS.iter().enumerate() {
        let x = ms_gfx_x(i);
        gfx.push((id.to_string(), crate::menu_look::glass_obj(ctx, id, MS_GFX_BTN, (x, MS_GFX_Y),
            crate::menu_look::GLASS_LILAC, false, vec!["ui".into(), "button".into()])));
        gfx.push((format!("{id}_text"), GameObject::build(format!("{id}_text")).size(MS_GFX_BTN.0, 60.0)
            .position(x, MS_GFX_Y + (MS_GFX_BTN.1 - 52.0) * 0.5).tag("ui").build(ctx)));
    }
    gfx.push(("ms_fx_label".into(), GameObject::build("ms_fx_label").size(MS_SLIDER_TRACK_W, 70.0)
        .position(MS_SLIDER_TRACK_X, MS_FX_Y - 100.0).tag("ui").build(ctx)));
    for (i, id) in MS_FX_IDS.iter().enumerate() {
        let x = ms_gfx_x(i);
        gfx.push((id.to_string(), crate::menu_look::glass_obj(ctx, id, MS_GFX_BTN, (x, MS_FX_Y),
            crate::menu_look::GLASS_LILAC, false, vec!["ui".into(), "button".into()])));
        gfx.push((format!("{id}_text"), GameObject::build(format!("{id}_text")).size(MS_GFX_BTN.0, 60.0)
            .position(x, MS_FX_Y + (MS_GFX_BTN.1 - 52.0) * 0.5).tag("ui").build(ctx)));
    }
    let mut scene = crate::menu_look::with_page_look(ctx, Scene::new("menu_settings"), "SETTINGS")
        .with_object("ms_panel", panel);
    for (id, o) in gfx { scene = scene.with_object(&id, o); }
    for (i, id) in MS_FX_IDS.iter().enumerate() {
        scene = scene.with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: format!("ms_fx_{i}") },
                target: Target::name(*id),
                button: Some(MouseButton::Left),
            },
            Target::name(*id),
        );
    }
    for (i, id) in MS_GFX_IDS.iter().enumerate() {
        scene = scene.with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: format!("ms_gfx_{i}") },
                target: Target::name(*id),
                button: Some(MouseButton::Left),
            },
            Target::name(*id),
        );
    }
    scene
        .with_object(MS_LABEL_NAMES[0],          ms_label_0)
        .with_object(MS_LABEL_NAMES[1],          ms_label_1)
        .with_object(MS_LABEL_NAMES[2],          ms_label_2)
        .with_object(MS_SLIDER_TRACKS[0],        ms_track_0)
        .with_object(MS_SLIDER_TRACKS[1],        ms_track_1)
        .with_object(MS_SLIDER_TRACKS[2],        ms_track_2)
        .with_object(MS_SLIDER_THUMBS[0],        ms_thumb_0)
        .with_object(MS_SLIDER_THUMBS[1],        ms_thumb_1)
        .with_object(MS_SLIDER_THUMBS[2],        ms_thumb_2)
        .with_object("ms_back_btn",              back_btn)
        .with_object("ms_back_text",             back_text_obj)
        // Use the Quartz event system for the back button so that
        // load_scene fires *within* process_mouse_press_events — after
        // objects_under_cursor is computed with the menu_settings camera.
        // A manual on_mouse_press callback calling load_scene changes the
        // camera first, causing process_mouse_press_events to hit menu
        // buttons with the wrong camera, randomly re-opening settings.
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "ms_back".into() },
                target: Target::name("ms_back_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("ms_back_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            for var in ["vol_master", "vol_music", "vol_sound"] {
                if canvas.get_var(var).is_none() { canvas.set_var(var, 1.0f32); }
            }

            canvas.set_var("ms_dragging", -1i32);

            // The with_event on ms_back_btn fires Action::Custom { name: "ms_back" }.
            // register_custom_event maps that name to the actual load_scene call.
            // Re-registered every visit so the handler is always live.
            canvas.register_custom_event("ms_back".into(), |c| crate::transition::go(c, "menu"));
            for (i, q) in crate::graphics::Quality::ALL.into_iter().enumerate() {
                canvas.register_custom_event(format!("ms_gfx_{i}"), move |c| {
                    crate::graphics::set_quality(q);
                    ms_gfx_refresh(c);
                });
            }
            ms_gfx_refresh(canvas);
            for (i, (bit, _)) in crate::graphics::FX_ALL.into_iter().enumerate() {
                canvas.register_custom_event(format!("ms_fx_{i}"), move |c| {
                    crate::graphics::toggle_fx(bit);
                    ms_fx_refresh(c);
                });
            }
            ms_fx_refresh(canvas);

            // Position thumbs immediately (virtual coords, no scale dependency).
            ms_position_slider_thumbs(canvas);

            // Mark labels dirty so the on_update tick renders them at the
            // correct window scale (virtual_scale() is guaranteed correct
            // inside on_update, unlike on_enter which may fire before the
            // first real build() pass sets actual_size).
            canvas.set_var("ms_text_dirty", true);
            // Also set them now so they're present even if the dirty-flush is
            // skipped; the on_update re-renders at the real scale afterwards.
            menu_settings_update_labels(canvas);

            crate::menu_look::register_page_tick(canvas);
            set_page_back_label(canvas, "ms_back_text");

            // Register the on_update handler once.  It flushes ms_text_dirty
            // on the first tick after scene load — when virtual_scale() is
            // guaranteed to reflect the real window dimensions.
            let ms_update_registered = matches!(
                canvas.get_var("ms_update_registered"),
                Some(Value::Bool(true))
            );
            if !ms_update_registered {
                canvas.on_update(|c| {
                    if !c.is_scene("menu_settings") { return; }
                    if c.get_bool("ms_text_dirty") {
                        ms_gfx_refresh(c);
                        ms_fx_refresh(c);
                        menu_settings_update_labels(c);
                        ms_position_slider_thumbs(c);
                        c.set_var("ms_text_dirty", false);
                    }
                });
                canvas.set_var("ms_update_registered", true);
            }

            // Mouse move — drag slider thumb.
            let ms_mouse_registered = matches!(
                canvas.get_var("ms_mouse_registered"),
                Some(Value::Bool(true))
            );
            if !ms_mouse_registered {
                canvas.on_mouse_move(|c, pos| {
                    if !c.is_scene("menu_settings") { return; }
                    let dragging = c.get_i32("ms_dragging");
                    if dragging < 0 { return; }
                    let idx = dragging as usize;
                    if idx >= 3 { return; }
                    // Menu camera has no zoom — pos is already in virtual-screen space.
                    let vol = ((pos.0 - MS_SLIDER_TRACK_X) / MS_SLIDER_TRACK_W).clamp(0.0, 1.0);
                    set_volume_value(c, MS_SLIDER_VARS[idx], vol);
                    let thumb_x = MS_SLIDER_TRACK_X + vol * (MS_SLIDER_TRACK_W - MS_SLIDER_THUMB_W);
                    let thumb_y = MS_SLIDER_Y[idx] - (MS_SLIDER_THUMB_H - MS_SLIDER_TRACK_H) / 2.0;
                    if let Some(obj) = c.get_game_object_mut(MS_SLIDER_THUMBS[idx]) {
                        obj.position = (thumb_x, thumb_y);
                    }
                    menu_settings_update_labels(c);
                    ms_update_menu_bgm(c);
                });

                // Slider click-to-set only.  Back button is handled via
                // with_event so load_scene fires inside process_mouse_press_events.
                canvas.on_mouse_press(|c, btn, pos| {
                    if !c.is_scene("menu_settings") { return; }
                    if btn != MouseButton::Left { return; }
                    let ux = pos.0;
                    let uy = pos.1;
                    if ux >= MS_SLIDER_TRACK_X && ux <= MS_SLIDER_TRACK_X + MS_SLIDER_TRACK_W {
                        for idx in 0..3usize {
                            if uy >= MS_SLIDER_Y[idx] - 40.0 && uy <= MS_SLIDER_Y[idx] + 64.0 {
                                let vol = ((ux - MS_SLIDER_TRACK_X) / MS_SLIDER_TRACK_W).clamp(0.0, 1.0);
                                set_volume_value(c, MS_SLIDER_VARS[idx], vol);
                                let thumb_x = MS_SLIDER_TRACK_X + vol * (MS_SLIDER_TRACK_W - MS_SLIDER_THUMB_W);
                                let thumb_y = MS_SLIDER_Y[idx] - (MS_SLIDER_THUMB_H - MS_SLIDER_TRACK_H) / 2.0;
                                if let Some(obj) = c.get_game_object_mut(MS_SLIDER_THUMBS[idx]) {
                                    obj.position = (thumb_x, thumb_y);
                                }
                                menu_settings_update_labels(c);
                                ms_update_menu_bgm(c);
                                c.set_var("ms_dragging", idx as i32);
                                return;
                            }
                        }
                    }
                });

                canvas.on_mouse_release(|c, _btn, _pos| {
                    if !c.is_scene("menu_settings") { return; }
                    c.set_var("ms_dragging", -1i32);
                });

                canvas.set_var("ms_mouse_registered", true);
            }

            // Key handler — A/D J/L N/M adjust sliders by 5%.
            let ms_key_registered = matches!(
                canvas.get_var("ms_key_registered"),
                Some(Value::Bool(true))
            );
            if !ms_key_registered {
                canvas.on_key_press(|c, key| {
                    if !c.is_scene("menu_settings") { return; }
                    let adjust = match key {
                        Key::Character(ch) if ch == "a" => Some(("vol_master", -0.05f32)),
                        Key::Character(ch) if ch == "d" => Some(("vol_master",  0.05f32)),
                        Key::Character(ch) if ch == "j" => Some(("vol_music",  -0.05f32)),
                        Key::Character(ch) if ch == "l" => Some(("vol_music",   0.05f32)),
                        Key::Character(ch) if ch == "n" => Some(("vol_sound",  -0.05f32)),
                        Key::Character(ch) if ch == "m" => Some(("vol_sound",   0.05f32)),
                        _ => None,
                    };
                    if let Some((var, delta)) = adjust {
                        let cur = volume_value(c, var, 1.0);
                        set_volume_value(c, var, cur + delta);
                        ms_position_slider_thumbs(c);
                        menu_settings_update_labels(c);
                        ms_update_menu_bgm(c);
                    }
                });
                canvas.set_var("ms_key_registered", true);
            }
        })
}

// ── Shared gameover infrastructure ───────────────────────────────────────────

struct GameoverTheme {
    scene_name:   &'static str,
    pfx:          &'static str,   // e.g. "go", "sun_go", "oxy_go"
    tint:         Color,
    title_w:      u32,
    title_top:    [u8; 4],
    title_bot:    [u8; 4],
    retry_rgb:    (u8, u8, u8),
    box_border:   [u8; 4],
    box_fill:     [u8; 4],
}

/// Shared on_enter setup: camera reset, dist bar fill, retry/menu button labels,
/// countup registration.  Each gameover variant calls this then adds its title.
fn gameover_on_enter_common(canvas: &mut Canvas, pfx: &str) {
    canvas.set_camera(Camera::new((VW, VH), (VW, VH)));
    let Some(font) = ui_font() else { return; };
    let s = canvas.virtual_scale();
    let dist_fill = (canvas.get_f32("last_distance") / 40000.0).clamp(0.0, 1.0);
    if let Some(obj) = canvas.get_game_object_mut(&format!("{pfx}_dist_bar")) {
        obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (600.0, 44.0), 0.0), image: bar_img(600, 44, dist_fill, 80, 220, 160).into(), color: None });
    }
    for (id_sfx, label) in [("_retry_text", "RETRY"), ("_menu_text", "MENU")] {
        if let Some(obj) = canvas.get_game_object_mut(&format!("{pfx}{id_sfx}")) {
            obj.set_drawable(Box::new(ui_text_spec(label, &font, 42.0 * s, Color(255, 255, 255, 255), 520.0 * s)));
        }
    }
    init_gameover_countup(canvas, &format!("{pfx}_stats_text"), 1000.0);
}

fn build_gameover_scene_generic(
    ctx: &mut Context,
    t: GameoverTheme,
    on_enter: impl FnMut(&mut Canvas) + Clone + 'static,
) -> Scene {
    let pfx = t.pfx;
    let bg = GameObject::new_rect(ctx, format!("{pfx}_bg").into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    let bg_tint = GameObject::new_rect(ctx, format!("{pfx}_bg_tint").into(),
        Some(scene_tint(VW + 800.0, VH, t.tint)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    let title = {
        let (w, h) = (t.title_w, 230u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let blend = py as f32 / h as f32;
            img.put_pixel(px, py, image::Rgba([
                (t.title_top[0] as f32 + (t.title_bot[0] as f32 - t.title_top[0] as f32) * blend) as u8,
                (t.title_top[1] as f32 + (t.title_bot[1] as f32 - t.title_top[1] as f32) * blend) as u8,
                (t.title_top[2] as f32 + (t.title_bot[2] as f32 - t.title_top[2] as f32) * blend) as u8,
                t.title_top[3],
            ]));
        }}
        GameObject::new_rect(ctx, format!("{pfx}_title").into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, 230.0), 0.0), image: img.into(), color: None }),
            (w as f32, 230.0), (VW / 2.0 - w as f32 / 2.0, VH * 0.20),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    let dist_bar = GameObject::new_rect(ctx, format!("{pfx}_dist_bar").into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (600.0, 44.0), 0.0), image: bar_img(600, 44, 0.0, 80, 220, 160).into(), color: None }),
        (600.0, 44.0), (VW / 2.0 - 300.0, VH * 0.37),
        vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    let make_btn = |ctx: &mut Context, id: &str, (r, g, b): (u8, u8, u8), y: f32| {
        let (w, h) = (520u32, 130u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px==0||px==w-1||py==0||py==h-1||px==1||px==w-2||py==1||py==h-2;
            img.put_pixel(px, py, image::Rgba([r, g, b, if border { 255 } else { 200 }]));
        }}
        GameObject::new_rect(ctx, id.to_string().into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, y),
            vec!["ui".into(), "button".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    let retry_btn   = make_btn(ctx, &format!("{pfx}_retry_btn"),   t.retry_rgb,     VH * 0.66);
    let go_menu_btn = make_btn(ctx, &format!("{pfx}_menu_btn"),    (50, 80, 160),   VH * 0.80);
    let stats_box = {
        let (w, h) = (1060u32, 260u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
            img.put_pixel(px, py, image::Rgba(if border { t.box_border } else { t.box_fill }));
        }}
        GameObject::new_rect(ctx, format!("{pfx}_stats_box").into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW / 2.0 - w as f32 / 2.0, VH * 0.44 - 10.0),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    let tw = t.title_w as f32;
    let title_text  = GameObject::build(&format!("{pfx}_title_text")).size(tw, 230.0).position(VW * 0.5 - tw * 0.5, VH * 0.20 + (230.0 - 74.0) / 2.0).tag("ui").build(ctx);
    let retry_text  = GameObject::build(&format!("{pfx}_retry_text")).size(520.0, 130.0).position(VW * 0.5 - 260.0, VH * 0.66 + (130.0 - 54.0) / 2.0).tag("ui").build(ctx);
    let menu_text   = GameObject::build(&format!("{pfx}_menu_text")).size(520.0, 130.0).position(VW * 0.5 - 260.0, VH * 0.80 + (130.0 - 54.0) / 2.0).tag("ui").build(ctx);
    let stats_text  = GameObject::build(&format!("{pfx}_stats_text")).size(1000.0, 240.0).position(VW * 0.5 - 500.0, VH * 0.44).tag("ui").build(ctx);
    let retry_id    = format!("{pfx}_retry_btn");
    let menu_id     = format!("{pfx}_menu_btn");
    let retry_evt   = format!("{pfx}_retry");
    let menu_evt    = format!("{pfx}_menu");
    Scene::new(t.scene_name)
        .with_object(format!("{pfx}_bg"),         bg)
        .with_object(format!("{pfx}_bg_tint"),    bg_tint)
        .with_object(format!("{pfx}_title"),      title)
        .with_object(format!("{pfx}_dist_bar"),   dist_bar)
        .with_object(format!("{pfx}_stats_box"),  stats_box)
        .with_object(&retry_id,                   retry_btn)
        .with_object(&menu_id,                    go_menu_btn)
        .with_object(format!("{pfx}_title_text"), title_text)
        .with_object(format!("{pfx}_retry_text"), retry_text)
        .with_object(format!("{pfx}_menu_text"),  menu_text)
        .with_object(format!("{pfx}_stats_text"), stats_text)
        .with_event(GameEvent::MousePress { action: Action::Custom { name: retry_evt.clone().into() }, target: Target::name(&retry_id), button: Some(MouseButton::Left) }, Target::name(&retry_id))
        .with_event(GameEvent::KeyPress   { key: Key::Named(NamedKey::Space), action: Action::Custom { name: retry_evt.clone().into() }, target: Target::name(&retry_id), modifiers: None }, Target::name(&retry_id))
        .with_event(GameEvent::MousePress { action: Action::Custom { name: menu_evt.clone().into() },  target: Target::name(&menu_id),  button: Some(MouseButton::Left) }, Target::name(&menu_id))
        .on_enter(on_enter)
}

pub fn build_gameover_scene(ctx: &mut Context) -> Scene {
    build_gameover_scene_generic(ctx,
        GameoverTheme {
                scene_name: "gameover", pfx: "go",
                tint: Color(230, 50, 50, 120), title_w: 1300,
                title_top: [255, 90, 40, 255], title_bot: [255, 0, 40, 255],
                retry_rgb: (50, 160, 90),
                box_border: [180, 200, 220, 200], box_fill: [20, 15, 30, 200],
            },
        |canvas| {
            let died_to_oxygen = matches!(canvas.get_var("died_to_oxygen"), Some(Value::Bool(true)));
            gameover_on_enter_common(canvas, "go");
            if let Some(font) = ui_font() {
                let s = canvas.virtual_scale();
                let title = if died_to_oxygen { "YOU RAN OUT OF OXYGEN" } else { "YOU FELL" };
                let size  = if died_to_oxygen { 50.0 } else { 58.0 };
                if let Some(obj) = canvas.get_game_object_mut("go_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec(title, &font, size * s, Color(0,0,0,255), 1300.0 * s)));
                }
            }
            if died_to_oxygen { canvas.set_var("died_to_oxygen", false); }
            canvas.register_custom_event("go_retry".into(), |c| c.load_scene("game"));
            canvas.register_custom_event("go_menu".into(),  |c| crate::transition::go_slow(c, "menu"));
        },
    )
}

pub fn build_gameover_sun_scene(ctx: &mut Context) -> Scene {
    build_gameover_scene_generic(ctx,
        GameoverTheme {
                scene_name: "gameover_sun", pfx: "sun_go",
                tint: Color(230, 120, 20, 150), title_w: 1900,
                title_top: [255, 160, 10, 255], title_bot: [255, 80, 10, 255],
                retry_rgb: (160, 80, 20),
                box_border: [255, 180, 80, 200], box_fill: [30, 15, 5, 200],
            },
        |canvas| {
            gameover_on_enter_common(canvas, "sun_go");
            if let Some(font) = ui_font() {
                let s = canvas.virtual_scale();
                if let Some(obj) = canvas.get_game_object_mut("sun_go_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec("YOU FLEW TOO CLOSE TO THE SUN", &font, 48.0 * s, Color(0,0,0,255), 1900.0 * s)));
                }
            }
            canvas.register_custom_event("sun_go_retry".into(), |c| c.load_scene("game"));
            canvas.register_custom_event("sun_go_menu".into(),  |c| crate::transition::go_slow(c, "menu"));
        },
    )
}

pub fn build_gameover_oxygen_scene(ctx: &mut Context) -> Scene {
    build_gameover_scene_generic(ctx,
        GameoverTheme {
                scene_name: "gameover_oxygen", pfx: "oxy_go",
                tint: Color(30, 170, 210, 150), title_w: 1700,
                title_top: [130, 240, 255, 255], title_bot: [90, 240, 255, 255],
                retry_rgb: (50, 160, 90),
                box_border: [120, 220, 255, 200], box_fill: [8, 25, 35, 200],
            },
        |canvas| {
            gameover_on_enter_common(canvas, "oxy_go");
            if let Some(font) = ui_font() {
                let s = canvas.virtual_scale();
                if let Some(obj) = canvas.get_game_object_mut("oxy_go_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec("YOU RAN OUT OF OXYGEN", &font, 52.0 * s, Color(0,0,0,255), 1700.0 * s)));
                }
            }
            canvas.register_custom_event("oxy_go_retry".into(), |c| c.load_scene("game"));
            canvas.register_custom_event("oxy_go_menu".into(),  |c| crate::transition::go_slow(c, "menu"));
        },
    )
}
// ── Achievements scene ───────────────────────────────────────────────────────
//
// One column of cards, every achievement in the gold-master card's format: a
// gold-edged glass panel, the title in gold, the description under it, and a
// check on the right once it is earned. The column scrolls (wheel, arrow keys,
// W/S, Page Up/Down, or dragging/swiping the list); cards fade out at the top
// and bottom of the window they scroll in, and a scrollbar shows where you are.

/// Card size, the gap between cards, and the window the list scrolls in.
const ACH_CARD_W: f32 = 2300.0;
const ACH_CARD_H: f32 = 196.0;
const ACH_GAP: f32 = 24.0;
const ACH_TOP: f32 = 380.0;
const ACH_BOTTOM: f32 = VH - 300.0;
/// How far into the window's edge a card fades out over.
const ACH_FADE: f32 = 150.0;

fn ach_count() -> usize { crate::achievements::page_rows().len() }

/// How far the list can scroll.
fn ach_max_scroll() -> f32 {
    let content = ach_count() as f32 * (ACH_CARD_H + ACH_GAP) - ACH_GAP;
    (content - (ACH_BOTTOM - ACH_TOP)).max(0.0)
}

/// Top-left of card `i` with the list scrolled by `scroll`.
fn ach_card_pos(i: usize, scroll: f32) -> (f32, f32) {
    ((VW - ACH_CARD_W) * 0.5, ACH_TOP + i as f32 * (ACH_CARD_H + ACH_GAP) - scroll)
}

/// Scroll the list toward `delta` more (clamped).
fn ach_scroll_by(c: &mut Canvas, delta: f32) {
    let t = (c.get_f32("ach_scroll_target") + delta).clamp(0.0, ach_max_scroll());
    c.set_var("ach_scroll_target", t);
}

/// Redraw card `i`'s words at opacity `a` (0..1).
fn ach_draw_card_text(c: &mut Canvas, i: usize, a: f32) {
    let Some(font) = ui_font() else { return };
    let s = c.virtual_scale();
    let all = crate::achievements::page_rows();
    let Some(&(_, title, desc, tier)) = all.get(i) else { return };
    let done = c.get_bool(&format!("ach_done_{i}"));
    let fade = |col: Color| Color(col.0, col.1, col.2, (col.3 as f32 * a) as u8);
    // The title in its tier's colour; not earned yet, a step quieter.
    let (r, g, b) = tier.rgb();
    let (tc, dc) = if done {
        (Color(r, g, b, 255), Color(210, 220, 235, 235))
    } else {
        (Color(r, g, b, 150), Color(170, 175, 200, 170))
    };
    // A secret stays a secret until it is earned.
    let secret = tier == crate::achievements::Tier::Secret && !done;
    let title = if secret { "???".to_string() } else { title.to_string() };
    let desc = if secret {
        format!("{}  \u{2022}  A secret. Keep playing.", tier.label())
    } else {
        format!("{}  \u{2022}  {desc}", tier.label())
    };
    if let Some(obj) = c.get_game_object_mut(&format!("ach_card_title_{i}")) {
        obj.set_drawable(Box::new(ui_text_left_spec(&title, &font, 44.0 * s, fade(tc), (ACH_CARD_W - 260.0) * s)));
    }
    if let Some(obj) = c.get_game_object_mut(&format!("ach_card_desc_{i}")) {
        obj.set_drawable(Box::new(ui_text_left_spec(&desc, &font, 28.0 * s, fade(dc), (ACH_CARD_W - 260.0) * s)));
    }
    if let Some(obj) = c.get_game_object_mut(&format!("ach_card_check_{i}")) {
        let sym = crate::scenes::game::helpers::symbol_font().unwrap_or_else(|| font.clone());
        obj.set_drawable(Box::new(ui_text_spec("\u{2713}", &sym, 56.0 * s, fade(Color(130, 255, 165, 255)), 120.0 * s)));
    }
}

/// Place every card for the current scroll, fade the ones at the edges,
/// and move the scrollbar.
fn ach_layout(c: &mut Canvas) {
    let scroll = c.get_f32("ach_scroll");
    for i in 0..ach_count() {
        let (x, y) = ach_card_pos(i, scroll);
        let a_top = ((y - (ACH_TOP - ACH_FADE)) / ACH_FADE).clamp(0.0, 1.0);
        let a_bot = (((ACH_BOTTOM + ACH_FADE) - (y + ACH_CARD_H)) / ACH_FADE).clamp(0.0, 1.0);
        // Quantised, so a card's text is only rebuilt when it visibly changes.
        let a = (a_top.min(a_bot) * 12.0).round() / 12.0;
        let on = a > 0.0;
        let done = c.get_bool(&format!("ach_done_{i}"));
        let place = |c: &mut Canvas, name: String, pos: (f32, f32), vis: bool| {
            if let Some(o) = c.get_game_object_mut(&name) { o.position = pos; o.visible = vis; }
        };
        place(c, format!("ach_card_{i}"), (x, y), on);
        place(c, format!("ach_card_title_{i}"), (x + 50.0, y + 34.0), on);
        place(c, format!("ach_card_desc_{i}"), (x + 50.0, y + 112.0), on);
        place(c, format!("ach_card_check_{i}"), (x + ACH_CARD_W - 170.0, y + (ACH_CARD_H - 96.0) * 0.5 + 6.0), on && done);
        let key = format!("ach_alpha_{i}");
        if (c.get_f32(&key) - a).abs() > 0.001 {
            c.set_var(&key, a);
            let mut img = crate::menu_look::glass(ACH_CARD_W, ACH_CARD_H, crate::menu_look::GLASS_GOLD, false);
            img.color = Some(Color(255, 255, 255, (a * 255.0) as u8));
            if let Some(o) = c.get_game_object_mut(&format!("ach_card_{i}")) { o.set_image(img); }
            if on { ach_draw_card_text(c, i, a); }
        }
    }
    // The scrollbar: only when there is somewhere to scroll.
    let max = ach_max_scroll();
    let track_h = ACH_BOTTOM - ACH_TOP;
    let bar_x = (VW + ACH_CARD_W) * 0.5 + 50.0;
    if let Some(o) = c.get_game_object_mut("ach_scroll_track") {
        o.visible = max > 0.0;
        o.position = (bar_x, ACH_TOP);
    }
    let thumb_h = (track_h * track_h / (track_h + max)).max(120.0);
    let thumb_y = ACH_TOP + if max > 0.0 { (track_h - thumb_h) * scroll / max } else { 0.0 };
    if let Some(o) = c.get_game_object_mut("ach_scroll_thumb") {
        o.visible = max > 0.0;
        o.position = (bar_x - 4.0, thumb_y);
        if (o.size.1 - thumb_h).abs() > 0.5 {
            o.size = (22.0, thumb_h);
            o.set_image(Image { shape: ShapeType::Rectangle(0.0, (22.0, thumb_h), 0.0),
                image: solid(210, 190, 255, 220).into(), color: None });
        }
    }
}

/// Wheel, keys, drag and the per-frame ease. Registered once; every handler
/// checks it is on the achievements page.
fn register_ach_scrolling(canvas: &mut Canvas) {
    if canvas.get_bool("ach_scroll_registered") { return; }
    canvas.set_var("ach_scroll_registered", true);
    canvas.on_mouse_scroll(|c, (_dx, dy)| {
        if !c.is_scene("achievements") { return; }
        // Line-based wheels report small steps, trackpads report pixels.
        let step = if dy.abs() <= 6.0 { dy * 160.0 } else { dy * 2.0 };
        ach_scroll_by(c, -step);
    });
    canvas.on_key_press(|c, key| {
        if !c.is_scene("achievements") { return; }
        let page = ACH_BOTTOM - ACH_TOP - ACH_CARD_H;
        let d = match key {
            Key::Named(NamedKey::ArrowDown) => ACH_CARD_H + ACH_GAP,
            Key::Named(NamedKey::ArrowUp) => -(ACH_CARD_H + ACH_GAP),
            Key::Character(ch) if ch == "s" => ACH_CARD_H + ACH_GAP,
            Key::Character(ch) if ch == "w" => -(ACH_CARD_H + ACH_GAP),
            Key::Named(NamedKey::PageDown) => page,
            Key::Named(NamedKey::PageUp) => -page,
            Key::Named(NamedKey::Home) => -1.0e6,
            Key::Named(NamedKey::End) => 1.0e6,
            _ => return,
        };
        ach_scroll_by(c, d);
    });
    // Drag (mouse or finger) anywhere over the list.
    canvas.on_mouse_press(|c, btn, pos| {
        if !c.is_scene("achievements") || btn != MouseButton::Left { return; }
        let over = pos.1 >= ACH_TOP - 40.0 && pos.1 <= ACH_BOTTOM + 40.0;
        c.set_var("ach_dragging", over);
        c.set_var("ach_drag_y", pos.1);
    });
    canvas.on_mouse_move(|c, pos| {
        if !c.is_scene("achievements") || !c.get_bool("ach_dragging") { return; }
        let dy = pos.1 - c.get_f32("ach_drag_y");
        c.set_var("ach_drag_y", pos.1);
        let t = (c.get_f32("ach_scroll_target") - dy).clamp(0.0, ach_max_scroll());
        c.set_var("ach_scroll_target", t);
        // A drag follows the finger directly.
        c.set_var("ach_scroll", t);
    });
    canvas.on_mouse_release(|c, _btn, _pos| {
        if c.is_scene("achievements") { c.set_var("ach_dragging", false); }
    });
    canvas.on_update(|c| {
        if !c.is_scene("achievements") { return; }
        let target = c.get_f32("ach_scroll_target").clamp(0.0, ach_max_scroll());
        c.set_var("ach_scroll_target", target);
        let cur = c.get_f32("ach_scroll");
        let next = if (target - cur).abs() < 0.5 { target } else { cur + (target - cur) * 0.22 };
        c.set_var("ach_scroll", next);
        ach_layout(c);
    });
}

pub fn build_achievements_scene(ctx: &mut Context) -> Scene {
    let back_btn = crate::menu_look::page_back_btn(ctx, "ach_back_btn");

    let mut cards: Vec<(String, GameObject)> = Vec::new();
    for i in 0..ach_count() {
        let (x, y) = ach_card_pos(i, 0.0);
        cards.push((format!("ach_card_{i}"), crate::menu_look::glass_obj(ctx, &format!("ach_card_{i}"),
            (ACH_CARD_W, ACH_CARD_H), (x, y), crate::menu_look::GLASS_GOLD, false, vec!["ui".into()])));
        cards.push((format!("ach_card_title_{i}"), GameObject::build(format!("ach_card_title_{i}"))
            .size(ACH_CARD_W - 260.0, 60.0).position(x + 50.0, y + 34.0).tag("ui").build(ctx)));
        cards.push((format!("ach_card_desc_{i}"), GameObject::build(format!("ach_card_desc_{i}"))
            .size(ACH_CARD_W - 260.0, 46.0).position(x + 50.0, y + 112.0).tag("ui").build(ctx)));
        cards.push((format!("ach_card_check_{i}"), GameObject::build(format!("ach_card_check_{i}"))
            .size(120.0, 96.0).position(x + ACH_CARD_W - 170.0, y + (ACH_CARD_H - 96.0) * 0.5 + 6.0).tag("ui").build(ctx)));
    }
    let track_h = ACH_BOTTOM - ACH_TOP;
    let mut track = GameObject::new_rect(ctx, "ach_scroll_track".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (14.0, track_h), 0.0), image: solid(150, 128, 225, 70).into(), color: None }),
        (14.0, track_h), (0.0, ACH_TOP), vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    track.visible = false;
    let mut thumb = GameObject::new_rect(ctx, "ach_scroll_thumb".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (22.0, 120.0), 0.0), image: solid(210, 190, 255, 220).into(), color: None }),
        (22.0, 120.0), (0.0, ACH_TOP), vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    thumb.visible = false;
    let back_text  = crate::menu_look::page_back_text(ctx, "ach_back_text");

    let mut scene = crate::menu_look::with_page_look(ctx, Scene::new("achievements"), "ACHIEVEMENTS");
    for (id, o) in cards { scene = scene.with_object(&id, o); }
    scene
        .with_object("ach_progress", GameObject::build("ach_progress")
            .size(1200.0, 60.0).position((VW - 1200.0) * 0.5, ACH_TOP - 110.0).tag("ui").build(ctx))
        .with_object("ach_scroll_track", track)
        .with_object("ach_scroll_thumb", thumb)
        .with_object("ach_back_btn",   back_btn)
        .with_object("ach_back_text",  back_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "ach_back".into() },
                target: Target::name("ach_back_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("ach_back_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            crate::menu_look::register_page_tick(canvas);
            register_ach_scrolling(canvas);
            set_page_back_label(canvas, "ach_back_text");
            let earned = {
                let p = crate::profile::profile();
                let g = p.lock().unwrap();
                g.achievements.clone()
            };
            let rows = crate::achievements::page_rows();
            let live = crate::achievements::ACHIEVEMENTS.len();
            let got = crate::achievements::ACHIEVEMENTS.iter().filter(|a| earned.iter().any(|e| e == a.id)).count();
            if let (Some(font), s) = (ui_font(), canvas.virtual_scale()) {
                if let Some(o) = canvas.get_game_object_mut("ach_progress") {
                    o.set_drawable(Box::new(ui_text_spec(&format!("{got} / {live} EARNED"), &font, 34.0 * s,
                        Color(215, 205, 255, 230), 1200.0 * s)));
                }
            }
            for (i, (id, _, _, _)) in rows.into_iter().enumerate() {
                let done = earned.iter().any(|e| e == id);
                canvas.set_var(&format!("ach_done_{i}"), done);
                // Forces the first layout to draw every card.
                canvas.set_var(&format!("ach_alpha_{i}"), -1.0f32);
            }
            canvas.set_var("ach_scroll", 0.0f32);
            canvas.set_var("ach_scroll_target", 0.0f32);
            canvas.set_var("ach_dragging", false);
            ach_layout(canvas);

            canvas.register_custom_event("ach_back".into(), |c| crate::transition::go(c, "menu"));
        })
}

/// Stats layout: the two columns' text and the glass cards behind them.
const STATS_LEFT_X: f32 = 560.0;
const STATS_RIGHT_X: f32 = 2160.0;
const STATS_COL_Y: f32 = 480.0;
const STATS_CARD_W: f32 = 1260.0;
const STATS_CARD_H: f32 = 720.0;

/// Every page's back pill reads the same.
fn set_page_back_label(c: &mut Canvas, id: &str) {
    let Some(font) = ui_font() else { return };
    let s = c.virtual_scale();
    if let Some(obj) = c.get_game_object_mut(id) {
        obj.set_drawable(Box::new(ui_text_spec(crate::menu_look::PAGE_BACK_LABEL, &font, 32.0 * s,
            Color(230, 222, 255, 255), crate::menu_look::PAGE_BACK_SIZE.0 * s)));
    }
}

// ── Stats scene ──────────────────────────────────────────────────────────────
// Two-column layout.  Stats are placeholders (not yet tracked).
// Left col:  Coins Collected / Deaths / Best Distance / Turrets Destroyed
// Right col: Hooks / Time Played / Bosses Defeated / Bounces

pub fn build_stats_scene(ctx: &mut Context) -> Scene {
    let back_btn = crate::menu_look::page_back_btn(ctx, "stats_back_btn");

    // A glass card behind each column (they replace the old divider line).
    let card = |ctx: &mut Context, id: &str, x: f32| crate::menu_look::glass_obj(ctx, id,
        (STATS_CARD_W, STATS_CARD_H), (x - 140.0, STATS_COL_Y - 110.0), crate::menu_look::GLASS_LILAC, false, vec!["ui".into()]);
    let left_card = card(ctx, "stats_left_card", STATS_LEFT_X);
    let right_card = card(ctx, "stats_right_card", STATS_RIGHT_X);

    // Two text objects, one per column.  Populated in on_enter.
    let make_col_text = |ctx: &mut Context, id: &str, x: f32| {
        GameObject::build(id)
            .size(1380.0, 1300.0)
            .position(x, STATS_COL_Y)
            .tag("ui")
            .build(ctx)
    };
    let left_col  = make_col_text(ctx, "stats_left_col",  STATS_LEFT_X);
    let right_col = make_col_text(ctx, "stats_right_col", STATS_RIGHT_X);

    let back_text = crate::menu_look::page_back_text(ctx, "stats_back_text");

    crate::menu_look::with_page_look(ctx, Scene::new("stats"), "STATISTICS")
        .with_object("stats_left_card",  left_card)
        .with_object("stats_right_card", right_card)
        .with_object("stats_left_col",   left_col)
        .with_object("stats_right_col",  right_col)
        .with_object("stats_back_btn",   back_btn)
        .with_object("stats_back_text",  back_text)
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "stats_back".into() },
                target: Target::name("stats_back_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("stats_back_btn"),
        )
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            crate::menu_look::register_page_tick(canvas);
            set_page_back_label(canvas, "stats_back_text");
            if let Some(font) = ui_font() {
                let s = canvas.virtual_scale();

                // Left column: 4 per-profile lifetime stats.
                let lc_name = Color(185, 170, 240, 225);
                let lc_val  = Color(255, 255, 255, 255);
                let col_w   = 1380.0 * s;

                let (total_coins, deaths, best_dist, runs, bosses,
                     best_casual, best_normal, best_bossrush) = {
                    let p = crate::profile::profile();
                    let g = p.lock().unwrap();
                    (g.total_coins, g.deaths, g.best_distance, g.runs_played,
                     g.bosses_defeated,
                     g.best_distance_casual, g.best_distance_normal, g.best_distance_bossrush)
                };
                let left_stats: [(&str, String); 4] = [
                    ("COINS COLLECTED", format!("{}", total_coins)),
                    ("DEATHS",          format!("{}", deaths)),
                    ("BEST DISTANCE",   format!("{} px", best_dist)),
                    ("RUNS PLAYED",     format!("{}", runs)),
                ];
                let right_stats: [(&str, String); 4] = [
                    ("BEST  NORMAL",    format!("{} px", best_normal)),
                    ("BEST  CASUAL",    format!("{} px", best_casual)),
                    ("BEST  BOSS RUSH", format!("{} px", best_bossrush)),
                    ("BOSSES DEFEATED", format!("{}", bosses)),
                ];

                let make_stat_text = |name: &str, val: &str, font: &Font, s: f32, name_col: Color, val_col: Color, w: f32| {
                    Text::new(
                        vec![
                            Span::new(name.to_string(), 38.0 * s, Some(46.0 * s), Arc::new(font.clone()), name_col, 0.0),
                            Span::new(format!("  {}", val), 48.0 * s, Some(46.0 * s), Arc::new(font.clone()), val_col, 0.0),
                        ],
                        Some(w),
                        Align::Left,
                        None,
                    )
                };

                // Build multiline text for each column by stacking stat blocks.
                // Each stat gets 2 lines (name row, value row) with visual gap.
                // We use individual drawables stacked via a helper below.
                // For simplicity, combine stat entries into one multi-span Text
                // using newlines between entries.
                let mut left_spans: Vec<Span> = Vec::new();
                let mut right_spans: Vec<Span> = Vec::new();
                let line_h = Some(52.0 * s);
                let gap_h  = Some(28.0 * s);
                let _ = make_stat_text; // suppress unused warning

                for (i, (name, val)) in left_stats.iter().enumerate() {
                    left_spans.push(Span::new(name.to_string(), 36.0 * s, line_h, Arc::new(font.clone()), lc_name, 0.0));
                    left_spans.push(Span::new(format!("\n"), 36.0 * s, line_h, Arc::new(font.clone()), lc_name, 0.0));
                    left_spans.push(Span::new(val.to_string(), 52.0 * s, line_h, Arc::new(font.clone()), lc_val, 0.0));
                    if i < left_stats.len() - 1 {
                        left_spans.push(Span::new("\n\n".to_string(), 20.0 * s, gap_h, Arc::new(font.clone()), lc_val, 0.0));
                    }
                }
                for (i, (name, val)) in right_stats.iter().enumerate() {
                    right_spans.push(Span::new(name.to_string(), 36.0 * s, line_h, Arc::new(font.clone()), lc_name, 0.0));
                    right_spans.push(Span::new("\n".to_string(), 36.0 * s, line_h, Arc::new(font.clone()), lc_name, 0.0));
                    right_spans.push(Span::new(val.to_string(), 52.0 * s, line_h, Arc::new(font.clone()), lc_val, 0.0));
                    if i < right_stats.len() - 1 {
                        right_spans.push(Span::new("\n\n".to_string(), 20.0 * s, gap_h, Arc::new(font.clone()), lc_val, 0.0));
                    }
                }

                if let Some(obj) = canvas.get_game_object_mut("stats_left_col") {
                    obj.set_drawable(Box::new(Text::new(left_spans, Some(col_w), Align::Left, None)));
                }
                if let Some(obj) = canvas.get_game_object_mut("stats_right_col") {
                    obj.set_drawable(Box::new(Text::new(right_spans, Some(col_w), Align::Left, None)));
                }
            }

            canvas.register_custom_event("stats_back".into(), |c| crate::transition::go(c, "menu"));
        })
}

// ── Daily scene: the reward calendar and the daily challenge ─────────────────
//
// Top card: seven days of META, the day to claim lit gold, earned days
// ticked, a CLAIM button (or how long until tomorrow). Bottom card: today's
// challenge (one modifier, one distance, the same world for everyone), the
// best distance today, and PLAY.

const DAILY_CARD_W: f32 = 2600.0;
const DAILY_X: f32 = (VW - DAILY_CARD_W) * 0.5;
const DAILY_REWARD_Y: f32 = 330.0;
const DAILY_REWARD_H: f32 = 760.0;
const DAILY_CHAL_Y: f32 = 1150.0;
const DAILY_CHAL_H: f32 = 700.0;
const DAILY_TILE: (f32, f32) = (300.0, 330.0);
const DAILY_TILE_GAP: f32 = 30.0;
const DAILY_BTN: (f32, f32) = (720.0, 130.0);

fn daily_tile_x(i: usize) -> f32 {
    let row = 7.0 * DAILY_TILE.0 + 6.0 * DAILY_TILE_GAP;
    DAILY_X + (DAILY_CARD_W - row) * 0.5 + i as f32 * (DAILY_TILE.0 + DAILY_TILE_GAP)
}

fn daily_text(c: &mut Canvas, id: &str, text: &str, size: f32, col: Color, width: f32) {
    let Some(font) = ui_font() else { return };
    let s = c.virtual_scale();
    if let Some(obj) = c.get_game_object_mut(id) {
        obj.set_drawable(Box::new(ui_text_spec(text, &font, size * s, col, width * s)));
        obj.visible = true;
    }
}

fn hms(secs: u64) -> String {
    format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
}

/// Redraw the whole page from the profile.
fn daily_refresh(c: &mut Canvas) {
    let (slot, claimed) = crate::daily::reward_state();
    let streak = { let g = crate::profile::profile(); let p = g.lock().unwrap(); p.daily_streak };
    let streak_now = if claimed { streak } else if slot == 0 { 0 } else { streak };
    daily_text(c, "daily_reward_head", &format!("DAILY REWARD   \u{2022}   STREAK  {streak_now}"), 46.0,
        Color(250, 228, 170, 255), DAILY_CARD_W);
    for i in 0..7 {
        let earned = i < slot || (claimed && i == slot);
        let today = i == slot && !claimed;
        let accent = if today { crate::menu_look::GLASS_GOLD } else if earned { [130, 220, 160] } else { crate::menu_look::GLASS_LILAC };
        if let Some(o) = c.get_game_object_mut(&format!("daily_tile_{i}")) {
            let mut img = crate::menu_look::glass(DAILY_TILE.0, DAILY_TILE.1, accent, today);
            img.color = Some(Color(255, 255, 255, if earned || today { 255 } else { 170 }));
            o.set_image(img);
        }
        let (dc, mc) = if today { (Color(255, 236, 170, 255), Color(255, 240, 190, 255)) }
            else if earned { (Color(170, 235, 190, 230), Color(170, 235, 190, 230)) }
            else { (Color(190, 180, 225, 190), Color(215, 205, 245, 200)) };
        daily_text(c, &format!("daily_tile_day_{i}"), &format!("DAY {}", i + 1), 32.0, dc, DAILY_TILE.0);
        daily_text(c, &format!("daily_tile_amt_{i}"), &format!("+{}", crate::daily::REWARD_CALENDAR[i]), if i == 6 { 70.0 } else { 60.0 }, mc, DAILY_TILE.0);
        daily_text(c, &format!("daily_tile_meta_{i}"), if earned { "META  CLAIMED" } else { "META" }, 28.0, dc, DAILY_TILE.0);
    }
    if claimed {
        daily_text(c, "daily_claim_text", "CLAIMED", 44.0, Color(170, 235, 190, 255), DAILY_BTN.0);
    } else {
        daily_text(c, "daily_claim_text", &format!("CLAIM  +{}", crate::daily::REWARD_CALENDAR[slot]), 44.0,
            Color(255, 240, 190, 255), DAILY_BTN.0);
    }
    if let Some(o) = c.get_game_object_mut("daily_claim_btn") {
        let mut img = crate::menu_look::glass(DAILY_BTN.0, DAILY_BTN.1, if claimed { [130, 220, 160] } else { crate::menu_look::GLASS_GOLD }, !claimed);
        img.color = Some(Color(255, 255, 255, if claimed { 150 } else { 255 }));
        o.set_image(img);
    }

    let ch = crate::daily::today_challenge();
    let done = crate::daily::done_today();
    let best = crate::daily::best_today();
    daily_text(c, "daily_chal_head", "TODAY'S CHALLENGE", 46.0, Color(215, 200, 255, 255), DAILY_CARD_W);
    daily_text(c, "daily_chal_name", ch.modifier.name(), 86.0, Color(255, 236, 170, 255), DAILY_CARD_W);
    daily_text(c, "daily_chal_blurb", ch.modifier.blurb(), 38.0, Color(220, 225, 240, 240), DAILY_CARD_W);
    daily_text(c, "daily_chal_goal",
        &format!("Reach {} px   \u{2022}   Reward {} META   \u{2022}   Same world for everyone today", fmt_thousands(ch.goal as u64), ch.reward),
        32.0, Color(200, 195, 235, 230), DAILY_CARD_W);
    let status = if done {
        format!("COMPLETE   \u{2022}   best today {} px", fmt_thousands(best as u64))
    } else if best > 0 {
        format!("best today {} px   \u{2022}   {}% of the way", fmt_thousands(best as u64), ((best as f32 / ch.goal) * 100.0).min(99.0) as u32)
    } else {
        "not attempted yet".to_string()
    };
    daily_text(c, "daily_chal_status", &status, 34.0,
        if done { Color(170, 235, 190, 255) } else { Color(190, 185, 225, 220) }, DAILY_CARD_W);
    daily_text(c, "daily_play_text", if done { "PLAY AGAIN" } else { "PLAY" }, 48.0, Color(240, 232, 255, 255), DAILY_BTN.0);
    c.set_var("daily_countdown_secs", -1i32);
}

fn fmt_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    out
}

pub fn build_daily_reward_scene(ctx: &mut Context) -> Scene {
    let back_btn = crate::menu_look::page_back_btn(ctx, "daily_back_btn");
    let back_text = crate::menu_look::page_back_text(ctx, "daily_back_text");
    let gold = crate::menu_look::GLASS_GOLD;
    let lilac = crate::menu_look::GLASS_LILAC;
    let mut objs: Vec<(String, GameObject)> = vec![
        ("daily_reward_card".into(), crate::menu_look::glass_obj(ctx, "daily_reward_card", (DAILY_CARD_W, DAILY_REWARD_H), (DAILY_X, DAILY_REWARD_Y), gold, false, vec!["ui".into()])),
        ("daily_chal_card".into(), crate::menu_look::glass_obj(ctx, "daily_chal_card", (DAILY_CARD_W, DAILY_CHAL_H), (DAILY_X, DAILY_CHAL_Y), lilac, false, vec!["ui".into()])),
    ];
    let text = |ctx: &mut Context, id: &str, w: f32, x: f32, y: f32| -> (String, GameObject) {
        (id.to_string(), GameObject::build(id).size(w, 60.0).position(x, y).tag("ui").build(ctx))
    };
    objs.push(text(ctx, "daily_reward_head", DAILY_CARD_W, DAILY_X, DAILY_REWARD_Y + 40.0));
    for i in 0..7 {
        let x = daily_tile_x(i);
        let y = DAILY_REWARD_Y + 140.0;
        objs.push((format!("daily_tile_{i}"), crate::menu_look::glass_obj(ctx, &format!("daily_tile_{i}"), DAILY_TILE, (x, y), lilac, false, vec!["ui".into()])));
        objs.push(text(ctx, &format!("daily_tile_day_{i}"), DAILY_TILE.0, x, y + 34.0));
        objs.push(text(ctx, &format!("daily_tile_amt_{i}"), DAILY_TILE.0, x, y + 120.0));
        objs.push(text(ctx, &format!("daily_tile_meta_{i}"), DAILY_TILE.0, x, y + 240.0));
    }
    let claim_pos = ((VW - DAILY_BTN.0) * 0.5, DAILY_REWARD_Y + DAILY_REWARD_H - DAILY_BTN.1 - 120.0);
    objs.push(("daily_claim_btn".into(), crate::menu_look::glass_obj(ctx, "daily_claim_btn", DAILY_BTN, claim_pos, gold, true, vec!["ui".into(), "button".into()])));
    objs.push(text(ctx, "daily_claim_text", DAILY_BTN.0, claim_pos.0, claim_pos.1 + (DAILY_BTN.1 - 50.0) * 0.5));
    objs.push(text(ctx, "daily_countdown", DAILY_CARD_W, DAILY_X, claim_pos.1 + DAILY_BTN.1 + 26.0));
    objs.push(text(ctx, "daily_chal_head", DAILY_CARD_W, DAILY_X, DAILY_CHAL_Y + 40.0));
    objs.push(text(ctx, "daily_chal_name", DAILY_CARD_W, DAILY_X, DAILY_CHAL_Y + 120.0));
    objs.push(text(ctx, "daily_chal_blurb", DAILY_CARD_W, DAILY_X, DAILY_CHAL_Y + 245.0));
    objs.push(text(ctx, "daily_chal_goal", DAILY_CARD_W, DAILY_X, DAILY_CHAL_Y + 320.0));
    objs.push(text(ctx, "daily_chal_status", DAILY_CARD_W, DAILY_X, DAILY_CHAL_Y + 390.0));
    let play_pos = ((VW - DAILY_BTN.0) * 0.5, DAILY_CHAL_Y + DAILY_CHAL_H - DAILY_BTN.1 - 50.0);
    objs.push(("daily_play_btn".into(), crate::menu_look::glass_obj(ctx, "daily_play_btn", DAILY_BTN, play_pos, lilac, true, vec!["ui".into(), "button".into()])));
    objs.push(text(ctx, "daily_play_text", DAILY_BTN.0, play_pos.0, play_pos.1 + (DAILY_BTN.1 - 54.0) * 0.5));

    let mut scene = crate::menu_look::with_page_look(ctx, Scene::new("daily_reward"), "DAILY");
    for (id, o) in objs { scene = scene.with_object(&id, o); }
    let press = |scene: Scene, name: &str, target: &str| scene.with_event(
        GameEvent::MousePress {
            action: Action::Custom { name: name.into() },
            target: Target::name(target),
            button: Some(MouseButton::Left),
        },
        Target::name(target),
    );
    let scene = scene.with_object("daily_back_btn", back_btn).with_object("daily_back_text", back_text);
    let scene = press(scene, "daily_back", "daily_back_btn");
    let scene = press(scene, "daily_claim", "daily_claim_btn");
    let scene = press(scene, "daily_play", "daily_play_btn");
    scene.on_enter(|canvas| {
        let cam = Camera::new((VW, VH), (VW, VH));
        canvas.set_camera(cam);
        crate::menu_look::register_page_tick(canvas);
        set_page_back_label(canvas, "daily_back_text");
        daily_refresh(canvas);
        if !canvas.get_bool("daily_tick_registered") {
            canvas.set_var("daily_tick_registered", true);
            canvas.on_update(|c| {
                if !c.is_scene("daily_reward") { return; }
                // The countdown to tomorrow, redrawn when the minute changes.
                let left = crate::daily::secs_to_tomorrow();
                let key = (left / 60) as i32;
                if c.get_i32("daily_countdown_secs") != key {
                    c.set_var("daily_countdown_secs", key);
                    let (_, claimed) = crate::daily::reward_state();
                    let t = if claimed { format!("Next reward in {}", hms(left)) } else { format!("New challenge in {}", hms(left)) };
                    daily_text(c, "daily_countdown", &t, 30.0, Color(190, 185, 225, 220), DAILY_CARD_W);
                }
            });
        }
        canvas.register_custom_event("daily_back".into(), |c| crate::transition::go(c, "menu"));
        canvas.register_custom_event("daily_claim".into(), |c| {
            if let Some(meta) = crate::daily::claim_reward() {
                log::info!("daily reward: +{meta} META");
                if let Some(cam) = c.camera_mut() {
                    cam.flash_with(Color(255, 230, 150, 120), 0.35, FlashMode::Pulse, FlashEase::Sharp, 0.6, 0.0);
                }
                crate::achievements::check_achievements(c, 0, 0.0);
                daily_refresh(c);
            }
        });
        canvas.register_custom_event("daily_play".into(), |c| {
            if crate::transition::busy(c) { return; }
            c.set_var(crate::daily::DAILY_PENDING_VAR, true);
            crate::mode::set_mode(c, crate::mode::GameMode::Normal);
            c.set_var("boss_mode_active", false);
            c.set_var("game_intro_t", crate::constants::GAME_INTRO_TICKS);
            c.load_scene("game");
        });
        // Test hook: `DAILY_CLAIM=1` claims on entry, to film the page after.
        if std::env::var("DAILY_CLAIM").is_ok() {
            let _ = crate::daily::claim_reward();
            daily_refresh(canvas);
        }
    })
}

// ── Boss Order scene (testing) ───────────────────────────────────────────────
//
// Reconfigures which boss occupies which roster slot, so a fight can be reached
// without playing the roster in order.
//
// WHY IT LIVES ON THE MAIN MENU. `boss_kind_for_index` is consulted at ARENA
// ENTRY, once per fight, so an order set here applies from the next teleport
// onward — nothing about it is too late, and it does not need to be reachable
// mid-run. Against that, a modal opened during a fight is exactly the shape
// that produced the upgrade-node soft-lock (the soft-pause block returns before
// input handling), and that is not a risk worth taking for a test instrument.
//
// Left column: the roster, in shipped order, as pickable names.
// Right column: the slots, filled in the order picked.
// CLEAR empties the assignment, CONFIRM installs it, BACK leaves it untouched.

/// Slots picked so far, as indices into `BOSS_ROSTER`. Menu-local, and only
/// read into the real override by CONFIRM — so backing out changes nothing.
static BOSS_ORDER_PICKS: std::sync::OnceLock<Mutex<Vec<usize>>> = std::sync::OnceLock::new();

fn boss_order_picks() -> &'static Mutex<Vec<usize>> {
    BOSS_ORDER_PICKS.get_or_init(|| Mutex::new(Vec::new()))
}

const BOSS_ORDER_SLOTS: usize = BOSS_ROSTER.len();

// Row geometry for the pickable roster.
//
// ONE TEXT OBJECT PER BUTTON, not one block of spans. A single `Text` with
// seven spans lays them out at the FONT's line height, which has nothing to do
// with the button spacing — so the names stacked tightly at the top of the
// panel while the buttons ran down it. Nothing about that is fixable by
// resizing the buttons; the labels have to belong to them.
const BO_ROW_FONT: f32 = 46.0;
const BO_ROW_H: f32 = 116.0;
const BO_ROW_GAP: f32 = 12.0;
const BO_COL_W: f32 = 1380.0;
const BO_COL_X: f32 = 320.0;
const BO_COL_Y: f32 = 400.0;

/// Top of row `i`, and its height, in VIRTUAL units. Buttons and their labels
/// both go through this, so they cannot drift apart.
pub fn boss_order_row(i: usize) -> (f32, f32) {
    (BO_COL_Y + i as f32 * BO_ROW_H, BO_ROW_H - BO_ROW_GAP)
}

fn boss_order_refresh(canvas: &mut Canvas) {
    let Some(font) = ui_font() else { return; };
    let sc = canvas.virtual_scale();
    let picks = boss_order_picks().lock().unwrap().clone();

    // Left: the roster, one label per button. A name already assigned is dimmed
    // and shows which slot it went to.
    for (i, kind) in BOSS_ROSTER.iter().enumerate() {
        let slot = picks.iter().position(|p| *p == i);
        let (label, colour) = match slot {
            Some(n) => (format!("{}   ->  SLOT {}", kind.name(), n + 1), Color(125, 135, 158, 255)),
            None    => (kind.name().to_string(), Color(235, 240, 255, 255)),
        };
        if let Some(obj) = canvas.get_game_object_mut(&format!("bo_pick_text_{i}")) {
            obj.set_drawable(Box::new(ui_text_left_spec(
                &label, &font, BO_ROW_FONT * sc, colour, BO_COL_W * sc,
            )));
        }
    }

    // Right: the slots, in fight order.
    let mut slot_spans: Vec<Span> = Vec::new();
    for n in 0..BOSS_ORDER_SLOTS {
        let (label, colour) = match picks.get(n) {
            Some(&i) => (format!("{}.  {}\n", n + 1, BOSS_ROSTER[i].name()), Color(255, 220, 90, 255)),
            None => {
                // Unassigned slots are dealt at random each run, except the
                // last, which is always the final boss.
                let label = if n + 1 == BOSS_ORDER_SLOTS {
                    format!("{}.  ({})\n", n + 1, crate::constants::FINAL_BOSS.name())
                } else {
                    format!("{}.  (RANDOM)\n", n + 1)
                };
                (label, Color(110, 118, 138, 255))
            }
        };
        slot_spans.push(Span::new(label, BO_ROW_FONT * sc, Some(BO_ROW_H * sc),
            std::sync::Arc::new(font.clone()), colour, 0.0));
    }
    if let Some(obj) = canvas.get_game_object_mut("bo_slot_col") {
        obj.set_drawable(Box::new(Text::new(slot_spans, Some(1380.0 * sc), Align::Left, None)));
    }

    let status = if picks.is_empty() {
        if boss_order_is_overridden() {
            "An override is ACTIVE. Pick a new order, or CLEAR to restore the shipped one.".to_string()
        } else {
            "Shipped order. Click bosses in the order you want to fight them.".to_string()
        }
    } else {
        format!("{} of {} slots assigned — CONFIRM to apply.", picks.len(), BOSS_ORDER_SLOTS)
    };
    if let Some(obj) = canvas.get_game_object_mut("bo_status_text") {
        obj.set_drawable(Box::new(ui_text_spec(
            &status, &font, 34.0 * sc, Color(200, 210, 235, 255), 3000.0 * sc,
        )));
    }
}

pub fn build_boss_order_scene(ctx: &mut Context) -> Scene {
    let bg = GameObject::new_rect(ctx, "bo_bg".into(),
        Some(bright_background_2(VW + 800.0, VH)),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    let bg_tint = GameObject::new_rect(ctx, "bo_bg_tint".into(),
        Some(scene_tint(VW + 800.0, VH, Color(90, 40, 60, 170))),
        (VW + 800.0, VH), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);

    let button = |ctx: &mut Context, id: &str, x: f32, y: f32, w: u32, h: u32, tint: [u8; 3]| {
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 4 || px >= w - 4 || py < 4 || py >= h - 4;
            img.put_pixel(px, py, image::Rgba([tint[0], tint[1], tint[2], if border { 255 } else { 200 }]));
        }}
        GameObject::new_rect(ctx, id.into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (x, y), vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };

    // One invisible hit box per roster entry, laid over the roster text.
    let mut scene = Scene::new("boss_order")
        .with_object("bo_bg", bg)
        .with_object("bo_bg_tint", bg_tint);

    for i in 0..BOSS_ROSTER.len() {
        let id = format!("bo_pick_{i}");
        let (y, h) = boss_order_row(i);
        let mut hit = GameObject::new_rect(ctx, id.clone().into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (BO_COL_W, h), 0.0),
                image: solid(255, 255, 255, 22).into(), color: None,
            }),
            (BO_COL_W, h), (BO_COL_X, y), vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        hit.gravity = 0.0;
        // The button's own label, sitting ON it. Inset and nudged down so the
        // text sits inside the box rather than on its top edge.
        let text = GameObject::build(&format!("bo_pick_text_{i}"))
            .size(BO_COL_W - 60.0, h)
            .position(BO_COL_X + 30.0, y + h * 0.22)
            .tag("ui")
            .build(ctx);
        scene = scene.with_object(&format!("bo_pick_text_{i}"), text);
        scene = scene.with_object(&id, hit).with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: format!("bo_pick_{i}") },
                target: Target::name(&id),
                button: Some(MouseButton::Left),
            },
            Target::name(&id),
        );
    }

    let slot_col = GameObject::build("bo_slot_col")
        .size(1380.0, 900.0).position(2100.0, BO_COL_Y).tag("ui").build(ctx);
    let title = GameObject::build("bo_title_text")
        .size(2400.0, 200.0).position(VW * 0.5 - 1200.0, VH * 0.04).tag("ui").build(ctx);
    let status = GameObject::build("bo_status_text")
        .size(3000.0, 120.0).position(VW * 0.5 - 1500.0, 250.0).tag("ui").build(ctx);
    let roster_hdr = GameObject::build("bo_roster_hdr")
        .size(1380.0, 90.0).position(320.0, 310.0).tag("ui").build(ctx);
    let slot_hdr = GameObject::build("bo_slot_hdr")
        .size(1380.0, 90.0).position(2100.0, 310.0).tag("ui").build(ctx);

    let clear_btn   = button(ctx, "bo_clear_btn",   560.0, 1810.0, 640, 150, [90, 60, 60]);
    let confirm_btn = button(ctx, "bo_confirm_btn", 1600.0, 1810.0, 640, 150, [50, 100, 70]);
    let back_btn    = button(ctx, "bo_back_btn",    2640.0, 1810.0, 640, 150, [70, 70, 90]);
    let clear_text   = GameObject::build("bo_clear_text").size(640.0, 150.0).position(560.0, 1810.0).tag("ui").build(ctx);
    let confirm_text = GameObject::build("bo_confirm_text").size(640.0, 150.0).position(1600.0, 1810.0).tag("ui").build(ctx);
    let back_text    = GameObject::build("bo_back_text").size(640.0, 150.0).position(2640.0, 1810.0).tag("ui").build(ctx);

    scene
        .with_object("bo_roster_hdr", roster_hdr)
        .with_object("bo_slot_hdr", slot_hdr)
        .with_object("bo_slot_col", slot_col)
        .with_object("bo_title_text", title)
        .with_object("bo_status_text", status)
        .with_object("bo_clear_btn", clear_btn)
        .with_object("bo_confirm_btn", confirm_btn)
        .with_object("bo_back_btn", back_btn)
        .with_object("bo_clear_text", clear_text)
        .with_object("bo_confirm_text", confirm_text)
        .with_object("bo_back_text", back_text)
        .with_event(GameEvent::MousePress {
            action: Action::Custom { name: "bo_clear".into() },
            target: Target::name("bo_clear_btn"), button: Some(MouseButton::Left),
        }, Target::name("bo_clear_btn"))
        .with_event(GameEvent::MousePress {
            action: Action::Custom { name: "bo_confirm".into() },
            target: Target::name("bo_confirm_btn"), button: Some(MouseButton::Left),
        }, Target::name("bo_confirm_btn"))
        .with_event(GameEvent::MousePress {
            action: Action::Custom { name: "bo_back".into() },
            target: Target::name("bo_back_btn"), button: Some(MouseButton::Left),
        }, Target::name("bo_back_btn"))
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            // Start from whatever is installed, so reopening shows the live
            // order rather than a blank slate that implies nothing is set.
            {
                let mut picks = boss_order_picks().lock().unwrap();
                picks.clear();
                if let Some(order) = boss_order_override() {
                    for kind in order {
                        if let Some(i) = BOSS_ROSTER.iter().position(|k| *k == kind) {
                            picks.push(i);
                        }
                    }
                }
            }

            if let Some(font) = ui_font() {
                let sc = canvas.virtual_scale();
                let label = |canvas: &mut Canvas, id: &str, text: &str, size: f32, colour: Color, w: f32| {
                    if let Some(obj) = canvas.get_game_object_mut(id) {
                        obj.set_drawable(Box::new(ui_text_spec(text, &font, size * sc, colour, w * sc)));
                    }
                };
                label(canvas, "bo_title_text", "BOSS ORDER  (TESTING)", 64.0, Color(255, 210, 120, 255), 2400.0);
                label(canvas, "bo_clear_text", "CLEAR", 44.0, Color(255, 200, 200, 255), 640.0);
                label(canvas, "bo_confirm_text", "CONFIRM", 44.0, Color(200, 255, 210, 255), 640.0);
                label(canvas, "bo_back_text", "BACK", 44.0, Color(225, 225, 235, 255), 640.0);
                if let Some(obj) = canvas.get_game_object_mut("bo_roster_hdr") {
                    obj.set_drawable(Box::new(ui_text_left_spec(
                        "PICK A BOSS", &font, 38.0 * sc, Color(160, 200, 255, 255), 1380.0 * sc)));
                }
                if let Some(obj) = canvas.get_game_object_mut("bo_slot_hdr") {
                    obj.set_drawable(Box::new(ui_text_left_spec(
                        "FIGHT ORDER", &font, 38.0 * sc, Color(160, 200, 255, 255), 1380.0 * sc)));
                }
            }

            boss_order_refresh(canvas);

            for i in 0..BOSS_ROSTER.len() {
                canvas.register_custom_event(format!("bo_pick_{i}"), move |c| {
                    {
                        let mut picks = boss_order_picks().lock().unwrap();
                        // Clicking an assigned boss un-assigns it, so a mistake
                        // is undone by clicking the same name rather than by
                        // clearing the whole list.
                        if let Some(at) = picks.iter().position(|p| *p == i) {
                            picks.remove(at);
                        } else if picks.len() < BOSS_ORDER_SLOTS {
                            picks.push(i);
                        }
                    }
                    boss_order_refresh(c);
                });
            }
            canvas.register_custom_event("bo_clear".into(), |c| {
                boss_order_picks().lock().unwrap().clear();
                set_boss_order_override(None);
                boss_order_refresh(c);
            });
            canvas.register_custom_event("bo_confirm".into(), |c| {
                let picks = boss_order_picks().lock().unwrap().clone();
                if picks.is_empty() {
                    set_boss_order_override(None);
                } else {
                    set_boss_order_override(Some(
                        picks.iter().map(|&i| BOSS_ROSTER[i]).collect(),
                    ));
                }
                c.load_scene("menu");
            });
            canvas.register_custom_event("bo_back".into(), |c| crate::transition::go(c, "menu"));
        })
}
