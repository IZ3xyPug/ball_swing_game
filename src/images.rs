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
    composite_starfield_gradient_with_ratio(starfield, gradient, out_w, out_h, blend_h, 0.56)
}

pub fn composite_starfield_gradient_with_ratio(
    starfield: &image::RgbaImage,
    gradient: &image::RgbaImage,
    out_w: u32,
    out_h: u32,
    blend_h: u32,
    split_ratio: f32,
) -> image::RgbaImage {
    // Keep the same soft blend logic, but move the split down so the
    // space/starfield area occupies more of the screen.
    let split_y = ((out_h as f32) * split_ratio.clamp(0.35, 0.90)) as u32;
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

/// Turret composite image: circle body at center with a rectangular barrel
/// extending to the right.  The object is square so the rotation pivot is
/// the body's centre.
pub fn turret_img(
    body_r: u32,
    barrel_len: u32,
    barrel_w: u32,
    body_rgb: (u8, u8, u8),
    barrel_rgb: (u8, u8, u8),
) -> image::RgbaImage {
    let size = (body_r + barrel_len) * 2;
    let mut img = image::RgbaImage::new(size, size);
    let cx = size / 2;
    let cy = size / 2;
    // body circle
    for py in 0..size {
        for px in 0..size {
            let dx = px as f32 - cx as f32 + 0.5;
            let dy = py as f32 - cy as f32 + 0.5;
            if dx * dx + dy * dy <= (body_r * body_r) as f32 {
                img.put_pixel(px, py, image::Rgba([body_rgb.0, body_rgb.1, body_rgb.2, 255]));
            }
        }
    }
    // barrel rectangle extending right from body edge
    let bx_start = cx;
    let bx_end = (cx + body_r + barrel_len).min(size);
    let by_start = cy.saturating_sub(barrel_w / 2);
    let by_end = (cy + barrel_w / 2).min(size);
    for py in by_start..by_end {
        for px in bx_start..bx_end {
            img.put_pixel(px, py, image::Rgba([barrel_rgb.0, barrel_rgb.1, barrel_rgb.2, 255]));
        }
    }
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

/// Cached circle: returns `Arc<RgbaImage>` keyed by (radius, r, g, b).
/// Each unique combo is rasterized once; subsequent calls return the cached Arc.
pub fn circle_cached(radius: u32, r: u8, g: u8, b: u8) -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Mutex<HashMap<(u32, u8, u8, u8), Arc<image::RgbaImage>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (radius, r, g, b);
    let mut guard = map.lock().unwrap();
    if let Some(cached) = guard.get(&key) {
        return cached.clone();
    }
    let img = circle_img(radius, r, g, b);
    let arc: Arc<image::RgbaImage> = Arc::new(img);
    guard.insert(key, arc.clone());
    arc
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

fn pad_img_with_tuning(
    w: u32,
    h: u32,
    r: u8,
    g: u8,
    b: u8,
    corner_ratio: f32,
    source_y_stretch: f32,
) -> image::RgbaImage {
    static PAD_BASE: OnceLock<Option<image::RgbaImage>> = OnceLock::new();

    let base = PAD_BASE.get_or_init(|| {
        image::load_from_memory(
            include_bytes!("../assets/rounded_rectangle.png"),
        )
        .map(|img| {
            let rgba = img.to_rgba8();

            // Trim transparent padding so nine-slice uses the actual rounded shape,
            // not the oversized source canvas.
            let mut min_x = rgba.width();
            let mut min_y = rgba.height();
            let mut max_x = 0u32;
            let mut max_y = 0u32;
            let mut found = false;

            for y in 0..rgba.height() {
                for x in 0..rgba.width() {
                    if rgba.get_pixel(x, y)[3] > 0 {
                        found = true;
                        min_x = min_x.min(x);
                        min_y = min_y.min(y);
                        max_x = max_x.max(x);
                        max_y = max_y.max(y);
                    }
                }
            }

            if !found {
                return rgba;
            }

            let cw = max_x.saturating_sub(min_x).saturating_add(1);
            let ch = max_y.saturating_sub(min_y).saturating_add(1);
            image::imageops::crop_imm(&rgba, min_x, min_y, cw, ch).to_image()
        })
        .ok()
    });

    if let Some(base_src) = base {
        let mut stretched_src: Option<image::RgbaImage> = None;
        let src: &image::RgbaImage = if (source_y_stretch - 1.0).abs() > f32::EPSILON {
            let stretched_h = ((base_src.height() as f32) * source_y_stretch).round() as u32;
            let final_h = stretched_h.max(base_src.height()).max(1);
            stretched_src = Some(image::imageops::resize(
                base_src,
                base_src.width().max(1),
                final_h,
                image::imageops::FilterType::CatmullRom,
            ));
            stretched_src.as_ref().unwrap()
        } else {
            base_src
        };

        let sw = src.width();
        let sh = src.height();
        if sw > 0 && sh > 0 {
            // Use 9-slice sampling so rounded corners keep their shape on wide buttons.
            let mut corner = ((sw.min(sh) as f32) * corner_ratio).round() as u32;
            let max_corner = (sw.min(sh) / 2).saturating_sub(1);
            corner = corner.clamp(1, max_corner.max(1));

            let left_d = corner.min(w / 2);
            let right_d = corner.min(w.saturating_sub(left_d));
            let top_d = corner.min(h / 2);
            let bottom_d = corner.min(h.saturating_sub(top_d));

            let center_dw = w.saturating_sub(left_d + right_d);
            let center_dh = h.saturating_sub(top_d + bottom_d);

            let left_s = corner;
            let right_s = corner;
            let top_s = corner;
            let bottom_s = corner;

            let center_sw = sw.saturating_sub(left_s + right_s);
            let center_sh = sh.saturating_sub(top_s + bottom_s);

            let mut out = image::RgbaImage::new(w, h);
            for py in 0..h {
                let sy = if py < top_d {
                    if top_d > 0 { py.saturating_mul(top_s) / top_d } else { 0 }
                } else if py >= h.saturating_sub(bottom_d) {
                    let dy = py.saturating_sub(h.saturating_sub(bottom_d));
                    let offs = if bottom_d > 0 { dy.saturating_mul(bottom_s) / bottom_d } else { 0 };
                    sh.saturating_sub(bottom_s).saturating_add(offs)
                } else {
                    let dy = py.saturating_sub(top_d);
                    let offs = if center_dh > 0 { dy.saturating_mul(center_sh) / center_dh } else { 0 };
                    top_s.saturating_add(offs)
                }
                .min(sh.saturating_sub(1));

                for px in 0..w {
                    let sx = if px < left_d {
                        if left_d > 0 { px.saturating_mul(left_s) / left_d } else { 0 }
                    } else if px >= w.saturating_sub(right_d) {
                        let dx = px.saturating_sub(w.saturating_sub(right_d));
                        let offs = if right_d > 0 { dx.saturating_mul(right_s) / right_d } else { 0 };
                        sw.saturating_sub(right_s).saturating_add(offs)
                    } else {
                        let dx = px.saturating_sub(left_d);
                        let offs = if center_dw > 0 { dx.saturating_mul(center_sw) / center_dw } else { 0 };
                        left_s.saturating_add(offs)
                    }
                    .min(sw.saturating_sub(1));

                    let p = src.get_pixel(sx, sy);
                    let luma = p[0] as f32 / 255.0;
                    let a = p[3];
                    if a == 0 {
                        out.put_pixel(px, py, image::Rgba([0, 0, 0, 0]));
                        continue;
                    }
                    let tr = (r as f32 * luma).clamp(0.0, 255.0) as u8;
                    let tg = (g as f32 * luma).clamp(0.0, 255.0) as u8;
                    let tb = (b as f32 * luma).clamp(0.0, 255.0) as u8;
                    out.put_pixel(px, py, image::Rgba([tr, tg, tb, a]));
                }
            }
            return out;
        }
    }

    // Fallback to the old procedural rounded rectangle if asset loading fails.
    let mut img = image::RgbaImage::new(w, h);
    let w_i = w as i32;
    let h_i = h as i32;
    let max_corner_r = ((w.min(h) as i32) / 2).saturating_sub(1).max(1);
    let corner_r = (((h as f32) * corner_ratio).round() as i32).clamp(1, max_corner_r);
    for py in 0..h {
        for px in 0..w {
            let x = px as i32;
            let y = py as i32;

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

pub fn pad_img(w: u32, h: u32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    // Bounce pads: restored to a more natural old profile.
    pad_img_with_tuning(w, h, r, g, b, 0.48, 1.0)
}

fn pause_pad_img(w: u32, h: u32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    // Pause UI: keep the fuller profile you approved.
    pad_img_with_tuning(w, h, r, g, b, 0.62 * 1.33 * 1.33 * 1.5, 1.33 * 1.33 * 1.5)
}

cached_image!(pad_image_cached, pad_img(PAD_W as u32, PAD_H as u32, C_PAD.0, C_PAD.1, C_PAD.2));

/// Cached pad image keyed by (w, h, r, g, b). Each unique color/size combo
/// is rasterized once; subsequent calls return the cached Arc.
pub fn pad_cached(w: u32, h: u32, r: u8, g: u8, b: u8) -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Mutex<HashMap<(u32, u32, u8, u8, u8), Arc<image::RgbaImage>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (w, h, r, g, b);
    let mut guard = map.lock().unwrap();
    if let Some(cached) = guard.get(&key) {
        return cached.clone();
    }
    let img = pad_img(w, h, r, g, b);
    let arc: Arc<image::RgbaImage> = Arc::new(img);
    guard.insert(key, arc.clone());
    arc
}

pub fn pause_pad_cached(w: u32, h: u32, r: u8, g: u8, b: u8) -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Mutex<HashMap<(u32, u32, u8, u8, u8), Arc<image::RgbaImage>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (w, h, r, g, b);
    let mut guard = map.lock().unwrap();
    if let Some(cached) = guard.get(&key) {
        return cached.clone();
    }
    let img = pause_pad_img(w, h, r, g, b);
    let arc: Arc<image::RgbaImage> = Arc::new(img);
    guard.insert(key, arc.clone());
    arc
}

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

/// Cached spinner image keyed by (w, h, r, g, b).
pub fn spinner_cached(w: u32, h: u32, r: u8, g: u8, b: u8) -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Mutex<HashMap<(u32, u32, u8, u8, u8), Arc<image::RgbaImage>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (w, h, r, g, b);
    let mut guard = map.lock().unwrap();
    if let Some(cached) = guard.get(&key) {
        return cached.clone();
    }
    let img = spinner_img(w, h, r, g, b);
    let arc: Arc<image::RgbaImage> = Arc::new(img);
    guard.insert(key, arc.clone());
    arc
}

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
    // Add horizontal overscan so the tint covers any safe-area padding.
    const OVERSCAN: u32 = 400;
    let w = VW as u32 + OVERSCAN * 2;
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
    let mut img = (*pause_pad_cached(w, h, r, g, b)).clone();
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

/// Cached asteroid image resized to hook dimensions (HOOK_R*2 × HOOK_R*2).
/// Loaded from disk once; subsequent calls return the same Arc.
pub fn asteroid_hook_image_cached() -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let d = (HOOK_R * 2.0) as u32;
        match image::open(ASSET_ASTEROID) {
            Ok(img) => Arc::new(image::imageops::resize(
                &img.into_rgba8(), d, d,
                image::imageops::FilterType::Lanczos3,
            )),
            Err(_) => Arc::new(circle_img(HOOK_R as u32, 128, 128, 128)),
        }
    }).clone()
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

