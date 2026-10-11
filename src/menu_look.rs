//! The main menu's look: "living sky".
//!
//! Modelled on the title screens of Hollow Knight, Celeste and Outer Wilds:
//! the game's own world is the backdrop and is alive (the night sky, the
//! moving stars, a planet bobbing, the player's own cat
//! swinging from a node), a big glowing logo, and a clean text list on the
//! left with one item lit and a marker beside it — no boxes. Secondary
//! screens sit as small text links in the bottom-right corner.
//!
//! The shop is not a separate screen: it is the sky ABOVE the menu (Alto's
//! Adventure's trick). Opening it eases the camera up through one continuous
//! sky; the layers move at different rates as it rises (parallax), so it reads
//! as looking up rather than as a scene change. See `tick_menu_look`.
//!
//! The buttons keep the names `menu.rs` wires clicks and keys to
//! (`start_btn`, `menu_shop_btn`, ...); here they are invisible hit areas
//! behind the text. The list is driven by `menu_sel`: arrow keys and the
//! mouse move it, Enter (or a click) activates it, Space still plays.

use quartz::*;
use std::sync::{Arc, OnceLock};

use crate::constants::*;
use crate::objects::{ui_text_left_spec, ui_text_spec};
use crate::scenes::game::helpers::ui_font;

/// The menu occupies y = MENU_Y..MENU_Y+VH (the shop is above it, at 0..VH,
/// with sky between); every menu position here is relative to that.
use crate::menu::MENU_Y;

/// How far past the world's top and bottom the sky is drawn. A window taller
/// than 16:9 shows more than VH of the world; without this it showed black.
const SKY_BLEED: f32 = VH * 0.5;

// ── Layout ────────────────────────────────────────────────────────────────────
const LOGO_POS: (f32, f32) = (230.0, 240.0);
const LOGO_W: f32 = 1700.0;
const TAGLINE_Y: f32 = 600.0;
const LIST_X: f32 = 330.0;
const LIST_Y0: f32 = 960.0;
const LIST_STEP: f32 = 168.0;
const ITEM_W: f32 = 700.0;
const ITEM_H: f32 = 140.0;
const ITEM_SIZE: f32 = 66.0;
/// The mode picker sits on the PLAY row, to its right.
const MODE_X: f32 = LIST_X + 520.0;
const MODE_W: f32 = 760.0;
/// The hero swing: the node it hangs from, the rope's length, and the cat.
const NODE_POS: (f32, f32) = (VW * 0.70, 330.0);
const ROPE_LEN: f32 = 980.0;
const CAT_SIZE: f32 = 320.0;
const NODE_SIZE: f32 = 210.0;
/// A far planet, high on the right, cut by the screen's top.
const PLANET_POS: (f32, f32) = (VW * 0.80, -260.0);
const PLANET_SIZE: f32 = 980.0;
/// Parallax: the share of the camera's climb into the shop each layer moves
/// WITH the camera. 0 is pinned to the world (the horizon, the swing, the UI);
/// near 1 is far away and barely moves.
const PAR_SKY: f32 = 0.0;
const PAR_FAR_STARS: f32 = 0.5;
const PAR_NEAR_STARS: f32 = 0.2;
const PAR_PLANET: f32 = 0.5;
const PAR_GALAXY: f32 = 0.05;
/// A spiral galaxy that lives up in the shop's sky, low in its view below the
/// cards; from the menu only a sliver of it shows at the top edge.
const GALAXY_POS: (f32, f32) = (600.0, 1628.0);
const GALAXY_SIZE: f32 = 600.0;
const SWING_AMP: f32 = 0.62;
const SWING_RATE: f32 = 0.0155;

/// The list, top to bottom: (hit-area object, text object, label). Boss order
/// is a phone-only entry (it is `B` on desktop).
fn items() -> Vec<(&'static str, &'static str, &'static str)> {
    let mut v = vec![
        ("start_btn", "menu_start_text", "PLAY"),
        ("menu_shop_btn", "menu_shop_text", "SHOP"),
        ("menu_settings_btn", "menu_settings_text", "SETTINGS"),
        ("menu_profile_btn", "menu_profile_text", "PROFILES"),
    ];
    if mobile_controls_enabled() {
        v.push(("menu_boss_order_btn", "menu_boss_order_text", "BOSS ORDER"));
    }
    v
}

/// The corner links: (hit-area object, text object, label, width).
const LINKS: [(&str, &str, &str, f32); 3] = [
    ("menu_achievements_btn", "menu_achievements_text", "ACHIEVEMENTS", 430.0),
    ("menu_stats_btn", "menu_stats_text", "STATS", 230.0),
    ("menu_daily_btn", "menu_daily_text", "DAILY REWARD", 430.0),
];
const LINKS_Y: f32 = 1955.0;
const LINKS_RIGHT: f32 = VW - 200.0;
const LINK_GAP: f32 = 40.0;

/// Colours: the lit item, the rest, the accents.
const LIT: Color = Color(255, 255, 255, 255);
const DIM: Color = Color(160, 185, 230, 190);
const ACCENT: Color = Color(150, 200, 255, 255);

fn link_xs() -> [f32; 3] {
    let mut xs = [0.0; 3];
    let mut right = LINKS_RIGHT;
    for i in (0..3).rev() {
        xs[i] = right - LINKS[i].3;
        right = xs[i] - LINK_GAP;
    }
    xs
}

// ── Art (built once, shared Arcs: nothing re-uploads per frame) ──────────────

