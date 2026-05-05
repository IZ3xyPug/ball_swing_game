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

// ── Image cache (built once at startup) ────────────────────────────────────
static CARD_CACHE: OnceLock<Vec<[std::sync::Arc<image::RgbaImage>; 2]>> = OnceLock::new();

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
            obj.position = (slot_x, card_y);
        }
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_label_{s}")) {
            obj.position = (slot_x, label_y);
        }
    }
}

pub fn update_all_slot_images(c: &mut Canvas, selected: usize) {
    let w = CARD_W as f32;
    let h = CARD_H as f32;
    let cache = get_card_cache();
    for s in 0..NUM_SLOTS {
        let char_idx = slot_char(selected, s);
        let img = cache[char_idx][if s == SLOT_CENTER { 1 } else { 0 }].clone();
        if let Some(obj) = c.get_game_object_mut(&format!("shop_slot_{s}")) {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (w, h), 0.0),
                image: img,
                color: None,
            });
        }
    }
}

pub fn update_all_slot_labels(c: &mut Canvas, selected: usize) {
    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        let s_scale = c.virtual_scale();
        for slot in 0..NUM_SLOTS {
            let char_idx = slot_char(selected, slot);
            let name = format!("shop_slot_label_{slot}");
            if let Some(obj) = c.get_game_object_mut(&name) {
                obj.set_drawable(Box::new(ui_text_spec(
                    PLAYER_CHAR_NAMES[char_idx], &font,
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

    update_slot_positions(canvas, 0.0);
    update_all_slot_images(canvas, 0);
    update_all_slot_labels(canvas, 0);

    if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
        let s = canvas.virtual_scale();
        if let Some(obj) = canvas.get_game_object_mut("shop_title_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "SELECT CHARACTER", &font, 100.0 * s, Color(255, 255, 255, 255), 1600.0 * s,
            )));
        }
        if let Some(obj) = canvas.get_game_object_mut("shop_instr_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "A / D  or  \u{2190}\u{2192}  to browse",
                &font, 30.0 * s, Color(130, 160, 195, 180), 1000.0 * s,
            )));
        }
        if let Some(obj) = canvas.get_game_object_mut("shop_select_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "SELECT", &font, 32.0 * s, Color(255, 255, 255, 255), 380.0 * s,
            )));
        }
        if let Some(obj) = canvas.get_game_object_mut("shop_back_text") {
            obj.set_drawable(Box::new(ui_text_spec(
                "\u{25C4}  BACK", &font, 32.0 * s, Color(215, 230, 255, 255), 380.0 * s,
            )));
        }
    }
}

/// Process one key-press event for the shop carousel.
pub fn handle_shop_key(c: &mut Canvas, key: &Key) {
    let n   = NUM_CHARS as i32;
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
    update_all_slot_images(c, new_sel as usize);
    update_all_slot_labels(c, new_sel as usize);
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
    let dir = c.get_i32("shop_scroll_dir");
    if dir != 0 {
        let held = c.get_i32("shop_scroll_held_ticks") + 1;
        c.set_var("shop_scroll_held_ticks", held);
        const HOLD_DELAY:  i32 = 28;
        const HOLD_REPEAT: i32 = 7;
        if held > HOLD_DELAY && (held - HOLD_DELAY) % HOLD_REPEAT == 0 {
            let n   = NUM_CHARS as i32;
            let cur = c.get_i32("shop_selected");
            let new_sel = ((cur + dir) + n) % n;
            c.set_var("shop_selected", new_sel);
            c.set_var("shop_slide_offset", dir as f32 * CARD_SPACING);
            update_all_slot_images(c, new_sel as usize);
            update_all_slot_labels(c, new_sel as usize);
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
    get_card_cache(); // pre-warm

    let bg_w = VW + 800.0;
    let bg_h = VH;

    let bg = GameObject::new_rect(
        ctx, "shop_bg".into(),
        Some(bright_background(bg_w, bg_h)),
        (bg_w, bg_h), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let strip_h = (CARD_H + 200) as u32;
    let card_strip = {
        let sw = bg_w as u32;
        let mut img = image::RgbaImage::new(sw, strip_h);
        for py in 0..strip_h { for px in 0..sw {
            img.put_pixel(px, py, image::Rgba([10, 28, 50, 180]));
        }}
        draw_rect(&mut img, 0, 0, sw, 3, [60, 100, 160, 180]);
        draw_rect(&mut img, 0, strip_h - 3, sw, 3, [60, 100, 160, 180]);
        GameObject::new_rect(
            ctx, "shop_card_strip".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (sw as f32, strip_h as f32), 0.0),
                image: img.into(), color: None,
            }),
            (sw as f32, strip_h as f32),
            (-400.0, CARD_CENTER_Y - CARD_H as f32 / 2.0 - 100.0),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let title_obj = GameObject::build("shop_title_text")
        .size(1600.0, 220.0)
        .position(VW * 0.5 - 800.0, VH * 0.055)
        .build(ctx);

    let instr_y = CARD_CENTER_Y - CARD_H as f32 / 2.0 - 100.0 - 90.0;
    let instr_obj = GameObject::build("shop_instr_text")
        .size(1000.0, 80.0)
        .position(VW * 0.5 - 500.0, instr_y)
        .build(ctx);

    let strip_bottom_y = CARD_CENTER_Y + CARD_H as f32 / 2.0 + 100.0;

    let select_btn = {
        let (bw, bh) = (380u32, 110u32);
        let mut img = image::RgbaImage::new(bw, bh);
        for py in 0..bh { for px in 0..bw {
            let border = px < 3 || px >= bw - 3 || py < 3 || py >= bh - 3;
            img.put_pixel(px, py, image::Rgba([40, 160, 90, if border { 255 } else { 200 }]));
        }}
        GameObject::new_rect(
            ctx, "shop_select_btn".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (bw as f32, bh as f32), 0.0),
                image: img.into(), color: None,
            }),
            (bw as f32, bh as f32),
            (VW / 2.0 - bw as f32 / 2.0, strip_bottom_y + 30.0),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };
    let select_text_obj = GameObject::build("shop_select_text")
        .size(380.0, 110.0)
        .position(VW / 2.0 - 190.0, strip_bottom_y + 30.0 + (110.0 - 36.0) / 2.0)
        .build(ctx);

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
        let slot = GameObject::new_rect(
            ctx, format!("shop_slot_{s}").into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img, color: None }),
            (w, h), (slot_x, card_y),
            vec!["shop_card".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        );
        let label = GameObject::build(format!("shop_slot_label_{s}"))
            .size(w, 80.0)
            .position(slot_x, label_y)
            .build(ctx);
        scene = scene
            .with_object(format!("shop_slot_{s}"),       slot)
            .with_object(format!("shop_slot_label_{s}"), label);
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