// ─────────────────────────────────────────────────────────────────────────────
// Space zone images
// ─────────────────────────────────────────────────────────────────────────────

/// Rocket pad image: teal/cyan panel with three upward-pointing arrow chevrons.
/// The image is (w × h) pixels, tinted by (r, g, b).
pub fn rocket_pad_img(w: u32, h: u32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    // Background fill — semi-transparent dark panel
    draw_rect(&mut img, 0, 0, w, h, [12, 12, 18, 240]);
    // Top and bottom glow edge
    draw_rect(&mut img, 0, 0, w, 5, [r, g, b, 240]);
    draw_rect(&mut img, 0, h.saturating_sub(5), w, 5, [r, g, b, 180]);
    draw_rect(&mut img, 0, 0, 4, h, [r, g, b, 120]);
    draw_rect(&mut img, w.saturating_sub(4), 0, 4, h, [r, g, b, 120]);

    // Bright center stripe
    let mid_h = h / 2;
    draw_rect(&mut img, 0, mid_h.saturating_sub(3), w, 6, [r, g, b, 60]);

    // Three chevron arrows pointing upward
    let arrow_count = 3u32;
    let arrow_w = w / (arrow_count * 2 + 1);
    let arrow_h = (h as f32 * 0.55) as u32;
    let gap = arrow_w;
    let total_arrow_w = arrow_count * arrow_w + (arrow_count - 1) * gap;
    let start_x = (w - total_arrow_w) / 2;

    for i in 0..arrow_count {
        let ax = start_x + i * (arrow_w + gap);
        let ay = (h - arrow_h) / 2;
        // Chevron: draw from bottom-left to tip, then tip to bottom-right
        let mid_x = ax + arrow_w / 2;
        let tip_y = ay;
        let base_y = ay + arrow_h;
        // Left side of chevron
        for t in 0..arrow_h {
            let tx = ax + (t * (arrow_w / 2)) / arrow_h.max(1);
            let ty = base_y - t;
            if tx < w && ty < h {
                let bright = (r as f32 * 1.4).min(255.0) as u8;
                let bg_val = (g as f32 * 1.4).min(255.0) as u8;
                let bb_val = (b as f32 * 1.4).min(255.0) as u8;
                draw_rect(&mut img, tx, ty, 4, 4, [bright, bg_val, bb_val, 240]);
            }
        }
        // Right side of chevron
        for t in 0..arrow_h {
            let tx = mid_x + (t * (arrow_w / 2)) / arrow_h.max(1);
            let ty = tip_y + t;
            if tx < w && ty < h {
                let bright = (r as f32 * 1.4).min(255.0) as u8;
                let bg_val = (g as f32 * 1.4).min(255.0) as u8;
                let bb_val = (b as f32 * 1.4).min(255.0) as u8;
                draw_rect(&mut img, tx, ty, 4, 4, [bright, bg_val, bb_val, 240]);
            }
        }
        let _ = (mid_x, tip_y);
    }

    img
}