fn smooth(e0: f32, e1: f32, x: f32) -> f32 { let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

/// The sky quad's world span: the whole world, shop and menu, plus bleed.
fn sky_span() -> (f32, f32) { (-SKY_BLEED, MENU_Y + VH + SKY_BLEED) }

/// The night, the full height of the world (shop above, menu below): deep
/// space at the top, navy overhead of the menu, a violet glow at the horizon.
fn sky_image() -> Arc<image::RgbaImage> {
    static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    IMG.get_or_init(|| { let (top, bot) = sky_span(); sky_band(top, bot, 720) }).clone()
}

/// The sky's colour at world height `y`.
fn sky_colour(y: f32) -> [u8; 3] {
    // Stops at WORLD heights: the shop's sky, then the menu's.
    // Blue: deep navy overhead to a blue glow at the horizon.
    let stops: [(f32, [f32; 3]); 6] = [
        (0.0, [2.0, 4.0, 14.0]), (VH * 0.56, [5.0, 12.0, 34.0]),
        (MENU_Y, [3.0, 7.0, 22.0]), (MENU_Y + VH * 0.45, [7.0, 16.0, 44.0]),
        (MENU_Y + VH * 0.78, [18.0, 40.0, 92.0]), (MENU_Y + VH, [52.0, 98.0, 168.0]),
    ];
    let t = y.clamp(stops[0].0, stops[5].0);
    let i = stops.iter().position(|s| s.0 >= t).unwrap_or(stops.len() - 1).max(1);
    let (a, b) = (stops[i - 1], stops[i]);
    let k = smooth(0.0, 1.0, (t - a.0) / (b.0 - a.0));
    [0, 1, 2].map(|j| (a.1[j] + (b.1[j] - a.1[j]) * k) as u8)
}

/// The sky gradient between world heights `top` and `bot`, `h` rows.
fn sky_band(top: f32, bot: f32, h: u32) -> Arc<image::RgbaImage> {
    Arc::new(image::RgbaImage::from_fn(8, h, |_, y| {
        let c = sky_colour(top + (bot - top) * y as f32 / (h - 1).max(1) as f32);
        image::Rgba([c[0], c[1], c[2], 255])
    }))
}

/// Stars on a transparent field: `bright` for the drifting near layer.
fn star_layer(seed: u64, count: u32, bright: bool) -> Arc<image::RgbaImage> {
    // Full height: the layers tile vertically up into the shop's sky, so a
    // starless band would show as a seam (the horizon hides the bottom).
    let (w, h) = (1920u32, 1080u32);
    let mut img = image::RgbaImage::new(w, h);
    let mut s = seed.max(1);
    let mut next = || { s ^= s << 13; s ^= s >> 7; s ^= s << 17; s };
    for _ in 0..count {
        let x = (next() % w as u64) as u32;
        let y = (next() % h as u64) as u32;
        let b = 90 + (next() % 166) as u32;
        let b = if bright { b } else { b * 2 / 3 };
        let big = next() % 100 < if bright { 18 } else { 6 };
        let px = |img: &mut image::RgbaImage, x: u32, y: u32, a: u32| {
            if x < w && y < h { img.put_pixel(x, y, image::Rgba([235, 238, 255, a.min(255) as u8])); }
        };
        px(&mut img, x, y, b);
        if big { px(&mut img, x + 1, y, b / 2); px(&mut img, x, y + 1, b / 2); px(&mut img, x.wrapping_sub(1), y, b / 2); px(&mut img, x, y.wrapping_sub(1), b / 2); }
    }
    Arc::new(img)
}

fn static_stars() -> Arc<image::RgbaImage> {
    static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    IMG.get_or_init(|| star_layer(0xA11CE, 1100, false)).clone()
}

fn moving_stars() -> Arc<image::RgbaImage> {
    static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    IMG.get_or_init(|| star_layer(0xBEE5, 320, true)).clone()
}

/// The cloud tops at the horizon: the game's aurora photo, its lower part,
/// tinted into the night and faded in at its top edge.
fn horizon_image() -> Arc<image::RgbaImage> {
    static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    IMG.get_or_init(|| {
        let src = image::load_from_memory(include_bytes!("../assets/aurora_earth.gif"))
            .map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::new(4, 4));
        let full = image::imageops::resize(&src, 1600, 900, image::imageops::FilterType::Triangle);
        let top = 470u32;
        let (w, h) = (1600u32, 900 - top);
        Arc::new(image::RgbaImage::from_fn(w, h, |x, y| {
            let p = full.get_pixel(x, y + top);
            let fade = smooth(0.0, 0.42, y as f32 / h as f32);
            let tint = [0.26, 0.40, 0.68];
            image::Rgba([(p[0] as f32 * tint[0]) as u8, (p[1] as f32 * tint[1]) as u8, (p[2] as f32 * tint[2]) as u8, (fade * 255.0) as u8])
        }))
    }).clone()
}

