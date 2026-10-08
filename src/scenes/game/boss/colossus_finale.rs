//! colossus_finale.rs — the Colossus's last stand, dressed for it.
//!
//! Once the hands and the torso have fallen the head fights alone, and it
//! should look like the climax of the fight, not a leftover. This module
//! layers over what `colossus.rs` already draws (the head, its gravity well,
//! charge orb and gaze beam) without changing any of the fight's logic:
//!
//! * the head ENRAGES: a molten aura and flame tongues off its crown, eyes that
//!   burn with a bloom and an anamorphic streak, embers pouring off it;
//! * the gravity well becomes real bent space (the gravity-lens shader) with
//!   debris spiralling into it, and the charge orb gets streaks converging on
//!   it, layered heat and crackling energy;
//! * the gaze beam is a white-hot core in a gold and orange sheath, its
//!   warning lane a soft-edged red band, and its detonations fireballs with
//!   sparks;
//! * the arena darkens to a red, heart-beating vignette with ash falling.
//!
//! The beam, orb, well and detonation re-skins apply whenever the head
//! attacks (they ARE the head's attack); the enrage, embers, ash and vignette
//! only in the last stage, ramping in over two seconds.
//!
//! All art is generated once and cached as shared `Arc`s, so nothing here
//! uploads a texture per frame; fades are tints on those same textures.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;

/// Layers: the parts sit at 21 (`LAYER_SPACE_HOOK`).
const L_AURA: i32 = 19;
const L_FLAMES: i32 = 20;
const L_EYES: i32 = 23;
const L_EMBERS: i32 = 24;
/// Behind the fight, over the sky. It used to sit over the fight (90), but
/// a sprite writes depth wherever it draws, and the shader effects drawn
/// after it (the gravity lens) failed the depth test under its darkened
/// edges — the lens came out cut off along a jagged line near the screen
/// edge, moving as the player moved.
const L_VIGNETTE: i32 = 18;

const EMBERS: usize = 80;
const ASH: usize = 28;
const DEBRIS: usize = 28;

/// Where the eyes are on the head sprite (fractions of its size).
const EYES: [(f32, f32); 2] = [(0.36, 0.47), (0.64, 0.47)];

/// How much of the gravity well the lens is drawn over (world px across).
const LENS_D: f32 = 4800.0;

/// Ticks for the enrage to come all the way in.
const ENRAGE_TICKS: f32 = 120.0;

// ── Art (built once) ─────────────────────────────────────────────────────────

type Img = Arc<image::RgbaImage>;

fn cached(slot: &'static OnceLock<Img>, f: impl FnOnce() -> image::RgbaImage) -> Img {
    slot.get_or_init(|| Arc::new(f())).clone()
}

/// A radial gradient through `stops` (radius 0..1 → rgba), smooth.
fn radial(n: u32, stops: &[(f32, [f32; 4])]) -> image::RgbaImage {
    let r = n as f32 * 0.5;
    image::RgbaImage::from_fn(n, n, |x, y| {
        let d = (((x as f32 + 0.5 - r).powi(2) + (y as f32 + 0.5 - r).powi(2)).sqrt() / r).min(1.0);
        let i = stops.iter().position(|s| s.0 >= d).unwrap_or(stops.len() - 1).max(1);
        let (a, b) = (stops[i - 1], stops[i]);
        let t = ((d - a.0) / (b.0 - a.0).max(1e-4)).clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t);
        let c = [0, 1, 2, 3].map(|k| (a.1[k] + (b.1[k] - a.1[k]) * t).clamp(0.0, 255.0) as u8);
        image::Rgba(c)
    })
}

/// The molten aura behind the head: hot near the centre, a red rim, gone.
fn aura_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || radial(256, &[
        (0.0, [255.0, 170.0, 70.0, 200.0]), (0.35, [255.0, 110.0, 40.0, 150.0]),
        (0.7, [190.0, 30.0, 20.0, 70.0]), (1.0, [90.0, 0.0, 10.0, 0.0]),
    ]))
}

