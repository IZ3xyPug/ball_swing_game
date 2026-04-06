use crate::constants::*;
use crate::images::*;

pub fn coin_counter_img(count: u32) -> image::RgbaImage {
    let w = 300;
    let h = 70;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    let mut coin = circle_img(18, C_COIN.0, C_COIN.1, C_COIN.2);
    image::imageops::overlay(&mut img, &coin, 12, 17);
    coin = circle_img(8, 40, 20, 45);
    image::imageops::overlay(&mut img, &coin, 22, 27);

    let clamped = count.min(9999);
    let digits = format!("{:04}", clamped);
    let mut dx = 90;
    for ch in digits.bytes() {
        draw_digit_7seg(&mut img, dx, 12, 2, (ch - b'0') as u8, [250, 250, 255, 255]);
        dx += 42;
    }
    img
}

pub fn momentum_counter_img(momentum: f32) -> image::RgbaImage {
    let w = 300;
    let h = 62;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    let m = momentum.clamp(0.0, 999.0).round() as u32;
    let digits = format!("{:03}", m);
    let mut dx = 152;
    for ch in digits.bytes() {
        draw_digit_7seg(&mut img, dx, 12, 2, (ch - b'0') as u8, [250, 250, 255, 255]);
        dx += 42;
    }

    let fill = (momentum / MOMENTUM_CAP).clamp(0.0, 1.0);
    let meter = bar_img(120, 16, fill, 255, 170, 90);
    image::imageops::overlay(&mut img, &meter, 14, 23);
    img
}

pub fn x_counter_img(x: f32) -> image::RgbaImage {
    let w = 300;
    let h = 62;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    draw_rect(&mut img, 14, 12, 10, 10, [255, 180, 100, 255]);
    draw_rect(&mut img, 34, 12, 10, 10, [255, 180, 100, 255]);
    draw_rect(&mut img, 24, 22, 10, 10, [255, 180, 100, 255]);
    draw_rect(&mut img, 14, 32, 10, 10, [255, 180, 100, 255]);
    draw_rect(&mut img, 34, 32, 10, 10, [255, 180, 100, 255]);

    let x_int = x.round().clamp(-99999.0, 99999.0) as i32;
    let text = format!("{:+06}", x_int);
    let mut dx = 72;
    for ch in text.bytes() {
        if ch == b'-' {
            draw_rect(&mut img, dx + 8, 30, 18, 4, [250, 250, 255, 255]);
            dx += 30;
        } else if ch == b'+' {
            draw_rect(&mut img, dx + 8, 30, 18, 4, [250, 250, 255, 255]);
            draw_rect(&mut img, dx + 15, 22, 4, 20, [250, 250, 255, 255]);
            dx += 30;
        } else if ch.is_ascii_digit() {
            draw_digit_7seg(&mut img, dx, 12, 2, (ch - b'0') as u8, [250, 250, 255, 255]);
            dx += 30;
        }
    }
    img
}

pub fn y_counter_img(y: f32) -> image::RgbaImage {
    let w = 300;
    let h = 62;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    draw_rect(&mut img, 14, 12, 10, 24, [120, 220, 255, 255]);
    draw_rect(&mut img, 24, 12, 10, 12, [120, 220, 255, 255]);
    draw_rect(&mut img, 24, 24, 10, 12, [120, 220, 255, 255]);
    draw_rect(&mut img, 34, 24, 10, 12, [120, 220, 255, 255]);
    draw_rect(&mut img, 34, 36, 10, 12, [120, 220, 255, 255]);

    let y_int = y.round().clamp(-9999.0, 9999.0) as i32;
    let text = format!("{:+05}", y_int);
    let mut dx = 92;
    for ch in text.bytes() {
        if ch == b'-' {
            draw_rect(&mut img, dx + 8, 30, 18, 4, [250, 250, 255, 255]);
            dx += 34;
        } else if ch == b'+' {
            draw_rect(&mut img, dx + 8, 30, 18, 4, [250, 250, 255, 255]);
            draw_rect(&mut img, dx + 15, 22, 4, 20, [250, 250, 255, 255]);
            dx += 34;
        } else if ch.is_ascii_digit() {
            draw_digit_7seg(&mut img, dx, 12, 2, (ch - b'0') as u8, [250, 250, 255, 255]);
            dx += 34;
        }
    }
    img
}