/// The far star layer, tiled from the menu up past the shop's top.
fn far_star_rows() -> [(&'static str, f32); 4] {
    [("menu_stars_far", MENU_Y), ("menu_stars_far_up", MENU_Y - VH),
     ("menu_stars_far_up2", MENU_Y - VH * 2.0), ("menu_stars_far_up3", MENU_Y - VH * 3.0)]
}

/// The extra left-hand copy of each drifting row (see `menu_objects`).
fn near_star_lefts() -> [(&'static str, f32); 3] {
    [("menu_stars_a_l", MENU_Y), ("menu_stars_c_l", MENU_Y - VH), ("menu_stars_e_l", MENU_Y - VH * 2.0)]
}

/// The near (drifting) stars: three rows, each a wrapping pair.
fn near_star_tiles() -> [(&'static str, f32); 6] {
    [("menu_stars_a", MENU_Y), ("menu_stars_b", MENU_Y),
     ("menu_stars_c", MENU_Y - VH), ("menu_stars_d", MENU_Y - VH),
     ("menu_stars_e", MENU_Y - VH * 2.0), ("menu_stars_f", MENU_Y - VH * 2.0)]
}

/// The horizon image's bottom row, averaged: what lies below it.
fn horizon_floor() -> Arc<image::RgbaImage> {
    static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    IMG.get_or_init(|| {
        let h = horizon_image();
        let y = h.height() - 1;
        let mut sum = [0u32; 3];
        for x in 0..h.width() { let p = h.get_pixel(x, y); for k in 0..3 { sum[k] += p[k] as u32; } }
        let n = h.width();
        solid([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, 255])
    }).clone()
}

fn solid(rgba: [u8; 4]) -> Arc<image::RgbaImage> {
    Arc::new(image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba)))
}

fn clear_pixel() -> Arc<image::RgbaImage> {
    static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    IMG.get_or_init(|| solid([0, 0, 0, 0])).clone()
}

/// The selection marker: a small glowing diamond.
fn marker_image() -> Arc<image::RgbaImage> {
    static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    IMG.get_or_init(|| {
        let n = 64u32;
        Arc::new(image::RgbaImage::from_fn(n, n, |x, y| {
            let (u, v) = ((x as f32 + 0.5 - 32.0) / 32.0, (y as f32 + 0.5 - 32.0) / 32.0);
            let d = u.abs() + v.abs();
            let core = smooth(0.42, 0.36, d);
            let glow = (1.0 - d).max(0.0).powf(2.2) * 0.55;
            let a = core.max(glow);
            let c = [255.0 * core + 150.0 * (1.0 - core), 255.0 * core + 200.0 * (1.0 - core), 255.0];
            image::Rgba([c[0] as u8, c[1] as u8, c[2] as u8, (a * 255.0) as u8])
        }))
    }).clone()
}

/// A soft light bar behind the lit item, fading out to the right.
fn bar_image() -> Arc<image::RgbaImage> {
    static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    IMG.get_or_init(|| {
        let (w, h) = (256u32, 32u32);
        Arc::new(image::RgbaImage::from_fn(w, h, |x, y| {
            let fx = x as f32 / w as f32;
            let fy = (y as f32 + 0.5) / h as f32;
            let a = smooth(0.0, 0.08, fx) * smooth(1.0, 0.2, fx) * (1.0 - ((fy - 0.5) * 2.0).abs()).powf(1.5);
            image::Rgba([90, 160, 255, (a * 90.0) as u8])
        }))
    }).clone()
}

/// The logo: BALL SWING in the game's font, a cool white-to-lilac fill, a
/// dark outline so it holds on any sky, and a violet glow around it.
fn logo_image() -> Option<Arc<image::RgbaImage>> {
    glow_text("BALL SWING", 260.0)
}

/// Text in the logo's style (fill, outline, violet glow), at `px` glyph size.
/// Built once per (text, size); the page titles use it too.
pub fn glow_text(text: &str, px: f32) -> Option<Arc<image::RgbaImage>> {
    glow_text_rgb(text, px, [80, 150, 255], [170, 210, 255])
}

/// `glow_text` in other colours: the glow, and the fill at the foot of the
/// letters (the top is always white). Cached by all of its arguments.
pub fn glow_text_rgb(text: &str, px: f32, glow: [u8; 3], foot: [u8; 3]) -> Option<Arc<image::RgbaImage>> {
    type Key = (String, u32, [u8; 3], [u8; 3]);
    static CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<Key, Option<Arc<image::RgbaImage>>>>> = OnceLock::new();
    let key = (text.to_string(), px as u32, glow, foot);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().unwrap().get(&key) { return v.clone(); }
    let v = build_glow_text(text, px, glow, foot);
    cache.lock().unwrap().insert(key, v.clone());
    v
}

fn build_glow_text(text: &str, px: f32, glow_rgb: [u8; 3], foot: [u8; 3]) -> Option<Arc<image::RgbaImage>> {
    {
        let font = fontdue::Font::from_bytes(&include_bytes!("../assets/font.ttf")[..], fontdue::FontSettings::default()).ok()?;
        // Everything below was tuned at the logo's 260 px; it scales with size.
        let k = px / 260.0;
        let spacing = 22.0f32 * k;
        let lm = font.horizontal_line_metrics(px)?;
        let pad = (70.0 * k).round().max(8.0) as i32;
        let glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = text.chars().map(|c| font.rasterize(c, px)).collect();
        let width: f32 = glyphs.iter().map(|(m, _)| m.advance_width + spacing).sum::<f32>() - spacing;
        let (w, h) = (width.ceil() as i32 + pad * 2, (lm.ascent - lm.descent).ceil() as i32 + pad * 2);
        let mut alpha = vec![0f32; (w * h) as usize];
        let mut pen = pad as f32;
        for (m, bmp) in &glyphs {
            let gx = (pen + m.xmin as f32).round() as i32;
            let gy = (pad as f32 + lm.ascent - (m.height as f32 + m.ymin as f32)).round() as i32;
            for yy in 0..m.height as i32 { for xx in 0..m.width as i32 {
                let (x, y) = (gx + xx, gy + yy);
                if x >= 0 && y >= 0 && x < w && y < h {
                    let a = bmp[(yy * m.width as i32 + xx) as usize] as f32 / 255.0;
                    let i = (y * w + x) as usize;
                    alpha[i] = alpha[i].max(a);
                }
            }}
            pen += m.advance_width + spacing;
        }
        // Outline: the glyphs grown by a few px.
        let grow = |src: &[f32], r: i32| -> Vec<f32> {
            let mut out = vec![0f32; src.len()];
            for y in 0..h { for x in 0..w {
                let mut m = 0f32;
                for dy in -r..=r { for dx in -r..=r {
                    if dx * dx + dy * dy > r * r { continue; }
                    let (xx, yy) = (x + dx, y + dy);
                    if xx >= 0 && yy >= 0 && xx < w && yy < h { m = m.max(src[(yy * w + xx) as usize]); }
                }}
                out[(y * w + x) as usize] = m;
            }}
            out
        };
        let outline = grow(&alpha, (7.0 * k).round().max(2.0) as i32);
        // Glow: the outline, blurred wide.
        let mut glow = outline.clone();
        let r = (26.0 * k).round().max(4.0) as i32;
        for _ in 0..3 {
            let mut tmp = vec![0f32; glow.len()];
            for y in 0..h { for x in 0..w {
                let (lo, hi) = ((x - r).max(0), (x + r).min(w - 1));
                tmp[(y * w + x) as usize] = (lo..=hi).map(|xx| glow[(y * w + xx) as usize]).sum::<f32>() / (hi - lo + 1) as f32;
            }}
            for y in 0..h { for x in 0..w {
                let (lo, hi) = ((y - r).max(0), (y + r).min(h - 1));
                glow[(y * w + x) as usize] = (lo..=hi).map(|yy| tmp[(yy * w + x) as usize]).sum::<f32>() / (hi - lo + 1) as f32;
            }}
        }
        let img = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
            let i = (y as i32 * w + x as i32) as usize;
            let t = ((y as f32 - pad as f32) / (lm.ascent - lm.descent)).clamp(0.0, 1.0);
            // Fill: white at the top to `foot` at the foot of the letters.
            let fill = [0, 1, 2].map(|k| 255.0 + (foot[k] as f32 - 255.0) * t);
            let edge = [22.0, 14.0, 52.0];
            let gl = glow_rgb.map(|v| v as f32);
            let (fa, oa, ga) = (alpha[i], outline[i], (glow[i] * 1.6).min(1.0) * 0.75);
            // Glow under the outline under the fill, composited straight.
            let mut c = [0f32; 3]; let mut a = 0f32;
            for (col, al) in [(gl, ga), (edge, oa), (fill, fa)] {
                for k in 0..3 { c[k] = col[k] * al + c[k] * (1.0 - al); }
                a = al + a * (1.0 - al);
            }
            if a < 0.004 { return image::Rgba([0, 0, 0, 0]); }
            let n = |k: usize| (c[k] / a.max(1e-3)).clamp(0.0, 255.0) as u8;
            image::Rgba([n(0), n(1), n(2), (a * 255.0) as u8])
        });
        Some(Arc::new(img))
    }
}

// ── Objects ──────────────────────────────────────────────────────────────────

fn quad(ctx: &mut Context, id: &str, img: Arc<image::RgbaImage>, size: (f32, f32), pos: (f32, f32), tags: Vec<String>) -> (String, GameObject) {
    let o = GameObject::new_rect(ctx, id.into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img, color: None }),
        size, pos, tags, (0.0, 0.0), (1.0, 1.0), 0.0);
    (id.to_string(), o)
}

fn text_obj(ctx: &mut Context, id: &str, size: (f32, f32), pos: (f32, f32)) -> (String, GameObject) {
    (id.to_string(), GameObject::build(id).size(size.0, size.1).position(pos.0, pos.1).tag("ui").build(ctx))
}

