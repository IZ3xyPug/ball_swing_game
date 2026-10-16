//! What happens in the sky over certain worlds, and SKY EVENTS.
//!
//! Everything is screen-space and laid on the sky through `bg`'s rect, as
//! the nebula is, so it zooms, drops for a boss and flips with the sky. The
//! art is drawn here from noise, once, and kept.
//!
//! * WEATHER, over the worlds that have it (the gas giant, the toxic world):
//!   lightning flickering in the clouds and a storm's eye turning slowly on
//!   the face (drifting as the planet turns under you).
//! * SKY EVENTS (a shop toggle): a meteor shower every minute or two. M starts
//!   one at once, for looking at it.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;

type Img = Arc<image::RgbaImage>;

// ── Noise ────────────────────────────────────────────────────────────────────

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
    for o in 0..5 {
        sum += value(x * f, y * f, seed + o * 17) * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2.03;
    }
    sum / norm
}
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}
fn rgba(c: [f32; 3], a: f32) -> image::Rgba<u8> {
    image::Rgba([c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8, (a.clamp(0.0, 1.0) * 255.0) as u8])
}

// ── The art ──────────────────────────────────────────────────────────────────

fn cached(cell: &'static OnceLock<Img>, f: impl FnOnce() -> image::RgbaImage) -> Img {
    cell.get_or_init(|| Arc::new(f())).clone()
}

/// A soft glow, white (tinted where it is used), built flat in the shape it
/// is drawn: the renderer fills a box by cropping an image to it, not
/// stretching, so a round image in a wide box showed only its middle.
const GLOW_SIZE: (u32, u32) = (256, 90);
fn glow_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || image::RgbaImage::from_fn(GLOW_SIZE.0, GLOW_SIZE.1, |x, y| {
        let (hw, hh) = (GLOW_SIZE.0 as f32 * 0.5, GLOW_SIZE.1 as f32 * 0.5);
        let d = (((x as f32 + 0.5 - hw) / hw).powi(2) + ((y as f32 + 0.5 - hh) / hh).powi(2)).sqrt();
        let a = (1.0 - d).max(0.0).powf(1.3);
        rgba([255.0; 3], a)
    }))
}

/// The storm's eye, one frame per step of its turn: spiral cloud bands round
/// a clear centre, built already squashed into perspective (the shape it is
/// drawn at; see `glow_img`).
const EYE_FRAMES: usize = 24;
const EYE_SIZE: (u32, u32) = (640, 62);
fn eye_frames() -> &'static Vec<Img> {
    static F: OnceLock<Vec<Img>> = OnceLock::new();
    F.get_or_init(|| (0..EYE_FRAMES).map(|f| {
        let turn = f as f32 / EYE_FRAMES as f32 * std::f32::consts::TAU / 2.0; // two arms: half a turn repeats
        Arc::new(image::RgbaImage::from_fn(EYE_SIZE.0, EYE_SIZE.1, |x, y| {
            let (u, v) = ((x as f32 + 0.5) / EYE_SIZE.0 as f32 * 2.0 - 1.0, (y as f32 + 0.5) / EYE_SIZE.1 as f32 * 2.0 - 1.0);
            let r = (u * u + v * v).sqrt();
            if r > 1.0 { return image::Rgba([0, 0, 0, 0]); }
            // Soft, ragged bands winding in: cloud, not a drawn spiral.
            let ang = v.atan2(u) - turn + r * 5.0;
            let n = fbm(u * 4.0 + r * 2.0, v * 4.0, 401);
            let fine = fbm(u * 11.0, v * 11.0, 409);
            let arms = smooth(0.15, 0.95, 0.5 + 0.4 * (ang * 2.0).sin() + (n - 0.5) * 1.2);
            let eye = smooth(0.05, 0.2, r);
            let a = arms * eye * (1.0 - smooth(0.35, 1.0, r)) * (0.55 + 0.45 * fine) * 0.75;
            rgba(mix3([200.0, 205.0, 220.0], [255.0, 255.0, 255.0], n), a)
        }))
    }).collect())
}

