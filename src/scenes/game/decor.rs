//! decor.rs — distant scenery drifting behind the swing.
//!
//! The normal-zone backdrop is a screen-fixed sky: over a long Casual or
//! Normal stretch nothing in it moved, which is most of why minute forty
//! looked like minute four. These are planets, a moon, a galaxy, satellites
//! and wrecks — and, rarely, an oddity: a UFO, a space whale, a drifting
//! astronaut, a rubber duck — that slide past with camera parallax.
//!
//! SPARSE by design: a piece every ~25-45 s of travel, never more than
//! `DECOR_MAX_LIVE` at once, dimmed and behind everything, so it reads as
//! "the world is big and you are moving through it" and never as clutter
//! competing with the nodes. Art: assets/pixellab/decor (PixelLab,
//! provenance in SOURCES.md).

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

/// One kind of scenery.
pub struct DecorKind {
    pub name: &'static str,
    pub png: &'static [u8],
    /// On-screen size range, virtual px.
    pub size: (f32, f32),
    /// Screen px moved per world px the camera moves: small is far.
    pub parallax: f32,
    /// Relative chance of being picked.
    pub weight: u32,
    /// Its own motion: screen px per tick, and degrees per tick of spin.
    pub drift: (f32, f32),
    pub spin: f32,
    /// A resting tilt (degrees), for things drawn pointing up.
    pub tilt: f32,
    /// Bobs gently up and down.
    pub bob: bool,
    /// An oddity: rare, and reported when seen (achievement hook).
    pub rare: bool,
    /// Multiplied into the art: dims and fades it into the distance.
    pub tint: (u8, u8, u8, u8),
}

macro_rules! png {
    ($n:literal) => {
        include_bytes!(concat!("../../../assets/pixellab/decor/", $n, ".png"))
    };
}

// Atmospheric perspective: the further, the darker, bluer and fainter —
// full-strength art sat in the node band and competed with the nodes.
const FAR: (u8, u8, u8, u8) = (118, 124, 158, 175);
const MID: (u8, u8, u8, u8) = (150, 156, 186, 200);
const NEAR: (u8, u8, u8, u8) = (182, 186, 210, 220);

