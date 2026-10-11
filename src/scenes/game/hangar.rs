//! The station hangar: now and then the run passes through a station's
//! hangar bay, open at both ends.
//!
//! It is the same facility as the screen transition's blast door, built the
//! same way: every point of it has a height and a material, and it is shaded
//! from that relief, lit by its own lights, polished, and its lights bloom.
//!   * The bay repeats every `TILE_W`: a chrome spine at each bay seam (the
//!     door's spine, red lock lights at the platform), chamfered insets with
//!     fin banks or viewports, cyan strips and hydraulic lines tracing their
//!     chamfers; a ceiling beam whose light bars throw pools and shafts down
//!     the wall; a platform across the middle with downlights under it; the
//!     floor's hazard band at the fall line. Viewport bays and bulkhead bays
//!     (a blast door onto the platform) alternate.
//!   * The hull above and below: armour plates, dark accent wedges edged in
//!     cyan, caged lamps, the spine running through.
//!   * Each end is the transition's blast door standing open: its halves
//!     drawn back into the housing, their slanted hazard edges and red lock
//!     lights showing; the housing's lip lit cyan, glowing into the opening.
//! All of it is drawn here, in code, once (see `render`).
//!
//! It comes along every so often (`HANGAR_GAP`), never in space or a fight;
//! H starts one just ahead (testing).

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;

/// One repeat of the bay, world px.
const TILE_W: f32 = 1600.0;
/// World heights: the ceiling's underside, the floor's top (the fall line),
/// and how far the hull reaches beyond each.
const CEIL_Y: f32 = -1400.0;
const DECK_Y: f32 = VH + 150.0;
const HULL: f32 = 1200.0;
/// The platform across the middle: its deck's surface (world y; anything
/// walking it stands here).
const WALK_Y: f32 = 1150.0;
/// The end housing: how far it reaches out past the bay, and the pillar's
/// width inside it.
const MOUTH_OUT: f32 = 760.0;
const PILLAR_W: f32 = 300.0;
/// Overlap of each bay piece onto the next (real art, not a copy), so the
/// joins never open.
const OVERLAP: f32 = 40.0;
/// Pieces in the pool (the longest hangar is this many tiles).
const MAX_TILES: usize = 6;
/// Distance before the first hangar, and between hangars (px).
const HANGAR_FIRST: f32 = 18_000.0;
const HANGAR_GAP: (f32, f32) = (60_000.0, 95_000.0);
/// How far ahead of the player a hangar opens.
const HANGAR_AHEAD: f32 = VW * 1.4;
/// Art is drawn at this share of world size, then stretched. Every texture
/// stays under ~1,024 px a side: the renderer shrinks taller ones, and a
/// shrunk texture drew short of its object.
const ART: f32 = 1000.0 / (DECK_Y - CEIL_Y);

const LAYER: i32 = 0;

#[derive(Default)]
struct Hangar {
    next_at: f32,
    /// The hangar in progress: its start x and length.
    span: Option<(f32, f32)>,
    rng: u32,
    h_was_down: bool,
}

fn state() -> &'static Mutex<Hangar> {
    static S: OnceLock<Mutex<Hangar>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Hangar::default()))
}

/// A run is starting.
pub fn reset() {
    // `HANGAR_NOW=1`: the first one straight away (testing).
    let first = if std::env::var("HANGAR_NOW").is_ok() { 0.0 } else { HANGAR_FIRST };
    *state().lock().unwrap() = Hangar { next_at: first, span: None, rng: 0x9A46_A511, h_was_down: false };
    // Build the art now, off the game thread, so the first hangar is ready.
    static WARM: OnceLock<()> = OnceLock::new();
    WARM.get_or_init(|| { std::thread::spawn(|| { let _ = art(); for t in 3..=5 { let _ = outer(t); } }); });
}

// ── Materials, after the transition's door ──────────────────────────────────

const STEEL: [f32; 3] = [58.0, 74.0, 82.0];
const ACCENT: [f32; 3] = [32.0, 42.0, 48.0];
const RECESS: [f32; 3] = [30.0, 38.0, 44.0];
const FIN: [f32; 3] = [70.0, 84.0, 92.0];
const BRIGHT: [f32; 3] = [86.0, 100.0, 108.0];
const DARK: [f32; 3] = [20.0, 24.0, 28.0];
const RED: [f32; 3] = [255.0, 40.0, 45.0];
const STRIP: [f32; 3] = [140.0, 215.0, 245.0];
const LAMP: [f32; 3] = [200.0, 235.0, 255.0];
const HAZ_RED: [f32; 3] = [150.0, 26.0, 30.0];
const HAZ_WHITE: [f32; 3] = [196.0, 196.0, 190.0];

/// One point of the station: its height (relief), its colour, the light it
/// gives off, whether it is there at all, and whether it lights itself
/// (glass, lamps: not shaded by the scene's light).
#[derive(Clone, Copy)]
struct Spot { h: f32, col: [f32; 3], emit: [f32; 3], solid: bool, own: bool, glass: f32 }

impl Spot {
    fn new(h: f32, col: [f32; 3]) -> Self { Spot { h, col, emit: [0.0; 3], solid: true, own: false, glass: 0.0 } }
    fn empty() -> Self { Spot { h: 0.0, col: [0.0; 3], emit: [0.0; 3], solid: false, own: false, glass: 0.0 } }
    fn glow(mut self, c: [f32; 3], k: f32) -> Self { for i in 0..3 { self.emit[i] += c[i] * k; } self }
}

fn hash2(i: i32, j: i32, salt: u32) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x8da6_b343) ^ (j as u32).wrapping_mul(0xd816_3841) ^ salt.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13; h = h.wrapping_mul(0x5bd1_e995); h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

/// A chrome cylinder across its width: `u` -1..1 → (height, colour).
fn cyl(u: f32) -> (f32, [f32; 3]) {
    let curve = (1.0 - u * u).max(0.0).sqrt();
    let shine = (1.0 - ((u + 0.35) / 0.22).abs()).clamp(0.0, 1.0);
    let back = (1.0 - ((u - 0.55) / 0.2).abs()).clamp(0.0, 1.0) * 0.3;
    let v = 34.0 + 80.0 * curve + 150.0 * shine + 40.0 * back;
    (0.7 + 0.5 * curve, [v * 0.92, v * 1.0, v * 1.06])
}
fn chrome(u: f32) -> Spot { let (h, c) = cyl(u); Spot::new(h, c) }

/// Red and white hazard stripes, `t` along the band.
fn hazard(t: f32) -> Spot {
    let stripe = ((t / 64.0).floor() as i32).rem_euclid(2) == 0;
    Spot::new(1.05, if stripe { HAZ_RED } else { HAZ_WHITE })
}

/// A lit strip in a dark channel.
fn strip(k: f32) -> Spot { Spot::new(0.8, [30.0, 60.0, 70.0]).glow(STRIP, k) }

/// A caged lamp (the door's), centred at the origin: a bright cell behind
/// bars. Lights itself.
fn caged_lamp(lx: f32, ly: f32) -> Option<Spot> {
    if lx.abs() >= 40.0 || ly.abs() >= 84.0 { return None; }
    let cage = ly.abs() > 77.0 || lx.abs() > 34.0 || ly.rem_euclid(33.0) < 5.0;
    let mut s = Spot::new(1.2, if cage { [120.0, 186.0, 224.0] } else { [175.0, 245.0, 255.0] });
    s.own = true;
    Some(s.glow([70.0, 170.0, 230.0], if cage { 0.3 } else { 0.8 }))
}

/// An up-pointing triangle: the distance in from its nearest edge.
fn tri(x: f32, y: f32, s: f32) -> f32 {
    let h = s * 0.866;
    let e_bot = h / 3.0 - y;
    let m = (y + h * 2.0 / 3.0) * (s / (2.0 * h)) - x.abs();
    e_bot.min(m * 0.866)
}

/// The caution sign (the door's): a white triangle, red border, red mark.
fn caution(x: f32, y: f32, size: f32) -> Option<Spot> {
    let t = tri(x, y, size);
    if t <= 0.0 { return None; }
    let red = [200.0, 34.0, 36.0];
    if t < size * 0.07 { return Some(Spot::new(1.1, red)); }
    let s = size / 140.0;
    let ex = x.abs() < 7.0 * s && ((y > -34.0 * s && y < 9.0 * s) || (y - 24.0 * s).abs() < 7.0 * s);
    Some(Spot::new(1.1, if ex { red } else { [232.0, 232.0, 226.0] }))
}

/// A red lock light, `d` across its depth (0..32), lit at its core.
fn lock_light(d: f32) -> Spot {
    let core = 1.0 - ((d - 17.0).abs() / 14.0).powi(4);
    Spot::new(0.9, [70.0, 10.0, 12.0]).glow([255.0, 120.0, 110.0], core.max(0.0) * 1.1).glow(RED, 0.55)
}

