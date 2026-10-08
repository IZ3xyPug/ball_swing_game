use quartz::*;
use image::AnimationDecoder;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};
use crate::constants::*;
use crate::images::*;
use crate::objects::ui_text_spec;

// ── Character data ──────────────────────────────────────────────────────────
pub const SHOP_CHARS: &[(u8, u8, u8)] = PLAYER_CHAR_COLORS;
pub const NUM_CHARS:  usize = SHOP_CHARS.len();

// ── Card geometry ───────────────────────────────────────────────────────────
pub const CARD_W: u32  = 460;
pub const CARD_H: u32  = 720;
pub const CARD_SPACING: f32 = 620.0;
pub const CARD_CENTER_Y: f32 = 900.0;   // y in shop viewport (shop at y=0..VH)

pub const NUM_SLOTS:   usize = 7;
pub const SLOT_CENTER: usize = 3;

// ── Category data ───────────────────────────────────────────────────────────
pub const SHOP_CAT_NAMES:     &[&str]          = &["CHARACTERS", "ROPES", "BACKGROUNDS", "TRAILS", "UPGRADES", "ABILITIES"];
pub const SHOP_CAT_SUBTITLES: &[&str]          = &["CHANGE YOUR BALL", "STYLE YOUR SWING", "CHANGE THE SCENE", "LEAVE YOUR MARK", "SPEND META \u{2022} PERMANENT", "ACTIVE POWERS \u{2022} TAP TO USE"];
pub const SHOP_CAT_COLORS:    &[(u8, u8, u8)]  = &[
    ( 60,  80, 180), // 0 Characters  — indigo
    ( 20, 150, 140), // 1 Ropes       — teal
    ( 90,  40, 160), // 2 Backgrounds — purple
    (180,  80,  20), // 3 Trails      — orange
    (140,  30, 100), // 4 Purchasable — magenta
    (200, 110,  30), // 5 Abilities   — amber
];
pub const NUM_CATS: usize = SHOP_CAT_NAMES.len();

/// Per-category item colours (placeholder swatches for non-character categories).
pub const SHOP_ROPE_COLORS: &[(u8,u8,u8)] = &[
    (220, 220, 220), // 0 white
    (180, 140,  80), // 1 classic
    ( 60, 200, 220), // 2 cyan
    (240, 200,  60), // 3 gold
    (160, 220, 100), // 4 lime
    (255,  90, 150), // 5 rose
    (170, 110, 255), // 6 violet
    (255, 120,  40), // 7 ember
    (160, 225, 255), // 8 ice
    (225,  40,  60), // 9 crimson
];
/// Colour 0 is each style's own look (the energy rope's blue, each premium
/// style's signature colour); the rest are gradient-mapped onto the style.
pub const SHOP_ROPE_NAMES: &[&str] = &["ORIGINAL", "CLASSIC", "CYAN", "GOLD", "LIME", "ROSE", "VIOLET", "EMBER", "ICE", "CRIMSON"];

pub const SHOP_BG_COLORS: &[(u8,u8,u8)] = &[
    ( 20,  15,  50), // 0 midnight
    ( 15,  70,  35), // 1 forest
    ( 10,  40,  90), // 2 ocean
    (120,  80,  25), // 3 desert
    ( 55,  20,  90), // 4 nebula
];
pub const SHOP_BG_NAMES: &[&str] = &["MIDNIGHT", "FOREST", "OCEAN", "DESERT", "NEBULA"];

pub const SHOP_TRAIL_COLORS: &[(u8,u8,u8)] = &[
    (255, 255, 255), // 0 white
    (240, 100,  20), // 1 fire
    ( 80, 200, 255), // 2 ice
    (220, 190,  30), // 3 gold
    (200,  60, 220), // 4 mystic
];
pub const SHOP_TRAIL_NAMES: &[&str] = &["WHITE", "FIRE", "ICE", "GOLD", "MYSTIC"];

/// Returns (colors, names) slices for the given category index.
/// Card colours and names for the permanent-upgrade category, built once from
/// `profile::PERM_UPGRADES` so the shop can never drift from the table that
/// actually governs cost and effect.
fn upgrade_cards() -> (&'static [(u8, u8, u8)], &'static [&'static str]) {
    static COLORS: OnceLock<Vec<(u8, u8, u8)>> = OnceLock::new();
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    (
        COLORS.get_or_init(|| crate::profile::PERM_UPGRADES.iter().map(|u| u.color).collect()),
        NAMES.get_or_init(|| crate::profile::PERM_UPGRADES.iter().map(|u| u.name).collect()),
    )
}

/// Card colours and names for the abilities, from `profile::ABILITIES`.
fn ability_cards() -> (&'static [(u8, u8, u8)], &'static [&'static str]) {
    static COLORS: OnceLock<Vec<(u8, u8, u8)>> = OnceLock::new();
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    (
        COLORS.get_or_init(|| crate::profile::ABILITIES.iter().map(|a| a.color).collect()),
        NAMES.get_or_init(|| crate::profile::ABILITIES.iter().map(|a| a.name).collect()),
    )
}

/// The abilities category: bought, then equipped, then upgraded.
#[inline]
pub fn is_ability_category(cat: i32) -> bool { cat == 5 }

fn cat_items(cat: i32) -> (&'static [(u8,u8,u8)], &'static [&'static str]) {
    match cat {
        0 => (PLAYER_CHAR_COLORS, PLAYER_CHAR_NAMES),
        // Rope STYLES; the colour is its own row under the carousel.
        1 => (crate::cosmetics::ROPE_STYLE_SWATCHES, crate::cosmetics::ROPE_STYLE_NAMES),
        2 => (SHOP_BG_COLORS,     SHOP_BG_NAMES),
        3 => (SHOP_TRAIL_COLORS,  SHOP_TRAIL_NAMES),
        4 => upgrade_cards(),
        5 => ability_cards(),
        _ => (PLAYER_CHAR_COLORS, PLAYER_CHAR_NAMES),
    }
}

/// True for categories that are bought rather than equipped.
#[inline]
pub fn is_upgrade_category(cat: i32) -> bool { cat == 4 }

/// Returns (x, y, w, h) for each category card button (3+3 grid layout).
fn cat_card_rect(idx: usize) -> (f32, f32, f32, f32) {
    let (col, row) = ((idx % 3) as f32, (idx / 3) as f32);
    (300.0 + col * 1100.0, 330.0 + row * 600.0, 1040.0, 520.0)
}

// ── Image cache (built once at startup) ────────────────────────────────────
static CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();
static ROPE_CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();
static BG_CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();
static TRAIL_CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();
static ABILITY_CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();
static UPGRADE_CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();

pub fn get_card_cache() -> &'static Vec<[std::sync::Arc<image::RgbaImage>; 2]> {
    CARD_CACHE.get_or_init(|| {
        (0..NUM_CHARS).map(|i| {
            // The calico and every breed show their own curled-up art; the
            // plain colours keep their orb.
            let art = if i == 0 {
                first_gif_frame(include_bytes!("../assets/calicoball.gif"))
            } else {
                crate::cosmetics::cat_skin(i)
                    .and_then(|s| image::load_from_memory(s.frames[0]).ok())
                    .map(|img| img.to_rgba8())
            };
            match art {
                Some(art) => [
                    std::sync::Arc::new(art_card_img(&art, 2, false)),
                    std::sync::Arc::new(art_card_img(&art, 2, true)),
                ],
                None => {
                    let (r, g, b) = SHOP_CHARS[i];
                    [
                        std::sync::Arc::new(shop_card_img(r, g, b, false)),
                        std::sync::Arc::new(shop_card_img(r, g, b, true)),
                    ]
                }
            }
        }).collect()
    })
}

fn first_gif_frame(bytes: &[u8]) -> Option<image::RgbaImage> {
    let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    decoder.into_frames().next()?.ok().map(|f| f.into_buffer())
}

/// A rope style's card art: its first tile, three deep, mirrored the way
/// the rope draws it; the classic energy rope shows its own GIF frame.
fn rope_style_art(style: usize) -> Option<image::RgbaImage> {
    if style != 0 {
        let s = crate::cosmetics::ROPE_STYLES.get(style)?;
        return crate::cosmetics::rope_card_art(style, s.signature);
    }
    let tile = first_gif_frame(include_bytes!("../assets/energy_hook_1.gif"))?;
    let (w, h) = tile.dimensions();
    let n = 2;
    let mut strip = image::RgbaImage::new(w, h * n);
    for k in 0..n {
        let t = if k % 2 == 1 { image::imageops::flip_vertical(&tile) } else { tile.clone() };
        image::imageops::overlay(&mut strip, &t, 0, (k * h) as i64);
    }
    Some(strip)
}

fn get_item_card_cache(cat: i32) -> Option<&'static Vec<[std::sync::Arc<image::RgbaImage>; 2]>> {
    let make_cache = |colors: &'static [(u8, u8, u8)]| {
        (0..colors.len()).map(|i| {
            let (r, g, b) = colors[i];
            [
                std::sync::Arc::new(shop_card_img(r, g, b, false)),
                std::sync::Arc::new(shop_card_img(r, g, b, true)),
            ]
        }).collect::<Vec<_>>()
    };

    match cat {
        1 => Some(ROPE_CARD_CACHE.get_or_init(|| {
            (0..crate::cosmetics::ROPE_STYLES.len()).map(|i| {
                match rope_style_art(i) {
                    Some(art) => {
                        let k = 3;
                        [
                            std::sync::Arc::new(art_card_img(&art, k, false)),
                            std::sync::Arc::new(art_card_img(&art, k, true)),
                        ]
                    }
                    None => {
                        let (r, g, b) = crate::cosmetics::ROPE_STYLE_SWATCHES[i];
                        [
                            std::sync::Arc::new(shop_card_img(r, g, b, false)),
                            std::sync::Arc::new(shop_card_img(r, g, b, true)),
                        ]
                    }
                }
            }).collect()
        })),
        // Upgrades: a plain card; the animated emblem rides on top of it
        // (`shop_slot_icon_*`, see `tick_slot_icons`).
        4 => Some(UPGRADE_CARD_CACHE.get_or_init(|| {
            let blank = image::RgbaImage::new(1, 1);
            (0..crate::profile::PERM_UPGRADES.len()).map(|_| [
                std::sync::Arc::new(art_card_img(&blank, 1, false)),
                std::sync::Arc::new(art_card_img(&blank, 1, true)),
            ]).collect()
        })),
        2 => Some(BG_CARD_CACHE.get_or_init(|| make_cache(SHOP_BG_COLORS))),
        3 => Some(TRAIL_CARD_CACHE.get_or_init(|| make_cache(SHOP_TRAIL_COLORS))),
        // Abilities: the orb in the ability's colour, with its in-run icon on it.
        // Abilities: a plain card; the animated emblem rides on top of it.
        5 => Some(ABILITY_CARD_CACHE.get_or_init(|| {
            let blank = image::RgbaImage::new(1, 1);
            (0..crate::profile::ABILITIES.len()).map(|_| [
                std::sync::Arc::new(art_card_img(&blank, 1, false)),
                std::sync::Arc::new(art_card_img(&blank, 1, true)),
            ]).collect()
        })),
        _ => None,
    }
}

// ── Card image builder ──────────────────────────────────────────────────────