/// A meteor: a streak, bright at its head (the right end), fading behind.
fn meteor_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || image::RgbaImage::from_fn(512, 24, |x, y| {
        let t = x as f32 / 511.0;
        let across = (-(((y as f32 - 11.5) / (2.0 + 4.0 * t)).powi(2))).exp();
        let head = (-((1.0 - t) / 0.04).powi(2)).exp();
        let a = (t.powf(2.2) * 0.8 + head) * across;
        rgba(mix3([255.0, 190.0, 140.0], [255.0, 255.0, 255.0], t), a)
    }))
}

// ── Shop pictures ────────────────────────────────────────────────────────────

/// The SKY EVENTS card's picture, `w` px wide, on the night sky: a fall of
/// meteors.
pub fn card_art(_idx: usize, w: u32) -> image::RgbaImage {
    let h = (w as f32 * 0.6) as u32;
    let mut img = image::RgbaImage::from_fn(w, h, |x, y| {
        let v = if hash(x as i32, y as i32, 408) > 0.996 { 200 } else { 0 };
        image::Rgba([6 + v, 5 + v, 18 + v, 255])
    });
    let m = meteor_img();
    for k in 0..7u32 {
        let len = w / 4 + (k * 37 % (w / 5));
        let streak = image::imageops::resize(&*m, len, (len as f32 * 24.0 / 512.0) as u32 + 1, image::imageops::FilterType::Triangle);
        let streak = image::imageops::rotate180(&streak);
        image::imageops::overlay(&mut img, &streak, (w / 10 + k * w / 9) as i64, (h / 6 + (k * 53) % (h * 2 / 3)) as i64);
    }
    img
}

/// Build all the art off the main thread (the shop and the run both use it).
pub fn prewarm() {
    std::thread::spawn(|| {
        let _ = (glow_img(), meteor_img());
        let _ = eye_frames();
    });
}

// ── Objects ──────────────────────────────────────────────────────────────────

const FLASHES: usize = 4;
const METEORS: usize = 16;

fn flash_id(i: usize) -> String { format!("wx_flash_{i}") }
fn meteor_id(i: usize) -> String { format!("ev_meteor_{i}") }
const EYE_ID: &str = "wx_eye";

/// The objects, for the scene builder: over the sky, behind the world.
pub fn objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    let mut ids = vec![EYE_ID.to_string()];
    ids.extend((0..FLASHES).map(flash_id));
    ids.extend((0..METEORS).map(meteor_id));
    ids.into_iter().map(|id| {
        let mut o = GameObject::new_rect(ctx, id.clone(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (8.0, 8.0), 0.0), image: crate::images::solid(0, 0, 0, 0).into(), color: None }),
            (8.0, 8.0), (-9000.0, -9000.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        o.ignore_zoom = true;
        o.unlit = true;
        o.visible = false;
        (id, o)
    }).collect()
}

fn hide(c: &mut Canvas, id: &str) { if let Some(o) = c.get_game_object_mut(id) { o.visible = false; } }

fn place(c: &mut Canvas, id: &str, img: &Img, centre: (f32, f32), size: (f32, f32), rot: f32, tint: Color) {
    if let Some(o) = c.get_game_object_mut(id) {
        o.size = size;
        o.position = (centre.0 - size.0 * 0.5, centre.1 - size.1 * 0.5);
        o.rotation = rot;
        // The shape is the image's own size; the object's size scales it
        // (a shape at the screen size sampled past the image's edge).
        let own = (img.width() as f32, img.height() as f32);
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, own, 0.0), image: img.clone(), color: Some(tint) });
        o.visible = true;
    }
}

// ── The tick ─────────────────────────────────────────────────────────────────

struct Meteor { x: f32, y: f32, vx: f32, vy: f32, len: f32, life: f32, max: f32 }

