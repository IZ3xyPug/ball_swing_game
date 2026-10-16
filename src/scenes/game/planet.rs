//! The planet below: where its surface begins, and EARTH's night.
//!
//! The planet itself is baked into the sky picture (`compute_bg_images`,
//! `planet_sky`). Over EARTH (the original photo) this lays a night side: the
//! same photo shaded dark and blue, its cities lit, the air's glow dimmed,
//! faded in as the run goes on (a whole day every `DAY_PX` of travel) and out
//! again. Placed on the sky's own rect every frame, so it zooms, drops for a
//! boss and flips with the sky; by day nothing is drawn over the planet.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;

/// Where the photo's surface begins, as a sky-image row.
pub const HORIZON_ROW: f32 = VH * 0.825;
/// Where the overlay starts (the top of the air's glow), as a sky-image row.
const TOP_ROW: f32 = VH * 0.50;
/// The overlay is built at half the sky's resolution.
const IMG_W: u32 = (VW / 2.0) as u32;
const IMG_H: u32 = ((VH - TOP_ROW) / 2.0) as u32;

/// Travel for one whole day and night, in world px.
pub const DAY_PX: f32 = 90_000.0;

const ID: &str = "planet_night";

// ── Noise (for where the cities are) ─────────────────────────────────────────

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0
}

fn value(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(fx), s(fy));
    let a = hash(xi, yi, seed) + (hash(xi + 1, yi, seed) - hash(xi, yi, seed)) * sx;
    let b = hash(xi, yi + 1, seed) + (hash(xi + 1, yi + 1, seed) - hash(xi, yi + 1, seed)) * sx;
    a + (b - a) * sy
}

