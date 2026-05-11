use quartz::*;
use std::sync::OnceLock;
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
pub const SHOP_CAT_NAMES:     &[&str]          = &["CHARACTERS", "ROPES", "BACKGROUNDS", "TRAILS", "PURCHASABLE"];
pub const SHOP_CAT_SUBTITLES: &[&str]          = &["CHANGE YOUR BALL", "STYLE YOUR SWING", "CHANGE THE SCENE", "LEAVE YOUR MARK", "COMING SOON"];
pub const SHOP_CAT_COLORS:    &[(u8, u8, u8)]  = &[
    ( 60,  80, 180), // 0 Characters  — indigo
    ( 20, 150, 140), // 1 Ropes       — teal
    ( 90,  40, 160), // 2 Backgrounds — purple
    (180,  80,  20), // 3 Trails      — orange
    (140,  30, 100), // 4 Purchasable — magenta
];
pub const NUM_CATS: usize = SHOP_CAT_NAMES.len();

/// Per-category item colours (placeholder swatches for non-character categories).
pub const SHOP_ROPE_COLORS: &[(u8,u8,u8)] = &[
    (220, 220, 220), // 0 white
    (180, 140,  80), // 1 classic
    ( 60, 200, 220), // 2 cyan
    (240, 200,  60), // 3 gold
    (160, 220, 100), // 4 lime
];
pub const SHOP_ROPE_NAMES: &[&str] = &["WHITE", "CLASSIC", "CYAN", "GOLD", "LIME"];

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
fn cat_items(cat: i32) -> (&'static [(u8,u8,u8)], &'static [&'static str]) {
    match cat {
        0 => (PLAYER_CHAR_COLORS, PLAYER_CHAR_NAMES),
        1 => (SHOP_ROPE_COLORS,   SHOP_ROPE_NAMES),
        2 => (SHOP_BG_COLORS,     SHOP_BG_NAMES),
        3 => (SHOP_TRAIL_COLORS,  SHOP_TRAIL_NAMES),
        _ => (PLAYER_CHAR_COLORS, PLAYER_CHAR_NAMES),
    }
}

/// Returns (x, y, w, h) for each category card button (2+3 grid layout).
fn cat_card_rect(idx: usize) -> (f32, f32, f32, f32) {
    match idx {
        0 => ( 360.0,  310.0, 1520.0, 540.0),
        1 => (1960.0,  310.0, 1520.0, 540.0),
        2 => ( 300.0,  940.0, 1040.0, 500.0),
        3 => (1400.0,  940.0, 1040.0, 500.0),
        4 => (2500.0,  940.0, 1040.0, 500.0),
        _ => (   0.0,    0.0,  100.0, 100.0),
    }
}

// ── Image cache (built once at startup) ────────────────────────────────────
static CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();
static ROPE_CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();
static BG_CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();
static TRAIL_CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();

pub fn get_card_cache() -> &'static Vec<[std::sync::Arc<image::RgbaImage>; 2]> {
    CARD_CACHE.get_or_init(|| {
        (0..NUM_CHARS).map(|i| {
            let (r, g, b) = SHOP_CHARS[i];
            [
                std::sync::Arc::new(shop_card_img(r, g, b, false)),
                std::sync::Arc::new(shop_card_img(r, g, b, true)),
            ]
        }).collect()
    })
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
        1 => Some(ROPE_CARD_CACHE.get_or_init(|| make_cache(SHOP_ROPE_COLORS))),
        2 => Some(BG_CARD_CACHE.get_or_init(|| make_cache(SHOP_BG_COLORS))),
        3 => Some(TRAIL_CARD_CACHE.get_or_init(|| make_cache(SHOP_TRAIL_COLORS))),
        _ => None,
    }
}