/// Cached rocket pad image. Returns `Arc<RgbaImage>` keyed by (w, h, r, g, b).
pub fn rocket_pad_image_cached() -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    CACHE.get_or_init(|| {
        Arc::new(rocket_pad_img(
            ROCKET_PAD_W as u32,
            ROCKET_PAD_H as u32,
            C_ROCKET_PAD.0,
            C_ROCKET_PAD.1,
            C_ROCKET_PAD.2,
        ))
    }).clone()
}

/// Planet image: a filled body circle (radius `visual_r`) centered inside a
/// larger image of radius `gravity_r`, with a faint ring at the gravity boundary.
/// The image dimensions are `(gravity_r * 2) × (gravity_r * 2)`.
pub fn planet_img(visual_r: f32, gravity_r: f32, r: u8, g: u8, b: u8) -> image::RgbaImage {
    let d = (gravity_r * 2.0).ceil().max(2.0) as u32;
    let mut img = image::RgbaImage::new(d, d);
    let ctr = gravity_r;

    // Gravity field ring: very subtle, just a thin semi-transparent outline
    let ring_thickness = 3.0f32;
    let ring_r = gravity_r - ring_thickness * 0.5;

    // Planet body with radial gradient (brighter core, darker limb)
    let vis_r_sq = visual_r * visual_r;
    let grav_r_sq = gravity_r * gravity_r;
    let _ = grav_r_sq;

    for py in 0..d {
        for px in 0..d {
            let dx = px as f32 + 0.5 - ctr;
            let dy = py as f32 + 0.5 - ctr;
            let dist_sq = dx * dx + dy * dy;
            let dist = dist_sq.sqrt();

            if dist_sq <= vis_r_sq {
                // Inside planet body — radial gradient (bright center → dark edge)
                let t = (dist / visual_r).clamp(0.0, 1.0);
                // Quadratic falloff: bright at center, atmospheric haze at limb
                let luma = 1.0 - t * t * 0.62;
                // Subtle color variation: cooler top, warmer bottom
                let latitude = 1.0 - (dy / visual_r + 1.0) * 0.5; // 0 = south, 1 = north
                let band_shift = (latitude * std::f32::consts::PI * 3.0).sin() * 0.08;
                let pr = ((r as f32 * (luma + band_shift)).clamp(0.0, 255.0)) as u8;
                let pg = ((g as f32 * luma).clamp(0.0, 255.0)) as u8;
                let pb = ((b as f32 * (luma - band_shift * 0.5)).clamp(0.0, 255.0)) as u8;
                // Atmosphere edge: thin bright fringe just inside limb
                let atmos = if t > 0.88 { ((t - 0.88) / 0.12 * 60.0) as u8 } else { 0 };
                let fr = pr.saturating_add(atmos / 3);
                let fg = pg.saturating_add(atmos / 2);
                let fb = pb.saturating_add(atmos);
                img.put_pixel(px, py, image::Rgba([fr, fg, fb, 255]));
            } else if (dist - ring_r).abs() < ring_thickness {
                // Gravity field ring: very faint outline
                let t = 1.0 - (dist - ring_r).abs() / ring_thickness;
                let alpha = (t * 38.0) as u8;
                img.put_pixel(px, py, image::Rgba([r, g, b, alpha]));
            }
        }
    }
    img
}