/// A card carrying pixel art (upscaled by `scale`, nearest) where the orb
/// would be: cats curled up, ropes as a strip of their tile.
fn art_card_img(art: &image::RgbaImage, scale: u32, selected: bool) -> image::RgbaImage {
    let mut img = shop_card_img(0, 0, 0, selected);
    // Clear the orb shop_card_img drew; this card's art replaces it.
    let (bgr, bgg, bgb, bga) = card_fill(selected);
    let (cx, cy, cr) = ((CARD_W / 2) as i32, (CARD_H / 2) as i32 - 60, 121i32);
    for py in (cy - cr).max(0)..(cy + cr).min(CARD_H as i32) {
        for px in (cx - cr).max(0)..(cx + cr).min(CARD_W as i32) {
            let (dx, dy) = (px - cx, py - cy);
            if dx * dx + dy * dy <= cr * cr {
                img.put_pixel(px as u32, py as u32, image::Rgba([bgr, bgg, bgb, bga]));
            }
        }
    }
    let (w, h) = art.dimensions();
    let big = image::imageops::resize(art, w * scale, h * scale, image::imageops::FilterType::Nearest);
    let ox = cx - (w * scale / 2) as i32;
    let oy = (cy - (h * scale / 2) as i32).max(12);
    image::imageops::overlay(&mut img, &big, ox as i64, oy as i64);
    img
}

fn shop_card_img(r: u8, g: u8, b: u8, selected: bool) -> image::RgbaImage {
    let w = CARD_W;
    let h = CARD_H;
    let mut img = image::RgbaImage::new(w, h);

    let (br, bg_v, bb, ba) = card_fill(selected);
    for py in 0..h { for px in 0..w {
        img.put_pixel(px, py, image::Rgba([br, bg_v, bb, ba]));
    }}

    let bw    = if selected { 6u32 } else { 3u32 };
    let bcolr = if selected { [255u8, 255, 255, 255] } else { [150u8, 128, 220, 150] };
    draw_rect(&mut img, 0, 0, w, bw, bcolr);
    draw_rect(&mut img, 0, h - bw, w, bw, bcolr);
    draw_rect(&mut img, 0, 0, bw, h, bcolr);
    draw_rect(&mut img, w - bw, 0, bw, h, bcolr);

    let cr: i32 = 120;
    let cx: i32 = (w / 2) as i32;
    let cy: i32 = (h / 2) as i32 - 60;
    for py in 0..h { for px in 0..w {
        let dx = px as i32 - cx;
        let dy = py as i32 - cy;
        if dx * dx + dy * dy <= cr * cr {
            let norm = ((dx * dx + dy * dy) as f32).sqrt() / cr as f32;
            let bright = 1.0 + 0.45 * (1.0 - norm);
            let pr = (r as f32 * bright).min(255.0) as u8;
            let pg = (g as f32 * bright).min(255.0) as u8;
            let pb = (b as f32 * bright).min(255.0) as u8;
            img.put_pixel(px, py, image::Rgba([pr, pg, pb, 255]));
        }
    }}

    let div_y = ((h as f32) * 0.74) as u32;
    draw_rect(&mut img, 12, div_y, w - 24, 2, [160, 140, 220, 120]);
    img
}

/// A carousel card's fill: violet glass over the sky, a touch lighter and
/// denser when it is the one in the middle.
fn card_fill(selected: bool) -> (u8, u8, u8, u8) {
    if selected { (52, 40, 100, 205) } else { (22, 16, 48, 165) }
}

/// Add a wide glow round whatever was drawn since `before`, in the left
/// `zone_w` px of the card.
fn bloom_icon(img: &mut image::RgbaImage, before: &image::RgbaImage, rgb: (u8, u8, u8), zone_w: usize) {
    let (w, h) = (zone_w, img.height() as usize);
    let mut m = vec![0f32; w * h];
    for y in 0..h { for x in 0..w {
        let (a, b) = (img.get_pixel(x as u32, y as u32), before.get_pixel(x as u32, y as u32));
        let d = (0..3).map(|i| (a[i] as f32 - b[i] as f32).abs()).fold(0.0f32, f32::max) / 255.0;
        // The ring is drawn before the snapshot; include bright pixels too.
        let lum = (a[0] as f32 + a[1] as f32 + a[2] as f32) / (3.0 * 255.0);
        m[y * w + x] = d.max(((lum - 0.45) * 2.0).clamp(0.0, 1.0));
    }}
    let blur = |m: &mut Vec<f32>, r: usize| {
        let mut t = vec![0f32; m.len()];
        for y in 0..h { for x in 0..w {
            let (lo, hi) = (x.saturating_sub(r), (x + r).min(w - 1));
            t[y * w + x] = m[y * w + lo..=y * w + hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
        }}
        for y in 0..h { for x in 0..w {
            let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
            m[y * w + x] = (lo..=hi).map(|yy| t[yy * w + x]).sum::<f32>() / (hi - lo + 1) as f32;
        }}
    };
    let mut wide = m.clone();
    for _ in 0..3 { blur(&mut wide, 14); }
    let mut tight = m;
    for _ in 0..2 { blur(&mut tight, 4); }
    for y in 0..h { for x in 0..w {
        let g = (wide[y * w + x] * 4.0 + tight[y * w + x] * 1.3).min(1.8);
        if g <= 0.01 { continue; }
        let p = img.get_pixel_mut(x as u32, y as u32);
        let add = |c: u8, base: u8| (base as f32 + c as f32 * g * 0.75 + 60.0 * g * g * 0.25).min(255.0) as u8;
        *p = image::Rgba([add(rgb.0, p[0]), add(rgb.1, p[1]), add(rgb.2, p[2]), p[3]]);
    }}
}

/// The shop's RGB text: each letter its own hue, the rainbow flowing along
/// the word over time (`t` in seconds).
pub fn rgb_text_spec(text: &str, font: &Font, size: f32, width: f32, t: f32) -> Text {
    let spans = text.chars().enumerate().map(|(i, ch)| {
        let hue = (t * 0.25 + i as f32 * 0.07).rem_euclid(1.0);
        let (r, g, b) = hsv(hue, 0.55, 1.0);
        Span::new(ch.to_string(), size, Some(size * 1.25), std::sync::Arc::new(font.clone()),
            Color(r, g, b, 255), 0.0)
    }).collect();
    Text::new(spans, Some(width), Align::Center, None)
}

fn hsv(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - f * s), v * (1.0 - (1.0 - f) * s));
    let (r, g, b) = match i as i32 % 6 { 0 => (v, t, p), 1 => (q, v, p), 2 => (p, v, t), 3 => (p, q, v), 4 => (t, p, v), _ => (v, p, q) };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Every few frames while the shop is up: the title and the category names
/// cycle their RGB.
pub fn tick_rgb_text(c: &mut Canvas) {
    let f = c.get_i32("shop_rgb_t") + 1;
    c.set_var("shop_rgb_t", f);
    if f % 3 != 0 { return; }
    let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) else { return };
    let t = f as f32 / 60.0;
    let s = c.virtual_scale();
    let title = match c.get_var("shop_title_now") { Some(Value::Str(v)) => v, _ => "SHOP".to_string() };
    if let Some(obj) = c.get_game_object_mut("shop_title_text") {
        if obj.visible { obj.set_drawable(Box::new(rgb_text_spec(&title, &font, 110.0 * s, 1600.0 * s, t))); }
    }
    if c.get_i32("shop_screen") == 0 {
        for i in 0..NUM_CATS {
            let (_, _, cw, _) = cat_card_rect(i);
            if let Some(obj) = c.get_game_object_mut(&format!("shop_cat_label_{i}")) {
                if obj.visible {
                    obj.set_drawable(Box::new(rgb_text_spec(SHOP_CAT_NAMES[i], &font, 58.0 * s, cw * 0.44 * s, t + i as f32 * 0.4)));
                }
            }
        }
    }
}

/// How opaque the category cards are: glass, so the sky reads through them.
const CAT_CARD_ALPHA: u8 = 175;

