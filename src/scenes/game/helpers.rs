use crate::constants::*;
use crate::images::circle_cached;
use crate::objects::ui_text_spec;
use quartz::{Canvas, Color, Font, GameObject, Image, ShapeType, SoundOptions, Value};
use quartz::AnimatedSprite;
use std::sync::OnceLock;
use std::io::Cursor;
use image::AnimationDecoder;

/// Find objects in `live` within pickup range of the player.
/// collect_r = PLAYER_R + min(obj_w, obj_h)/2 + 10. Uses the engine's `objects_in_radius` API.
pub fn find_collected_pickups(c: &Canvas, live: &[String], obj_w: f32, obj_h: f32) -> Vec<String> {
    let player = match c.get_game_object("player") { Some(p) => p, None => return vec![] };
    let collect_r = PLAYER_R + (obj_w.min(obj_h)) * 0.5 + 10.0;
    c.objects_in_radius(player, collect_r).into_iter()
        .filter(|o| live.contains(&o.id))
        .map(|o| o.id.clone())
        .collect()
}

/// Play the currently-selected death sound. Call before any load_scene("gameover*").
/// death_sound_mode: 0 = man_game_over (default), 1 = arcade_game_over.
pub fn play_death_sound(c: &mut Canvas) {
    let mode = match c.get_var("death_sound_mode") {
        Some(Value::I32(v)) => v,
        _ => 0,
    };
    let asset = if mode == 1 { ASSET_ARCADE_GAME_OVER } else { ASSET_WOBBLY_MEOW };
    let vol = sfx_vol(c, 0.65);
    c.play_sound_with(asset, SoundOptions::new().volume(vol));
}

/// Compute effective SFX volume: base * vol_master * vol_sound.
pub fn sfx_vol(c: &Canvas, base: f32) -> f32 {
    let master = match c.get_var("vol_master") {
        Some(Value::F32(v)) => v.clamp(0.0, 1.0),
        _ => 1.0,
    };
    let sound = match c.get_var("vol_sound") {
        Some(Value::F32(v)) => v.clamp(0.0, 1.0),
        _ => 1.0,
    };
    (base * master * sound).clamp(0.0, 1.0)
}

/// Hook image using circle_cached — keeps hooks in the same
/// render batch as other Rectangle objects to avoid z-order artifacts.
pub fn hook_img(r: u8, g: u8, b: u8) -> Image {
    Image { shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0), image: circle_cached(HOOK_R as u32, r, g, b), color: None }
}

/// Cached decoded + resized GIF frames (decoded once, cloned cheaply on each spawn).
static HOOK_ARTIFACT_FRAMES: OnceLock<Vec<image::RgbaImage>> = OnceLock::new();

fn decode_hook_artifact_frames() -> Vec<image::RgbaImage> {
    let bytes = std::fs::read(ASSET_HOOK_ARTIFACT_GIF).expect("hook_artifact.gif missing");
    let d = (HOOK_ARTIFACT_R * 2.0).round() as u32;
    let cursor = Cursor::new(bytes);
    if let Ok(decoder) = image::codecs::gif::GifDecoder::new(cursor) {
        let frames: Vec<image::RgbaImage> = decoder.into_frames()
            .filter_map(|f| f.ok())
            .map(|f| {
                let buf = f.into_buffer();
                let (w, h) = (buf.width(), buf.height());
                if w == d && h == d { return buf; }
                let scale = (d as f32 / w as f32).min(d as f32 / h as f32);
                let rw = (w as f32 * scale).round().max(1.0) as u32;
                let rh = (h as f32 * scale).round().max(1.0) as u32;
                let resized = image::imageops::resize(&buf, rw, rh, image::imageops::FilterType::Nearest);
                let ox = ((d.saturating_sub(rw)) / 2) as i64;
                let oy = ((d.saturating_sub(rh)) / 2) as i64;
                let mut canvas = image::RgbaImage::from_pixel(d, d, image::Rgba([0, 0, 0, 0]));
                image::imageops::overlay(&mut canvas, &resized, ox, oy);
                canvas
            })
            .collect();
        if !frames.is_empty() { return frames; }
    }
    vec![image::RgbaImage::from_pixel(d, d, image::Rgba([200, 200, 200, 255]))]
}

/// Prewarm the artifact frame cache (call from a background thread at startup).
pub fn prewarm_hook_artifact() {
    HOOK_ARTIFACT_FRAMES.get_or_init(decode_hook_artifact_frames);
}