/// Cached planet image. Returns `Arc<RgbaImage>` keyed by (visual_r_bits, gravity_r_bits, r, g, b).
pub fn planet_img_cached(visual_r: f32, gravity_r: f32, r: u8, g: u8, b: u8) -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Mutex<HashMap<(u32, u32, u8, u8, u8), Arc<image::RgbaImage>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (visual_r.to_bits(), gravity_r.to_bits(), r, g, b);
    let mut guard = map.lock().unwrap();
    if let Some(cached) = guard.get(&key) {
        return cached.clone();
    }
    let img: Arc<image::RgbaImage> = planet_img(visual_r, gravity_r, r, g, b).into();
    guard.insert(key, img.clone());
    img
}

/// Black hole image: deeply dark concentric fade-rings.
/// `radius` = total image half-size. Gravity extent = full radius.
pub fn black_hole_img(radius: f32) -> image::RgbaImage {
    let d = (radius * 2.0).ceil().max(2.0) as u32;
    let mut img = image::RgbaImage::new(d, d);
    let ctr = radius;
    let event_horizon_r = radius * 0.22;
    let inner_halo_r    = radius * 0.44;

    for py in 0..d {
        for px in 0..d {
            let dx = px as f32 + 0.5 - ctr;
            let dy = py as f32 + 0.5 - ctr;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > radius { continue; }

            let t = dist / radius; // 0=center, 1=edge

            // Deep event horizon: near-black with barely-visible dark purple tinge
            let alpha = if dist <= event_horizon_r {
                // Solid dark core
                let t_core = dist / event_horizon_r;
                // Slight visibility so it's not invisible on space bg
                (20.0 + t_core * 18.0) as u8
            } else if dist <= inner_halo_r {
                // Dim accretion fringe
                let t_halo = (dist - event_horizon_r) / (inner_halo_r - event_horizon_r);
                (38.0 - t_halo * 22.0) as u8
            } else {
                // Outer subtle gradient ring
                let t_outer = (dist - inner_halo_r) / (radius - inner_halo_r);
                (16.0 * (1.0 - t_outer * t_outer)) as u8
            };

            let pr = (C_BLACKHOLE.0 as f32 * (1.0 - t * 0.5)) as u8;
            let pg = (C_BLACKHOLE.1 as f32) as u8;
            let pb = ((C_BLACKHOLE.2 as f32) + t * 20.0).min(60.0) as u8;
            img.put_pixel(px, py, image::Rgba([pr, pg, pb, alpha]));
        }
    }
    img
}

