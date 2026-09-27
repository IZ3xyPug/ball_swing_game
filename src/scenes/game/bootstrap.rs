use quartz::*;
use image::{AnimationDecoder, ImageDecoder};
use std::sync::OnceLock;

use crate::achievements::*;

/// Decode a GIF from bytes, tint each frame with a brownish-red colour shift,
/// and return it as an `AnimatedSprite` using `from_frames`.
fn tint_asteroid_gif_brownish_red(bytes: &'static [u8], size: (f32, f32), fps: f32) -> Option<AnimatedSprite> {
    use std::io::Cursor;
    let cursor  = Cursor::new(bytes);
    let decoder = image::codecs::gif::GifDecoder::new(cursor).ok()?;
    let frames: Vec<image::RgbaImage> = decoder.into_frames()
        .filter_map(|f| f.ok())
        .map(|f| {
            let mut img = f.into_buffer();
            for px in img.pixels_mut() {
                if px[3] == 0 { continue; }
                let r = (px[0] as f32 * 0.85 + 28.0).min(255.0) as u8;
                let g = (px[1] as f32 * 0.35).max(0.0) as u8;
                let b = (px[2] as f32 * 0.12).max(0.0) as u8;
                px[0] = r; px[1] = g; px[2] = b;
            }
            img
        })
        .collect();
    if frames.is_empty() { return None; }
    Some(AnimatedSprite::from_frames(frames, size, fps))
}

/// Returns the brownish-red asteroid animation template.
/// The GIF is decoded and tinted once on first call; subsequent calls clone the cached result.
pub fn hook_asteroid_anim_for_spawn() -> Option<AnimatedSprite> {
    static CACHED: OnceLock<Option<AnimatedSprite>> = OnceLock::new();
    CACHED.get_or_init(|| {
        tint_asteroid_gif_brownish_red(
            include_bytes!("../../../assets/asteroid.gif"),
            (crate::constants::SPACE_ASTEROID_SIZE_MIN, crate::constants::SPACE_ASTEROID_SIZE_MIN),
            8.0,
        )
    }).clone()
}

use crate::constants::*;
use crate::hud::*;
use crate::images::*;
use crate::objects::*;
use super::space_zone::blackhole1_template;

const LAYER_SOLAR_CEILING: i32 = 12;
const LAYER_SPACE_BLACKHOLE: i32 = 18;
const LAYER_SPACE_PLANET: i32 = 19;
const LAYER_SPACE_ASTEROID: i32 = 20;
const LAYER_SPACE_HOOK: i32 = 21;
const LAYER_SPACE_COIN: i32 = 22;
const LAYER_SPACE_RED_COIN: i32 = 23;
pub(crate) const LAYER_ROPE: i32 = 20;
pub(crate) const LAYER_PLAYER: i32 = 42;
const LAYER_AIRSHIELD: i32 = 43;
const LAYER_ENERGY_HOOK_REF: i32 = 150;

/// A serpent rift is a hole in the BACKGROUND. Everything in the arena draws
/// over it — the body, the rope, the player — which is what makes them pass
/// THROUGH the portal rather than behind it.
///
/// Named rather than written inline because the ordering is the whole point and
/// a comment did not protect it: this was 28, in front of the body at
/// `LAYER_SPACE_HOOK`, under a comment claiming it was behind it.
pub(crate) const LAYER_SERPENT_RIFT: i32 = LAYER_SPACE_HOOK - 2;
pub(crate) const LAYER_SERPENT_BODY: i32 = LAYER_SPACE_HOOK;

/// All pools and starter hook names created during scene construction.
pub struct PoolSets {
    pub starter_names: Vec<String>,
    pub pool_free:     Vec<String>,
    pub pad_free:      Vec<String>,
    pub spinner_free:  Vec<String>,
    pub coin_free:     Vec<String>,
    pub flip_free:     Vec<String>,
    pub score_x2_free: Vec<String>,
    pub zero_g_free:   Vec<String>,
    pub gate_free:     Vec<String>,
    pub gwell_free:    Vec<String>,
    pub turret_free:   Vec<String>,
    pub bullet_free:   Vec<String>,
    pub coin_static_sprite:  Image,
    pub coin_anim_template:  Option<AnimatedSprite>,
    #[allow(dead_code)]
    pub score_x2_anim_template: Option<AnimatedSprite>,
    pub tech_bounce_static_img: Image,
    pub tech_bounce_static_img_flipped: Image,
    pub tech_bounce_anim_frames: Vec<Image>,
    pub tech_bounce_anim_frames_flipped: Vec<Image>,
    pub pad_thruster_static_img: Image,
    pub pad_thruster_anim_template: Option<AnimatedSprite>,
    pub pad_thruster_anim_template_flipped: Option<AnimatedSprite>,
    // ── Space zone pools
    pub rocket_pad_free:   Vec<String>,
    pub space_planet_free: Vec<String>,
    pub space_hook_free:   Vec<String>,
    pub space_coin_free:   Vec<String>,
    pub space_blue_coin_free: Vec<String>,
    pub space_bh_free:     Vec<String>,
    pub space_asteroid_free: Vec<String>,
    pub space_red_coin_free: Vec<String>,
    pub space_oxygen_pickup_free: Vec<String>,
    pub upgrade_free: Vec<String>,
    // ── Gravity cannon pool
    pub cannon_free:       Vec<String>,
    // ── Boss fight
    pub boss_bolt_free: Vec<String>,
    pub impact_free: Vec<String>,
    pub boss_asteroid_ids: Vec<String>,
    // ── Comets
    pub comet_free: Vec<String>,
    // ── Comet warnings
    pub warn_free: Vec<String>,
}