/// Every menu object, in draw order: the sky, then the hero, then the UI.
pub fn menu_objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    let y0 = MENU_Y;
    let mut v = Vec::new();
    // The sky spans the whole world, shop and menu: one sky, not two screens.
    let (top, bot) = sky_span();
    v.push(quad(ctx, "menu_bg", sky_image(), (VW, bot - top), (0.0, top), vec![]));
    for (id, y) in far_star_rows() {
        v.push(quad(ctx, id, static_stars(), (VW, VH), (0.0, y), vec![]));
    }
    let galaxy = crate::scenes::game::helpers::pl_image_cached(include_bytes!("../assets/pixellab/decor/galaxy_spiral.png"), 256.0);
    if let Some(g) = galaxy {
        let o = GameObject::new_rect(ctx, "menu_galaxy".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (GALAXY_SIZE, GALAXY_SIZE), 0.0), image: g, color: Some(Color(170, 150, 220, 200)) }),
            (GALAXY_SIZE, GALAXY_SIZE), GALAXY_POS, vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        v.push(("menu_galaxy".into(), o));
    }
    for (id, y) in near_star_tiles() {
        v.push(quad(ctx, id, moving_stars(), (VW, VH), (0.0, y), vec![]));
    }
    // A third copy per drifting row, to the left: the pair covers exactly
    // one screen, and the sling's zoom-out shows past it.
    for (id, y) in near_star_lefts() {
        v.push(quad(ctx, id, moving_stars(), (VW, VH), (-VW, y), vec![]));
    }
    let planet = crate::scenes::game::helpers::pl_image_cached(include_bytes!("../assets/pixellab/decor/planet_ringed.png"), 512.0);
    if let Some(p) = planet {
        // Far off: dimmed and cooled into the night (a multiply; still solid).
        // Drawn after the drifting stars so none of them cross in front of it.
        let o = GameObject::new_rect(ctx, "menu_planet".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (PLANET_SIZE, PLANET_SIZE), 0.0), image: p, color: Some(Color(105, 96, 150, 255)) }),
            (PLANET_SIZE, PLANET_SIZE), (PLANET_POS.0, y0 + PLANET_POS.1), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        v.push(("menu_planet".into(), o));
    }
    v.push(quad(ctx, "menu_horizon", horizon_image(), (VW, VH * 0.4), (0.0, y0 + VH * 0.6), vec![]));
    // Below the horizon's edge (only a tall window sees it): its own colour.
    v.push(quad(ctx, "menu_floor", horizon_floor(), (VW, SKY_BLEED), (0.0, y0 + VH), vec![]));
    // The hero: a node, a rope, the player's cat.
    v.push(quad(ctx, "menu_rope", solid([255, 255, 255, 255]), (12.0, ROPE_LEN), (NODE_POS.0, y0 + NODE_POS.1), vec![]));
    {
        let mut node = GameObject::new_rect(ctx, "menu_node".into(), None::<Image>, (NODE_SIZE, NODE_SIZE),
            (NODE_POS.0 - NODE_SIZE * 0.5, y0 + NODE_POS.1 - NODE_SIZE * 0.5), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        node.set_animation(crate::scenes::game::helpers::hook_artifact_anim());
        v.push(("menu_node".into(), node));
    }
    v.push(quad(ctx, "menu_cat", clear_pixel(), (CAT_SIZE, CAT_SIZE), (NODE_POS.0, y0 + NODE_POS.1 + ROPE_LEN), vec![]));
    // The UI.
    if let Some(logo) = logo_image() {
        let lh = LOGO_W * logo.height() as f32 / logo.width() as f32;
        v.push(quad(ctx, "menu_logo", logo, (LOGO_W, lh), (LOGO_POS.0, y0 + LOGO_POS.1), vec!["ui".into()]));
    }
    v.push(text_obj(ctx, "menu_sub_text", (LOGO_W, 60.0), (LOGO_POS.0, y0 + TAGLINE_Y)));
    v.push(quad(ctx, "menu_sel_bar", bar_image(), (ITEM_W + 380.0, ITEM_H), (LIST_X - 120.0, y0 + LIST_Y0), vec!["ui".into()]));
    v.push(quad(ctx, "menu_sel_marker", marker_image(), (54.0, 54.0), (LIST_X - 92.0, y0 + LIST_Y0), vec!["ui".into()]));
    for (k, (hit, text, _)) in items().into_iter().enumerate() {
        let y = y0 + LIST_Y0 + k as f32 * LIST_STEP;
        v.push(quad(ctx, hit, clear_pixel(), (ITEM_W, ITEM_H), (LIST_X - 40.0, y), vec!["ui".into(), "button".into()]));
        v.push(text_obj(ctx, text, (ITEM_W, ITEM_H), (LIST_X, y + (ITEM_H - ITEM_SIZE * 1.1) * 0.5)));
    }
    // The mode picker on the PLAY row.
    v.push(quad(ctx, "menu_mode_selector", clear_pixel(), (MODE_W, ITEM_H), (MODE_X, y0 + LIST_Y0), vec!["ui".into()]));
    v.push(text_obj(ctx, "menu_mode_name_text", (MODE_W, ITEM_H), (MODE_X, y0 + LIST_Y0 + (ITEM_H - 46.0) * 0.5)));
    v.push(text_obj(ctx, "menu_mode_desc_text", (MODE_W + 300.0, 50.0), (MODE_X - 150.0, y0 + LIST_Y0 + ITEM_H - 6.0)));
    // Corner links and the controls hint.
    for (i, x) in link_xs().into_iter().enumerate() {
        let (hit, text, _, w) = LINKS[i];
        v.push(quad(ctx, hit, clear_pixel(), (w, 90.0), (x, y0 + LINKS_Y), vec!["ui".into(), "button".into()]));
        v.push(text_obj(ctx, text, (w, 90.0), (x, y0 + LINKS_Y + 24.0)));
    }
    v.push(text_obj(ctx, "menu_hint_text", (1600.0, 50.0), (LIST_X - 92.0, y0 + LINKS_Y + 30.0)));
    v
}

/// Put the player's equipped cat on the rope (call on menu entry).
pub fn apply_menu_cat(c: &mut Canvas) {
    let char_idx = (c.get_i32("player_char_selected").max(0) as usize).min(PLAYER_CHAR_COLORS.len() - 1);
    let anim = if char_idx == 0 {
        AnimatedSprite::new(include_bytes!("../assets/calicoball.gif"), (CAT_SIZE, CAT_SIZE), CALICO_FPS).ok()
    } else {
        crate::cosmetics::cat_sprite(char_idx, CAT_SIZE, CALICO_FPS)
    };
    let rope = {
        let i = (c.get_i32("player_rope_selected").max(0) as usize).min(crate::shop::SHOP_ROPE_COLORS.len() - 1);
        crate::shop::SHOP_ROPE_COLORS[i]
    };
    if let Some(o) = c.get_game_object_mut("menu_cat") {
        match anim {
            Some(mut a) => { a.set_fps(0.0); o.set_animation(a); }
            None => {
                o.animated_sprite = None;
                let (r, g, b) = PLAYER_CHAR_COLORS[char_idx];
                o.set_image(Image { shape: ShapeType::Ellipse(0.0, (CAT_SIZE, CAT_SIZE), 0.0), image: crate::images::circle_cached((CAT_SIZE * 0.5) as u32, r, g, b), color: None });
            }
        }
    }
    if let Some(o) = c.get_game_object_mut("menu_rope") {
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, (12.0, ROPE_LEN), 0.0), image: solid([rope.0, rope.1, rope.2, 235]), color: None });
    }
}

