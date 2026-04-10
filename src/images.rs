use std::sync::{Arc, OnceLock};
use crate::constants::*;

// ─────────────────────────────────────────────────────────────────────────────
// Primitives
// ─────────────────────────────────────────────────────────────────────────────
pub fn solid(r: u8, g: u8, b: u8, a: u8) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(1, 1);
    img.put_pixel(0, 0, image::Rgba([r, g, b, a]));
    img
}

pub fn circle_img(radius: u32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    let d = radius * 2;
    let mut img = image::RgbaImage::new(d, d);
    let c = radius as f32;
    for py in 0..d { for px in 0..d {
        let dx = px as f32 - c + 0.5;
        let dy = py as f32 - c + 0.5;
        if dx*dx + dy*dy <= c*c {
            img.put_pixel(px, py, image::Rgba([r, g, b, 255]));
        }
    }}
    img
}

pub fn gradient_rect(w: u32, h: u32, (r0,g0,b0): (u8,u8,u8), (r1,g1,b1): (u8,u8,u8)) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    for py in 0..h {
        let t = py as f32 / h as f32;
        let r = (r0 as f32*(1.0-t) + r1 as f32*t) as u8;
        let g = (g0 as f32*(1.0-t) + g1 as f32*t) as u8;
        let b = (b0 as f32*(1.0-t) + b1 as f32*t) as u8;
        for px in 0..w { img.put_pixel(px, py, image::Rgba([r, g, b, 255])); }
    }
    img
}

pub fn bar_img(w: u32, h: u32, fill: f32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    let filled = (w as f32 * fill.clamp(0.0, 1.0)) as u32;
    for py in 0..h { for px in 0..w {
        let c = if px == 0 || px == w-1 || py == 0 || py == h-1 {
            image::Rgba([200u8, 200, 200, 255])
        } else if px < filled {
            image::Rgba([r, g, b, 255])
        } else {
            image::Rgba([20u8, 20, 30, 200])
        };
        img.put_pixel(px, py, c);
    }}
    img
}

pub fn draw_rect(img: &mut image::RgbaImage, x: u32, y: u32, w: u32, h: u32, c: [u8; 4]) {
    let max_x = (x + w).min(img.width());
    let max_y = (y + h).min(img.height());
    for py in y..max_y {
        for px in x..max_x {
            img.put_pixel(px, py, image::Rgba(c));
        }
    }
}

pub fn draw_digit_7seg(img: &mut image::RgbaImage, x: u32, y: u32, scale: u32, digit: u8, c: [u8; 4]) {
    let seg = [
        [true,  true,  true,  true,  true,  true,  false], // 0
        [false, true,  true,  false, false, false, false], // 1
        [true,  true,  false, true,  true,  false, true ], // 2
        [true,  true,  true,  true,  false, false, true ], // 3
        [false, true,  true,  false, false, true,  true ], // 4
        [true,  false, true,  true,  false, true,  true ], // 5
        [true,  false, true,  true,  true,  true,  true ], // 6
        [true,  true,  true,  false, false, false, false], // 7
        [true,  true,  true,  true,  true,  true,  true ], // 8
        [true,  true,  true,  true,  false, true,  true ], // 9
    ];
    let d = digit.min(9) as usize;
    let th = 2 * scale;
    let w = 9 * scale;
    let h = 16 * scale;
    if seg[d][0] { draw_rect(img, x + th, y, w - 2*th, th, c); }
    if seg[d][1] { draw_rect(img, x + w - th, y + th, th, h/2 - th, c); }
    if seg[d][2] { draw_rect(img, x + w - th, y + h/2, th, h/2 - th, c); }
    if seg[d][3] { draw_rect(img, x + th, y + h - th, w - 2*th, th, c); }
    if seg[d][4] { draw_rect(img, x, y + h/2, th, h/2 - th, c); }
    if seg[d][5] { draw_rect(img, x, y + th, th, h/2 - th, c); }
    if seg[d][6] { draw_rect(img, x + th, y + h/2 - th/2, w - 2*th, th, c); }
}