/// Build a category card image (w×h) with a gradient, colored border,
/// a glowing orb on the left, a unique icon per category,
/// and a darker translucent band at the bottom for the text labels.
fn shop_cat_card_img(idx: usize, w: u32, h: u32) -> image::RgbaImage {
    let (cr, cg, cb) = SHOP_CAT_COLORS[idx];
    let mut img = image::RgbaImage::new(w, h);

    // Gradient background (dark navy → slightly lighter)
    for py in 0..h {
        let t = py as f32 / h as f32;
        let br = (18.0 + 14.0 * t) as u8;
        let bg = (13.0 + 10.0 * t) as u8;
        let bb = (40.0 + 22.0 * t) as u8;
        for px in 0..w {
            img.put_pixel(px, py, image::Rgba([br, bg, bb, CAT_CARD_ALPHA]));
        }
    }

    // Glow orb in the left quarter of the card
    let glow_cx = (w / 4) as i32;
    let glow_cy = (h / 2) as i32;
    let glow_r  = (h as f32 * 0.38) as i32;
    for py in 0..h {
        for px in 0..w {
            let dx = px as i32 - glow_cx;
            let dy = py as i32 - glow_cy;
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            let t = (1.0 - dist / glow_r as f32).clamp(0.0, 1.0);
            if t > 0.0 {
                let p = img.get_pixel(px, py);
                let t = t * t * (3.0 - 2.0 * t);
                let nr = (p[0] as f32 + cr as f32 * t * 0.85).min(255.0) as u8;
                let ng = (p[1] as f32 + cg as f32 * t * 0.85).min(255.0) as u8;
                let nb = (p[2] as f32 + cb as f32 * t * 0.85).min(255.0) as u8;
                img.put_pixel(px, py, image::Rgba([nr, ng, nb, CAT_CARD_ALPHA]));
            }
        }
    }

    // Ring outline on the orb
    let ring_r = glow_r as f32 * 0.68;
    for py in 0..h {
        for px in 0..w {
            let dx = px as i32 - glow_cx;
            let dy = py as i32 - glow_cy;
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            if (dist - ring_r).abs() < 3.5 {
                let bright_r = (cr as f32 * 1.5).min(255.0) as u8;
                let bright_g = (cg as f32 * 1.5).min(255.0) as u8;
                let bright_b = (cb as f32 * 1.5).min(255.0) as u8;
                img.put_pixel(px, py, image::Rgba([bright_r, bright_g, bright_b, 210]));
            }
        }
    }

    // Unique icon per category (bright pixel-art in the orb center). A copy
    // of the card from before it is drawn, so its strokes can be found again
    // for the bloom below.
    let before_icon = img.clone();
    let icon_s = ((glow_r as f32 * 0.30) as u32).max(6);
    let ic = [
        (cr as f32 * 1.7).min(255.0) as u8,
        (cg as f32 * 1.7).min(255.0) as u8,
        (cb as f32 * 1.7).min(255.0) as u8,
        255,
    ];
    match idx {
        0 => {
            // Characters: filled circle
            for py in 0..h { for px in 0..w {
                let dx = px as i32 - glow_cx;
                let dy = py as i32 - glow_cy;
                if dx * dx + dy * dy <= (icon_s as i32) * (icon_s as i32) {
                    img.put_pixel(px, py, image::Rgba(ic));
                }
            }}
        }
        1 => {
            // Ropes: crosshatch (+)
            let half = icon_s as i32;
            draw_rect(&mut img,
                (glow_cx - half) as u32, (glow_cy - 5) as u32,
                (half * 2) as u32, 10, ic);
            draw_rect(&mut img,
                (glow_cx - 5) as u32, (glow_cy - half) as u32,
                10, (half * 2) as u32, ic);
        }
        2 => {
            // Backgrounds: landscape icon — shifted down for visual centering
            let half = icon_s as i32;
            let vert_off = half / 3;
            draw_rect(&mut img,
                (glow_cx - half * 2) as u32, (glow_cy - half + vert_off) as u32,
                (half * 4) as u32, half as u32, ic);
            draw_rect(&mut img,
                (glow_cx - half * 2) as u32, (glow_cy + vert_off) as u32,
                (half * 4) as u32, 6, ic);
        }
        3 => {
            // Trails: three diagonal dots
            for i in 0..3i32 {
                let tx = glow_cx + (i - 1) * icon_s as i32 * 2;
                let ty = glow_cy + (i - 1) * icon_s as i32;
                let r  = (icon_s / 2) as i32;
                for py in 0..h { for px in 0..w {
                    let dx = px as i32 - tx;
                    let dy = py as i32 - ty;
                    if dx * dx + dy * dy <= r * r {
                        img.put_pixel(px, py, image::Rgba(ic));
                    }
                }}
            }
        }
        5 => {
            // Abilities: a lightning bolt, a zigzag of thick strokes.
            let k = icon_s as f32 * 1.4;
            let pts = [(0.35, -1.0), (-0.35, 0.08), (0.3, 0.0), (-0.3, 1.0)];
            for w in pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                let steps = 80;
                for i in 0..=steps {
                    let t = i as f32 / steps as f32;
                    let x = glow_cx as f32 + (a.0 + (b.0 - a.0) * t) * k;
                    let y = glow_cy as f32 + (a.1 + (b.1 - a.1) * t) * k;
                    draw_rect(&mut img, (x - 6.0).max(0.0) as u32, (y - 6.0).max(0.0) as u32, 12, 12, ic);
                }
            }
        }
        4 => {
            // Coins: ring + vertical bar
            let r_out = icon_s as i32;
            let r_in  = (icon_s as i32 - 6).max(0);
            for py in 0..h { for px in 0..w {
                let dx = px as i32 - glow_cx;
                let dy = py as i32 - glow_cy;
                let d2 = dx * dx + dy * dy;
                if d2 <= r_out * r_out && d2 >= r_in * r_in {
                    img.put_pixel(px, py, image::Rgba(ic));
                }
            }}
            draw_rect(&mut img,
                (glow_cx - 5) as u32, (glow_cy - r_out * 3 / 4) as u32,
                10, (r_out * 3 / 2) as u32, ic);
        }
        _ => {}
    }

    // Bloom: the icon's strokes (and the ring) blurred wide and added back
    // in the category's colour, so the icon glows like a light rather than
    // sitting flat on the card.
    bloom_icon(&mut img, &before_icon, (cr, cg, cb), (w / 2) as usize);

    // Vertical separator line between icon zone and text zone.
    // Keep original per-category styling; ropes get a subtle dark underlay so
    // their separator reads the same as other categories on this background.
    let sep_x = w / 2;
    if idx == 1 {
        draw_rect(&mut img, sep_x, 20, 2, h - 40, [0, 0, 0, 110]);
    }
    // Tinted base line.
    draw_rect(&mut img, sep_x, 20, 2, h - 40, [cr, cg, cb, 80]);
    // Bright glow highlight in the centre of the line (1px wide).
    draw_rect(&mut img, sep_x, 20, 1, h - 40, [255, 255, 255, 100]);

    // Bottom text-band: darken lower 38% for contrast behind labels
    let band_h  = h * 38 / 100;
    let band_y0 = h - band_h;
    for py in band_y0..h {
        let t = (py - band_y0) as f32 / band_h as f32;
        for px in 0..w {
            let p = img.get_pixel(px, py);
            let nr = (p[0] as f32 * (1.0 - t * 0.4)) as u8;
            let ng = (p[1] as f32 * (1.0 - t * 0.4)) as u8;
            let nb = (p[2] as f32 * (1.0 - t * 0.4)) as u8;
            img.put_pixel(px, py, image::Rgba([nr, ng, nb, CAT_CARD_ALPHA]));
        }
    }

    // Top accent stripe
    draw_rect(&mut img, 0, 0, w, 10, [cr, cg, cb, 200]);

    // Border
    let bw = 4u32;
    draw_rect(&mut img, 0, 0, w, bw, [cr, cg, cb, 220]);
    draw_rect(&mut img, 0, h - bw, w, bw, [cr, cg, cb, 220]);
    draw_rect(&mut img, 0, 0, bw, h, [cr, cg, cb, 220]);
    draw_rect(&mut img, w - bw, 0, bw, h, [cr, cg, cb, 220]);

    img
}

// Category card image cache (one image per category, keyed by index+size).
static CAT_CARD_CACHE: OnceLock<Vec<std::sync::Arc<image::RgbaImage>>> = OnceLock::new();

pub fn get_cat_card_cache() -> &'static Vec<std::sync::Arc<image::RgbaImage>> {
    CAT_CARD_CACHE.get_or_init(|| {
        (0..NUM_CATS).map(|i| {
            let (_, _, w, h) = cat_card_rect(i);
            std::sync::Arc::new(shop_cat_card_img(i, w as u32, h as u32))
        }).collect()
    })
}

// ── Category screen helpers ─────────────────────────────────────────────────

fn set_visible(c: &mut Canvas, name: &str, v: bool) {
    if let Some(obj) = c.get_game_object_mut(name) { obj.visible = v; }
}

/// Switch to the category selection screen (called from init_shop and shop_back).
pub fn show_categories(c: &mut Canvas) {
    c.set_var("shop_screen", 0i32);
    c.set_var("shop_title_now", Value::Str("SHOP".to_string()));

    for i in 0..NUM_CATS {
        set_visible(c, &format!("shop_cat_btn_{i}"), true);
        set_visible(c, &format!("shop_cat_label_{i}"), true);
        set_visible(c, &format!("shop_cat_sub_{i}"), true);
    }
    for name in ["shop_card_strip", "shop_instr_text", "shop_select_btn", "shop_select_text",
                 "shop_prev_btn", "shop_next_btn"] {
        set_visible(c, name, false);
    }
    for s in 0..NUM_SLOTS {
        set_visible(c, &format!("shop_slot_{s}"), false);
        set_visible(c, &format!("shop_slot_label_{s}"), false);
    }
    show_rope_colors(c, false);
    c.clear_effect("shop_preview_trail");

    // Update title and back button text
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        let s = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("shop_title_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "SHOP", &font, 110.0 * s, Color(255, 255, 255, 255), 1600.0 * s,
            )));
        }
        if let Some(obj) = c.get_game_object_mut("shop_back_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "\u{2193}  MENU", &font, 32.0 * s, Color(230, 222, 255, 255), 380.0 * s,
            )));
        }
    }
}

/// Switch to the carousel view for a given category index.
///
/// Category 4 (permanent upgrades) uses the SAME carousel as the cosmetic
/// categories rather than the single hardcoded "buy a heart" line it used to
/// be — it just buys on select instead of equipping.
pub fn show_carousel(c: &mut Canvas, cat: i32) {
    c.set_var("shop_screen", 1i32);
    c.set_var("shop_active_category", cat);

    // Always hide category objects
    for i in 0..NUM_CATS {
        set_visible(c, &format!("shop_cat_btn_{i}"), false);
        set_visible(c, &format!("shop_cat_label_{i}"), false);
        set_visible(c, &format!("shop_cat_sub_{i}"), false);
    }

    // All carousel categories, cosmetic and purchasable alike.
    for name in ["shop_card_strip", "shop_instr_text", "shop_select_btn", "shop_select_text",
                 "shop_prev_btn", "shop_next_btn"] {
        set_visible(c, name, true);
    }
    for s in 0..NUM_SLOTS {
        set_visible(c, &format!("shop_slot_{s}"), true);
        set_visible(c, &format!("shop_slot_label_{s}"), true);
    }

    // Reset carousel state — but initialise the selected slot to the currently
    // equipped cosmetic for this category, so the carousel reflects what the
    // player already owns instead of always jumping back to slot 0.
    let saved = match cat {
        0 => c.get_i32("player_char_selected"),
        1 => c.get_i32("player_rope_selected"),
        2 => c.get_i32("player_bg_selected"),
        3 => c.get_i32("player_trail_selected"),
        // Abilities open on the equipped one.
        5 => {
            let g = crate::profile::profile();
            let eq = g.lock().unwrap().equipped_ability.clone();
            crate::profile::ABILITIES.iter().position(|a| a.id == eq).unwrap_or(0) as i32
        }
        _ => 0,
    }.max(0);
    let (items, _names) = cat_items(cat);
    let sel = (saved as usize).min(items.len().saturating_sub(1)) as i32;
    c.set_var("shop_selected",          sel);
    c.set_var("shop_slide_offset",      0.0f32);
    c.set_var("shop_scroll_dir",        0i32);
    c.set_var("shop_scroll_held_ticks", 0i32);
    update_slot_positions(c, 0.0);
    update_all_slot_images(c, sel as usize, cat);
    update_all_slot_labels(c, sel as usize, cat);
    show_rope_colors(c, cat == 1);
    if cat != 1 {
        c.clear_effect("shop_preview_trail");
    }

    refresh_select_text(c, cat, sel as usize);

    // Update title and back button text
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        let s = c.virtual_scale();
        let cat_name = SHOP_CAT_NAMES.get(cat as usize).copied().unwrap_or("SHOP");
        c.set_var("shop_title_now", Value::Str(cat_name.to_string()));
        if let Some(obj) = c.get_game_object_mut("shop_title_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                cat_name, &font, 100.0 * s, Color(255, 255, 255, 255), 1600.0 * s,
            )));
        }
        let instr = carousel_instruction(c, cat, 0);
        if let Some(obj) = c.get_game_object_mut("shop_instr_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                &instr,
                &font, 30.0 * s, Color(150, 180, 215, 210), 1600.0 * s,
            )));
        }
        let action = if is_upgrade_category(cat) || is_ability_category(cat) { "BUY" } else { "SELECT" };
        if let Some(obj) = c.get_game_object_mut("shop_select_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                action, &font, 32.0 * s, Color(255, 255, 255, 255), 380.0 * s,
            )));
        }
        let instr = carousel_instruction(c, cat, sel as usize);
        if let Some(obj) = c.get_game_object_mut("shop_instr_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                &instr, &font, 30.0 * s, Color(150, 180, 215, 210), 1600.0 * s,
            )));
        }
        if let Some(obj) = c.get_game_object_mut("shop_back_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "\u{2039}  BACK", &font, 32.0 * s, Color(230, 222, 255, 255), 380.0 * s,
            )));
        }
    }
    refresh_select_text(c, cat, sel as usize);
}

// ── Carousel helpers ────────────────────────────────────────────────────────

#[inline]
pub fn slot_char(selected: usize, slot: usize) -> usize {
    ((selected as i64 + slot as i64 - SLOT_CENTER as i64)
        .rem_euclid(NUM_CHARS as i64)) as usize
}

pub fn update_slot_positions(c: &mut Canvas, offset: f32) {
    let w = CARD_W as f32;
    let h = CARD_H as f32;
    let card_y  = CARD_CENTER_Y - h / 2.0;
    let label_y = card_y + h * 0.76 + 10.0;
    for s in 0..NUM_SLOTS {
        let slot_x = VW / 2.0 - w / 2.0
            + (s as f32 - SLOT_CENTER as f32) * CARD_SPACING
            + offset;
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_{s}")) {
            obj.size = (w, h);
            obj.position = (slot_x, card_y);
            obj.update_image_shape();
        }
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_label_{s}")) {
            obj.position = (slot_x, label_y);
        }
        // The emblem sits where a card's orb would be.
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_icon_{s}")) {
            obj.position = (slot_x + w * 0.5 - ICON_SIZE * 0.5, card_y + h * 0.5 - 60.0 - ICON_SIZE * 0.5);
        }
    }
}