/// A burning eye: white core, gold, orange bloom.
fn eye_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || radial(128, &[
        (0.0, [255.0, 255.0, 240.0, 255.0]), (0.12, [255.0, 240.0, 170.0, 255.0]),
        (0.35, [255.0, 150.0, 50.0, 170.0]), (0.7, [230.0, 60.0, 20.0, 50.0]), (1.0, [200.0, 30.0, 10.0, 0.0]),
    ]))
}

/// A thin horizontal lens streak (bright middle, tapering both ways).
fn streak_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || {
        let (w, h) = (256u32, 16u32);
        image::RgbaImage::from_fn(w, h, |x, y| {
            let fx = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
            let fy = (y as f32 + 0.5) / h as f32 * 2.0 - 1.0;
            let a = (1.0 - fx.abs()).powf(2.2) * (1.0 - fy.abs()).powf(1.6);
            image::Rgba([255, 200, 140, (a * 255.0) as u8])
        })
    })
}

/// A soft glowing dot (embers, debris heads).
fn dot_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || radial(32, &[
        (0.0, [255.0, 255.0, 230.0, 255.0]), (0.3, [255.0, 220.0, 150.0, 230.0]), (1.0, [255.0, 140.0, 40.0, 0.0]),
    ]))
}

/// A flake of ash: a soft grey speck.
fn ash_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || radial(24, &[
        (0.0, [70.0, 62.0, 66.0, 220.0]), (0.6, [55.0, 45.0, 50.0, 140.0]), (1.0, [40.0, 30.0, 35.0, 0.0]),
    ]))
}

/// A streak (debris sucked into the well): bright head, fading tail.
fn trail_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || {
        let (w, h) = (128u32, 12u32);
        image::RgbaImage::from_fn(w, h, |x, y| {
            let fx = (x as f32 + 0.5) / w as f32;
            let fy = ((y as f32 + 0.5) / h as f32 * 2.0 - 1.0).abs();
            let a = fx.powf(1.8) * (1.0 - fy).powf(1.3);
            image::Rgba([230, 210, 255, (a * 255.0) as u8])
        })
    })
}

/// The charge orb, layered: white-hot core, gold body, orange corona.
fn orb_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || radial(256, &[
        (0.0, [255.0, 255.0, 255.0, 255.0]), (0.22, [255.0, 250.0, 210.0, 255.0]),
        (0.5, [255.0, 200.0, 90.0, 240.0]), (0.78, [255.0, 120.0, 40.0, 140.0]), (1.0, [220.0, 50.0, 20.0, 0.0]),
    ]))
}

/// The beam core, across its width: white centre, gold, orange, soft edges.
/// Tiled along the length by the strip's stretch.
fn beam_core_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || {
        let (w, h) = (8u32, 64u32);
        image::RgbaImage::from_fn(w, h, |_, y| {
            let d = ((y as f32 + 0.5) / h as f32 * 2.0 - 1.0).abs();
            let stops = [(0.0, [255.0, 255.0, 255.0, 255.0]), (0.25, [255.0, 245.0, 200.0, 255.0]),
                         (0.55, [255.0, 190.0, 80.0, 230.0]), (0.82, [255.0, 110.0, 40.0, 130.0]), (1.0, [220.0, 50.0, 20.0, 0.0])];
            let i = stops.iter().position(|s| s.0 >= d).unwrap_or(4).max(1);
            let (a, b) = (stops[i - 1], stops[i]);
            let t = ((d - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
            let c = [0, 1, 2, 3].map(|k| (a.1[k] + (b.1[k] - a.1[k]) * t) as u8);
            image::Rgba(c)
        })
    })
}

/// The beam's warning lane: a soft red band, brighter edges, faint centre
/// line, so it reads as "this whole width" without a hard rectangle.
fn lane_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || {
        let (w, h) = (8u32, 64u32);
        image::RgbaImage::from_fn(w, h, |_, y| {
            let d = ((y as f32 + 0.5) / h as f32 * 2.0 - 1.0).abs();
            let body = 0.22 * (1.0 - d * d);
            let edge = (1.0 - ((d - 0.86) / 0.1).abs()).clamp(0.0, 1.0) * 0.7;
            let centre = (1.0 - d / 0.06).clamp(0.0, 1.0) * 0.35;
            let fade = 1.0 - ((d - 0.97) / 0.03).clamp(0.0, 1.0);
            let a = (body + edge + centre).min(1.0) * fade;
            image::Rgba([255, 60, 40, (a * 255.0) as u8])
        })
    })
}