/// Returns an AnimatedSprite for the hook artifact GIF, frozen at frame 0.
/// Call `sprite.reset(); sprite.set_fps(HOOK_ARTIFACT_FPS)` to play it on grab.
pub fn hook_artifact_anim() -> AnimatedSprite {
    let d = HOOK_ARTIFACT_R * 2.0;
    let size = (d, d);
    // Clone cached frames — much cheaper than re-decoding from disk each time.
    let frames = HOOK_ARTIFACT_FRAMES.get_or_init(decode_hook_artifact_frames).clone();
    let mut anim = AnimatedSprite::from_frames(frames, size, HOOK_ARTIFACT_FPS);
    anim.set_fps(0.001);
    anim
}

// ── Green artifact hook (special hook gif) ──────────────────────────────────

// ── Zero-G overlay ─────────────────────────────────────────────────────────
static ZERO_G_OVERLAY_FRAMES: OnceLock<Vec<image::RgbaImage>> = OnceLock::new();
fn decode_zero_g_overlay_frames() -> Vec<image::RgbaImage> {
    let bytes = std::fs::read(ASSET_ZERO_G_GIF).expect("ZeroG.gif missing");
    let d = 256u32; // ZeroG.gif is natively 256×256
    let cursor = Cursor::new(bytes);
    if let Ok(decoder) = image::codecs::gif::GifDecoder::new(cursor) {
        let frames: Vec<image::RgbaImage> = decoder.into_frames()
            .filter_map(|f| f.ok())
            .map(|f| {
                let buf = f.into_buffer();
                let (w, h) = (buf.width(), buf.height());
                if w == d && h == d { return buf; }
                let scale = (d as f32 / w as f32).min(d as f32 / h as f32);
                let rw = (w as f32 * scale).round().max(1.0) as u32;
                let rh = (h as f32 * scale).round().max(1.0) as u32;
                let resized = image::imageops::resize(&buf, rw, rh, image::imageops::FilterType::Nearest);
                let ox = ((d.saturating_sub(rw)) / 2) as i64;
                let oy = ((d.saturating_sub(rh)) / 2) as i64;
                let mut canvas = image::RgbaImage::from_pixel(d, d, image::Rgba([0, 0, 0, 0]));
                image::imageops::overlay(&mut canvas, &resized, ox, oy);
                canvas
            })
            .collect();
        if !frames.is_empty() { return frames; }
    }
    vec![image::RgbaImage::from_pixel(d, d, image::Rgba([135, 220, 255, 180]))]
}
pub fn prewarm_zero_g_overlay() {
    ZERO_G_OVERLAY_FRAMES.get_or_init(decode_zero_g_overlay_frames);
}
pub fn zero_g_overlay_anim() -> AnimatedSprite {
    let size = (256.0, 256.0); // matches native 256×256 GIF
    let frames = ZERO_G_OVERLAY_FRAMES.get_or_init(decode_zero_g_overlay_frames).clone();
    let mut anim = AnimatedSprite::from_frames(frames, size, 16.0);
    anim.set_fps(0.001); // frozen until activated
    anim
}

static HOOK_ARTIFACT_GREEN_FRAMES: OnceLock<Vec<image::RgbaImage>> = OnceLock::new();

fn decode_hook_artifact_green_frames() -> Vec<image::RgbaImage> {
    let bytes = std::fs::read(ASSET_HOOK_ARTIFACT_GREEN_GIF).expect("hook_artifact_green.gif missing");
    let d = (HOOK_ARTIFACT_R * 2.0).round() as u32;
    let cursor = Cursor::new(bytes);
    if let Ok(decoder) = image::codecs::gif::GifDecoder::new(cursor) {
        let frames: Vec<image::RgbaImage> = decoder.into_frames()
            .filter_map(|f| f.ok())
            .map(|f| {
                let buf = f.into_buffer();
                let (w, h) = (buf.width(), buf.height());
                if w == d && h == d { return buf; }
                let scale = (d as f32 / w as f32).min(d as f32 / h as f32);
                let rw = (w as f32 * scale).round().max(1.0) as u32;
                let rh = (h as f32 * scale).round().max(1.0) as u32;
                let resized = image::imageops::resize(&buf, rw, rh, image::imageops::FilterType::Nearest);
                let ox = ((d.saturating_sub(rw)) / 2) as i64;
                let oy = ((d.saturating_sub(rh)) / 2) as i64;
                let mut canvas = image::RgbaImage::from_pixel(d, d, image::Rgba([0, 0, 0, 0]));
                image::imageops::overlay(&mut canvas, &resized, ox, oy);
                canvas
            })
            .collect();
        if !frames.is_empty() { return frames; }
    }
    vec![image::RgbaImage::from_pixel(d, d, image::Rgba([52, 196, 84, 255]))]
}

