use crate::constants::*;
use crate::images::circle_cached;
use quartz::{Canvas, Image, ShapeType, SoundOptions, Value};
use quartz::AnimatedSprite;
use std::sync::OnceLock;
use std::io::Cursor;
use image::AnimationDecoder;

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
    Image {
        shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
        image: circle_cached(HOOK_R as u32, r, g, b),
        color: None,
    }
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

pub fn pad_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_PAD_ZONE1,
        2 => C_PAD_ZONE2,
        _ => C_PAD,
    }
}

pub fn pad_hit_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_PAD_HIT_ZONE1,
        2 => C_PAD_HIT_ZONE2,
        _ => C_PAD_HIT,
    }
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
