use crate::constants::*;
use crate::images::{circle_cached, asteroid_hook_image_cached};
use quartz::{Image, ShapeType};
use std::sync::OnceLock;

/// Hook image using circle_cached — keeps hooks in the same
/// render batch as other Rectangle objects to avoid z-order artifacts.
pub fn hook_img(r: u8, g: u8, b: u8) -> Image {
    Image {
        shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
        image: circle_cached(HOOK_R as u32, r, g, b),
        color: None,
    }
}

#[derive(Copy, Clone)]
pub enum AsteroidHookState {
    Base,
    Near,
    On,
}

fn asteroid_bucket_for_id(id: &str) -> usize {
    let mut n: u32 = 0;
    let mut any = false;
    for b in id.bytes() {
        if b.is_ascii_digit() {
            any = true;
            n = n.wrapping_mul(10).wrapping_add((b - b'0') as u32);
        }
    }
    if any { (n % 3) as usize } else { 0 }
}

fn asteroid_scale_for_bucket(bucket: usize) -> f32 {
    match bucket {
        0 => 1.50, // small (base hook size)
        1 => 1.90, // medium
        _ => 2.25, // big
    }
}

fn asteroid_state_idx(state: AsteroidHookState) -> usize {
    match state {
        AsteroidHookState::Base => 0,
        AsteroidHookState::Near => 1,
        AsteroidHookState::On => 2,
    }
}

fn tint_asteroid_pixels(mut img: image::RgbaImage, state: AsteroidHookState) -> image::RgbaImage {
    let (mul, add_r, add_g, add_b) = match state {
        AsteroidHookState::Base => (1.00, 0.0, 0.0, 0.0),
        AsteroidHookState::Near => (1.12, 16.0, 12.0, 3.0),
        AsteroidHookState::On => (1.25, 34.0, 24.0, 6.0),
    };
    for px in img.pixels_mut() {
        if px[3] == 0 {
            continue;
        }
        let r = (px[0] as f32 * mul + add_r).min(255.0);
        let g = (px[1] as f32 * mul + add_g).min(255.0);
        let b = (px[2] as f32 * mul + add_b).min(255.0);
        px[0] = r as u8;
        px[1] = g as u8;
        px[2] = b as u8;
    }
    img
}

fn build_asteroid_variant(scale: f32, state: AsteroidHookState) -> image::RgbaImage {
    let base = asteroid_hook_image_cached();
    let (w, h) = base.dimensions();
    let zw = ((w as f32 * scale).round() as u32).max(w);
    let zh = ((h as f32 * scale).round() as u32).max(h);
    let zoomed = image::imageops::resize(
        base.as_ref(),
        zw,
        zh,
        image::imageops::FilterType::Lanczos3,
    );
    let x0 = (zw - w) / 2;
    let y0 = (zh - h) / 2;
    let cropped = image::imageops::crop_imm(&zoomed, x0, y0, w, h).to_image();
    tint_asteroid_pixels(cropped, state)
}

/// Asteroid hook image with deterministic small/medium/big variants by id
/// and pixel-only highlight variants for near/grab states.
pub fn hook_asteroid_img_for_id(id: &str, state: AsteroidHookState) -> Image {
    type Cache = [[std::sync::Arc<image::RgbaImage>; 3]; 3];
    static CACHE: OnceLock<Cache> = OnceLock::new();

    let cache = CACHE.get_or_init(|| {
        std::array::from_fn(|bucket| {
            let scale = asteroid_scale_for_bucket(bucket);
            [
                std::sync::Arc::new(build_asteroid_variant(scale, AsteroidHookState::Base)),
                std::sync::Arc::new(build_asteroid_variant(scale, AsteroidHookState::Near)),
                std::sync::Arc::new(build_asteroid_variant(scale, AsteroidHookState::On)),
            ]
        })
    });

    let bucket = asteroid_bucket_for_id(id);
    let state_idx = asteroid_state_idx(state);
    Image {
        // Ellipse mask avoids square-edge highlighting artifacts.
        shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
        image: cache[bucket][state_idx].clone(),
        color: None,
    }
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
