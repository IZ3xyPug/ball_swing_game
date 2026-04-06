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

pub fn boost_img(w: u32, h: u32) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.5;
    let outer = w.min(h) as f32 * 0.50;
    let inner = w.min(h) as f32 * 0.28;
    for py in 0..h { for px in 0..w {
        let dx = px as f32 - cx;
        let dy = py as f32 - cy;
        let d = (dx*dx + dy*dy).sqrt();
        let pxl = if d <= outer && d >= inner {
            image::Rgba([C_BOOST.0, C_BOOST.1, C_BOOST.2, 235])
        } else if d < inner {
            image::Rgba([30, 130, 75, 140])
        } else {
            image::Rgba([0, 0, 0, 0])
        };
        img.put_pixel(px, py, pxl);
    }}
    img
}
cached_image!(boost_image_cached, boost_img(BOOST_W as u32, BOOST_H as u32));

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
    for py in 0..h { for px in 0..w {
        img.put_pixel(px, py, image::Rgba([0, 0, 0, 160]));
    }}
    let letters: &[&[(u32, u32, u32, u32)]] = &[
        &[(0,0,10,50), (0,0,30,10), (30,0,10,30), (0,20,30,10)],       // P
        &[(0,0,10,50), (0,0,30,10), (30,0,10,50), (0,24,30,10)],       // A
        &[(0,0,10,50), (0,40,30,10), (30,0,10,50)],                     // U
        &[(0,0,40,10), (0,0,10,30), (0,20,40,10), (30,20,10,30), (0,40,40,10)], // S
        &[(0,0,40,10), (0,0,10,50), (0,20,30,10), (0,40,40,10)],       // E
        &[(0,0,10,50), (0,0,30,10), (30,10,10,30), (0,40,30,10)],      // D
    ];
    let letter_w = 50u32;
    let scale = 3u32;
    let total_w = letters.len() as u32 * letter_w * scale;
    let base_x = w / 2 - total_w / 2;
    let base_y = h / 2 - 25 * scale;
    let col = image::Rgba([255, 255, 255, 240]);
    for (li, segs) in letters.iter().enumerate() {
        let lx = base_x + li as u32 * letter_w * scale;
        for &(sx, sy, sw, sh) in *segs {
            for py in 0..(sh * scale) { for px in 0..(sw * scale) {
                let fx = lx + sx * scale + px;
                let fy = base_y + sy * scale + py;
                if fx < w && fy < h { img.put_pixel(fx, fy, col); }
            }}
        }
    }
    img
}