// ── The bay ─────────────────────────────────────────────────────────────────
//
// In bay coordinates: `bx` 0..TILE_W across, world y down. Symmetric about
// the bay's middle (800): the spine sits on each bay seam, so `e` (distance
// to the nearest seam) lays out the sides.

const SPINE: f32 = 130.0;
const PISTON: f32 = 270.0;
/// The insets' sides, and their chamfers.
const INSET: (f32, f32) = (330.0, 1270.0);
const CUT: f32 = 170.0;
/// Bands, world y.
const BEAM: (f32, f32) = (CEIL_Y, -1150.0);
const UPPER: (f32, f32) = (-1040.0, -220.0);
const PIPES: (f32, f32) = (-150.0, -40.0);
const MID: (f32, f32) = (60.0, 800.0);
/// The platform: its deck's back edge (it is seen from a little above, so
/// its top shows, running back to the wall, as the floor's does), and its
/// front face under the deck's lip.
const WALK_BACK: f32 = 900.0;
const SLAB: (f32, f32) = (WALK_Y, 1290.0);
/// The floor's top seen from above: its back edge at the wall, its front
/// edge the fall line (`DECK_Y`).
/// The engine room below the deck: its grated floor (world y).
const GRATE: f32 = 2200.0;
/// The station's gear outside: the band above the upper hull and below the
/// lower one (trusses, solar wings, radiators, an antenna; pods).
const DRESS: f32 = 900.0;
/// Outside, from the bay outwards (world units): the roof's skin, the
/// pylons, then the module above (a long capsule) and the engine nacelle
/// below, inset from the ends; the end housings' depth.
const ROOF: f32 = 190.0;
const MODULE: (f32, f32) = (330.0, 1050.0);
const MODULE_IN: f32 = 420.0;
const KEEL: f32 = 230.0;
const NACELLE: (f32, f32) = (360.0, 900.0);
const NACELLE_IN: (f32, f32) = (300.0, 620.0);
const HOUSING: f32 = 480.0;

enum Fill { Fins, Glass }

/// A recessed inset after the door's: chamfered at the two corners on its
/// "seam" side (the bottom, or the top), a cyan strip and two hydraulic
/// lines inside along that side, fins or glass within, a bevelled frame, an
/// engraved outline and a rivet row outside it. `None` well outside it.
fn inset(bx: f32, y: f32, y0: f32, y1: f32, seam_bottom: bool, fill: Fill) -> Option<Spot> {
    const R2: f32 = std::f32::consts::SQRT_2;
    let (x0, x1) = INSET;
    let dv = if seam_bottom { y1 - y } else { y - y0 };
    let far = if seam_bottom { y - y0 } else { y1 - y };
    let cut = (bx - x0) + dv - CUT;
    let cut2 = (x1 - bx) + dv - CUT;
    let inside = bx > x0 && bx < x1 && y > y0 && y < y1 && cut > 0.0 && cut2 > 0.0;
    let seam_side = dv.min(cut / R2).min(cut2 / R2);
    let edge = (bx - x0).min(x1 - bx).min(far).min(seam_side);
    if !inside && edge < -90.0 { return None; }
    if inside {
        match fill {
            Fill::Glass => {
                // The frame's inner lip, then the glass: see-through (the
                // sky, its stars and whatever drifts past show behind it),
                // faintly tinted blue, two reflections across it, darker and
                // a cyan rim of light round its edge.
                if edge < 14.0 { return Some(Spot::new(1.15, BRIGHT)); }
                let refl = (-(((bx - 800.0) * 0.6 + (y - y0) - 300.0) / 60.0).powi(2)).exp() * 0.10
                    + (-(((bx - 800.0) * 0.6 + (y - y0) - 420.0) / 20.0).powi(2)).exp() * 0.07;
                let mut s = Spot::empty();
                s.col = [14.0, 30.0, 52.0];
                s.glass = 0.16 + 0.22 * (1.0 - ((edge - 14.0) / 60.0).clamp(0.0, 1.0));
                let rim = (1.0 - (edge - 14.0) / 10.0).clamp(0.0, 1.0);
                return Some(s.glow([200.0, 225.0, 255.0], refl).glow(STRIP, rim * 0.5));
            }
            Fill::Fins => {
                for off in [20.0, 50.0] {
                    if (seam_side - off).abs() < 11.0 && edge > off - 11.0 {
                        let mut s = chrome((seam_side - off) / 11.0);
                        if ((bx + dv) % 240.0) < 18.0 { s = Spot::new(1.2, [96.0, 112.0, 120.0]); }
                        return Some(s);
                    }
                }
                let mut s = Spot::new(0.5, RECESS);
                let lx = bx - x0;
                let (f0, f1) = if seam_bottom { (y0 + 80.0, y1 - 150.0) } else { (y0 + 150.0, y1 - 80.0) };
                let (fx0, fx1) = (90.0, (x1 - x0) - 90.0);
                if y > f0 && y < f1 && lx > fx0 && lx < fx1 && seam_side > 104.0 {
                    let f = (lx - fx0) % 26.0;
                    s = Spot::new(if f < 9.0 { 0.42 } else { 0.78 }, FIN);
                    if y < f0 + 12.0 || y > f1 - 12.0 || lx < fx0 + 12.0 || lx > fx1 - 12.0 || seam_side < 116.0 {
                        s = Spot::new(0.9, BRIGHT);
                    }
                }
                // Indicator lights on the far side, near the end.
                let iy = if seam_bottom { y0 + 44.0 } else { y1 - 44.0 };
                for k in 0..3 {
                    let r = ((lx - (fx1 - 30.0 - k as f32 * 46.0)).powi(2) + (y - iy).powi(2)).sqrt();
                    if r < 10.0 { s = Spot::new(0.6, [20.0, 60.0, 70.0]).glow(STRIP, 0.9); }
                }
                let ps = seam_side - 76.0;
                if ps.abs() < 5.0 { s = s.glow(STRIP, 1.0); }
                return Some(s);
            }
        }
    }
    if edge > -18.0 { return Some(Spot::new(1.1, STEEL)); }
    let out = -edge;
    if (out - 46.0).abs() < 2.5 { return Some(Spot::new(0.75, STEEL)); }
    if (out - 74.0).abs() < 9.0 {
        let along = (bx + y).rem_euclid(120.0);
        if ((along - 60.0).powi(2) + (out - 74.0).powi(2)).sqrt() < 8.0 { return Some(Spot::new(1.25, STEEL)); }
    }
    None
}

/// The spine on a bay seam (the door's): stacked plates between chrome
/// rails, a red lock light at the platform; a piston bay either side.
/// `e` is the distance from the seam, `y` world.
fn spine(e: f32, y: f32) -> Spot {
    if e < SPINE {
        let ur = (e - (SPINE - 22.0)) / 14.0;
        if ur.abs() < 1.0 { return chrome(ur); }
        if e > SPINE - 6.0 { return Spot::new(0.35, DARK); }
        let ly = y - (WALK_BACK - 70.0);
        if e < 60.0 && ly.abs() < 32.0 { return lock_light(ly + 16.0); }
        let mut s = Spot::new(1.2, STEEL);
        let local = (y - CEIL_Y).rem_euclid(250.0);
        if local < 9.0 { s.h = 0.3; }
        let ring = (e - 82.0).abs().min((local - 34.0).abs()).min((local - 216.0).abs());
        if ring < 3.0 && e < 85.0 && local > 31.0 && local < 219.0 { s.h = 0.85; }
        for by in [20.0, 230.0] {
            if ((e - 70.0).powi(2) + (local - by).powi(2)).sqrt() < 9.0 { s.h = 1.35; }
        }
        return s;
    }
    // The piston bay: a dark recess with a chrome cylinder and a thin line.
    let mut s = Spot::new(0.35, RECESS);
    let u = (e - 205.0) / 32.0;
    if u.abs() < 1.0 { s = chrome(u); }
    let u2 = (e - 152.0) / 9.0;
    if u2.abs() < 1.0 { let (h, c) = cyl(u2); s = Spot::new(h * 0.9, c); }
    if (y - CEIL_Y).rem_euclid(210.0) < 26.0 && (e - 205.0).abs() < 46.0 { s = Spot::new(1.3, STEEL); }
    if e > PISTON - 10.0 { s = Spot::new(0.2, DARK); }
    s
}