pub const DECOR_KINDS: &[DecorKind] = &[
    // Planets: big, far, slow.
    DecorKind { name: "ringed gas giant", png: png!("planet_ringed"), size: (620.0, 900.0), parallax: 0.09, weight: 10, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "ice world", png: png!("planet_ice"), size: (420.0, 640.0), parallax: 0.10, weight: 10, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "desert world", png: png!("planet_desert"), size: (420.0, 640.0), parallax: 0.10, weight: 10, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "ocean world", png: png!("planet_ocean"), size: (460.0, 700.0), parallax: 0.10, weight: 10, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "violet giant", png: png!("planet_violet"), size: (560.0, 820.0), parallax: 0.08, weight: 9, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "lava world", png: png!("planet_lava"), size: (380.0, 560.0), parallax: 0.11, weight: 8, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "cratered moon", png: png!("moon_cratered"), size: (300.0, 460.0), parallax: 0.13, weight: 10, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: FAR },
    DecorKind { name: "spiral galaxy", png: png!("galaxy_spiral"), size: (700.0, 1000.0), parallax: 0.05, weight: 6, drift: (0.0, 0.0), spin: 0.01, tilt: 0.0, bob: false, rare: false, tint: (110, 116, 150, 160) },
    // Middle distance.
    DecorKind { name: "small ringed planet", png: png!("ringed_small"), size: (200.0, 300.0), parallax: 0.16, weight: 7, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: MID },
    DecorKind { name: "tiny moon", png: png!("tiny_moon"), size: (150.0, 230.0), parallax: 0.17, weight: 6, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: false, rare: false, tint: MID },
    DecorKind { name: "ring station", png: png!("station"), size: (240.0, 340.0), parallax: 0.18, weight: 6, drift: (0.0, 0.0), spin: 0.05, tilt: 0.0, bob: false, rare: false, tint: MID },
    DecorKind { name: "satellite", png: png!("satellite"), size: (170.0, 240.0), parallax: 0.22, weight: 7, drift: (-0.3, 0.05), spin: 0.08, tilt: 0.0, bob: false, rare: false, tint: NEAR },
    DecorKind { name: "derelict", png: png!("derelict"), size: (220.0, 320.0), parallax: 0.2, weight: 5, drift: (-0.4, 0.1), spin: 0.03, tilt: -8.0, bob: false, rare: false, tint: MID },
    DecorKind { name: "asteroids", png: png!("asteroids"), size: (200.0, 300.0), parallax: 0.2, weight: 7, drift: (0.0, 0.0), spin: 0.06, tilt: 0.0, bob: false, rare: false, tint: MID },
    DecorKind { name: "crystals", png: png!("crystals"), size: (170.0, 240.0), parallax: 0.2, weight: 4, drift: (0.0, 0.0), spin: 0.04, tilt: 0.0, bob: true, rare: false, tint: NEAR },
    DecorKind { name: "space buoy", png: png!("buoy"), size: (130.0, 180.0), parallax: 0.24, weight: 3, drift: (0.0, 0.0), spin: 0.0, tilt: 0.0, bob: true, rare: false, tint: NEAR },
    // Movers: across the sky on their own.
    DecorKind { name: "comet", png: png!("comet"), size: (220.0, 320.0), parallax: 0.15, weight: 5, drift: (7.5, 1.6), spin: 0.0, tilt: 12.0, bob: false, rare: false, tint: NEAR },
    DecorKind { name: "rocket", png: png!("rocket"), size: (150.0, 210.0), parallax: 0.2, weight: 3, drift: (-3.0, -3.2), spin: 0.0, tilt: -43.0, bob: false, rare: false, tint: NEAR },
    // Oddities: rare, and worth pointing out to a friend.
    DecorKind { name: "UFO", png: png!("ufo"), size: (180.0, 240.0), parallax: 0.22, weight: 1, drift: (-1.6, 0.0), spin: 0.0, tilt: 0.0, bob: true, rare: true, tint: NEAR },
    DecorKind { name: "space whale", png: png!("space_whale"), size: (520.0, 700.0), parallax: 0.1, weight: 1, drift: (-1.0, -0.15), spin: 0.0, tilt: 0.0, bob: true, rare: true, tint: MID },
    DecorKind { name: "astronaut", png: png!("astronaut"), size: (150.0, 200.0), parallax: 0.24, weight: 1, drift: (-0.5, 0.2), spin: 0.35, tilt: 0.0, bob: false, rare: true, tint: NEAR },
    DecorKind { name: "space jellyfish", png: png!("jellyfish"), size: (190.0, 260.0), parallax: 0.2, weight: 1, drift: (0.0, -0.6), spin: 0.0, tilt: 0.0, bob: true, rare: true, tint: NEAR },
    DecorKind { name: "rubber duck", png: png!("rubber_duck"), size: (120.0, 160.0), parallax: 0.26, weight: 1, drift: (-0.2, 0.1), spin: 0.25, tilt: 0.0, bob: false, rare: true, tint: NEAR },
];

/// Pooled decor objects, and the most that may be live at once.
pub const DECOR_POOL: usize = 4;
pub const DECOR_MAX_LIVE: usize = 3;
/// World px of camera travel between pieces, and before the first one.
pub const DECOR_GAP: (f32, f32) = (7_500.0, 13_500.0);
pub const DECOR_FIRST_AT: f32 = 1_800.0;
/// The band of sky decor may occupy (screen y of its centre): above the
/// cloud tops at the bottom of the backdrop, below the HUD.
pub const DECOR_Y: (f32, f32) = (260.0, 1_150.0);

pub fn decor_id(i: usize) -> String {
    format!("bg_decor_{i}")
}