#[derive(Default)]
struct Sim {
    rng: u32,
    t: u32,
    /// Weather.
    flashes: [(f32, f32, f32, f32); FLASHES], // (sky u, sky row, size, frames left)
    next_flash: f32,
    eye_u: f32,
    /// Events.
    shower_in: f32,
    shower_left: f32,
    meteors: Vec<Meteor>,
    prev_px: Option<f32>,
    m_was_down: bool,
}

impl Sim {
    fn rand(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        self.rng = x;
        (x % 100_000) as f32 / 100_000.0
    }
}

fn sim() -> &'static Mutex<Sim> {
    static S: OnceLock<Mutex<Sim>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Sim { rng: 0x9E37_79B9, shower_in: 40.0 * 60.0, next_flash: 90.0, ..Default::default() }))
}

pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (px, flipped, no_sky, boss, dark) = {
        let s = st.lock().unwrap();
        (s.px, s.gravity_dir < 0.0, s.in_space_mode || s.space_launch_active, s.boss_active, s.dark_mode)
    };
    let weather = crate::sky::PLANETS[crate::sky::planet_of(c)].weather;
    let events = c.get_bool(crate::sky::EVENTS_VAR);
    let m_down = c.key("m");
    let sky = c.get_game_object("bg").map(|o| (o.position, o.size));
    let off = no_sky || dark || c.get_bool("bg_vivid") || sky.is_none();
    let mut m = sim().lock().unwrap();
    m.t = m.t.wrapping_add(1);
    let t = m.t as f32;
    // M: a meteor shower now (for testing; works with or without the card).
    if m_down && !m.m_was_down { m.shower_left = 5.0 * 60.0; }
    m.m_was_down = m_down;
    // Forward travel, for drift (a jump of 2,000 px or more is a warp).
    let dx = m.prev_px.map_or(0.0, |p| px - p);
    m.prev_px = Some(px);
    let forward = if dx.abs() > 2_000.0 { 0.0 } else { dx.max(0.0) };
    m.eye_u = (m.eye_u + forward * 0.006).rem_euclid(VW * 1.6);

    let Some(((bx, by), (bw, bh))) = sky.filter(|_| !off) else {
        drop(m);
        hide(c, EYE_ID);
        for i in 0..FLASHES { hide(c, &flash_id(i)); }
        for i in 0..METEORS { hide(c, &meteor_id(i)); }
        return;
    };
    // A sky-picture point to the screen, exactly where the sky draws it
    // (mirrored when the sky is flipped): the weather sits on the planet.
    let (ox, oy, sc) = super::background::sky_picture_rect(c).unwrap_or((bx, by, bw / VW));
    let _ = bh;
    let (sx, sy) = (sc, sc);
    let to_screen = |u: f32, v: f32| (ox + u * sx, oy + if flipped { VH - v } else { v } * sy);

    // ── Weather ──
    let horizon = crate::scenes::game::planet::HORIZON_ROW;
    if weather {
        // The storm's eye on the planet, turning.
        let frames = eye_frames();
        let frame = frames[(t / 8.0) as usize % frames.len()].clone();
        // On the planet's face from the start, drifting left as you travel
        // and coming round again.
        let u = (VW * 0.62 - m.eye_u).rem_euclid(VW * 1.6) - VW * 0.2;
        let (ex, ey) = to_screen(u, horizon + 55.0);
        let ew = 1000.0 * sx;
        let eh = ew * EYE_SIZE.1 as f32 / EYE_SIZE.0 as f32;
        // Flashes: one now and then, flickering, deep in the clouds.
        m.next_flash -= 1.0;
        if m.next_flash <= 0.0 {
            let slot = m.flashes.iter().position(|f| f.3 <= 0.0);
            if let Some(i) = slot {
                let (fu, fv, fs) = (VW * (0.1 + m.rand() * 0.8), horizon + 20.0 + m.rand() * 130.0, 600.0 + m.rand() * 700.0);
                m.flashes[i] = (fu, fv, fs, 14.0);
            }
            m.next_flash = 25.0 + m.rand() * 100.0;
        }
        let flashes = m.flashes;
        for f in m.flashes.iter_mut() { f.3 -= 1.0; }
        drop(m);
        place(c, EYE_ID, &frame, (ex, ey), (ew, eh), if flipped { 180.0 } else { 0.0 }, Color(236, 236, 250, 150));
        for (i, (fu, fv, fs, left)) in flashes.iter().enumerate() {
            if *left <= 0.0 { hide(c, &flash_id(i)); continue; }
            // Two quick pulses, the second brighter.
            let k = 14.0 - left;
            let a = if k < 3.0 { 0.7 } else if k < 5.0 { 0.15 } else { (1.0 - (k - 5.0) / 9.0).max(0.0) };
            // Nearer the horizon, smaller (farther away).
            let near = ((fv - horizon) / 170.0).clamp(0.2, 1.0);
            let (fx, fy) = to_screen(*fu, *fv);
            let fw = fs * near * sx;
            place(c, &flash_id(i), &glow_img(), (fx, fy), (fw, fw * GLOW_SIZE.1 as f32 / GLOW_SIZE.0 as f32), if flipped { 180.0 } else { 0.0 },
                Color(245, 240, 255, (a * 255.0) as u8));
        }
        m = sim().lock().unwrap();
    } else {
        drop(m);
        hide(c, EYE_ID);
        for i in 0..FLASHES { hide(c, &flash_id(i)); }
        m = sim().lock().unwrap();
    }

    // ── Meteor showers ──
    if (events || m.shower_left > 0.0) && !boss {
        if m.shower_left > 0.0 {
            m.shower_left -= 1.0;
            if m.meteors.len() < METEORS && m.rand() < 0.09 {
                let (x, y) = (VW * (0.15 + m.rand() * 0.95), VH * (0.02 + m.rand() * 0.3));
                let sp = 34.0 + m.rand() * 26.0;
                let ang = (200.0 + m.rand() * 18.0f32).to_radians();
                let max = 22.0 + m.rand() * 18.0;
                let len = 260.0 + m.rand() * 380.0;
                m.meteors.push(Meteor { x, y, vx: ang.cos() * sp, vy: -ang.sin() * sp, len, life: max, max });
            }
        } else if events {
            m.shower_in -= 1.0;
            if m.shower_in <= 0.0 {
                m.shower_left = 5.0 * 60.0;
                m.shower_in = (70.0 + m.rand() * 70.0) * 60.0;
            }
        }
        for me in m.meteors.iter_mut() { me.x += me.vx; me.y += me.vy; me.life -= 1.0; }
        m.meteors.retain(|me| me.life > 0.0);
        let meteors: Vec<(f32, f32, f32, f32, f32)> = m.meteors.iter().map(|me| (me.x, me.y, me.vy.atan2(me.vx).to_degrees(), me.len, me.life / me.max)).collect();
        drop(m);
        let img = meteor_img();
        for i in 0..METEORS {
            match meteors.get(i) {
                Some(&(x, y, ang, len, k)) => {
                    // The image's head is its right end: place it so the
                    // head leads, the tail trailing back along the path.
                    let (cx, cy) = (x - ang.to_radians().cos() * len * 0.5, y - ang.to_radians().sin() * len * 0.5);
                    let a = (k * 3.0).min(1.0) * k.sqrt();
                    place(c, &meteor_id(i), &img, (cx, cy), (len, len * 24.0 / 512.0), ang, Color(255, 255, 255, (a * 255.0) as u8));
                }
                None => hide(c, &meteor_id(i)),
            }
        }
    } else {
        m.meteors.clear();
        drop(m);
        for i in 0..METEORS { hide(c, &meteor_id(i)); }
    }
}

/// Start a meteor shower now (the headless driver's look at one).
pub fn trigger_events() {
    let mut m = sim().lock().unwrap();
    m.shower_left = 5.0 * 60.0;
}