// ── Card image builder ──────────────────────────────────────────────────────
fn shop_card_img(r: u8, g: u8, b: u8, selected: bool) -> image::RgbaImage {
    let w = CARD_W;
    let h = CARD_H;
    let mut img = image::RgbaImage::new(w, h);

    let (br, bg_v, bb) = if selected { (45u8, 65u8, 90u8) } else { (22u8, 35u8, 55u8) };
    for py in 0..h { for px in 0..w {
        img.put_pixel(px, py, image::Rgba([br, bg_v, bb, 235]));
    }}

    let bw    = if selected { 6u32 } else { 3u32 };
    let bcolr = if selected { [255u8, 255, 255, 255] } else { [80u8, 110, 150, 180] };
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
    draw_rect(&mut img, 12, div_y, w - 24, 2, [100, 130, 165, 140]);
    img
}

/// Build a category card image (w×h) with a gradient, colored border,
/// a glowing orb on the left, a unique icon per category,
/// and a darker translucent band at the bottom for the text labels.
fn shop_cat_card_img(idx: usize, w: u32, h: u32) -> image::RgbaImage {
    let (cr, cg, cb) = SHOP_CAT_COLORS[idx];
    let mut img = image::RgbaImage::new(w, h);

    // Gradient background (dark navy → slightly lighter)
    for py in 0..h {
        let t = py as f32 / h as f32;
        let br = (12.0 + 16.0 * t) as u8;
        let bg = (18.0 + 14.0 * t) as u8;
        let bb = (32.0 + 20.0 * t) as u8;
        for px in 0..w {
            img.put_pixel(px, py, image::Rgba([br, bg, bb, 230]));
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
                let nr = (p[0] as f32 + cr as f32 * t * 0.6).min(255.0) as u8;
                let ng = (p[1] as f32 + cg as f32 * t * 0.6).min(255.0) as u8;
                let nb = (p[2] as f32 + cb as f32 * t * 0.6).min(255.0) as u8;
                img.put_pixel(px, py, image::Rgba([nr, ng, nb, 230]));
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

    // Unique icon per category (bright pixel-art in the orb center)
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
            img.put_pixel(px, py, image::Rgba([nr, ng, nb, 230]));
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

/// Switch to the category selection screen (called from init_shop and shop_back).
pub fn show_categories(c: &mut Canvas) {
    c.set_var("shop_screen", 0i32);

    // Show category objects
    for i in 0..NUM_CATS {
        if let Some(obj) = c.get_game_object_mut(&format!("shop_cat_btn_{i}"))   { obj.visible = true; }
        if let Some(obj) = c.get_game_object_mut(&format!("shop_cat_label_{i}")) { obj.visible = true; }
        if let Some(obj) = c.get_game_object_mut(&format!("shop_cat_sub_{i}"))   { obj.visible = true; }
    }

    // Hide carousel objects
    if let Some(obj) = c.get_game_object_mut("shop_card_strip")  { obj.visible = false; }
    if let Some(obj) = c.get_game_object_mut("shop_instr_text")  { obj.visible = false; }
    if let Some(obj) = c.get_game_object_mut("shop_select_btn")  { obj.visible = false; }
    if let Some(obj) = c.get_game_object_mut("shop_select_text") { obj.visible = false; }
    for s in 0..NUM_SLOTS {
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_{s}"))       { obj.visible = false; }
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_label_{s}")) { obj.visible = false; }
    }

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
                "\u{25C4}  MENU", &font, 32.0 * s, Color(215, 230, 255, 255), 380.0 * s,
            )));
        }
    }
}

