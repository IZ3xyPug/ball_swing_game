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
    // Arcs, handed to the sky object as they are: the renderer keys its
    // textures by the Arc, so the same one is uploaded once a session, not
    // a fresh 33 MB copy (twice) at the start of every run.
    vivid:         Arc<image::RgbaImage>,
    vivid_flip:    Arc<image::RgbaImage>,
    palettes:      Vec<Arc<image::RgbaImage>>,
    palettes_flip: Vec<Arc<image::RgbaImage>>,
    space:         Arc<image::RgbaImage>,
    transparent_stars: Arc<image::RgbaImage>,
}

// SAFETY: image::RgbaImage and Arc<image::RgbaImage> are Send + Sync.
unsafe impl Send for BgImages {}
unsafe impl Sync for BgImages {}

static BG_IMAGES: OnceLock<BgImages> = OnceLock::new();

fn compute_bg_images() -> BgImages {
    let t0 = std::time::Instant::now();
    let imgs = compute_bg_images_inner();
    if std::env::var("START_PROFILE").is_ok() { eprintln!("BG_IMAGES built in {:?}", t0.elapsed()); }
    imgs
}

fn compute_bg_images_inner() -> BgImages {
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

    // Earth's surface, as a planet: the photo's clouds over an ocean and land
    // (the photo alone is all cloud, and pulled to the sky's colour it read
    // as fog).
    let earth_surface = earth_surface(&grad_start);
    // One per background colour, built side by side (each is a full-size
    // picture; one after another they took most of the ~1.6 s this took).
    let palettes: Vec<image::RgbaImage> = std::thread::scope(|scope| {
        let earth_surface = &earth_surface;
        let grad_start = &grad_start;
        let handles: Vec<_> = SHOP_BG_COLORS.iter().map(|&(pr, pg, pb)| scope.spawn(move || {
        let mut tinted = grad_start.clone();
        let hz = (bg_h as f32 * EARTH_HORIZON) as u32;
        for (x, y, px) in tinted.enumerate_pixels_mut() {
            // The air keeps the sky's strong tint, as it always had; the
            // ground only a little of it, so it keeps its own colours.
            let ground = earth_surface.get_pixel(x, y);
            let g = ((y as f32 - (hz as f32 - 8.0)) / 14.0).clamp(0.0, 1.0);
            let air = [px[0] as f32 * 0.30 + pr as f32 * 0.70, px[1] as f32 * 0.30 + pg as f32 * 0.70, px[2] as f32 * 0.30 + pb as f32 * 0.70];
            let land = [ground[0] as f32 * 0.78 + pr as f32 * 0.22, ground[1] as f32 * 0.78 + pg as f32 * 0.22, ground[2] as f32 * 0.78 + pb as f32 * 0.22];
            for i in 0..3 { px[i] = (air[i] + (land[i] - air[i]) * g).min(255.0) as u8; }
        }
        composite_starfield_gradient(sf, &tinted, bg_w, bg_h, blend_h)
        })).collect();
        handles.into_iter().map(|h| h.join().expect("palette")).collect()
    });
    let palettes_flip: Vec<Arc<image::RgbaImage>> =
        palettes.iter().map(|img| Arc::new(flip_image_vertical(img))).collect();
    let palettes: Vec<Arc<image::RgbaImage>> = palettes.into_iter().map(Arc::new).collect();

    let space = starfield.image.clone();

    // The moving stars, transparent between them, and faded out over the
    // planet: these panels cover the whole sky, so their stars were showing
    // on the planet's ground as if through gaps in it.
    let transparent_stars = {
        let mut img: image::RgbaImage = (*sf).clone();
        let (fade0, fade1) = (bg_h as f32 * 0.62, bg_h as f32 * 0.78);
        for (_, y, pixel) in img.enumerate_pixels_mut() {
            if pixel[0] < 20 && pixel[1] < 20 && pixel[2] < 25 {
                pixel[3] = 0;
            }
            let k = 1.0 - ((y as f32 - fade0) / (fade1 - fade0)).clamp(0.0, 1.0);
            pixel[3] = (pixel[3] as f32 * k) as u8;
        }
        Arc::new(img)
    };

    let vivid_flip = Arc::new(flip_image_vertical(&vivid));
    let vivid = Arc::new(vivid);

    BgImages { vivid, vivid_flip, palettes, palettes_flip, space, transparent_stars }
}

/// Earth's daytime sky picture for a background (Earth's night is built
/// from it, so the two always match).
pub fn earth_day_sky(bg_sel: usize) -> Arc<image::RgbaImage> {
    let imgs = BG_IMAGES.get_or_init(compute_bg_images);
    imgs.palettes[bg_sel.min(imgs.palettes.len() - 1)].clone()
}

/// Where the photo's ground begins (a share of the sky picture's height).
const EARTH_HORIZON: f32 = 0.825;

/// Earth's ground for the sky picture (rows above the horizon are left
/// black, unused): the photo's own clouds, white and lit, over a deep blue
/// ocean with continents on it, laid out in perspective (a camera above a
/// plane, as the horizon photo was taken), dimming with distance into haze.
fn earth_surface(photo: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = photo.dimensions();
    let hz = h as f32 * EARTH_HORIZON;
    // The photo is an upscaled GIF: blur away its palette steps before
    // reading cloud from it, or every cloud edge is a staircase.
    let photo = image::imageops::blur(photo, 5.0);
    let y0 = (hz as u32).saturating_sub(10);
    // Rows are independent: split them across the cores.
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()) as u32;
    let per = ((h - y0) + threads - 1) / threads;
    let rows: Vec<(u32, Vec<[u8; 4]>)> = std::thread::scope(|scope| {
        let photo = &photo;
        let handles: Vec<_> = (0..threads).map(|t| scope.spawn(move || {
            ((y0 + t * per)..(y0 + (t + 1) * per).min(h))
                .map(|y| (y, earth_surface_row(photo, y, hz)))
                .collect::<Vec<_>>()
        })).collect();
        handles.into_iter().flat_map(|h| h.join().expect("earth rows")).collect()
    });
    let mut out = image::RgbaImage::new(w, h);
    for (y, row) in rows {
        for (x, p) in row.into_iter().enumerate() { out.put_pixel(x as u32, y, image::Rgba(p)); }
    }
    out
}

/// One row of Earth's ground (see `earth_surface`).
fn earth_surface_row(photo: &image::RgbaImage, y: u32, hz: f32) -> Vec<[u8; 4]> {
    let (w, h) = photo.dimensions();
    let hash = |x: i32, y: i32, s: u32| -> f32 {
        let mut v = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ s.wrapping_mul(0xCB1A_B31F);
        v ^= v >> 13; v = v.wrapping_mul(0x5bd1_e995); v ^= v >> 15;
        (v & 0xFFFF) as f32 / 65535.0
    };
    let noise = |x: f32, y: f32, s: u32| -> f32 {
        let (xi, yi) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - x.floor(), y - y.floor());
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let a = hash(xi, yi, s) + (hash(xi + 1, yi, s) - hash(xi, yi, s)) * sx;
        let b = hash(xi, yi + 1, s) + (hash(xi + 1, yi + 1, s) - hash(xi, yi + 1, s)) * sx;
        a + (b - a) * sy
    };
    // Octaves finer than a pixel of ground there are left out (no shimmer).
    let fbm = |x: f32, y: f32, s: u32, fp: f32| -> f32 {
        let (mut sum, mut amp, mut f, mut norm) = (0.0, 0.5, 1.0, 0.0);
        for o in 0..6 {
            if 1.0 / f < fp * 2.0 { break; }
            sum += noise(x * f, y * f, s + o) * amp; norm += amp; amp *= 0.5; f *= 2.03;
        }
        if norm > 0.0 { sum / norm } else { 0.5 }
    };
    let below = (y as f32 + 0.5 - hz).max(0.35);
    let z = h as f32 / below;
    let fp = (z * z / h as f32).max(z / h as f32);
    let haze = 1.0 - (-z * 0.004).exp();
    (0..w).map(|x| {
        let gx = (x as f32 + 0.5 - w as f32 * 0.5) * z / h as f32;
        // Continents: low-frequency shapes, coasts broken up.
        let land = fbm(gx * 0.03 + 11.0, z * 0.03, 7, fp * 0.03);
        // Not out at the horizon: there a coast is thinner than a pixel and
        // came out as stray streaks.
        let far = ((z - 25.0) / 40.0).clamp(0.0, 1.0);
        let is_land = ((land - 0.50) / 0.03).clamp(0.0, 1.0) * (1.0 - far);
        let veg = fbm(gx * 0.3, z * 0.3, 8, fp * 0.3);
        let ocean = [10.0 + 14.0 * veg, 38.0 + 22.0 * veg, 84.0 + 34.0 * veg];
        let terrain = [52.0 + 70.0 * veg, 74.0 + 50.0 * veg, 44.0 + 22.0 * veg];
        let mut c = [0.0f32; 3];
        for i in 0..3 { c[i] = ocean[i] + (terrain[i] - ocean[i]) * is_land; }
        // The photo's clouds over it: its bright parts are cloud tops.
        let p = photo.get_pixel(x, y);
        let l = (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0;
        let cloud = ((l - 0.68) / 0.24).clamp(0.0, 1.0);
        let cloud = cloud * cloud * (3.0 - 2.0 * cloud);
        let white = [235.0 * l + 20.0, 240.0 * l + 18.0, 250.0 * l + 12.0];
        for i in 0..3 { c[i] += (white[i] - c[i]) * cloud; }
        // Far off, the air thickens over it.
        let hz_col = [150.0, 175.0, 215.0];
        for i in 0..3 { c[i] += (hz_col[i] - c[i]) * haze * 0.85; }
        [c[0].min(255.0) as u8, c[1].min(255.0) as u8, c[2].min(255.0) as u8, 255]
    }).collect()
}

/// The sky with a SKY planet other than Earth (and its flip): the planet's
/// picture laid in where the original photo is, pulled only lightly towards
/// the background's colour. Only the current one is kept (each is a
/// full-size sky).
type SkyPair = Arc<(Arc<image::RgbaImage>, Arc<image::RgbaImage>)>;
fn planet_sky(planet: usize, bg_sel: usize) -> SkyPair {
    static CURRENT: OnceLock<std::sync::Mutex<Option<((usize, usize), SkyPair)>>> = OnceLock::new();
    let cur = CURRENT.get_or_init(|| std::sync::Mutex::new(None));
    if let Some((k, p)) = cur.lock().unwrap().as_ref() {
        if *k == (planet, bg_sel) { return p.clone(); }
    }
    let imgs = BG_IMAGES.get_or_init(compute_bg_images);
    let (bg_w, bg_h) = (VW as u32, VH as u32);
    let photo = crate::sky::planet_photo(planet);
    // The new worlds are made at the sky's own size: no resample.
    let mut grad = if photo.dimensions() == (bg_w, bg_h) { photo }
        else { image::imageops::resize(&photo, bg_w, bg_h, image::imageops::FilterType::Triangle) };
    let (pr, pg, pb) = SHOP_BG_COLORS[bg_sel.min(SHOP_BG_COLORS.len() - 1)];
    let k = crate::sky::planet_tint(planet);
    for px in grad.pixels_mut() {
        px[0] = (px[0] as f32 * (1.0 - k) + pr as f32 * k).min(255.0) as u8;
        px[1] = (px[1] as f32 * (1.0 - k) + pg as f32 * k).min(255.0) as u8;
        px[2] = (px[2] as f32 * (1.0 - k) + pb as f32 * k).min(255.0) as u8;
    }
    // Its air dimmed away from the horizon: the original photo is pulled
    // hard to the dark sky colour, and a world at full glow beside it
    // floods half the screen. Bright at the limb, fading as it rises.
    let (h0, h1) = (bg_h as f32 * 0.55, bg_h as f32 * 0.82);
    for y in 0..bg_h {
        let t = ((y as f32 - h0) / (h1 - h0)).clamp(0.0, 1.0);
        let k = 0.45 + 0.55 * t * t * (3.0 - 2.0 * t);
        if k >= 1.0 { continue; }
        for x in 0..bg_w {
            let p = grad.get_pixel_mut(x, y);
            for i in 0..3 { p[i] = (p[i] as f32 * k) as u8; }
        }
    }
    let sky = composite_starfield_gradient(&imgs.space, &grad, bg_w, bg_h, bg_h / 8);
    let flip = Arc::new(flip_image_vertical(&sky));
    let pair: SkyPair = Arc::new((Arc::new(sky), flip));
    *cur.lock().unwrap() = Some(((planet, bg_sel), pair.clone()));
    pair
}