/// The bulkhead bay's blast door onto the platform (a small one of the
/// transition's): a slanted seam, hazard bands along it, red lock lights,
/// caution signs; pistons and caged lamps beside it.
fn blast_door(bx: f32, y: f32) -> Option<Spot> {
    let (d0, d1, top) = (540.0, 1060.0, 340.0);
    if let Some(l) = caged_lamp(bx - 400.0, y - 620.0).or_else(|| caged_lamp(bx - 1200.0, y - 620.0)) { return Some(l); }
    for cx in [480.0, 1120.0] {
        if (bx - cx).abs() < 30.0 && y > top && y < WALK_BACK {
            let u = (bx - cx) / 18.0;
            if u.abs() < 1.0 {
                let mut s = chrome(u);
                if (y - top).rem_euclid(160.0) < 20.0 { s = Spot::new(1.3, STEEL); }
                return Some(s);
            }
            return Some(Spot::new(0.35, RECESS));
        }
    }
    if bx < d0 - 40.0 || bx > d1 + 40.0 || y < top - 40.0 || y > WALK_BACK { return None; }
    if bx < d0 || bx > d1 || y < top {
        return Some(Spot::new(if bx < d0 - 34.0 || bx > d1 + 34.0 || y < top - 34.0 { 1.3 } else { 1.15 }, STEEL));
    }
    let seam = 760.0 - (bx - 800.0) * 0.12;
    let d = (y - seam).abs();
    if d < 5.0 { return Some(Spot::new(0.3, [10.0, 12.0, 14.0])); }
    if (bx - 800.0).abs() < 46.0 && d < 32.0 { return Some(lock_light(d)); }
    if d < 44.0 {
        if d > 39.0 { return Some(Spot::new(0.3, [24.0, 28.0, 32.0])); }
        return Some(hazard(bx + d));
    }
    for cy in [seam - 210.0, seam + 200.0] {
        if let Some(c) = caution(bx - 800.0, y - cy, 90.0) { return Some(c); }
    }
    if (bx - 800.0).abs() < 70.0 {
        let mut s = Spot::new(1.2, STEEL);
        if (y - seam).rem_euclid(120.0) < 6.0 { s.h = 0.35; }
        if (bx - 800.0).abs() > 64.0 { s.h = 0.4; }
        return Some(s);
    }
    let mut s = Spot::new(1.0, STEEL);
    let ind = (bx - d0).min(d1 - bx).min(y - top).min(WALK_BACK - y).min((d - 44.0) * 2.0);
    if (ind - 34.0).abs() < 2.5 { s.h = 0.7; }
    Some(s)
}

/// A deck's top surface seen from a little above, `t` 0 at its back edge to
/// 1 at its front: plates in rows that grow towards the viewer, their seams
/// slanting the same way (one oblique view, so the bays agree), darker back
/// at the wall, a glossy reflection of the ceiling's light bars, a bright
/// lip at the front.
fn deck_top(bx: f32, t: f32, rows: f32) -> Spot {
    if t > 0.93 { return Spot::new(1.4, [150.0, 160.0, 168.0]); }
    // Perspective rows: near ones taller.
    let u = ((1.0 / (0.5 + 0.5 * t)) - 1.0) / (1.0 / 0.5 - 1.0); // 1 back .. 0 front
    let row = u * rows;
    let xs = bx + (1.0 - t) * 90.0;
    let mut s = Spot::new(1.0, [122.0, 134.0, 142.0]);
    if row.fract() < 0.06 * (1.0 + u) { s = Spot::new(0.45, [70.0, 80.0, 88.0]); }
    if (xs.rem_euclid(200.0)) < 4.0 + 3.0 * t { s = Spot::new(0.5, [70.0, 80.0, 88.0]); }
    // A yellow-black safety line a little in from the front.
    if (t - 0.82).abs() < 0.035 { s = if ((bx + t * 40.0) / 40.0).floor() as i32 % 2 == 0 { Spot::new(1.0, [210.0, 170.0, 40.0]) } else { Spot::new(1.0, [36.0, 34.0, 30.0]) }; }
    // Back at the wall: the corner's shadow.
    let ao = 0.55 + 0.45 * t.powf(0.6);
    s.col = s.col.map(|v| v * ao);
    // The light bars mirrored in the polished deck.
    for lx in [520.0, 1080.0] {
        let d = (bx - lx).abs();
        let k = (-(d / 70.0).powi(2)).exp() * (0.25 + 0.75 * t);
        s = s.glow([150.0, 180.0, 200.0], k * 0.35);
    }
    s
}

/// The platform: its deck, the slab (hazard-banded face, cyan strip
/// under its lip, downlights beneath). `None` where the wall shows through.
fn platform(bx: f32, y: f32) -> Option<Spot> {
    if y >= WALK_BACK && y < SLAB.0 { return Some(deck_top(bx, (y - WALK_BACK) / (SLAB.0 - WALK_BACK), 3.0)); }
    // (No railing: the deck is the floor of this level, as the bottom one is.)
    if y < WALK_BACK { return None; }
    if y >= SLAB.0 && y < SLAB.1 {
        // The deck's front face, as thick as the floor's: a hazard band.
        if y < SLAB.0 + 16.0 { return Some(Spot::new(1.1, STEEL)); }
        if y < SLAB.0 + 52.0 { return Some(hazard(bx + y)); }
        if y < SLAB.0 + 70.0 { return Some(Spot::new(1.05, STEEL)); }
        if y > SLAB.1 - 14.0 && y < SLAB.1 - 7.0 { return Some(strip(0.9)); }
        if y >= SLAB.1 - 7.0 {
            for lx in [400.0, 800.0, 1200.0] {
                if (bx - lx).abs() < 22.0 { return Some(Spot::new(0.8, LAMP).glow(LAMP, 0.9)); }
            }
            return Some(Spot::new(0.3, DARK));
        }
        return Some(Spot::new(1.1, STEEL));
    }
    None
}

/// The ceiling beam: bolts and an engraved line on its face, light bars on
/// its underside (the bay's primary light).
fn beam(bx: f32, y: f32) -> Spot {
    if y > BEAM.1 - 44.0 {
        for lx in [520.0, 1080.0] {
            if (bx - lx).abs() < 120.0 && y > BEAM.1 - 34.0 && y < BEAM.1 - 8.0 {
                let mut l = Spot::new(1.0, LAMP);
                l.own = true;
                if (bx - lx + 120.0).rem_euclid(30.0) < 4.0 { l.col = [120.0, 150.0, 170.0]; return l.glow(LAMP, 0.4); }
                return l.glow(LAMP, 1.4);
            }
        }
        return Spot::new(0.4, RECESS);
    }
    let mut s = Spot::new(1.25, STEEL);
    if (y - (BEAM.0 + 110.0)).abs() < 3.0 { s.h = 0.7; }
    if ((y - (BEAM.0 + 60.0)).abs() < 9.0 || (y - (BEAM.0 + 170.0)).abs() < 9.0) && (bx.rem_euclid(120.0) - 60.0).abs() < 9.0 { s.h = 1.45; }
    s
}

/// The bay (no mouths), at `bx` in bay `k` (viewports on even bays).
fn bay(k: i32, bx: f32, y: f32) -> Spot {
    let window = k.rem_euclid(2) == 0;
    let e = bx.min(TILE_W - bx);
    if y >= BEAM.0 && y < BEAM.1 { return beam(bx, y); }
    // Hazard trim along the beam's foot.
    if y >= BEAM.1 && y < BEAM.1 + 34.0 {
        if y > BEAM.1 + 28.0 { return Spot::new(0.3, DARK); }
        return hazard(bx + y);
    }
    // The deck first: it stands out in front of the wall and its spines.
    if y >= WALK_BACK && y < SLAB.1 { if let Some(p) = platform(bx, y) { return p; } }
    if y >= SLAB.1 && y >= GRATE { return engine_room(k, bx, y); }
    if e < PISTON { return spine(e, y); }
    if let Some(p) = platform(bx, y) { return p; }
    // Hydraulic conduits across the bay, under the upper inset.
    for c in [PIPES.0 + 30.0, PIPES.1 - 30.0] {
        let u = (y - c) / 16.0;
        if u.abs() < 1.0 {
            let mut s = chrome(u);
            if bx.rem_euclid(400.0) < 22.0 { s = Spot::new(1.3, BRIGHT); }
            return s;
        }
    }
    let upper = if window { Fill::Glass } else { Fill::Fins };
    if let Some(s) = inset(bx, y, UPPER.0, UPPER.1, true, upper) { return s; }
    if window {
        if let Some(s) = inset(bx, y, MID.0, MID.1, false, Fill::Glass) { return s; }
    } else if let Some(s) = blast_door(bx, y) { return s; }
    if y >= SLAB.1 { return engine_room(k, bx, y); }
    // The plain steel face, an engraved line under the beam and above the
    // floor.
    let mut s = Spot::new(1.0, STEEL);
    if (y - (BEAM.1 + 60.0)).abs() < 2.5 { s.h = 0.7; }
    s
}