/// Cached black hole image keyed by radius bits.
pub fn black_hole_img_cached(radius: f32) -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Mutex<HashMap<u32, Arc<image::RgbaImage>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = radius.to_bits();
    let mut guard = map.lock().unwrap();
    if let Some(cached) = guard.get(&key) {
        return cached.clone();
    }
    let img: Arc<image::RgbaImage> = black_hole_img(radius).into();
    guard.insert(key, img.clone());
    img
}

/// Space coin image: bright golden star shape for high-value space collectibles.
pub fn space_coin_img(radius: u32) -> image::RgbaImage {
    let d = radius * 2;
    let mut img = image::RgbaImage::new(d, d);
    let ctr = radius as f32;
    let r = radius as f32;
    let inner_r = r * 0.42;
    let outer_r = r * 0.90;
    let points = 6u32;

    for py in 0..d {
        for px in 0..d {
            let dx = px as f32 + 0.5 - ctr;
            let dy = py as f32 + 0.5 - ctr;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > r { continue; }

            // Star mask: interpolate between inner and outer radius by angle
            let angle = dy.atan2(dx);
            let sector = (angle / (std::f32::consts::TAU / points as f32)) * 2.0;
            let frac = sector - sector.floor();
            let star_r = inner_r + (outer_r - inner_r) * (1.0 - (frac - 0.5).abs() * 2.0);

            if dist <= star_r {
                let t = dist / star_r;
                // Gold gradient: bright center → warm edge
                let luma = 1.0 - t * 0.45;
                let gold_r = (C_SPACE_COIN.0 as f32 * luma).min(255.0) as u8;
                let gold_g = (C_SPACE_COIN.1 as f32 * luma).min(255.0) as u8;
                let gold_b = (C_SPACE_COIN.2 as f32 * luma * 0.4).min(255.0) as u8;
                img.put_pixel(px, py, image::Rgba([gold_r, gold_g, gold_b, 255]));
            }
        }
    }
    img
}