fn decode_tech_bounce_frames_stretched() -> Vec<Image> {
    let bytes = include_bytes!("../../../assets/techbouncernew.gif");
    let cursor = std::io::Cursor::new(bytes.as_slice());
    let Ok(decoder) = image::codecs::gif::GifDecoder::new(cursor) else {
        return vec![load_image_sized(ASSET_TECH_BOUNCE_GIF, PAD_W, PAD_H)];
    };

    let (gif_w, gif_h) = decoder.dimensions();
    let mut composed = image::RgbaImage::from_pixel(gif_w.max(1), gif_h.max(1), image::Rgba([0, 0, 0, 0]));
    let Ok(frames) = decoder.into_frames().collect_frames() else {
        return vec![load_image_sized(ASSET_TECH_BOUNCE_GIF, PAD_W, PAD_H)];
    };

    let out_w = PAD_W.max(1.0).round() as u32;
    let out_h = PAD_H.max(1.0).round() as u32;
    let mut composed_frames: Vec<image::RgbaImage> = Vec::with_capacity(frames.len());

    for frame in frames {
        let left = frame.left();
        let top = frame.top();
        let patch = frame.into_buffer();
        image::imageops::overlay(&mut composed, &patch, left as i64, top as i64);
        composed_frames.push(composed.clone());
    }

    let mut min_x = gif_w;
    let mut min_y = gif_h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    for frame in &composed_frames {
        for y in 0..frame.height() {
            for x in 0..frame.width() {
                if frame.get_pixel(x, y).0[3] > 0 {
                    found = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
    }

    let crop_bounds = if found {
        let pad = 2u32;
        let crop_x = min_x.saturating_sub(pad);
        let crop_y = min_y.saturating_sub(pad);
        let crop_right = (max_x + pad).min(gif_w.saturating_sub(1));
        let crop_bottom = (max_y + pad).min(gif_h.saturating_sub(1));
        (
            crop_x,
            crop_y,
            crop_right.saturating_sub(crop_x).saturating_add(1),
            crop_bottom.saturating_sub(crop_y).saturating_add(1),
        )
    } else {
        (0, 0, gif_w.max(1), gif_h.max(1))
    };

    // Preserve gameplay footprint while keeping art scale proportional
    // to the source frame occupancy so the visible pad does not overfill PAD_W/PAD_H.
    let draw_w = ((out_w as f32) * (crop_bounds.2 as f32 / gif_w.max(1) as f32))
        .round()
        .clamp(1.0, out_w as f32) as u32;
    // Keep approximately the same total width while restoring vertical proportion
    // from the cropped source aspect ratio so the pad is not overly squashed.
    let draw_h = ((draw_w as f32) * (crop_bounds.3 as f32 / crop_bounds.2.max(1) as f32))
        .round()
        .clamp(1.0, out_h as f32) as u32;
    let offset_x = (out_w.saturating_sub(draw_w) / 2) as i64;
    // Top-anchor so pad collisions begin at the first visible top pixels.
    let offset_y = 0i64;

    let mut out: Vec<Image> = Vec::with_capacity(composed_frames.len());
    for frame in composed_frames {
        let cropped = image::imageops::crop_imm(
            &frame,
            crop_bounds.0,
            crop_bounds.1,
            crop_bounds.2,
            crop_bounds.3,
        )
        .to_image();
        let scaled = image::imageops::resize(
            &cropped,
            draw_w,
            draw_h,
            image::imageops::FilterType::Nearest,
        );
        let mut framed = image::RgbaImage::from_pixel(out_w, out_h, image::Rgba([0, 0, 0, 0]));
        image::imageops::overlay(&mut framed, &scaled, offset_x, offset_y);
        out.push(Image { shape: ShapeType::Rectangle(0.0, (PAD_W, PAD_H), 0.0), image: framed.into(), color: None });
    }

    if out.is_empty() {
        vec![load_image_sized(ASSET_TECH_BOUNCE_GIF, PAD_W, PAD_H)]
    } else {
        out
    }
}

/// Build a pool of space-coin `GameObject`s, returning the free-list and a
/// vec of (id, object) pairs ready to be folded into the scene.
fn build_space_coin_pool(
    ctx: &mut Context,
    prefix: &str,
    pool_size: usize,
    spawn_xy: f32,
    asset_bytes: &'static [u8],
    coin_r: f32,
    layer: i32,
    coin_tag: &'static str,
) -> (Vec<String>, Vec<(String, GameObject)>) {
    let d = coin_r * 2.0;
    let static_img = load_image_sized(asset_bytes, d, d);
    let anim_template = AnimatedSprite::new(asset_bytes, (d, d), SPACE_COIN_ANIM_FPS).ok();
    let mut free = Vec::with_capacity(pool_size);
    let mut objects = Vec::with_capacity(pool_size);
    for i in 0..pool_size {
        let id = format!("{prefix}_{i}");
        let mut obj = make_coin(ctx, &id, spawn_xy, spawn_xy);
        obj.set_image(static_img.clone());
        if let Some(anim) = &anim_template {
            obj.set_animation(anim.clone());
            if let Some(a) = obj.animated_sprite.as_mut() { a.set_frame(0); }
        }
        obj.tags.retain(|t| t != "space_catcoin" && t != "space_catcoin_blue" && t != "space_catcoin_red");
        obj.tags.push(coin_tag.to_string());
        obj.visible = false;
        obj.layer = layer;
        free.push(id.clone());
        objects.push((id, obj));
    }
    (free, objects)
}

/// Hidden, ignore-zoom HUD rect at layer 100.  Pass `None` for objects whose image is set later.
fn hud_obj(ctx: &mut Context, id: &str, w: f32, h: f32, x: f32, y: f32, img: Option<Image>) -> GameObject {
    let mut obj = GameObject::new_rect(ctx, id.into(), img, (w, h), (x, y),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    obj.ignore_zoom = true;
    obj.layer = 100;
    obj.visible = false;
    obj
}

/// Rectangle-shaped `Image` from an `RgbaImage`.
fn rect_img(w: f32, h: f32, data: image::RgbaImage) -> Image {
    Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: data.into(), color: None }
}

/// Build a simple object pool: call `make` for each id, set invisible, add to scene.
fn simple_pool(ctx: &mut Context, scene: Scene, prefix: &str, size: usize,
    make: impl Fn(&mut Context, &str) -> GameObject) -> (Scene, Vec<String>) {
    let mut free = Vec::with_capacity(size);
    let mut scene = scene;
    for i in 0..size {
        let id = format!("{prefix}_{i}");
        let mut obj = make(ctx, &id);
        obj.visible = false;
        free.push(id.clone());
        scene = scene.with_object(id, obj);
    }
    (scene, free)
}

pub fn build_scene_objects(ctx: &mut Context) -> (Scene, PoolSets) {
    // ── Background images ────────────────────────────────────────────────
    let bg_texture_w = VW as u32;
    let bg_texture_h = VH as u32;
    let bg_zone_start = image::load_from_memory(ASSET_AURORA_EARTH_GIF)
        .map(|img| {
            image::imageops::resize(
                &img.to_rgba8(),
                bg_texture_w,
                bg_texture_h,
                image::imageops::FilterType::Triangle,
            )
        })
        .unwrap_or_else(|_| gradient_rect(bg_texture_w, bg_texture_h, C_SKY_TOP, C_SKY_BOT));

    let mut bg = GameObject::new_rect(ctx, "bg".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: bg_zone_start.clone().into(), color: None }),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    bg.ignore_zoom = true;

    let mut bg_space = GameObject::new_rect(ctx, "bg_space".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: bg_zone_start.clone().into(), color: None }),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    bg_space.ignore_zoom = true;
    bg_space.visible = false;

    let mut bg_stars_b = GameObject::new_rect(ctx, "bg_stars_b".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: solid(0, 0, 0, 0).into(), color: None }),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    bg_stars_b.ignore_zoom = true;
    bg_stars_b.visible = false;

    // ── Energy hook reference display (top-right corner) ─────────────────
    const ASTEROID_W: f32 = 480.0;
    const ASTEROID_H: f32 = 480.0;
    let mut asteroid = GameObject::new_rect(ctx, "asteroid".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (ASTEROID_W, ASTEROID_H), 0.0), image: solid(0, 0, 0, 0).into(), color: None }),
        (ASTEROID_W, ASTEROID_H), (VW - ASTEROID_W - 80.0, 80.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    if let Ok(anim) = AnimatedSprite::new(
        include_bytes!("../../../assets/energy_hook_1.gif"),
        (ASTEROID_W, ASTEROID_H),
        8.0,
    ) {
        asteroid.set_image(anim.get_current_image());
        asteroid.set_animation(anim);
    }
    asteroid.ignore_zoom = true;
    asteroid.layer = LAYER_ENERGY_HOOK_REF;
    asteroid.visible = false;

    // ── Player — engine-native gravity ───────────────────────────────────
    let mut player = GameObject::new_rect(ctx, "player".into(),
        Some(Image { shape: ShapeType::Ellipse(0.0, (PLAYER_R*2.0, PLAYER_R*2.0), 0.0), image: circle_cached(PLAYER_R as u32, C_PLAYER.0, C_PLAYER.1, C_PLAYER.2), color: None }),
        (PLAYER_R*2.0, PLAYER_R*2.0), (SPAWN_X - PLAYER_R, SPAWN_Y - PLAYER_R),
        vec!["player".into()], (18.0, 0.0), (1.0, 1.0), 0.0);
    // Opt into gravity well forces.
    player.gravity_all_sources = true;
    player.gravity_falloff = GravityFalloff::InverseSquare;
    // Collide with asteroids as a solid circle body.
    player.collision_mode  = CollisionMode::solid_circle(PLAYER_R);
    player.collision_layer = PLAYER_COLLISION_LAYER;
    player.collision_mask  = ASTEROID_COLLISION_LAYER;
    player.layer = LAYER_PLAYER;

    // Calicoball animated sprite — replaces the procedural circle.
    // fps=0 freezes auto-advance; tick_player_ball_animation drives frames manually.
    if let Ok(mut calico) = AnimatedSprite::new(
        include_bytes!("../../../assets/calicoball.gif"),
        (PLAYER_R * 2.0, PLAYER_R * 2.0),
        CALICO_FPS,
    ) {
        calico.set_fps(0.0);
        player.set_animation(calico);
    }

    // Velocity-facing air shield. The gif is mirrored on X once at load time,
    // then rotated each post-physics tick based on player net velocity.
    let mut airshield = GameObject::new_rect(ctx, "airshield".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (AIRSHIELD_W, AIRSHIELD_H), 0.0), image: solid(0, 0, 0, 0).into(), color: None }),
        (AIRSHIELD_W, AIRSHIELD_H), (SPAWN_X - AIRSHIELD_W * 0.5, SPAWN_Y - AIRSHIELD_H * 0.5),
        vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    if let Ok(mut anim) = AnimatedSprite::new(
        include_bytes!("../../../assets/airshield2.gif"),
        (AIRSHIELD_W, AIRSHIELD_H),
        AIRSHIELD_ANIM_FPS,
    ) {
        anim.set_mirrored(true);
        airshield.set_image(anim.get_current_image());
        airshield.set_animation(anim);
    }
    airshield.visible = false;
    airshield.layer = LAYER_AIRSHIELD;

    let rope_beam_h = ROPE_THICKNESS;
    let mut rope = GameObject::new_rect(ctx, "rope".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (4.0, rope_beam_h), 0.0), image: solid(C_ROPE.0, C_ROPE.1, C_ROPE.2, 255).into(), color: None }),
        (4.0, rope_beam_h), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    rope.visible = false;
    rope.layer = LAYER_ROPE;

    // No "danger floor" bar. A red strip pinned to the bottom of the screen
    // marked where falling used to kill; nothing ever read it (the fall is
    // tested against the player's own position in `State`), and it drew as
    // a flat red line across every frame. Removed 2026-09-27.

    // ── HUD elements ─────────────────────────────────────────────────────
    let mut dist_bar = GameObject::new_rect(ctx, "dist_bar".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (920.0, 48.0), 0.0), image: bar_img(920, 48, 0.0, 80, 220, 160).into(), color: None }),
        (920.0, 48.0), (VW * 0.5 - 460.0, 30.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    dist_bar.ignore_zoom = true;
    dist_bar.layer = 100;

    let coin_counter      = hud_obj(ctx, "coin_counter",      640.0, 168.0, 26.0,       24.0,  Some(rect_img(640.0, 168.0, coin_counter_img(0))));
    let score_counter     = hud_obj(ctx, "score_counter",     420.0,  98.0, VW - 450.0, 40.0,  Some(rect_img(420.0,  98.0, score_counter_img(0))));
    let hearts_total_w    = HEART_W * MAX_HEARTS as f32 + HEART_GAP * (MAX_HEARTS as f32 - 1.0);
    let hearts_hud        = hud_obj(ctx, "hearts_hud", hearts_total_w, HEART_H, HEART_HUD_X, HEART_HUD_Y,
        Some(rect_img(hearts_total_w, HEART_H, hearts_img(MAX_HEARTS as u32, MAX_HEARTS as u32))));
    let momentum_counter  = hud_obj(ctx, "momentum_counter",  420.0,  86.0, 30.0,       150.0, Some(rect_img(420.0,  86.0, momentum_counter_img(0.0))));
    let gravity_indicator = hud_obj(ctx, "gravity_indicator", 308.0,  84.0, 30.0,       248.0, Some(rect_img(308.0,  84.0, gravity_indicator_img(false, true))));
    let y_meter           = hud_obj(ctx, "y_meter",           420.0,  86.0, 30.0,       344.0, Some(rect_img(420.0,  86.0, y_counter_img(SPAWN_Y))));
    let x_meter           = hud_obj(ctx, "x_meter",           420.0,  86.0, 30.0,       442.0, Some(rect_img(420.0,  86.0, x_counter_img(SPAWN_X))));

    let mut combo_flash = {
        let (w, h) = (420u32, 80u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            img.put_pixel(px, py, image::Rgba([255, 200, 60, 230]));
        }}
        GameObject::new_rect(ctx, "combo_flash".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, VH*0.08),
            vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    combo_flash.visible = false;
    combo_flash.ignore_zoom = true;
    combo_flash.layer = 100;

    let mut pause_overlay = {
        const PO_OVERSCAN: f32 = 400.0;
        let po_w = VW + PO_OVERSCAN * 2.0;
        let obj = GameObject::new_rect(ctx, "pause_overlay".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (po_w, VH), 0.0), image: pause_overlay_img().into(), color: None }),
            (po_w, VH), (-PO_OVERSCAN, 0.0),
            vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj
    };
    pause_overlay.visible = false;
    pause_overlay.layer = 10_000;
    pause_overlay.ignore_zoom = true;

    // The in-game pause control (Android only; hidden on desktop, which has P).
    //
    // Two faint bars in a rounded box, at low alpha. Deliberately quiet: a
    // bright PAUSE button in the corner of every frame is read past a thousand
    // times to be used once, and it was the atmosphere cost that got the last
    // one removed. This one has to be findable, not noticeable.
    let pause_touch_btn = {
        let (bx, by, bw, bh) = PAUSE_TOUCH_BTN;
        let (w, h) = (bw as u32, bh as u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            img.put_pixel(px, py, image::Rgba([180, 210, 245, 46]));
        }}
        // Two vertical bars.
        let bar_w = w / 7;
        let (y0, y1) = (h / 4, h - h / 4);
        for py in y0..y1 {
            for px in 0..bar_w {
                img.put_pixel(w / 3 - bar_w / 2 + px, py, image::Rgba([225, 240, 255, 190]));
                img.put_pixel(2 * w / 3 - bar_w / 2 + px, py, image::Rgba([225, 240, 255, 190]));
            }
        }
        let mut obj = GameObject::new_rect(ctx, "pause_touch_btn".into(),
            Some(Image {
                shape: ShapeType::RoundedRectangle(0.0, (bw, bh), 0.0, bh * 0.22),
                image: img.into(), color: None,
            }),
            (bw, bh), (bx, by), vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.visible = false;
        obj.layer = 9_500; // under the pause menu, over the world
        obj.ignore_zoom = true;
        obj.gravity = 0.0;
        obj
    };

    let flip_timer_hud     = hud_obj(ctx, "flip_timer",     504.0, 118.0, VW * 0.5 - 252.0, 560.0,
        Some(rect_img(504.0, 118.0, flip_timer_img(FLIP_DURATION, FLIP_DURATION))));
    let zero_g_timer_hud   = hud_obj(ctx, "zero_g_timer",   504.0, 118.0, VW * 0.5 - 252.0, 690.0,
        Some(rect_img(504.0, 118.0, flip_timer_img(ZERO_G_DURATION, ZERO_G_DURATION))));
    let score_x2_timer_hud = hud_obj(ctx, "score_x2_timer", 504.0, 118.0, VW * 0.5 - 252.0, 820.0,
        Some(rect_img(504.0, 118.0, score_x2_timer_img(SCORE_X2_DURATION, SCORE_X2_DURATION))));

    let toast_w = GOLD_MASTER_TOAST_WIDTH;
    let toast_h = GOLD_MASTER_TOAST_HEIGHT;
    let mut achievement_toast_panel = {
        let (w, h) = (toast_w as u32, toast_h as u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            let border = px < 4 || px >= w - 4 || py < 4 || py >= h - 4;
            img.put_pixel(px, py, image::Rgba([24, 30, 44, if border { 240 } else { 210 }]));
        }}
        GameObject::new_rect(ctx, GOLD_MASTER_TOAST_PANEL_NAME.into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW * 0.5 - w as f32 * 0.5, -(h as f32) - 32.0),
            vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    achievement_toast_panel.visible = false;
    achievement_toast_panel.ignore_zoom = true;
    achievement_toast_panel.layer = 150;

    let mut achievement_toast_title = GameObject::build(GOLD_MASTER_TOAST_TITLE_NAME)
        .size(1080.0, 54.0)
        .position(VW * 0.5 - 520.0, -toast_h - 10.0)
        .tag("hud")
        .build(ctx);
    achievement_toast_title.visible = false;
    achievement_toast_title.ignore_zoom = true;
    achievement_toast_title.layer = 151;

    let mut achievement_toast_desc = GameObject::build(GOLD_MASTER_TOAST_DESC_NAME)
        .size(1080.0, 42.0)
        .position(VW * 0.5 - 520.0, -toast_h + 46.0)
        .tag("hud")
        .build(ctx);
    achievement_toast_desc.visible = false;
    achievement_toast_desc.ignore_zoom = true;
    achievement_toast_desc.layer = 151;

    let mut achievement_toast_check = GameObject::build(GOLD_MASTER_TOAST_CHECK_NAME)
        .size(120.0, 88.0)
        .position(VW * 0.5 + 470.0, -toast_h + 34.0)
        .tag("hud")
        .build(ctx);
    achievement_toast_check.visible = false;
    achievement_toast_check.ignore_zoom = true;
    achievement_toast_check.layer = 151;

    let mut coin_magnet_radius = {
        let d = (COIN_MAGNET_RADIUS * 2.0).round().max(2.0) as u32;
        let mut img = image::RgbaImage::new(d, d);
        let r = COIN_MAGNET_RADIUS;
        let ctr = r;
        for py in 0..d {
            for px in 0..d {
                let dx = px as f32 + 0.5 - ctr;
                let dy = py as f32 + 0.5 - ctr;
                let dist = (dx * dx + dy * dy).sqrt();
                if (dist - r).abs() <= 2.0 {
                    img.put_pixel(px, py, image::Rgba([255, 245, 140, 200]));
                } else {
                    img.put_pixel(px, py, image::Rgba([0, 0, 0, 0]));
                }
            }
        }
        GameObject::new_rect(ctx, "coin_magnet_radius".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (d as f32, d as f32), 0.0), image: img.into(), color: None }),
            (d as f32, d as f32), (SPAWN_X - COIN_MAGNET_RADIUS, SPAWN_Y - COIN_MAGNET_RADIUS),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    coin_magnet_radius.visible = false;

    // ── Fullscreen effect overlays (shown/hidden by tick_hud) ────────────
    // zero_g_overlay: kept invisible — its gif is shown via the ability icon HUD.
    let mut zero_g_overlay = GameObject::new_rect(ctx, "zero_g_overlay".into(),
        None::<Image>, (256.0, 256.0), (VW * 0.5 - 128.0, VH * 0.5 - 128.0),
        vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    zero_g_overlay.ignore_zoom = true;
    zero_g_overlay.visible = false;
    zero_g_overlay.layer = 50;

    // ── Solar flare presentation ─────────────────────────────────────────
    // Full-screen wash for the telegraph and the flare itself. Sits under the
    // pause overlay but over gameplay, and is re-tinted every frame by
    // `solar::draw_flare_overlay` as the telegraph ramps.
    let mut flare_overlay = GameObject::new_rect(ctx, "flare_overlay".into(),
        None::<Image>, (VW, VH), (0.0, 0.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    flare_overlay.ignore_zoom = true;
    flare_overlay.visible = false;
    flare_overlay.layer = 60;

    // Instruction banner. The flare has to be readable without audio and
    // without prior knowledge, so the telegraph says what to do, not just that
    // something is happening.
    let mut flare_banner = GameObject::new_rect(ctx, "flare_banner".into(),
        None::<Image>, (1800.0, 90.0), (VW * 0.5 - 900.0, VH * 0.16),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    flare_banner.ignore_zoom = true;
    flare_banner.visible = false;
    flare_banner.layer = 61;

    // Eclipse warning banner, above the flare banner so both can be up.
    let mut eclipse_banner = GameObject::new_rect(ctx, "eclipse_banner".into(),
        None::<Image>, (1900.0, 96.0), (VW * 0.5 - 950.0, VH * 0.09),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    eclipse_banner.ignore_zoom = true;
    eclipse_banner.visible = false;
    eclipse_banner.layer = 62;

    // Animated catcoingold icon overlaid on the coin counter slot.
    // coin_counter is at (26, 24), icon slot is at (12, 28) within it → abs (38, 52).
    let mut coin_icon_anim = GameObject::new_rect(ctx, "coin_icon_anim".into(),
        None::<Image>, (112.0, 112.0), (38.0, 52.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    coin_icon_anim.ignore_zoom = true;
    coin_icon_anim.layer = 101;
    coin_icon_anim.visible = false;

    // ── Ability icons (shown to the left of the scoreboard) ──────────────
    // All ignore_zoom=true so they sit in screen-space like the rest of the HUD.
    const ICON_W: f32 = 120.0;
    const ICON_H: f32 = 120.0;
    const ICON_Y: f32 = 30.0;
    // Stack icons rightmost-first, each 130px apart, 20px left of the score_counter.
    // score_counter is at (VW - 450.0, 40.0), so first icon right edge ≈ VW - 470.
    const ICON_X0: f32 = VW - 450.0 - ICON_W - 30.0; // flip (outermost)
    const ICON_X1: f32 = ICON_X0 - ICON_W - 10.0;    // zero_g
    const ICON_X2: f32 = ICON_X1 - ICON_W - 10.0;    // score_x2

    let flip_icon = hud_obj(ctx, "flip_icon", ICON_W, ICON_H, ICON_X0, ICON_Y,
        Some(Image { shape: ShapeType::Rectangle(0.0, (ICON_W, ICON_H), 0.0), image: flip_image_cached(), color: None }));
    let zero_g_icon   = hud_obj(ctx, "zero_g_icon",   ICON_W, ICON_H, ICON_X1, ICON_Y, None);
    let score_x2_icon = hud_obj(ctx, "score_x2_icon", ICON_W, ICON_H, ICON_X2, ICON_Y, None);

    // ── Starter hooks ────────────────────────────────────────────────────
    let starter_hooks = crate::level_gen::starter_hooks();

    let mut scene = Scene::new("game")
        .with_object("bg",           bg)
        .with_object("bg_space",     bg_space)
        .with_object("bg_stars_b",   bg_stars_b)
        .with_object("asteroid",     asteroid)
        .with_object("rope",         rope)
        .with_object("player",       player)
        .with_object("airshield",    airshield)
        .with_object("dist_bar",     dist_bar)
        .with_object("coin_counter", coin_counter)
        .with_object("score_counter", score_counter)
        .with_object("hearts_hud",    hearts_hud)
        .with_object("momentum_counter", momentum_counter)
        .with_object("gravity_indicator", gravity_indicator)
        .with_object("y_meter", y_meter)
        .with_object("x_meter", x_meter)
        .with_object("combo_flash",  combo_flash)
        .with_object("flip_timer", flip_timer_hud)
        .with_object("zero_g_timer", zero_g_timer_hud)
        .with_object("score_x2_timer", score_x2_timer_hud)
        .with_object(GOLD_MASTER_TOAST_PANEL_NAME, achievement_toast_panel)
        .with_object(GOLD_MASTER_TOAST_TITLE_NAME, achievement_toast_title)
        .with_object(GOLD_MASTER_TOAST_DESC_NAME, achievement_toast_desc)
        .with_object(GOLD_MASTER_TOAST_CHECK_NAME, achievement_toast_check)
        .with_object("coin_magnet_radius", coin_magnet_radius)
        .with_object("zero_g_overlay",    zero_g_overlay)
        .with_object("flare_overlay",     flare_overlay)
        .with_object("flare_banner",      flare_banner)
        .with_object("eclipse_banner",    eclipse_banner)
        .with_object("coin_icon_anim",    coin_icon_anim)
        .with_object("flip_icon",         flip_icon)
        .with_object("zero_g_icon",       zero_g_icon)
        .with_object("score_x2_icon",     score_x2_icon);

    // ── Asteroid animation template (shared by hook pool and asteroid pool) ───
    // Decode once here; hook pool and space_asteroid pool both clone from this.
    let hook_asteroid_anim = tint_asteroid_gif_brownish_red(
        include_bytes!("../../../assets/asteroid.gif"),
        (SPACE_ASTEROID_SIZE_MIN, SPACE_ASTEROID_SIZE_MIN),
        8.0,
    );

    // ── Hook pool ────────────────────────────────────────────────────────
    let mut starter_names: Vec<String> = Vec::new();
    let mut pool_free: Vec<String> = Vec::new();
    for i in 0..HOOK_POOL_SIZE {
        let id = format!("hook_{i}");
        // Vary size across the asteroid range so hooks have different scales.
        let size = SPACE_ASTEROID_SIZE_MIN + (i as f32 * 73.0) % (SPACE_ASTEROID_SIZE_MAX - SPACE_ASTEROID_SIZE_MIN);
        // Gentle deterministic drift — small enough not to leave the play area fast.
        let dv_x = ((i as f32 * 0.7 + 0.3) % 1.0 - 0.5) * 0.35;
        let dv_y = ((i as f32 * 1.3 + 0.2) % 1.0 - 0.5) * 0.18;
        let (init_x, init_y) = if i < starter_hooks.len() {
            let (hx, hy) = starter_hooks[i];
            (hx - size * 0.5, hy - size * 0.5)
        } else {
            (-2000.0, -2000.0)
        };
        let mut obj = GameObject::new_rect(ctx, id.clone(), None::<Image>,
            (size, size), (init_x, init_y),
            vec!["hook".into()], (dv_x, dv_y), (1.0, 1.0), 0.0);
        // Only set animation on starter hooks (initially visible).
        // Pool hooks are invisible; enabling their animation would tick
        // all 60+ GIF sprites every frame even off-screen, causing lag.
        if i < starter_hooks.len() {
            if let Some(anim) = &hook_asteroid_anim {
                obj.set_animation(anim.clone());
            }
        }
        obj.gravity = 0.0;
        obj.rotation_momentum = ((i as f32 * 0.9 + 0.1) % 1.0 - 0.5) * 0.008;
        obj.layer = LAYER_SPACE_HOOK;
        if i < starter_hooks.len() {
            starter_names.push(id.clone());
        } else {
            obj.visible = false;
            pool_free.push(id.clone());
        }
        scene = scene.with_object(id, obj);
    }

    // ── Pad pool ─────────────────────────────────────────────────────────
    // Keep pad image + rounded corner geometry in sync so highlights and
    // silhouette edges match the rendered bounce-pad art.
    let tech_bounce_anim_frames = decode_tech_bounce_frames_stretched();
    let tech_bounce_static_img = tech_bounce_anim_frames
        .first()
        .cloned()
        .unwrap_or_else(|| load_image_sized(ASSET_TECH_BOUNCE_GIF, PAD_W, PAD_H));
    let tech_bounce_anim_frames_flipped: Vec<Image> = tech_bounce_anim_frames.iter()
        .map(|img| flip_vertical(img.clone()))
        .collect();
    let tech_bounce_static_img_flipped = tech_bounce_anim_frames_flipped
        .first()
        .cloned()
        .unwrap_or_else(|| flip_vertical(tech_bounce_static_img.clone()));
    let pad_thruster_anim_template = AnimatedSprite::new(
        include_bytes!("../../../assets/thruster1.gif"),
        (PAD_THRUSTER_W, PAD_THRUSTER_H),
        PAD_THRUSTER_FPS,
    ).ok();
    let pad_thruster_anim_template_flipped: Option<AnimatedSprite> = pad_thruster_anim_template
        .as_ref()
        .map(|a| { let mut f = a.clone(); f.flip_vertical_frames(); f });
    let pad_thruster_static_img = pad_thruster_anim_template
        .as_ref()
        .map(|a| a.get_current_image())
        .unwrap_or_else(|| load_image_sized(include_bytes!("../../../assets/thruster1.gif"), PAD_THRUSTER_W, PAD_THRUSTER_H));
    let mut pad_free: Vec<String> = Vec::new();
    for i in 0..PAD_POOL_SIZE {
        let id = format!("pad_{i}");
        let mut obj = make_pad(ctx, &id, -3000.0, -3000.0);
        obj.set_image(tech_bounce_static_img.clone());
        obj.layer = 5;
        obj.visible = false;
        pad_free.push(id.clone());
        scene = scene.with_object(id, obj);

        let thr_id = format!("pad_{i}_thruster");
        let mut thr = GameObject::new_rect(ctx, thr_id.clone(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (PAD_THRUSTER_W, PAD_THRUSTER_H), 0.0), image: pad_thruster_static_img.image.clone(), color: None }),
            (PAD_THRUSTER_W, PAD_THRUSTER_H), (-3000.0, -3000.0),
            vec!["pad_thruster".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        thr.layer = 4;
        thr.visible = false;
        scene = scene.with_object(thr_id, thr);
    }

    // ── Spinner pool ─────────────────────────────────────────────────────
    let (mut scene, spinner_free) = simple_pool(ctx, scene, "spinner", SPINNER_POOL_SIZE,
        |c, id| make_spinner(c, id, -3500.0, -3500.0));

    // ── Coin pool ────────────────────────────────────────────────────────
    let coin_static_sprite = load_image_sized(ASSET_COIN_GIF, COIN_R * 2.0, COIN_R * 2.0);
    let coin_anim_template = AnimatedSprite::new(
        include_bytes!("../../../assets/catcoingold.gif"),
        (COIN_R * 2.0, COIN_R * 2.0),
        6.0,
    ).ok();
    let score_x2_anim_template = AnimatedSprite::new(
        include_bytes!("../../../assets/2x.gif"),
        (SCORE_X2_W, SCORE_X2_H),
        12.0,
    ).ok();

    let mut coin_free: Vec<String> = Vec::new();
    for i in 0..COIN_POOL_SIZE {
        let id = format!("coin_{i}");
        let mut obj = make_coin(ctx, &id, -3700.0, -3700.0);
        obj.set_image(coin_static_sprite.clone());
        obj.visible = false;
        coin_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Flip pool ────────────────────────────────────────────────────────
    let (mut scene, flip_free) = simple_pool(ctx, scene, "flip", FLIP_POOL_SIZE,
        |c, id| make_flip(c, id, -3800.0, -3800.0));

    // ── Score x2 pool ────────────────────────────────────────────────────
    let score_x2_sprite = load_image_sized(ASSET_SCORE_X2_GIF, SCORE_X2_W, SCORE_X2_H);
    let mut score_x2_free: Vec<String> = Vec::new();
    for i in 0..SCORE_X2_POOL_SIZE {
        let id = format!("score_x2_{i}");
        let mut obj = make_score_x2(ctx, &id, -3850.0, -3850.0);
        obj.set_image(score_x2_sprite.clone());
        if let Some(anim) = &score_x2_anim_template {
            obj.set_animation(anim.clone());
        }
        obj.visible = false;
        score_x2_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Zero-g pool ──────────────────────────────────────────────────────
    let zero_g_anim_template = AnimatedSprite::new(
        include_bytes!("../../../assets/ZeroG.gif"),
        (ZERO_G_W, ZERO_G_H),
        8.0,
    ).ok();
    let mut zero_g_free: Vec<String> = Vec::new();
    for i in 0..ZERO_G_POOL_SIZE {
        let id = format!("zero_g_{i}");
        let mut obj = make_zero_g(ctx, &id, -3875.0, -3875.0);
        if let Some(anim) = &zero_g_anim_template {
            obj.set_animation(anim.clone());
        }
        obj.visible = false;
        zero_g_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Gate pool ────────────────────────────────────────────────────────
    let mut gate_free: Vec<String> = Vec::new();
    for i in 0..GATE_POOL_SIZE {
        let gid = format!("gate_{i}");
        let top_id = format!("{gid}_top");
        let bot_id = format!("{gid}_bot");

        let mut top_obj = make_gate_segment(ctx, &top_id, -3900.0, -3900.0, GATE_TOP_SEG_H, gate_top_image_cached());
        top_obj.visible = false;
        scene = scene.with_object(top_id, top_obj);

        let mut bot_obj = make_gate_segment(ctx, &bot_id, -3900.0, -3900.0, GATE_BOT_SEG_H, gate_bot_image_cached());
        bot_obj.visible = false;
        scene = scene.with_object(bot_id, bot_obj);

        gate_free.push(gid);
    }

    // ── Gravity well pool ────────────────────────────────────────────────
    let mut gwell_free: Vec<String> = Vec::new();
    for i in 0..GWELL_POOL_SIZE {
        let id = format!("gwell_{i}");
        let default_visual_r = PLAYER_R * GWELL_VISUAL_SCALE_MIN;
        let mut obj = make_gravity_well(ctx, &id, -4000.0, -4000.0, GWELL_RADIUS_MIN, GWELL_STRENGTH_MIN, default_visual_r);
        obj.visible = false;
        gwell_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Turret pool ──────────────────────────────────────────────────────
    let (scene, turret_free) = simple_pool(ctx, scene, "turret", TURRET_POOL_SIZE,
        |c, id| make_turret(c, id, -4500.0, -4500.0));

    // ── Bullet pool ──────────────────────────────────────────────────────
    let (scene, bullet_free) = simple_pool(ctx, scene, "bullet", BULLET_POOL_SIZE,
        |c, id| make_turret_bullet(c, id));

    // ── Rocket pad pool ───────────────────────────────────────────────────
    let (scene, rocket_pad_free) = simple_pool(ctx, scene, "rocket_pad", ROCKET_PAD_POOL_SIZE,
        |c, id| make_rocket_pad(c, id, -5200.0, -5200.0));

    // ── Gravity cannon pool ───────────────────────────────────────────────
    let (mut scene, cannon_free) = simple_pool(ctx, scene, "cannon", CANNON_POOL_SIZE,
        |c, id| make_gravity_cannon(c, id, -6000.0, -6000.0));

    // ── Space planet pool ─────────────────────────────────────────────────
    let mut space_planet_free: Vec<String> = Vec::new();
    for i in 0..SPACE_PLANET_POOL_SIZE {
        let id = format!("space_planet_{i}");
        let mut obj = make_planet(ctx, &id, -5500.0, -5500.0,
            SPACE_PLANET_RADIUS_SM_MIN, SPACE_PLANET_RADIUS_SM_MIN * SPACE_PLANET_GRAV_R_MULT, 0);
        obj.visible = false;
        obj.layer = LAYER_SPACE_PLANET;
        obj.planet_radius = None;
        space_planet_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Space hook pool ───────────────────────────────────────────────────
    // Animations are NOT set here to avoid ticking 160 invisible GIF sprites every frame.
    // call `hook_asteroid_anim_for_spawn()` in space_zone.rs when a hook becomes visible.
    let mut space_hook_free: Vec<String> = Vec::new();
    for i in 0..SPACE_HOOK_POOL_SIZE {
        let id = format!("space_hook_{i}");
        let size = SPACE_ASTEROID_SIZE_MIN + (i as f32 * 59.0) % (SPACE_ASTEROID_SIZE_MAX - SPACE_ASTEROID_SIZE_MIN);
        let mut obj = GameObject::new_rect(ctx, id.clone(), None::<Image>,
            (size, size), (-5700.0, -5700.0),
            vec!["hook".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.gravity = 0.0;
        obj.rotation_momentum = ((i as f32 * 1.1 + 0.4) % 1.0 - 0.5) * 0.008;
        obj.visible = false;
        obj.layer = LAYER_SPACE_HOOK;
        space_hook_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Space coin pool ───────────────────────────────────────────────────
    let (space_coin_free, sc_objs) = build_space_coin_pool(
        ctx, "space_coin", SPACE_COIN_POOL_SIZE, -5900.0,
        include_bytes!("../../../assets/catcoin.gif"),
        SPACE_COIN_R, LAYER_SPACE_COIN, "space_catcoin",
    );
    for (id, obj) in sc_objs { scene = scene.with_object(id, obj); }

    // ── Space blue-coin pool ─────────────────────────────────────────────
    let (space_blue_coin_free, sbc_objs) = build_space_coin_pool(
        ctx, "space_blue_coin", SPACE_BLUE_COIN_POOL_SIZE, -6450.0,
        include_bytes!("../../../assets/catcoinblue.gif"),
        SPACE_RED_COIN_R, LAYER_SPACE_RED_COIN, "space_catcoin_blue",
    );
    for (id, obj) in sbc_objs { scene = scene.with_object(id, obj); }

    // ── Space black hole pool ─────────────────────────────────────────────
    let mut space_bh_free: Vec<String> = Vec::new();
    for i in 0..SPACE_BLACKHOLE_POOL_SIZE {
        let id = format!("space_bh_{i}");
        let mut obj = make_black_hole(ctx, &id, -6100.0, -6100.0, SPACE_BLACKHOLE_RADIUS_MIN);
        obj.visible = false;
        obj.layer = LAYER_SPACE_BLACKHOLE;
        obj.planet_radius = None;
        space_bh_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Main-world decorative asteroid pool ──────────────────────────────
    let asteroid_space_img: image::RgbaImage =
        image::load_from_memory(include_bytes!("../../../assets/asteroid.gif"))
            .or_else(|_| image::load_from_memory(ASSET_ASTEROID))
        .map(|img| img.into_rgba8())
        .unwrap_or_else(|_| solid(120, 120, 132, 255));
    let asteroid_anim_template = tint_asteroid_gif_brownish_red(
        include_bytes!("../../../assets/asteroid.gif"),
        (SPACE_ASTEROID_SIZE_MIN, SPACE_ASTEROID_SIZE_MIN),
        8.0,
    );
    let mut space_asteroid_free: Vec<String> = Vec::new();
    for i in 0..SPACE_ASTEROID_POOL_SIZE {
        let id = format!("space_asteroid_{i}");
        let mut obj = GameObject::new_rect(ctx, id.clone(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (SPACE_ASTEROID_SIZE_MIN, SPACE_ASTEROID_SIZE_MIN), 0.0), image: asteroid_space_img.clone().into(), color: None }),
            (SPACE_ASTEROID_SIZE_MIN, SPACE_ASTEROID_SIZE_MIN), (-6300.0, -6300.0),
            vec!["hook".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        if let Some(anim) = &asteroid_anim_template {
            obj.set_animation(anim.clone());
        }
        obj.collision_mode  = CollisionMode::solid_circle(SPACE_ASTEROID_SIZE_MIN * 0.5);
        obj.collision_layer = ASTEROID_COLLISION_LAYER;
        obj.collision_mask  = ASTEROID_COLLISION_LAYER | PLAYER_COLLISION_LAYER;
        obj.visible = false;
        obj.layer = LAYER_SPACE_ASTEROID;
        space_asteroid_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Space red-coin pool ───────────────────────────────────────────────
    let (space_red_coin_free, src_objs) = build_space_coin_pool(
        ctx, "space_red_coin", SPACE_RED_COIN_POOL_SIZE, -6500.0,
        include_bytes!("../../../assets/catcoingold.gif"),
        SPACE_RED_COIN_R, LAYER_SPACE_RED_COIN, "space_catcoin_red",
    );
    for (id, obj) in src_objs { scene = scene.with_object(id, obj); }

    // ── Space oxygen canister pool ────────────────────────────────────────
    let mut space_oxygen_pickup_free: Vec<String> = Vec::new();
    {
        let d = SPACE_OXYGEN_PICKUP_R * 2.0;
        let img = oxygen_canister_img();
        for i in 0..SPACE_OXYGEN_PICKUP_POOL_SIZE {
            let id = format!("space_oxygen_pickup_{i}");
            let mut obj = GameObject::new_rect(ctx, id.clone(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0), image: img.clone(), color: None }),
                (d, d), (-6300.0, -6300.0),
                vec!["space_oxygen_pickup".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.gravity = 0.0;
            obj.visible = false;
            obj.layer = LAYER_SPACE_ASTEROID;
            space_oxygen_pickup_free.push(id.clone());
            scene = scene.with_object(id, obj);
        }
    }

    // ── Roguelike upgrade node pool ────────────────────────────────────────
    let mut upgrade_free: Vec<String> = Vec::new();
    {
        let d = UPGRADE_R * 2.0;
        let img = ring_outline_img(UPGRADE_R as u32, C_UPGRADE.0, C_UPGRADE.1, C_UPGRADE.2);
        for i in 0..UPGRADE_POOL_SIZE {
            let id = format!("upgrade_node_{i}");
            let mut obj = GameObject::new_rect(ctx, id.clone(),
                Some(Image { shape: ShapeType::Ellipse(0.0, (d, d), 0.0), image: img.clone().into(), color: None }),
                (d, d), (-6000.0, -6000.0),
                vec!["upgrade_node".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.gravity = 0.0;
            obj.visible = false;
            obj.layer = 40;
            obj.set_glow(GlowConfig { color: Color(C_UPGRADE.0, C_UPGRADE.1, C_UPGRADE.2, 200), width: 18.0 });
            upgrade_free.push(id.clone());
            scene = scene.with_object(id, obj);
        }
    }

    // ── Solar ceiling ─────────────────────────────────────────────────────
    // Placeholder object only — AnimatedSprite is decoded lazily on first
    // enter_space() to avoid a multi-second freeze at game startup.
    {
        let mut solar_ceiling = GameObject::new_rect(ctx, "solar_ceiling".into(), None::<Image>,
            (VW, SPACE_SOLAR_H), (0.0, -SPACE_SOLAR_H), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        solar_ceiling.visible     = false;
        solar_ceiling.layer       = LAYER_SOLAR_CEILING; // behind gameplay objects; still above background
        solar_ceiling.ignore_zoom = true; // screen-space: slides in from top as player approaches
        scene = scene.with_object("solar_ceiling", solar_ceiling);
    }

    // ── Space HUD objects ─────────────────────────────────────────────────
    // Oxygen bar (replaces dist_bar while in space)
    let mut oxygen_bar_obj = GameObject::new_rect(ctx, "oxygen_bar".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (OXYGEN_BAR_W, OXYGEN_BAR_H), 0.0), image: oxygen_bar_img(1.0, OXYGEN_BAR_W as u32, OXYGEN_BAR_H as u32).into(), color: None }),
        (OXYGEN_BAR_W, OXYGEN_BAR_H), (VW * 0.5 - OXYGEN_BAR_W * 0.5, 30.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    oxygen_bar_obj.visible = false;
    oxygen_bar_obj.ignore_zoom = true;
    oxygen_bar_obj.layer = 100;
    scene = scene.with_object("oxygen_bar", oxygen_bar_obj);

    // Welcome text
    let mut space_welcome_text = GameObject::build("space_welcome_text")
        .size(1100.0, 140.0)
        .position((VW - 1100.0) * 0.5, VH * 0.32)
        .tag("hud")
        .build(ctx);
    space_welcome_text.visible = false;
    space_welcome_text.ignore_zoom = true;
    space_welcome_text.layer = 200;
    scene = scene.with_object("space_welcome_text", space_welcome_text);

    // Pause overlay last so it renders above everything.
    scene = scene.with_object("pause_overlay", pause_overlay)
        .with_object("pause_touch_btn", pause_touch_btn);

    // ── Pause menu buttons (above overlay) ───────────────────────────────
    let pause_btn_w: f32 = 700.0;
    let pause_btn_h: f32 = 170.0;
    let pause_btn_x: f32 = (VW - pause_btn_w) / 2.0;
    let pause_title_w: f32 = 650.0;
    let pause_title_h: f32 = 100.0;

    let mut pause_title = GameObject::new_rect(ctx, "pause_title".into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (pause_title_w, pause_title_h), 0.0), image: pause_title_img().into(), color: None }),
        (pause_title_w, pause_title_h), ((VW - pause_title_w) / 2.0, VH * 0.20),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
    pause_title.visible = false;
    pause_title.layer = 10_001;
    pause_title.ignore_zoom = true;

    let make_pause_btn = |ctx: &mut Context, name: &str, r: u8, g: u8, b: u8, label: &str, y: f32| {
        let img = pause_btn_img(pause_btn_w as u32, pause_btn_h as u32, r, g, b, label);
        let corner_r = (pause_btn_h * 0.48 * 1.33).clamp(1.0, pause_btn_h * 0.5 - 1.0);
        let mut obj = GameObject::new_rect(ctx, name.to_string().into(),
            Some(Image { shape: ShapeType::RoundedRectangle(0.0, (pause_btn_w, pause_btn_h), 0.0, corner_r), image: img.into(), color: None }),
            (pause_btn_w, pause_btn_h), (pause_btn_x, y),
            vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.visible = false;
        obj.layer = 10_001;
        obj.ignore_zoom = true;
        obj
    };

    let pause_resume_btn = make_pause_btn(ctx, "pause_resume_btn", 50, 160, 90, "RESUME", 780.0);
    let pause_restart_btn = make_pause_btn(ctx, "pause_restart_btn", 60, 120, 200, "RESTART", 1000.0);
    let pause_settings_btn = make_pause_btn(ctx, "pause_settings_btn", 80, 80, 100, "SETTINGS", 1220.0);
    let pause_menu_btn = make_pause_btn(ctx, "pause_menu_btn", 170, 65, 65, "MENU", 1440.0);

    // No on-screen SWING or PAUSE buttons. Swinging is the whole right half of
    // the screen and pausing is a swipe — see the touch-control block in
    // `constants.rs`. A permanent button in the corner of a space game is read
    // past constantly and pressed once.

    let mut start_prompt_text = GameObject::build("start_prompt_text")
        .size(1300.0, 120.0)
        .position((VW - 1300.0) * 0.5, VH * 0.50)
        .tag("hud")
        .build(ctx);
    start_prompt_text.visible = false;
    start_prompt_text.layer = 10_002;
    start_prompt_text.ignore_zoom = true;

    // Cannon fast-travel prompt (shown while the player is held and can afford
    // the hyper-transit; press F to accept, otherwise the default launch fires).
    let mut cannon_prompt_text = GameObject::build("cannon_prompt_text")
        .size(1500.0, 120.0)
        .position((VW - 1500.0) * 0.5, VH * 0.16)
        .tag("hud")
        .build(ctx);
    cannon_prompt_text.visible = false;
    cannon_prompt_text.layer = 10_002;
    cannon_prompt_text.ignore_zoom = true;

    // ── Roguelike upgrade choice dialogue (HUD) ─────────────────────────
    // Shown while the player is held at an upgrade node; options are set by
    // upgrades.rs::update_dialogue_text and selected with keys 1-5 / Esc.
    // Panel is tall enough for the title + meta + up to 6 option lines.
    let dlg_panel = {
        let (w, h) = (1100u32, 780u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h {
            for px in 0..w {
                let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
                img.put_pixel(px, py, image::Rgba([16, 26, 48, if border { 235 } else { 205 }]));
            }
        }
        let mut obj = GameObject::new_rect(ctx, "upgrade_dialogue_panel".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW * 0.5 - w as f32 * 0.5, VH * 0.5 - h as f32 * 0.5),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.visible = false;
        obj.ignore_zoom = true;
        obj.layer = 10_001;
        obj
    };
    let mut dlg_title = GameObject::build("upgrade_dialogue_title")
        .size(1000.0, 80.0)
        .position(VW * 0.5 - 500.0, VH * 0.5 - 320.0)
        .tag("hud")
        .build(ctx);
    dlg_title.visible = false;
    dlg_title.ignore_zoom = true;
    dlg_title.layer = 10_001;
    let mut dlg_meta = GameObject::build("upgrade_dialogue_meta")
        .size(1000.0, 70.0)
        .position(VW * 0.5 - 500.0, VH * 0.5 - 255.0)
        .tag("hud")
        .build(ctx);
    dlg_meta.visible = false;
    dlg_meta.ignore_zoom = true;
    dlg_meta.layer = 10_001;
    let mut dlg_opts = Vec::new();
    for i in 0..6 {
        let y = VH * 0.5 - 187.0 + i as f32 * 84.0;
        let mut o = GameObject::build(format!("upgrade_opt_{i}"))
            .size(1000.0, 72.0)
            .position(VW * 0.5 - 500.0, y)
            .tag("hud")
            .build(ctx);
        o.visible = false;
        o.ignore_zoom = true;
        o.layer = 10_001;
        dlg_opts.push(o);
    }

    // Three independent label objects — one above each slider track — so
    // vertical positioning is exact rather than relying on \n spacing.
    // Positioned at SLIDER_Y[i] - 100 so the label sits just above its track.
    let make_settings_label = |ctx: &mut Context, id: &str, y: f32| {
        let mut obj = GameObject::build(id)
            .size(1400.0, 80.0)
            .position((VW - 1400.0) * 0.5, y)
            .tag("hud")
            .build(ctx);
        obj.visible = false;
        obj.layer = 10_004;
        obj.ignore_zoom = true;
        obj
    };
    let settings_label_0 = make_settings_label(ctx, "settings_label_0", 720.0);
    let settings_label_1 = make_settings_label(ctx, "settings_label_1", 1020.0);
    let settings_label_2 = make_settings_label(ctx, "settings_label_2", 1320.0);

    let settings_back_btn = make_pause_btn(ctx, "settings_back_btn", 80, 80, 100, "BACK", 1660.0);

    // ── Settings volume sliders ───────────────────────────────────────────
    // Tracks are thin solid-color rectangles; thumbs are small rounded rects.
    // Both are ignore_zoom so they sit in virtual-screen space like the pause buttons.
    const SLIDER_TRACK_W: f32 = 1400.0;
    const SLIDER_TRACK_H: f32 = 24.0;
    const SLIDER_THUMB_W: f32 = 60.0;
    const SLIDER_THUMB_H: f32 = 80.0;
    const SLIDER_TRACK_X: f32 = (VW - SLIDER_TRACK_W) / 2.0;
    // Track Y positions (one per volume: master, music, sound)
    const SLIDER_Y: [f32; 3] = [820.0, 1120.0, 1420.0];

    let make_slider_track = |ctx: &mut Context, id: &str, y: f32| {
        let mut obj = GameObject::new_rect(ctx, id.into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (SLIDER_TRACK_W, SLIDER_TRACK_H), 0.0), image: solid(60, 62, 88, 220).into(), color: None }),
            (SLIDER_TRACK_W, SLIDER_TRACK_H), (SLIDER_TRACK_X, y),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.visible = false;
        obj.layer = 10_003;
        obj.ignore_zoom = true;
        obj
    };

    let make_slider_thumb = |ctx: &mut Context, id: &str, y: f32| {
        let mut obj = GameObject::new_rect(ctx, id.into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (SLIDER_THUMB_W, SLIDER_THUMB_H), 0.0), image: solid(210, 220, 255, 255).into(), color: None }),
            (SLIDER_THUMB_W, SLIDER_THUMB_H), (SLIDER_TRACK_X, y - (SLIDER_THUMB_H - SLIDER_TRACK_H) / 2.0),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.visible = false;
        obj.layer = 10_005;
        obj.ignore_zoom = true;
        obj
    };

    let slider_master_track = make_slider_track(ctx, "slider_master_track", SLIDER_Y[0]);
    let slider_master_thumb = make_slider_thumb(ctx, "slider_master_thumb", SLIDER_Y[0]);
    let slider_music_track  = make_slider_track(ctx, "slider_music_track",  SLIDER_Y[1]);
    let slider_music_thumb  = make_slider_thumb(ctx, "slider_music_thumb",  SLIDER_Y[1]);
    let slider_sound_track  = make_slider_track(ctx, "slider_sound_track",  SLIDER_Y[2]);
    let slider_sound_thumb  = make_slider_thumb(ctx, "slider_sound_thumb",  SLIDER_Y[2]);

    scene = scene
        .with_object("pause_title", pause_title)
        .with_object("pause_resume_btn", pause_resume_btn)
        .with_object("pause_restart_btn", pause_restart_btn)
        .with_object("pause_settings_btn", pause_settings_btn)
        .with_object("pause_menu_btn", pause_menu_btn)
        .with_object("start_prompt_text", start_prompt_text)
        .with_object("cannon_prompt_text", cannon_prompt_text)
        .with_object("upgrade_dialogue_panel", dlg_panel)
        .with_object("upgrade_dialogue_title", dlg_title)
        .with_object("upgrade_dialogue_meta", dlg_meta)
        .with_object("settings_label_0", settings_label_0)
        .with_object("settings_label_1", settings_label_1)
        .with_object("settings_label_2", settings_label_2)
        .with_object("settings_back_btn", settings_back_btn)
        .with_object("slider_master_track", slider_master_track)
        .with_object("slider_master_thumb", slider_master_thumb)
        .with_object("slider_music_track",  slider_music_track)
        .with_object("slider_music_thumb",  slider_music_thumb)
        .with_object("slider_sound_track",  slider_sound_track)
        .with_object("slider_sound_thumb",  slider_sound_thumb);

    // Register the upgrade-dialogue option text objects.
    for (i, opt) in dlg_opts.into_iter().enumerate() {
        scene = scene.with_object(format!("upgrade_opt_{i}"), opt);
    }

    // ── Boss body ─────────────────────────────────────────────────────────
    {
        // The Sun Devourer's own size — see `DEVOURER_BODY_SIZE`. This object
        // is only used by that fight; the other bosses build their own parts.
        let s = DEVOURER_BODY_SIZE;
        let body = crate::scenes::game::helpers::pl_image_cached(ASSET_PL_DEVOURER_BODY, s)
            .unwrap_or_else(|| std::sync::Arc::new(
                solid(C_BOSS_BODY.0, C_BOSS_BODY.1, C_BOSS_BODY.2, 255)));
        let mut boss_obj = GameObject::new_rect(ctx, "boss".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (s, s), 0.0), image: body, color: None }),
            (s, s), (-6000.0, -6000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        boss_obj.layer = LAYER_SPACE_HOOK;
        boss_obj.gravity = 0.0;
        boss_obj.visible = false;
        scene = scene.with_object("boss", boss_obj);
    }

    // ── Multi-part boss parts (Colossus / Serpent) ───────────────────────
    // Each multi-part boss has its OWN visual set so they never reuse parts.
    // Circle visuals, positioned / revealed / tinted by the boss tick at the
    // part offsets. Hidden so single-body bosses never show them.
    {
        // Colossus: 4 bodies (two hands, torso, head), each assembled from
        // primitives into a single composite silhouette so they read as a hand,
        // a torso and a head — not a slightly-large circle.
        const COLOSSUS_COLORS: [(u8, u8, u8); 4] = [
            (80, 100, 230),   // hand_l
            (80, 140, 245),   // hand_r
            (190, 75, 60),    // torso
            (150, 80, 205),   // head
        ];
        for (i, col) in COLOSSUS_COLORS.iter().enumerate() {
            let name = format!("colossus_part_{i}");
            let s = colossus_part_size(i as u32);
            let su = s.round().max(2.0) as u32;
            // Generated art where it decodes, the drawn composite otherwise.
            let pl = match i {
                0 | 1 => ASSET_PL_COLOSSUS_HAND,
                2     => ASSET_PL_COLOSSUS_TORSO,
                _     => ASSET_PL_COLOSSUS_HEAD,
            };
            // Part 1 is the LEFT hand and gets a mirrored copy of the sprite.
            // Without this the Colossus has two right hands, which is the kind
            // of thing nobody can un-see once they have noticed it.
            let img = crate::scenes::game::helpers::pl_image_cached_mirrored(pl, s, i == 1)
                .unwrap_or_else(|| std::sync::Arc::new(match i {
                    0 | 1 => crate::images::colossus_hand(su, *col),
                    2     => crate::images::colossus_torso(su, *col),
                    _     => crate::images::colossus_head(su, *col),
                }));
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (s, s), 0.0), image: img.into(), color: None }),
                (s, s), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK;
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object(&name, obj);
        }
        // Colossus propulsion plumes: one per part, the energy jetting from
        // its openings that holds it up. Transparent strips; the THRUST PLUME
        // effect draws them; colossus.rs places them on the parts each frame.
        // Behind the parts, so the jet emerges from INSIDE the opening.
        for i in 0..4 {
            let name = format!("colossus_plume_{i}");
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (100.0, 50.0), 0.0),
                             image: std::sync::Arc::new(solid(0, 0, 0, 0)).into(), color: None }),
                (100.0, 50.0), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK - 1;
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object(&name, obj);
        }
        // ── Serpent ──────────────────────────────────────────────────
        // One object per PART: segments, then the tail, then the head, matching
        // `boss_parts_for_kind`. The image is swapped per frame in the fight
        // (the seam animates), so the placeholder here only has to exist.
        //
        // Every piece is TETHERABLE — the "hook" tag is what makes this boss
        // double as traversal, which is the mechanic the whole fight is built
        // around: you ride the thing you are dismantling.
        for i in 0..(SERPENT_SEGMENTS + 2) {
            let name = format!("serpent_part_{i}");
            let d = SERPENT_SEGMENT_SIZE;
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image {
                    shape: ShapeType::Rectangle(0.0, (d, d), 0.0),
                    image: crate::images::serpent_segment_cached(d as u32, 0).into(),
                    color: None,
                }),
                (d, d), (-9000.0, -9000.0),
                vec!["boss".into(), "hook".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK;
            obj.gravity = 0.0;
            obj.collision_mode = CollisionMode::NonPlatform;
            obj.visible = false;
            scene = scene.with_object(&name, obj);
        }
        // Rift markers: shared by the rift-strike sequence and the wormhole
        // gambit, since only one of the two is ever running.
        {
            for i in 0..SERPENT_RIFT_SLOTS {
                let name = format!("serpent_rift_{i}");
                let d = SERPENT_RIFT_R.max(SERPENT_GAMBIT_HOLE_R) * 2.0;
                let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                    Some(Image {
                        shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                        image: crate::images::gravity_well_img(
                            (d * 0.5) as u32, 120, 255, 190).into(),
                        color: None,
                    }),
                    (d, d), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                obj.layer = LAYER_SERPENT_RIFT;
                obj.gravity = 0.0;
                obj.visible = false;
                scene = scene.with_object(&name, obj);
            }
        }
        // The exposed energy spine, drawn between head and tail once the body
        // between them is gone.
        {
            let name = "serpent_spine";
            let mut obj = GameObject::new_rect(ctx, name.into(),
                Some(Image {
                    shape: ShapeType::Rectangle(0.0, (100.0, SERPENT_LASH_THICKNESS), 0.0),
                    image: crate::images::solid(255, 200, 90, 190).into(), color: None,
                }),
                (100.0, SERPENT_LASH_THICKNESS), (-9000.0, -9000.0),
                vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = 29;
            obj.gravity = 0.0;
            obj.visible = false;
            obj.set_glow(GlowConfig { color: Color(255, 210, 110, 200), width: 34.0 });
            scene = scene.with_object(name, obj);
        }
        // ── Sun Devourer lunge lane ──────────────────────────────────
        // Transparent strip; the STRIKE LANE effect draws it (tick_desperation).
        {
            let mut obj = GameObject::new_rect(ctx, "devourer_lunge_lane".into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (100.0, 100.0), 0.0),
                             image: std::sync::Arc::new(solid(0, 0, 0, 0)).into(), color: None }),
                (100.0, 100.0), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK - 1;
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object("devourer_lunge_lane", obj);
        }
        // ── The Gravity Weaver ───────────────────────────────────────
        // Four spindles and the loom (index WEAVER_SPINDLES), then a thread
        // strip per part (the lane and the shuttle thread draw on it), and
        // the lens quad the inversion telegraph draws on. Self-lit: the
        // violet is the light source in this fight.
        for i in 0..=WEAVER_SPINDLES {
            let name = format!("weaver_part_{i}");
            let loom = i == WEAVER_SPINDLES;
            let d = if loom { WEAVER_LOOM_SIZE } else { WEAVER_SPINDLE_SIZE };
            let (art, idle): (&'static [u8], &'static [&'static [u8]]) = if loom {
                (ASSET_PL_WEAVER_LOOM, &ASSET_PL_WEAVER_LOOM_IDLE)
            } else {
                (ASSET_PL_WEAVER_SPINDLE, &ASSET_PL_WEAVER_SPINDLE_IDLE)
            };
            let img = crate::scenes::game::helpers::pl_image_cached(art, d)
                .unwrap_or_else(|| std::sync::Arc::new(crate::images::solid(120, 60, 200, 255)));
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0), image: img.into(), color: None }),
                (d, d), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK;
            obj.gravity = 0.0;
            obj.visible = false;
            obj.unlit = true;
            if let Some(anim) = crate::scenes::game::helpers::pl_sprite(idle, d, WEAVER_IDLE_FPS) {
                obj.animated_sprite = Some(anim);
            }
            obj.set_glow(GlowConfig {
                color: Color(WEAVER_SRGB.0, WEAVER_SRGB.1, WEAVER_SRGB.2, 190),
                width: if loom { 14.0 } else { 9.0 },
            });
            scene = scene.with_object(&name, obj);
        }
        for i in 0..=WEAVER_SPINDLES {
            let name = format!("weaver_thread_{i}");
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (100.0, WEAVER_THREAD_W), 0.0),
                             image: std::sync::Arc::new(solid(0, 0, 0, 0)).into(), color: None }),
                (100.0, WEAVER_THREAD_W), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK - 1;
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object(&name, obj);
        }
        {
            let d = WEAVER_LOOM_SIZE * WEAVER_LENS_SCALE;
            let mut obj = GameObject::new_rect(ctx, "weaver_lens".into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0),
                             image: std::sync::Arc::new(solid(0, 0, 0, 0)).into(), color: None }),
                (d, d), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK - 2;
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object("weaver_lens", obj);
        }
        // ── The Flare Titan ──────────────────────────────────────────
        // Four vents and the core (index TITAN_VENTS), an arc strip and a
        // landing zone per part (the prominence draws on them), and the
        // corona quad the kindle's solar wind draws on. Self-lit: the star
        // is the light source in this fight.
        for i in 0..=TITAN_VENTS {
            let name = format!("titan_part_{i}");
            let core = i == TITAN_VENTS;
            let d = if core { TITAN_CORE_SIZE } else { TITAN_VENT_SIZE };
            let (art, idle): (&'static [u8], &'static [&'static [u8]]) = if core {
                (ASSET_PL_TITAN_CORE, &ASSET_PL_TITAN_CORE_IDLE)
            } else {
                (ASSET_PL_TITAN_VENT, &ASSET_PL_TITAN_VENT_IDLE)
            };
            let img = crate::scenes::game::helpers::pl_image_cached(art, d)
                .unwrap_or_else(|| std::sync::Arc::new(crate::images::solid(255, 150, 40, 255)));
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0), image: img.into(), color: None }),
                (d, d), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK;
            obj.gravity = 0.0;
            obj.visible = false;
            obj.unlit = true;
            if let Some(anim) = crate::scenes::game::helpers::pl_sprite(idle, d, TITAN_IDLE_FPS) {
                obj.animated_sprite = Some(anim);
            }
            scene = scene.with_object(&name, obj);
        }
        for i in 0..=TITAN_VENTS {
            for (name, layer) in [(format!("titan_arc_{i}"), LAYER_SPACE_HOOK - 1), (format!("titan_zone_{i}"), LAYER_SPACE_HOOK - 1)] {
                let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                    Some(Image { shape: ShapeType::Rectangle(0.0, (100.0, 100.0), 0.0),
                                 image: std::sync::Arc::new(solid(0, 0, 0, 0)).into(), color: None }),
                    (100.0, 100.0), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                obj.layer = layer;
                obj.gravity = 0.0;
                obj.visible = false;
                scene = scene.with_object(&name, obj);
            }
        }
        {
            let d = TITAN_CORE_SIZE * TITAN_CORONA_SCALE;
            let mut obj = GameObject::new_rect(ctx, "titan_corona".into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0),
                             image: std::sync::Arc::new(solid(0, 0, 0, 0)).into(), color: None }),
                (d, d), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK - 2;
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object("titan_corona", obj);
        }
        // ── The Magnetar ─────────────────────────────────────────────
        // Two poles and the core (index MAGNETAR_POLES), a beam strip per
        // beam (a pole's, or the bare core's four), and the field quad the
        // pulses draw on. Self-lit.
        for i in 0..=MAGNETAR_POLES {
            let name = format!("magnetar_part_{i}");
            let core = i == MAGNETAR_POLES;
            let d = if core { MAGNETAR_CORE_SIZE } else { MAGNETAR_POLE_SIZE };
            let (art, idle): (&'static [u8], &'static [&'static [u8]]) = if core {
                (ASSET_PL_MAGNETAR_CORE, &ASSET_PL_MAGNETAR_CORE_IDLE)
            } else {
                (ASSET_PL_MAGNETAR_POLE, &ASSET_PL_MAGNETAR_POLE_IDLE)
            };
            let img = crate::scenes::game::helpers::pl_image_cached(art, d)
                .unwrap_or_else(|| std::sync::Arc::new(crate::images::solid(120, 200, 255, 255)));
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0), image: img.into(), color: None }),
                (d, d), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK;
            obj.gravity = 0.0;
            obj.visible = false;
            obj.unlit = true;
            if let Some(anim) = crate::scenes::game::helpers::pl_sprite(idle, d, MAGNETAR_IDLE_FPS) {
                obj.animated_sprite = Some(anim);
            }
            scene = scene.with_object(&name, obj);
        }
        for k in 0..crate::scenes::game::boss::MAGNETAR_BEAMS {
            let name = format!("magnetar_beam_{k}");
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (100.0, 100.0), 0.0),
                             image: std::sync::Arc::new(solid(0, 0, 0, 0)).into(), color: None }),
                (100.0, 100.0), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK - 1;
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object(&name, obj);
        }
        {
            let d = MAGNETAR_FIELD_VISUAL;
            let mut obj = GameObject::new_rect(ctx, "magnetar_field".into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0),
                             image: std::sync::Arc::new(solid(0, 0, 0, 0)).into(), color: None }),
                (d, d), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK - 2;
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object("magnetar_field", obj);
        }
        // ── Conductor ────────────────────────────────────────────────
        // A metronome core, a ring of tuning-fork spars, an expanding beat ring
        // and two sweeping bar lines.
        //
        // Every one of these is parked offscreen with gravity and momentum at
        // zero. A manually-positioned object left with the default gravity
        // accumulates engine momentum while parked and flies off the moment it
        // is revealed.
        {
            // Core: the metronome case. The bob is separate so it can be moved
            // along its arc without rebuilding an image every frame.
            //
            // The Conductor has its OWN size, larger than the shared one — see
            // `CONDUCTOR_CORE_SIZE`.
            let d = CONDUCTOR_CORE_SIZE;
            // Generated art if it decodes, the drawn one if it does not. The
            // procedural version stays the fallback rather than being deleted:
            // it is correct, it costs nothing, and it is what renders if an
            // asset is ever missing from a build.
            let img = crate::scenes::game::helpers::pl_image(ASSET_PL_CORE, d)
                .unwrap_or_else(|| {
                    crate::images::conductor_core(d.round().max(2.0) as u32, (86, 200, 190))
                });
            let mut obj = GameObject::new_rect(ctx, "conductor_core".into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0), image: img.into(), color: None }),
                (d, d), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            // The idle loop. Set here so the core breathes from the moment it
            // appears; `update_animation` only advances it while visible.
            obj.animated_sprite =
                crate::scenes::game::helpers::pl_sprite(
                    &ASSET_PL_CORE_IDLE, d, PL_CORE_IDLE_FPS);
            obj.layer = LAYER_SPACE_HOOK;
            obj.gravity = 0.0;
            obj.momentum = (0.0, 0.0);
            obj.visible = false;
            scene = scene.with_object("conductor_core", obj);

            // The bob: swings across the case's scale slot, one beat per side.
            let bd = CONDUCTOR_CORE_SIZE * 0.17;
            let mut bob = GameObject::new_rect(ctx, "conductor_bob".into(),
                Some(Image {
                    shape: ShapeType::Ellipse(0.0, (bd, bd), 0.0),
                    image: crate::images::circle_cached((bd * 0.5) as u32, 255, 236, 150),
                    color: None,
                }),
                (bd, bd), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            bob.layer = LAYER_SPACE_HOOK + 1;
            bob.gravity = 0.0;
            bob.momentum = (0.0, 0.0);
            bob.visible = false;
            scene = scene.with_object("conductor_bob", bob);

            // Spars: the first visual beat channel.
            for i in 0..CONDUCTOR_SPARS {
                let name = format!("conductor_spar_{i}");
                let sd = CONDUCTOR_SPAR_SIZE;
                let img = crate::scenes::game::helpers::pl_image(ASSET_PL_SPAR, sd)
                    .unwrap_or_else(|| {
                        crate::images::conductor_spar(
                            sd.round().max(2.0) as u32, (120, 225, 215))
                    });
                let mut o = GameObject::new_rect(ctx, name.clone().into(),
                    Some(Image { shape: ShapeType::Rectangle(0.0, (sd, sd), 0.0), image: img.into(), color: None }),
                    (sd, sd), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                // Each spar rings on its own phase so the ring travels round
                // the circle instead of all six flashing as one.
                o.animated_sprite = crate::scenes::game::helpers::pl_sprite(
                    &ASSET_PL_SPAR_RING, sd, PL_SPAR_RING_FPS);
                if let Some(sp) = o.animated_sprite.as_mut() {
                    sp.set_frame(i % ASSET_PL_SPAR_RING.len());
                }
                o.layer = LAYER_SPACE_HOOK;
                o.gravity = 0.0;
                o.momentum = (0.0, 0.0);
                o.visible = false;
                scene = scene.with_object(&name, o);
            }

            // Beat ring: the second visual beat channel. Drawn at full size and
            // SCALED down per frame rather than redrawn, so its texture is
            // uploaded once for the whole fight.
            // Rasterised SMALL and displayed large. A ring at its full 900px
            // radius is 3.2 million pixels each doing a sqrt, and the game
            // crate's own loops are not optimised in a test build (opt-level 3
            // covers dependencies only) — it pushed the test suite from 16
            // seconds to over ninety. A ring is smooth, so nothing about it
            // needs texel-per-pixel detail.
            const RING_TEX_R: u32 = 192;
            // Born SMALL. `obj.size` is both the drawn extent and the physics
            // AABB (`update_image_shape` rescales the shape from it), so an
            // object parked at its display size puts a 1800x1800 body into the
            // solver every frame for the whole run. Measured: four oversized
            // decorations took the test suite from 16s to 40s while invisible.
            // The fight sets the real size on the frames it is shown.
            let rd = CONDUCTOR_DORMANT;
            let ring_img = crate::images::conductor_beat_ring(RING_TEX_R, (150, 240, 235), 210);
            let mut ring = GameObject::new_rect(ctx, "conductor_beat_ring".into(),
                Some(Image { shape: ShapeType::Ellipse(0.0, (rd, rd), 0.0), image: ring_img.into(), color: None }),
                (rd, rd), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            ring.layer = LAYER_SPACE_HOOK - 1;
            ring.gravity = 0.0;
            ring.momentum = (0.0, 0.0);
            ring.visible = false;
            scene = scene.with_object("conductor_beat_ring", ring);

            // Bar lines: two segments each, above and below a moving safe gap.
            // Two objects rather than one image with a hole, because the gap
            // moves every bar and an image rebuilt per bar is a texture upload
            // per bar.
            for i in 0..2 {
                for half in ["top", "bot"] {
                    let name = format!("conductor_bar_{i}_{half}");
                    // Small texture, and a small dormant size for the same
                    // reason as the ring above.
                    let (w, h) = (CONDUCTOR_DORMANT, CONDUCTOR_DORMANT);
                    let img = crate::images::conductor_bar_segment(24, 384, (170, 245, 255));
                    let mut o = GameObject::new_rect(ctx, name.clone().into(),
                        Some(Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img.into(), color: None }),
                        (w, h), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                    o.layer = LAYER_SPACE_HOOK;
                    o.gravity = 0.0;
                    o.momentum = (0.0, 0.0);
                    o.visible = false;
                    scene = scene.with_object(&name, o);
                }
            }
        }

        // Conductor weakpoint marker + Resonance pips.
        {
            // The marker is drawn at the REAL strike radius and parked small,
            // for the same reason as the ring above: size is the physics AABB
            // as well as the drawn extent.
            let wd = CONDUCTOR_WEAK_R * 2.0;
            let img = crate::images::conductor_weakpoint(
                (wd * 0.5).round().max(2.0) as u32, (255, 236, 150));
            let mut o = GameObject::new_rect(ctx, "conductor_weak_zone".into(),
                Some(Image { shape: ShapeType::Ellipse(0.0, (wd, wd), 0.0), image: img.into(), color: None }),
                (CONDUCTOR_DORMANT, CONDUCTOR_DORMANT), (-9000.0, -9000.0),
                vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            o.layer = LAYER_SPACE_HOOK - 1;
            o.gravity = 0.0;
            o.momentum = (0.0, 0.0);
            o.visible = false;
            o.ignore_zoom = true;
            scene = scene.with_object("conductor_weak_zone", o);

            // The close-range guard. A WORLD object (not ignore_zoom) because
            // it has to sit on the core and scale with the arena the way the
            // boss does, and it is parked small for the usual reason: `size`
            // is the physics AABB as well as the drawn extent.
            let ed = CONDUCTOR_EQ_R * 2.0;
            let mut g = GameObject::new_rect(ctx, "conductor_eq".into(),
                Some(Image {
                    shape: ShapeType::Rectangle(0.0, (ed, ed), 0.0),
                    // TRANSPARENT. The guard is drawn entirely by its mega
                    // effect; the object is only the rectangle that effect is
                    // resolved onto. An opaque drawable here renders as a
                    // white box over the whole boss whenever the guard is
                    // live — which is exactly what it did.
                    image: crate::images::solid(0, 0, 0, 0).into(),
                    color: None,
                }),
                (CONDUCTOR_DORMANT, CONDUCTOR_DORMANT), (-9000.0, -9000.0),
                vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            g.layer = LAYER_SPACE_HOOK - 2;
            g.gravity = 0.0;
            g.momentum = (0.0, 0.0);
            g.visible = false;
            // Purely a visual: the damage is a radius test in `tick_eq_guard`,
            // not a contact, because it has to fire on a BAND of the arc
            // rather than anywhere inside the rectangle.
            g.collision_mode = CollisionMode::NonPlatform;
            scene = scene.with_object("conductor_eq", g);

            // The Resonance meter: one strip carrying the waveform effect.
            //
            // Replaced three pips. A count says how many; the wave says how
            // close, and on a fight scored by rhythm the honest picture of the
            // charge is a sound wave — which is also what changes colour when
            // the boss becomes vulnerable.
            {
                let mut o = GameObject::new_rect(ctx, "conductor_meter".into(),
                    Some(Image {
                        shape: ShapeType::Rectangle(0.0, (CONDUCTOR_METER_W, CONDUCTOR_METER_H), 0.0),
                        image: crate::images::solid(0, 0, 0, 0).into(),
                        color: None,
                    }),
                    (CONDUCTOR_METER_W, CONDUCTOR_METER_H), (-9000.0, -9000.0),
                    vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                o.layer = 10_000;
                o.gravity = 0.0;
                o.momentum = (0.0, 0.0);
                o.visible = false;
                o.ignore_zoom = true;
                scene = scene.with_object("conductor_meter", o);
            }
        }

        // The Conductor's beat field: four edge strips.
        //
        // Not one full-screen quad — the effect is a vignette, so a
        // full-screen pass pays fill for fragments it renders invisible.
        // Measured at +1.70ms a frame (+33%) at phone resolution.
        for (i, name) in ["conductor_field_top", "conductor_field_bot",
                          "conductor_field_left", "conductor_field_right"]
            .iter().enumerate()
        {
            let (w, h) = if i < 2 {
                (VW, VH * BEAT_FIELD_DEPTH)
            } else {
                (VW * BEAT_FIELD_DEPTH, VH)
            };
            let mut o = GameObject::new_rect(ctx, (*name).into(),
                Some(Image {
                    shape: ShapeType::Rectangle(0.0, (w, h), 0.0),
                    image: crate::images::solid(0, 0, 0, 0).into(),
                    color: None,
                }),
                (CONDUCTOR_DORMANT, CONDUCTOR_DORMANT), (-9000.0, -9000.0),
                vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            // Under the HUD, over the world: never on top of a number the
            // player needs to read.
            o.layer = 9_400;
            o.gravity = 0.0;
            o.momentum = (0.0, 0.0);
            o.visible = false;
            o.ignore_zoom = true;
            scene = scene.with_object(*name, o);
        }

        // Colossus danger zones: translucent discs that show exactly where each
        // part's attack will land, for ~1s before it fires. Sized to match the
        // part's zone radius (hand / torso / head). Hidden until a telegraph.
        const COLOSSUS_ZONE_RADII: [f32; 4] = [
            COLOSSUS_HAND_ZONE_R,
            COLOSSUS_HAND_ZONE_R,
            COLOSSUS_TORSO_ZONE_R,
            COLOSSUS_HEAD_ZONE_R,
        ];
        for (i, zr) in COLOSSUS_ZONE_RADII.iter().enumerate() {
            let name = format!("colossus_zone_{i}");
            let d = (zr * 2.0).round().max(2.0) as u32;
            let img = crate::images::danger_zone(d / 2, 255, 90, 40);
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image { shape: ShapeType::Ellipse(0.0, (d as f32, d as f32), 0.0), image: img.into(), color: None }),
                (d as f32, d as f32), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK;
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object(&name, obj);
        }
        // Colossus attack paths: translucent red strips showing the trajectory a
        // part will follow (from where it started the telegraph to its target),
        // so the player can read and dodge the whole swing, not just the landing
        // spot. Hidden until a telegraph; resized/rotated each frame in boss.rs.
        for i in 0..4 {
            let name = format!("colossus_path_{i}");
            let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (100.0, COLOSSUS_PATH_THICKNESS), 0.0), image: crate::images::solid(255, 40, 30, 90).into(), color: None }),
                (100.0, COLOSSUS_PATH_THICKNESS), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = LAYER_SPACE_HOOK - 1; // behind the parts
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object(&name, obj);
        }
        // Colossus gravity well: a large translucent swirl centred on the head,
        // shown while its gaze attack winds up/fires. The image is small and
        // stretched to the object size so the raster stays cheap.
        {
            let name = "colossus_well";
            let well_d = COLOSSUS_GRAVITY_RANGE * 2.0;
            let img = crate::images::gravity_well_img(400, 140, 90, 220);
            let mut obj = GameObject::new_rect(ctx, name.into(),
                Some(Image { shape: ShapeType::Ellipse(0.0, (well_d, well_d), 0.0), image: img.into(), color: None }),
                (well_d, well_d), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            obj.layer = 28; // under the parts, so the well reads as a ground zone
            obj.gravity = 0.0;
            obj.visible = false;
            scene = scene.with_object(name, obj);
        }
        // Boss arena boundary walls: translucent barriers marking the left/right
        // edges and floor of the fight so the player can see the play area
        // limits. Positioned/sized per-tick in boss.rs.
        {
            let wall_th = ARENA_WALL_THICKNESS;
            let wall_h = 4800.0;
            // Transparent: the wall is drawn by the ENERGY WALL effect, attached
            // each frame by `tick_arena_walls`, so it can scroll and flicker.
            let wall_img = crate::images::solid(0, 0, 0, 0);
            for name in ["arena_wall_l", "arena_wall_r"] {
                let mut obj = GameObject::new_rect(ctx, name.into(),
                    Some(Image { shape: ShapeType::Rectangle(0.0, (wall_th, wall_h), 0.0), image: wall_img.clone().into(), color: None }),
                    (wall_th, wall_h), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                obj.layer = 19; // behind the boss parts / player
                obj.gravity = 0.0;
                obj.visible = false;
                // NOT a platform. The default collision mode is `Surface`,
                // which resolves against a body's SURFACE NORMAL — and an
                // unrotated rectangle's normal is (0, -1), i.e. "up".
                //
                // A 140 x 9600 wall in that mode is a floor standing on its
                // end: its collidable face is its top edge, ~9000px above the
                // arena, so a player descending anywhere inside that thin,
                // arena-tall box was resolved onto it and held there. That is
                // the "stuck against the wall falling". The bounce below owns
                // the walls' behaviour instead, so the solver must not also
                // have an opinion about them.
                obj.collision_mode = CollisionMode::NonPlatform;
                obj.set_glow(GlowConfig { color: Color(120, 170, 255, 120), width: 26.0 });
                scene = scene.with_object(name, obj);
            }
        }
        // Gaze-beam charge orb + bright core (the head attack). The charge orb
        // grows at the head while it winds up; the bright core is the beam
        // itself, travelling along the telegraphed path when it fires.
        {
            let cr = 120.0;
            let img = circle_cached(cr as u32, 255, 240, 140);
            let mut charge = GameObject::new_rect(ctx, "colossus_charge".into(),
                Some(Image { shape: ShapeType::Ellipse(0.0, (cr * 2.0, cr * 2.0), 0.0), image: img.into(), color: None }),
                (cr * 2.0, cr * 2.0), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            charge.layer = 30;
            charge.gravity = 0.0;
            charge.visible = false;
            charge.set_glow(GlowConfig { color: Color(255, 240, 140, 255), width: 40.0 });
            scene = scene.with_object("colossus_charge", charge);

            let core_th = 28.0;
            let mut core = GameObject::new_rect(ctx, "colossus_beam_core".into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (100.0, core_th), 0.0), image: crate::images::solid(255, 250, 200, 235).into(), color: None }),
                (100.0, core_th), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            core.layer = 31; // just above the path field
            core.gravity = 0.0;
            core.visible = false;
            core.set_glow(GlowConfig { color: Color(255, 250, 200, 200), width: 30.0 });
            scene = scene.with_object("colossus_beam_core", core);

            // The beam is drawn as a POLYLINE, not one rotated strip: a curved
            // beam cannot be a single rectangle, and the same pools draw a
            // straight one with the segments simply collinear — so there is one
            // code path rather than a straight case and a curved case.
            // `..._tel_` is the dim telegraph field, `..._core_` the bright core
            // that travels along it as the beam fires.
            for i in 0..COLOSSUS_BEAM_SEGMENTS {
                let name = format!("colossus_beam_tel_{i}");
                let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                    Some(Image { shape: ShapeType::Rectangle(0.0, (100.0, COLOSSUS_BEAM_THICKNESS), 0.0), image: crate::images::solid(255, 40, 30, 80).into(), color: None }),
                    (100.0, COLOSSUS_BEAM_THICKNESS), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                obj.layer = LAYER_SPACE_HOOK - 1; // behind the parts
                obj.gravity = 0.0;
                obj.visible = false;
                // Deliberately NO glow: a glow is a second, larger drawable per
                // object, and this strip is already the widest thing on screen.
                // The bright core carries the read.
                scene = scene.with_object(&name, obj);

                let name = format!("colossus_beam_core_{i}");
                let core_th = COLOSSUS_BEAM_THICKNESS * 0.46;
                let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                    Some(Image { shape: ShapeType::Rectangle(0.0, (100.0, core_th), 0.0), image: crate::images::solid(255, 250, 200, 235).into(), color: None }),
                    (100.0, core_th), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                obj.layer = 31;
                obj.gravity = 0.0;
                obj.visible = false;
                obj.set_glow(GlowConfig { color: Color(255, 250, 200, 200), width: 34.0 });
                scene = scene.with_object(&name, obj);
            }

            // Core-vent spokes: plasma bars radiating from the torso, rotated
            // around it while the vent runs. Positioned and rotated per tick in
            // boss.rs; hidden whenever the torso is not venting.
            for i in 0..COLOSSUS_VENT_SPOKES {
                let name = format!("colossus_vent_{i}");
                let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                    Some(Image { shape: ShapeType::Rectangle(0.0, (COLOSSUS_VENT_LENGTH, COLOSSUS_VENT_THICKNESS), 0.0), image: crate::images::solid(255, 170, 90, 210).into(), color: None }),
                    (COLOSSUS_VENT_LENGTH, COLOSSUS_VENT_THICKNESS), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                obj.layer = 29; // under the parts, over the arena dressing
                obj.gravity = 0.0;
                obj.visible = false;
                obj.set_glow(GlowConfig { color: Color(255, 190, 110, 220), width: 34.0 });
                scene = scene.with_object(&name, obj);
            }

            // The clap's force wave: a ring that expands from the impact point.
            {
                let name = "colossus_clap_wave";
                let r = COLOSSUS_CLAP_WAVE_R;
                let ring = gwell_ring_cached(r, 190, 220, 255, GWELL_RING_COUNT, 200.0);
                let mut obj = GameObject::new_rect(ctx, name.into(),
                    Some(Image { shape: ShapeType::Ellipse(0.0, (r * 2.0, r * 2.0), 0.0), image: ring, color: None }),
                    (r * 2.0, r * 2.0), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                obj.layer = 32; // over everything the fight draws
                obj.gravity = 0.0;
                obj.visible = false;
                scene = scene.with_object(name, obj);
            }

            // Little contact explosions along the beam's path: small bright
            // circles that quickly grow a few sizes as the beam sweeps across.
            for i in 0..8 {
                let name = format!("colossus_beam_explode_{i}");
                let er = COLOSSUS_BEAM_EXPLODE_R1;
                let img = circle_cached(er as u32, 255, 160, 60);
                let mut obj = GameObject::new_rect(ctx, name.clone().into(),
                    Some(Image { shape: ShapeType::Ellipse(0.0, (er * 2.0, er * 2.0), 0.0), image: img.into(), color: None }),
                    (er * 2.0, er * 2.0), (-9000.0, -9000.0), vec!["boss".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
                obj.layer = 31;
                obj.gravity = 0.0;
                obj.visible = false;
                obj.set_glow(GlowConfig { color: Color(255, 170, 70, 255), width: 26.0 });
                scene = scene.with_object(&name, obj);
            }
        }
    }

    // ── Boss weakpoint markers ────────────────────────────────────────────
    // Bright gold rings drawn on top of the purple body so weakpoints are
    // obvious during a playtest. Follow the boss in boss.rs.
    {
        let wpr = BOSS_WEAKPOINT_R;
        let d = (wpr * 2.0).round().max(2.0) as u32;
        // Transparent. These used to be filled gold rings drawn ON TOP of the
        // boss body — four opaque discs over the art. The marker attached each
        // frame carries them now, in the same reticle geometry every other
        // boss uses for "hit here".
        let ring: std::sync::Arc<image::RgbaImage> =
            std::sync::Arc::new(solid(0, 0, 0, 0));
        let _ = gwell_ring_cached(wpr, 255, 210, 80, GWELL_RING_COUNT, 235.0);
        for (i, _) in BOSS_WEAKPOINT_OFFSETS.iter().enumerate() {
            let id = format!("boss_weak_{i}");
            let mut wp = GameObject::new_rect(
                ctx, id.clone().into(),
                Some(Image { shape: ShapeType::Ellipse(0.0, (d as f32, d as f32), 0.0), image: ring.clone(), color: None }),
                (d as f32, d as f32), (-6000.0, -6000.0),
                vec!["boss_weak".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
            );
            wp.layer = LAYER_SPACE_HOOK;
            wp.gravity = 0.0;
            wp.visible = false;
            // NO GLOW. Making the image transparent was not enough: a glow is
            // drawn from the object's SHAPE, not its image, so four gold halos
            // kept sitting over the boss art after the discs behind them were
            // gone. This was the "old image layered on top" all along.
            wp.clear_glow();
            scene = scene.with_object(&id, wp);
        }
    }

    // ── Boss warp wormhole overlay ────────────────────────────────────────
    // A large wormhole gif shown briefly when the player is warped into (or out
    // of) a boss arena. Animation set at warp time in boss.rs.
    {
        let mut wf = GameObject::new_rect(ctx, "warp_flash".into(),
            None::<Image>, (VW, VH), (-6000.0, -6000.0),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        wf.visible = false;
        wf.layer = 9000;
        scene = scene.with_object("warp_flash", wf);
    }

    // ── Boss teleport threshold marker ─────────────────────────────────────
    // A huge black-hole swirl at the boss threshold (x = BOSS_THRESHOLD_X) so
    // the player has a clear visual that they are heading into something
    // special; the teleport threshold is near its centre. It sits on a very low
    // world layer (-1; below the default layer 0 used by turrets/pads/rocket-
    // pads, spinners at 5 and hooks at 21) so ALL playable obstacles render in
    // front of it, and a down-pointing arrow floats just above it on a higher
    // layer so it stays visible. Shown in boss mode as the player approaches
    // (boss.rs::spawn_boss_approach_nodes).
    {
        let d = BOSS_MARKER_D;
        let mut mk = GameObject::new_rect(ctx, "boss_threshold_marker".into(),
            None::<Image>, (d, d), (BOSS_THRESHOLD_X - d * 0.5, BOSS_MARKER_Y - d * 0.5),
            vec!["boss_marker".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        mk.set_animation(blackhole1_template());
        mk.visible = false;
        mk.gravity = 0.0;
        mk.layer = 1;
        scene = scene.with_object("boss_threshold_marker", mk);

        // Down-pointing arrow above the marker (points at the teleport point).
        let asz = 220.0;
        let acx = BOSS_THRESHOLD_X;
        let acy = BOSS_MARKER_Y - 900.0;
        let mut arr = GameObject::new_rect(ctx, "boss_marker_arrow".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (asz, asz), 0.0), image: arrow_img(asz as u32, 255, 200, 60, 240).into(), color: None }),
            (asz, asz), (acx - asz * 0.5, acy - asz * 0.5),
            vec!["boss_marker".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        arr.rotation = 90.0; // point down at the black hole
        arr.visible = false;
        arr.gravity = 0.0;
        arr.layer = 40;
        scene = scene.with_object("boss_marker_arrow", arr);
    }

    // ── Last-boss: barrier + generators ────────────────────────────────────
    // Placeholder recolored shapes; real art can replace them later.
    {
        let gr = BOSS_GENERATOR_R;
        let gd = (gr * 2.0).round().max(2.0) as u32;
        let (gen_intact, _, gen_idle) = devourer_generator_assets();
        let gen_img: std::sync::Arc<image::RgbaImage> =
            crate::scenes::game::helpers::pl_image_cached(gen_intact, gd as f32)
            .unwrap_or_else(|| std::sync::Arc::new(circle_img(
                gr as u32, C_BOSS_GENERATOR.0, C_BOSS_GENERATOR.1, C_BOSS_GENERATOR.2)));
        for i in 0..BOSS_GENERATOR_COUNT {
            let id = format!("boss_gen_{i}");
            let mut gen = GameObject::new_rect(
                ctx, id.clone().into(),
                Some(Image { shape: ShapeType::Ellipse(0.0, (gd as f32, gd as f32), 0.0), image: gen_img.clone().into(), color: None }),
                (gd as f32, gd as f32), (-6000.0, -6000.0),
                vec!["boss_gen".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
            );
            gen.layer = LAYER_SPACE_HOOK;
            gen.gravity = 0.0;
            gen.visible = false;
            // Self-powered, and the fight's objective: drawn as authored in
            // every phase instead of dimming with the arena in a darkness.
            // (In daylight the ambient is 1.0, so unlit looks identical.)
            gen.unlit = true;
            // Idle loop: the core pulsing is what says "this is live".
            if let Some(anim) = crate::scenes::game::helpers::pl_sprite(
                gen_idle, gd as f32, DEVOURER_GENERATOR_IDLE_FPS)
            {
                gen.animated_sprite = Some(anim);
            }
            // A cyan halo that follows the machine's own outline and blooms —
            // the same energy as the dome it feeds.
            gen.set_glow(GlowConfig { color: Color(C_BOSS_GENERATOR.0, C_BOSS_GENERATOR.1, C_BOSS_GENERATOR.2, 200), width: 10.0 });
            scene = scene.with_object(&id, gen);
        }
        // Shield tethers: one per generator, the power line it feeds into the
        // boss's dome. Transparent strip; the ENERGY TETHER effect draws it,
        // and sun_devourer.rs stretches and turns it between the two each
        // frame. Behind the generators, so the line plugs INTO them.
        for i in 0..BOSS_GENERATOR_COUNT {
            let id = format!("boss_gen_tether_{i}");
            let mut t = GameObject::new_rect(
                ctx, id.clone().into(),
                Some(Image {
                    shape: ShapeType::Rectangle(0.0, (100.0, DEVOURER_TETHER_WIDTH), 0.0),
                    image: std::sync::Arc::new(solid(0, 0, 0, 0)).into(),
                    color: None,
                }),
                (100.0, DEVOURER_TETHER_WIDTH), (-6000.0, -6000.0),
                vec!["boss_gen".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
            );
            t.layer = LAYER_SPACE_HOOK - 1;
            t.gravity = 0.0;
            t.visible = false;
            scene = scene.with_object(&id, t);
        }
        // Boss forcefield: a glowing ring around the boss while the generators
        // are still up (the boss is invulnerable until they are destroyed).
        {
            let d = (BOSS_SIZE * 1.5).round() as u32;
            // Transparent: the shield is drawn by the ENERGY DOME effect,
            // attached each frame. A static ring image reads as a decal
            // painted over the boss rather than as a field holding it.
            let ring: std::sync::Arc<image::RgbaImage> =
                std::sync::Arc::new(solid(0, 0, 0, 0));
            let _ = gwell_ring_cached(BOSS_SIZE * 0.75, C_BOSS_BARRIER.0, C_BOSS_BARRIER.1, C_BOSS_BARRIER.2, 3, 150.0);
            let mut ff = GameObject::new_rect(ctx, "boss_forcefield".into(),
                Some(Image { shape: ShapeType::Ellipse(0.0, (d as f32, d as f32), 0.0), image: ring, color: None }),
                (d as f32, d as f32), (-6000.0, -6000.0),
                vec!["boss_forcefield".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            ff.layer = LAYER_SPACE_HOOK + 2;
            ff.gravity = 0.0;
            ff.visible = false;
            scene = scene.with_object("boss_forcefield", ff);
        }

    }

    // ── Boss HP bar ───────────────────────────────────────────────────────
    {
        let (bw, bh) = (BOSS_HP_BAR_W as u32, BOSS_HP_BAR_H as u32);
        let mut bar_img = image::RgbaImage::new(bw, bh);
        for py in 0..bh { for px in 0..bw {
            bar_img.put_pixel(px, py, image::Rgba([C_BOSS_HP_FILL.0, C_BOSS_HP_FILL.1, C_BOSS_HP_FILL.2, 255]));
        }}
        let mut boss_hp_bar = GameObject::new_rect(ctx, "boss_hp_bar".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (BOSS_HP_BAR_W, BOSS_HP_BAR_H), 0.0), image: bar_img.into(), color: None }),
            (BOSS_HP_BAR_W, BOSS_HP_BAR_H), (VW * 0.5 - BOSS_HP_BAR_W * 0.5, VH * 0.12),
            vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        boss_hp_bar.visible = false;
        boss_hp_bar.ignore_zoom = true;
        boss_hp_bar.layer = 101;
        scene = scene.with_object("boss_hp_bar", boss_hp_bar);
    }

    // ── Boss-name indicator + off-screen objective arrows ─────────────────
    // `boss_name_text` is a HUD banner (text drawable set in build_scene.rs)
    // shown during the fight. The arrows are HUD sprites that appear at the
    // screen edge pointing at the boss / generators when they are off-screen.
    {
        let mut boss_name_text = GameObject::build("boss_name_text")
            .size(1000.0, 100.0)
            .position((VW - 1000.0) * 0.5, VH * 0.17)
            .tag("hud")
            .build(ctx);
        boss_name_text.visible = false;
        boss_name_text.ignore_zoom = true;
        boss_name_text.layer = 102;
        scene = scene.with_object("boss_name_text", boss_name_text);

        let asz = 64u32;
        let arrow = arrow_img(asz, C_BOSS_BARRIER.0, C_BOSS_BARRIER.1, C_BOSS_BARRIER.2, 230);
        let mut boss_off_arrow = GameObject::new_rect(ctx, "boss_off_arrow".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (asz as f32, asz as f32), 0.0), image: arrow.clone().into(), color: None }),
            (asz as f32, asz as f32), (VW * 0.5, VH * 0.5),
            vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        boss_off_arrow.visible = false;
        boss_off_arrow.ignore_zoom = true;
        boss_off_arrow.layer = 102;
        scene = scene.with_object("boss_off_arrow", boss_off_arrow);

        for i in 0..BOSS_GENERATOR_COUNT {
            let id = format!("gen_arrow_{i}");
            let mut a = GameObject::new_rect(ctx, id.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (asz as f32, asz as f32), 0.0), image: arrow.clone().into(), color: None }),
                (asz as f32, asz as f32), (VW * 0.5, VH * 0.5),
                vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
            a.visible = false;
            a.ignore_zoom = true;
            a.layer = 102;
            scene = scene.with_object(&id, a);
        }
    }

    // ── Hit-registration pool ─────────────────────────────────────────────
    //
    // Transparent objects that exist only to carry an impact effect. Parked
    // far off screen when idle, like every other pool here.
    let mut impact_free: Vec<String> = Vec::new();
    for i in 0..IMPACT_POOL_SIZE {
        let id = format!("impact_{i}");
        let mut obj = GameObject::new_rect(ctx, id.clone(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (64.0, 64.0), 0.0),
                image: solid(0, 0, 0, 0).into(),
                color: None,
            }),
            (8.0, 8.0), (-9000.0, -9000.0),
            vec!["impact".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.gravity = 0.0;
        obj.visible = false;
        // Above the bosses: a hit has to be visible over the thing it hit.
        obj.layer = LAYER_SPACE_HOOK + 4;
        impact_free.push(id.clone());
        scene = scene.with_object(&id, obj);
    }

    // ── Boss bolt pool ────────────────────────────────────────────────────
    let mut boss_bolt_free: Vec<String> = Vec::new();
    for i in 0..BOSS_BOLT_POOL_SIZE {
        let id = format!("boss_bolt_{i}");
        let mut obj = GameObject::new_rect(ctx, id.clone(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (BOSS_BOLT_W, BOSS_BOLT_H), 0.0),
                image: crate::scenes::game::helpers::pl_image_fit_cached(
                    ASSET_PL_DEVOURER_BOLT, BOSS_BOLT_W, BOSS_BOLT_H)
                    .unwrap_or_else(|| std::sync::Arc::new(
                        solid(C_BOSS_BOLT.0, C_BOSS_BOLT.1, C_BOSS_BOLT.2, 255))),
                color: None,
            }),
            (BOSS_BOLT_W, BOSS_BOLT_H), (-7000.0, -7000.0),
            vec!["boss_bolt".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.gravity = 0.0; obj.visible = false;
        boss_bolt_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Boss arena asteroids ──────────────────────────────────────────────
    let mut boss_asteroid_ids: Vec<String> = Vec::new();
    for i in 0..BOSS_ASTEROID_COUNT {
        let id = format!("boss_asteroid_{i}");
        let size = boss_arena_asteroid_size(i);
        let mut obj = GameObject::new_rect(ctx, id.clone(), None::<Image>,
            (size, size), (-8000.0, -8000.0),
            vec!["hook".into()], (0.0, 0.0), (0.95, 0.95), 0.0);
        obj.gravity = 0.0;
        obj.rotation_momentum = ((i as f32 * 1.7 + 0.3) % 1.0 - 0.5) * 0.006;
        obj.layer = LAYER_SPACE_HOOK;
        obj.collision_mode  = CollisionMode::solid_circle(size * 0.5);
        obj.collision_layer = ASTEROID_COLLISION_LAYER;
        obj.collision_mask  = ASTEROID_COLLISION_LAYER | PLAYER_COLLISION_LAYER;
        obj.visible = false;
        boss_asteroid_ids.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Comet pool ────────────────────────────────────────────────────────
    let mut comet_free: Vec<String> = Vec::new();
    for i in 0..COMET_POOL_SIZE {
        let id = format!("comet_{i}");
        let mut obj = GameObject::new_rect(ctx, id.clone(), None::<Image>,
            (COMET_SIZE, COMET_SIZE), (-9000.0, -9000.0),
            vec!["comet".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.gravity = 0.0; obj.visible = false;
        obj.collision_mode = CollisionMode::NonPlatform; obj.layer = 10;
        comet_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Comet warning pool ─────────────────────────────────────────────────
    let mut warn_free: Vec<String> = Vec::new();
    for i in 0..COMET_WARN_POOL_SIZE {
        let id = format!("comet_warn_{i}");
        let mut obj = GameObject::new_rect(ctx, id.clone(), None::<Image>,
            (COMET_WARN_W, COMET_WARN_H), (-9500.0, -9500.0),
            vec!["comet_warn".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.gravity = 0.0; obj.visible = false;
        obj.collision_mode = CollisionMode::NonPlatform; obj.layer = 11;
        warn_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    let pools = PoolSets {
        starter_names, pool_free, pad_free, spinner_free,
        coin_free, flip_free, score_x2_free, zero_g_free,
        gate_free, gwell_free, turret_free, bullet_free,
        coin_static_sprite, coin_anim_template, score_x2_anim_template,
        tech_bounce_static_img, tech_bounce_static_img_flipped,
        tech_bounce_anim_frames, tech_bounce_anim_frames_flipped,
        pad_thruster_static_img, pad_thruster_anim_template, pad_thruster_anim_template_flipped,
        rocket_pad_free, space_planet_free, space_hook_free,
        space_coin_free, space_blue_coin_free, space_bh_free,
        space_asteroid_free, space_red_coin_free, cannon_free,
        space_oxygen_pickup_free, upgrade_free,
        boss_bolt_free, impact_free, boss_asteroid_ids, comet_free, warn_free,
    };

    (scene, pools)
}
