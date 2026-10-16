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
    bg_zone_start: &Arc<image::RgbaImage>,
    bg_zone_start_vivid: &Arc<image::RgbaImage>,
    bg_zone_start_flip: &Arc<image::RgbaImage>,
    bg_zone_start_vivid_flip: &Arc<image::RgbaImage>,
) {
    let s = st.lock().unwrap();
    let dark = s.dark_mode;
    let flipped = s.gravity_dir < 0.0;
    let py = s.py;
    let in_space_mode = s.in_space_mode;
    let boss = s.boss_active;
    drop(s);
    // Boss fights: the planet sinks so only its glowing atmosphere shows
    // along the bottom. Eased in and out, never a jump.
    let drop_target = if boss { BOSS_SKY_DROP } else { 0.0 };
    let sky_drop = c.get_f32("boss_sky_drop");
    let sky_drop = sky_drop + (drop_target - sky_drop) * 0.04;
    c.set_var("boss_sky_drop", sky_drop);
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
        let (view_y, zoom) = super::decor::frame_view_y(c);
        let s = sky_scale(zoom);
        *bg_scale_smooth = s;
        // Past the camera's furthest zoom-out it follows the climb, and the
        // world falls away below; the sky used to just stop. It now sinks
        // too, by the height climbed beyond that point, so going higher still
        // reads as going higher (the planet drops away, the nebula comes
        // down into view). Nothing in the sky moves before then.
        let _ = view_y;
        let target = if flipped { 0.0 } else { sky_climb_shift(py, zoom) };
        // Eased, so the swing's own bob never jerks the sky about.
        let climb = c.get_f32("sky_climb_smooth");
        let climb = climb + (target - climb) * 0.12;
        c.set_var("sky_climb_smooth", climb);
        let drop = sky_drop + climb;

        if let Some(obj) = c.get_game_object_mut("bg") {
            let (x, y, w, h) = sky_rect(s, flipped);
            obj.size = (w, h);
            obj.position = (x, y + if flipped { -drop } else { drop });
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
    // The planet is part of the sky picture: a new one re-applies it.
    let planet = c.get_i32("sky_planet_shown").max(0) as usize;
    let key = (dark, bg_sel + planet * 100, vivid, flipped, in_space_mode);
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
        let image_data: Arc<image::RgbaImage> = if dark {
            Arc::new(image::RgbaImage::from_pixel(4, 4, image::Rgba([4, 4, 8, 255])))
        } else if flipped {
            if vivid { bg_zone_start_vivid_flip.clone() } else { bg_zone_start_flip.clone() }
        } else if vivid {
            bg_zone_start_vivid.clone()
        } else {
            bg_zone_start.clone()
        };
        if let Some(obj) = c.get_game_object_mut("bg") {
            obj.visible = true;
            obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: image_data, color: None });
        }
    }

}

/// How far (screen px) the sky has sunk for a player at world height `py`:
/// none below the height where the camera reaches its furthest zoom-out, then
/// by the height climbed past it. Measured from the player, so it starts from
/// exactly nothing at that height (measured from the camera, which follows
/// the player all along, it jumped in the moment the zoom hit its limit).
pub fn sky_climb_shift(py: f32, zoom: f32) -> f32 {
    // The height zoom is (VH - margin) / height above the ground line.
    let at_limit = (VH - ZOOM_TOP_MARGIN) * ZOOM_MAX;
    let above = (VH - py - at_limit).max(0.0);
    (above * zoom * SKY_CLIMB_PARALLAX).min(SKY_CLIMB_MAX)
}

/// Where the sky PICTURE actually lands on screen: the renderer fills an
/// object's box by scaling its image to COVER it, centred, and the sky's box
/// (`sky_rect`) is a little wider than the picture, so the picture overflows
/// it top and bottom. Returns (left, top, scale): picture px (u, v) is at
/// (left + u * scale, top + v * scale). Anything that must sit exactly on
/// something baked into the picture (the planet's horizon) maps through this.
pub fn sky_picture_rect(c: &Canvas) -> Option<(f32, f32, f32)> {
    let o = c.get_game_object("bg")?;
    Some(cover_rect(o.position, o.size))
}

/// Where a VW x VH picture lands when scaled to cover the box at `pos` of
/// `size`, centred: (left, top, scale).
pub fn cover_rect(pos: (f32, f32), size: (f32, f32)) -> (f32, f32, f32) {
    let sc = (size.0 / VW).max(size.1 / VH);
    (pos.0 + (size.0 - VW * sc) * 0.5, pos.1 + (size.1 - VH * sc) * 0.5, sc)
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
    fn the_sky_picture_covers_its_box_centred() {
        for s in [1.0, 0.85, sky_scale(SKY_ZOOM_FLOOR)] {
            let (x, y, w, h) = sky_rect(s, false);
            let (ox, oy, sc) = cover_rect((x, y), (w, h));
            // Covers: no gap on any side.
            assert!(ox <= x + 0.01 && oy <= y + 0.01);
            assert!(ox + VW * sc >= x + w - 0.01 && oy + VH * sc >= y + h - 0.01);
            // Centred, and fills one dimension exactly.
            assert!(((ox + VW * sc * 0.5) - (x + w * 0.5)).abs() < 0.01);
            assert!(((oy + VH * sc * 0.5) - (y + h * 0.5)).abs() < 0.01);
            assert!((VW * sc - w).abs() < 0.01 || (VH * sc - h).abs() < 0.01);
        }
    }

    #[test]
    fn the_sky_sinks_only_past_the_zoom_limit_and_starts_from_nothing() {
        let z = 1.0 / ZOOM_MAX;
        // The player height at which the camera reaches its furthest zoom.
        let limit_py = VH - (VH - ZOOM_TOP_MARGIN) * ZOOM_MAX;
        assert_eq!(sky_climb_shift(limit_py + 500.0, z), 0.0, "below the limit");
        assert_eq!(sky_climb_shift(limit_py, z), 0.0, "at the limit");
        // A little past it, a little sink: no jump on crossing.
        let just = sky_climb_shift(limit_py - 10.0, z);
        assert!(just > 0.0 && just < 5.0, "{just}");
        // More with height, never past the cap.
        let mut last = 0.0;
        for k in 1..200 {
            let v = sky_climb_shift(limit_py - k as f32 * 400.0, z);
            assert!(v >= last && v <= SKY_CLIMB_MAX);
            last = v;
        }
    }

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
