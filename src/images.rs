use std::sync::{Arc, OnceLock, Mutex};
use std::collections::HashMap;
use crate::constants::*;

// ─────────────────────────────────────────────────────────────────────────────
// Primitives
// ─────────────────────────────────────────────────────────────────────────────

/// Generate a gravity well image with concentric stepped-alpha rings.
/// `visual_r` is the visual radius in pixels. The returned image is (2*visual_r) square.
/// Rings fade from high alpha in the center to low alpha at the edge.
pub fn gwell_ring_img(visual_r: f32, r: u8, g: u8, b: u8, ring_count: u32, base_alpha: f32) -> image::RgbaImage {
    let d = (visual_r * 2.0).ceil().max(2.0) as u32;
    let mut img = image::RgbaImage::new(d, d);
    let ctr = visual_r;
    let rings = ring_count.max(1);

    for py in 0..d {
        for px in 0..d {
            let dx = px as f32 + 0.5 - ctr;
            let dy = py as f32 + 0.5 - ctr;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > visual_r { continue; }

            // Which ring band does this pixel fall in? (0 = innermost)
            let norm = dist / visual_r; // 0..1
            let band = (norm * rings as f32).floor().min(rings as f32 - 1.0) as u32;

            // Alpha steps down from center outward
            let step_alpha = 1.0 - (band as f32 / rings as f32);
            let alpha = (base_alpha * step_alpha).clamp(0.0, 255.0) as u8;

            // Slight brightness boost toward center
            let bright = 1.0 + 0.3 * (1.0 - norm);
            let pr = (r as f32 * bright).min(255.0) as u8;
            let pg = (g as f32 * bright).min(255.0) as u8;
            let pb = (b as f32 * bright).min(255.0) as u8;

            img.put_pixel(px, py, image::Rgba([pr, pg, pb, alpha]));
        }
    }
    img
}

/// Cached version of `gwell_ring_img`. Returns an `Arc<RgbaImage>` that is
/// shared across all callers with the same parameters. The cache key encodes
/// `(visual_r, r, g, b, ring_count, base_alpha)` so each unique combo is
/// rasterized exactly once.
pub fn gwell_ring_cached(visual_r: f32, r: u8, g: u8, b: u8, ring_count: u32, base_alpha: f32) -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Mutex<HashMap<(u32, u8, u8, u8, u32, u32), Arc<image::RgbaImage>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (visual_r.to_bits(), r, g, b, ring_count, base_alpha.to_bits());
    let mut guard = map.lock().unwrap();
    if let Some(cached) = guard.get(&key) {
        return cached.clone();
    }
    let img: Arc<image::RgbaImage> = gwell_ring_img(visual_r, r, g, b, ring_count, base_alpha).into();
    guard.insert(key, img.clone());
    img
}

/// Flip an RgbaImage vertically (mirror top-to-bottom).
pub fn flip_image_vertical(src: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = (src.width(), src.height());
    let mut out = image::RgbaImage::new(w, h);
    for py in 0..h {
        for px in 0..w {
            out.put_pixel(px, py, *src.get_pixel(px, h - 1 - py));
        }
    }
    out
}