/// The animated emblems' size on a card.
const ICON_SIZE: f32 = 330.0;

/// Every frame on the carousel: each slot's emblem (upgrades and abilities
/// only), animated, in its item's colour.
pub fn tick_slot_icons(c: &mut Canvas) {
    let cat = c.get_i32("shop_active_category");
    let on = c.get_i32("shop_screen") == 1 && (is_upgrade_category(cat) || is_ability_category(cat));
    let t = c.get_i32("shop_rgb_t") as f32 / 60.0;
    let n = if is_upgrade_category(cat) { crate::profile::PERM_UPGRADES.len() } else { crate::profile::ABILITIES.len() };
    let sel = c.get_i32("shop_selected").max(0) as usize;
    for s in 0..NUM_SLOTS {
        let id = format!("shop_slot_icon_{s}");
        if !on {
            if c.get_game_object(&id).is_some_and(|o| o.visible) {
                c.clear_effect(&id);
                if let Some(o) = c.get_game_object_mut(&id) { o.visible = false; }
            }
            continue;
        }
        let idx = ((sel as i64 + s as i64 - SLOT_CENTER as i64).rem_euclid(n.max(1) as i64)) as usize;
        let (icon, rgb) = if is_upgrade_category(cat) {
            (idx as u32, crate::profile::PERM_UPGRADES[idx].color)
        } else {
            (7 + idx as u32, crate::profile::ABILITIES[idx].color)
        };
        let card_visible = c.get_game_object(&format!("shop_slot_{s}")).is_some_and(|o| o.visible);
        if let Some(o) = c.get_game_object_mut(&id) { o.visible = card_visible; }
        if !card_visible { continue; }
        // The middle card's emblem burns brighter.
        let intensity = if s == SLOT_CENTER { 1.0 } else { 0.7 };
        c.attach_effect(&id, Effect::ShopIcon { icon, time: t + idx as f32 * 0.7, intensity },
            EffectColor::srgb8(rgb.0, rgb.1, rgb.2), (ICON_SIZE, ICON_SIZE));
    }
}

pub fn update_all_slot_images(c: &mut Canvas, selected: usize, cat: i32) {
    let w = CARD_W as f32;
    let h = CARD_H as f32;
    let (colors, _) = cat_items(cat);
    let n = colors.len();
    let char_cache = if cat == 0 { Some(get_card_cache()) } else { None };
    let item_cache = get_item_card_cache(cat);
    for s in 0..NUM_SLOTS {
        let idx = ((selected as i64 + s as i64 - SLOT_CENTER as i64)
            .rem_euclid(n as i64)) as usize;
        let img: std::sync::Arc<image::RgbaImage> = if let Some(cache) = char_cache {
            cache[idx][if s == SLOT_CENTER { 1 } else { 0 }].clone()
        } else if let Some(cache) = item_cache {
            cache[idx][if s == SLOT_CENTER { 1 } else { 0 }].clone()
        } else {
            let (r, g, b) = colors[idx];
            std::sync::Arc::new(shop_card_img(r, g, b, s == SLOT_CENTER))
        };
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_{s}")) {
            obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img, color: None });
            // Keep canonical card bounds to avoid any visual size drift during rapid swaps.
            obj.size = (w, h);
            obj.update_image_shape();
        }
    }
}

pub fn update_all_slot_labels(c: &mut Canvas, selected: usize, cat: i32) {
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        let s_scale = c.virtual_scale();
        let (colors, names) = cat_items(cat);
        let n = colors.len();
        for slot in 0..NUM_SLOTS {
            let idx = ((selected as i64 + slot as i64 - SLOT_CENTER as i64)
                .rem_euclid(n as i64)) as usize;
            let name = format!("shop_slot_label_{slot}");
            // Upgrade cards carry their own state — a name alone cannot tell
            // the player whether they can afford the next rank or already own
            // every one of them.
            let (label, colour) = if is_upgrade_category(cat) {
                upgrade_slot_label(idx)
            } else if is_ability_category(cat) {
                ability_slot_label(idx)
            } else if cat == 0 || cat == 1 {
                look_slot_label(c, cat, idx)
            } else {
                (names[idx].to_string(), Color(180, 210, 240, 200))
            };
            if let Some(obj) = c.get_game_object_mut(&name) {
                obj.set_drawable(Box::new(ui_text_spec(
                    &label, &font,
                    28.0 * s_scale, colour,
                    (CARD_W as f32) * s_scale,
                )));
            }
        }
    }
}

/// Redraw the whole carousel for `selected` (cards, labels, the blurb and
/// the action button) — after anything outside the carousel moved it.
pub fn refresh_carousel(c: &mut Canvas, cat: i32, selected: usize) {
    update_all_slot_images(c, selected, cat);
    update_all_slot_labels(c, selected, cat);
    refresh_instruction(c, cat, selected);
    refresh_select_text(c, cat, selected);
    if cat == 1 {
        place_rope_color_ring(c);
    }
}

/// Refresh the instruction line for the current selection, so the blurb under
/// the cards always describes the card in the centre.
pub fn refresh_instruction(c: &mut Canvas, cat: i32, selected: usize) {
    let text = carousel_instruction(c, cat, selected);
    let scale = c.virtual_scale();
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        if let Some(obj) = c.get_game_object_mut("shop_instr_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                &text, &font, 30.0 * scale, Color(150, 180, 215, 210), 1600.0 * scale,
            )));
        }
    }
}

/// What a look (cat or rope style) costs, whether it is owned, and whether
/// it is the one equipped.
pub fn look_state(c: &Canvas, cat: i32, idx: usize) -> (String, crate::cosmetics::Price, bool, bool) {
    use crate::cosmetics::*;
    let (key, price, name) = if cat == 0 {
        (char_key(idx), char_price(idx), PLAYER_CHAR_NAMES.get(idx).copied().unwrap_or(""))
    } else {
        let s = &ROPE_STYLES[idx.min(ROPE_STYLES.len() - 1)];
        (rope_key(idx), s.price, s.name)
    };
    let equipped = if cat == 0 {
        c.get_i32("player_char_selected").max(0) as usize == idx
    } else {
        c.get_i32("player_rope_style_selected").max(0) as usize == idx
    };
    (name.to_string(), price, owns(&key, price), equipped)
}

fn look_slot_label(c: &Canvas, cat: i32, idx: usize) -> (String, Color) {
    use crate::cosmetics::Price;
    let (name, price, owned, equipped) = look_state(c, cat, idx);
    let meta = {
        let g = crate::profile::profile();
        let m = g.lock().unwrap().meta_currency;
        m
    };
    if equipped {
        (format!("{name}  \u{2022}  EQUIPPED"), Color(140, 255, 190, 230))
    } else if owned {
        (name, Color(180, 210, 240, 200))
    } else {
        match price {
            Price::Meta(n) if meta >= n => (format!("{name}  \u{2022}  {n} META"), Color(255, 236, 150, 240)),
            Price::Meta(n) => (format!("{name}  \u{2022}  {n} META"), Color(160, 175, 200, 190)),
            Price::Premium(_) => (format!("{name}  \u{2022}  SOON"), Color(160, 175, 200, 190)),
            Price::Free => (name, Color(180, 210, 240, 200)),
        }
    }
}

/// The action button says BUY for a look not owned yet.
pub fn refresh_select_text(c: &mut Canvas, cat: i32, selected: usize) {
    if cat == 1 {
        tint_original_swatch(c, selected);
    }
    let action = if is_upgrade_category(cat) {
        "BUY".to_string()
    } else if is_ability_category(cat) {
        ability_action(selected).0
    } else if cat == 0 || cat == 1 {
        let (_, price, owned, _) = look_state(c, cat, selected);
        if owned { "SELECT".to_string() } else { format!("BUY \u{2022} {}", price.label()) }
    } else {
        "SELECT".to_string()
    };
    let s = c.virtual_scale();
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        if let Some(obj) = c.get_game_object_mut("shop_select_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                &action, &font, 32.0 * s, Color(255, 255, 255, 255), 380.0 * s,
            )));
        }
    }
}

/// SELECT on a cat or rope-style card: equip it if owned, buy it first if
/// not. False (and a message on the instruction line) when it cannot be
/// bought, so the caller stays on the carousel instead of equipping.
pub fn select_look(c: &mut Canvas, cat: i32, idx: usize) -> bool {
    use crate::cosmetics::*;
    let (name, price, owned, _) = look_state(c, cat, idx);
    if owned {
        return true;
    }
    let key = if cat == 0 { char_key(idx) } else { rope_key(idx) };
    match buy(&key, price) {
        Ok(()) => {
            flash_instruction(c, &format!("UNLOCKED {name}!"));
            true
        }
        Err(short) if short == u64::MAX => {
            flash_instruction(c, &format!("{name} IS COMING SOON"));
            false
        }
        Err(short) => {
            flash_instruction(c, &format!("NEED {short} MORE META FOR {name}"));
            let sel = c.get_i32("shop_selected").max(0) as usize;
            update_all_slot_labels(c, sel, cat);
            false
        }
    }
}

/// Put a message on the instruction line for a moment; `tick_shop` puts
/// the blurb back.
fn flash_instruction(c: &mut Canvas, text: &str) {
    let scale = c.virtual_scale();
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        if let Some(obj) = c.get_game_object_mut("shop_instr_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                text, &font, 34.0 * scale, Color(255, 236, 150, 240), 1600.0 * scale,
            )));
        }
    }
    c.set_var("shop_instr_restore", 150i32);
}

// ── Rope colour row ─────────────────────────────────────────────────────────

/// One swatch per rope colour, under the carousel on the ROPES screen.
pub const ROPE_SWATCH_D: f32 = 96.0;
const ROPE_SWATCH_GAP: f32 = 34.0;

fn rope_swatch_pos(i: usize) -> (f32, f32) {
    let n = SHOP_ROPE_COLORS.len() as f32;
    let total = n * ROPE_SWATCH_D + (n - 1.0) * ROPE_SWATCH_GAP;
    let x0 = VW * 0.5 - total * 0.5;
    let strip_bottom_y = CARD_CENTER_Y + CARD_H as f32 / 2.0 + 100.0;
    (x0 + i as f32 * (ROPE_SWATCH_D + ROPE_SWATCH_GAP), strip_bottom_y + 30.0 + 110.0 + 50.0)
}

fn show_rope_colors(c: &mut Canvas, show: bool) {
    for i in 0..SHOP_ROPE_COLORS.len() {
        set_visible(c, &format!("shop_rope_color_{i}"), show);
    }
    set_visible(c, "shop_rope_color_ring", show);
    set_visible(c, "shop_rope_color_text", show);
    if show {
        place_rope_color_ring(c);
    }
}

/// Swatch 0, ORIGINAL, wears the colour it means: the browsed style's own.
fn tint_original_swatch(c: &mut Canvas, style: usize) {
    let (r, g, b) = crate::cosmetics::rope_tint(style, 0, SHOP_ROPE_COLORS[0]);
    let d = ROPE_SWATCH_D;
    if let Some(o) = c.get_game_object_mut("shop_rope_color_0") {
        o.set_image(Image { shape: ShapeType::Ellipse(0.0, (d, d), 0.0), image: circle_cached((d * 0.5) as u32, r, g, b), color: None });
    }
}