/// A detonation: a fireball, hot centre to a red, smoky edge.
fn fireball_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || radial(128, &[
        (0.0, [255.0, 255.0, 230.0, 255.0]), (0.25, [255.0, 220.0, 120.0, 250.0]),
        (0.55, [255.0, 120.0, 40.0, 210.0]), (0.8, [160.0, 40.0, 30.0, 110.0]), (1.0, [60.0, 20.0, 20.0, 0.0]),
    ]))
}

/// The arena's last-stand vignette: clear centre, red-black edges.
fn vignette_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || {
        let n = 256u32;
        image::RgbaImage::from_fn(n, n, |x, y| {
            let fx = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let fy = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let d = (fx * fx * 0.8 + fy * fy).sqrt();
            let a = ((d - 0.45) / 0.75).clamp(0.0, 1.0).powf(1.5);
            image::Rgba([70, 0, 6, (a * 235.0) as u8])
        })
    })
}

fn clear_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    cached(&I, || image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 0])))
}

// ── Objects ──────────────────────────────────────────────────────────────────

fn quad(name: &str, img: Img, layer: i32) -> GameObject {
    let mut o = GameObject::build(name)
        .position(-9000.0, -9000.0)
        .size(8.0, 8.0)
        .image(Image { shape: ShapeType::Rectangle(0.0, (8.0, 8.0), 0.0), image: img, color: None })
        .layer(layer)
        .finish();
    o.visible = false;
    o.gravity = 0.0;
    o
}

fn ensure(c: &mut Canvas) {
    if c.get_game_object("cf_aura").is_some() { return; }
    c.add_game_object("cf_aura".into(), quad("cf_aura", aura_img(), L_AURA));
    c.add_game_object("cf_flames".into(), quad("cf_flames", aura_img(), L_FLAMES));
    for i in 0..2 {
        let n = format!("cf_eye_{i}");
        c.add_game_object(n.clone(), quad(&n, eye_img(), L_EYES));
    }
    c.add_game_object("cf_streak".into(), quad("cf_streak", streak_img(), L_EYES));
    c.add_game_object("cf_orb_fx".into(), quad("cf_orb_fx", clear_img(), 31));
    for i in 0..EMBERS {
        let n = format!("cf_ember_{i}");
        c.add_game_object(n.clone(), quad(&n, dot_img(), L_EMBERS));
    }
    for i in 0..ASH {
        let n = format!("cf_ash_{i}");
        c.add_game_object(n.clone(), quad(&n, ash_img(), L_EMBERS));
    }
    for i in 0..DEBRIS {
        let n = format!("cf_debris_{i}");
        c.add_game_object(n.clone(), quad(&n, trail_img(), 29));
    }
    let mut v = GameObject::build("cf_vignette")
        .position(-VW * 0.5, -VH * 0.5)
        .size(VW * 2.0, VH * 2.0)
        .image(Image { shape: ShapeType::Rectangle(0.0, (VW * 2.0, VH * 2.0), 0.0), image: vignette_img(), color: None })
        .layer(L_VIGNETTE)
        .ignore_zoom()
        .finish();
    v.visible = false;
    v.unlit = true;
    c.add_game_object("cf_vignette".into(), v);
}

/// Place a quad centred at `at`, `size`, tinted (rgba multiplies the art).
fn put(c: &mut Canvas, name: &str, img: &Img, at: (f32, f32), size: (f32, f32), rot: f32, tint: [u8; 4]) {
    if let Some(o) = c.get_game_object_mut(name) {
        o.visible = tint[3] > 0;
        o.size = size;
        o.position = (at.0 - size.0 * 0.5, at.1 - size.1 * 0.5);
        o.rotation = rot;
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img.clone(),
            color: Some(Color(tint[0], tint[1], tint[2], tint[3])) });
    }
}

fn hide(c: &mut Canvas, name: &str) {
    if let Some(o) = c.get_game_object_mut(name) { o.visible = false; }
}

// ── Particles ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Default)]
struct P { x: f32, y: f32, vx: f32, vy: f32, life: f32, max: f32, size: f32, seed: f32 }