// ── The engine room, under the deck ─────────────────────────────────────────

const ER_WALL: [f32; 3] = [30.0, 36.0, 42.0];
const AMBER_HOT: [f32; 3] = [255.0, 150.0, 60.0];
const PLASMA: [f32; 3] = [140.0, 230.0, 255.0];

/// The engine room (`y` below the deck): a dark machinery wall, conduits,
/// a grated floor with amber lights; a reactor core in a glass column (even
/// bays) or twin turbines over a valve manifold (odd).
fn engine_room(k: i32, bx: f32, y: f32) -> Spot {
    let e = bx.min(TILE_W - bx);
    // The grated floor.
    if y >= GRATE {
        if y < GRATE + 10.0 { return Spot::new(1.3, BRIGHT); }
        for lx in [400.0, 1200.0] {
            if (bx - lx).abs() < 40.0 && y < GRATE + 24.0 { return Spot::new(0.9, [90.0, 60.0, 30.0]).glow(AMBER_HOT, 0.9); }
        }
        let g = bx.rem_euclid(24.0) < 5.0 || (y - GRATE).rem_euclid(20.0) < 4.0;
        return Spot::new(if g { 0.9 } else { 0.4 }, if g { [56.0, 62.0, 70.0] } else { [14.0, 16.0, 20.0] });
    }
    if e < PISTON { return spine(e, y); }
    // Conduits along the top of the room.
    for c in [SLAB.1 + 50.0, SLAB.1 + 86.0] {
        let u = (y - c) / 14.0;
        if u.abs() < 1.0 { return chrome(u); }
    }
    if k.rem_euclid(2) == 0 {
        // The reactor: caps top and bottom, a glass column, the plasma core
        // in rings of containment.
        let (c0, c1) = (1440.0, 2140.0);
        let (hx0, hx1) = (590.0, 1010.0);
        for (y0, y1, band) in [(c0 - 80.0, c0, true), (c1, c1 + 60.0, false)] {
            if y >= y0 && y < y1 && bx > hx0 && bx < hx1 {
                if band && y > y1 - 26.0 { return hazard(bx + y); }
                let mut s = Spot::new(1.25, STEEL);
                if bx < hx0 + 8.0 || bx > hx1 - 8.0 { s.h = 0.4; }
                return s;
            }
        }
        if y >= c0 && y < c1 {
            let (gx0, gx1) = (640.0, 960.0);
            if bx > gx0 && bx < gx1 {
                let u = (bx - 800.0) / 160.0;
                if ((y - c0).rem_euclid(110.0) - 55.0).abs() < 9.0 { return chrome(u * 0.95).glow(PLASMA, 0.08); }
                let core = (-(u / 0.22).powi(2)).exp();
                let flicker = 0.85 + 0.15 * (y / 37.0).sin() * (y / 13.0).cos();
                let mut s = Spot::new(0.6, [14.0, 30.0, 40.0]);
                s.own = true;
                let rim = (1.0 - (1.0 - u.abs()) / 0.1).clamp(0.0, 1.0);
                s.col = [20.0 + 40.0 * rim, 44.0 + 60.0 * rim, 56.0 + 70.0 * rim];
                return s.glow(PLASMA, core * 1.3 * flicker + 0.12).glow([255.0, 255.0, 255.0], core.powi(3) * 0.8);
            }
            // Feed lines from the walls into the column.
            for c in [1640.0, 1940.0] {
                let u = (y - c) / 22.0;
                if u.abs() < 1.0 && ((bx > 300.0 && bx < gx0) || (bx > gx1 && bx < 1300.0)) {
                    let mut s = chrome(u);
                    if (bx - 450.0).abs() < 14.0 || (bx - 1150.0).abs() < 14.0 { s = Spot::new(1.3, [96.0, 112.0, 120.0]); }
                    return s;
                }
            }
        }
    } else {
        // Twin turbines: a ring casing, curved blades, a lit hub, amber heat
        // behind the blades.
        for cx in [520.0, 1080.0] {
            let (dx, dy) = (bx - cx, y - 1720.0);
            let r = (dx * dx + dy * dy).sqrt();
            if r < 220.0 {
                if r > 190.0 { let u = (r - 205.0) / 15.0; return chrome(u); }
                if r < 34.0 { return Spot::new(1.3, BRIGHT).glow(AMBER_HOT, (1.0 - r / 34.0) * 0.5); }
                let a = dy.atan2(dx) + r / 160.0;
                let blade = (a * 9.0 / std::f32::consts::TAU).fract();
                let s = if blade < 0.32 { Spot::new(1.0, [92.0, 104.0, 112.0]) } else { Spot::new(0.35, [16.0, 18.0, 22.0]).glow(AMBER_HOT, 0.18 * (1.0 - r / 190.0)) };
                return s;
            }
        }
        // The manifold: a thick pipe across, red valve wheels on it.
        let u = (y - 2040.0) / 30.0;
        if u.abs() < 1.0 && bx > 300.0 && bx < 1300.0 {
            for vx in [800.0] {
                let (dx, dy) = (bx - vx, y - 2040.0);
                let r = (dx * dx + dy * dy).sqrt();
                if r < 46.0 && (r > 36.0 || dx.abs() < 5.0 || dy.abs() < 5.0) { return Spot::new(1.25, [170.0, 34.0, 36.0]); }
            }
            return chrome(u);
        }
        if let Some(c) = caution(bx - 800.0, y - 1480.0, 110.0) { return c; }
    }
    // The machinery wall: tall plates, dark.
    let mut s = Spot::new(0.9, ER_WALL);
    if bx.rem_euclid(200.0) < 5.0 { s.h = 0.4; }
    if (y - (GRATE - 40.0)).abs() < 3.0 { s.h = 0.5; }
    s
}

// ── Outside ─────────────────────────────────────────────────────────────────

/// A horizontal cylinder's shading across it (`v` -1 top .. 1 bottom): lit
/// from above and in front, a highlight along its upper shoulder.
fn round(v: f32) -> f32 {
    let v = v.clamp(-0.999, 0.999);
    let nz = (1.0 - v * v).sqrt();
    0.18 + 0.9 * (-v * 0.45 + nz * 0.8).max(0.0) + (-((v + 0.55) / 0.1).powi(2)).exp() * 0.45
}

/// A long capsule from `x0..x1`, `d0..d1` (`None` outside it): its skin
/// shaded round, seams along and across it, a collar ring standing proud at
/// every bay seam, a row of lit portholes.
fn capsule(lx: f32, d: f32, x0: f32, x1: f32, d0: f32, d1: f32, ports: bool) -> Option<Spot> {
    let r = (d1 - d0) * 0.5;
    let dc = (d0 + d1) * 0.5;
    let dist = rrect(lx, d, x0, d0, x1, d1, r);
    // The collars stand out past the skin.
    let seam = (lx + 800.0).rem_euclid(TILE_W) - 800.0; // distance to the nearest bay seam
    let collar = seam.abs() < 34.0 && lx > x0 + r && lx < x1 - r;
    if dist > if collar { 26.0 } else { 0.0 } { return None; }
    let v = (d - dc) / (r + if collar { 26.0 } else { 0.0 });
    let shade = round(v);
    if collar {
        let mut s = Spot::new(1.3, [96.0, 108.0, 116.0].map(|c| c * shade));
        if (seam.abs() - 22.0).abs() < 3.0 { s.h = 0.5; }
        if (v * 6.0).fract().abs() < 0.08 && seam.abs() < 14.0 { s.h = 1.45; }
        return Some(s);
    }
    let mut s = Spot::new(1.0, [72.0, 88.0, 98.0].map(|c| c * shade));
    if dist > -6.0 { s = Spot::new(1.25, [120.0, 136.0, 146.0].map(|c| c * shade)); }
    if (v.abs() - 0.42).abs() < 0.015 { s.h = 0.5; }
    if (lx - x0).rem_euclid(400.0) < 4.0 && lx > x0 + r && lx < x1 - r { s.h = 0.5; }
    if ports && v.abs() < 0.09 {
        let p = (lx - x0).rem_euclid(110.0) - 55.0;
        let pr = (p * p + ((d - dc) * 1.0).powi(2)).sqrt();
        if pr < 15.0 && lx > x0 + r && lx < x1 - r {
            let warm = ((lx / 110.0).floor() as i32).rem_euclid(3) != 0;
            let c = if warm { [255.0, 214.0, 150.0] } else { [170.0, 230.0, 255.0] };
            return Some(Spot::new(0.9, c.map(|v| v * 0.6)).glow(c, 0.7));
        }
        if pr < 21.0 && lx > x0 + r && lx < x1 - r { return Some(Spot::new(1.3, [140.0, 150.0, 160.0].map(|c| c * shade))); }
    }
    Some(s)
}