fn place_rope_color_ring(c: &mut Canvas) {
    let sel = (c.get_i32("player_rope_selected").max(0) as usize).min(SHOP_ROPE_COLORS.len() - 1);
    let (x, y) = rope_swatch_pos(sel);
    let d = ROPE_SWATCH_D + 28.0;
    if let Some(o) = c.get_game_object_mut("shop_rope_color_ring") {
        o.position = (x - 14.0, y - 14.0);
        o.size = (d, d);
    }
    let scale = c.virtual_scale();
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        if let Some(obj) = c.get_game_object_mut("shop_rope_color_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                &format!("COLOUR  \u{2022}  {}   (W / S)", SHOP_ROPE_NAMES[sel]),
                &font, 28.0 * scale, Color(180, 210, 240, 220), 1200.0 * scale,
            )));
        }
    }
}

/// Pick rope colour `i`: equipped at once and saved, whichever style is on.
pub fn select_rope_color(c: &mut Canvas, i: usize) {
    let i = i.min(SHOP_ROPE_COLORS.len() - 1);
    c.set_var("player_rope_selected", i as i32);
    // The menu's swinging cat's rope takes the colour straight away.
    crate::menu_look::apply_menu_cat(c);
    place_rope_color_ring(c);
    let (ch, bg, tr) = (
        c.get_i32("player_char_selected").max(0) as u32,
        c.get_i32("player_bg_selected").max(0) as u32,
        c.get_i32("player_trail_selected").max(0) as u32,
    );
    crate::profile::save_cosmetics(ch, i as u32, bg, tr);
}

/// `NAME  RANK 2/4  •  320 META` — or `MAXED`, or the shortfall.
fn upgrade_slot_label(idx: usize) -> (String, Color) {
    let Some(u) = crate::profile::PERM_UPGRADES.get(idx) else {
        return (String::new(), Color(180, 210, 240, 200));
    };
    let (owned, meta) = {
        let g = crate::profile::profile();
        let p = g.lock().unwrap();
        (p.upgrade_level(u.id), p.meta_currency)
    };
    match crate::profile::upgrade_cost(u, owned) {
        None => (
            format!("{}   {}/{}   •   MAXED", u.name, owned, u.max),
            Color(140, 255, 190, 230),
        ),
        Some(cost) if meta >= cost => (
            format!("{}   {}/{}   •   {} META", u.name, owned, u.max, cost),
            Color(255, 236, 150, 240),
        ),
        Some(cost) => (
            format!("{}   {}/{}   •   NEED {}", u.name, owned, u.max, cost),
            Color(160, 175, 200, 190),
        ),
    }
}

/// `NAME  •  BUY 500` / `NAME  2/4  •  EQUIPPED` and so on.
fn ability_slot_label(idx: usize) -> (String, Color) {
    let Some(a) = crate::profile::ABILITIES.get(idx) else {
        return (String::new(), Color(180, 210, 240, 200));
    };
    let (owned, meta, equipped) = {
        let g = crate::profile::profile();
        let p = g.lock().unwrap();
        (p.upgrade_level(a.id), p.meta_currency, p.equipped_ability == a.id)
    };
    if owned == 0 {
        let cost = crate::profile::ability_cost(a, 0).unwrap_or(0);
        let col = if meta >= cost { Color(255, 236, 150, 240) } else { Color(160, 175, 200, 190) };
        return (format!("{}\n{} META", a.name, cost), col);
    }
    let rank = format!("{}/{}", owned, a.max);
    if equipped {
        (format!("{}\nRANK {}  \u{2022}  EQUIPPED", a.name, rank), Color(140, 255, 190, 230))
    } else {
        (format!("{}\nRANK {}", a.name, rank), Color(180, 210, 240, 220))
    }
}

/// The action button for an ability card, and what it will do:
/// BUY (locked), EQUIP (owned, not equipped), UPGRADE (equipped), MAXED.
pub fn ability_action(idx: usize) -> (String, u8) {
    let Some(a) = crate::profile::ABILITIES.get(idx) else { return ("SELECT".into(), 0) };
    let (owned, equipped) = {
        let g = crate::profile::profile();
        let p = g.lock().unwrap();
        (p.upgrade_level(a.id), p.equipped_ability == a.id)
    };
    match (owned, equipped, crate::profile::ability_cost(a, owned)) {
        (0, _, Some(c)) => (format!("BUY \u{2022} {c}"), 0),
        (_, false, _) => ("EQUIP".into(), 1),
        (_, true, Some(c)) => (format!("UPGRADE \u{2022} {c}"), 2),
        (_, true, None) => ("MAXED".into(), 3),
    }
}

/// SELECT on an ability card: buy it, equip it, or buy its next rank.
pub fn select_ability(c: &mut Canvas, idx: usize) {
    let Some(a) = crate::profile::ABILITIES.get(idx) else { return };
    let (_, what) = ability_action(idx);
    match what {
        1 => {
            crate::profile::equip_ability(a.id);
            flash_instruction(c, &format!("{} EQUIPPED", a.name));
        }
        0 | 2 => match crate::profile::buy_ability_rank(a.id) {
            Ok(1) => flash_instruction(c, &format!("UNLOCKED {}!  EQUIPPED", a.name)),
            Ok(r) => flash_instruction(c, &format!("{} UPGRADED TO RANK {}", a.name, r)),
            Err(e) => flash_instruction(c, &e),
        },
        _ => flash_instruction(c, &format!("{} IS FULLY UPGRADED", a.name)),
    }
    let sel = c.get_i32("shop_selected").max(0) as usize;
    update_all_slot_labels(c, sel, 5);
    refresh_select_text(c, 5, sel);
}

/// The line under the carousel: what the selected upgrade actually does, plus
/// the player's balance. Cosmetic categories keep the browse hint.
pub fn carousel_instruction(c: &Canvas, cat: i32, selected: usize) -> String {
    if cat == 0 || cat == 1 {
        let meta = {
            let g = crate::profile::profile();
            let m = g.lock().unwrap().meta_currency;
            m
        };
        return format!("A / D  or  \u{2190}\u{2192}  to browse   \u{2022}   YOU HAVE {meta} META");
    }
    if is_ability_category(cat) {
        let meta = {
            let g = crate::profile::profile();
            let m = g.lock().unwrap().meta_currency;
            m
        };
        return match crate::profile::ABILITIES.get(selected) {
            Some(a) => format!("{}   \u{2022}   UPGRADES: {}   \u{2022}   YOU HAVE {} META", a.blurb, a.per_rank, meta),
            None => format!("YOU HAVE {meta} META"),
        };
    }
    if !is_upgrade_category(cat) {
        return "A / D  or  \u{2190}\u{2192}  to browse".to_string();
    }
    let meta = {
        let g = crate::profile::profile();
        let m = g.lock().unwrap().meta_currency;
        m
    };
    let _ = c;
    match crate::profile::PERM_UPGRADES.get(selected) {
        Some(u) => format!("{}   •   YOU HAVE {} META   •   ENTER TO BUY", u.blurb, meta),
        None => format!("YOU HAVE {meta} META"),
    }
}

// ── Public API for menu.rs ─────────────────────────────────────────────────

/// Reset shop state and populate text drawables. Call when entering shop view.
pub fn init_shop(canvas: &mut Canvas) {
    canvas.set_var("shop_selected",          0i32);
    canvas.set_var("shop_slide_offset",      0.0f32);
    canvas.set_var("shop_scroll_dir",        0i32);
    canvas.set_var("shop_scroll_held_ticks", 0i32);
    canvas.set_var("shop_screen",            0i32);
    canvas.set_var("shop_active_category",  -1i32);
    canvas.set_var("shop_preview_angle",     0.0f32);
    canvas.set_var("shop_instr_restore",     0i32);

    // Pre-warm caches
    get_card_cache();
    get_cat_card_cache();

    // Set category label texts — use card-relative width so text stays inside each box
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        let s = canvas.virtual_scale();
        for i in 0..NUM_CATS {
            let (_, _, cw, _) = cat_card_rect(i);
            let label_w = cw * 0.44;  // right-half text area, ~6% margin from right edge
            if let Some(obj) = canvas.get_game_object_mut(&format!("shop_cat_label_{i}")) {
                obj.set_drawable(Box::new(ui_text_spec(
                    SHOP_CAT_NAMES[i], &font, 58.0 * s,
                    Color(255, 255, 255, 255), label_w * s,
                )));
            }
            if let Some(obj) = canvas.get_game_object_mut(&format!("shop_cat_sub_{i}")) {
                obj.set_drawable(Box::new(ui_text_spec(
                    SHOP_CAT_SUBTITLES[i], &font, 26.0 * s,
                    Color(200, 215, 235, 180), label_w * s,
                )));
            }
        }
    }

    show_categories(canvas);
}

/// Process one key-press event for the shop carousel.
pub fn handle_shop_key(c: &mut Canvas, key: &Key) {
    // Input only handled on the carousel screen
    if c.get_i32("shop_screen") != 1 { return; }
    // Enter does what the SELECT button does.
    if *key == Key::Named(NamedKey::Enter) {
        c.run(Action::Custom { name: "shop_select".into() });
        return;
    }
    let cat = c.get_i32("shop_active_category");
    let (colors, _) = cat_items(cat);
    let n = colors.len() as i32;
    let cur = c.get_i32("shop_selected");
    if cat == 1 {
        let step: i32 = match key {
            Key::Character(ch) if ch == "w" => -1,
            Key::Character(ch) if ch == "s" => 1,
            Key::Named(NamedKey::ArrowUp) => -1,
            Key::Named(NamedKey::ArrowDown) => 1,
            _ => 0,
        };
        if step != 0 {
            let n = SHOP_ROPE_COLORS.len() as i32;
            let now = c.get_i32("player_rope_selected").max(0);
            select_rope_color(c, (((now + step) % n + n) % n) as usize);
            return;
        }
    }
    let dir: i32 = match key {
        Key::Character(ch) if ch == "a" => -1,
        Key::Character(ch) if ch == "d" =>  1,
        Key::Named(NamedKey::ArrowLeft)  => -1,
        Key::Named(NamedKey::ArrowRight) =>  1,
        _ => return,
    };
    c.set_var("shop_scroll_dir",        dir);
    c.set_var("shop_scroll_held_ticks", 0i32);
    let new_sel = ((cur + dir) + n) % n;
    c.set_var("shop_selected", new_sel);
    c.set_var("shop_slide_offset", dir as f32 * CARD_SPACING);
    update_all_slot_images(c, new_sel as usize, cat);
    update_all_slot_labels(c, new_sel as usize, cat);
    refresh_instruction(c, cat, new_sel as usize);
    refresh_select_text(c, cat, new_sel as usize);
}

/// Process one key-release for the shop hold-scroll.
pub fn handle_shop_key_release(c: &mut Canvas, key: &Key) {
    match key {
        Key::Character(ch) if ch == "a" || ch == "d" => {}
        Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight) => {}
        _ => return,
    }
    c.set_var("shop_scroll_dir",        0i32);
    c.set_var("shop_scroll_held_ticks", 0i32);
}