/// The pooled objects, hidden, for the scene builder. Inserted straight
/// after the backdrop so they draw behind everything else on layer 0.
pub fn decor_objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    (0..DECOR_POOL)
        .map(|i| {
            let id = decor_id(i);
            let mut o = GameObject::new_rect(
                ctx, id.clone().into(),
                Some(Image { shape: ShapeType::Rectangle(0.0, (64.0, 64.0), 0.0),
                             image: Arc::new(image::RgbaImage::new(1, 1)), color: None }),
                (64.0, 64.0), (-9000.0, -9000.0), vec!["decor".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
            );
            o.ignore_zoom = true;
            o.gravity = 0.0;
            o.visible = false;
            (id, o)
        })
        .collect()
}

/// Weighted pick, never the kind that was picked last, and at most one
/// oddity at a time on screen.
pub fn pick_kind(seed: &mut u64, last: Option<usize>, rare_live: bool) -> usize {
    let ok = |i: usize| Some(i) != last && !(rare_live && DECOR_KINDS[i].rare);
    let total: u32 = (0..DECOR_KINDS.len()).filter(|&i| ok(i)).map(|i| DECOR_KINDS[i].weight).sum();
    let mut r = (lcg(seed) * total.max(1) as f32) as u32;
    for i in 0..DECOR_KINDS.len() {
        if !ok(i) { continue; }
        let w = DECOR_KINDS[i].weight;
        if r < w {
            return i;
        }
        r -= w;
    }
    0
}