// ── Text ─────────────────────────────────────────────────────────────────────

/// (Re)draw every label; the lit item is white, the rest dim.
pub fn draw_menu_text(c: &mut Canvas) {
    let Some(font) = ui_font() else { return };
    let s = c.virtual_scale();
    let sel = c.get_i32("menu_sel").max(0) as usize;
    for (k, (_, text, label)) in items().into_iter().enumerate() {
        let col = if k == sel { LIT } else { DIM };
        if let Some(o) = c.get_game_object_mut(text) {
            o.set_drawable(Box::new(ui_text_left_spec(label, &font, ITEM_SIZE * s, col, ITEM_W * s)));
        }
    }
    let idx = (c.get_i32("game_mode_idx").max(0) as usize).min(crate::menu::GAME_MODES.len() - 1);
    let (name, desc) = crate::menu::GAME_MODES[idx];
    let mode_col = if sel == 0 { ACCENT } else { Color(150, 140, 200, 200) };
    let mode = format!("<   {name}   >");
    for (id, t, sz, col, w, left) in [
        ("menu_mode_name_text", mode.as_str(), 40.0, mode_col, MODE_W, false),
        ("menu_mode_desc_text", desc, 22.0, Color(160, 150, 210, if sel == 0 { 220 } else { 150 }), MODE_W + 300.0, false),
        ("menu_sub_text", "S W I N G   T H R O U G H   T H E   S T A R S", 26.0, Color(170, 200, 245, 210), LOGO_W, false),
        ("menu_hint_text", "SPACE  PLAY      UP / DOWN  SELECT      LEFT / RIGHT  MODE      ENTER  OPEN", 28.0, Color(170, 195, 240, 215), 1600.0, true),
    ] {
        if let Some(o) = c.get_game_object_mut(id) {
            let spec = if left { ui_text_left_spec(t, &font, sz * s, col, w * s) } else { ui_text_spec(t, &font, sz * s, col, w * s) };
            o.set_drawable(Box::new(spec));
        }
    }
    let hover = c.get_i32("menu_link_hover");
    for (i, (_, text, label, w)) in LINKS.iter().enumerate() {
        let col = if hover == i as i32 { LIT } else { Color(190, 210, 245, 230) };
        if let Some(o) = c.get_game_object_mut(text) {
            o.set_drawable(Box::new(ui_text_spec(label, &font, 34.0 * s, col, w * s)));
        }
    }
}

// ── Selection ────────────────────────────────────────────────────────────────

/// How many list items there are.
pub fn item_count() -> i32 { items().len() as i32 }

/// Move the lit item by `delta` (wrapping).
pub fn step_selection(c: &mut Canvas, delta: i32) {
    let n = item_count();
    let sel = (c.get_i32("menu_sel") + delta).rem_euclid(n);
    c.set_var("menu_sel", sel);
    draw_menu_text(c);
}

/// The hit-area object of the lit item (what Enter opens).
pub fn selected_button(c: &Canvas) -> &'static str {
    let v = items();
    v[(c.get_i32("menu_sel").max(0) as usize).min(v.len() - 1)].0
}

// ── Per frame ────────────────────────────────────────────────────────────────

