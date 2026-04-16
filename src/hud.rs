use crate::constants::*;
use crate::images::*;
use std::sync::OnceLock;

fn hud_coin_icon() -> &'static image::RgbaImage {
    static ICON: OnceLock<image::RgbaImage> = OnceLock::new();
    ICON.get_or_init(|| {
        let fallback = || circle_img(18, C_COIN.0, C_COIN.1, C_COIN.2);
        let decoded = image::load_from_memory_with_format(
            include_bytes!("../assets/coin.gif"),
            image::ImageFormat::Gif,
        )
        .map(|img| img.to_rgba8())
        .unwrap_or_else(|_| fallback());

        image::imageops::resize(
            &decoded,
            48,
            48,
            image::imageops::FilterType::CatmullRom,
        )
    })
}

pub fn coin_counter_img(count: u32) -> image::RgbaImage {
    let w = 420;
    let h = 98;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    image::imageops::overlay(&mut img, hud_coin_icon(), 16, 25);

    let clamped = count.min(9999);
    let digits = format!("{:04}", clamped);
    let mut dx = 100;
    for ch in digits.bytes() {
        draw_digit_7seg(&mut img, dx, 17, 3, (ch - b'0') as u8, [250, 250, 255, 255]);
        dx += 56;
    }
    img
}

pub fn momentum_counter_img(momentum: f32) -> image::RgbaImage {
    let w = 420;
    let h = 86;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    let m = momentum.clamp(0.0, 999.0).round() as u32;
    let digits = format!("{:03}", m);
    let mut dx = 210;
    for ch in digits.bytes() {
        draw_digit_7seg(&mut img, dx, 17, 3, (ch - b'0') as u8, [250, 250, 255, 255]);
        dx += 56;
    }

    let fill = (momentum / MOMENTUM_CAP).clamp(0.0, 1.0);
    let meter = bar_img(168, 22, fill, 255, 170, 90);
    image::imageops::overlay(&mut img, &meter, 18, 32);
    img
}

pub fn x_counter_img(x: f32) -> image::RgbaImage {
    let w = 420;
    let h = 86;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    draw_rect(&mut img, 18, 16, 14, 14, [255, 180, 100, 255]);
    draw_rect(&mut img, 46, 16, 14, 14, [255, 180, 100, 255]);
    draw_rect(&mut img, 32, 30, 14, 14, [255, 180, 100, 255]);
    draw_rect(&mut img, 18, 44, 14, 14, [255, 180, 100, 255]);
    draw_rect(&mut img, 46, 44, 14, 14, [255, 180, 100, 255]);

    let x_int = x.round().clamp(-99999.0, 99999.0) as i32;
    let text = format!("{:+06}", x_int);
    let mut dx = 96;
    for ch in text.bytes() {
        if ch == b'-' {
            draw_rect(&mut img, dx + 10, 42, 24, 5, [250, 250, 255, 255]);
            dx += 40;
        } else if ch == b'+' {
            draw_rect(&mut img, dx + 10, 42, 24, 5, [250, 250, 255, 255]);
            draw_rect(&mut img, dx + 20, 30, 5, 28, [250, 250, 255, 255]);
            dx += 40;
        } else if ch.is_ascii_digit() {
            draw_digit_7seg(&mut img, dx, 17, 3, (ch - b'0') as u8, [250, 250, 255, 255]);
            dx += 40;
        }
    }
    img
}

pub fn y_counter_img(y: f32) -> image::RgbaImage {
    let w = 420;
    let h = 86;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    draw_rect(&mut img, 18, 16, 14, 34, [120, 220, 255, 255]);
    draw_rect(&mut img, 32, 16, 14, 17, [120, 220, 255, 255]);
    draw_rect(&mut img, 32, 33, 14, 17, [120, 220, 255, 255]);
    draw_rect(&mut img, 46, 33, 14, 17, [120, 220, 255, 255]);
    draw_rect(&mut img, 46, 50, 14, 17, [120, 220, 255, 255]);

    let y_int = y.round().clamp(-9999.0, 9999.0) as i32;
    let text = format!("{:+05}", y_int);
    let mut dx = 120;
    for ch in text.bytes() {
        if ch == b'-' {
            draw_rect(&mut img, dx + 10, 42, 24, 5, [250, 250, 255, 255]);
            dx += 46;
        } else if ch == b'+' {
            draw_rect(&mut img, dx + 10, 42, 24, 5, [250, 250, 255, 255]);
            draw_rect(&mut img, dx + 20, 30, 5, 28, [250, 250, 255, 255]);
            dx += 46;
        } else if ch.is_ascii_digit() {
            draw_digit_7seg(&mut img, dx, 17, 3, (ch - b'0') as u8, [250, 250, 255, 255]);
            dx += 46;
        }
    }
    img
}

pub fn gravity_indicator_img(flipped: bool, enabled: bool) -> image::RgbaImage {
    let w = 308;
    let h = 84;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    let active = if enabled { [255, 245, 120, 255] } else { [120, 120, 120, 255] };
    let idle = [40, 45, 58, 220];
    if flipped {
        draw_rect(&mut img, 22, 10, 264, 26, active);
        draw_rect(&mut img, 22, 48, 264, 26, idle);
    } else {
        draw_rect(&mut img, 22, 10, 264, 26, idle);
        draw_rect(&mut img, 22, 48, 264, 26, active);
    }
    img
}

/// Flip timer HUD: shows remaining seconds as a countdown with a depleting bar.
/// `remaining_ticks` is the number of ticks left; `total_ticks` is the max (e.g. 600).
pub fn flip_timer_img(remaining_ticks: u32, total_ticks: u32) -> image::RgbaImage {
    let w = 504u32;
    let h = 118u32;
    let mut img = image::RgbaImage::new(w, h);
    // Background panel
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 220]);
    draw_rect(&mut img, 0, 0, w, 2, [255, 245, 120, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [255, 245, 120, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [255, 245, 120, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [255, 245, 120, 255]);

    // Gravity flip icon (up arrow)
    draw_rect(&mut img, 30, 42, 6, 28, [255, 245, 120, 255]);
    draw_rect(&mut img, 20, 50, 28, 6, [255, 245, 120, 255]);
    draw_rect(&mut img, 24, 44, 18, 6, [255, 245, 120, 255]);
    draw_rect(&mut img, 30, 38, 6, 6, [255, 245, 120, 255]);

    // Seconds remaining (2 digits, e.g. "10", "09", ...)
    let secs = (remaining_ticks + 59) / 60; // ceil to avoid showing 0 while active
    let secs = secs.min(99);
    let tens = (secs / 10) as u8;
    let ones = (secs % 10) as u8;
    draw_digit_7seg(&mut img, 84, 17, 3, tens, [255, 245, 120, 255]);
    draw_digit_7seg(&mut img, 140, 17, 3, ones, [255, 245, 120, 255]);

    // "S" label
    draw_rect(&mut img, 207, 20, 28, 6, [200, 200, 210, 255]);
    draw_rect(&mut img, 207, 20, 6, 17, [200, 200, 210, 255]);
    draw_rect(&mut img, 207, 37, 28, 6, [200, 200, 210, 255]);
    draw_rect(&mut img, 229, 37, 6, 17, [200, 200, 210, 255]);
    draw_rect(&mut img, 207, 54, 28, 6, [200, 200, 210, 255]);

    // Depleting bar
    let fill = if total_ticks > 0 { remaining_ticks as f32 / total_ticks as f32 } else { 0.0 };
    let bar = bar_img(420, 20, fill, 255, 245, 120);
    image::imageops::overlay(&mut img, &bar, 42, 87);

    img
}
