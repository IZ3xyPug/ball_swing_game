use quartz::*;
use crate::constants::*;
use crate::images::*;
use crate::objects::ui_text_spec;

// ── Character roster (r, g, b, name) ─────────────────────────────────────────
const SHOP_CHARS: &[(u8, u8, u8)] = &[
    (200, 200, 220), // silver
    (60,  160, 240), // blue
    (80,  210, 130), // green
    (240, 150,  60), // orange
    (180, 100, 240), // purple
    (240,  90,  90), // red
];
const SHOP_CHAR_NAMES: &[&str] = &[
    "SILVER", "BLUE", "GREEN", "ORANGE", "PURPLE", "RED",
];
const NUM_CHARS: usize = SHOP_CHARS.len();

// ── Card geometry (in virtual pixels) ────────────────────────────────────────
const CARD_W: u32  = 460;
const CARD_H: u32  = 720;
const CARD_SPACING: f32 = 620.0;
const CARD_CENTER_Y: f32 = 900.0;

// ── Card image builder ────────────────────────────────────────────────────────
fn shop_card_img(r: u8, g: u8, b: u8, selected: bool) -> image::RgbaImage {
    let w = CARD_W;
    let h = CARD_H;
    let mut img = image::RgbaImage::new(w, h);

    // Background fill
    let (br, bg_v, bb) = if selected { (45u8, 65u8, 90u8) } else { (22u8, 35u8, 55u8) };
    for py in 0..h { for px in 0..w {
        img.put_pixel(px, py, image::Rgba([br, bg_v, bb, 235]));
    }}

    // Border
    let bw    = if selected { 6u32 } else { 3u32 };
    let bcolr = if selected { [255u8, 255, 255, 255] } else { [80u8, 110, 150, 180] };
    draw_rect(&mut img, 0, 0, w, bw, bcolr);
    draw_rect(&mut img, 0, h - bw, w, bw, bcolr);
    draw_rect(&mut img, 0, 0, bw, h, bcolr);
    draw_rect(&mut img, w - bw, 0, bw, h, bcolr);

    // Circle character (centred slightly above mid)
    let cr: i32 = 120;
    let cx: i32 = (w / 2) as i32;
    let cy: i32 = (h / 2) as i32 - 60;
    for py in 0..h { for px in 0..w {
        let dx = px as i32 - cx;
        let dy = py as i32 - cy;
        if dx * dx + dy * dy <= cr * cr {
            let norm = ((dx * dx + dy * dy) as f32).sqrt() / cr as f32;
            let bright = 1.0 + 0.45 * (1.0 - norm); // highlight toward centre
            let pr = (r as f32 * bright).min(255.0) as u8;
            let pg = (g as f32 * bright).min(255.0) as u8;
            let pb = (b as f32 * bright).min(255.0) as u8;
            img.put_pixel(px, py, image::Rgba([pr, pg, pb, 255]));
        }
    }}

    // Thin divider below circle
    let div_y = ((h as f32) * 0.74) as u32;
    draw_rect(&mut img, 12, div_y, w - 24, 2, [100, 130, 165, 140]);

    img
}

// ── Card position update (slide animation helper) ─────────────────────────────
fn update_card_positions(c: &mut Canvas, offset: f32) {
    let w = CARD_W as f32;
    let h = CARD_H as f32;
    for i in 0..NUM_CHARS {
        let name = format!("shop_card_{i}");
        let base_x = VW / 2.0 - w / 2.0 + i as f32 * CARD_SPACING;
        let card_x = base_x + offset;
        let card_y = CARD_CENTER_Y - h / 2.0;
        if let Some(obj) = c.get_game_object_mut(&name) {
            obj.position = (card_x, card_y);
        }
        // Per-card label follows card
        let label_name = format!("shop_card_label_{i}");
        if let Some(obj) = c.get_game_object_mut(&label_name) {
            obj.position = (card_x, card_y + h * 0.76 + 10.0);
        }
    }
}

// ── Card image update (selected highlight) — only rebuilds changed cards ──────
fn update_card_images(c: &mut Canvas, old_selected: usize, new_selected: usize) {
    let w = CARD_W as f32;
    let h = CARD_H as f32;
    // Collect the two indices that need updating (dedup if same)
    let mut to_update: Vec<usize> = Vec::with_capacity(2);
    to_update.push(new_selected);
    if old_selected != new_selected {
        to_update.push(old_selected);
    }
    for i in to_update {
        if i >= NUM_CHARS { continue; }
        let (r, g, b) = SHOP_CHARS[i];
        let img = shop_card_img(r, g, b, i == new_selected);
        let name = format!("shop_card_{i}");
        if let Some(obj) = c.get_game_object_mut(&name) {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (w, h), 0.0),
                image: img.into(),
                color: None,
            });
        }
    }
}