/// Animate the sky and the swing (always — they are on screen through the
/// climb into the shop), and, while on the menu, follow the mouse for hover
/// and ease the marker to the lit item.
pub fn tick_menu_look(c: &mut Canvas) {
    let t = c.get_i32("menu_t") + 1;
    c.set_var("menu_t", t);
    let tf = t as f32;
    let y0 = MENU_Y;
    // How far the camera has climbed from the menu towards the shop.
    let climb = MENU_Y - c.camera().map_or(MENU_Y, |cam| cam.position.1);
    let lift = |par: f32| -climb * par;
    if let Some(o) = c.get_game_object_mut("menu_bg") { o.position.1 = sky_span().0 + lift(PAR_SKY); }
    for (id, y) in far_star_rows() {
        if let Some(o) = c.get_game_object_mut(id) { o.position.1 = (y + lift(PAR_FAR_STARS)).round(); }
    }
    for (id, y) in near_star_tiles().into_iter().chain(near_star_lefts()) {
        if let Some(o) = c.get_game_object_mut(id) { o.position.1 = (y + lift(PAR_NEAR_STARS)).round(); }
    }
    if let Some(o) = c.get_game_object_mut("menu_galaxy") {
        o.position = (GALAXY_POS.0 + (tf * 0.003).sin() * 12.0, GALAXY_POS.1 + lift(PAR_GALAXY));
        o.rotation = tf * 0.02;
    }

    // Drifting layers: two copies each, wrapping.
    let drift = |c: &mut Canvas, a: &str, b: &str, speed: f32| {
        let w = c.get_game_object(a).map_or(VW, |o| o.size.0);
        let x = -((tf * speed).rem_euclid(w));
        if let Some(o) = c.get_game_object_mut(a) { o.position.0 = x.round(); }
        if let Some(o) = c.get_game_object_mut(b) { o.position.0 = (x + w).round(); }
        if let Some(o) = c.get_game_object_mut(&format!("{a}_l")) { o.position.0 = (x - w).round(); }
    };
    drift(c, "menu_stars_a", "menu_stars_b", 0.35);
    drift(c, "menu_stars_c", "menu_stars_d", 0.35);
    drift(c, "menu_stars_e", "menu_stars_f", 0.35);
    // The planet's pop when clicked: a damped spring, about a third of a
    // second, swelling first then settling with a little undershoot.
    let pop = match c.get_var("planet_pop_t") {
        Some(Value::I32(t0)) => {
            let e = (t - t0) as f32;
            if (0.0..40.0).contains(&e) { 1.0 + 0.14 * (-e / 7.0).exp() * (e * 0.42).sin() } else { 1.0 }
        }
        _ => 1.0,
    };
    if let Some(o) = c.get_game_object_mut("menu_planet") {
        let size = PLANET_SIZE * pop;
        let grow = (size - PLANET_SIZE) * 0.5;
        o.size = (size, size);
        o.position = (PLANET_POS.0 + (tf * 0.004).sin() * 30.0 - grow, y0 + PLANET_POS.1 + (tf * 0.009).sin() * 18.0 + lift(PAR_PLANET) - grow);
        o.rotation = (tf * 0.006).sin() * 3.0;
    }

    // The swing: a slow pendulum, the cat curling at the ends of the arc.
    let th = SWING_AMP * (tf * SWING_RATE).sin();
    let (nx, ny) = (NODE_POS.0, y0 + NODE_POS.1);
    let (cx, cy) = (nx + ROPE_LEN * th.sin(), ny + ROPE_LEN * th.cos());
    if let Some(o) = c.get_game_object_mut("menu_rope") {
        let (mx, my) = ((nx + cx) * 0.5, (ny + cy) * 0.5);
        o.position = (mx - 6.0, my - ROPE_LEN * 0.5);
        o.rotation = -th.to_degrees();
    }
    if let Some(o) = c.get_game_object_mut("menu_cat") {
        o.position = (cx - CAT_SIZE * 0.5, cy - CAT_SIZE * 0.5);
        o.rotation = -th.to_degrees() * 1.6 + tf * 0.4;
    }

    if c.get_bool("menu_in_shop") { return; }
    // Launching (PLAY) or arriving: the UI is placed by `tick_launch`.
    if tick_launch(c) { return; }

    // Hover: the mouse over an item lights it; over a link, underline-lights it.
    if let Some(sp) = c.mouse_position() {
        let w = c.screen_to_world(sp);
        let inside = |c: &Canvas, id: &str| c.get_game_object(id).map_or(false, |o|
            w.0 >= o.position.0 && w.0 <= o.position.0 + o.size.0 && w.1 >= o.position.1 && w.1 <= o.position.1 + o.size.1);
        let moved = c.get_f32("menu_mouse_x") != sp.0 || c.get_f32("menu_mouse_y") != sp.1;
        c.set_var("menu_mouse_x", sp.0);
        c.set_var("menu_mouse_y", sp.1);
        if moved {
            for (k, (hit, _, _)) in items().into_iter().enumerate() {
                if (inside(c, hit) || (k == 0 && inside(c, "menu_mode_selector"))) && c.get_i32("menu_sel") != k as i32 {
                    c.set_var("menu_sel", k as i32);
                    draw_menu_text(c);
                }
            }
            let link = LINKS.iter().position(|l| inside(c, l.0)).map_or(-1, |i| i as i32);
            if link != c.get_i32("menu_link_hover") {
                c.set_var("menu_link_hover", link);
                draw_menu_text(c);
            }
        }
    }

    // Marker and bar ease to the lit row; the lit text nudges right.
    let sel = c.get_i32("menu_sel").max(0) as f32;
    let target = y0 + LIST_Y0 + sel * LIST_STEP;
    let cur = { let v = c.get_f32("menu_marker_y"); if v == 0.0 { target } else { v } };
    let y = cur + (target - cur) * 0.25;
    c.set_var("menu_marker_y", y);
    let pulse = 1.0 + 0.08 * (tf * 0.08).sin();
    if let Some(o) = c.get_game_object_mut("menu_sel_marker") {
        let s = 54.0 * pulse;
        o.size = (s, s);
        o.position = (LIST_X - 92.0 - (s - 54.0) * 0.5, y + (ITEM_H - s) * 0.5 - 4.0);
    }
    if let Some(o) = c.get_game_object_mut("menu_sel_bar") { o.position.1 = y; }
    for (k, (_, text, _)) in items().into_iter().enumerate() {
        let lit = k as f32 == sel;
        let key = format!("menu_nudge_{k}");
        let n = c.get_f32(&key);
        let n = n + ((if lit { 26.0 } else { 0.0 }) - n) * 0.25;
        c.set_var(&key, n);
        if let Some(o) = c.get_game_object_mut(text) { o.position.0 = LIST_X + n; }
    }
}

// ── The other menu pages (settings, profiles, achievements, stats, daily) ───
//
// They share the menu's look so leaving the menu does not feel like leaving
// the game: the same night sky, drifting stars and horizon behind them, a
// title in the logo's glowing style, content on violet glass, and the same
// "‹ BACK" pill bottom-left that the shop uses.

/// The scenes that wear the page look (their drift is ticked by `register_page_tick`).
const PAGE_SCENES: [&str; 5] = ["menu_settings", "profile", "achievements", "stats", "daily_reward"];

/// Title: glyph size, and the top of the image.
const PAGE_TITLE_PX: f32 = 130.0;
const PAGE_TITLE_Y: f32 = 70.0;

/// Where every page's back pill sits (the shop's spot), and its size.
pub const PAGE_BACK_POS: (f32, f32) = (120.0, VH - 220.0);
pub const PAGE_BACK_SIZE: (f32, f32) = (380.0, 110.0);

/// The text on every back pill (the font has no `◄`; it drew a box).
pub const PAGE_BACK_LABEL: &str = "<  BACK";

/// Glass accents.
/// (Named for its first colour; the menu went blue.)
pub const GLASS_LILAC: [u8; 3] = [120, 175, 255];
pub const GLASS_GOLD: [u8; 3] = [240, 200, 110];
pub const GLASS_ROSE: [u8; 3] = [255, 120, 150];

/// A panel of violet glass: translucent fill with a soft top sheen, a thin
/// border in `accent`. `strong` is denser (buttons, dialogs). Cached by its
/// arguments, so a page asking twice gets the same texture.
pub fn glass(w: f32, h: f32, accent: [u8; 3], strong: bool) -> Image {
    static CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<(u32, u32, [u8; 3], bool), Arc<image::RgbaImage>>>> = OnceLock::new();
    // Rasterised at most 1/2 size and stretched: it is flat colour and lines.
    let (tw, th) = (((w * 0.5) as u32).max(8), ((h * 0.5) as u32).max(8));
    let key = (tw, th, accent, strong);
    let cache = CACHE.get_or_init(Default::default);
    let img = cache.lock().unwrap().entry(key).or_insert_with(|| {
        let fill: [f32; 4] = if strong { [22.0, 40.0, 92.0, 215.0] } else { [12.0, 20.0, 48.0, 168.0] };
        let tint = |k: f32| [0, 1, 2].map(|j| fill[j] * (1.0 - k) + accent[j] as f32 * k);
        Arc::new(image::RgbaImage::from_fn(tw, th, |x, y| {
            let edge = x.min(tw - 1 - x).min(y).min(th - 1 - y);
            if edge < 2 {
                return image::Rgba([accent[0], accent[1], accent[2], if strong { 235 } else { 170 }]);
            }
            let fy = y as f32 / th as f32;
            // A faint sheen of the accent along the top, fading down.
            let c = tint(0.10 * (1.0 - smooth(0.0, 0.5, fy)));
            image::Rgba([c[0] as u8, c[1] as u8, c[2] as u8, fill[3] as u8])
        }))
    }).clone();
    Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img, color: None }
}

/// A glass panel object.
pub fn glass_obj(ctx: &mut Context, id: &str, size: (f32, f32), pos: (f32, f32), accent: [u8; 3], strong: bool, tags: Vec<String>) -> GameObject {
    GameObject::new_rect(ctx, id.to_string().into(), Some(glass(size.0, size.1, accent, strong)),
        size, pos, tags, (0.0, 0.0), (1.0, 1.0), 0.0)
}