/// Composite a starfield (quartz Image) into the upper half of a gradient.
/// The starfield occupies `0..split_y` and the gradient fills `split_y..h`.
/// A soft blend zone of `blend_h` pixels smooths the transition.
pub fn composite_starfield_gradient(
    starfield: &image::RgbaImage,
    gradient: &image::RgbaImage,
    out_w: u32,
    out_h: u32,
    blend_h: u32,
) -> image::RgbaImage {
    let split_y = out_h / 2;
    let mut img = image::RgbaImage::new(out_w, out_h);
    for py in 0..out_h {
        for px in 0..out_w {
            let star_px = starfield.get_pixel(px % starfield.width(), py % starfield.height());
            let grad_px = gradient.get_pixel(px % gradient.width(), py % gradient.height());

            let pixel = if py < split_y.saturating_sub(blend_h) {
                // Pure starfield
                *star_px
            } else if py < split_y + blend_h {
                // Blend zone
                let blend_start = split_y.saturating_sub(blend_h) as f32;
                let blend_end = (split_y + blend_h) as f32;
                let t = ((py as f32 - blend_start) / (blend_end - blend_start)).clamp(0.0, 1.0);
                let sr = star_px[0] as f32;
                let sg = star_px[1] as f32;
                let sb = star_px[2] as f32;
                let gr = grad_px[0] as f32;
                let gg = grad_px[1] as f32;
                let gb = grad_px[2] as f32;
                image::Rgba([
                    (sr * (1.0 - t) + gr * t) as u8,
                    (sg * (1.0 - t) + gg * t) as u8,
                    (sb * (1.0 - t) + gb * t) as u8,
                    255,
                ])
            } else {
                // Pure gradient
                *grad_px
            };
            img.put_pixel(px, py, pixel);
        }
    }
    img
}

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
    let corner_r = ((h as f32) * 0.32).round() as i32;
    let w_i = w as i32;
    let h_i = h as i32;
    for py in 0..h {
        for px in 0..w {
            let x = px as i32;
            let y = py as i32;

            // Rounded-rect mask: keep center strips, carve outside quarter-circles.
            let in_mid_x = x >= corner_r && x < (w_i - corner_r);
            let in_mid_y = y >= corner_r && y < (h_i - corner_r);
            let inside = if in_mid_x || in_mid_y {
                true
            } else {
                let cx = if x < corner_r { corner_r } else { w_i - corner_r - 1 };
                let cy = if y < corner_r { corner_r } else { h_i - corner_r - 1 };
                let dx = x - cx;
                let dy = y - cy;
                dx * dx + dy * dy <= corner_r * corner_r
            };

            if !inside {
                img.put_pixel(px, py, image::Rgba([0, 0, 0, 0]));
                continue;
            }

            let border = px < 3 || px >= w - 3 || py < 3 || py >= h - 3;
            let (cr, cg, cb) = if border {
                (
                    r.saturating_div(2).saturating_add(90),
                    g.saturating_div(2).saturating_add(90),
                    b.saturating_div(2).saturating_add(90),
                )
            } else {
                (r, g, b)
            };
            img.put_pixel(px, py, image::Rgba([cr, cg, cb, 240]));
        }
    }
    img
}
cached_image!(pad_image_cached, pad_img(PAD_W as u32, PAD_H as u32, C_PAD.0, C_PAD.1, C_PAD.2));

pub fn spinner_img(w: u32, h: u32, base_r: u8, base_g: u8, base_b: u8) -> image::RgbaImage {
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
            (base_r, base_g, base_b)
        } else {
            (
                base_r.saturating_sub(35),
                base_g.saturating_sub(30),
                base_b.saturating_sub(25),
            )
        };
        img.put_pixel(px, py, image::Rgba([r, g, b, 245]));
    }}
    img
}
cached_image!(spinner_image_cached, spinner_img(SPINNER_W as u32, SPINNER_H as u32, C_SPINNER.0, C_SPINNER.1, C_SPINNER.2));

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
    draw_rect(&mut img, 0, 0, w, h, [0, 0, 0, 170]);
    img
}

pub fn pause_title_img() -> image::RgbaImage {
    let scale = 14u32;
    let text = "PAUSED";
    let text_w = text.len() as u32 * 6 * scale;
    let text_h = 5 * scale;
    let w = text_w + scale * 4;
    let h = text_h + scale * 2;
    let mut img = image::RgbaImage::new(w, h);
    let tx = (w - text_w) / 2;
    let ty = (h - text_h) / 2;
    draw_word(&mut img, tx, ty, scale, text, [255, 255, 255, 255]);
    img
}

pub fn pause_btn_img(w: u32, h: u32, r: u8, g: u8, b: u8, label: &str) -> image::RgbaImage {
    let mut img = pad_img(w, h, r, g, b);
    let scale = 4u32;
    let text_w = label.len() as u32 * 6 * scale;
    let text_h = 5 * scale;
    let tx = (w.saturating_sub(text_w)) / 2;
    let ty = (h.saturating_sub(text_h)) / 2;
    draw_word(&mut img, tx, ty, scale, label, [255, 255, 255, 255]);
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
        b'W' => [1,0,0,0,1, 1,0,0,0,1, 1,0,1,0,1, 1,1,0,1,1, 1,0,0,0,1],
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