pub fn prewarm_hook_artifact_green() {
    HOOK_ARTIFACT_GREEN_FRAMES.get_or_init(decode_hook_artifact_green_frames);
}

/// Returns an AnimatedSprite for the green hook artifact GIF, frozen at frame 0.
pub fn hook_artifact_green_anim() -> AnimatedSprite {
    let d = HOOK_ARTIFACT_R * 2.0;
    let size = (d, d);
    let frames = HOOK_ARTIFACT_GREEN_FRAMES.get_or_init(decode_hook_artifact_green_frames).clone();
    let mut anim = AnimatedSprite::from_frames(frames, size, HOOK_ARTIFACT_FPS);
    anim.set_fps(0.001);
    anim
}

pub fn hook_base_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_HOOK_ZONE1,
        2 => C_HOOK_ZONE2,
        _ => C_HOOK,
    }
}

pub fn hook_near_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_HOOK_NEAR_ZONE1,
        2 => C_HOOK_NEAR_ZONE2,
        _ => C_HOOK_NEAR,
    }
}

pub fn hook_on_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_HOOK_ON_ZONE1,
        2 => C_HOOK_ON_ZONE2,
        _ => C_HOOK_ON,
    }
}

#[inline]
pub fn is_special_hook_obj(obj: &GameObject) -> bool {
    obj.tags.iter().any(|t| t == SPECIAL_HOOK_TAG)
}

#[inline]
pub fn is_extended_hook_obj(obj: &GameObject) -> bool {
    obj.tags.iter().any(|t| t == EXTENDED_HOOK_TAG)
}

#[inline]
pub fn hook_base_for_obj(obj: &GameObject, zone_idx: usize) -> (u8, u8, u8) {
    if is_extended_hook_obj(obj) {
        C_HOOK_EXTENDED
    } else if is_special_hook_obj(obj) {
        C_HOOK_SPECIAL
    } else {
        hook_base_for_zone(zone_idx)
    }
}

#[inline]
pub fn hook_near_for_obj(obj: &GameObject, zone_idx: usize) -> (u8, u8, u8) {
    if is_extended_hook_obj(obj) {
        C_HOOK_EXTENDED_NEAR
    } else if is_special_hook_obj(obj) {
        C_HOOK_SPECIAL_NEAR
    } else {
        hook_near_for_zone(zone_idx)
    }
}

#[inline]
pub fn hook_on_for_obj(obj: &GameObject, zone_idx: usize) -> (u8, u8, u8) {
    if is_extended_hook_obj(obj) {
        C_HOOK_EXTENDED_ON
    } else if is_special_hook_obj(obj) {
        C_HOOK_SPECIAL_ON
    } else {
        hook_on_for_zone(zone_idx)
    }
}

/// Circle/rounded-rectangle overlap using signed-distance math.
/// Rectangle position is top-left (x, y) with size (w, h).
#[inline]
pub fn circle_overlaps_rounded_rect(
    cx: f32,
    cy: f32,
    circle_r: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    corner_r: f32,
) -> bool {
    if w <= 0.0 || h <= 0.0 || circle_r < 0.0 {
        return false;
    }

    let rr = corner_r.clamp(0.0, 0.5 * w.min(h));
    let rcx = x + w * 0.5;
    let rcy = y + h * 0.5;
    let qx = (cx - rcx).abs() - (w * 0.5 - rr);
    let qy = (cy - rcy).abs() - (h * 0.5 - rr);
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    let outside = (ox * ox + oy * oy).sqrt();
    let inside = qx.max(qy).min(0.0);
    let signed_dist = outside + inside - rr;
    signed_dist <= circle_r
}

pub fn spinner_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_SPINNER_ZONE1,
        2 => C_SPINNER_ZONE2,
        _ => C_SPINNER,
    }
}

#[inline]
pub fn pad_thruster_id(pad_id: &str) -> String {
    format!("{pad_id}_thruster")
}

// ── Comet warning images ──────────────────────────────────────────────────────
// Each image is decoded once (OnceLock) and cloned cheaply on each use.

fn load_warn_rgba(bytes: &[u8]) -> std::sync::Arc<image::RgbaImage> {
    let src = image::load_from_memory(bytes)
        .expect("warn image decode failed")
        .to_rgba8();
    let resized = image::imageops::resize(
        &src,
        COMET_WARN_W as u32,
        COMET_WARN_H as u32,
        image::imageops::FilterType::Lanczos3,
    );
    std::sync::Arc::new(resized)
}

