use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

/// Update background gradient when zone, vivid state, or gravity direction changes.
pub fn tick_background(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    prev_bg_theme: &mut Option<(bool, usize, bool, bool, bool)>,
    bg_scale_smooth: &mut f32,
    bg_zone_start: &image::RgbaImage,
    bg_zone_start_vivid: &image::RgbaImage,
    bg_zone_start_flip: &image::RgbaImage,
    bg_zone_start_vivid_flip: &image::RgbaImage,
) {
    let s = st.lock().unwrap();
    let dark = s.dark_mode;
    let flipped = s.gravity_dir < 0.0;
    let py = s.py;
    let in_space_mode = s.in_space_mode;
    drop(s);
    let vivid = c.get_bool("bg_vivid");

    // Parallax panels (bg_space, bg_stars_b) are fully managed by the parallax block.
    if in_space_mode {
        // bg is the solid dark backdrop; star panels overlay it.
        if let Some(obj) = c.get_game_object_mut("bg") {
            const OVERSCAN: f32 = 200.0;
            const BG_RAISE: f32 = 150.0;
            obj.size = (VW + OVERSCAN * 2.0, VH + BG_RAISE);
            obj.position = (-OVERSCAN, -BG_RAISE);
            obj.visible = true;
            obj.update_image_shape();
        }
    } else {
        // Outside space: the sky zooms out with the camera (a share of it, see
        // `sky_scale`), about the same edge the camera's height zoom holds
        // still: the screen's bottom, or its top when gravity is flipped. More
        // sky then shows on the far side, which `nebula.rs` extends.
        // This frame's zoom, so the sky does not trail the world by a frame.
        let _ = py;
        let s = sky_scale(super::decor::frame_view_y(c).1);
        *bg_scale_smooth = s;

        if let Some(obj) = c.get_game_object_mut("bg") {
            let (x, y, w, h) = sky_rect(s, flipped);
            obj.size = (w, h);
            obj.position = (x, y);
            obj.update_image_shape();
        }
    }

    // Asteroid: fixed size, anchored to top-right corner (does not scale with bg zoom).
    {
        const BASE_W: f32 = 480.0;
        const BASE_H: f32 = 480.0;
        const MARGIN: f32 = 80.0;
        if let Some(obj) = c.get_game_object_mut("asteroid") {
            obj.size = (BASE_W, BASE_H);
            obj.position = (VW - BASE_W - MARGIN, MARGIN);
            obj.update_image_shape();
        }
    }

    // Key includes in_space_mode so background updates on space entry/exit.
    let bg_sel = match c.get_var("player_bg_selected") {
        Some(Value::I32(v)) => v.max(0) as usize,
        _ => 0,
    }.min(4);
    let key = (dark, bg_sel, vivid, flipped, in_space_mode);
    if *prev_bg_theme == Some(key) { return; }
    *prev_bg_theme = Some(key);

    if in_space_mode {
        // Space: bg gets a solid near-black image.
        let dark_img = image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 1, 6, 255]));
        if let Some(obj) = c.get_game_object_mut("bg") {
            obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: dark_img.into(), color: None });
        }
    } else {
        // Normal zone: aurora gradient background.
        let image_data: image::RgbaImage = if dark {
            let mut img = image::RgbaImage::new(4, 4);
            for py in 0..4 { for px in 0..4 { img.put_pixel(px, py, image::Rgba([4, 4, 8, 255])); } }
            img
        } else if flipped {
            if vivid { bg_zone_start_vivid_flip.clone() } else { bg_zone_start_flip.clone() }
        } else if vivid {
            bg_zone_start_vivid.clone()
        } else {
            bg_zone_start.clone()
        };
        if let Some(obj) = c.get_game_object_mut("bg") {
            obj.visible = true;
            obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: image_data.into(), color: None });
        }
    }

}

/// Where the sky is this frame, for things that live in it (planets, the
/// nebula): positions in the sky image's own px (VW x VH; row 0 is its top
/// edge, negative rows are above it), mapped through `bg`'s current rect so
/// they zoom with the sky. Read after `tick_background` has placed `bg`.
#[derive(Clone, Copy, Debug)]
pub struct SkyFrame {
    x: f32, y: f32,
    /// Screen px per sky px, now and at normal zoom.
    sx: f32, sy: f32, sx0: f32,
    /// The sky's scale: 1 at normal zoom, smaller zoomed out.
    pub s: f32,
    flipped: bool,
}

impl SkyFrame {
    pub fn to_screen(&self, u: f32, v: f32) -> (f32, f32) {
        // Flipped, the sky image is drawn upside down.
        let row = if self.flipped { VH - v } else { v };
        (self.x + u * self.sx, self.y + row * self.sy)
    }
    pub fn to_sky(&self, px: f32, py: f32) -> (f32, f32) {
        let row = (py - self.y) / self.sy;
        ((px - self.x) / self.sx, if self.flipped { VH - row } else { row })
    }
    /// Screen px at normal zoom to sky px (for speeds authored on screen).
    pub fn sky_px(&self, screen_px_at_normal_zoom: f32) -> f32 {
        screen_px_at_normal_zoom / self.sx0
    }
}

pub fn sky_frame(c: &Canvas, flipped: bool) -> Option<SkyFrame> {
    let o = c.get_game_object("bg")?;
    let (w0, _) = { let r = sky_rect(1.0, flipped); (r.2, r.3) };
    let ((x, y), (w, h)) = (o.position, o.size);
    Some(SkyFrame { x, y, sx: w / VW, sy: h / VH, sx0: w0 / VW, s: w / w0, flipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spot_in_the_sky_maps_to_the_screen_and_back() {
        for flipped in [false, true] {
            for s in [1.0, 0.85, sky_scale(SKY_ZOOM_FLOOR)] {
                let (x, y, w, h) = sky_rect(s, flipped);
                let w0 = sky_rect(1.0, flipped).2;
                let f = SkyFrame { x, y, sx: w / VW, sy: h / VH, sx0: w0 / VW, s: w / w0, flipped };
                assert!((f.s - s).abs() < 1e-4);
                for (u, v) in [(0.0, 0.0), (1_000.0, -400.0), (3_000.0, 900.0)] {
                    let (px, py) = f.to_screen(u, v);
                    let (u2, v2) = f.to_sky(px, py);
                    assert!((u - u2).abs() < 1e-2 && (v - v2).abs() < 1e-2, "{flipped} {s}: ({u},{v}) -> ({u2},{v2})");
                }
            }
        }
    }
}