/// The planet's sky if it is built, else start building it (off the main
/// thread) and say not yet. Building one takes ~150 ms: done on the frame a
/// run began, it froze the dive into the cat.
fn planet_sky_ready(planet: usize, bg_sel: usize) -> Option<SkyPair> {
    static BUILDING: std::sync::Mutex<Option<(usize, usize)>> = std::sync::Mutex::new(None);
    static CURRENT_PEEK: OnceLock<std::sync::Mutex<Option<((usize, usize), SkyPair)>>> = OnceLock::new();
    let peek = CURRENT_PEEK.get_or_init(|| std::sync::Mutex::new(None));
    if let Some((k, p)) = peek.lock().unwrap().as_ref() {
        if *k == (planet, bg_sel) { return Some(p.clone()); }
    }
    let mut b = BUILDING.lock().unwrap();
    if *b != Some((planet, bg_sel)) {
        *b = Some((planet, bg_sel));
        std::thread::spawn(move || {
            let p = planet_sky(planet, bg_sel);
            *CURRENT_PEEK.get_or_init(|| std::sync::Mutex::new(None)).lock().unwrap() = Some(((planet, bg_sel), p));
        });
    }
    None
}

/// The sky picture (and its flip) for the picked planet and background, and
/// which planet it shows: Earth's sky stands in until another is ready.
fn sky_for(planet: usize, bg_sel: usize) -> (Arc<image::RgbaImage>, Arc<image::RgbaImage>, usize) {
    let imgs = BG_IMAGES.get_or_init(compute_bg_images);
    if planet != 0 {
        if let Some(p) = planet_sky_ready(planet, bg_sel) { return (p.0.clone(), p.1.clone(), planet); }
    }
    let b = bg_sel.min(imgs.palettes.len() - 1);
    (imgs.palettes[b].clone(), imgs.palettes_flip[b].clone(), 0)
}

/// Build the picked planet's sky ahead of the run, off the main thread.
pub fn prewarm_sky(planet: usize, bg_sel: usize) {
    if planet == 0 { return; }
    let _ = planet_sky_ready(planet, bg_sel);
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
use super::upgrades;
use super::helpers::*;
const PAUSE_MENU_ANIM_FRAMES: i32 = 14;
const PLAYER_TRAIL_EMITTER_NAME: &str = "player_trail";
const PLAYER_TRAIL_MID_NAME:  &str = "player_trail_b";
const PLAYER_TRAIL_ACCENT_NAME: &str = "player_trail_c";

fn mid_trail_color(near: (u8, u8, u8)) -> (u8, u8, u8) {
    const ANCHOR: (f32, f32, f32) = (100.0, 120.0, 255.0);
    let b = |c: u8, a: f32| (c as f32 * 0.35 + a * 0.65).round() as u8;
    (b(near.0, ANCHOR.0), b(near.1, ANCHOR.1), b(near.2, ANCHOR.2))
}

/// The trail's three emitters (streak, glow, its own particles) under the given
/// names. Shared with the shop's preview so it shows the very trail you get.
pub(crate) fn trail_emitters(trail_color: (u8, u8, u8), names: [&str; 3]) -> [Emitter; 3] {
    // Each trail in the style of its shop card: the streak runs from a bright
    // head colour to a deep tail colour, and a third emitter adds the trail's
    // own particles (speed glints, embers, ice shards, glitter, mystic stars).
    let idx = SHOP_TRAIL_COLORS.iter().position(|c| *c == trail_color);
    let (near, mid) = match idx {
        Some(1) => ((255, 225, 120), (230, 60, 15)),
        Some(2) => ((235, 250, 255), (60, 150, 255)),
        Some(3) => ((255, 240, 160), (215, 145, 20)),
        Some(4) => ((255, 165, 240), (125, 55, 235)),
        Some(_) => ((255, 255, 255), (150, 180, 240)),
        None => (trail_color, mid_trail_color(trail_color)),
    };
    let trail_near = EmitterBuilder::new(names[0])
        .rate(350.0).lifetime(0.18).velocity(0.0, 0.0)
        .spread(10.0, 10.0).size(45.0)
        .color(near.0, near.1, near.2, 240)
        .color_end(mid.0, mid.1, mid.2, 0)
        .size_end(12.0).shape(ParticleShape::Circle)
        .interpolate_position(true)
        .render_layer(3).gravity_scale(0.0)
        .collision(CollisionResponse::None).build();
    let trail_mid = EmitterBuilder::new(names[1])
        .rate(280.0).lifetime(0.40).velocity(0.0, 0.0)
        .spread(20.0, 20.0).size(32.0)
        .color(mid.0, mid.1, mid.2, 200)
        .color_end(mid.0, mid.1, mid.2, 0)
        .size_end(10.0).shape(ParticleShape::Circle)
        .interpolate_position(true)
        .render_layer(2).gravity_scale(0.0)
        .collision(CollisionResponse::None).build();
    let accent = match idx {
        // Fire: embers rising off the tail.
        Some(1) => EmitterBuilder::new(names[2])
            .rate(90.0).lifetime(0.6).velocity(0.0, -150.0).spread(140.0, 90.0)
            .size(11.0).size_end(2.0).shape(ParticleShape::Circle)
            .color(255, 200, 70, 255).color_end(230, 50, 10, 0).gravity_scale(-0.2),
        // Ice: little shards tumbling off and falling.
        Some(2) => EmitterBuilder::new(names[2])
            .rate(60.0).lifetime(0.55).velocity(0.0, 0.0).spread(120.0, 90.0)
            .size(12.0).size_end(3.0).shape(ParticleShape::Square)
            .color(210, 245, 255, 255).color_end(80, 160, 255, 0).gravity_scale(0.3),
        // Gold: glitter twinkling out.
        Some(3) => EmitterBuilder::new(names[2])
            .rate(120.0).lifetime(0.5).velocity(0.0, 0.0).spread(160.0, 160.0)
            .size(8.0).size_end(0.5).shape(ParticleShape::Circle)
            .color(255, 245, 170, 255).color_end(255, 200, 60, 0).gravity_scale(0.0),
        // Mystic: soft stars drifting round it.
        Some(4) => EmitterBuilder::new(names[2])
            .rate(80.0).lifetime(0.75).velocity(0.0, 0.0).spread(190.0, 190.0)
            .size(10.0).size_end(1.0).shape(ParticleShape::Soft { roundness: 0.3 })
            .color(255, 225, 255, 255).color_end(140, 80, 255, 0).gravity_scale(-0.05),
        // White: quick bright glints.
        _ => EmitterBuilder::new(names[2])
            .rate(45.0).lifetime(0.3).velocity(0.0, 0.0).spread(90.0, 90.0)
            .size(8.0).size_end(0.5).shape(ParticleShape::Circle)
            .color(255, 255, 255, 255).color_end(170, 200, 255, 0).gravity_scale(0.0),
    }
    .interpolate_position(true).render_layer(3).collision(CollisionResponse::None).build();
    [trail_near, trail_mid, accent]
}

/// Remove existing trail emitters, build fresh ones from `trail_color`, and attach to "player".
fn rebuild_player_trail(c: &mut Canvas, trail_color: (u8, u8, u8)) {
    let idx = SHOP_TRAIL_COLORS.iter().position(|c| *c == trail_color);
    let names = [PLAYER_TRAIL_EMITTER_NAME, PLAYER_TRAIL_MID_NAME, PLAYER_TRAIL_ACCENT_NAME];
    for n in names { c.remove_emitter(n); }
    for e in trail_emitters(trail_color, names) { c.add_emitter(e); }
    for n in names { c.attach_emitter_to(n, "player"); }
    // MYSTIC: its twin twisting strands too.
    super::mystic_trail::set(c, idx == Some(4));
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
    c.set_var("pause_menu_open", false);
    for name in ["pause_overlay", "pause_panel", "pause_title", "pause_divider", "pause_stats_text",
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

/// Close the pause menu and put the world back the way it was.
///
/// Factored out because three things now dismiss the pause menu — the P key,
/// the RESUME button, and the pause gesture on mobile — and each of them has to
/// restore exactly the same state. Two of the three used to carry their own
/// copy of this, and a copy is one edit away from resuming into a slightly
/// different world than the one that was paused.
/// Which pause-menu button a virtual-space touch landed on, by the button's own
/// bounds.
///
/// The pause UI is `ignore_zoom`: its objects hold VIRTUAL positions and are
/// drawn at base scale, and input positions are virtual too, so the two compare
/// directly. The old test multiplied the input by the camera's zoom first,
/// which is only the identity when the zoom happens to be 1 — and the game
/// zooms with the player's height, so the buttons drifted out from under the
/// finger by however far the camera happened to be zoomed. That is why the
/// menu could be opened and not closed: `P` never went near this code.
///
/// Reading the objects' real rectangles also removes the hardcoded 780/950/1000
/// numbers, which were a second copy of the layout.
/// The live run's state, reachable from handlers registered outside the scene.
///
/// Input handlers must be registered at `App::new` — a mouse-press handler
/// registered inside a scene's `on_enter` never receives presses in the live
/// window — so they cannot capture a local created inside `build_game_scene`.
/// There is one game scene and one run at a time, so a process singleton is the
/// honest shape for it.
/// Turn the engine's lighting on and add the player's light (attached to the
/// player on each game entry). Called at app start; idempotent.
pub fn enable_game_lighting(canvas: &mut Canvas) {
    if canvas.has_lighting() { return; }
    canvas.enable_lighting(LightingConfig {
        max_lights: LIGHTING_MAX_LIGHTS,
        ..LightingConfig::default()
    });
    // `enable_lighting` also switches on a BLOOM post pass, and nothing in
    // the game wants that outside a darkness phase (night mode brings its
    // own). It stayed on through the whole first run — only leaving a run
    // cleared it — so the FIRST run alone had bloom: the PLAY flash and the
    // menu shutter's flash blew out to solid white for a third of a second,
    // only the first time. Cleared here so every run looks like the rest.
    canvas.clear_post_override();
    canvas.add_light(LightSource::new(
        "__player_light",
        (0.0, 0.0),
        Color(210, 220, 255, 255),
        1100.0,
        0.8,
    ));
    // OFF until a darkness phase turns it on (`eclipse::begin_night_mode`).
    // It used to start ON, and only `end_night_mode` — run when a run ends —
    // switched it off: so for the whole FIRST run it lit everything near the
    // player, the PLAY flash and the menu shutter blew out to white, and
    // only the first time.
    canvas.set_light_enabled("__player_light", false);
}

pub fn run_state() -> &'static Arc<Mutex<Option<Arc<Mutex<State>>>>> {
    static S: std::sync::OnceLock<Arc<Mutex<Option<Arc<Mutex<State>>>>>> = std::sync::OnceLock::new();
    S.get_or_init(|| Arc::new(Mutex::new(None)))
}

/// Which upgrade-dialogue line a click landed on, 1-5, or 6 for "leave".
///
/// A click anywhere inside the panel but not on a line does nothing; a click
/// outside the panel closes it, which is what every other modal in the game
/// does.
fn upgrade_option_at(c: &Canvas, pos: (f32, f32)) -> Option<u8> {
    // The dialogue is `ignore_zoom` too — see the note in `pause_button_at`.
    let pos = c.input_to_unzoomed(pos);
    let panel = c.get_game_object("upgrade_dialogue_panel")?;
    if !panel.visible { return None; }
    for i in 0..6u8 {
        if let Some(obj) = c.get_game_object(&format!("upgrade_opt_{i}")) {
            if obj.visible
                && pos.0 >= obj.position.0
                && pos.0 <= obj.position.0 + obj.size.0
                && pos.1 >= obj.position.1
                && pos.1 <= obj.position.1 + obj.size.1
            {
                return Some(i + 1);
            }
        }
    }
    let inside = pos.0 >= panel.position.0
        && pos.0 <= panel.position.0 + panel.size.0
        && pos.1 >= panel.position.1
        && pos.1 <= panel.position.1 + panel.size.1;
    if inside { None } else { Some(6) }
}

fn pause_button_at(c: &Canvas, pos: (f32, f32)) -> Option<&'static str> {
    // The pause UI is `ignore_zoom`, so it is laid out with the base scale
    // while input arrives divided by a scale that includes the camera zoom.
    // Without this the whole menu is unreachable by exactly the zoom factor —
    // measured on the device as a tap at x=1478 against a Resume button
    // spanning 1570..2270 at zoom 1.3.
    let pos = c.input_to_unzoomed(pos);
    // `settings_back_btn` is not in PAUSE_BTN_LAYOUT (that list drives the
    // slide-in animation, and the settings panel does not slide), so it is
    // named explicitly or the settings screen loses its way out.
    let names = PAUSE_BTN_LAYOUT
        .iter()
        .map(|&(n, _, _)| n)
        .chain(std::iter::once("settings_back_btn"));
    for name in names {
        if let Some(obj) = c.get_game_object(name) {
            if obj.visible
                && pos.0 >= obj.position.0
                && pos.0 <= obj.position.0 + obj.size.0
                && pos.1 >= obj.position.1
                && pos.1 <= obj.position.1 + obj.size.1
            {
                return Some(name);
            }
        }
    }
    None
}

fn resume_from_pause(c: &mut Canvas) {
    // Back in phase with the tick loop, which restarts just below.
    crate::audio_state::resume_game_bgm();
    let from_stasis = matches!(c.get_var("pause_came_from_stasis"), Some(Value::Bool(true)));
    c.set_var("pause_came_from_stasis", false);
    c.resume();
    c.set_var("pause_animating", false);
    c.set_var("pause_anim_frames", 0);
    hide_pause_ui(c);
    c.set_var("settings_open", false);
    c.set_var("settings_dragging", -1i32);
    if from_stasis {
        // Back to the soft-pause stasis, which is where it came from.
        c.set_var("game_paused", true);
    } else {
        // Back to gameplay.
        c.set_var("game_paused", false);
        c.set_var("start_prompt_active", false);
    }
    // Put the start prompt back if the run is still waiting to begin.
    // `hide_pause_ui` clears it along with the menu, so without this a run
    // paused during the intro resumed with no prompt and no obvious way to
    // start. The pause control comes back on its own, from the tick callback
    // that derives it, once ticks resume.
    if c.get_bool("start_prompt_active") {
        if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
            obj.visible = true;
        }
    }
    rebuild_player_trail(c, selected_trail_color(c));
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.visible = true;
    }
    if !from_stasis {
        let was_hooked = matches!(c.get_var("rope_visible_at_pause"), Some(Value::Bool(true)));
        if let Some(obj) = c.get_game_object_mut("rope") {
            obj.visible = was_hooked;
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

/// Show the pause menu overlay + buttons (off-screen for the entrance slide).
/// `from_stasis` records whether the pause was opened from a soft-pause stasis
/// so that resume returns to the stasis rather than straight to gameplay.
pub(crate) fn open_pause_menu(c: &mut Canvas, from_stasis: bool) {
    // Dead: the death screen is the menu.
    if super::death_screen::active(c) { return; }
    c.set_var("pause_came_from_stasis", from_stasis);
    // Reset any leftover settings state so the main pause buttons are always
    // hit-testable when the menu first opens (a stale `settings_open` would
    // route clicks to the settings-slider branch and swallow them).
    c.set_var("settings_open", false);
    c.set_var("settings_dragging", -1i32);
    c.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
    c.remove_emitter(PLAYER_TRAIL_MID_NAME);
    c.remove_emitter(PLAYER_TRAIL_ACCENT_NAME);
    super::mystic_trail::set(c, false);
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.visible = false;
    }
    let rope_was_visible = c.get_game_object("rope").map_or(false, |o| o.visible);
    c.set_var("rope_visible_at_pause", rope_was_visible);
    if let Some(obj) = c.get_game_object_mut("rope") {
        obj.visible = false;
    }
    // The dim is in place at once (it covers the screen whatever its shape);
    // the card and its buttons slide in over it.
    if let Some(obj) = c.get_game_object_mut("pause_overlay") {
        obj.position = super::bootstrap::PAUSE_OVERLAY_POS;
        obj.visible = true;
    }
    if let Some(obj) = c.get_game_object_mut("pause_panel") {
        let (px, py) = super::bootstrap::PAUSE_PANEL_POS;
        obj.position = (px, py - VH);
        obj.visible = true;
    }
    for &(name, bx, by) in PAUSE_BTN_LAYOUT.iter() {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position = (bx, by - VH);
            obj.visible = true;
        }
    }
    // Hide the things that would otherwise draw straight through the menu.
    // Done HERE rather than derived in a tick callback: `c.pause()` stops tick
    // callbacks entirely, so anything not already hidden by the time the menu
    // finishes opening stays on screen for as long as the game is paused —
    // which is how "HOLD SPACE TO BEGIN" ended up printed across the Restart
    // button, and the pause control itself stayed lit behind the menu.
    for name in ["start_prompt_text", "pause_touch_btn"] {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
        }
    }
    // The run so far, under the title.
    if let Some(st) = run_state().lock().unwrap().as_ref().cloned() {
        let (dist, coins, score) = { let s = st.lock().unwrap(); (s.distance, s.coin_count, s.score) };
        let group = |n: u64| { let d = n.to_string(); let mut o = String::new();
            for (i, ch) in d.chars().enumerate() { if i > 0 && (d.len() - i) % 3 == 0 { o.push(','); } o.push(ch); } o };
        let line = format!("DISTANCE  {}     \u{2022}     COINS  {}     \u{2022}     SCORE  {}", group(dist.max(0.0) as u64), group(coins as u64), group(score as u64));
        if let Some(font) = ui_font() {
            let s = c.virtual_scale();
            if let Some(o) = c.get_game_object_mut("pause_stats_text") {
                o.set_drawable(Box::new(ui_text_spec(&line, &font, 34.0 * s, Color(196, 184, 245, 230), (super::bootstrap::PAUSE_PANEL_SIZE.0 - 160.0) * s)));
            }
        }
    }
    c.set_var("pause_anim_total", PAUSE_MENU_ANIM_FRAMES);
    c.set_var("pause_anim_frames", PAUSE_MENU_ANIM_FRAMES);
    c.set_var("pause_animating", true);
}