pub fn gravity_indicator_img(flipped: bool, enabled: bool) -> image::RgbaImage {
    let w = 220;
    let h = 60;
    let mut img = image::RgbaImage::new(w, h);
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 210]);
    draw_rect(&mut img, 0, 0, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [170, 170, 190, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [170, 170, 190, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [170, 170, 190, 255]);

    let active = if enabled { [255, 245, 120, 255] } else { [120, 120, 120, 255] };
    let idle = [40, 45, 58, 220];
    if flipped {
        draw_rect(&mut img, 16, 8, 188, 18, active);
        draw_rect(&mut img, 16, 34, 188, 18, idle);
    } else {
        draw_rect(&mut img, 16, 8, 188, 18, idle);
        draw_rect(&mut img, 16, 34, 188, 18, active);
    }
    img
}

/// Flip timer HUD: shows remaining seconds as a countdown with a depleting bar.
/// `remaining_ticks` is the number of ticks left; `total_ticks` is the max (e.g. 600).
pub fn flip_timer_img(remaining_ticks: u32, total_ticks: u32) -> image::RgbaImage {
    let w = 360u32;
    let h = 84u32;
    let mut img = image::RgbaImage::new(w, h);
    // Background panel
    draw_rect(&mut img, 0, 0, w, h, [15, 18, 28, 220]);
    draw_rect(&mut img, 0, 0, w, 2, [255, 245, 120, 255]);
    draw_rect(&mut img, 0, h - 2, w, 2, [255, 245, 120, 255]);
    draw_rect(&mut img, 0, 0, 2, h, [255, 245, 120, 255]);
    draw_rect(&mut img, w - 2, 0, 2, h, [255, 245, 120, 255]);

    // Gravity flip icon (up arrow)
    draw_rect(&mut img, 22, 30, 4, 20, [255, 245, 120, 255]);
    draw_rect(&mut img, 14, 36, 20, 4, [255, 245, 120, 255]);
    draw_rect(&mut img, 18, 32, 12, 4, [255, 245, 120, 255]);
    draw_rect(&mut img, 22, 28, 4, 4, [255, 245, 120, 255]);

    // Seconds remaining (2 digits, e.g. "10", "09", ...)
    let secs = (remaining_ticks + 59) / 60; // ceil to avoid showing 0 while active
    let secs = secs.min(99);
    let tens = (secs / 10) as u8;
    let ones = (secs % 10) as u8;
    draw_digit_7seg(&mut img, 60, 12, 2, tens, [255, 245, 120, 255]);
    draw_digit_7seg(&mut img, 102, 12, 2, ones, [255, 245, 120, 255]);

    // "S" label
    draw_rect(&mut img, 148, 14, 20, 4, [200, 200, 210, 255]);
    draw_rect(&mut img, 148, 14, 4, 12, [200, 200, 210, 255]);
    draw_rect(&mut img, 148, 26, 20, 4, [200, 200, 210, 255]);
    draw_rect(&mut img, 164, 26, 4, 12, [200, 200, 210, 255]);
    draw_rect(&mut img, 148, 38, 20, 4, [200, 200, 210, 255]);

    // Depleting bar
    let fill = if total_ticks > 0 { remaining_ticks as f32 / total_ticks as f32 } else { 0.0 };
    let bar = bar_img(300, 14, fill, 255, 245, 120);
    image::imageops::overlay(&mut img, &bar, 30, 62);

    img
}