/// The station outside the bay, `d` out from the ceiling (above) or the
/// floor (below), for a bay `len` long: the roof's skin (or the keel's),
/// pylons out to the module above (or the engine nacelle below), and the
/// gear on them. Space between: it is not a box.
fn outside(lx: f32, d: f32, above: bool, len: f32) -> Spot {
    let k = (lx / TILE_W).floor() as i32;
    let bx = lx - k as f32 * TILE_W;
    let e = bx.min(TILE_W - bx);
    let skin = if above { ROOF } else { KEEL };
    if d < skin {
        if !above && d < 110.0 {
            // The engine room's floor edge, its front face: a hazard band.
            if d < 16.0 { return Spot::new(1.1, STEEL); }
            if d < 52.0 { return hazard(bx + d); }
            if d < 70.0 { return Spot::new(1.05, STEEL); }
            if (d - 84.0).abs() < 4.0 { return strip(1.0); }
            return Spot::new(0.35, RECESS);
        }
        if d < 40.0 { return Spot::new(1.3, BRIGHT); }
        if d < 70.0 { if (d - 55.0).abs() < 4.0 { return strip(1.0); } return Spot::new(0.35, RECESS); }
        if e < SPINE { return spine(e, if above { CEIL_Y - d } else { DECK_Y + d }); }
        if d > skin - 14.0 { return Spot::new(1.3, BRIGHT); }
        if let Some(l) = caged_lamp(bx - 800.0, (d - (skin + 70.0) * 0.5) * 1.6) { return l; }
        let mut s = Spot::new(1.0, STEEL);
        if (bx - 400.0).rem_euclid(800.0) < 4.0 { s.h = 0.4; }
        return s;
    }
    let (m0, m1, x0, x1) = if above { (MODULE.0, MODULE.1, MODULE_IN, len - MODULE_IN) } else { (NACELLE.0, NACELLE.1, NACELLE_IN.0, len - NACELLE_IN.1) };
    // Pylons from the skin out to the module (or nacelle), on the bay seams
    // and halfway, a tunnel in the middle of each.
    if d < m0 + 20.0 && lx > x0 && lx < x1 {
        if e < 60.0 { return chrome(e / 60.0); }
        let u = (bx - 800.0) / 90.0;
        if u.abs() < 1.0 {
            let mut s = chrome(u);
            s.col = s.col.map(|c| c * 0.8);
            if (d - skin).rem_euclid(60.0) < 8.0 { s.h = 1.35; }
            return s;
        }
    }
    if d >= m0 - 30.0 && d < m1 + 30.0 {
        if let Some(s) = capsule(lx, d, x0, x1, m0, m1, above) { return s; }
        if !above {
            // The engine at the nacelle's back end: a flared bell, its throat
            // glowing blue, the exhaust's glow past it.
            let (b0, b1) = (x1, x1 + 220.0);
            let dc = (m0 + m1) * 0.5;
            if lx >= b0 - 10.0 && lx < b1 {
                let t = ((lx - b0) / (b1 - b0)).clamp(0.0, 1.0);
                let half = (m1 - m0) * (0.34 + 0.16 * t * t);
                let off = (d - dc).abs();
                if off < half {
                    if off > half - 30.0 { return Spot::new(1.2, [110.0, 118.0, 126.0].map(|c| c * round((d - dc) / half))); }
                    let mut s = Spot::new(0.5, [30.0, 50.0, 80.0]);
                    s.own = true;
                    return s.glow([120.0, 190.0, 255.0], 0.4 + 0.9 * t).glow([255.0, 255.0, 255.0], t.powi(3) * 0.5 * (1.0 - off / half));
                }
            }
            if lx >= b1 && lx < b1 + 160.0 {
                let off = (d - dc).abs() / ((m1 - m0) * 0.5);
                let f = 1.0 - (lx - b1) / 160.0;
                if off < 1.0 { return Spot::empty().glow([110.0, 180.0, 255.0], f * f * (1.0 - off) * 0.9); }
            }
        }
    }
    if d >= m1 + 30.0 && lx > x0 + 200.0 && lx < x1 - 200.0 {
        return gear(k, bx, d - m1 - 30.0, above);
    }
    Spot::empty()
}

/// A lattice truss along `d0..d1` (chrome chords, diagonals between).
fn truss(bx: f32, d: f32, d0: f32, d1: f32) -> Option<Spot> {
    if d < d0 - 10.0 || d > d1 + 10.0 { return None; }
    for c in [d0, d1] { let u = (d - c) / 10.0; if u.abs() < 1.0 { return Some(chrome(u)); } }
    let cell = d1 - d0;
    let f = bx.rem_euclid(cell * 2.0);
    let diag = if f < cell { f - (d - d0) } else { (cell * 2.0 - f) - (d - d0) };
    if diag.abs() < 6.0 || bx.rem_euclid(cell) < 6.0 { return Some(Spot::new(1.0, [96.0, 108.0, 116.0])); }
    None
}

/// A solar wing between `x0..x1`, `d0..d1`: dark blue cells in a silver
/// frame, the grid between them catching the light.
fn solar(bx: f32, d: f32, x0: f32, x1: f32, d0: f32, d1: f32) -> Option<Spot> {
    if bx < x0 || bx > x1 || d < d0 || d > d1 { return None; }
    let edge = (bx - x0).min(x1 - bx).min(d - d0).min(d1 - d);
    if edge < 12.0 { return Some(Spot::new(1.2, [150.0, 160.0, 170.0])); }
    let (cx, cy) = ((bx - x0).rem_euclid(60.0), (d - d0).rem_euclid(48.0));
    if cx < 4.0 || cy < 4.0 { return Some(Spot::new(1.0, [80.0, 110.0, 140.0])); }
    let t = (d - d0) / (d1 - d0);
    let mut s = Spot::new(0.9, [24.0 + 20.0 * t, 44.0 + 26.0 * t, 96.0 + 30.0 * t]);
    // The sun's glint across the wing.
    let g = (-(((bx - x0) * 0.5 + (d - d0) - 260.0) / 50.0).powi(2)).exp();
    s = s.glow([120.0, 160.0, 220.0], g * 0.25);
    Some(s)
}

/// The station's gear above the upper hull (`d` up from it) or below the
/// lower one: a truss along it, and per bay, a solar mast with two wings or
/// a radiator and an antenna with a dish (above); a pod or radiator fins
/// (below). Empty space between.
fn gear(k: i32, bx: f32, d: f32, above: bool) -> Spot {
    if d >= DRESS { return Spot::empty(); }
    let e = bx.min(TILE_W - bx);
    // Struts from the hull up to the truss, on the bay seams.
    if e < 24.0 && d < 70.0 { return chrome(e / 24.0); }
    if let Some(t) = truss(bx, d, 70.0, 150.0) { return t; }
    let even = k.rem_euclid(2) == 0;
    if above && even {
        let u = (bx - 800.0) / 22.0;
        if u.abs() < 1.0 && d < 820.0 { return chrome(u); }
        if (d - 820.0).abs() < 14.0 && (bx - 800.0).abs() < 14.0 { return Spot::new(1.0, [70.0, 10.0, 12.0]).glow(RED, 1.0); }
        // The wings' booms, then the wings.
        if (d - 530.0).abs() < 8.0 && (bx - 800.0).abs() < 700.0 { return chrome((d - 530.0) / 8.0); }
        if let Some(p) = solar(bx, d, 200.0, 760.0, 300.0, 760.0).or_else(|| solar(bx, d, 840.0, 1400.0, 300.0, 760.0)) { return p; }
    } else if above {
        // A radiator: ribbed white panels on a frame.
        if bx > 260.0 && bx < 980.0 && d > 190.0 && d < 520.0 {
            let edge = (bx - 260.0).min(980.0 - bx).min(d - 190.0).min(520.0 - d);
            if edge < 10.0 { return Spot::new(1.2, [150.0, 160.0, 170.0]); }
            let rib = (bx - 260.0).rem_euclid(36.0);
            return Spot::new(if rib < 8.0 { 1.15 } else { 0.85 }, [170.0, 176.0, 180.0]);
        }
        if (bx - 620.0).abs() < 14.0 && d >= 150.0 && d <= 190.0 { return Spot::new(1.0, [96.0, 108.0, 116.0]); }
        // An antenna mast with a dish, a beacon on top.
        let u = (bx - 1200.0) / 12.0;
        if u.abs() < 1.0 && d < 760.0 { return chrome(u); }
        if (d - 760.0).abs() < 12.0 && (bx - 1200.0).abs() < 12.0 { return Spot::new(1.0, [70.0, 10.0, 12.0]).glow(RED, 1.0); }
        let (dx, dd) = ((bx - 1200.0) / 150.0, (d - 560.0) / 70.0);
        let r = dx * dx + dd * dd;
        if r < 1.0 && dd < 0.35 - dx * dx * 0.6 {
            return Spot::new(0.8 + 0.6 * (1.0 - r), [150.0, 158.0, 166.0]);
        }
    } else if even {
        // A pod slung under the keel: a capsule, lit windows along it.
        let (pc, pw, ph) = (800.0, 420.0, 110.0);
        let (dx, dd) = (bx - pc, d - 330.0);
        let rr = rrect(dx, dd, -pw, -ph, pw, ph, ph);
        if rr < 0.0 {
            let v = (dd / ph).clamp(-1.0, 1.0);
            let mut s = Spot::new(0.7 + 0.6 * (1.0 - v * v).sqrt(), STEEL);
            if (dd + 10.0).abs() < 8.0 && dx.rem_euclid(50.0) < 22.0 && dx.abs() < pw - 80.0 { s = Spot::new(0.9, LAMP).glow(STRIP, 0.6); }
            return s;
        }
        if (bx - 520.0).abs() < 14.0 && d > 150.0 && d < 230.0 { return Spot::new(1.0, [96.0, 108.0, 116.0]); }
        if (bx - 1080.0).abs() < 14.0 && d > 150.0 && d < 230.0 { return Spot::new(1.0, [96.0, 108.0, 116.0]); }
    } else if bx > 300.0 && bx < 1300.0 && d > 150.0 {
        // Radiator fins hanging from the keel, longer towards the middle.
        let len = 260.0 + 160.0 * (1.0 - ((bx - 800.0) / 500.0).powi(2));
        let f = (bx - 300.0).rem_euclid(50.0);
        if f < 16.0 && d < 150.0 + len { return Spot::new(1.1, [160.0, 168.0, 174.0]); }
    }
    Spot::empty()
}