// ─────────────────────────────────────────────────────────────────────────────
// Object images (with caching macro)
// ─────────────────────────────────────────────────────────────────────────────
macro_rules! cached_image {
    ($name:ident, $init:expr) => {
        pub fn $name() -> Arc<image::RgbaImage> {
            static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
            IMG.get_or_init(|| Arc::from($init)).clone()
        }
    };
}

pub fn pad_img(w: u32, h: u32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    for py in 0..h { for px in 0..w {
        let border = px < 2 || px >= w-2 || py < 2 || py >= h-2;
        let (cr, cg, cb) = if border { (r/2+80, g/2+80, b/2+80) } else { (r, g, b) };
        img.put_pixel(px, py, image::Rgba([cr, cg, cb, 240]));
    }}
    img
}
cached_image!(pad_image_cached, pad_img(PAD_W as u32, PAD_H as u32, C_PAD.0, C_PAD.1, C_PAD.2));

pub fn spinner_img(w: u32, h: u32) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    for py in 0..h { for px in 0..w {
        let cx = w as f32 * 0.5;
        let cy = h as f32 * 0.5;
        let dx = px as f32 - cx;
        let dy = py as f32 - cy;
        let radial = (dx*dx + dy*dy).sqrt();
        let edge = radial > (w.min(h) as f32 * 0.45);
        let stripe = ((px / 8) + (py / 8)) % 2 == 0;
        let (r, g, b) = if edge {
            (255, 235, 230)
        } else if stripe {
            (C_SPINNER.0, C_SPINNER.1, C_SPINNER.2)
        } else {
            (220, 70, 65)
        };
        img.put_pixel(px, py, image::Rgba([r, g, b, 245]));
    }}
    img
}
cached_image!(spinner_image_cached, spinner_img(SPINNER_W as u32, SPINNER_H as u32));

pub fn flip_img(w: u32, h: u32) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    for py in 0..h { for px in 0..w {
        let border = px < 2 || py < 2 || px >= w-2 || py >= h-2;
        let diag = ((px as i32 - py as i32).abs() < 4) || (((w - px - 1) as i32 - py as i32).abs() < 4);
        let c = if border {
            [255, 255, 220, 255]
        } else if diag {
            [255, 170, 90, 240]
        } else {
            [C_FLIP.0, C_FLIP.1, C_FLIP.2, 220]
        };
        img.put_pixel(px, py, image::Rgba(c));
    }}
    img
}
cached_image!(flip_image_cached, flip_img(FLIP_W as u32, FLIP_H as u32));

pub fn gate_img(w: u32, h: u32) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    for py in 0..h { for px in 0..w {
        let border = px < 4 || px >= w - 4 || py < 4 || py >= h - 4;
        let stripe = (py / 16) % 2 == 0;
        let (r, g, b) = if border {
            (255, 190, 190)
        } else if stripe {
            (210, 70, 70)
        } else {
            (170, 45, 45)
        };
        img.put_pixel(px, py, image::Rgba([r, g, b, 245]));
    }}
    img
}
cached_image!(gate_top_image_cached, gate_img(GATE_W as u32, GATE_TOP_SEG_H as u32));
cached_image!(gate_bot_image_cached, gate_img(GATE_W as u32, GATE_BOT_SEG_H as u32));