/// Cached space coin image keyed by radius.
pub fn space_coin_img_cached(radius: u32) -> Arc<image::RgbaImage> {
    static CACHE: OnceLock<Mutex<HashMap<u32, Arc<image::RgbaImage>>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap();
    if let Some(cached) = guard.get(&radius) {
        return cached.clone();
    }
    let img: Arc<image::RgbaImage> = space_coin_img(radius).into();
    guard.insert(radius, img.clone());
    img
}

/// Oxygen bar image. `fill` is 0.0 (empty) to 1.0 (full).
/// Color blends green → yellow → red as fill decreases.
pub fn oxygen_bar_img(fill: f32, w: u32, h: u32) -> image::RgbaImage {
    let fill = fill.clamp(0.0, 1.0);
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [180, 180, 200, 255]);
    draw_rect(&mut img, 0, h.saturating_sub(2), w, 2, [180, 180, 200, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [180, 180, 200, 255]);
    draw_rect(&mut img, w.saturating_sub(2), 0, 2, h, [180, 180, 200, 255]);

    let bar_color = if fill > 0.50 {
        let t = (fill - 0.50) * 2.0; // 0..1 (1 = full green)
        let r = (C_OXY_FULL.0 as f32 * t + C_OXY_MID.0 as f32 * (1.0 - t)) as u8;
        let g = (C_OXY_FULL.1 as f32 * t + C_OXY_MID.1 as f32 * (1.0 - t)) as u8;
        let b = (C_OXY_FULL.2 as f32 * t + C_OXY_MID.2 as f32 * (1.0 - t)) as u8;
        [r, g, b, 255]
    } else {
        let t = fill * 2.0; // 0..1 (1 = yellow)
        let r = (C_OXY_MID.0 as f32 * t + C_OXY_LOW.0 as f32 * (1.0 - t)) as u8;
        let g = (C_OXY_MID.1 as f32 * t + C_OXY_LOW.1 as f32 * (1.0 - t)) as u8;
        let b = (C_OXY_MID.2 as f32 * t + C_OXY_LOW.2 as f32 * (1.0 - t)) as u8;
        [r, g, b, 255]
    };

    let inner_w = w.saturating_sub(4);
    let inner_h = h.saturating_sub(4);
    let filled_w = ((inner_w as f32 * fill) as u32).min(inner_w);
    draw_rect(&mut img, 2, 2, filled_w, inner_h, bar_color);

    // "O2" label on the left
    let scale = (h / 6).max(2);
    let label_x = 8u32;
    let label_y = h / 2 - scale * 2;
    draw_block_char(&mut img, label_x, label_y, scale, b'O', [220, 220, 240, 220]);

    img
}