/// Signed distance to a rounded rectangle (negative inside).
fn rrect(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> f32 {
    let (cx, cy, hx, hy) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5, (x1 - x0) * 0.5, (y1 - y0) * 0.5);
    let r = r.min(hx).min(hy);
    let (qx, qy) = ((x - cx).abs() - (hx - r), (y - cy).abs() - (hy - r));
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

/// The station (no mouths) at hangar-local x, world y, for a bay `len`
/// long.
fn station(lx: f32, y: f32, len: f32) -> Spot {
    if y < CEIL_Y { return outside(lx, CEIL_Y - y, true, len); }
    if y >= DECK_Y { return outside(lx, y - DECK_Y, false, len); }
    let k = (lx / TILE_W).floor() as i32;
    bay(k, lx - k as f32 * TILE_W, y)
}
/// The ends are the same for any length; this one stands in.
const REF_LEN: f32 = 4.0 * TILE_W;

// ── The ends: the blast door, standing open ─────────────────────────────────

/// One half of the open door, `u` across from its middle, `d` in from its
/// edge: the hazard band along the edge, the red lock light, the spine.
fn door_half(u: f32, d: f32) -> Spot {
    if d < 5.0 { return Spot::new(0.3, [10.0, 12.0, 14.0]); }
    if u.abs() < 48.0 && d < 32.0 { return lock_light(d); }
    if d < 105.0 {
        if d > 96.0 { return Spot::new(0.3, [24.0, 28.0, 32.0]); }
        return hazard(u + d);
    }
    if u.abs() < 90.0 {
        let mut s = Spot::new(1.2, STEEL);
        if (d - 105.0).rem_euclid(250.0) < 9.0 { s.h = 0.3; }
        if u.abs() > 84.0 { s.h = 0.35; }
        return s;
    }
    Spot::new(1.0, STEEL)
}

/// The entrance at `mx` (hangar-local x; the exit is drawn as its mirror),
/// world y. Outside, the housing (`-MOUTH_OUT..0`) above and below the
/// opening; the door's halves, drawn back into it, their edges showing
/// below and above its lips; the pillar (`0..PILLAR_W`); the bay beyond.
fn mouth(mx: f32, y: f32) -> Spot {
    if mx >= PILLAR_W { return station(mx, y, REF_LEN); }
    if mx >= 0.0 && (y < CEIL_Y || y >= DECK_Y) {
        // Over the pillar the housing steps down, rounded, to the roof.
        let d = if y < CEIL_Y { CEIL_Y - y } else { y - DECK_Y };
        let t = (mx / PILLAR_W).clamp(0.0, 1.0);
        let top = HOUSING - (HOUSING - ROOF) * (t * t * (3.0 - 2.0 * t));
        if d < top && d >= ROOF.max(if y < CEIL_Y { ROOF } else { KEEL }) - 1.0 {
            let mut s = Spot::new(1.0, STEEL);
            if d > top - 14.0 { s = Spot::new(1.3, BRIGHT); }
            return s;
        }
        return station(mx, y, REF_LEN);
    }
    if mx >= 0.0 {
        // The pillar: a heavy spine, chrome rails at its edges, caged lamps,
        // a caution sign halfway.
        let u0 = (mx - 22.0) / 16.0;
        let u1 = (mx - (PILLAR_W - 22.0)) / 16.0;
        if u0.abs() < 1.0 { return chrome(u0); }
        if u1.abs() < 1.0 { return chrome(u1); }
        if mx > PILLAR_W - 6.0 || mx < 6.0 { return Spot::new(0.3, DARK); }
        let c = PILLAR_W * 0.5;
        if let Some(l) = caged_lamp(mx - c, y - (CEIL_Y + 420.0)).or_else(|| caged_lamp(mx - c, y - (DECK_Y - 420.0))) { return l; }
        if let Some(s) = caution(mx - c, y - (CEIL_Y + DECK_Y) * 0.5, 120.0) { return s; }
        let mut s = Spot::new(1.25, STEEL);
        if (y - CEIL_Y).rem_euclid(250.0) < 9.0 { s.h = 0.3; }
        if ((mx - c).abs() - 70.0).abs() < 2.5 { s.h = 0.8; }
        return s;
    }
    if mx < -MOUTH_OUT { return Spot::empty(); }
    // The door: its halves, the edges slanted (the transition's seam).
    let (dx0, dx1) = (-MOUTH_OUT + 60.0, -60.0);
    let dc = (dx0 + dx1) * 0.5;
    let slant = (mx - dc) * -0.2;
    let edge_top = CEIL_Y + 150.0 + slant;
    let edge_bot = DECK_Y - 150.0 + slant;
    if y > CEIL_Y && y < DECK_Y {
        if mx > dx0 && mx < dx1 && (y < edge_top || y > edge_bot) {
            let d = if y < edge_top { edge_top - y } else { y - edge_bot };
            return door_half(mx - dc, d);
        }
        return Spot::empty();
    }
    // The housing: lips at the opening, then armour; its outer corner cut.
    let d = if y <= CEIL_Y { CEIL_Y - y } else { y - DECK_Y };
    if d >= HOUSING { return Spot::empty(); }
    // The housing's outer corner, rounded.
    let rc = 300.0;
    let x_out = if d > HOUSING - rc { -MOUTH_OUT + rc - (rc * rc - (d - (HOUSING - rc)).powi(2)).max(0.0).sqrt() } else { -MOUTH_OUT };
    if d > HOUSING - 14.0 && mx > x_out { return Spot::new(1.3, BRIGHT); }
    if mx < x_out { return Spot::empty(); }
    if mx < x_out + 14.0 { return Spot::new(1.3, BRIGHT).glow(STRIP, if mx < x_out + 5.0 { 0.7 } else { 0.0 }); }
    if d < 34.0 {
        if (d - 16.0).abs() < 5.0 { return strip(1.1); }
        return Spot::new(1.35, BRIGHT);
    }
    if d < 78.0 { if d > 72.0 { return Spot::new(0.3, DARK); } return hazard(mx + d); }
    // The door's slot: guide rails either side of where it is stowed.
    for gx in [dx0 - 10.0, dx1 + 10.0] {
        let u = (mx - gx) / 12.0;
        if u.abs() < 1.0 && d < HOUSING - 30.0 && mx > x_out + 30.0 { return chrome(u); }
    }
    if let Some(l) = caged_lamp(mx - (-MOUTH_OUT + 160.0), d - 160.0) { return l; }
    let (a0, a1) = (250.0, 420.0);
    if d > a0 && d < a1 {
        let w0 = (-MOUTH_OUT + 300.0 + (d - a0) * 0.55).max(x_out + 60.0);
        if mx > w0 && mx < -100.0 {
            let edge = (mx - w0).min(-100.0 - mx).min(d - a0).min(a1 - d);
            let s = Spot::new(1.15, ACCENT);
            return if edge < 10.0 { s.glow(STRIP, (1.0 - edge / 10.0) * 0.8) } else { s };
        }
    }
    let mut s = Spot::new(1.0, STEEL);
    if (d - 78.0).rem_euclid(330.0) < 6.0 { s.h = 0.3; }
    s
}

// ── Light ───────────────────────────────────────────────────────────────────

/// The light falling on a point (hangar-local x, world y): inside the bay,
/// the ceiling's light bars (primary: pools spreading down the wall) and the
/// platform's downlights (secondary, under it), over a dim ambient; outside,
/// a cool spotlight from above, as on the transition's door.
fn light(lx: f32, y: f32) -> f32 {
    if y < CEIL_Y || y >= DECK_Y || lx < 0.0 {
        // Outside, lit from above: brighter over the station than under it
        // (the rounded parts shade themselves, see `round`).
        return if y < DECK_Y { 0.95 } else { 0.7 };
    }
    if y >= SLAB.1 {
        // The engine room: dark, lit by its machines.
        let bx = lx.rem_euclid(TILE_W);
        let k = (lx / TILE_W).floor() as i32;
        let mut l = 0.32;
        if k.rem_euclid(2) == 0 { l += (-((bx - 800.0) / 380.0).powi(2)).exp() * 0.75; }
        else { for cx in [520.0, 1080.0] { l += (-(((bx - cx).powi(2) + (y - 1720.0).powi(2)).sqrt() / 330.0).powi(2)).exp() * 0.45; } }
        for fx in [400.0, 1200.0] { l += (-(((bx - fx).powi(2) + (y - GRATE).powi(2) * 2.0).sqrt() / 260.0).powi(2)).exp() * 0.5; }
        return l.min(1.05);
    }
    let mut l: f32 = 0.3;
    let k = (lx / TILE_W).floor();
    let dy = y - BEAM.1;
    if dy > 0.0 {
        for kk in [k - 1.0, k, k + 1.0] {
            for bx in [520.0, 1080.0] {
                let dx = lx - (kk * TILE_W + bx);
                let spread = 150.0 + 0.42 * dy;
                l += (-(dx / spread).powi(2)).exp() * 0.62 / (1.0 + dy / 900.0);
            }
        }
    }
    let dy2 = y - SLAB.1;
    if dy2 > 0.0 {
        for kk in [k - 1.0, k, k + 1.0] {
            for bx in [400.0, 800.0, 1200.0] {
                let dx = lx - (kk * TILE_W + bx);
                let spread = 60.0 + 0.5 * dy2;
                l += (-(dx / spread).powi(2)).exp() * (-dy2 / 600.0).exp() * 0.45;
            }
        }
    }
    // A little light up from the floor's edge.
    l += (-(DECK_Y - y) / 300.0).exp() * 0.15;
    l.min(1.05)
}

/// Light shafts under the ceiling's bars: the air lit (added, not shaded).
fn shaft(lx: f32, y: f32) -> f32 {
    if lx < 0.0 || y < BEAM.1 || y >= WALK_BACK { return 0.0; }
    let k = (lx / TILE_W).floor();
    let dy = y - BEAM.1;
    let mut a = 0.0;
    for kk in [k - 1.0, k, k + 1.0] {
        for bx in [520.0, 1080.0] {
            let dx = lx - (kk * TILE_W + bx);
            a += (-(dx / (110.0 + 0.36 * dy)).powi(2)).exp() * (-dy / 1300.0).exp();
        }
    }
    a * 20.0
}

// ── Rendering ───────────────────────────────────────────────────────────────

type Img = image::RgbaImage;

/// Box-blur a channel buffer in place, `r` px, horizontally then vertically.
fn box_blur(buf: &mut [f32], w: usize, h: usize, r: usize) {
    let n = (2 * r + 1) as f32;
    let mut tmp = vec![0.0f32; w.max(h)];
    for y in 0..h {
        let row = &mut buf[y * w..(y + 1) * w];
        let mut acc = row[0] * (r as f32 + 1.0);
        for i in 1..=r { acc += row[i.min(w - 1)]; }
        for x in 0..w {
            tmp[x] = acc / n;
            acc += row[(x + r + 1).min(w - 1)] - row[x.saturating_sub(r)];
        }
        row.copy_from_slice(&tmp[..w]);
    }
    for x in 0..w {
        let mut acc = buf[x] * (r as f32 + 1.0);
        for i in 1..=r { acc += buf[i.min(h - 1) * w + x]; }
        for y in 0..h {
            tmp[y] = acc / n;
            acc += buf[(y + r + 1).min(h - 1) * w + x] - buf[y.saturating_sub(r) * w + x];
        }
        for y in 0..h { buf[y * w + x] = tmp[y]; }
    }
}

/// Render the station over hangar-local `x0..x0+w`, world `y0..y0+h`, at
/// `ART`, from `spot` (what is where): relief shading lit by `light` (a
/// light from the upper left catching every raised edge), polished metal (a
/// broad sheen, brushed streaks), the light shafts, and the lights' bloom
/// (their light blurred twice, small and wide, added over everything and
/// spilling into empty space as light of its own).
fn render(x0: f32, y0: f32, w: f32, h: f32, spot: &dyn Fn(f32, f32) -> Spot) -> Img {
    let s = ART;
    let m = (260.0 * s).ceil() as i32; // bloom margin, px
    let (ow, oh) = ((w * s).round() as i32, (h * s).round() as i32);
    let (bw, bh) = ((ow + 2 * m) as usize, (oh + 2 * m) as usize);
    let at = |px: i32, py: i32| (x0 + ((px - m) as f32 + 0.5) / s, y0 + ((py - m) as f32 + 0.5) / s);
    let mut spots = Vec::with_capacity(bw * bh);
    for py in 0..bh as i32 { for px in 0..bw as i32 { let (wx, wy) = at(px, py); spots.push(spot(wx, wy)); } }
    let emit: [Vec<f32>; 3] = std::array::from_fn(|c| spots.iter().map(|sp| sp.emit[c]).collect());
    let mut small = emit.clone();
    let mut wide = emit;
    let (r1, r2) = (((26.0 * s) as usize).max(2), ((100.0 * s) as usize).max(4));
    for c in 0..3 {
        box_blur(&mut small[c], bw, bh, r1); box_blur(&mut small[c], bw, bh, r1);
        box_blur(&mut wide[c], bw, bh, r2); box_blur(&mut wide[c], bw, bh, r2);
    }
    let mut img = Img::new(ow as u32, oh as u32);
    for oy in 0..oh { for ox in 0..ow {
        let (px, py) = (ox + m, oy + m);
        let i = py as usize * bw + px as usize;
        let sp = spots[i];
        let (wx, wy) = at(px, py);
        let bloom = [0, 1, 2].map(|c| small[c][i] * 0.9 + wide[c][i] * 0.6);
        if !sp.solid {
            // Empty, or glass: what lies behind shows through. Glass covers
            // it a little (its tint), light adds over it. Premultiplied.
            let rgb = [0, 1, 2].map(|c| sp.col[c] * sp.glass + sp.emit[c] + bloom[c]);
            let k = sp.glass.max(rgb[0].max(rgb[1]).max(rgb[2]) / 255.0).min(1.0);
            if k > 0.004 {
                img.put_pixel(ox as u32, oy as u32, image::Rgba([rgb[0].min(255.0) as u8, rgb[1].min(255.0) as u8, rgb[2].min(255.0) as u8, (k * 255.0) as u8]));
            }
            continue;
        }
        let mut c = if sp.own {
            sp.col
        } else {
            let nb = spots[i - bw - 1];
            let nh = if nb.solid { nb.h } else { sp.h };
            let slope = (sp.h - nh).clamp(-1.0, 1.0);
            let l = light(wx, wy);
            let sheen = 0.22 * (0.5 + 0.5 * (std::f32::consts::TAU * (wx / TILE_W - wy / 2600.0)).sin()).powi(8);
            let brush = (hash2((wx / 2.0).floor() as i32, (wy / 70.0).floor() as i32, 21) - 0.5) * 0.14
                + (hash2((wx / 9.0).floor() as i32, (wy / 300.0).floor() as i32, 22) - 0.5) * 0.08;
            let mut out = [0.0f32; 3];
            for k in 0..3 {
                let base = sp.col[k] * l * (0.55 + 0.45 * sp.h.min(1.35)) * (1.0 + sheen + brush);
                let rim = [190.0, 225.0, 240.0][k] * (0.35 + 0.65 * l.min(1.0));
                out[k] = if slope > 0.0 { base + rim * slope * 1.1 } else { base * (1.0 + slope * 1.1).max(0.15) };
            }
            out
        };
        let sh = shaft(wx, wy);
        for k in 0..3 { c[k] += sp.emit[k] + bloom[k] + sh * [0.62, 0.82, 1.0][k]; }
        img.put_pixel(ox as u32, oy as u32, image::Rgba([c[0].clamp(0.0, 255.0) as u8, c[1].clamp(0.0, 255.0) as u8, c[2].clamp(0.0, 255.0) as u8, 255]));
    }}
    img
}

/// Every piece, built once: per bay parity (viewport, bulkhead) the hull
/// above, the wall and the hull below; the entrance in three (housing above,
/// the opening, housing below), the exit its mirror.
struct Art { walls: [Arc<Img>; 2], end_in: [Arc<Img>; 3], end_out: [Arc<Img>; 3] }

/// The bands of height every piece is cut into: the gear above, the hull
/// above, the bay, the hull below, the gear below. The ends have no gear.
const BANDS: [(f32, f32); 5] = [
    (CEIL_Y - HULL - DRESS, CEIL_Y - HULL), (CEIL_Y - HULL, CEIL_Y), (CEIL_Y, DECK_Y),
    (DECK_Y, DECK_Y + HULL), (DECK_Y + HULL, DECK_Y + HULL + DRESS),
];
const END_BANDS: [(f32, f32); 3] = [BANDS[1], BANDS[2], BANDS[3]];

fn art() -> &'static Art {
    static A: OnceLock<Art> = OnceLock::new();
    A.get_or_init(|| {
        let (a, b) = BANDS[2];
        let wall = |k: usize| Arc::new(render(k as f32 * TILE_W, a, TILE_W + OVERLAP, b - a, &|x, y| station(x, y, REF_LEN)));
        let end_in = END_BANDS.map(|(a, b)| Arc::new(render(-MOUTH_OUT, a, MOUTH_OUT + PILLAR_W, b - a, &mouth)));
        let end_out = end_in.clone().map(|i| { let mut f = (*i).clone(); image::imageops::flip_horizontal_in_place(&mut f); Arc::new(f) });
        Art { walls: [wall(0), wall(1)], end_in, end_out }
    })
}