/// The back pill every page uses, bottom-left.
pub fn page_back_btn(ctx: &mut Context, id: &str) -> GameObject {
    glass_obj(ctx, id, PAGE_BACK_SIZE, PAGE_BACK_POS, GLASS_LILAC, false, vec!["ui".into(), "button".into()])
}

/// Its label object (text set by the page, `PAGE_BACK_LABEL`).
pub fn page_back_text(ctx: &mut Context, id: &str) -> GameObject {
    GameObject::build(id).size(PAGE_BACK_SIZE.0, PAGE_BACK_SIZE.1)
        .position(PAGE_BACK_POS.0, PAGE_BACK_POS.1 + (PAGE_BACK_SIZE.1 - 36.0) * 0.5)
        .tag("ui").build(ctx)
}

/// Add the page backdrop (sky, stars, horizon) and a glowing `title` to a
/// scene. Call it FIRST, so all of it draws behind the page's content.
pub fn with_page_look(ctx: &mut Context, mut scene: Scene, title: &str) -> Scene {
    // The menu's slice of the one tall sky, so the colours match exactly.
    let (top, bot) = sky_span();
    let mut objs = vec![quad(ctx, "page_sky", sky_image(), (VW, bot - top), (0.0, top - MENU_Y), vec![])];
    for (id, y) in [("page_stars_far", 0.0), ("page_stars_far_up", -VH)] {
        objs.push(quad(ctx, id, static_stars(), (VW, VH), (0.0, y), vec![]));
    }
    for (id, y) in [("page_stars_a", 0.0), ("page_stars_b", 0.0), ("page_stars_c", -VH), ("page_stars_d", -VH)] {
        objs.push(quad(ctx, id, moving_stars(), (VW, VH), (0.0, y), vec![]));
    }
    objs.push(quad(ctx, "page_horizon", horizon_image(), (VW, VH * 0.4), (0.0, VH * 0.6), vec![]));
    objs.push(quad(ctx, "page_floor", horizon_floor(), (VW, SKY_BLEED), (0.0, VH), vec![]));
    if let Some(t) = glow_text(title, PAGE_TITLE_PX) {
        let (w, h) = (t.width() as f32, t.height() as f32);
        objs.push(quad(ctx, "page_title", t, (w, h), ((VW - w) * 0.5, PAGE_TITLE_Y), vec!["ui".into()]));
    }
    for (id, o) in objs { scene = scene.with_object(&id, o); }
    scene
}

/// Drift the pages' stars. Registered once; runs only on the page scenes.
pub fn register_page_tick(c: &mut Canvas) {
    if c.get_bool("page_tick_registered") { return; }
    c.set_var("page_tick_registered", true);
    c.on_update(|c| {
        if !PAGE_SCENES.iter().any(|s| c.is_scene(s)) { return; }
        let t = c.get_i32("page_t") + 1;
        c.set_var("page_t", t);
        let x = -((t as f32 * 0.35).rem_euclid(VW));
        for (a, b) in [("page_stars_a", "page_stars_b"), ("page_stars_c", "page_stars_d")] {
            if let Some(o) = c.get_game_object_mut(a) { o.position.0 = x.round(); }
            if let Some(o) = c.get_game_object_mut(b) { o.position.0 = (x + VW).round(); }
        }
    });
}

// ── Launch (PLAY) and arrival ────────────────────────────────────────────────
//
// PLAY: the menu takes itself apart — the logo lifts away, the list slides
// out left, the corner text drops — while the camera dives into the swinging
// cat. At the bottom of the dive the run loads, opening zoomed in on the
// player and pulling out (`GAME_INTRO_*`), so the run grows out of the menu.
//
// Arriving on the menu, the same pieces slide IN, after the shutter opens.

/// Ticks of the launch: the draw-back (the sling pulled), then the release
/// into the dive; and of the arrival (after its delay).
/// The pull (easing out, slowing into full stretch) and the hold at full
/// stretch; the release runs from there to `LAUNCH_TICKS`.
const PULL_TICKS: i32 = 30;
const DRAW_TICKS: i32 = PULL_TICKS + 12;
const LAUNCH_TICKS: i32 = DRAW_TICKS + 38;
/// The draw-back: the zoom the camera eases OUT to before the dive.
const DRAW_ZOOM: f32 = 0.84;

/// The backdrop's screen-wide layers, stretched so a zoomed-out view (the
/// sling's draw-back) never sees past their edges.
fn widen_backdrop(c: &mut Canvas) {
    let extra = VW * 0.3;
    for id in ["menu_horizon", "menu_floor", "menu_stars_far", "menu_stars_far_up"] {
        if let Some(o) = c.get_game_object_mut(id) {
            o.position.0 = -extra;
            o.size.0 = VW + extra * 2.0;
            o.update_image_shape();
        }
    }
    // And the cloud layer reaches down past the screen's bottom edge, so the
    // flat fill below it (a plain band) never shows while zoomed out.
    if let Some(o) = c.get_game_object_mut("menu_horizon") {
        o.size.1 += VH * 0.14;
        o.update_image_shape();
    }
    // The fill is drawn over the clouds, so it has to go for them to show.
    if let Some(o) = c.get_game_object_mut("menu_floor") { o.visible = false; }
}
const ARRIVE_DELAY: i32 = 22;
const ARRIVE_TICKS: i32 = 34;
/// How far the camera dives (zoom at the bottom).
///
/// Kept modest on purpose: the menu's backdrop layers are single quads as
/// wide as the screen, and a quad drawn larger than the GPU's texture limit
/// (8192 px on desktop, 4096 on a phone) is lost — at 2.8x the sky and the
/// horizon dropped out entirely. 1.7x keeps a screen-wide layer under 4096 px
/// on a phone. The run's own opening (`GAME_INTRO_ZOOM`) carries the rest of
/// the dive.
const LAUNCH_ZOOM: f32 = 1.7;

/// The UI pieces that move, and which way they leave: (id, dx, dy).
fn ui_moves() -> Vec<(String, f32, f32)> {
    let mut v: Vec<(String, f32, f32)> = vec![
        ("menu_logo".into(), 0.0, -1100.0), ("menu_sub_text".into(), 0.0, -1100.0),
        ("menu_sel_bar".into(), -1700.0, 0.0), ("menu_sel_marker".into(), -1700.0, 0.0),
        ("menu_mode_name_text".into(), -2200.0, 0.0), ("menu_mode_desc_text".into(), -2200.0, 0.0),
        ("menu_mode_selector".into(), -2200.0, 0.0), ("menu_hint_text".into(), 0.0, 600.0),
    ];
    for (k, (hit, text, _)) in items().into_iter().enumerate() {
        // Staggered: lower items travel further, so they peel off in turn.
        let dx = -1500.0 - 180.0 * k as f32;
        v.push((hit.into(), dx, 0.0));
        v.push((text.into(), dx, 0.0));
    }
    for (hit, text, _, _) in LINKS { v.push((hit.into(), 0.0, 600.0)); v.push((text.into(), 0.0, 600.0)); }
    v
}

