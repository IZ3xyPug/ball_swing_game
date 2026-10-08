//! The station hangar: now and then the run passes through a big space
//! station's hangar bay, open at both ends.
//!
//! It is scenery around the normal level: the grab nodes, coins and hazards
//! carry on through it. Built from repeating pieces, so it can be any length:
//!   * the back wall (bays of plated steel, I-beam ribs with teal strip
//!     lights, catwalks, a viewport onto the stars, pools of lamplight);
//!   * the ceiling (exterior hull plating above, a truss with hanging lamps
//!     below);
//!   * the deck (hazard-striped landing deck at the fall line, hull below);
//!   * a door jamb at each open end (a heavy frame with chevrons and beacons).
//! All of it is drawn here, in code, once, and stretched with hard edges.
//!
//! It comes along every so often (`HANGAR_GAP`), never in space or a fight;
//! H starts one just ahead (testing).

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;

/// One repeat of the wall, ceiling and deck, world px.
const TILE_W: f32 = 1600.0;
/// World heights: the ceiling's underside, the deck's top (the fall line),
/// and how far the hull reaches beyond each.
const CEIL_Y: f32 = -1400.0;
const DECK_Y: f32 = VH + 150.0;
const HULL: f32 = 1200.0;
/// The door jamb's width.
const JAMB_W: f32 = 420.0;
/// Pieces in the pool (the longest hangar is this many tiles).
const MAX_TILES: usize = 6;
/// Distance before the first hangar, and between hangars (px).
const HANGAR_FIRST: f32 = 18_000.0;
const HANGAR_GAP: (f32, f32) = (60_000.0, 95_000.0);
/// How far ahead of the player a hangar opens.
const HANGAR_AHEAD: f32 = VW * 1.4;
/// Art is drawn at this share of world size, then stretched. Each texture
/// is kept under ~1,024 px a side: the renderer shrinks taller ones, and a
/// shrunk texture drew short of its object, leaving gaps between the bays.
const ART: f32 = 1.0 / 3.0;
const ART_WALL: f32 = 1000.0 / (DECK_Y - CEIL_Y);
const ART_JAMB: f32 = 1000.0 / (DECK_Y + HULL - (CEIL_Y - HULL));

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
    WARM.get_or_init(|| { std::thread::spawn(|| { let _ = (wall_img(), ceil_img(), deck_img(), jamb_img()); }); });
}

// ── Art ──────────────────────────────────────────────────────────────────────

type Img = image::RgbaImage;

fn px(img: &mut Img, x: i32, y: i32, c: [u8; 4]) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        img.put_pixel(x as u32, y as u32, image::Rgba(c));
    }
}

fn rect(img: &mut Img, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
    for yy in y..y + h { for xx in x..x + w { px(img, xx, yy, c); } }
}

/// Add light (premultiplied by `k`) over a pixel.
fn glow(img: &mut Img, x: i32, y: i32, c: [f32; 3], k: f32) {
    if x < 0 || y < 0 || x as u32 >= img.width() || y as u32 >= img.height() || !(k > 0.0) { return; }
    let p = img.get_pixel_mut(x as u32, y as u32);
    for i in 0..3 { p[i] = (p[i] as f32 + c[i] * k).min(255.0) as u8; }
}

/// A plated panel with a bevel: light top/left edge, dark bottom/right.
fn panel(img: &mut Img, x: i32, y: i32, w: i32, h: i32, base: [u8; 3]) {
    let b = |d: i32| [(base[0] as i32 + d).clamp(0, 255) as u8, (base[1] as i32 + d).clamp(0, 255) as u8, (base[2] as i32 + d).clamp(0, 255) as u8, 255];
    rect(img, x, y, w, h, b(0));
    rect(img, x, y, w, 2, b(26));
    rect(img, x, y, 2, h, b(16));
    rect(img, x, y + h - 2, w, 2, b(-22));
    rect(img, x + w - 2, y, 2, h, b(-16));
    // Rivets in the corners.
    for (rx, ry) in [(x + 5, y + 5), (x + w - 7, y + 5), (x + 5, y + h - 7), (x + w - 7, y + h - 7)] {
        rect(img, rx, ry, 2, 2, b(34));
    }
}

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15; x = x.wrapping_mul(0x2C1B_3C6D); x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65535.0
}