/// The outside for a bay `tiles` long, per tile: the bands above and below
/// (gear, roof and module; keel and nacelle, gear). Built once per length.
fn outer(tiles: usize) -> Arc<Vec<[Arc<Img>; 4]>> {
    static O: OnceLock<Mutex<std::collections::HashMap<usize, Arc<Vec<[Arc<Img>; 4]>>>>> = OnceLock::new();
    let m = O.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Some(v) = m.lock().unwrap().get(&tiles) { return v.clone(); }
    let len = tiles as f32 * TILE_W;
    let v: Vec<[Arc<Img>; 4]> = (0..tiles).map(|i| {
        [0usize, 1, 3, 4].map(|b| { let (a, bb) = BANDS[b]; Arc::new(render(i as f32 * TILE_W, a, TILE_W + OVERLAP, bb - a, &|x, y| station(x, y, len))) })
    }).collect();
    let v = Arc::new(v);
    m.lock().unwrap().insert(tiles, v.clone());
    v
}

// ── Objects ──────────────────────────────────────────────────────────────────

const PARTS: [&str; 5] = ["gear_up", "top", "wall", "bot", "gear_dn"];
const END_PARTS: [&str; 3] = ["top", "wall", "bot"];

fn ids() -> Vec<String> {
    // The bays first, each over the overlap of the one before; the ends over
    // everything.
    let mut v = Vec::new();
    for i in 0..MAX_TILES { for p in PARTS { v.push(format!("hangar_{p}_{i}")); } }
    for end in ["in", "out"] { for p in END_PARTS { v.push(format!("hangar_{end}_{p}")); } }
    v
}

