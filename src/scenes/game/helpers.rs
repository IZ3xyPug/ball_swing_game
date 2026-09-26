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
    c.play_sound_bytes_with(asset, SoundOptions::new().volume(vol));
}

/// Decode a PixelLab PNG to an image scaled to `size`, nearest-neighbour.
///
/// NEAREST, not a smooth filter: these are pixel art generated at 128px and
/// drawn at 150-360px in world space. Any interpolating filter turns the hard
/// pixel edges into mush, which is the one thing that makes generated pixel art
/// look cheap in motion.
pub fn pl_image(bytes: &'static [u8], size: f32) -> Option<image::RgbaImage> {
    let d = size.round().max(2.0) as u32;
    let src = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(image::imageops::resize(
        &src, d, d, image::imageops::FilterType::Nearest))
}

/// `pl_image_cached`, optionally flipped VERTICALLY to swap handedness.
///
/// Vertically, not horizontally, and that is the whole point. The hands are
/// drawn in profile pointing RIGHT because the engine rotates them to aim, so
/// a horizontal flip would make the left hand point LEFT and every aim would
/// come out 180 degrees wrong. Flipping top-to-bottom swaps which hand it
/// looks like while leaving the direction it points alone.
///
/// Flipped in the CACHE rather than at the draw site, so the result keeps a
/// stable `Arc` and the atlas still uploads it once.
pub fn pl_image_cached_mirrored(bytes: &'static [u8], size: f32, mirror: bool)
    -> Option<std::sync::Arc<image::RgbaImage>>
{
    if !mirror {
        return pl_image_cached(bytes, size);
    }
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    static CACHE: OnceLock<Mutex<HashMap<(usize, u32), Option<Arc<image::RgbaImage>>>>> =
        OnceLock::new();
    let key = (bytes.as_ptr() as usize, size.round().max(2.0) as u32);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(g) = cache.lock() {
        if let Some(hit) = g.get(&key) {
            return hit.clone();
        }
    }
    let built = pl_image(bytes, size)
        .map(|img| Arc::new(image::imageops::flip_vertical(&img)));
    if let Ok(mut g) = cache.lock() {
        g.insert(key, built.clone());
    }
    built
}

/// `pl_image` for a NON-SQUARE target, cached by (asset, w, h).
///
/// Trims the sprite to its opaque bounds before resizing. The generated art is
/// drawn on a square canvas with transparent padding, so stretching the whole
/// 128x128 into an 80x30 bolt would squash the padding along with the art and
/// leave the bolt a third of the height it should be.
pub fn pl_image_fit_cached(bytes: &'static [u8], w: f32, h: f32)
    -> Option<std::sync::Arc<image::RgbaImage>>
{
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    static CACHE: OnceLock<Mutex<HashMap<(usize, u32, u32), Option<Arc<image::RgbaImage>>>>> =
        OnceLock::new();
    let (tw, th) = (w.round().max(2.0) as u32, h.round().max(2.0) as u32);
    let key = (bytes.as_ptr() as usize, tw, th);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(g) = cache.lock() {
        if let Some(hit) = g.get(&key) {
            return hit.clone();
        }
    }

    let built = (|| {
        let src = image::load_from_memory(bytes).ok()?.to_rgba8();
        // Opaque bounding box.
        let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
        for (x, y, p) in src.enumerate_pixels() {
            if p.0[3] > 0 {
                x0 = x0.min(x); y0 = y0.min(y);
                x1 = x1.max(x); y1 = y1.max(y);
            }
        }
        if x0 > x1 || y0 > y1 {
            return None; // fully transparent
        }
        let cropped = image::imageops::crop_imm(
            &src, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image();
        Some(Arc::new(image::imageops::resize(
            &cropped, tw, th, image::imageops::FilterType::Nearest)))
    })();

    if let Ok(mut g) = cache.lock() {
        g.insert(key, built.clone());
    }
    built
}

/// `pl_image`, cached by (asset, size).
///
/// The serpent rebuilds every piece's image inside its draw loop, so an
/// uncached decode-and-resize would run a PNG decode per segment per frame.
/// Keyed on the slice's ADDRESS rather than its contents: these are all
/// `include_bytes!` statics, so the pointer is a stable identity and hashing
/// 7KB of PNG per lookup would cost more than the decode saved.
/// Returns an `Arc`, and the same `Arc` for the same (asset, size) every time.
///
/// That matters beyond avoiding the decode: the renderer's texture atlas is
/// keyed by `Arc::as_ptr`, so handing back a fresh allocation each call would
/// re-upload the texture to the GPU every frame. A stable pointer means one
/// upload for the life of the process.
pub fn pl_image_cached(bytes: &'static [u8], size: f32)
    -> Option<std::sync::Arc<image::RgbaImage>>
{
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    static CACHE: OnceLock<Mutex<HashMap<(usize, u32), Option<Arc<image::RgbaImage>>>>> =
        OnceLock::new();
    let key = (bytes.as_ptr() as usize, size.round().max(2.0) as u32);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    // Decode OUTSIDE the lock. Holding it across the resize would serialise
    // every caller behind the first one, and this runs in a draw loop.
    if let Ok(g) = cache.lock() {
        if let Some(hit) = g.get(&key) {
            return hit.clone();
        }
    }
    let built = pl_image(bytes, size).map(Arc::new);
    if let Ok(mut g) = cache.lock() {
        g.insert(key, built.clone());
    }
    built
}

/// Build an `AnimatedSprite` from PixelLab's numbered frame PNGs.
///
/// Returns `None` if ANY frame fails to decode, rather than a short loop: a
/// sprite quietly missing three of its nine frames animates at the wrong speed
/// and looks like a timing bug rather than a missing asset.
pub fn pl_sprite(frames: &[&'static [u8]], size: f32, fps: f32) -> Option<AnimatedSprite> {
    let decoded: Vec<image::RgbaImage> =
        frames.iter().filter_map(|b| pl_image(b, size)).collect();
    if decoded.len() != frames.len() {
        return None;
    }
    Some(AnimatedSprite::from_frames(decoded, (size, size), fps))
}

/// Play the player's impact sound, at most once every few ticks.
///
/// This is the sound that used to fire on every hook GRAB. Moving it to real
/// impacts is the difference between a noise that marks something happening
/// and one that marks the player doing the thing the game is about.
pub fn play_impact_sfx(c: &mut Canvas, st: &std::sync::Arc<std::sync::Mutex<crate::state::State>>) {
    {
        let mut s = st.lock().unwrap();
        if s.impact_sfx_cd > 0 {
            return;
        }
        s.impact_sfx_cd = IMPACT_SFX_COOLDOWN;
    }
    let vol = sfx_vol(c, IMPACT_SFX_VOL);
    c.play_sound_bytes_with(ASSET_CARTOON_CAT, SoundOptions::new().volume(vol));
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
    // Embedded, not read from disk: the old path was the build machine's
    // absolute path, which crashes on any other device.
    let d = (HOOK_ARTIFACT_R * 2.0).round() as u32;
    let cursor = Cursor::new(ASSET_HOOK_ARTIFACT_GIF);
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
    let d = 256u32; // ZeroG.gif is natively 256×256
    let cursor = Cursor::new(ASSET_ZERO_G_GIF);
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
    let d = (HOOK_ARTIFACT_R * 2.0).round() as u32;
    let cursor = Cursor::new(ASSET_HOOK_ARTIFACT_GREEN_GIF);
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

/// Keep the `warp_flash` overlay centred on the player each frame so the
/// speed-line / wormhole origin tracks the ball, rather than sitting at the
/// middle of the screen. Only repositions (leaves the sprite size alone, which
/// is set per-effect). Read the player centre first, then mutate the overlay.
pub fn center_warp_on_player(c: &mut Canvas) {
    let Some(p) = c.get_game_object("player") else { return; };
    let cx = p.position.0 + p.size.0 * 0.5;
    let cy = p.position.1 + p.size.1 * 0.5;
    if let Some(obj) = c.get_game_object_mut("warp_flash") {
        obj.position = (cx - obj.size.0 * 0.5, cy - obj.size.1 * 0.5);
    }
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