/// Switch to the carousel view for a given category index.
/// For category 4 (Purchasable), shows a placeholder empty screen instead.
pub fn show_carousel(c: &mut Canvas, cat: i32) {
    c.set_var("shop_screen", 1i32);
    c.set_var("shop_active_category", cat);

    // Always hide category objects
    for i in 0..NUM_CATS {
        if let Some(obj) = c.get_game_object_mut(&format!("shop_cat_btn_{i}"))   { obj.visible = false; }
        if let Some(obj) = c.get_game_object_mut(&format!("shop_cat_label_{i}")) { obj.visible = false; }
        if let Some(obj) = c.get_game_object_mut(&format!("shop_cat_sub_{i}"))   { obj.visible = false; }
    }

    if cat == 4 {
        // Purchasable — no carousel yet, show empty placeholder screen
        if let Some(obj) = c.get_game_object_mut("shop_card_strip")  { obj.visible = false; }
        if let Some(obj) = c.get_game_object_mut("shop_instr_text")  { obj.visible = true; }
        if let Some(obj) = c.get_game_object_mut("shop_select_btn")  { obj.visible = false; }
        if let Some(obj) = c.get_game_object_mut("shop_select_text") { obj.visible = false; }
        for s in 0..NUM_SLOTS {
            if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_{s}"))       { obj.visible = false; }
            if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_label_{s}")) { obj.visible = false; }
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
            let s = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("shop_title_text") {
                obj.set_drawable(Box::new(ui_text_spec(
                    "PURCHASABLE", &font, 100.0 * s, Color(255, 255, 255, 255), 1600.0 * s,
                )));
            }
            if let Some(obj) = c.get_game_object_mut("shop_instr_text") {
                obj.set_drawable(Box::new(ui_text_spec(
                    "COMING SOON",
                    &font, 48.0 * s, Color(180, 160, 220, 200), 1200.0 * s,
                )));
            }
            if let Some(obj) = c.get_game_object_mut("shop_back_text") {
                obj.set_drawable(Box::new(ui_text_spec(
                    "\u{25C4}  BACK", &font, 32.0 * s, Color(215, 230, 255, 255), 380.0 * s,
                )));
            }
        }
        return;
    }

    // Normal carousel categories (0-3)
    if let Some(obj) = c.get_game_object_mut("shop_card_strip")  { obj.visible = true; }
    if let Some(obj) = c.get_game_object_mut("shop_instr_text")  { obj.visible = true; }
    if let Some(obj) = c.get_game_object_mut("shop_select_btn")  { obj.visible = true; }
    if let Some(obj) = c.get_game_object_mut("shop_select_text") { obj.visible = true; }
    for s in 0..NUM_SLOTS {
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_{s}"))       { obj.visible = true; }
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_label_{s}")) { obj.visible = true; }
    }

    // Reset carousel state
    c.set_var("shop_selected",          0i32);
    c.set_var("shop_slide_offset",      0.0f32);
    c.set_var("shop_scroll_dir",        0i32);
    c.set_var("shop_scroll_held_ticks", 0i32);
    update_slot_positions(c, 0.0);
    update_all_slot_images(c, 0, cat);
    update_all_slot_labels(c, 0, cat);

    // Update title and back button text
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        let s = c.virtual_scale();
        let cat_name = SHOP_CAT_NAMES.get(cat as usize).copied().unwrap_or("SHOP");
        if let Some(obj) = c.get_game_object_mut("shop_title_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                cat_name, &font, 100.0 * s, Color(255, 255, 255, 255), 1600.0 * s,
            )));
        }
        if let Some(obj) = c.get_game_object_mut("shop_instr_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "A / D  or  \u{2190}\u{2192}  to browse",
                &font, 30.0 * s, Color(130, 160, 195, 180), 1000.0 * s,
            )));
        }
        if let Some(obj) = c.get_game_object_mut("shop_select_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "SELECT", &font, 32.0 * s, Color(255, 255, 255, 255), 380.0 * s,
            )));
        }
        if let Some(obj) = c.get_game_object_mut("shop_back_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "\u{25C4}  BACK", &font, 32.0 * s, Color(215, 230, 255, 255), 380.0 * s,
            )));
        }
    }
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
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (w, h), 0.0),
                image: img,
                color: None,
            });
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
            if let Some(obj) = c.get_game_object_mut(&name) {
                obj.set_drawable(Box::new(ui_text_spec(
                    names[idx], &font,
                    28.0 * s_scale, Color(180, 210, 240, 200),
                    (CARD_W as f32) * s_scale,
                )));
            }
        }
    }
}