pub fn pause_overlay_img() -> image::RgbaImage {
    let w = VW as u32;
    let h = VH as u32;
    let mut img = image::RgbaImage::new(w, h);

    let panel_w = (w as f32 * 0.40) as u32;
    let panel_x = (w - panel_w) / 2;
    let panel_right = panel_x + panel_w;

    // Dark translucent side columns (flush against the center panel).
    draw_rect(&mut img, 0, 0, panel_x, h, [0, 0, 0, 170]);
    draw_rect(&mut img, panel_right, 0, w - panel_right, h, [0, 0, 0, 170]);

    // White center panel
    draw_rect(&mut img, panel_x, 0, panel_w, h, [250, 250, 250, 255]);
    draw_rect(&mut img, panel_x, 0, 3, h, [28, 28, 28, 255]);
    draw_rect(&mut img, panel_x + panel_w - 3, 0, 3, h, [28, 28, 28, 255]);

    // Horizontal option rails in the center panel
    let rail_w = (panel_w as f32 * 0.74) as u32;
    let rail_x = panel_x + (panel_w - rail_w) / 2;
    draw_rect(&mut img, rail_x, h / 2 - 180, rail_w, 4, [28, 28, 28, 240]);
    draw_rect(&mut img, rail_x, h / 2 - 40, rail_w, 4, [28, 28, 28, 240]);
    draw_rect(&mut img, rail_x, h / 2 + 100, rail_w, 4, [28, 28, 28, 240]);
    draw_rect(&mut img, rail_x, h / 2 + 240, rail_w, 4, [28, 28, 28, 240]);

    // Pixel-styled black text: PAUSED / RESUME / MENU / SETTINGS
    let col = [18, 18, 18, 255];
    let scale = 4u32;

    // PAUSED
    draw_word(&mut img, panel_x + panel_w / 2 - 250, h / 2 - 300, scale, "PAUSED", col);
    // Menu options
    draw_word(&mut img, panel_x + panel_w / 2 - 170, h / 2 - 150, scale, "RESUME", col);
    draw_word(&mut img, panel_x + panel_w / 2 - 130, h / 2 - 10, scale, "MENU", col);
    draw_word(&mut img, panel_x + panel_w / 2 - 190, h / 2 + 130, scale, "SETTINGS", col);

    img
}

fn draw_word(img: &mut image::RgbaImage, x: u32, y: u32, scale: u32, text: &str, color: [u8; 4]) {
    let mut cx = x;
    for ch in text.bytes() {
        draw_block_char(img, cx, y, scale, ch, color);
        cx += 6 * scale;
    }
}

fn draw_block_char(img: &mut image::RgbaImage, x: u32, y: u32, s: u32, ch: u8, c: [u8; 4]) {
    let glyph: [u8; 25] = match ch {
        b'A' => [0,1,1,1,0, 1,0,0,0,1, 1,1,1,1,1, 1,0,0,0,1, 1,0,0,0,1],
        b'D' => [1,1,1,1,0, 1,0,0,0,1, 1,0,0,0,1, 1,0,0,0,1, 1,1,1,1,0],
        b'E' => [1,1,1,1,1, 1,0,0,0,0, 1,1,1,1,0, 1,0,0,0,0, 1,1,1,1,1],
        b'G' => [0,1,1,1,1, 1,0,0,0,0, 1,0,1,1,1, 1,0,0,0,1, 0,1,1,1,1],
        b'I' => [1,1,1,1,1, 0,0,1,0,0, 0,0,1,0,0, 0,0,1,0,0, 1,1,1,1,1],
        b'M' => [1,0,0,0,1, 1,1,0,1,1, 1,0,1,0,1, 1,0,0,0,1, 1,0,0,0,1],
        b'N' => [1,0,0,0,1, 1,1,0,0,1, 1,0,1,0,1, 1,0,0,1,1, 1,0,0,0,1],
        b'P' => [1,1,1,1,0, 1,0,0,0,1, 1,1,1,1,0, 1,0,0,0,0, 1,0,0,0,0],
        b'R' => [1,1,1,1,0, 1,0,0,0,1, 1,1,1,1,0, 1,0,1,0,0, 1,0,0,1,0],
        b'S' => [0,1,1,1,1, 1,0,0,0,0, 0,1,1,1,0, 0,0,0,0,1, 1,1,1,1,0],
        b'T' => [1,1,1,1,1, 0,0,1,0,0, 0,0,1,0,0, 0,0,1,0,0, 0,0,1,0,0],
        b'U' => [1,0,0,0,1, 1,0,0,0,1, 1,0,0,0,1, 1,0,0,0,1, 0,1,1,1,0],
        b' ' => [0; 25],
        _ => [0; 25],
    };
    for gy in 0..5u32 {
        for gx in 0..5u32 {
            if glyph[(gy * 5 + gx) as usize] == 1 {
                draw_rect(img, x + gx * s, y + gy * s, s, s, c);
            }
        }
    }
}