/// The back wall: one bay between two ribs.
fn wall_img() -> Arc<Img> {
    static I: OnceLock<Arc<Img>> = OnceLock::new();
    I.get_or_init(|| {
        let (w, h) = ((TILE_W * ART_WALL) as i32, ((DECK_Y - CEIL_Y) * ART_WALL) as i32);
        let mut img = Img::new(w as u32, h as u32);
        // Base: deep blue steel, darker high up.
        for y in 0..h {
            let t = y as f32 / h as f32;
            let c = [(16.0 + 14.0 * t) as u8, (22.0 + 16.0 * t) as u8, (34.0 + 20.0 * t) as u8, 255];
            rect(&mut img, 0, y, w, 1, c);
        }
        // Plates: rows of panels, staggered.
        let rib = 26;
        let (pw, ph) = ((w - rib) / 3, h / 9);
        for row in 0..9 {
            let off = if row % 2 == 0 { 0 } else { pw / 2 };
            for col in -1..4 {
                let x = rib + col * pw + off;
                let shade = (hash((row * 7 + col + 3) as u32) * 10.0) as u8;
                panel(&mut img, x + 2, row * ph + 2, pw - 4, ph - 4, [30 + shade, 38 + shade, 54 + shade]);
            }
        }
        // The viewport: a long window onto the stars, upper middle.
        let (vx, vy, vw, vh) = (rib + 30, h * 2 / 9 + 10, w - rib - 60, h / 9 - 20);
        rect(&mut img, vx - 6, vy - 6, vw + 12, vh + 12, [58, 70, 92, 255]);
        rect(&mut img, vx, vy, vw, vh, [3, 6, 16, 255]);
        for i in 0..90 {
            let sx = vx + (hash(i * 3 + 1) * vw as f32) as i32;
            let sy = vy + (hash(i * 3 + 2) * vh as f32) as i32;
            let b = (120.0 + hash(i * 3 + 3) * 135.0) as u8;
            px(&mut img, sx, sy, [b, b, (b as u16 + 20).min(255) as u8, 255]);
        }
        // Window mullions.
        for k in 1..4 { rect(&mut img, vx + vw * k / 4 - 2, vy, 4, vh, [58, 70, 92, 255]); }
        // Catwalks: two decks with rails and small amber lights.
        for &cy in &[h * 5 / 9, h * 7 / 9] {
            rect(&mut img, 0, cy, w, 8, [70, 78, 92, 255]);
            rect(&mut img, 0, cy + 8, w, 3, [20, 24, 32, 255]);
            rect(&mut img, 0, cy - 18, w, 2, [92, 100, 116, 255]);
            for k in 0..(w / 22) { rect(&mut img, k * 22, cy - 18, 2, 18, [80, 88, 104, 255]); }
            for k in 0..(w / 60) {
                let lx = 30 + k * 60;
                rect(&mut img, lx, cy + 2, 4, 3, [255, 190, 90, 255]);
                for dy in -6..8 { for dx in -8..10 {
                    let d = ((dx * dx + dy * dy) as f32).sqrt();
                    glow(&mut img, lx + dx, cy + 2 + dy, [255.0, 170.0, 70.0], (1.0 - d / 10.0).max(0.0) * 0.35);
                }}
            }
        }
        // Lamplight: pools of cool light falling from the ceiling lamps.
        for &lx in &[w / 3, w * 5 / 6] {
            for y in 0..h * 2 / 3 {
                // (Clamped: past 1 the power below is NaN, which drew white.)
                let fy = (y as f32 / (h as f32 * 0.66)).min(1.0);
                let half = 10.0 + fy * 110.0;
                for x in (lx as f32 - half) as i32..(lx as f32 + half) as i32 {
                    let fx = ((x - lx) as f32 / half).abs();
                    let k = (1.0 - fx * fx) * (1.0 - fy).powf(1.4) * 0.16;
                    glow(&mut img, x, y, [150.0, 200.0, 255.0], k);
                }
            }
        }
        // The rib: an I-beam at the bay's left edge, a teal strip light up it.
        rect(&mut img, 0, 0, rib, h, [44, 52, 66, 255]);
        rect(&mut img, 0, 0, 4, h, [74, 84, 102, 255]);
        rect(&mut img, rib - 4, 0, 4, h, [26, 30, 40, 255]);
        rect(&mut img, rib / 2 - 2, 0, 4, h, [60, 230, 220, 255]);
        for y in 0..h { for dx in -10..11 {
            glow(&mut img, rib / 2 + dx, y, [60.0, 220.0, 210.0], (1.0 - dx.abs() as f32 / 11.0).max(0.0).powi(2) * 0.4);
        }}
        for k in 0..(h / 40) { rect(&mut img, 6, 10 + k * 40, 3, 3, [96, 108, 128, 255]); rect(&mut img, rib - 9, 10 + k * 40, 3, 3, [96, 108, 128, 255]); }
        // Stencil marks low on the wall: a bay number made of blocks.
        let (bx, by) = (rib + 40, h * 8 / 9 + 10);
        let digits: [[u8; 15]; 2] = [
            [1,1,1, 0,0,1, 0,1,1, 0,0,1, 1,1,1], // 3
            [1,1,1, 1,0,1, 1,1,1, 0,0,1, 1,1,1], // 9
        ];
        for (n, d) in digits.iter().enumerate() {
            for i in 0..15 {
                if d[i] == 1 { rect(&mut img, bx + n as i32 * 30 + (i % 3) as i32 * 8, by + (i / 3) as i32 * 8, 7, 7, [200, 170, 70, 200]); }
            }
        }
        Arc::new(img)
    }).clone()
}