/// The pooled pieces, hidden, for the scene builder: straight after the far
/// decor, so behind every gameplay object.
pub fn objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    ids().into_iter().map(|id| {
        let mut o = GameObject::new_rect(ctx, id.clone().into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (64.0, 64.0), 0.0), image: Arc::new(Img::new(1, 1)), color: None }),
            (64.0, 64.0), (-90_000.0, -90_000.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        o.visible = false;
        o.layer = LAYER;
        o.gravity = 0.0;
        // Lit by its own baked lights, like the transition's door: the
        // scene's lighting brightened it to a washed-out pale grey.
        o.unlit = true;
        (id, o)
    }).collect()
}

fn place(c: &mut Canvas, id: &str, img: Arc<Img>, x: f32, y: f32, w: f32, h: f32) {
    if let Some(o) = c.get_game_object_mut(id) {
        o.size = (w, h);
        o.position = (x, y);
        o.visible = true;
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img, color: None });
    }
}

fn hide(c: &mut Canvas) {
    for id in ids() {
        if let Some(o) = c.get_game_object_mut(&id) { o.visible = false; }
    }
}

/// Open a hangar starting `x`, `tiles` repeats long.
fn open(c: &mut Canvas, x: f32, tiles: usize) {
    let tiles = tiles.clamp(2, MAX_TILES);
    let len = tiles as f32 * TILE_W;
    state().lock().unwrap().span = Some((x, len));
    let a = art();
    let o = outer(tiles);
    for i in 0..MAX_TILES {
        for (b, p) in PARTS.iter().enumerate() {
            let id = format!("hangar_{p}_{i}");
            if i < tiles {
                let (y0, y1) = BANDS[b];
                let img = if b == 2 { a.walls[i % 2].clone() } else { o[i][[0, 1, 0, 2, 3][b]].clone() };
                place(c, &id, img, x + i as f32 * TILE_W, y0, TILE_W + OVERLAP, y1 - y0);
            } else if let Some(o) = c.get_game_object_mut(&id) {
                o.visible = false;
            }
        }
    }
    for (b, p) in END_PARTS.iter().enumerate() {
        let (y0, y1) = END_BANDS[b];
        place(c, &format!("hangar_in_{p}"), a.end_in[b].clone(), x - MOUTH_OUT, y0, MOUTH_OUT + PILLAR_W, y1 - y0);
        place(c, &format!("hangar_out_{p}"), a.end_out[b].clone(), x + len - PILLAR_W, y0, MOUTH_OUT + PILLAR_W, y1 - y0);
    }
    log::info!("hangar: {} px from x={x:.0}", len);
}

/// Every game frame.
pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (px, dist, blocked) = {
        let s = st.lock().unwrap();
        (s.px, s.distance, s.in_space_mode || s.space_launch_active || s.boss_active || s.dead)
    };
    let key = c.key("h");
    let (pressed, due, span) = {
        let mut h = state().lock().unwrap();
        let pressed = key && !h.h_was_down;
        h.h_was_down = key;
        (pressed, dist >= h.next_at && h.span.is_none(), h.span)
    };
    if blocked {
        if span.is_some() {
            state().lock().unwrap().span = None;
            hide(c);
        }
        return;
    }
    if let Some((x, len)) = span {
        // Over once the far end is well behind.
        if px > x + len + VW * 2.0 {
            let mut h = state().lock().unwrap();
            h.span = None;
            let r = { h.rng ^= h.rng << 13; h.rng ^= h.rng >> 17; h.rng ^= h.rng << 5; h.rng };
            let k = (r % 10_000) as f32 / 10_000.0;
            h.next_at = dist + HANGAR_GAP.0 + (HANGAR_GAP.1 - HANGAR_GAP.0) * k;
            drop(h);
            hide(c);
        }
        return;
    }
    if pressed || due {
        let r = { let mut h = state().lock().unwrap(); h.rng ^= h.rng << 13; h.rng ^= h.rng >> 17; h.rng ^= h.rng << 5; h.rng };
        let tiles = 3 + (r % 3) as usize; // 4,800 - 8,000 px
        open(c, px + HANGAR_AHEAD, tiles);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every texture fits the renderer's limit (larger ones are shrunk and
    /// draw short of their object). `HANGAR_ART=<dir>`: also write them out.
    #[test]
    fn the_art_fits_and_can_be_looked_at() {
        let a = art();
        let mut all: Vec<(String, Arc<Img>)> = Vec::new();
        for (k, w) in a.walls.iter().enumerate() { all.push((format!("bay{k}_wall"), w.clone())); }
        for (i, t) in outer(4).iter().enumerate() { for (j, p) in ["gear_up", "top", "bot", "gear_dn"].iter().enumerate() { all.push((format!("t{i}_{p}"), t[j].clone())); } }
        for (b, p) in END_PARTS.iter().enumerate() { all.push((format!("in_{p}"), a.end_in[b].clone())); }
        for (name, img) in all {
            assert!(img.width() <= 1024 && img.height() <= 1024, "{name} is {}x{}", img.width(), img.height());
            if let Ok(dir) = std::env::var("HANGAR_ART") { img.save(format!("{dir}/{name}.png")).unwrap(); }
        }
    }
}