pub fn tick_decor(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let cam = c.camera().map(|cam| cam.position).unwrap_or((0.0, 0.0));
    let (hidden, first) = {
        let s = st.lock().unwrap();
        (s.in_space_mode || s.boss_active || s.dead || s.space_launch_active, s.decor_cam_prev.is_none())
    };
    if hidden {
        let mut s = st.lock().unwrap();
        s.decor_cam_prev = Some(cam);
        let live: Vec<usize> = s.decor.iter().enumerate().filter(|(_, d)| d.active).map(|(i, _)| i).collect();
        for d in s.decor.iter_mut() { d.active = false; }
        drop(s);
        for i in live {
            if let Some(o) = c.get_game_object_mut(&decor_id(i)) { o.visible = false; }
        }
        return;
    }
    if first {
        st.lock().unwrap().decor_cam_prev = Some(cam);
    }

    let mut s = st.lock().unwrap();
    if s.decor.len() < DECOR_POOL {
        s.decor.resize(DECOR_POOL, DecorSlot::default());
    }
    let prev = s.decor_cam_prev.unwrap_or(cam);
    let (dx, dy) = (cam.0 - prev.0, cam.1 - prev.1);
    s.decor_cam_prev = Some(cam);
    // A camera jump (respawn, warp) is not travel: ignore it.
    let (dx, dy) = if dx.abs() > 2_000.0 || dy.abs() > 2_000.0 { (0.0, 0.0) } else { (dx, dy) };
    if dx > 0.0 {
        s.decor_travel += dx;
    }
    s.decor_ticks = s.decor_ticks.wrapping_add(1);
    let t = s.decor_ticks as f32;

    // Spawn: just off the right edge, clear of whatever is already up.
    let live = s.decor.iter().filter(|d| d.active).count();
    if s.decor_travel >= s.decor_next_at && live < DECOR_MAX_LIVE {
        if let Some(slot) = s.decor.iter().position(|d| !d.active) {
            let rare_live = s.decor.iter().any(|d| d.active && DECOR_KINDS[d.kind].rare);
            let last = s.decor_last_kind;
            let mut seed = s.seed ^ (s.decor_travel as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let kind = pick_kind(&mut seed, last, rare_live);
            let k = &DECOR_KINDS[kind];
            let size = lcg_range(&mut seed, k.size.0, k.size.1);
            // A few tries for a height that does not overlap a live piece.
            let mut y = lcg_range(&mut seed, DECOR_Y.0, DECOR_Y.1);
            for _ in 0..6 {
                let clash = s.decor.iter().any(|d| {
                    d.active && (d.y - y).abs() < (d.size + size) * 0.5 + 120.0 && d.x > VW * 0.55
                });
                if !clash { break; }
                y = lcg_range(&mut seed, DECOR_Y.0, DECOR_Y.1);
            }
            let x = if k.drift.0 > 0.5 { -size * 0.6 } else { VW + size * 0.6 };
            s.decor[slot] = DecorSlot { active: true, kind, x, y, size, rot: k.tilt, phase: lcg(&mut seed) * 6.283, seen: false };
            s.decor_last_kind = Some(kind);
            let gap = lcg_range(&mut seed, DECOR_GAP.0, DECOR_GAP.1);
            s.decor_next_at = s.decor_travel + gap;
            s.seed = seed;
        }
    }
    if s.decor_next_at <= 0.0 {
        s.decor_next_at = DECOR_FIRST_AT;
    }

    // Move: parallax with the camera, plus each kind's own drift.
    let mut rare_seen: Option<&'static str> = None;
    let mut draw: Vec<(usize, DecorSlot)> = Vec::new();
    let mut gone: Vec<usize> = Vec::new();
    for (i, d) in s.decor.iter_mut().enumerate() {
        if !d.active { continue; }
        let k = &DECOR_KINDS[d.kind];
        d.x += -dx * k.parallax + k.drift.0;
        d.y += -dy * k.parallax * 0.35 + k.drift.1;
        d.rot += k.spin;
        let half = d.size * 0.6;
        let on_screen = d.x > half && d.x < VW - half;
        if on_screen && k.rare && !d.seen {
            d.seen = true;
            rare_seen = Some(k.name);
        }
        // Off either side, or drifted far out of the sky band: done.
        if d.x < -d.size || d.x > VW + d.size * 1.2 || d.y < -d.size || d.y > VH + d.size {
            d.active = false;
            gone.push(i);
            continue;
        }
        draw.push((i, *d));
    }
    drop(s);

    for i in gone {
        if let Some(o) = c.get_game_object_mut(&decor_id(i)) { o.visible = false; }
    }
    for (i, d) in draw {
        let k = &DECOR_KINDS[d.kind];
        let bob = if k.bob { (t * 0.03 + d.phase).sin() * d.size * 0.04 } else { 0.0 };
        // Quantised so the upscale cache holds a handful of sizes, not one
        // per spawn.
        let px = ((d.size / 32.0).round() * 32.0).max(32.0);
        let Some(img) = crate::scenes::game::helpers::pl_image_cached(k.png, px) else { continue };
        if let Some(o) = c.get_game_object_mut(&decor_id(i)) {
            o.size = (d.size, d.size);
            o.position = (d.x - d.size * 0.5, d.y - d.size * 0.5 + bob);
            o.rotation = d.rot;
            o.visible = true;
            let (r, g, b, a) = k.tint;
            o.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (d.size, d.size), 0.0),
                image: img,
                color: Some(Color(r, g, b, a)),
            });
        }
    }
    if let Some(name) = rare_seen {
        // For achievements ("I saw a whale!"): the oddity just came into view.
        c.set_var("decor_rare_seen", Value::Str(name.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_piece_of_decor_decodes() {
        for k in DECOR_KINDS {
            assert!(image::load_from_memory(k.png).is_ok(), "{} does not decode", k.name);
            assert!(k.size.0 <= k.size.1 && k.parallax > 0.0 && k.weight > 0, "{}", k.name);
        }
    }

    #[test]
    fn oddities_are_rare_and_never_twice_in_a_row() {
        let mut seed = 12345u64;
        let mut last = None;
        let (mut rare, mut n) = (0, 0);
        for _ in 0..4000 {
            let k = pick_kind(&mut seed, last, false);
            assert_ne!(Some(k), last, "the same piece twice in a row");
            if DECOR_KINDS[k].rare { rare += 1; }
            n += 1;
            last = Some(k);
        }
        let share = rare as f32 / n as f32;
        assert!(share > 0.02 && share < 0.12, "oddities are {:.1}% of the sky", share * 100.0);
        // Never a second oddity while one is up.
        for _ in 0..500 {
            let k = pick_kind(&mut seed, None, true);
            assert!(!DECOR_KINDS[k].rare);
        }
    }
}