// ── Scene builder ─────────────────────────────────────────────────────────────
pub fn build_shop_scene(ctx: &mut Context) -> Scene {
    // Background — extended 400px each side to fill overscan edges
    let bg = {
        let bg_w = (VW + 800.0) as u32;
        let bg_h = VH as u32;
        let mut img = image::RgbaImage::new(bg_w, bg_h);
        for py in 0..bg_h { for px in 0..bg_w {
            let t = py as f32 / VH;
            img.put_pixel(px, py, image::Rgba([
                (12.0 + 20.0 * (1.0 - t)) as u8,
                (42.0 + 30.0 * (1.0 - t)) as u8,
                (68.0 + 30.0 * (1.0 - t)) as u8,
                255,
            ]));
        }}
        GameObject::new_rect(
            ctx, "shop_bg".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (bg_w as f32, bg_h as f32), 0.0), image: img.into(), color: None }),
            (bg_w as f32, bg_h as f32), (-400.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    // Card-area tint strip — also extended to cover overscan
    let strip_full_w = (VW + 800.0) as u32;
    let strip_h = (CARD_H + 200) as u32;
    let card_strip = {
        let mut img = image::RgbaImage::new(strip_full_w, strip_h);
        for py in 0..strip_h { for px in 0..strip_full_w {
            img.put_pixel(px, py, image::Rgba([10, 28, 50, 180]));
        }}
        draw_rect(&mut img, 0, 0, strip_full_w, 3, [60, 100, 160, 180]);
        draw_rect(&mut img, 0, strip_h - 3, strip_full_w, 3, [60, 100, 160, 180]);
        GameObject::new_rect(
            ctx, "shop_card_strip".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (strip_full_w as f32, strip_h as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (strip_full_w as f32, strip_h as f32),
            (-400.0, CARD_CENTER_Y - CARD_H as f32 / 2.0 - 100.0),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    // Title text object
    let title_obj = GameObject::build("shop_title_text")
        .size(1600.0, 220.0)
        .position(VW * 0.5 - 800.0, VH * 0.055)
        .build(ctx);

    // Character cards + per-card label objects
    let w = CARD_W as f32;
    let h = CARD_H as f32;
    let mut card_objects: Vec<(String, GameObject)> = Vec::new();
    for i in 0..NUM_CHARS {
        let (r, g, b) = SHOP_CHARS[i];
        let img = shop_card_img(r, g, b, i == 0);
        let card_x = VW / 2.0 - w / 2.0 + i as f32 * CARD_SPACING;
        let card_y = CARD_CENTER_Y - h / 2.0;
        let card = GameObject::new_rect(
            ctx,
            format!("shop_card_{i}").into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (w, h), 0.0),
                image: img.into(),
                color: None,
            }),
            (w, h), (card_x, card_y),
            vec!["shop_card".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        );
        card_objects.push((format!("shop_card_{i}"), card));

        // Per-card label (below the divider line, scrolls with its card)
        let label = GameObject::build(format!("shop_card_label_{i}"))
            .size(w, 80.0)
            .position(card_x, card_y + h * 0.76 + 10.0)
            .build(ctx);
        card_objects.push((format!("shop_card_label_{i}"), label));
    }

    // A/D hint — moved above the tint strip
    let instr_y = CARD_CENTER_Y - h / 2.0 - 100.0 - 90.0; // above tint top edge
    let instr_obj = GameObject::build("shop_instr_text")
        .size(1000.0, 80.0)
        .position(VW * 0.5 - 500.0, instr_y)
        .build(ctx);

    // Select button — bottom right, below tint strip
    let strip_bottom_y = CARD_CENTER_Y + h / 2.0 + 100.0;
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
                image: img.into(),
                color: None,
            }),
            (bw as f32, bh as f32), (VW - 560.0, strip_bottom_y + 30.0),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };
    let select_text_obj = GameObject::build("shop_select_text")
        .size(380.0, 110.0)
        .position(VW - 560.0, strip_bottom_y + 30.0 + (110.0 - 36.0) / 2.0)
        .build(ctx);

    // Back button
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
                image: img.into(),
                color: None,
            }),
            (bw as f32, bh as f32), (120.0, VH - 200.0),
            vec!["button".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };

    let back_text_obj = GameObject::build("shop_back_text")
        .size(380.0, 110.0)
        .position(120.0, VH - 200.0 + (110.0 - 36.0) / 2.0)
        .build(ctx);

    let mut scene = Scene::new("shop")
        .with_object("shop_bg",         bg)
        .with_object("shop_card_strip", card_strip)
        .with_object("shop_title_text", title_obj)
        .with_object("shop_instr_text", instr_obj)
        .with_object("shop_select_btn", select_btn)
        .with_object("shop_select_text", select_text_obj)
        .with_object("shop_back_btn",   back_btn)
        .with_object("shop_back_text",  back_text_obj);

    for (name, card_or_label) in card_objects {
        scene = scene.with_object(name, card_or_label);
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
        .on_enter(|canvas| {
            let cam = Camera::new((VW, VH), (VW, VH));
            canvas.set_camera(cam);

            // Reset carousel state
            canvas.set_var("shop_selected",           0i32);
            canvas.set_var("shop_slide_offset",       0.0f32);
            canvas.set_var("shop_slide_target",       0.0f32);
            canvas.set_var("shop_scroll_dir",         0i32);
            canvas.set_var("shop_scroll_held_ticks",  0i32);

            // Reset card positions (images already correct from build time)
            update_card_positions(canvas, 0.0);

            // Render static text
            if let Ok(font) = Font::from_bytes(include_bytes!("../assets/font.ttf")) {
                let s = canvas.virtual_scale();
                let cw = 1600.0 * s;
                if let Some(obj) = canvas.get_game_object_mut("shop_title_text") {
                    obj.set_drawable(Box::new(ui_text_spec(
                        "SELECT CHARACTER", &font, 100.0 * s, Color(255, 255, 255, 255), cw,
                    )));
                }
                if let Some(obj) = canvas.get_game_object_mut("shop_instr_text") {
                    obj.set_drawable(Box::new(ui_text_spec(
                        "A  /  D   to browse", &font, 30.0 * s, Color(130, 160, 195, 180), 1000.0 * s,
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
                // Per-card labels
                for i in 0..NUM_CHARS {
                    let name = format!("shop_card_label_{i}");
                    if let Some(obj) = canvas.get_game_object_mut(&name) {
                        obj.set_drawable(Box::new(ui_text_spec(
                            SHOP_CHAR_NAMES[i], &font, 28.0 * s,
                            Color(180, 210, 240, 200), (CARD_W as f32) * s,
                        )));
                    }
                }
            }

            // A/D key handler
            let shop_key_registered = matches!(
                canvas.get_var("shop_key_registered"),
                Some(Value::Bool(true))
            );
            if !shop_key_registered {
                canvas.on_key_press(|c, key| {
                    if !c.is_scene("shop") { return; }

                    let n   = NUM_CHARS as i32;
                    let cur = c.get_i32("shop_selected");
                    let dir: i32 = match key {
                        Key::Character(ch) if ch == "a" => -1,
                        Key::Character(ch) if ch == "d" =>  1,
                        Key::Named(NamedKey::ArrowLeft)  => -1,
                        Key::Named(NamedKey::ArrowRight) =>  1,
                        _ => return,
                    };

                    // Reset hold counter on new press
                    c.set_var("shop_scroll_dir", dir);
                    c.set_var("shop_scroll_held_ticks", 0i32);

                    let new_sel = ((cur + dir) + n) % n;
                    c.set_var("shop_selected", new_sel);
                    let target = -(new_sel as f32) * CARD_SPACING;
                    c.set_var("shop_slide_target", target);
                    update_card_images(c, cur as usize, new_sel as usize);
                });

                canvas.on_key_release(|c, key| {
                    if !c.is_scene("shop") { return; }
                    match key {
                        Key::Character(ch) if ch == "a" || ch == "d" => {}
                        Key::Named(NamedKey::ArrowLeft) | Key::Named(NamedKey::ArrowRight) => {}
                        _ => return,
                    }
                    c.set_var("shop_scroll_dir", 0i32);
                    c.set_var("shop_scroll_held_ticks", 0i32);
                });

                canvas.set_var("shop_key_registered", true);
            }

            // Slide animation + hold-scroll on_update
            let shop_anim_registered = matches!(
                canvas.get_var("shop_anim_registered"),
                Some(Value::Bool(true))
            );
            if !shop_anim_registered {
                canvas.on_update(|c| {
                    if !c.is_scene("shop") { return; }

                    // Hold-to-fast-scroll
                    let dir = c.get_i32("shop_scroll_dir");
                    if dir != 0 {
                        let held = c.get_i32("shop_scroll_held_ticks") + 1;
                        c.set_var("shop_scroll_held_ticks", held);
                        const HOLD_DELAY:  i32 = 28; // ~0.47s before repeat
                        const HOLD_REPEAT: i32 = 7;  // repeat every 7 ticks (~8.5 Hz)
                        if held > HOLD_DELAY && (held - HOLD_DELAY) % HOLD_REPEAT == 0 {
                            let n   = NUM_CHARS as i32;
                            let cur = c.get_i32("shop_selected");
                            let new_sel = ((cur + dir) + n) % n;
                            c.set_var("shop_selected", new_sel);
                            let target = -(new_sel as f32) * CARD_SPACING;
                            c.set_var("shop_slide_target", target);
                            update_card_images(c, cur as usize, new_sel as usize);
                        }
                    }

                    // Slide animation
                    let offset = c.get_f32("shop_slide_offset");
                    let target = c.get_f32("shop_slide_target");
                    let diff   = target - offset;

                    if diff.abs() < 1.0 {
                        if (offset - target).abs() > 0.01 {
                            c.set_var("shop_slide_offset", target);
                            update_card_positions(c, target);
                        }
                        return;
                    }

                    let new_offset = offset + diff * 0.18;
                    c.set_var("shop_slide_offset", new_offset);
                    update_card_positions(c, new_offset);
                });
                canvas.set_var("shop_anim_registered", true);
            }

            canvas.register_custom_event("shop_back".into(),   |c| c.load_scene("menu"));
            canvas.register_custom_event("shop_select".into(), |c| {
                let sel = c.get_i32("shop_selected");
                c.set_var("player_char_selected", sel);
            });
        })
}