/// The ceiling: hull plating above, a truss with hanging lamps below.
fn ceil_img() -> Arc<Img> {
    static I: OnceLock<Arc<Img>> = OnceLock::new();
    I.get_or_init(|| {
        let (w, h) = ((TILE_W * ART) as i32, (HULL * ART) as i32);
        let mut img = Img::new(w as u32, h as u32);
        // Hull: big outer plates (the station's skin, seen from outside).
        let hull_h = h * 3 / 5;
        for row in 0..4 { for col in 0..4 {
            let shade = (hash((row * 5 + col) as u32 + 77) * 14.0) as u8;
            panel(&mut img, col * w / 4, row * hull_h / 4, w / 4, hull_h / 4, [44 + shade, 50 + shade, 64 + shade]);
        }}
        // A red beacon on the hull.
        for dy in -12..13 { for dx in -12..13 {
            let d = ((dx * dx + dy * dy) as f32).sqrt();
            glow(&mut img, w / 2 + dx, hull_h / 3 + dy, [255.0, 60.0, 50.0], (1.0 - d / 12.0).max(0.0).powi(2));
        }}
        // The truss: top and bottom chords and diagonals.
        let (ty, th) = (hull_h, h - hull_h);
        rect(&mut img, 0, ty, w, th, [14, 18, 26, 255]);
        let chord = [62, 70, 86, 255];
        rect(&mut img, 0, ty, w, 8, chord);
        rect(&mut img, 0, h - 10, w, 10, chord);
        let step = 40;
        for k in 0..(w / step + 1) {
            let x0 = k * step;
            let (x1, y1) = (x0 + step, h - 10);
            let n = 40;
            for i in 0..n {
                let t = i as f32 / n as f32;
                let (xa, ya) = if k % 2 == 0 { (x0 as f32 + t * step as f32, ty as f32 + 8.0 + t * (y1 - ty - 8) as f32) }
                               else { (x0 as f32 + t * step as f32, y1 as f32 - t * (y1 - ty - 8) as f32) };
                rect(&mut img, xa as i32, ya as i32, 3, 3, [50, 58, 72, 255]);
            }
            let _ = x1;
        }
        // Hanging lamps, matching the wall's light pools.
        for &lx in &[w / 3, w * 5 / 6] {
            rect(&mut img, lx - 1, h - 10, 2, 6, [80, 90, 110, 255]);
            rect(&mut img, lx - 10, h - 6, 20, 6, [200, 230, 255, 255]);
            for dy in -14..8 { for dx in -24..25 {
                let d = ((dx * dx) as f32 * 0.4 + (dy * dy) as f32).sqrt();
                glow(&mut img, lx + dx, h - 4 + dy, [170.0, 215.0, 255.0], (1.0 - d / 16.0).max(0.0) * 0.6);
            }}
        }
        Arc::new(img)
    }).clone()
}

/// The deck: a hazard-striped landing deck at the fall line, hull below.
fn deck_img() -> Arc<Img> {
    static I: OnceLock<Arc<Img>> = OnceLock::new();
    I.get_or_init(|| {
        let (w, h) = ((TILE_W * ART) as i32, (HULL * ART) as i32);
        let mut img = Img::new(w as u32, h as u32);
        // Deck plates.
        let dh = h / 3;
        for col in 0..8 {
            let shade = (hash(col as u32 + 300) * 10.0) as u8;
            panel(&mut img, col * w / 8, 18, w / 8, dh - 18, [40 + shade, 46 + shade, 58 + shade]);
        }
        // Hazard stripe along the edge.
        for x in 0..w { for y in 0..16 {
            let on = ((x + y) / 12) % 2 == 0;
            px(&mut img, x, y, if on { [230, 190, 40, 255] } else { [24, 22, 18, 255] });
        }}
        rect(&mut img, 0, 16, w, 2, [10, 10, 12, 255]);
        // Landing lights along the deck.
        for k in 0..(w / 80) {
            let lx = 40 + k * 80;
            rect(&mut img, lx - 3, 26, 6, 4, [120, 255, 200, 255]);
            for dy in -8..10 { for dx in -10..11 {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                glow(&mut img, lx + dx, 28 + dy, [80.0, 255.0, 190.0], (1.0 - d / 11.0).max(0.0) * 0.4);
            }}
        }
        // Hull below.
        for row in 0..3 { for col in 0..4 {
            let shade = (hash((row * 5 + col) as u32 + 501) * 12.0) as u8;
            panel(&mut img, col * w / 4, dh + row * (h - dh) / 3, w / 4, (h - dh) / 3, [36 + shade, 42 + shade, 56 + shade]);
        }}
        Arc::new(img)
    }).clone()
}