fn fbm(x: f32, y: f32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut f, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..4 {
        sum += value(x * f, y * f, seed + o) * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ── Earth's night side ───────────────────────────────────────────────────────

/// The night overlay for a background: Earth's own daytime sky picture,
/// shaded to night (not blacked out: the ground still shows) with its cities
/// on, and over it a darkening that deepens towards the horizon. Clear at
/// its top edge.
pub fn night_image(bg_sel: usize) -> image::RgbaImage {
    let day = crate::scenes::game::build_scene::earth_day_sky(bg_sel);
    // Its rows TOP_ROW..VH, at half size.
    let top_px = TOP_ROW as u32;
    let band = image::imageops::crop_imm(&*day, 0, top_px, day.width(), day.height() - top_px).to_image();
    let band = image::imageops::resize(&band, IMG_W, IMG_H, image::imageops::FilterType::Triangle);
    let top = (TOP_ROW / 2.0) as u32;
    let horizon = HORIZON_ROW / 2.0;
    let full_h = (VH / 2.0) as f32;
    let mut img = image::RgbaImage::new(IMG_W, IMG_H);
    for py in 0..IMG_H {
        let row = (py + top) as f32;
        for px in 0..IMG_W {
            if row < horizon {
                // The air: dimmed, most near the horizon.
                let k = smooth(top as f32, horizon, row);
                img.put_pixel(px, py, image::Rgba([6, 6, 20, (k * 90.0) as u8]));
                continue;
            }
            // The surface: the day's own colours, in shade.
            let p = band.get_pixel(px, py);
            let mut c = [p[0] as f32 * 0.30, p[1] as f32 * 0.34, p[2] as f32 * 0.52];
            // Eased in over the first rows, so the ground meets the dimmed
            // air without a line.
            let ease = smooth(horizon, horizon + 14.0, row);
            let mut alpha = 110.0 + (175.0 - 110.0) * ease;
            // Cities: clustered across the land, finer towards the horizon.
            let depth = ((row - horizon) / (full_h - horizon)).clamp(0.0, 1.0);
            let (u, v) = (px as f32 / IMG_W as f32 * 9.0, depth.powf(1.7) * 5.0);
            let cluster = smooth(0.50, 0.64, fbm(u * 0.9, v * 0.9, 71));
            // Thick cloud (bright by day) hides the lights under it.
            let lum = (p[0] as f32 * 0.2126 + p[1] as f32 * 0.7152 + p[2] as f32 * 0.0722) / 255.0;
            let clear = 1.0 - smooth(0.45, 0.75, lum);
            if cluster > 0.0 && clear > 0.05 && hash(px as i32, py as i32, 5) > 0.955 - 0.04 * cluster {
                let k = (0.6 + 0.4 * cluster) * ease * clear;
                c = [c[0] + 255.0 * k, c[1] + 196.0 * k, c[2] + 120.0 * k];
                alpha = alpha + (255.0 - alpha) * ease * clear;
            }
            img.put_pixel(px, py, image::Rgba([c[0].min(255.0) as u8, c[1].min(255.0) as u8, c[2].min(255.0) as u8, alpha as u8]));
        }
    }
    img
}

/// The overlay per background, built once each, off the main thread (until
/// it is ready there is simply no night yet).
fn night_for(bg: usize) -> Option<Arc<image::RgbaImage>> {
    static READY: OnceLock<Mutex<Vec<(usize, Arc<image::RgbaImage>)>>> = OnceLock::new();
    static BUILDING: Mutex<Vec<usize>> = Mutex::new(Vec::new());
    let ready = READY.get_or_init(|| Mutex::new(Vec::new()));
    if let Some((_, img)) = ready.lock().unwrap().iter().find(|(k, _)| *k == bg) { return Some(img.clone()); }
    let mut b = BUILDING.lock().unwrap();
    if !b.contains(&bg) {
        b.push(bg);
        std::thread::spawn(move || {
            let img = Arc::new(night_image(bg));
            READY.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap().push((bg, img));
        });
    }
    None
}

/// Start building Earth's night ahead of the run.
pub fn prewarm(bg_sel: usize) { let _ = night_for(bg_sel); }

/// The overlay's object, for the scene builder (hidden).
pub fn objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    let mut o = GameObject::new_rect(ctx, ID.to_string(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: crate::images::solid(0, 0, 0, 0).into(), color: None }),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    o.ignore_zoom = true;
    o.visible = false;
    vec![(ID.to_string(), o)]
}

#[derive(Default, Clone)]
pub struct PlanetTick {
    shown: Option<usize>,
    night_q: Option<i32>,
    img: Option<Arc<image::RgbaImage>>,
}

impl PlanetTick {
    /// Re-apply the image (a re-entered scene has blank objects).
    pub fn refresh(&mut self) { self.shown = None; self.night_q = None; }
}

/// How far into the night `px` of travel is: 0 day .. 1 night, resting a
/// while at each end.
pub fn night_amount(px: f32) -> f32 {
    let wave = 0.5 - 0.5 * (px.max(0.0) / DAY_PX * std::f32::consts::TAU).cos();
    smooth(0.2, 0.8, wave)
}

pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>, t: &mut PlanetTick) {
    let (px, dark, flipped, no_sky) = {
        let s = st.lock().unwrap();
        (s.px, s.dark_mode, s.gravity_dir < 0.0, s.in_space_mode || s.space_launch_active)
    };
    // `planet_px_offset`: shifts the day (the headless driver's night shots).
    let night = night_amount(px + c.get_f32("planet_px_offset"));
    let q = (night * 50.0).round() as i32;
    // Earth's alone (the planet the sky is showing right now).
    let earth = c.get_i32("sky_planet_shown") == 0;
    let hidden = !earth || dark || no_sky || c.get_bool("bg_vivid") || q == 0;
    let Some((ox, oy, sc)) = (!hidden).then(|| super::background::sky_picture_rect(c)).flatten() else {
        if let Some(o) = c.get_game_object_mut(ID) { o.visible = false; }
        return;
    };
    let bg_sel = (c.get_i32("player_bg_selected").max(0) as usize).min(crate::shop::SHOP_BG_COLORS.len() - 1);
    if t.shown != Some(bg_sel) || t.img.is_none() {
        t.img = night_for(bg_sel);
        if t.img.is_none() {
            if let Some(o) = c.get_game_object_mut(ID) { o.visible = false; }
            return;
        }
        t.shown = Some(bg_sel);
        t.night_q = None;
    }
    if t.night_q != Some(q) {
        t.night_q = Some(q);
        let a = (q as f32 / 50.0 * 255.0) as u8;
        if let (Some(img), Some(o)) = (t.img.clone(), c.get_game_object_mut(ID)) {
            let (iw, ih) = (img.width() as f32, img.height() as f32);
            o.set_image(Image { shape: ShapeType::Rectangle(0.0, (iw, ih), 0.0), image: img, color: Some(Color(255, 255, 255, a)) });
        }
    }
    // Over the picture's rows TOP_ROW..VH (mirrored when flipped), exactly
    // where the sky draws them, so the night meets the baked horizon.
    let top = if flipped { oy } else { oy + TOP_ROW * sc };
    if let Some(o) = c.get_game_object_mut(ID) {
        o.position = (ox, top);
        o.size = (VW * sc, (VH - TOP_ROW) * sc);
        o.rotation = if flipped { 180.0 } else { 0.0 };
        o.visible = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_begins_in_daylight_and_turns_to_night_halfway() {
        assert!(night_amount(0.0) < 0.01);
        assert!(night_amount(DAY_PX * 0.5) > 0.99);
        assert!(night_amount(DAY_PX) < 0.01);
    }
}