/// Give the ball its tangential launch velocity and release the intro camera so
/// the orbit-start handoff feels smooth. Used by both the space key and a mouse
/// click to begin.
fn launch_from_orbit(c: &mut Canvas, st: &Arc<Mutex<State>>, ticks: f32) {
    const ORBIT_R: f32 = 240.0;
    const ORBIT_OMEGA: f32 = 0.038;
    let theta = -std::f32::consts::FRAC_PI_2 - ORBIT_OMEGA * ticks;
    let vx = ORBIT_R * ORBIT_OMEGA * theta.sin();
    let vy = -(ORBIT_R * ORBIT_OMEGA * theta.cos());
    let (in_space, gdir) = {
        let mut s = st.lock().unwrap();
        s.vx = vx;
        s.vy = vy;
        s.hooked = false;
        let gdir = s.gravity_dir;
        let in_space = s.in_space_mode;
        s.space_stasis_active = false;
        (in_space, gdir)
    };
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.momentum = (vx, vy);
        obj.gravity = GRAVITY * gdir;
    }
    if let Some(obj) = c.get_game_object_mut("rope") {
        obj.visible = false;
    }
    if !in_space {
        if let Some(cam) = c.camera_mut() {
            cam.zoom_anchor = None;
            cam.follow(Some(Target::name("player")));
            cam.snap_zoom(1.0);
        }
        c.set_var("start_follow_force_ticks", 180i32);
        c.set_var("start_zoom_recover_ticks", 0i32);
    }
}

const PAUSE_BTN_LAYOUT: [(&str, f32, f32); 7] = [
    ("pause_title",        (VW - 650.0) / 2.0, super::bootstrap::PAUSE_TITLE_Y),
    ("pause_divider",      (VW - super::bootstrap::PAUSE_DIVIDER.0) / 2.0, super::bootstrap::PAUSE_DIVIDER_Y),
    ("pause_stats_text",   (VW - (super::bootstrap::PAUSE_PANEL_SIZE.0 - 160.0)) / 2.0, super::bootstrap::PAUSE_STATS_Y),
    ("pause_resume_btn",   (VW - 700.0) / 2.0, 780.0),
    ("pause_restart_btn",  (VW - 700.0) / 2.0, 1000.0),
    ("pause_settings_btn", (VW - 700.0) / 2.0, 1220.0),
    ("pause_menu_btn",     (VW - 700.0) / 2.0, 1440.0),
];

pub(crate) fn switch_game_bgm(c: &mut Canvas, track_idx: i32, asset: &'static [u8], base_vol: f32) {
    if c.get_i32("bgm_track_index") != track_idx {
        let handle = c.play_sound_bytes_with(asset, SoundOptions::new().volume(music_volume(c, base_vol)).looping(true));
        audio_state::replace_game_bgm(handle);
        c.set_var("bgm_track_index", track_idx);
        c.set_var("bgm_base_vol", base_vol);
    }
}