/// A door jamb: the heavy frame at an open end, full height (hull to hull).
fn jamb_img() -> Arc<Img> {
    static I: OnceLock<Arc<Img>> = OnceLock::new();
    I.get_or_init(|| {
        let total = DECK_Y + HULL - (CEIL_Y - HULL);
        let (w, h) = ((JAMB_W * ART_JAMB).max(60.0) as i32, (total * ART_JAMB) as i32);
        let mut img = Img::new(w as u32, h as u32);
        // Frame body.
        for y in 0..h { rect(&mut img, 0, y, w, 1, [50, 58, 74, 255]); }
        rect(&mut img, 0, 0, 3, h, [86, 96, 116, 255]);
        rect(&mut img, w - 3, 0, 3, h, [24, 28, 38, 255]);
        // Segments with bolts.
        for k in 0..(h / 30) {
            let y = k * 30;
            rect(&mut img, 0, y, w, 2, [30, 34, 46, 255]);
            for bx in [6, w - 9] { rect(&mut img, bx, y + 6, 3, 3, [110, 120, 140, 255]); }
        }
        // Chevrons down the inner face (between ceiling and deck).
        let (a, b) = ((HULL * ART_JAMB) as i32, ((HULL + DECK_Y - CEIL_Y) * ART_JAMB) as i32);
        let cx0 = w / 2 - 16;
        for y in a..b {
            for x in cx0..cx0 + 32 {
                let on = ((y + (x - cx0 - 16).abs()) / 10) % 2 == 0;
                px(&mut img, x, y, if on { [235, 185, 40, 255] } else { [26, 24, 20, 255] });
            }
        }
        // Beacons up the frame: alternating amber and teal.
        for k in 0..12 {
            let y = a + (b - a) * k / 11;
            let c = if k % 2 == 0 { [255.0, 180.0, 60.0] } else { [70.0, 230.0, 220.0] };
            for dy in -9..10 { for dx in -9..10 {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                glow(&mut img, w / 2 + dx, y + dy, c, (1.0 - d / 9.0).max(0.0).powf(1.5) * 0.9);
            }}
        }
        Arc::new(img)
    }).clone()
}

// ── Objects ──────────────────────────────────────────────────────────────────

fn ids() -> Vec<String> {
    let mut v = Vec::new();
    for i in 0..MAX_TILES { for k in ["wall", "ceil", "deck"] { v.push(format!("hangar_{k}_{i}")); } }
    v.push("hangar_jamb_in".into());
    v.push("hangar_jamb_out".into());
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
    for i in 0..MAX_TILES {
        let tx = x + i as f32 * TILE_W;
        if i < tiles {
            place(c, &format!("hangar_wall_{i}"), wall_img(), tx, CEIL_Y, TILE_W, DECK_Y - CEIL_Y);
            place(c, &format!("hangar_ceil_{i}"), ceil_img(), tx, CEIL_Y - HULL, TILE_W, HULL);
            place(c, &format!("hangar_deck_{i}"), deck_img(), tx, DECK_Y, TILE_W, HULL);
        } else {
            for k in ["wall", "ceil", "deck"] {
                if let Some(o) = c.get_game_object_mut(&format!("hangar_{k}_{i}")) { o.visible = false; }
            }
        }
    }
    let top = CEIL_Y - HULL;
    let jh = DECK_Y + HULL - top;
    place(c, "hangar_jamb_in", jamb_img(), x - JAMB_W * 0.5, top, JAMB_W, jh);
    place(c, "hangar_jamb_out", jamb_img(), x + len - JAMB_W * 0.5, top, JAMB_W, jh);
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
        for (name, img) in [("wall", wall_img()), ("ceil", ceil_img()), ("deck", deck_img()), ("jamb", jamb_img())] {
            assert!(img.width() <= 1024 && img.height() <= 1024, "{name} is {}x{}", img.width(), img.height());
            if let Ok(dir) = std::env::var("HANGAR_ART") { img.save(format!("{dir}/{name}.png")).unwrap(); }
        }
    }
}