/// Per-frame carousel update: hold-scroll + slide animation.
/// Call from menu on_update when shop is visible.
pub fn tick_shop(c: &mut Canvas) {
    tick_rgb_text(c);
    tick_slot_icons(c);
    // Only animate when on the carousel screen
    if c.get_i32("shop_screen") != 1 { return; }
    let cat = c.get_i32("shop_active_category");
    let (colors, _) = cat_items(cat);
    let dir = c.get_i32("shop_scroll_dir");
    if dir != 0 {
        let held = c.get_i32("shop_scroll_held_ticks") + 1;
        c.set_var("shop_scroll_held_ticks", held);
        const HOLD_DELAY:  i32 = 28;
        const HOLD_REPEAT: i32 = 7;
        if held > HOLD_DELAY && (held - HOLD_DELAY) % HOLD_REPEAT == 0 {
            let n   = colors.len() as i32;
            let cur = c.get_i32("shop_selected");
            let new_sel = ((cur + dir) + n) % n;
            c.set_var("shop_selected", new_sel);
            c.set_var("shop_slide_offset", dir as f32 * CARD_SPACING);
            update_all_slot_images(c, new_sel as usize, cat);
            update_all_slot_labels(c, new_sel as usize, cat);
            refresh_instruction(c, cat, new_sel as usize);
            refresh_select_text(c, cat, new_sel as usize);
        }
    }

    // A purchase confirmation replaces the blurb for a moment; put the blurb
    // back so the line does not get stuck on a stale message.
    let restore = match c.get_var("shop_instr_restore") { Some(Value::I32(v)) => v, _ => 0 };
    if restore > 0 {
        c.set_var("shop_instr_restore", restore - 1);
        if restore == 1 {
            let sel = c.get_i32("shop_selected").max(0) as usize;
            refresh_instruction(c, cat, sel);
        }
    }

    let offset = c.get_f32("shop_slide_offset");
    if offset.abs() > 1.0 {
        let new_offset = offset * 0.80;
        c.set_var("shop_slide_offset", new_offset);
        update_slot_positions(c, new_offset);
    } else if offset.abs() > 0.01 {
        c.set_var("shop_slide_offset", 0.0f32);
        update_slot_positions(c, 0.0);
    }
}

// ── Live item preview (orbit + trail) ────────────────────────────────────────

/// How many dots the preview trail is made of.
pub const SHOP_TRAIL_DOTS: usize = 18;
/// Radius the shared trail-dot texture is rasterised at. The dots are stretched
/// to their real size by the quad, so one texture serves every size.
const TRAIL_DOT_TEX_R: u32 = 34;

/// Centre of the shop preview pane.
fn preview_center() -> (f32, f32) { (VW * 0.5, 250.0) }

/// How long the ROPES preview's rope is: an in-game rope of ~690 px at the
/// preview's 1.6x, lying across most of the panel.
const PREVIEW_ROPE_LEN: f32 = 1100.0;

/// Recent ball positions used to draw the trail streak.
static PREVIEW_TAIL: OnceLock<Mutex<VecDeque<(f32, f32)>>> = OnceLock::new();

fn bg_preview_img(idx: usize) -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Vec<Arc<image::RgbaImage>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        (0..SHOP_BG_COLORS.len()).map(|i| {
            let (r, g, b) = SHOP_BG_COLORS[i];
            let w = 700u32;
            let h = 260u32;
            let mut img = image::RgbaImage::new(w, h);
            for py in 0..h {
                let t = py as f32 / h as f32;
                let rr = (10.0 + (r as f32 - 10.0) * t) as u8;
                let gg = (18.0 + (g as f32 - 18.0) * t) as u8;
                let bb = (30.0 + (b as f32 - 30.0) * t) as u8;
                for px in 0..w {
                    img.put_pixel(px, py, image::Rgba([rr, gg, bb, 255]));
                }
            }
            Arc::new(img)
        }).collect()
    });
    cache[idx.min(cache.len().saturating_sub(1))].clone()
}

/// Cached animated calico ball for the character preview (slow playback).
fn cached_calico_ball_anim() -> Option<AnimatedSprite> {
    static CACHE: OnceLock<Option<AnimatedSprite>> = OnceLock::new();
    CACHE.get_or_init(|| {
        AnimatedSprite::new(
            include_bytes!("../assets/calicoball.gif"),
            (110.0, 110.0),
            CALICO_FPS,
        ).ok()
    }).clone()
}

/// Alpha-blend a filled circle onto `img` so the shop trail can tint/fade
/// without opaque, blocky squares.
fn draw_circle_alpha(img: &mut image::RgbaImage, cx: f32, cy: f32, r: f32, c: [u8; 4]) {
    let rr = r * r;
    let ca = c[3] as f32 / 255.0;
    let x0 = (cx - r).floor().max(0.0) as u32;
    let x1 = ((cx + r).ceil().min(img.width() as f32)) as u32;
    let y0 = (cy - r).floor().max(0.0) as u32;
    let y1 = ((cy + r).ceil().min(img.height() as f32)) as u32;
    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= rr {
                let p = img.get_pixel_mut(px, py);
                let ba = p[3] as f32 / 255.0;
                let outa = ca + ba * (1.0 - ca);
                if outa > 0.0 {
                    p[0] = ((c[0] as f32 * ca + p[0] as f32 * ba * (1.0 - ca)) / outa).round() as u8;
                    p[1] = ((c[1] as f32 * ca + p[1] as f32 * ba * (1.0 - ca)) / outa).round() as u8;
                    p[2] = ((c[2] as f32 * ca + p[2] as f32 * ba * (1.0 - ca)) / outa).round() as u8;
                    p[3] = (outa * 255.0).round() as u8;
                }
            }
        }
    }
}

/// Decode `energy_hook_1.gif` and recolour each frame to the rope colour
/// the way the game does (colour 0 keeps its own blue).
fn tint_energy_hook_gif(color_idx: usize, color: (u8, u8, u8), size: (f32, f32)) -> Option<AnimatedSprite> {
    use std::io::Cursor;
    let bytes = include_bytes!("../assets/energy_hook_1.gif");
    let cursor = Cursor::new(bytes);
    let decoder = image::codecs::gif::GifDecoder::new(cursor).ok()?;
    let frames: Vec<image::RgbaImage> = decoder.into_frames()
        .filter_map(|f| f.ok())
        .map(|f| {
            let img = f.into_buffer();
            if color_idx == 0 { img } else { crate::cosmetics::gradient_map(&img, color) }
        })
        .collect();
    if frames.is_empty() { return None; }
    Some(AnimatedSprite::from_frames(frames, size, 8.0))
}

/// Cached, per-colour recoloured energy-hook animation for the rope preview.
fn energy_hook_preview(color_idx: usize, color: (u8, u8, u8), size: (f32, f32)) -> Option<AnimatedSprite> {
    static CACHE: OnceLock<Mutex<HashMap<(usize, u8, u8, u8), Option<AnimatedSprite>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap();
    let key = (color_idx, color.0, color.1, color.2);
    if let Some(a) = guard.get(&key) {
        return a.clone();
    }
    let anim = tint_energy_hook_gif(color_idx, color, size);
    guard.insert(key, anim.clone());
    anim
}

/// A breed's frames for the shop preview, cycling slowly so the card shows
/// the cat opening up. Cached per breed.
fn breed_preview_anim(char_idx: usize) -> Option<AnimatedSprite> {
    static CACHE: OnceLock<Mutex<HashMap<usize, Option<AnimatedSprite>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap();
    if let Some(a) = guard.get(&char_idx) {
        return a.clone();
    }
    let anim = crate::cosmetics::cat_sprite(char_idx, 110.0, 3.0);
    guard.insert(char_idx, anim.clone());
    anim
}


/// Hide every pooled trail dot. The dots persist until told otherwise, so every
/// early return out of the preview has to clear them or they hang in the air
/// over whatever comes next.
fn hide_trail_dots(c: &mut Canvas) {
    for i in 0..SHOP_TRAIL_DOTS {
        set_visible(c, &format!("shop_trail_dot_{i}"), false);
    }
}