/// Register the pause menu's and the touch controls' input handlers.
///
/// Called from `App::new`, NOT from the game scene's `on_enter`, where this
/// whole block used to live. A canvas mouse-press handler registered inside a
/// scene's `on_enter` never receives presses in the live window — it works in
/// the headless harness, which is what let it look correct — so the pause
/// button did nothing, and once paused there was no way back out. The menu's
/// handler was moved out for exactly this reason; this is the same fix for the
/// game scene.
///
/// Every handler already guards on `is_scene("game")`, so registering them
/// before any scene is loaded is safe.
pub fn register_pause_ui_handlers(canvas: &mut Canvas) {
        // ── Pause menu button handlers (register once) ───────────────
        let pause_btns_registered = matches!(
            canvas.get_var("pause_btns_registered"),
            Some(Value::Bool(true))
        );
        if !pause_btns_registered {
            // Click handlers
            canvas.register_custom_event("pause_resume_click".into(), |c| {
                if !c.get_bool("game_paused") { return; }
                let from_stasis = matches!(c.get_var("pause_came_from_stasis"), Some(Value::Bool(true)));
                clear_pause_state(c);
                if from_stasis {
                    // Return to the soft-pause stasis (keep soft pause).
                    c.set_var("pause_came_from_stasis", false);
                    c.set_var("game_paused", true);
                    rebuild_player_trail(c, selected_trail_color(c));
                    if let Some(obj) = c.get_game_object_mut("player") { obj.visible = true; }
                } else {
                    rebuild_player_trail(c, selected_trail_color(c));
                    if let Some(obj) = c.get_game_object_mut("player") { obj.visible = true; }
                    let was_hooked = matches!(c.get_var("rope_visible_at_pause"), Some(Value::Bool(true)));
                    if let Some(obj) = c.get_game_object_mut("rope") { obj.visible = was_hooked; }
                }
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
                // The shutter closes over the game frozen where it is (it
                // animates while paused), then the menu opens behind it.
                c.pause();
                crate::transition::go_slow(c, "menu");
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
                        // Track the finger for the pause swipe. Before the
                        // early return below, because it has to be sampled
                        // while the pause menu is CLOSED — that is when the
                        // gesture that opens it happens.
                        if mobile_controls_enabled() {
                            c.set_var("touch_last_x", pos.0);
                            c.set_var("touch_last_y", pos.1);
                        }
                        // Only the full pause menu is clickable; a soft-pause
                        // stasis (boss orbit, respawn, hold-space) must not
                        // let you hit invisible buttons.
                        if !matches!(c.get_var("pause_menu_open"), Some(Value::Bool(true))) {
                            return;
                        }

                        // Pause/settings UI is ignore_zoom — see the note on
                        // `Canvas::input_to_unzoomed`. Taken from the LAYOUT's
                        // zoom rather than the camera's, because that is the
                        // one the objects were actually laid out with; the two
                        // differ mid-transition while a smooth zoom settles.

                        // If dragging a settings slider, update its position/value.
                        let dragging = c.get_i32("settings_dragging");
                        if dragging >= 0 && c.get_bool("settings_open") {
                            let idx = dragging as usize;
                            if idx < 3 {
                                let vol = ((c.input_to_unzoomed(pos).0 - SLIDER_TRACK_X) / SLIDER_TRACK_W).clamp(0.0, 1.0);
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

                        let (ux, uy) = c.input_to_unzoomed(pos);
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

                    // ── Modal prompts, before anything else can take the click.
                    //
                    // Both of these used to be keyboard-only (1-5/Esc for the
                    // upgrade node, F for the cannon), which on a phone means
                    // the node holds the player in a menu they cannot answer.
                    // Not gated on mobile: clicking a menu option works on
                    // desktop too, and the keys still do what they did.
                    if let Some(state_arc) = run_state().lock().unwrap().as_ref().cloned() {
                        if super::perks::reward_pending(c) {
                            if let Some(i) = super::perks::card_at(pos) {
                                c.set_var("mouse_left_held", false);
                                super::perks::choose(c, &state_arc, i);
                            }
                            return;
                        }
                        if state_arc.lock().unwrap().upgrade_dialogue_active {
                            if let Some(opt) = upgrade_option_at(c, pos) {
                                c.set_var("mouse_left_held", false);
                                upgrades::upgrade_dialogue_select(c, &state_arc, opt);
                            }
                            return;
                        }
                        let ft_ready = {
                            let st = state_arc.lock().unwrap();
                            st.cannon_ft_prompt && st.cannon_captured
                        };
                        if ft_ready {
                            let hpos = c.input_to_unzoomed(pos);
                            let hit = c.get_game_object("cannon_prompt_text").is_some_and(|o| {
                                o.visible
                                    && hpos.0 >= o.position.0
                                    && hpos.0 <= o.position.0 + o.size.0
                                    && hpos.1 >= o.position.1
                                    && hpos.1 <= o.position.1 + o.size.1
                            });
                            if hit {
                                state_arc.lock().unwrap().cannon_ft_active = true;
                                c.set_var("mouse_left_held", false);
                                return;
                            }
                        }
                    }

                    if mobile_controls_enabled() {
                        // Remember where this touch began, so the release
                        // can tell a swipe from a tap. `pos` is already in
                        // virtual space.

                        let paused = matches!(
                            c.get_var("pause_menu_open"), Some(Value::Bool(true))
                        );
                        if !paused {
                            // The pause control comes first: it sits on the
                            // left, so it can never take a touch the swing
                            // half wanted.
                            // Same space as the rest of the pause UI: this
                            // control is drawn `ignore_zoom`.
                            let bpos = c.input_to_unzoomed(pos);
                            if point_in_rect(bpos.0, bpos.1, PAUSE_TOUCH_BTN) {
                                let animating = matches!(
                                    c.get_var("pause_animating"), Some(Value::Bool(true))
                                );
                                if !animating {
                                    c.set_var("mouse_left_held", false);
                                    open_pause_menu(c, is_game_paused(c));
                                }
                                return;
                            }
                            // Holding the right half swings, exactly as
                            // holding the mouse button does on desktop. The
                            // whole half is the control, so it can be used
                            // without looking at it.
                            if mobile_swing_zone_contains(pos.0, pos.1) {
                                c.set_var("mouse_left_held", true);
                            }
                            return;
                        }
                        // Pause menu open: fall through to its own hit
                        // testing. Returning here instead made every button
                        // in the pause menu untappable on a phone.
                    }

                    // Only the full pause menu is clickable; a soft-pause
                    // stasis must not let you hit invisible buttons.
                    if !matches!(c.get_var("pause_menu_open"), Some(Value::Bool(true))) {
                        return;
                    }

                    // Pause/settings UI is ignore_zoom — see the note on
                    // `Canvas::input_to_unzoomed`.
                    let (ux, uy) = c.input_to_unzoomed(pos);

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

                    // Hit-tested against the buttons' own rectangles, in
                    // the virtual space the input already arrives in.
                    match pause_button_at(c, pos) {
                        Some("pause_resume_btn") => {
                            // Called directly rather than dispatched: this
                            // is the one control the player cannot do
                            // without, so it takes the shortest path there
                            // is to the same function `P` uses.
                            resume_from_pause(c);
                        }
                        Some("pause_restart_btn") => {
                            c.run(Action::Custom { name: "pause_restart_click".into() });
                        }
                        Some("pause_settings_btn") => {
                            c.run(Action::Custom { name: "pause_settings_click".into() });
                        }
                        Some("pause_menu_btn") => {
                            c.run(Action::Custom { name: "pause_menu_click".into() });
                        }
                        Some("settings_back_btn") => {
                            c.run(Action::Custom { name: "settings_back_click".into() });
                        }
                        _ => {}
                    }
                });

                // Show the pause control while a run is live (Android only).
                canvas.on_update(|c| {
                    if !c.is_scene("game") { return; }
                    if !mobile_controls_enabled() { return; }
                    // Also off while the menu is sliding in: `pause_menu_open`
                    // only becomes true when that animation FINISHES, so
                    // deriving from it alone re-showed the control on every
                    // frame of the opening animation — and the last of those
                    // frames is the state it stays in, because `c.pause()`
                    // stops tick callbacks from then on.
                    let show = !matches!(
                        c.get_var("pause_menu_open"), Some(Value::Bool(true))
                    ) && !matches!(
                        c.get_var("pause_animating"), Some(Value::Bool(true))
                    );
                    if let Some(obj) = c.get_game_object_mut("pause_touch_btn") {
                        obj.visible = show;
                    }
                });

                canvas.on_mouse_release(move |c, _btn, _pos| {
                    if !c.is_scene("game") { return; }
                    if mobile_controls_enabled() {
                        c.set_var("mouse_left_held", false);
                    }
                    c.set_var("settings_dragging", -1i32);
                });

                canvas.set_var("pause_ui_mouse_registered", true);
            }
            canvas.set_var("pause_btns_registered", true);
        }
}

pub fn build_game_scene(ctx: &mut Context) -> Scene {
    // Kick off background image computation immediately so the tutorial scene
    // renders without delay. By the time the player navigates to the game scene
    // the images are ready. The OnceLock guarantees exactly-once computation.
    std::thread::spawn(|| { BG_IMAGES.get_or_init(compute_bg_images); });
    super::sky_extras::prewarm();
    // Everything else the FIRST run builds on first touch, warmed now, at
    // app start. Warmed in the game's own `on_enter` (below) it was too late:
    // the opening level generation needs it in that same call, so the first
    // PLAY froze ~0.6 s (debug) at the end of the menu's dive while the
    // cannon frames and node art decoded. The shutter's art is the same story
    // for the first trip back to the menu.
    std::thread::spawn(|| {
        super::helpers::prewarm_hook_artifact();
        super::helpers::prewarm_hook_artifact_green();
        super::helpers::prewarm_zero_g_overlay();
        super::gravity_cannon::prewarm();
        crate::transition::prewarm();
        super::death_screen::prewarm();
        super::boss::fx_kit::prewarm();
    });

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
        space_oxygen_pickup_free, upgrade_free,
        boss_bolt_free, impact_free, boss_asteroid_ids, comet_free, warn_free,
    } = pools;

    let starter_hooks = crate::level_gen::starter_hooks();

    // Persistent state arc — created on first enter, reused on respawns.
    let persistent_state: Arc<Mutex<Option<Arc<Mutex<State>>>>> = Arc::clone(run_state());
    scene
        .on_enter(move |canvas| {
            let __t0 = std::time::Instant::now();
            let __prof = |what: &str| if std::env::var("START_PROFILE").is_ok() { eprintln!("START {what}: {:?}", __t0.elapsed()); };
            // ── Crystalline renderer ─────────────────────────────────────
            // Re-create the physics world on every game entry so there is no
            // stale solver state or leftover particles from a previous run.
            canvas.enable_crystalline();
            canvas.set_var("crystalline_ready", true);

            // ── Lighting (enables darkness attacks + shadows) ────────────
            // The engine side is wired; turning it on here makes set_ambient /
            // darkness phases actually render, and lights the player during
            // darkness. Default ambient is full-bright, so normal play is
            // unchanged.
            //
            // Normally already on: `enable_game_lighting` runs at app start.
            // Turned on HERE, mid-load, the first run's opening frames came out
            // blank white under the PLAY flash (and again for the first trip
            // back to the menu) — every later run, with lighting already on,
            // was fine. Kept as a fallback for any path that skipped startup.
            enable_game_lighting(canvas);
            canvas.attach_light("__player_light", "player", (0.0, 0.0));
            // Large world objects cast shadows during darkness phases.
            canvas.set_shadow_caster("player", true);
            canvas.set_shadow_caster("boss", true);

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
                        // Calico cat — always (re)set: the player object
                        // outlives a run, and a breed's frames from the last
                        // one would otherwise stay on it.
                        if let Ok(mut calico) = AnimatedSprite::new(
                            include_bytes!("../../../assets/calicoball.gif"),
                            (PLAYER_R * 2.0, PLAYER_R * 2.0),
                            CALICO_FPS,
                        ) {
                            calico.set_fps(0.0);
                            obj.set_animation(calico);
                        }
                    } else if let Some(mut cat) =
                        crate::cosmetics::cat_sprite(char_idx, PLAYER_R * 2.0, CALICO_FPS)
                    {
                        // A breed: curled, half uncurled, spread-eagle —
                        // driven frame by frame like the calico
                        // (tick_player_ball_animation), so fps 0.
                        cat.set_fps(0.0);
                        obj.set_animation(cat);
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
            // A boss loop still playing (a restart from the pause menu mid-
            // fight reloads this scene without ending the fight): the new run
            // gets the lane's music back. Boss tracks are numbered from 100.
            if audio_state::has_game_bgm() && canvas.get_i32("bgm_track_index") >= 100 {
                switch_game_bgm(canvas, BGM_TRACK_LANE, ASSET_BGM_TRACK_1, 0.084);
            }
            if !audio_state::has_game_bgm() {
                let handle = canvas.play_sound_bytes_with(
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

                    // ── Roguelike upgrade dialogue: route keys to the menu ──
                    if let Some(state_arc) = persistent_state_key.lock().unwrap().as_ref().cloned() {
                        if super::perks::reward_key(c, &state_arc, key) {
                            return;
                        }
                        if upgrades::upgrade_dialogue_key(c, &state_arc, key) {
                            return;
                        }
                    }

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

                    if *key == Key::Character("f".into()) {
                        // Accept the cannon hyper-transit prompt (spend coins to skip ahead).
                        let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                        if let Some(state_arc) = state_opt {
                            let mut s = state_arc.lock().unwrap();
                            if s.cannon_ft_prompt && s.cannon_captured {
                                s.cannon_ft_active = true;
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
                                Key::Character(k) if k == "c" => {
                                    // Debug: grant coins to test the cannon fast-travel.
                                    let mut s = state_arc.lock().unwrap();
                                    s.coin_count += 600;
                                    return;
                                }
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

                    // ── E = fire the equipped ability ──
                    if *key == Key::Character("e".into()) {
                        if !is_game_paused(c) {
                            let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                            if let Some(state_arc) = state_opt {
                                super::abilities::fire_from_key(c, &state_arc);
                            }
                        }
                        return;
                    }

                    // ── Debug: M = play the current boss's phase change now ──
                    if *key == Key::Character("m".into()) {
                        if !is_game_paused(c) {
                            let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                            if let Some(state_arc) = state_opt {
                                super::boss::debug_phase_cine(&state_arc);
                            }
                        }
                        return;
                    }

                    // ── Debug: N = spawn a planet at a random distance ──
                    // (N is also polled in the tick to stop the moving stars'
                    // auto-scroll; nothing restarts it now M is the reveal.)
                    if *key == Key::Character("n".into()) {
                        if !is_game_paused(c) {
                            let state_opt = persistent_state_key.lock().unwrap().as_ref().cloned();
                            if let Some(state_arc) = state_opt {
                                super::decor::debug_spawn_planet(c, &state_arc);
                            }
                        }
                        return;
                    }

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
                        let full_pause = c.is_paused();
                        if full_pause {
                            // Full pause menu open — resume.
                            resume_from_pause(c);
                        } else if is_pause {
                            // Soft-pause stasis: P opens the pause menu (full pause).
                            open_pause_menu(c, true);
                        } else if is_space {
                            // Soft-pause stasis: the run begins after HOLDING
                            // space/mouse for START_HOLD_TICKS (measured in the
                            // on_update `start_prompt_active` branch), not on a
                            // single press.
                        }
                    } else if is_pause {
                        // Gameplay: open the pause menu (full pause).
                        let animating = matches!(
                            c.get_var("pause_animating"),
                            Some(Value::Bool(true))
                        );
                        if animating { return; }
                        open_pause_menu(c, false);
                    }
                });
                canvas.set_var("pause_key_registered", true);
            }

            canvas.set_var("pause_anim_frames", 0);
            canvas.set_var("pause_anim_total", PAUSE_MENU_ANIM_FRAMES);
            canvas.set_var("pause_animating", false);
            canvas.set_var("game_paused", false);
            canvas.set_var("start_prompt_active", false);
            // These are read by the pause-menu code and may be hit before
            // open_pause_menu ever runs (e.g. during a respawn stasis), so
            // initialise them here to avoid a get_bool missing-key panic.
            canvas.set_var("pause_came_from_stasis", false);
            canvas.set_var("rope_visible_at_pause", false);
            canvas.set_var("pause_menu_open", false);
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
            // The interactive tutorial (a Casual run that cannot be lost), or
            // the daily challenge: the same world for everyone today.
            let tutorial = super::tutorial::begin_run(canvas);
            if tutorial { canvas.set_var(crate::daily::DAILY_PENDING_VAR, false); }
            let daily = crate::daily::begin_run(canvas);
            if let Some(ch) = daily { seed = ch.seed; }
            // A new order of bosses every run, the final boss always last
            // (kept in State as `boss_roster`).
            let boss_roster = crate::constants::deal_roster(seed);

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
            crate::level_gen::reset_patterns();
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

            // Permanent meta-progression (saved in the profile). Permanent extra
            // hearts persist across restarts; cache them into the canvas var so
            // the rest of the game reads the same source.
            let meta_extra_hearts = {
                let g = crate::profile::profile();
                let p = g.lock().unwrap();
                canvas.set_var(META_EXTRA_HEARTS_VAR, Value::I32(p.permanent_extra_hearts as i32));
                p.permanent_extra_hearts as i32
            };
            let start_max_hearts = crate::constants::MAX_HEARTS + meta_extra_hearts;

            // Permanent upgrades resolve once, here, and travel with the run.
            let perm = crate::profile::permanent_bonuses();
            super::seed_funds::begin(canvas, perm.start_coins);
            // So does the equipped ability (its uses and bar start fresh).
            super::abilities::start_run(canvas);
            super::abilities::register_input(canvas);

            // Test hook: start the run partway along the difficulty curve, so a
            // harness can sample what the world looks like at any minute
            // without simulating the play that would get there.
            let canvas_mode_idx = match canvas.get_var(crate::mode::MODE_VAR) {
                Some(Value::I32(i)) => Some(i),
                _ => None,
            };
            let seeded_distance = match canvas.get_var("debug_start_distance") {
                Some(Value::F32(d)) if d > 0.0 => d,
                Some(Value::F64(d)) if d > 0.0 => d as f32,
                _ => 0.0,
            };

            let fresh_state = State {
                px: start_px, py: start_py,
                vx: 0.0, vy: 0.0,
                hooked: false, hook_x: start_hook.0,
                hook_y: start_hook.1, rope_len: start_rope_len,
                active_hook: "hook_0".into(), distance: seeded_distance,
                // SEED FUNDS is paid in on the start screen (`seed_funds`).
                score: 0, coin_count: 0,
                gravity_dir: 1.0, score_time_awards: 0,
                score_distance_awards: 0,
                seed,
                pending: first_batch, live_hooks: starter_names.clone(),
                pool_free: pool_free.clone(),
                rightmost_x,
                gen_head_x,
                gen_head_y,
                last_hook_x: starter_hooks.last().map(|(x, _)| *x).unwrap_or(f32::NEG_INFINITY),
                last_hook_y: starter_hooks.last().map(|(_, y)| *y).unwrap_or(f32::NEG_INFINITY),
                world_sampler: crate::poisson::PoissonSampler::new(600.0), dead: false,
                ticks: 0, pad_live: Vec::new(),
                pad_free: pad_free.clone(), pad_rightmost: SPAWN_X,
                pad_origins: Vec::new(), spinner_live: Vec::new(),
                spinner_free: spinner_free.clone(), spinner_rightmost: SPAWN_X + VW * 0.65,
                spinner_origins: Vec::new(),
                // Temporarily disable spinner collisions/behavior.
                spinners_enabled: true, spinner_spin_enabled: true,
                spinner_hit_cooldown: 0, coin_live: Vec::new(),
                coin_free: coin_free.clone(), coin_rightmost: SPAWN_X, reward_arcs: Default::default(), run_stats: Default::default(), run_air_ticks: 0,
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
                space_extract:            false,

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

                space_oxygen_pickup_live:      Vec::new(),
                space_oxygen_pickup_free:      space_oxygen_pickup_free.clone(),
                space_oxygen_pickup_rightmost: SPAWN_X,

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
                cannon_damp_timer: 0,
                cannon_ft_prompt:  false, cannon_ft_active: false,
                cannon_fast_travel_grace: 0,
                boss_active:       false,
                boss_kind:         crate::constants::boss_kind_for_run(&boss_roster, 0),
                boss_roster:       boss_roster.clone(),
                boss_parts:        Vec::new(),
                boss_entry_ticks:  0, boss_spawned:      false,
                boss_cleared:      false, boss_hp:           crate::constants::BOSS_MAX_HP,
                boss_approach_nodes_spawned: false,
                boss_stasis_active: false, boss_stasis_ticks: 0, boss_stasis_hook: String::new(),
                boss_phase:        0.0, boss_vx:           0.0,
                boss_vy:           0.0, boss_shoot_timer:  crate::constants::BOSS_SHOOT_INTERVAL,
                boss_bolt_live:    Vec::new(), boss_bolt_free:    boss_bolt_free.clone(),
                impact_live: Vec::new(), impact_free: impact_free.clone(),
                hitstop_ticks: 0, hitstop_pos: (0.0, 0.0), hitstop_vel: (0.0, 0.0),
                hitstop_target: String::new(),
                boss_asteroids:    boss_asteroid_ids.clone(), hud_last_boss_hp:  -999,
                boss_dark_cooldown: BOSS_DARK_INTERVAL, boss_dark_ticks: 0,
                boss_dark_active:   false,
                boss_generators:    Vec::new(), boss_generator_hp: Vec::new(),
                boss_generator_snap: Vec::new(), boss_generator_flash: Vec::new(),
                boss_barrier_up:    true, boss_final_phase: false,
                boss_lunge_telegraph: BOSS_LUNGE_TELEGRAPH, boss_lunge_ticks: 0,
                boss_lunge_target:  (0.0, 0.0),
                boss_flare_window_ticks: 0,
                magnetar_clock: 0, magnetar_clock_ticks: 0, magnetar_beam: 0,
                magnetar_beam_ticks: 0, magnetar_beam_cycles: 0, magnetar_beam_hit: 0,
                magnetar_angle: 0.0, magnetar_spin: 1.0, magnetar_pull_next: true,
                magnetar_pulses: 0,
                boss_beat_ticks: 0, boss_beat_interval: 36, boss_resonance: 0,
                boss_was_hooked: false, boss_release_window: 0,
                boss_pattern_cooldown: 0,
                boss_meteor_lock_ticks: 0,
                weaver_flip_ticks: 0, weaver_flip_open: 0, weaver_orbit: 0.0,
                weaver_inverted: false, weaver_flips: 0,
                weaver_gauntlet: 0, weaver_gauntlet_ticks: 0, weaver_gauntlet_done: 0,
                weaver_gauntlet_parts: [0, 0], weaver_gauntlet_y: 0.0, weaver_gauntlet_fired: false,
                phase_cine: PhaseCine::default(), phase_cine_done: 0,
                titan_clock: 0, titan_clock_ticks: 0, titan_orbit: 0.0, titan_flares: 0,
                decor: Vec::new(), decor_cam_prev: None, decor_player_prev_x: None, decor_travel: 0.0,
                decor_next_at: 0.0, decor_last_kind: None, decor_ticks: 0,
                titan_charged: false, titan_burn_timer: 0, titan_guide_node: String::new(),
                titan_reshelter: false, grab_lockout_ticks: 0,
                boss_contact_cooldown: 0,
                conductor_beat_phase: 0.0,
                conductor_beat_clock: None,
                conductor_beat_in_bar: 0,
                conductor_loop_bar: 0,
                conductor_bar_hits: 0,
                conductor_last_kick: -1,
                conductor_cue_delay: 0,
                impact_sfx_cd: 0,
                conductor_wave_shape: 0,
                conductor_rng: 0x9E37_79B9_7F4A_7C15,
                conductor_eq: [0.0; 4],
                conductor_eq_cd: 0,
                conductor_core_vuln: false,
                boss_hp_max: BOSS_MAX_HP,
                conductor_beat_fx: Vec::new(),
                conductor_attack: 0,
                conductor_attack_ticks: 0,
                conductor_telegraph: 0,
                conductor_bar_dir: 1.0,
                conductor_bar_gap_y: BOSS_Y_CENTER,
                conductor_wave_count: 1,
                conductor_wave_hit: 0,
                conductor_bar_hit: false,
                conductor_ring_fx: Vec::new(),
                boss_part_invuln_ticks: 0,
                beam_explode_live: Vec::new(), buff_fx_attached:  Vec::new(),
                shield_fx_attached: Vec::new(), shield_player_fx:  false,
                buff_player_fx:     false,
                boss_torso_attack: 0,             boss_meteor_queue: Vec::new(),
                serpent_act:       SerpentAct::Prowl,
                serpent_act_ticks: 0,             serpent_cooldown:  SERPENT_ATTACK_GAP,
                serpent_rifts:     Vec::new(),    serpent_gambit_react: 0,
                serpent_gambit_exit: None,        serpent_gambit_exit_ticks: 0,
                serpent_roam_to:   None,          serpent_roam_ticks: 0,
                serpent_hole:      None,         serpent_surfaced:  0,
                serpent_next_hole: None,         serpent_coil_at:   (0.0, 0.0),
                serpent_rift_phase: RiftPhase::Approach,
                serpent_rift_ticks: 0,
                serpent_contact_cooldown: 0,      serpent_tail_out:  false,
                serpent_lash_from: None,
                serpent_tail_pos:  (0.0, 0.0),
                serpent_trail:     Vec::new(),    serpent_arc:       0.0,
                serpent_goal:      (0.0, 0.0),    serpent_band:      0.0,
                boss_hand_attack:  0,             boss_clap_wave:    0,
                boss_clap_at:      (0.0, 0.0),    boss_vent_hit_cooldown: 0,

                comet_live:        Vec::new(), comet_free:        comet_free.clone(),

                comet_warn_live:   Vec::new(), warn_free:         warn_free.clone(),
                comet_spawn_timer: COMET_SPAWN_INTERVAL,

                hearts:            start_max_hearts,
                max_hearts:        start_max_hearts,
                checkpoint_x:      start_hook.0,
                checkpoint_y:      start_hook.1,
                checkpoint_block:  0,
                respawn_active:    false,
                respawn_ticks:     0,
                player_buff:       0,
                buff_timer:        0,
                buff_hit_flash:    0,
                buff_absorbs:      crate::constants::BUFF_ABSORB_MAX,
                upgrade_live:      Vec::new(),
                upgrade_free:      upgrade_free.clone(),
                upgrade_rightmost: SPAWN_X + VW * 1.4,
                oxygen_drain_scale: 1.0,
                oxygen_drain_accum: 0.0,
                upgrade_momentum_bonus: false,
                run_heart_buys: 0,
                run_breath_buys: 0,
                run_momentum_buys: 0,
                run_heart_refill_buys: 0,
                run_magnet_buys: 0,
                upgrade_dialogue_active: false,
                upgrade_dialogue_node: String::new(),
                upgrade_hold_x: 0.0,
                upgrade_hold_y: 0.0,
                upgrade_hold_until_tether: false,
                space_visited: false,
                boss_killed: false,
                boss_index: 0,
                bosses_beaten: 0,
                boss_return_x: SPAWN_X,
                boss_return_y: SPAWN_Y,
                space_coins_collected: 0,
                flare_cooldown:    FLARE_INTERVAL_EASY as u32,
                flare_warn:        0,
                flare_active:      false,
                flare_active_ticks: 0,
                flare_damage_timer: 0,
                last_shield_x: SPAWN_X,
                frontier_repairs: 0,
                upgrade_hold_ticks: 0,
                eclipse_active: false,
                eclipse_t: 0.0,
                eclipse_shadow_ids: Vec::new(),
                eclipse_light_timer: 1,
                eclipse_node_buf: Vec::new(),
                perm_reach_mult: perm.reach_mult,
                perm_momentum_mult: perm.momentum_mult,
                perm_magnet_mult: perm.magnet_mult,
                perm_flare_wards: perm.flare_wards,
                flare_wards_left: perm.flare_wards,
                free_respawns_left: perm.free_respawns,
                flares_fired: 0,
                flare_hearts_lost: 0,
                flare_ticks_sheltered: 0,
                perk_release_mult: 1.0,
                perk_gravity_mult: 1.0,
                perk_buff_mult: 1.0,
                perk_coin_bonus: 0,
                perks_taken: 0,
                reward_offer: [0, 1, 2],
                node_offer: [2, 3, 4],
            };

            // The daily challenge's modifier changes the run's rules.
            let mut fresh_state = fresh_state;
            if let Some(ch) = daily {
                use crate::daily::Modifier::*;
                let s = &mut fresh_state;
                // Each one makes the run harder (they used to be half perks).
                match ch.modifier {
                    HeavyGravity => s.perk_gravity_mult *= 1.35,
                    ShortRope => s.perm_reach_mult *= 0.7,
                    GlassCat => { s.hearts = 1; s.max_hearts = 1; s.free_respawns_left = 0; }
                    SpeedLimit => s.perm_momentum_mult *= 0.75,
                    SolarStorm => { s.perm_flare_wards = 0; s.flare_wards_left = 0; }
                    LongHaul => { s.max_hearts = (s.max_hearts - 1).max(1); s.hearts = s.hearts.min(s.max_hearts); }
                }
                log::info!("daily challenge: {} to {} px", ch.modifier.name(), ch.goal);
            }
            // SOLAR STORM drives the flare clock through the flare system's
            // interval override; any other run clears it if a storm set it.
            let storm = daily.is_some_and(|ch| ch.modifier == crate::daily::Modifier::SolarStorm);
            if storm {
                canvas.set_var("debug_flare_interval", 25 * 60i32);
                canvas.set_var("daily_set_flares", true);
            } else if matches!(canvas.get_var("daily_set_flares"), Some(Value::Bool(true))) {
                canvas.set_var("debug_flare_interval", 0i32);
                canvas.set_var("daily_set_flares", false);
            }

            // Reuse persistent Arc across respawns.
            {
                let mut slot = persistent_state.lock().unwrap();
                if let Some(existing) = slot.as_ref() {
                    *existing.lock().unwrap() = fresh_state;
                } else {
                    *slot = Some(Arc::new(Mutex::new(fresh_state)));
                }
            }

            // Test hook: drop the run partway along the curve.
            //
            // Seeding `distance` alone was not enough — it gates the hazard
            // schedule, but the boss threshold and the eclipse that precedes it
            // are POSITION-based, so a harness that only moved the counter could
            // never reach them. Move the player and every spawn frontier too, so
            // the world generates around them from the first frame.
            if seeded_distance > 0.0 {
                let slot = persistent_state.lock().unwrap();
                if let Some(state_arc) = slot.as_ref() {
                    let mut s = state_arc.lock().unwrap();
                    let at = SPAWN_X + seeded_distance;
                    s.px = at;
                    s.hook_x = at;
                    s.rightmost_x = at;
                    s.gen_head_x = at;
                    // The reach clamp measures every new node against the last
                    // PLACED one. Left at the starter hook it pinned all forty
                    // seeded nodes back to ~3 800 px, leaving the seeded player
                    // in an empty world — the clamp doing exactly its job
                    // against stale input.
                    s.last_hook_x = at;
                    s.pending.clear();
                    let mut seed = s.seed;
                    let mut ghx = at;
                    let mut ghy = s.gen_head_y;
                    let anchor_y = ghy;
                    let batch = gen_hook_batch(&mut seed, at, &mut ghx, &mut ghy, seeded_distance);
                    s.seed = seed;
                    s.gen_head_x = ghx;
                    s.gen_head_y = ghy;
                    // A node where the seeded player orbits: the chain's first
                    // hop is over a rope length, so without it the player
                    // started with nothing in reach.
                    s.pending = std::iter::once(HookSpec { x: at, y: anchor_y, launch: false }).chain(batch).collect();
                    s.pad_rightmost = at;
                    s.spinner_rightmost = at;
                    s.coin_rightmost = at;
                    s.flip_rightmost = at;
                    s.score_x2_rightmost = at;
                    s.zero_g_rightmost = at;
                    s.gate_rightmost = at;
                    s.gwell_rightmost = at;
                    s.turret_rightmost = at;
                    s.cannon_rightmost = at;
                    s.rocket_pad_rightmost = at;
                    s.upgrade_rightmost = at;
                    s.space_asteroid_rightmost = at;
                    s.last_shield_x = at;
                    s.checkpoint_x = at;
                    s.py = start_py;
                    s.hook_y = start_py;
                    s.last_hook_y = start_py;
                    s.gen_head_y = start_py;
                    // Advance past any fights this position is already beyond,
                    // or the first one triggers on frame 1 and the seeded run
                    // never samples the world it was pointed at.
                    let mode = match canvas_mode_idx {
                        Some(i) => crate::mode::GameMode::from_index(i),
                        None => crate::mode::GameMode::default(),
                    };
                    let mut idx = 0u32;
                    while let Some(d) = crate::mode::boss_trigger_distance(mode, idx) {
                        if seeded_distance > d { idx += 1; } else { break; }
                    }
                    s.boss_index = idx;
                }
            }
            if seeded_distance > 0.0 {
                // Skip the "hold space to begin" standby orbit. It orbits the
                // starter hook, which a seeded run has been teleported away
                // from, so the run would sit in the intro forever and never
                // reach the content being sampled.
                canvas.set_var("start_prompt_active", false);
                canvas.set_var("game_paused", false);
                canvas.set_var("input_needs_edge_reset", false);
                canvas.set_var("start_orbit_ticks", 0i32);
                if let Some(obj) = canvas.get_game_object_mut("start_prompt_text") {
                    obj.visible = false;
                }
            }
            let state =
                persistent_state.lock().unwrap().as_ref().unwrap().clone();

            let (start_px, start_py) = if seeded_distance > 0.0 {
                (SPAWN_X + seeded_distance, start_py)
            } else {
                (start_px, start_py)
            };
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
                        if crate::constants::mobile_controls_enabled() { "HOLD SPACE TO BEGIN" } else { "HOLD SPACE OR CLICK TO BEGIN" },
                        &font,
                        52.0 * s,
                        Color(235, 245, 255, 255),
                        2000.0 * s,
                    )));
                    obj.visible = true;
                }

                if let Some(obj) = canvas.get_game_object_mut("cannon_prompt_text") {
                    obj.set_drawable(Box::new(ui_text_spec(
                        "PRESS  F  :  FAST TRAVEL",
                        &font,
                        44.0 * s,
                        Color(170, 240, 255, 255),
                        1500.0 * s,
                    )));
                    obj.visible = false;
                }

                if let Some(obj) = canvas.get_game_object_mut(TOAST_TITLE_NAME) {
                    obj.set_drawable(Box::new(ui_text_left_spec(
                        " ",
                        &font,
                        46.0 * s,
                        Color(250, 225, 120, 255),
                        1080.0 * s,
                    )));
                    obj.visible = false;
                }

                if let Some(obj) = canvas.get_game_object_mut("boss_name_text") {
                    obj.set_drawable(Box::new(ui_text_spec(
                        BOSS_NAME,
                        &font,
                        42.0 * s,
                        Color(200, 60, 220, 255),
                        1000.0 * s,
                    )));
                    obj.visible = false;
                }
                if let Some(obj) = canvas.get_game_object_mut(TOAST_DESC_NAME) {
                    obj.set_drawable(Box::new(ui_text_left_spec(
                        " ",
                        &font,
                        28.0 * s,
                        Color(210, 220, 235, 230),
                        1080.0 * s,
                    )));
                    obj.visible = false;
                }
                if let Some(obj) = canvas.get_game_object_mut(TOAST_CHECK_NAME) {
                    let sym = super::helpers::symbol_font().unwrap_or_else(|| font.clone());
                    obj.set_drawable(Box::new(ui_text_spec(
                        "✓",
                        &sym,
                        52.0 * s,
                        Color(130, 255, 165, 255),
                        120.0 * s,
                    )));
                    obj.visible = false;
                }
            }
            if let Some(obj) = canvas.get_game_object_mut("pause_overlay") {
                obj.position = super::bootstrap::PAUSE_OVERLAY_POS;
                obj.visible = false;
            }

            clear_toast(canvas);
            crate::achievements::on_run_start();
            super::juice::reset();
            super::hangar::reset();
            canvas.set_var(super::perks::REWARD_PENDING_VAR, false);
            super::perks::hide_reward(canvas);
            boss::intro::reset(canvas);
            boss::intro::prewarm();

            __prof("resets+intro");
            // Set background image AND apply the proper overscan/raise size so
            // the background fills the screen correctly from the first frame.
            {
                let bg_sel = match canvas.get_var("player_bg_selected") {
                    Some(Value::I32(v)) => v.max(0) as usize,
                    _ => 0,
                }.min(SHOP_BG_COLORS.len() - 1);
                let (sky, _, shown) = sky_for(crate::sky::planet_of(canvas), bg_sel);
                canvas.set_var("sky_planet_shown", shown as i32);
                if let Some(obj) = canvas.get_game_object_mut("bg") {
                    obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: sky.into(), color: None });
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

            __prof("sky image");
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

            __prof("spawning");
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
            // Reset the hold-to-start counter so a fresh run (incl. a retry
            // click) must again hold for START_HOLD_TICKS. A quick click still
            // can't auto-start because releasing resets the counter below
            // START_HOLD_TICKS.
            canvas.set_var("start_hold_ticks", 0i32);
            // Do NOT clear `mouse_left_held` here: if the player is still
            // holding the click that navigated here (e.g. the game-over RETRY
            // button), letting it stand means the hold-to-start counts from the
            // moment the title appears, so "click and keep holding" starts the
            // run instead of silently doing nothing. A quick click is harmless:
            // the release immediately drops the counter back to 0.
            // Reset the grab edge detector so the first launched frame always
            // sees the held input as a fresh press and auto-grabs — regardless
            // of the stale `_was_down` state carried over from a previous run.
            canvas.set_var("input_needs_edge_reset", true);
            // Do not hard-pause the engine here: hard-pause skips
            // apply_camera_transform, which can leave stale zoom from
            // the previous scene on screen. We gate gameplay with
            // `game_paused`/`start_prompt_active` instead.
            canvas.resume();

            __prof("start setup");
            // ── Pre-warm rope texture cache (background thread) ──────────
            {
                let color = match canvas.get_var("player_rope_selected") {
                    Some(Value::I32(v)) => v.max(0) as usize,
                    _ => 0,
                };
                physics::prewarm_rope_fx_cache(color);
            }
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

            __prof("prewarms");
            // ── Register grab/release events + mouse handlers ────────────
            events::register_events(canvas, &state);

            // Pause menu + touch input handlers are registered at App::new,
            // not here — see `register_pause_ui_handlers`.

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
                let mut n_was_down = false;
                let mut nebula_tick = super::nebula::NebulaTick::default();
                let mut planet_tick = super::planet::PlanetTick::default();
                let mut aurora_tick = super::aurora::AuroraTick::default();
                let mut scroll_init = false;
                let mut prev_scroll_in_space: Option<bool> = None;
                let mut prev_player_center: Option<(f32, f32)> = None;
                let mut prev_warp_flash = false;

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
                    // Dead, the run goes on without the player: the fight
                    // (a boss keeps moving and attacking) and its hit bursts,
                    // under the death screen. Nothing else of the run ticks.
                    {
                        let dead = st.lock().unwrap().dead;
                        if dead {
                            boss::tick_boss(c, &st);
                            boss::common::tick_impacts(c, &st);
                            super::death_screen::tick(c);
                            return;
                        }
                    }
                    // The boss reward cards rise in; the boss title card plays.
                    super::perks::tick_reward(c, &st);
                    boss::intro::tick(c);
                    super::tutorial::tick(c, &st);
                    if !c.get_bool("game_paused") { super::juice::tick(c, &st); }

                    // ── Achievements: pop toasts mid-run (like gold-master) ──
                    // Gold-master fires on coin pickup; the lifetime achievements
                    // only fired on death, so their toasts never showed. Poll
                    // the live run stats here so they pop while the run is live.
                    if !c.get_bool("game_paused") && !c.get_bool("start_prompt_active") {
                        let (run_coins, run_dist) = {
                            let s = st.lock().unwrap();
                            (s.coin_count as u64, s.distance)
                        };
                        let ticks = c.get_i32("achievement_check_ticks").saturating_add(1);
                        c.set_var("achievement_check_ticks", ticks);
                        if ticks % 30 == 0 {
                            crate::achievements::check_achievements(c, run_coins, run_dist);
                        }
                        crate::daily::tick(c, run_dist);
                        let (boss_up, alive) = { let s = st.lock().unwrap(); (s.boss_active && s.boss_spawned, !s.dead) };
                        if alive {
                            let aurora_up = c.get_bool("aurora_up");
                            crate::achievements::tick(c, boss_up, aurora_up);
                        }
                    }

                    // ── Hearts / checkpoint bookkeeping ──────────────────
                    super::hearts::tick_hearts(c, &st);

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
                                c.get_game_object_mut("pause_panel")
                            {
                                let (px, py) = super::bootstrap::PAUSE_PANEL_POS;
                                obj.position = (px, py + y);
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
                                    c.get_game_object_mut("pause_panel")
                                {
                                    obj.position = super::bootstrap::PAUSE_PANEL_POS;
                                }
                                for &(name, bx, by) in PAUSE_BTN_LAYOUT.iter() {
                                    if let Some(obj) = c.get_game_object_mut(name) {
                                        obj.position = (bx, by);
                                    }
                                }
                                c.set_var("pause_animating", false);
                                c.set_var("game_paused", true);
                                c.set_var("pause_menu_open", true);
                                // Hold the music with the clock. `c.pause()`
                                // below stops the tick, and a track that keeps
                                // running past it comes back out of phase with
                                // the beat the fight is scored on.
                                crate::audio_state::pause_game_bgm();
                                // The pause menu must not be dimmed by a boss
                                // darkness phase; restore full brightness.
                                if c.has_lighting() {
                                    c.set_ambient(Color(255, 255, 255, 255), 1.0);
                                }
                                c.pause();
                            }
                            return;
                        }
                        c.set_var("pause_animating", false);
                    }

                    if is_game_paused(c) {
                        // ── Upgrade stasis: tether out ─────────────────────────
                        // This block RETURNS before the normal input handling
                        // further down, so anything that needs input while the
                        // world is soft-paused has to poll it here. The start
                        // prompt already did; the post-upgrade hold did not, so
                        // the only way out of it — a grab — could never fire and
                        // the dialogue locked the run every single time.
                        {
                            let holding = {
                                let s = st.lock().unwrap();
                                s.upgrade_hold_until_tether && !s.upgrade_dialogue_active
                            };
                            if holding {
                                let action_now = c.key("space")
                                    || matches!(c.get_var("mouse_left_held"), Some(Value::Bool(true)));
                                let was = matches!(
                                    c.get_var("upgrade_hold_input_was"),
                                    Some(Value::Bool(true))
                                );
                                c.set_var("upgrade_hold_input_was", action_now);
                                if action_now && !was {
                                    c.run(Action::Custom { name: "do_grab".into() });
                                }
                                // Backstop, also unreachable from its old home
                                // below this return: never let the hold trap a run.
                                let stuck = {
                                    let mut s = st.lock().unwrap();
                                    s.upgrade_hold_ticks = s.upgrade_hold_ticks.saturating_add(1);
                                    s.upgrade_hold_ticks > UPGRADE_HOLD_MAX_TICKS
                                };
                                if stuck {
                                    {
                                        let mut s = st.lock().unwrap();
                                        s.upgrade_hold_until_tether = false;
                                        s.upgrade_hold_ticks = 0;
                                    }
                                    c.set_var("game_paused", false);
                                    c.set_var("upgrade_hold_released", true);
                                }
                            } else {
                                c.set_var("upgrade_hold_input_was", false);
                            }
                        }

                        // ── SEED FUNDS paid in on the start screen ─────────────
                        super::seed_funds::tick(c, &st);

                        // ── Hold space/mouse to begin ──────────────────────────
                        // The run begins only after HOLDING space/mouse for
                        // START_HOLD_TICKS, so a stray or retry click doesn't
                        // instantly launch + grab.
                        if c.get_bool("start_prompt_active") {
                            let space_held = c.key("space");
                            let mouse_held = matches!(c.get_var("mouse_left_held"), Some(Value::Bool(true)));
                            let hold = space_held || mouse_held;
                            let h = match c.get_var("start_hold_ticks") { Some(Value::I32(v)) => v, _ => 0 };
                            if hold {
                                c.set_var("start_hold_ticks", h + 1);
                                if h + 1 >= START_HOLD_TICKS {
                                    // Launch the run.
                                    let ticks = c.get_i32("start_orbit_ticks").max(0) as f32;
                                    c.set_var("pause_anim_frames", 0);
                                    c.set_var("game_paused", false);
                                    c.set_var("start_prompt_active", false);
                                    // A mouse hold that started the run is not a
                                    // swing on a computer (see `events.rs`).
                                    if !crate::constants::mobile_controls_enabled() {
                                        c.set_var("mouse_left_held", false);
                                    }
                                    hide_pause_ui(c);
                                    rebuild_player_trail(c, selected_trail_color(c));
                                    if let Some(obj) = c.get_game_object_mut("player") {
                                        obj.visible = true;
                                    }
                                    let was_hooked = matches!(c.get_var("rope_visible_at_pause"), Some(Value::Bool(true)));
                                    if let Some(obj) = c.get_game_object_mut("rope") {
                                        obj.visible = was_hooked;
                                    }
                                    launch_from_orbit(c, &st, ticks);
                                    return;
                                }
                            } else {
                                c.set_var("start_hold_ticks", 0);
                            }
                        }
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
                        // ── Upgrade dialogue / post-dialogue hold ────────
                        // World is paused here; hold the player at the node so
                        // they can choose without falling (or being hit).
                        {
                            let (dialogue, hold) = {
                                let s = st.lock().unwrap();
                                (s.upgrade_dialogue_active, s.upgrade_hold_until_tether)
                            };
                            if dialogue {
                                let (nx, ny) = {
                                    let s = st.lock().unwrap();
                                    (s.upgrade_hold_x, s.upgrade_hold_y)
                                };
                                if let Some(p) = c.get_game_object_mut("player") {
                                    p.position = (nx - PLAYER_R, ny - PLAYER_R);
                                    p.momentum = (0.0, 0.0);
                                    p.gravity = 0.0;
                                }
                            } else if hold {
                                if let Some(p) = c.get_game_object_mut("player") {
                                    p.momentum = (0.0, 0.0);
                                    p.gravity = 0.0;
                                }
                            }
                        }
                        // (The dim is `ignore_zoom`, pinned to the screen: it
                        // used to be shoved along by the camera's x here,
                        // which slid it off one side of the screen.)
                        // The sky stays alive behind the start prompt and pause.
                        super::aurora::tick_aurora(c, &st, &mut aurora_tick);
                        super::sky_extras::tick(c, &st);
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
                        s.grab_lockout_ticks = s.grab_lockout_ticks.saturating_sub(1);
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
                    // Skipped in boss mode so arena tether nodes can reach the
                    // upper-sky boss band.
                    if c.get_bool("asteroid_hooks_on") && !st.lock().unwrap().boss_active {
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

                    // ── Cannon fast-travel warp (two-phase speed lines) ────
                    gravity_cannon::tick_cannon_warp(c, &st);
                    // ── Gravity cannons ───────────────────────────────────
                    gravity_cannon::tick_cannons(c, &st);

                    // ── Cannon fast-travel prompt ─────────────────────────
                    // Show "PRESS F: FAST TRAVEL" while the player is held and
                    // can afford the hyper-transit; hide it otherwise.
                    {
                        let show = st.lock().unwrap().cannon_ft_prompt;
                        if let Some(obj) = c.get_game_object_mut("cannon_prompt_text") {
                            obj.visible = show;
                        }
                    }
                    // ── The equipped ability: its bar and button ──────────
                    super::abilities::tick(c, &st);
                    // ── Boss fight ────────────────────────────────────────
                    boss::tick_boss(c, &st);
                    // Hit bursts, after the fight so a hit registered this
                    // tick is already advancing on the frame it happens.
                    // Outside the boss dispatch because the player can take
                    // damage anywhere, not only in an arena.
                    boss::common::tick_impacts(c, &st);

                    // ── Roguelike upgrade nodes (spend coins) ──────────────
                    upgrades::tick_upgrades(c, &st);
                    super::eclipse::tick_eclipse(c, &st);

                    // ── Upgrade dialogue / post-dialogue stasis hold ───────
                    // (The world is soft-paused while this is true, so this is a
                    // backstop: hold the player at the stored node position.)
                    {
                        let (dialogue, hold) = {
                            let s = st.lock().unwrap();
                            (s.upgrade_dialogue_active, s.upgrade_hold_until_tether)
                        };
                        if dialogue {
                            let (nx, ny) = {
                                let s = st.lock().unwrap();
                                (s.upgrade_hold_x, s.upgrade_hold_y)
                            };
                            if let Some(p) = c.get_game_object_mut("player") {
                                p.position = (nx - PLAYER_R, ny - PLAYER_R);
                                p.momentum = (0.0, 0.0);
                                p.gravity = 0.0;
                            }
                            {
                                let mut s = st.lock().unwrap();
                                s.px = nx;
                                s.py = ny;
                                s.vx = 0.0;
                                s.vy = 0.0;
                            }
                        } else if hold {
                            if let Some(p) = c.get_game_object_mut("player") {
                                p.momentum = (0.0, 0.0);
                                p.gravity = 0.0;
                            }
                        }
                    }

                    // ── Fast-travel reveal flash ──────────────────────────
                    // When the warp speed-lines end, fire a white screen flash
                    // so the caught player is revealed (rather than a hard cut),
                    // and hold the receiving cannon at its windup for a few
                    // ticks during the reveal before it settles.
                    {
                        let warp_now = matches!(c.get_var("warp_flash_ticks"), Some(Value::I32(v)) if v > 0);
                        if prev_warp_flash && !warp_now {
                            if matches!(c.get_var("cannon_ft_reveal"), Some(Value::Bool(true))) {
                                c.set_var("cannon_ft_reveal", false);
                                c.set_var("cannon_ft_reveal_ticks", 8i32);
                                if let Some(cam) = c.camera_mut() {
                                    cam.flash_with(Color(255, 255, 255, 215), 0.5, FlashMode::FadeOut, FlashEase::Smooth, 1.0, 0.10);
                                }
                            }
                        }
                        prev_warp_flash = warp_now;
                    }

                    // ── Space zone ────────────────────────────────────────
                    super::space_zone::tick_space_zone(c, &st, frame_counter);

                    // ── Distance tracking ────────────────────────────────
                    {
                        let mut s = st.lock().unwrap();
                        // Boss arenas sit at their own stretch of world X, far
                        // past the level, and the player is warped there — so
                        // arena X is not progress. Counting it sent `distance`
                        // (and with it the difficulty curve AND the boss
                        // schedule) straight to maximum on the first fight.
                        // It also matches the design intent: the hour-long
                        // curve is an hour of SWINGING, boss fights excluded.
                        if !s.in_space_mode && !s.space_launch_active && !s.boss_active {
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
                        nebula_tick.refresh(); // and the sky's (a re-entered scene has blank objects)
                        planet_tick.refresh();
                        c.set_var("bg_force_refresh", false);
                    }
                    {
                        let imgs = BG_IMAGES.get_or_init(compute_bg_images);
                        let bg_sel = match c.get_var("player_bg_selected") {
                            Some(Value::I32(v)) => v.max(0) as usize,
                            _ => 0,
                        }.min(imgs.palettes.len().saturating_sub(1));
                        // A SKY planet's own sky (an Arc: nothing copied per frame).
                        let planet = crate::sky::planet_of(c);
                        let pair = if planet != 0 { planet_sky_ready(planet, bg_sel) } else { None };
                        c.set_var("sky_planet_shown", if pair.is_some() { planet as i32 } else { 0 });
                        let (sky, sky_flip) = match &pair {
                            Some(p) => (&p.0, &p.1),
                            None => (&imgs.palettes[bg_sel], &imgs.palettes_flip[bg_sel]),
                        };
                        background::tick_background(
                            c, &st, &mut prev_bg_theme, &mut bg_scale_smooth,
                            sky, &imgs.vivid,
                            sky_flip, &imgs.vivid_flip,
                        );
                        super::water::tick(c, &st);
                        super::planet::tick(c, &st, &mut planet_tick);
                    }

                    // ── Background star parallax ──────────────────────────
                    // Panels track bg's live size/position so stars scale exactly
                    // with the zoom effect. Modulus stays fixed at VW (constant)
                    // to prevent offset jumps; the result is then scaled to bg_w.
                    {
                        let in_space = { st.lock().unwrap().in_space_mode };
                        let n_now = c.key("n");
                        if n_now && !n_was_down { star_auto_scroll = false; }
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
                            for id in ["bg_stars_b", "bg_stars_up_a", "bg_stars_up_b"] {
                                if let Some(obj) = c.get_game_object_mut(id) {
                                    obj.set_image(img.clone());
                                }
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
                        // The second row, on the sky's far side: the zoom-out
                        // reveals sky there, and one row stops short of it.
                        let flipped = { st.lock().unwrap().gravity_dir < 0.0 };
                        let py2 = if flipped { py + bg_h } else { py - bg_h };
                        for (id, x) in [("bg_stars_up_a", px), ("bg_stars_up_b", px + bg_w)] {
                            if let Some(obj) = c.get_game_object_mut(id) {
                                obj.size = (bg_w, bg_h);
                                obj.position = (x, py2);
                                obj.visible = true;
                            }
                        }
                    }

                    // ── Nebula backdrop (F cycles the set) ──────────────
                    super::nebula::tick_nebula(c, &st, &mut nebula_tick);
                    super::aurora::tick_aurora(c, &st, &mut aurora_tick);
                    super::sky_extras::tick(c, &st);

                    // ── Distant decor (planets, wrecks, the odd whale) ──
                    super::decor::tick_decor(c, &st);
                    super::hangar::tick(c, &st);
                    super::mystic_trail::tick(c, &st);
                    super::space_zone::tick_vortex_fx(c, &st);

                    // ── Death check ──────────────────────────────────────
                    let mut s = st.lock().unwrap();
                    // Solar death: set by tick_space_zone when player reaches solar ceiling.
                    let died_to_sun = c.get_bool("died_to_sun");
                    let died_to_oxygen = c.get_bool("died_to_oxygen");
                    let dead_now = !s.god_mode && (died_to_sun || died_to_oxygen || s.hearts <= 0
                        || (s.gravity_dir > 0.0
                        && s.py > VH + 150.0)
                        || (s.gravity_dir < 0.0 && s.py < s.fall_ceiling_y()));
                    if dead_now {
                        // ── Falls can be survived with hearts remaining ──
                        let is_fall = !died_to_sun && !died_to_oxygen;
                        // Headless filming only (`HEADLESS_FREE_FALLS`): a fall
                        // with hearts left costs none, so the clip's hit is
                        // the boss's. Never set by the game.
                        let free_fall = s.hearts > 0
                            && matches!(c.get_var("debug_free_falls"), Some(Value::Bool(true)));
                        if is_fall {
                            drop(s);
                            let over = if free_fall { false } else { super::hearts::lose_heart(c, &st) };
                            if !over {
                                // In the boss arena, reset into the stasis orbit
                                // (boss progress preserved) instead of respawning
                                // at the outer checkpoint (which would strand the
                                // player at the wall, unable to re-enter).
                                if st.lock().unwrap().boss_active {
                                    boss::reset_boss_after_fall(c, &st);
                                } else {
                                    super::hearts::respawn(c, &st);
                                }
                                return;
                            }
                            // Last heart spent → fall through to game over.
                            s = st.lock().unwrap();
                        }

                        c.set_var("last_distance", s.distance);
                        c.set_var("last_score", s.score as i32);
                        c.set_var("last_coins", s.coin_count as i32);
                        // The run summary's numbers (see `death_screen`).
                        c.set_var("sum_secs", (s.ticks / 60) as i32);
                        c.set_var("sum_perks", s.perks_taken as i32);
                        c.set_var("sum_bosses", s.bosses_beaten as i32);
                        c.set_var("sum_chain", super::juice::best_chain() as i32);
                        {
                            let best = crate::profile::profile().lock().unwrap().best_distance as f32;
                            c.set_var("sum_new_best", s.distance > best && s.distance > 1000.0);
                        }
                        // Award permanent meta currency at the end of a run,
                        // based on distance travelled (normal-zone only — space
                        // X distance does NOT count) plus flat bonuses for
                        // visiting space and defeating the boss.
                        let distance_meta = s.distance as u64 / 5000; // 1 per 5000px
                        let space_bonus = if s.space_visited { 20 } else { 0 };
                        // Boss meta is awarded (and shown) at the moment of the
                        // victory stasis — don't double it at run end.
                        let boss_bonus = 0;
                        // Mode scales the payout: Normal is the efficient way
                        // to earn, Casual still funds progress (a mode that
                        // pays nothing reads as a dead end), Boss Rush sits
                        // just under Normal.
                        let mode = crate::mode::current_mode(c);
                        let raw = distance_meta + space_bonus + boss_bonus;
                        let meta_gain = (raw as f32 * mode.meta_multiplier()).round() as u64;
                        if meta_gain > 0 {
                            crate::profile::award_meta_currency(meta_gain);
                        }
                        // Record lifetime run stats on the active profile.
                        let run_distance = s.distance;
                        let run_coins = s.coin_count as u64;
                        let run_seconds = (s.ticks / 60) as u64;
                        let run_extras = s.run_stats;
                        let (run_score, run_bosses) = (s.score, s.bosses_beaten);
                        s.dead = true;
                        drop(s);
                        crate::profile::record_death();
                        let mode_idx = crate::mode::current_mode(c).index();
                        let place = crate::profile::record_run(mode_idx, run_distance, run_coins, run_seconds, run_extras, run_score, run_bosses);
                        if std::env::var("RUN_RECORD_LOG").is_ok() {
                            eprintln!("RUN_RECORD mode {mode_idx} dist {run_distance:.0} score {run_score} bosses {run_bosses} secs {run_seconds} place {place:?}");
                        }
                        // The summary says so when a run makes the leaderboard.
                        c.set_var("sum_board_place", place.map_or(0, |p| p as i32));
                        crate::daily::end_run(c, run_distance);
                        // Run is already recorded, so no extra run coins/distance.
                        crate::achievements::check_achievements(c, 0, 0.0);
                        // The eclipse and the boss darkness are cleaned up
                        // when the death screen is left (the game scene's
                        // exit), not here: the fight stays on screen under
                        // it, and full daylight over the arena's lights
                        // washed the whole frame out white.
                        c.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
                        c.remove_emitter(PLAYER_TRAIL_MID_NAME);
                        c.remove_emitter(PLAYER_TRAIL_ACCENT_NAME);
                        super::mystic_trail::set(c, false);
                        if let Some(obj) =
                            c.get_game_object_mut("player")
                        {
                            obj.visible = false;
                            // Parked: the run carries on around it.
                            obj.momentum = (0.0, 0.0);
                            obj.gravity = 0.0;
                        }
                        // A buff in hand would let the body land hits.
                        {
                            let mut s = st.lock().unwrap();
                            s.retire_buff();
                            s.hooked = false;
                        }
                        if let Some(obj) =
                            c.get_game_object_mut("rope")
                        {
                            obj.visible = false;
                        }
                        // The boss's music plays on under the death screen;
                        // the game scene's exit hands the lane its music back.
                        //
                        // The run stays on screen, Terraria-style: the death
                        // screen fades in over it (see `death_screen`).
                        let boss_name = {
                            let s = st.lock().unwrap();
                            (s.boss_active && s.boss_spawned).then(|| s.boss_kind.name())
                        };
                        let (cause, line) = if died_to_sun {
                            ("sun", "You flew too close to the sun".to_string())
                        } else if died_to_oxygen {
                            ("oxygen", "You ran out of oxygen".to_string())
                        } else if let Some(name) = boss_name {
                            ("fall", format!("Slain by {}", title_case(name)))
                        } else {
                            ("fall", "You fell".to_string())
                        };
                        c.set_var("died_to_sun", false);
                        c.set_var("died_to_oxygen", false);
                        c.set_var("death_cause", Value::Str(cause.to_string()));
                        let eclipse = c.get_bool("eclipse_active");
                        crate::achievements::on_death(c, cause, boss_name.is_some(), eclipse);
                        play_death_sound(c);
                        super::death_screen::begin(c, &line);
                    }
                });
                canvas.set_var("game_tick_registered", true);
            }
        })
        .on_exit(move |canvas| {
            super::tutorial::end(canvas);
            canvas.run(Action::DetachEmitter {
                emitter_name: PLAYER_TRAIL_EMITTER_NAME.to_string(),
            });
            canvas.run(Action::DetachEmitter {
                emitter_name: PLAYER_TRAIL_MID_NAME.to_string(),
            });
            canvas.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
            canvas.remove_emitter(PLAYER_TRAIL_MID_NAME);
            canvas.remove_emitter(PLAYER_TRAIL_ACCENT_NAME);
            super::mystic_trail::set(canvas, false);

            // Put the frame back, whatever the reason for leaving.
            //
            // The night-mode post pass is a SINGLE GLOBAL SLOT, so a scene that
            // leaves it on hands a darkened, bloomed frame to every scene after
            // it. Dying during an eclipse or a boss's darkness phase did
            // exactly that: the death path loads the gameover scene directly
            // and never went near `end_night_mode`, so the shader stayed on.
            //
            // Here rather than on each exit path, because there are four of
            // them (fall, sun, oxygen, menu) and the next one added would not
            // know it had to do this either.
            super::eclipse::end_night_mode(canvas);
            // The ability button lives on the canvas, not in the scene.
            super::abilities::hide(canvas);
            // Leaving from the death screen: take it down, and hand the lane
            // its music back from a boss loop still playing under it (a
            // no-op otherwise).
            if super::death_screen::active(canvas) {
                super::death_screen::remove(canvas);
                crate::scenes::game::boss::hide_conductor(canvas);
            }
            // Put out everything the run lit, however it is left — the death
            // screen, the pause menu, mid-fight. Lighting stays on for the
            // whole session (it is enabled at app start), so a light left on
            // here went on costing every frame of the menu: leaving the Sun
            // Devourer's fight left its boss light, its bolts' lights and its
            // darkness all running behind the menu. Only a fight that ENDED
            // used to clean these up.
            //
            // The eclipse first: `tick_eclipse` does not run once the scene
            // is gone, so its lights/post-override would leak into the next
            // run (boss-rush restart → acting as if still in an eclipse).
            if let Some(st) = run_state().lock().unwrap().as_ref().cloned() {
                super::eclipse::end_eclipse(canvas, &st);
            }
            super::eclipse::release_nodes_from_dark(canvas);
            if canvas.has_lighting() {
                canvas.set_ambient(Color(255, 255, 255, 255), 1.0);
                canvas.set_light_enabled("boss_light", false);
                for i in 0..BOSS_BOLT_POOL_SIZE {
                    canvas.set_light_enabled(&format!("boss_bolt_light_{i}"), false);
                }
            }
        })
}

/// "THE COLOSSUS" -> "The Colossus".
fn title_case(s: &str) -> String {
    s.split(' ').map(|w| {
        let mut ch = w.chars();
        match ch.next() {
            Some(f) => f.to_string() + &ch.as_str().to_lowercase(),
            None => String::new(),
        }
    }).collect::<Vec<_>>().join(" ")
}