#[derive(Default)]
struct Sim { embers: Vec<P>, ash: Vec<P>, debris: Vec<P>, rng: u64, t: u32, enrage: f32, well_lensed: bool }

fn sim() -> &'static Mutex<Sim> {
    static S: OnceLock<Mutex<Sim>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Sim { rng: 0x5EED_C0105, ..Default::default() }))
}

fn rnd(r: &mut u64) -> f32 {
    *r ^= *r << 13; *r ^= *r >> 7; *r ^= *r << 17;
    (*r >> 40) as f32 / (1u64 << 24) as f32
}

// ── Per frame ────────────────────────────────────────────────────────────────

fn hide_all(c: &mut Canvas) {
    for n in ["cf_aura", "cf_flames", "cf_eye_0", "cf_eye_1", "cf_streak", "cf_orb_fx", "cf_vignette"] { hide(c, n); }
    c.clear_effect("cf_orb_fx");
    for i in 0..EMBERS { hide(c, &format!("cf_ember_{i}")); }
    for i in 0..ASH { hide(c, &format!("cf_ash_{i}")); }
    for i in 0..DEBRIS { hide(c, &format!("cf_debris_{i}")); }
}

/// Every boss frame, after the Colossus's own tick.
pub(crate) fn tick_colossus_finale(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (on, head_alone, dead, frozen) = {
        let s = st.lock().unwrap();
        let on = s.boss_active && s.boss_spawned && s.boss_kind == BossKind::Colossus
            && s.boss_parts.len() >= 4 && s.boss_hp > 0;
        let alone = on && s.boss_parts[..3].iter().all(|p| !p.alive) && s.boss_parts[3].alive;
        (on, alone, false, super::phase_cine_active(&s))
    };
    // Not gated on `dead`: the fight carries on under the death screen.
    if !on || dead {
        if c.get_game_object("cf_aura").is_some() { hide_all(c); }
        let mut m = sim().lock().unwrap();
        m.enrage = 0.0;
        m.embers.clear(); m.ash.clear(); m.debris.clear();
        m.well_lensed = false;
        return;
    }
    ensure(c);
    let head = c.get_game_object("colossus_part_3")
        .filter(|o| o.visible)
        .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5, o.size.0));
    let Some((hx, hy, hs)) = head else { hide_all(c); return };

    let mut m = sim().lock().unwrap();
    m.t = m.t.wrapping_add(1);
    let t = m.t as f32;
    if head_alone { m.enrage = (m.enrage + 1.0 / ENRAGE_TICKS).min(1.0); } else { m.enrage = (m.enrage - 0.05).max(0.0); }
    let e = m.enrage;
    let ee = e * e * (3.0 - 2.0 * e);
    // A slow heartbeat: two beats close together, then a rest.
    let beat_t = (t % 75.0) / 75.0;
    let beat = (-((beat_t - 0.08) / 0.05).powi(2)).exp() + 0.6 * (-((beat_t - 0.26) / 0.05).powi(2)).exp();

    // ── The enraged head ──
    if ee > 0.01 {
        let s = hs * (1.85 + 0.08 * beat + 0.04 * (t * 0.05).sin());
        put(c, "cf_aura", &aura_img(), (hx, hy), (s, s), 0.0, [255, 255, 255, (210.0 * ee) as u8]);
        // A second, tighter heat layer breathing against the first (the fire
        // shader was tried here and left a hard-edged box around its quad).
        let s2 = hs * (1.35 + 0.12 * (t * 0.11).sin());
        put(c, "cf_flames", &aura_img(), (hx, hy - hs * 0.08), (s2, s2 * 1.08), 0.0, [255, 230, 160, (170.0 * ee) as u8]);
        // The eyes: blooming, flickering, and a lens streak through them.
        let flick = 0.88 + 0.12 * ((t * 0.9).sin() * (t * 0.37).cos());
        let es = hs * (0.30 + 0.06 * beat) * flick;
        let mut eye_pts = [(0.0, 0.0); 2];
        for (i, (fx, fy)) in EYES.iter().enumerate() {
            let at = (hx - hs * 0.5 + hs * fx, hy - hs * 0.5 + hs * fy);
            eye_pts[i] = at;
            put(c, &format!("cf_eye_{i}"), &eye_img(), at, (es, es), 0.0, [255, 255, 255, (255.0 * ee) as u8]);
        }
        let mid = ((eye_pts[0].0 + eye_pts[1].0) * 0.5, (eye_pts[0].1 + eye_pts[1].1) * 0.5);
        let sw = hs * (2.2 + 0.4 * beat);
        put(c, "cf_streak", &streak_img(), mid, (sw, hs * 0.07), 0.0, [255, 255, 255, (200.0 * ee * flick) as u8]);
    } else {
        for n in ["cf_aura", "cf_flames", "cf_eye_0", "cf_eye_1", "cf_streak"] { hide(c, n); }
    }

    // ── Embers pouring off it, ash falling across the arena ──
    let rng = &mut { m.rng };
    let mut embers = std::mem::take(&mut m.embers);
    let mut ash = std::mem::take(&mut m.ash);
    let mut debris = std::mem::take(&mut m.debris);
    if ee > 0.01 && !frozen {
        // Spawn: rate rises with the enrage.
        let want = (EMBERS as f32 * ee) as usize;
        while embers.len() < want {
            let a = rnd(rng) * std::f32::consts::TAU;
            let r = hs * (0.25 + 0.3 * rnd(rng));
            let max = 70.0 + 80.0 * rnd(rng);
            // One in three is a flame tongue off the crown: big, short-lived,
            // rising fast (drawn stretched); the rest are embers.
            let flame = rnd(rng) < 0.33;
            if flame {
                let max = 30.0 + 25.0 * rnd(rng);
                embers.push(P { x: hx + (rnd(rng) - 0.5) * hs * 0.7, y: hy - hs * 0.3,
                    vx: (rnd(rng) - 0.5) * 2.0, vy: -(9.0 + 7.0 * rnd(rng)), life: max, max,
                    size: -(90.0 + 110.0 * rnd(rng)), seed: rnd(rng) * 100.0 });
            } else {
                embers.push(P { x: hx + a.cos() * r, y: hy + a.sin() * r * 0.7 + hs * 0.1,
                    vx: (rnd(rng) - 0.5) * 4.0, vy: -(3.0 + 6.0 * rnd(rng)), life: max, max,
                    size: 14.0 + 30.0 * rnd(rng), seed: rnd(rng) * 100.0 });
            }
        }
        let want_ash = (ASH as f32 * ee) as usize;
        while ash.len() < want_ash {
            let max = 260.0 + 200.0 * rnd(rng);
            ash.push(P { x: hx + (rnd(rng) - 0.5) * 5200.0, y: hy - 2200.0 + rnd(rng) * 600.0,
                vx: 1.0 + rnd(rng) * 2.0, vy: 3.0 + 3.0 * rnd(rng), life: max, max,
                size: 16.0 + 22.0 * rnd(rng), seed: rnd(rng) * 100.0 });
        }
    }
    if !frozen {
        for p in embers.iter_mut() {
            p.vx += (t * 0.07 + p.seed).sin() * 0.25;
            p.vy -= 0.05;
            p.x += p.vx; p.y += p.vy; p.life -= 1.0;
        }
        for p in ash.iter_mut() {
            p.x += p.vx + (t * 0.03 + p.seed).sin() * 1.2; p.y += p.vy; p.life -= 1.0;
        }
    }
    embers.retain(|p| p.life > 0.0);
    ash.retain(|p| p.life > 0.0);

    // ── Gravity well: debris spiralling in, real lensing ──
    // The well is centred on the head (and resized below, so its own rect is
    // not where the fight put it): read the centre from the head.
    let well = c.get_game_object("colossus_well").filter(|o| o.visible).map(|_| (hx, hy, LENS_D * 0.5));
    let orb = c.get_game_object("colossus_charge").filter(|o| o.visible)
        .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5, o.size.0 * 0.5));
    if let Some((wx, wy, wr)) = well {
        if !m.well_lensed {
            m.well_lensed = true;
            // A clear RECTANGLE, as the Weaver's lens is: on an ellipse the
            // lens's rings were cut off along a jagged oval edge.
            if let Some(o) = c.get_game_object_mut("colossus_well") {
                o.set_image(Image { shape: ShapeType::Rectangle(0.0, (LENS_D, LENS_D), 0.0), image: clear_img(), color: None });
            }
        }
        // Drawn over the area around the head only: the well's own object
        // spans its whole 6400 px pull range, far more than the screen.
        let d = LENS_D;
        if let Some(o) = c.get_game_object_mut("colossus_well") {
            o.size = (d, d);
            o.position = (wx - d * 0.5, wy - d * 0.5);
        }
        // Kept low: at full strength the lens's rings cover the screen and
        // hide the beam lane, which is the thing the player must read.
        let strength = 0.12 + 0.05 * (t * 0.08).sin().abs() + 0.05 * ee;
        c.attach_effect("colossus_well", Effect::GravityLens { strength }, EffectColor::srgb8(170, 110, 255), (d, d));
        if !frozen {
            while debris.len() < DEBRIS {
                let a = rnd(rng) * std::f32::consts::TAU;
                let r = wr * (0.55 + 0.45 * rnd(rng));
                debris.push(P { x: wx + a.cos() * r, y: wy + a.sin() * r, vx: 0.0, vy: 0.0,
                    life: 1.0, max: 1.0, size: 120.0 + 160.0 * rnd(rng), seed: a });
            }
            for p in debris.iter_mut() {
                // Swirl in: tangential plus inward, faster as it nears.
                let (dx, dy) = (wx - p.x, wy - p.y);
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let (nx, ny) = (dx / dist, dy / dist);
                let speed = 18.0 + 2600.0 / dist.sqrt();
                p.vx = nx * speed * 0.55 - ny * speed;
                p.vy = ny * speed * 0.55 + nx * speed;
                p.x += p.vx; p.y += p.vy;
                if dist < hs * 0.4 { p.life = 0.0; }
            }
        }
    } else {
        if m.well_lensed {
            m.well_lensed = false;
            c.clear_effect("colossus_well");
            // Back to the size the fight positions it by.
            if let Some(o) = c.get_game_object_mut("colossus_well") {
                let full = COLOSSUS_GRAVITY_RANGE * 2.0;
                o.size = (full, full);
            }
        }
        if let Some((ox, oy, or)) = orb {
            // Streaks converging on the charging orb.
            if !frozen {
                while debris.len() < DEBRIS / 2 {
                    let a = rnd(rng) * std::f32::consts::TAU;
                    let r = or * (4.0 + 3.0 * rnd(rng));
                    debris.push(P { x: ox + a.cos() * r, y: oy + a.sin() * r, vx: 0.0, vy: 0.0,
                        life: 1.0, max: 1.0, size: 80.0 + 90.0 * rnd(rng), seed: a });
                }
                for p in debris.iter_mut() {
                    let (dx, dy) = (ox - p.x, oy - p.y);
                    let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                    p.vx = dx / dist * 26.0; p.vy = dy / dist * 26.0;
                    p.x += p.vx; p.y += p.vy;
                    if dist < or * 0.6 { p.life = 0.0; }
                }
            }
        } else {
            debris.clear();
        }
    }
    debris.retain(|p| p.life > 0.0);
    m.rng = *rng;
    drop(m);

    for i in 0..EMBERS {
        let n = format!("cf_ember_{i}");
        match embers.get(i) {
            Some(p) if p.size < 0.0 => {
                // A flame tongue: a tall soft dot, yellow at birth to red.
                let k = p.life / p.max;
                let s = -p.size * (0.4 + 0.6 * k);
                let a = (k * 1.4).min(1.0) * ((1.0 - k) * 8.0).min(1.0);
                put(c, &n, &dot_img(), (p.x, p.y), (s * 0.55, s), 0.0, [255, (90.0 + 150.0 * k) as u8, (30.0 * k) as u8, (a * 230.0) as u8]);
            }
            Some(p) => {
                let k = p.life / p.max;
                let a = (k * 1.6).min(1.0);
                let g = (140.0 + 115.0 * k) as u8;
                put(c, &n, &dot_img(), (p.x, p.y), (p.size * (0.5 + 0.5 * k), p.size * (0.5 + 0.5 * k)), 0.0, [255, g, (60.0 * k) as u8, (a * 255.0) as u8]);
            }
            None => hide(c, &n),
        }
    }
    for i in 0..ASH {
        let n = format!("cf_ash_{i}");
        match ash.get(i) {
            Some(p) => {
                let k = p.life / p.max;
                let a = (k * 3.0).min(1.0) * ((1.0 - k) * 6.0).min(1.0);
                put(c, &n, &ash_img(), (p.x, p.y), (p.size, p.size * 0.7), p.seed * 40.0, [255, 255, 255, (a * 200.0) as u8]);
            }
            None => hide(c, &n),
        }
    }
    for i in 0..DEBRIS {
        let n = format!("cf_debris_{i}");
        match debris.get(i) {
            Some(p) => {
                let ang = p.vy.atan2(p.vx).to_degrees();
                put(c, &n, &trail_img(), (p.x, p.y), (p.size, p.size * 0.09), ang, [255, 255, 255, 220]);
            }
            None => hide(c, &n),
        }
    }
    let mut m = sim().lock().unwrap();
    m.embers = embers; m.ash = ash; m.debris = debris;
    drop(m);

    // ── The charge orb: layered heat and crackling energy ──
    if let Some((ox, oy, or)) = orb {
        let d = or * 2.0;
        if let Some(o) = c.get_game_object_mut("colossus_charge") {
            o.clear_glow();
            o.set_image(Image { shape: ShapeType::Ellipse(0.0, (d, d), 0.0), image: orb_img(), color: None });
        }
        let fx = d * 1.9;
        if let Some(o) = c.get_game_object_mut("cf_orb_fx") {
            o.visible = true;
            o.size = (fx, fx);
            o.position = (ox - fx * 0.5, oy - fx * 0.5);
        }
        c.attach_effect("cf_orb_fx", Effect::AnimatedImage { image: orb_img(), flags: VfxFlags::ELECTRICITY, alpha: 0.85 },
            EffectColor::srgb8(255, 210, 120), (fx, fx));
    } else {
        hide(c, "cf_orb_fx");
        c.clear_effect("cf_orb_fx");
    }

    // ── The beam and its lane, re-skinned (they are drawn as strips) ──
    for i in 0..COLOSSUS_BEAM_SEGMENTS {
        for (pre, img) in [("colossus_beam_core", beam_core_img()), ("colossus_beam_tel", lane_img())] {
            if let Some(o) = c.get_game_object_mut(&format!("{pre}_{i}")) {
                if o.visible {
                    if pre == "colossus_beam_core" { o.clear_glow(); }
                    let size = o.size;
                    // A touch wider than the hit band: the soft edge is outside it.
                    o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img.clone(), color: None });
                }
            }
        }
    }
    // Detonations: fireballs with sparks, not flat discs.
    for i in 0..COLOSSUS_BEAM_EXPLODE_MAX_LIVE {
        let n = format!("colossus_beam_explode_{i}");
        let vis = c.get_game_object(&n).map_or(false, |o| o.visible);
        if vis {
            // Re-skinned only, never resized: the fight does not reset the
            // size of every live one each frame, so scaling here compounded.
            if let Some(o) = c.get_game_object_mut(&n) {
                o.clear_glow();
                let size = o.size;
                o.set_image(Image { shape: ShapeType::Ellipse(0.0, size, 0.0), image: fireball_img(), color: None });
            }
            let size = c.get_game_object(&n).map(|o| o.size).unwrap_or((8.0, 8.0));
            c.attach_effect(&n, Effect::AnimatedImage { image: fireball_img(), flags: VfxFlags::EXPLOSIVE_SPARKS, alpha: 1.0 },
                EffectColor::srgb8(255, 190, 90), size);
        } else {
            c.clear_effect(&n);
        }
    }

    // ── The arena: a red, heart-beating vignette ──
    if ee > 0.01 {
        let a = (ee * (0.65 + 0.35 * beat)).min(1.0);
        if let Some(o) = c.get_game_object_mut("cf_vignette") {
            o.visible = true;
            o.set_image(Image { shape: ShapeType::Rectangle(0.0, (VW * 2.0, VH * 2.0), 0.0), image: vignette_img(),
                color: Some(Color(255, 255, 255, (a * 255.0) as u8)) });
        }
    } else {
        hide(c, "cf_vignette");
    }
}