/// Per-frame shop preview: an orbiting ball with the selected character/trail
/// colour, plus a background preview for the background category. The trail
/// mirrors the gameplay loop's particle trail (round, tapering, trailing behind
/// the ball) and the ROPES category shows a recoloured energy-hook animation.
pub fn tick_shop_preview(c: &mut Canvas) {
    if !matches!(c.get_var("menu_in_shop"), Some(Value::Bool(true))) { return; }
    let cat = c.get_i32("shop_active_category");
    let sel = c.get_i32("shop_selected").max(0) as usize;
    if c.get_i32("shop_screen") != 1 || cat == 4 || cat == 5 {
        set_visible(c, "shop_preview_panel", false);
        set_visible(c, "shop_preview_ball", false);
        set_visible(c, "shop_preview_trail", false);
        set_visible(c, "shop_preview_bg", false);
        hide_trail_dots(c);
        return;
    }
    set_visible(c, "shop_preview_panel", true);

    let (pcx, pcy) = preview_center();

    // Background category: show only a scaled background preview, no ball/trail.
    if cat == 2 {
        set_visible(c, "shop_preview_ball", false);
        set_visible(c, "shop_preview_trail", false);
        set_visible(c, "shop_preview_bg", true);
        hide_trail_dots(c);
        if let Some(obj) = c.get_game_object_mut("shop_preview_bg") {
            let (bw, bh) = (700.0, 260.0);
            obj.size = (bw, bh);
            obj.position = (pcx - bw * 0.5, pcy - bh * 0.5);
            obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (bw, bh), 0.0), image: bg_preview_img(sel), color: None });
        }
        return;
    }
    set_visible(c, "shop_preview_bg", false);

    // Ropes category: the style being browsed, in the colour picked below.
    if cat == 1 {
        set_visible(c, "shop_preview_ball", false);
        hide_trail_dots(c);
        let color_idx = (c.get_i32("player_rope_selected").max(0) as usize).min(SHOP_ROPE_COLORS.len() - 1);
        let rgb = SHOP_ROPE_COLORS[color_idx];
        let t = c.get_f32("shop_preview_t") + 1.0 / 60.0;
        c.set_var("shop_preview_t", t);
        let style_idx = sel.min(crate::cosmetics::ROPE_STYLES.len() - 1);
        // The rope lies along the panel, between the title and the blurb (it
        // used to stand upright, straight through both), drawn the way the
        // game draws it at 1.6x.
        let (cx, cy) = (pcx, pcy + 25.0);
        let len = PREVIEW_ROPE_LEN;
        if style_idx == 0 {
            c.clear_effect("shop_preview_trail");
            if let Some(obj) = c.get_game_object_mut("shop_preview_trail") {
                // The classic GIF stretched end to end, as in the game.
                let w = ROPE_THICKNESS * 1.6;
                obj.size = (w, len);
                obj.position = (cx - w * 0.5, cy - len * 0.5);
                obj.rotation = 90.0;
                // Set once per colour: setting an animation restarts it, so
                // setting it every frame (as this did) froze it on frame 0.
                let tag = format!("classic_{color_idx}");
                if !obj.tags.iter().any(|t| t == &tag) {
                    if let Some(mut anim) = energy_hook_preview(color_idx, rgb, (w, len)) {
                        anim.set_fps(6.0);
                        obj.set_animation(anim);
                        obj.tags.retain(|t| !t.starts_with("classic_"));
                        obj.tags.push(tag);
                    }
                }
                obj.visible = true;
            }
        } else {
            // Premium: the in-game effect itself.
            let style = &crate::cosmetics::ROPE_STYLES[style_idx];
            let frame = crate::cosmetics::rope_frame(style, t);
            let tint = crate::cosmetics::rope_tint(style_idx, color_idx, rgb);
            if let Some(tile) = crate::cosmetics::rope_tile(style_idx, frame, tint) {
                let (w, h) = (style.beam * 1.6, len);
                if let Some(obj) = c.get_game_object_mut("shop_preview_trail") {
                    obj.animated_sprite = None;
                    obj.tags.retain(|t| !t.starts_with("classic_"));
                    obj.size = (w, h);
                    obj.position = (cx - w * 0.5, cy - h * 0.5);
                    obj.rotation = 90.0;
                    obj.visible = true;
                }
                let reps = h / (style.cell_len() * 1.6);
                c.attach_effect("shop_preview_trail",
                    Effect::TiledStrip { image: tile, repeats: reps, phase: -t * style.flow, mirror: style.mirror, alpha: 1.0 },
                    EffectColor::WHITE, (w, h));
            }
        }
        return;
    }

    // Advance the orbit.
    let angle = c.get_f32("shop_preview_angle") + 0.045;
    c.set_var("shop_preview_angle", angle);
    let orbit_r = 110.0;
    let bx = pcx + orbit_r * angle.cos();
    let by = pcy + orbit_r * angle.sin();

    let (ball_rgb, trail_rgb) = match cat {
        0 => (SHOP_CHARS[sel.min(SHOP_CHARS.len() - 1)], (235, 235, 255)),
        3 => ((235, 235, 235), SHOP_TRAIL_COLORS[sel.min(SHOP_TRAIL_COLORS.len() - 1)]),
        _ => ((235, 235, 235), (255, 255, 255)),
    };

    // Orbit ball. Calico (cat 0, sel 0) shows the balled-up animated cat in
    // slow motion; every other option is a solid coloured circle sized to the
    // calico's visible pixel extent (its full 110 frame has ~104px of cat) so
    // they read at the same scale.
    if let Some(obj) = c.get_game_object_mut("shop_preview_ball") {
        let breed = cat == 0 && crate::cosmetics::cat_skin(sel).is_some();
        let d = if cat == 0 && (sel == 0 || breed) { 110.0 } else { 88.0 };
        obj.size = (d, d);
        obj.position = (bx - d * 0.5, by - d * 0.5);
        if breed {
            // The breed's own frames, cycling: ball, half, spread-eagle.
            let showing = obj.tags.iter().any(|t| t == &format!("breed_{sel}"));
            if !showing {
                if let Some(anim) = breed_preview_anim(sel) {
                    obj.set_animation(anim);
                    obj.tags.retain(|t| !t.starts_with("breed_"));
                    obj.tags.push(format!("breed_{sel}"));
                }
            }
        } else if cat == 0 && sel == 0 {
            obj.tags.retain(|t| !t.starts_with("breed_"));
            if obj.animated_sprite.is_none() {
                if let Some(mut anim) = cached_calico_ball_anim() {
                    anim.set_fps(4.0); // slow playback while selected
                    obj.set_animation(anim);
                }
            }
        } else {
            obj.tags.retain(|t| !t.starts_with("breed_"));
            obj.animated_sprite = None;
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                image: circle_cached((d * 0.5) as u32, ball_rgb.0, ball_rgb.1, ball_rgb.2),
                color: None,
            });
            obj.update_image_shape();
        }
        obj.visible = true;
    }

    // Particle-style trail drawn behind the ball on a lower layer, sized to the
    // same ball-to-trail ratio as the in-game emitter (nice round fade).
    let points = {
        let tail = PREVIEW_TAIL.get_or_init(|| Mutex::new(VecDeque::new()));
        let mut t = tail.lock().unwrap();
        t.push_back((bx, by));
        while t.len() > 18 {
            t.pop_front();
        }
        t.iter().cloned().collect::<Vec<_>>()
    };
    // Place the pooled dots. One shared, cached circle texture; only the
    // position, size and tint change, so nothing is rasterised or uploaded.
    let max_r = 34.0_f32; // sized to read clearly behind the smaller circles
    let len = points.len().max(2) as f32;
    for i in 0..SHOP_TRAIL_DOTS {
        let name = format!("shop_trail_dot_{i}");
        match points.get(i) {
            Some(p) => {
                let t = (i as f32 + 1.0) / len;
                let r = max_r * (0.30 + 0.70 * t);
                let alpha = (36.0 + t * 200.0).min(255.0) as u8;
                if let Some(obj) = c.get_game_object_mut(&name) {
                    obj.set_image(Image {
                        shape: ShapeType::Ellipse(0.0, (r * 2.0, r * 2.0), 0.0),
                        image: circle_cached(TRAIL_DOT_TEX_R, 255, 255, 255),
                        color: Some(Color(trail_rgb.0, trail_rgb.1, trail_rgb.2, alpha)),
                    });
                    obj.size = (r * 2.0, r * 2.0);
                    obj.position = (p.0 - r, p.1 - r);
                    obj.visible = true;
                }
            }
            None => set_visible(c, &name, false),
        }
    }
    set_visible(c, "shop_preview_trail", false);
}