/// Remember every moving piece's resting place, once per visit.
fn take_bases(c: &mut Canvas) {
    if c.get_bool("menu_ui_base_taken") { return; }
    c.set_var("menu_ui_base_taken", true);
    for (id, _, _) in ui_moves() {
        let p = c.get_game_object(&id).map(|o| o.position).unwrap_or((0.0, 0.0));
        c.set_var(&format!("mb_x_{id}"), p.0);
        c.set_var(&format!("mb_y_{id}"), p.1);
    }
}

/// Every piece `amt` of the way out (0 home, 1 gone).
fn place_ui(c: &mut Canvas, amt: f32) {
    for (id, dx, dy) in ui_moves() {
        let (bx, by) = (c.get_f32(&format!("mb_x_{id}")), c.get_f32(&format!("mb_y_{id}")));
        if let Some(o) = c.get_game_object_mut(&id) { o.position = (bx + dx * amt, by + dy * amt); }
    }
}

/// Run the launch or the arrival, if either is on. True while one is (the
/// normal hover/marker logic waits).
pub fn tick_launch(c: &mut Canvas) -> bool {
    let launch = c.get_i32("menu_launch_t");
    if launch > 0 {
        take_bases(c);
        let t = launch;
        c.set_var("menu_launch_t", t + 1);
        let mid = (VW * 0.5, MENU_Y + VH * 0.5);
        // A sling: first DRAWN BACK — the camera eases OUT, so the whole menu
        // shrinks away — then RELEASED: the menu flies off and the camera
        // dives into the cat. The backdrop layers are widened for the
        // zoom-out (`widen_backdrop`), so its edges never show.
        if t == 1 {
            widen_backdrop(c);
            // PLAY pressed while the menu was still sliding in: that stops.
            c.set_var("menu_intro_t", 0i32);
        }
        let (zoom, centre) = if t <= DRAW_TICKS {
            // Easing OUT into full stretch: quick at first, slowing as it
            // nears the top, then held there — the sling straining.
            let k = (t as f32 / PULL_TICKS as f32).min(1.0);
            let d = 1.0 - (1.0 - k).powi(3);
            // Everything at home while the view eases out.
            place_ui(c, 0.0);
            (1.0 + (DRAW_ZOOM - 1.0) * d, mid)
        } else {
            let r = ((t - DRAW_TICKS) as f32 / (LAUNCH_TICKS - DRAW_TICKS) as f32).min(1.0);
            // The UI leaves fast; the camera accelerates into the dive.
            let out = 1.0 - (1.0 - (r * 1.6).min(1.0)).powi(3);
            place_ui(c, out);
            // Dive to where the cat WILL BE when the dive ends, fixed at the
            // release — not to where it is each frame. Chasing the swinging
            // cat made the camera keep correcting its aim mid-dive; this way
            // it flies one smooth path and the cat swings into the spot.
            if t == DRAW_TICKS + 1 {
                let end_t = (c.get_i32("menu_t") + (LAUNCH_TICKS - DRAW_TICKS)) as f32;
                let th = SWING_AMP * (end_t * SWING_RATE).sin();
                let (nx, ny) = (NODE_POS.0, MENU_Y + NODE_POS.1);
                c.set_var("menu_dive_x", nx + ROPE_LEN * th.sin());
                c.set_var("menu_dive_y", ny + ROPE_LEN * th.cos());
            }
            let cat = (c.get_f32("menu_dive_x"), c.get_f32("menu_dive_y"));
            let dive = r * r * r;
            let f = smooth(0.0, 1.0, r);
            (DRAW_ZOOM + (LAUNCH_ZOOM - DRAW_ZOOM) * dive, (mid.0 + (cat.0 - mid.0) * f, mid.1 + (cat.1 - mid.1) * f))
        };
        // The release: a little kick as the sling lets go.
        if t == DRAW_TICKS + 1 {
            if let Some(cam) = c.camera_mut() { cam.shake(10.0, 0.15); }
        }
        if let Some(cam) = c.camera_mut() {
            cam.zoom = zoom;
            cam.zoom_target = zoom;
            cam.zoom_anchor = None;
            // Kept inside the sky: the cat is off to the right, and centring
            // on it before the zoom is deep enough ran the view past the
            // sky's edge.
            let (vw, vh) = (VW / zoom, VH / zoom);
            let (top, bot) = sky_span();
            // Zoomed out the view is wider than the menu: centre it instead.
            let x = if vw >= VW { (VW - vw) * 0.5 } else { (centre.0 - vw * 0.5).clamp(0.0, VW - vw) };
            cam.position = (x, (centre.1 - vh * 0.5).clamp(top, bot - vh));
        }
        // The sky is the one layer far taller than the screen (all of the
        // shop's sky above the menu), so it would hit that limit first: while
        // diving it covers only the view, with that band's colours.
        if let Some((cx, cy)) = c.camera().map(|cam| cam.position) {
            let (vw, vh) = (VW / zoom, VH / zoom);
            let (y0, y1) = (cy - 100.0, cy + vh + 100.0);
            if let Some(o) = c.get_game_object_mut("menu_bg") {
                o.position = (cx - 100.0, y0);
                o.size = (vw + 200.0, y1 - y0);
                o.set_image(Image { shape: ShapeType::Rectangle(0.0, (vw + 200.0, y1 - y0), 0.0), image: sky_band(y0, y1, 64), color: None });
            }
        }
        if t >= LAUNCH_TICKS {
            c.run(Action::Custom { name: "start_game_now".into() });
        }
        return true;
    }
    let intro = c.get_i32("menu_intro_t");
    if intro > 0 {
        take_bases(c);
        c.set_var("menu_intro_t", intro + 1);
        let k = ((intro - ARRIVE_DELAY) as f32 / ARRIVE_TICKS as f32).clamp(0.0, 1.0);
        let e = 1.0 - (1.0 - k).powi(3);
        place_ui(c, 1.0 - e);
        if k >= 1.0 { c.set_var("menu_intro_t", 0i32); }
        return true;
    }
    false
}

/// Whether a world point is on the menu's planet (its disc, not the empty
/// corners of its square).
pub fn planet_hit(c: &Canvas, world: (f32, f32)) -> bool {
    let Some(o) = c.get_game_object("menu_planet") else { return false };
    if !o.visible { return false; }
    let (cx, cy) = (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5);
    let r = o.size.0 * 0.42;
    (world.0 - cx).powi(2) + (world.1 - cy).powi(2) <= r * r
}