macro_rules! warn_img_accessor {
    ($fn_name:ident, $lock_name:ident, $bytes:expr) => {
        static $lock_name: OnceLock<Image> = OnceLock::new();
        pub fn $fn_name() -> Image {
            $lock_name.get_or_init(|| {
                let rgba = load_warn_rgba($bytes);
                Image { shape: ShapeType::Rectangle(0.0, (COMET_WARN_W, COMET_WARN_H), 0.0), image: rgba, color: None }
            }).clone()
        }
    };
}

warn_img_accessor!(
    warn_img_dark,
    WARN_IMG_DARK,
    include_bytes!("../../../assets/exclamation_dark.webp")
);
warn_img_accessor!(
    warn_img_light,
    WARN_IMG_LIGHT,
    include_bytes!("../../../assets/exclamation_light.webp")
);
warn_img_accessor!(
    warn_img_dark_explode,
    WARN_IMG_DARK_EXPLODE,
    include_bytes!("../../../assets/exclamation_dark_explode.webp")
);
warn_img_accessor!(
    warn_img_light_explode,
    WARN_IMG_LIGHT_EXPLODE,
    include_bytes!("../../../assets/exclamation_light_explode.webp")
);

// ── Shared volume / audio helpers (used by menu.rs + build_scene.rs) ─────────

pub fn volume_value(c: &Canvas, var: &str, default: f32) -> f32 {
    match c.get_var(var) {
        Some(Value::F32(v)) => v.clamp(0.0, 1.0),
        _ => default,
    }
}

pub fn set_volume_value(c: &mut Canvas, var: &str, v: f32) {
    c.set_var(var, v.clamp(0.0, 1.0));
}

pub fn music_volume(c: &Canvas, base: f32) -> f32 {
    let master = volume_value(c, "vol_master", 1.0);
    let music  = volume_value(c, "vol_music",  1.0);
    (base * master * music).clamp(0.0, 1.0)
}

/// Returns true when the game is either engine-paused or has the game_paused var set.
#[inline]
pub fn is_game_paused(c: &Canvas) -> bool {
    c.is_paused() || matches!(c.get_var("game_paused"), Some(Value::Bool(true)))
}

/// UI font — parsed from font.ttf once, cloned cheaply on all subsequent calls.
pub fn ui_font() -> Option<Font> {
    static CACHED: OnceLock<Font> = OnceLock::new();
    CACHED.get_or_init(||
        Font::from_bytes(include_bytes!("../../../assets/font.ttf"))
            .expect("font.ttf must be valid")
    ).clone().into()
}

// ── Shared volume slider / label helpers ─────────────────────────────────────
// Settings panels in both the game scene and menu settings scene share identical
// slider geometry and label formatting. These helpers live here to avoid the
// duplication.

/// Position 3 volume slider thumbs given their object names.
/// Geometry matches both the game settings panel and the menu settings panel.
pub fn position_volume_sliders(c: &mut Canvas, thumb_names: [&str; 3]) {
    const TRACK_W: f32 = 1400.0;
    const THUMB_W: f32 = 60.0;
    const THUMB_H: f32 = 80.0;
    const TRACK_H: f32 = 24.0;
    const TRACK_X: f32 = (VW - TRACK_W) / 2.0;
    const Y: [f32; 3] = [820.0, 1120.0, 1420.0];
    const VARS: [&str; 3] = ["vol_master", "vol_music", "vol_sound"];
    for i in 0..3 {
        let vol = volume_value(c, VARS[i], 1.0);
        let thumb_x = TRACK_X + vol * (TRACK_W - THUMB_W);
        let thumb_y = Y[i] - (THUMB_H - TRACK_H) / 2.0;
        if let Some(obj) = c.get_game_object_mut(thumb_names[i]) {
            obj.position = (thumb_x, thumb_y);
        }
    }
}

/// Update 3 volume percentage labels given their object names.
pub fn update_volume_labels(c: &mut Canvas, label_names: [&str; 3]) {
    let master = volume_value(c, "vol_master", 1.0);
    let music  = volume_value(c, "vol_music",  1.0);
    let sound  = volume_value(c, "vol_sound",  1.0);
    let labels = [
        format!("MASTER VOLUME   {:>3}%", (master * 100.0).round() as i32),
        format!("MUSIC VOLUME    {:>3}%",  (music  * 100.0).round() as i32),
        format!("SOUND VOLUME    {:>3}%",  (sound  * 100.0).round() as i32),
    ];
    if let Some(font) = ui_font() {
        let s = c.virtual_scale();
        for i in 0..3 {
            if let Some(obj) = c.get_game_object_mut(label_names[i]) {
                obj.set_drawable(Box::new(ui_text_spec(
                    &labels[i], &font, 38.0 * s, Color(235, 245, 255, 255), 1500.0 * s,
                )));
            }
        }
    }
}