// ── Scene extender ─────────────────────────────────────────────────────────
/// Append all shop GameObjects and events to an existing scene.
/// Shop content lives at y = 0..VH (the "upper" region of the combined world).
/// The menu lives at y = VH..2VH. Camera pans from VH (menu) to 0 (shop).
pub fn extend_with_shop(ctx: &mut Context, scene: Scene) -> Scene {
    get_card_cache();     // pre-warm character carousel cache
    get_item_card_cache(1); // pre-warm rope carousel cache
    get_item_card_cache(2); // pre-warm background carousel cache
    get_item_card_cache(3); // pre-warm trail carousel cache
    get_cat_card_cache(); // pre-warm category card cache

    let bg_w = VW + 800.0;
    let bg_h = VH;

    // No backdrop of its own: the shop is the sky above the menu, and
    // `menu_look` draws that one sky across both. Kept as an empty object so
    // the scene's object list is unchanged.
    let bg = GameObject::new_rect(ctx, "shop_bg".into(),
        None::<Image>, (bg_w, bg_h), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);

    // Carousel card strip (hidden until a category is chosen)
    let strip_h = (CARD_H + 200) as u32;
    let card_strip = {
        // A flat panel with two hairlines. Rasterised at a small fixed width
        // and stretched, not at the quad's 4640px — that was 4.3 million pixels
        // written one at a time through `put_pixel` for a solid colour, and
        // over the texture limit besides. Only the height needs real
        // resolution, because the hairlines are horizontal.
        let sw = 8u32;
        let mut img = image::RgbaImage::new(sw, strip_h);
        for py in 0..strip_h { for px in 0..sw {
            img.put_pixel(px, py, image::Rgba([16, 10, 40, 105]));
        }}
        draw_rect(&mut img, 0, 0, sw, 3, [170, 140, 255, 130]);
        draw_rect(&mut img, 0, strip_h - 3, sw, 3, [170, 140, 255, 130]);
        let sw = bg_w as u32;
        let mut obj = GameObject::new_rect(ctx, "shop_card_strip".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (sw as f32, strip_h as f32), 0.0), image: img.into(), color: None }),
            (sw as f32, strip_h as f32),
            (-400.0, CARD_CENTER_Y - CARD_H as f32 / 2.0 - 100.0),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.visible = false;
        obj
    };

    // Title text (always visible; content set by show_categories / show_carousel)
    let title_obj = GameObject::build("shop_title_text")
        .size(1600.0, 220.0)
        .position(VW * 0.5 - 800.0, VH * 0.042)
        .build(ctx);

    // Carousel instruction text (hidden until carousel screen)
    let instr_y = CARD_CENTER_Y - CARD_H as f32 / 2.0 - 100.0 - 90.0;
    let mut instr_obj = GameObject::build("shop_instr_text")
        .size(1000.0, 80.0)
        .position(VW * 0.5 - 500.0, instr_y)
        .build(ctx);
    instr_obj.visible = false;

    let strip_bottom_y = CARD_CENTER_Y + CARD_H as f32 / 2.0 + 100.0;

    // Carousel SELECT button (hidden until carousel screen)
    let select_btn = {
        let (bw, bh) = (380u32, 110u32);
        let mut img = image::RgbaImage::new(bw, bh);
        for py in 0..bh { for px in 0..bw {
            let border = px < 3 || px >= bw - 3 || py < 3 || py >= bh - 3;
            img.put_pixel(px, py, if border { image::Rgba([215, 195, 255, 255]) } else { image::Rgba([112, 76, 210, 215]) });
        }}
        let mut obj = GameObject::new_rect(ctx, "shop_select_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (bw as f32, bh as f32), 0.0), image: img.into(), color: None }),
            (bw as f32, bh as f32),
            (VW / 2.0 - bw as f32 / 2.0, strip_bottom_y + 30.0),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.visible = false;
        obj
    };
    // Carousel arrows. REAL OBJECTS with their own click events, not a region
    // of a bigger widget: the shop half sits at world y 0..VH with the camera at
    // y=0, so world and virtual coordinates coincide there and the engine's own
    // hit testing works — which is why the category cards have always been
    // tappable while everything hand-rolled was not.
    //
    // Sized for a thumb, not for the glyph. These are the only way to move
    // through a category without a keyboard.
    let arrow_w = 260.0f32;
    let arrow_h = 360.0f32;
    let arrow_y = CARD_CENTER_Y - arrow_h * 0.5;
    let make_arrow = |ctx: &mut Context, name: &str, x: f32, left: bool| {
        let (w, h) = (arrow_w as u32, arrow_h as u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            img.put_pixel(px, py, image::Rgba([30, 20, 70, 95]));
        }}
        // A chevron, drawn as two strokes so it reads at a glance.
        let cx = w as f32 * 0.5;
        let cy = h as f32 * 0.5;
        for t in 0..(h / 3) {
            let dy = t as f32;
            let dx = if left { dy } else { -dy };
            for thick in 0..14i32 {
                let px0 = (cx + dx * 0.8 + thick as f32 - 7.0).round();
                for sign in [-1.0f32, 1.0] {
                    let py0 = (cy + sign * dy).round();
                    if px0 >= 0.0 && px0 < w as f32 && py0 >= 0.0 && py0 < h as f32 {
                        img.put_pixel(px0 as u32, py0 as u32, image::Rgba([220, 205, 255, 255]));
                    }
                }
            }
        }
        let mut obj = GameObject::new_rect(ctx, name.to_string().into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (arrow_w, arrow_h), 0.0), image: img.into(), color: None }),
            (arrow_w, arrow_h), (x, arrow_y),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.visible = false;
        obj
    };
    let shop_prev_btn = make_arrow(ctx, "shop_prev_btn", 120.0, true);
    let shop_next_btn = make_arrow(ctx, "shop_next_btn", VW - 120.0 - arrow_w, false);

    let mut select_text_obj = GameObject::build("shop_select_text")
        .size(380.0, 110.0)
        .position(VW / 2.0 - 190.0, strip_bottom_y + 30.0 + (110.0 - 36.0) / 2.0)
        .build(ctx);
    select_text_obj.visible = false;

    // BACK button (always visible when in shop; text changes per screen)
    let back_btn = {
        let (bw, bh) = (380u32, 110u32);
        let mut img = image::RgbaImage::new(bw, bh);
        for py in 0..bh { for px in 0..bw {
            let border = px < 3 || px >= bw - 3 || py < 3 || py >= bh - 3;
            img.put_pixel(px, py, if border { image::Rgba([170, 140, 255, 200]) } else { image::Rgba([36, 26, 82, 160]) });
        }}
        GameObject::new_rect(ctx, "shop_back_btn".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (bw as f32, bh as f32), 0.0), image: img.into(), color: None }),
            (bw as f32, bh as f32),
            (120.0, VH - 220.0),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0)
    };
    let back_text_obj = GameObject::build("shop_back_text")
        .size(380.0, 110.0)
        .position(120.0, VH - 220.0 + (110.0 - 36.0) / 2.0)
        .build(ctx);

    // Carousel slot GameObjects (7 slots, hidden until carousel screen)
    let w = CARD_W as f32;
    let h = CARD_H as f32;
    let card_y  = CARD_CENTER_Y - h / 2.0;
    let label_y = card_y + h * 0.76 + 10.0;
    let cache = get_card_cache();

    let mut scene = scene
        .with_object("shop_bg",          bg)
        .with_object("shop_card_strip",  card_strip)
        .with_object("shop_title_text",  title_obj)
        .with_object("shop_instr_text",  instr_obj)
        .with_object("shop_prev_btn",    shop_prev_btn)
        .with_object("shop_next_btn",    shop_next_btn)
        .with_object("shop_select_btn",  select_btn)
        .with_object("shop_select_text", select_text_obj)
        .with_object("shop_back_btn",    back_btn)
        .with_object("shop_back_text",   back_text_obj);
    // The UPGRADES page's demo clip (see `shop_demos`).
    for (id, o) in crate::shop_demos::demo_objects(ctx) {
        scene = scene.with_object(&id, o);
    }

    // ── Live item preview (orbit ball + trail + background preview) ─────────
    {
        let (pcx, pcy) = preview_center();
        let panel_w = 1300.0f32;
        let panel_h = 300.0f32;
        let mut preview_panel = GameObject::new_rect(ctx, "shop_preview_panel".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (panel_w, panel_h), 0.0), image: solid(22, 16, 50, 150).into(), color: None }),
            (panel_w, panel_h), (pcx - panel_w * 0.5, pcy - panel_h * 0.5),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        preview_panel.visible = false;

        let bd = 110.0f32;
        let mut preview_ball = GameObject::new_rect(ctx, "shop_preview_ball".into(),
            Some(Image { shape: ShapeType::Ellipse(0.0, (bd, bd), 0.0), image: circle_cached((bd * 0.5) as u32, 235, 235, 235), color: None }),
            (bd, bd), (pcx - bd * 0.5, pcy - bd * 0.5),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        preview_ball.layer = 46; // above the trail
        preview_ball.visible = false;

        // The trail is a POOL OF DOTS, not one rasterised image.
        //
        // It used to allocate a fresh 700x300 RgbaImage every frame, draw
        // eighteen circles into it on the CPU, and hand it to `set_image` as a
        // NEW `Arc` — so the atlas saw an unfamiliar pointer each frame and
        // uploaded 840 KB to the GPU and built a new bind group, sixty times a
        // second, for a decoration. That is the whole reason the shop preview
        // chugged.
        //
        // Pooled sprites share one cached circle texture and only move, which
        // is also how the real in-game trail works.
        let mut preview_trail = GameObject::new_rect(ctx, "shop_preview_trail".into(),
            None::<Image>, (600.0, 300.0), (pcx - 300.0, pcy - 150.0),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        preview_trail.layer = 44; // behind the ball
        preview_trail.visible = false;

        let mut trail_dots: Vec<(String, GameObject)> = Vec::new();
        for i in 0..SHOP_TRAIL_DOTS {
            let name = format!("shop_trail_dot_{i}");
            let d = (TRAIL_DOT_TEX_R * 2) as f32;
            let mut dot = GameObject::new_rect(ctx, name.clone().into(),
                Some(Image {
                    shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                    image: circle_cached(TRAIL_DOT_TEX_R, 255, 255, 255),
                    color: Some(Color(255, 255, 255, 200)),
                }),
                (d, d), (pcx - d * 0.5, pcy - d * 0.5),
                vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
            dot.layer = 44; // behind the ball, same as the old trail image
            dot.visible = false;
            trail_dots.push((name, dot));
        }

        let mut preview_bg = GameObject::new_rect(ctx, "shop_preview_bg".into(),
            None::<Image>, (700.0, 260.0), (pcx - 350.0, pcy - 130.0),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        preview_bg.layer = 42;
        preview_bg.visible = false;

        scene = scene
            .with_object("shop_preview_panel", preview_panel)
            .with_object("shop_preview_ball",  preview_ball)
            .with_object("shop_preview_trail", preview_trail)
            .with_object("shop_preview_bg",    preview_bg);
        for (name, dot) in trail_dots {
            scene = scene.with_object(&name, dot);
        }
    }

    // ── Rope colour row (ROPES screen) ────────────────────────────────────
    for i in 0..SHOP_ROPE_COLORS.len() {
        let (x, y) = rope_swatch_pos(i);
        let (r, g, b) = SHOP_ROPE_COLORS[i];
        let d = ROPE_SWATCH_D;
        let mut sw = GameObject::new_rect(ctx, format!("shop_rope_color_{i}").into(),
            Some(Image { shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                         image: circle_cached((d * 0.5) as u32, r, g, b), color: None }),
            (d, d), (x, y), vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        sw.layer = 12;
        sw.visible = false;
        scene = scene
            .with_object(format!("shop_rope_color_{i}"), sw)
            .with_event(
                GameEvent::MousePress {
                    action: Action::Custom { name: format!("shop_rope_color_{i}") },
                    target: Target::name(format!("shop_rope_color_{i}")),
                    button: Some(MouseButton::Left),
                },
                Target::name(format!("shop_rope_color_{i}")),
            );
    }
    {
        let d = ROPE_SWATCH_D + 28.0;
        let n = d as u32;
        let ring_img = image::RgbaImage::from_fn(n, n, |x, y| {
            let (dx, dy) = (x as f32 + 0.5 - d * 0.5, y as f32 + 0.5 - d * 0.5);
            let r = (dx * dx + dy * dy).sqrt();
            if (r - (d * 0.5 - 6.0)).abs() < 5.0 { image::Rgba([255, 255, 255, 235]) } else { image::Rgba([0, 0, 0, 0]) }
        });
        let mut ring = GameObject::new_rect(ctx, "shop_rope_color_ring".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0), image: ring_img.into(), color: None }),
            (d, d), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        ring.layer = 13;
        ring.visible = false;
        let (x0, y0) = rope_swatch_pos(0);
        let mut text = GameObject::build("shop_rope_color_text")
            .size(1200.0, 60.0)
            .position(VW * 0.5 - 600.0, y0 + ROPE_SWATCH_D + 18.0)
            .build(ctx);
        let _ = x0;
        text.visible = false;
        scene = scene
            .with_object("shop_rope_color_ring", ring)
            .with_object("shop_rope_color_text", text);
    }

    for s in 0..NUM_SLOTS {
        let char_idx = slot_char(0, s);
        let img = cache[char_idx][if s == SLOT_CENTER { 1 } else { 0 }].clone();
        let slot_x = VW / 2.0 - w / 2.0 + (s as f32 - SLOT_CENTER as f32) * CARD_SPACING;
        let mut slot = GameObject::new_rect(ctx, format!("shop_slot_{s}").into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img, color: None }),
            (w, h), (slot_x, card_y),
            vec!["shop_card".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        slot.visible = false;
        let mut label = GameObject::build(format!("shop_slot_label_{s}"))
            .size(w, 80.0)
            .position(slot_x, label_y)
            .build(ctx);
        label.visible = false;
        let mut icon = GameObject::new_rect(ctx, format!("shop_slot_icon_{s}").into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (ICON_SIZE, ICON_SIZE), 0.0),
                image: std::sync::Arc::new(image::RgbaImage::new(1, 1)), color: None }),
            (ICON_SIZE, ICON_SIZE), (slot_x + w * 0.5 - ICON_SIZE * 0.5, card_y + h * 0.5 - 60.0 - ICON_SIZE * 0.5),
            vec!["ui".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        icon.visible = false;
        scene = scene
            .with_object(format!("shop_slot_{s}"),       slot)
            .with_object(format!("shop_slot_icon_{s}"),  icon)
            .with_object(format!("shop_slot_label_{s}"), label);
    }

    // ── Category card buttons (2+3 grid layout) ────────────────────────────
    let cat_cache = get_cat_card_cache();
    for i in 0..NUM_CATS {
        let (cx, cy, cw, ch) = cat_card_rect(i);
        let cat_img = cat_cache[i].clone();
        let mut cat_btn = GameObject::new_rect(ctx, format!("shop_cat_btn_{i}").into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (cw, ch), 0.0), image: cat_img, color: None }),
            (cw, ch), (cx, cy),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0);
        cat_btn.visible = false; // shown by init_shop → show_categories

        // Name label: positioned in the center-right area of the card
        let label_x  = cx + cw * 0.52;
        let label_w  = cw * 0.46;
        let label_y2 = cy + ch * 0.38;
        let label_h  = ch * 0.28;
        let mut cat_label = GameObject::build(format!("shop_cat_label_{i}"))
            .size(label_w, label_h)
            .position(label_x, label_y2)
            .build(ctx);
        cat_label.visible = false;

        // Subtitle label: just below the name
        let sub_y = cy + ch * 0.67;
        let sub_h  = ch * 0.20;
        let mut cat_sub = GameObject::build(format!("shop_cat_sub_{i}"))
            .size(label_w, sub_h)
            .position(label_x, sub_y)
            .build(ctx);
        cat_sub.visible = false;

        scene = scene
            .with_object(format!("shop_cat_btn_{i}"),   cat_btn)
            .with_object(format!("shop_cat_label_{i}"), cat_label)
            .with_object(format!("shop_cat_sub_{i}"),   cat_sub)
            .with_event(
                GameEvent::MousePress {
                    action: Action::Custom { name: format!("shop_cat_{i}") },
                    target: Target::name(format!("shop_cat_btn_{i}")),
                    button: Some(MouseButton::Left),
                },
                Target::name(format!("shop_cat_btn_{i}")),
            );
    }

    scene
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "shop_prev".into() },
                target: Target::name("shop_prev_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("shop_prev_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "shop_next".into() },
                target: Target::name("shop_next_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("shop_next_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "shop_back".into() },
                target: Target::name("shop_back_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("shop_back_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "shop_select".into() },
                target: Target::name("shop_select_btn"),
                button: Some(MouseButton::Left),
            },
            Target::name("shop_select_btn"),
        )
        .with_event(
            GameEvent::MousePress {
                action: Action::Custom { name: "buy_perm_heart".into() },
                target: Target::name("shop_instr_text"),
                button: Some(MouseButton::Left),
            },
            Target::name("shop_instr_text"),
        )
}

/// Buy the selected permanent upgrade and refresh the carousel in place.
///
/// Stays on the upgrade screen rather than bouncing back to the categories —
/// buying ranks is a repeated action, and the labels have to update so the
/// player can see the new rank and the new price.
pub fn buy_selected_upgrade(c: &mut Canvas) {
    let cat = c.get_i32("shop_active_category");
    if !is_upgrade_category(cat) {
        return;
    }
    let sel = c.get_i32("shop_selected").max(0) as usize;
    let Some(u) = crate::profile::PERM_UPGRADES.get(sel) else { return };
    let outcome = crate::profile::buy_permanent_upgrade(u.id);
    update_all_slot_labels(c, sel, cat);

    let scale = c.virtual_scale();
    let text = match outcome {
        Ok(level) => format!("{} \u{2192} RANK {}", u.name, level),
        Err(reason) => format!("{}   \u{2022}   {}", u.name, reason),
    };
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        if let Some(obj) = c.get_game_object_mut("shop_instr_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                &text, &font, 30.0 * scale, Color(255, 236, 150, 240), 1600.0 * scale,
            )));
        }
    }
    // The confirmation replaces the blurb; put the blurb back shortly after.
    c.set_var("shop_instr_restore", 90i32);
}