fn bright_background(w: f32, h: f32) -> Image {
    star_field(w as u32, h as u32, STARFIELD_STAR_COUNT, 0xCAFE_BABE)
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
    let cat = c.get_i32("shop_active_category");
    let (colors, _) = cat_items(cat);
    let n = colors.len() as i32;
    let cur = c.get_i32("shop_selected");
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

    let bg = GameObject::new_rect(
        ctx, "shop_bg".into(),
        Some(bright_background(bg_w, bg_h)),
        (bg_w, bg_h), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    // Carousel card strip (hidden until a category is chosen)
    let strip_h = (CARD_H + 200) as u32;
    let card_strip = {
        let sw = bg_w as u32;
        let mut img = image::RgbaImage::new(sw, strip_h);
        for py in 0..strip_h { for px in 0..sw {
            img.put_pixel(px, py, image::Rgba([10, 28, 50, 180]));
        }}
        draw_rect(&mut img, 0, 0, sw, 3, [60, 100, 160, 180]);
        draw_rect(&mut img, 0, strip_h - 3, sw, 3, [60, 100, 160, 180]);
        let mut obj = GameObject::new_rect(
            ctx, "shop_card_strip".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (sw as f32, strip_h as f32), 0.0),
                image: img.into(), color: None,
            }),
            (sw as f32, strip_h as f32),
            (-400.0, CARD_CENTER_Y - CARD_H as f32 / 2.0 - 100.0),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
        );
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
            img.put_pixel(px, py, image::Rgba([40, 160, 90, if border { 255 } else { 200 }]));
        }}
        let mut obj = GameObject::new_rect(
            ctx, "shop_select_btn".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (bw as f32, bh as f32), 0.0),
                image: img.into(), color: None,
            }),
            (bw as f32, bh as f32),
            (VW / 2.0 - bw as f32 / 2.0, strip_bottom_y + 30.0),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        );
        obj.visible = false;
        obj
    };
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
            img.put_pixel(px, py, image::Rgba([40, 70, 120, if border { 255 } else { 190 }]));
        }}
        GameObject::new_rect(
            ctx, "shop_back_btn".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (bw as f32, bh as f32), 0.0),
                image: img.into(), color: None,
            }),
            (bw as f32, bh as f32),
            (120.0, VH - 220.0),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
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
        .with_object("shop_select_btn",  select_btn)
        .with_object("shop_select_text", select_text_obj)
        .with_object("shop_back_btn",    back_btn)
        .with_object("shop_back_text",   back_text_obj);

    for s in 0..NUM_SLOTS {
        let char_idx = slot_char(0, s);
        let img = cache[char_idx][if s == SLOT_CENTER { 1 } else { 0 }].clone();
        let slot_x = VW / 2.0 - w / 2.0 + (s as f32 - SLOT_CENTER as f32) * CARD_SPACING;
        let mut slot = GameObject::new_rect(
            ctx, format!("shop_slot_{s}").into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img, color: None }),
            (w, h), (slot_x, card_y),
            vec!["shop_card".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        );
        slot.visible = false;
        let mut label = GameObject::build(format!("shop_slot_label_{s}"))
            .size(w, 80.0)
            .position(slot_x, label_y)
            .build(ctx);
        label.visible = false;
        scene = scene
            .with_object(format!("shop_slot_{s}"),       slot)
            .with_object(format!("shop_slot_label_{s}"), label);
    }

    // ── Category card buttons (2+3 grid layout) ────────────────────────────
    let cat_cache = get_cat_card_cache();
    for i in 0..NUM_CATS {
        let (cx, cy, cw, ch) = cat_card_rect(i);
        let cat_img = cat_cache[i].clone();
        let mut cat_btn = GameObject::new_rect(
            ctx, format!("shop_cat_btn_{i}").into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (cw, ch), 0.0),
                image: cat_img, color: None,
            }),
            (cw, ch), (cx, cy),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        );
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
}
