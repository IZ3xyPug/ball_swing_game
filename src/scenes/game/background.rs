use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::zone_index_for_distance;
use crate::state::*;

/// Update background gradient when zone, vivid state, or gravity direction changes.
pub fn tick_background(
    c: &mut Canvas,
    st: &Arc<Mutex<State>>,
    prev_bg_theme: &mut Option<(bool, usize, bool, bool)>,
    bg_scale_smooth: &mut f32,
    bg_zone_start: &image::RgbaImage,
    bg_zone_purple: &image::RgbaImage,
    bg_zone_black: &image::RgbaImage,
    bg_zone_start_vivid: &image::RgbaImage,
    bg_zone_purple_vivid: &image::RgbaImage,
    bg_zone_black_vivid: &image::RgbaImage,
    bg_zone_start_flip: &image::RgbaImage,
    bg_zone_purple_flip: &image::RgbaImage,
    bg_zone_black_flip: &image::RgbaImage,
    bg_zone_start_vivid_flip: &image::RgbaImage,
    bg_zone_purple_vivid_flip: &image::RgbaImage,
    bg_zone_black_vivid_flip: &image::RgbaImage,
    _bg_zone_start_space: &image::RgbaImage,
    _bg_zone_purple_space: &image::RgbaImage,
    _bg_zone_black_space: &image::RgbaImage,
    _bg_zone_start_vivid_space: &image::RgbaImage,
    _bg_zone_purple_vivid_space: &image::RgbaImage,
    _bg_zone_black_vivid_space: &image::RgbaImage,
    _bg_zone_start_space_flip: &image::RgbaImage,
    _bg_zone_purple_space_flip: &image::RgbaImage,
    _bg_zone_black_space_flip: &image::RgbaImage,
    _bg_zone_start_vivid_space_flip: &image::RgbaImage,
    _bg_zone_purple_vivid_space_flip: &image::RgbaImage,
    _bg_zone_black_vivid_space_flip: &image::RgbaImage,
) {
    let s = st.lock().unwrap();
    let zone_idx = zone_index_for_distance(s.distance);
    let dark = s.dark_mode;
    let flipped = s.gravity_dir < 0.0;
    let py = s.py;
    drop(s);
    let vivid = matches!(c.get_var("bg_vivid"), Some(Value::Bool(true)));

    // Smoothly scale background once player rises above py = 500.
    // Starts at py = 500 (upper area of screen), reaches full effect at py = 500 - 1400 = -900.
    let up_t = if flipped {
        ((py - (VH - 500.0)) / 1400.0).clamp(0.0, 1.0)
    } else {
        ((500.0 - py) / 1400.0).clamp(0.0, 1.0)
    };
    let zoom_strength = match c.get_i32("space_zoom_mode") {
        4 => 0.44, // reduced version
        _ => 0.60, // default/current version
    };
    let target_scale = 1.0 + zoom_strength * up_t;
    // Lerp toward target so the transition eases rather than snapping each frame.
    *bg_scale_smooth = *bg_scale_smooth + (*bg_scale_smooth - target_scale).abs().min(1.0) * if target_scale > *bg_scale_smooth { 0.06 } else { 0.02 } * (target_scale - *bg_scale_smooth).signum();
    let bg_scale = *bg_scale_smooth;

    if let Some(obj) = c.get_game_object_mut("bg") {
        const OVERSCAN: f32 = 200.0;
        let w = VW * bg_scale + OVERSCAN * 2.0;
        let h = VH * bg_scale;
        obj.size = (w, h);
        let cx = -(w - VW) / 2.0;
        if flipped {
            obj.position = (cx, VH - h);
        } else {
            obj.position = (cx, 0.0);
        }
        obj.update_image_shape();
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

    // Disable overlay layer to avoid tint/whitening artifacts.
    if let Some(obj) = c.get_game_object_mut("bg_space") {
        obj.visible = false;
    }

    let key = (dark, zone_idx, vivid, flipped);
    if *prev_bg_theme == Some(key) { return; }
    *prev_bg_theme = Some(key);

    let image_data: image::RgbaImage = if dark {
        let mut img = image::RgbaImage::new(4, 4);
        for py in 0..4 { for px in 0..4 { img.put_pixel(px, py, image::Rgba([4, 4, 8, 255])); } }
        img
    } else if flipped {
        if vivid {
            match zone_idx {
                0 => bg_zone_start_vivid_flip.clone(),
                1 => bg_zone_purple_vivid_flip.clone(),
                _ => bg_zone_black_vivid_flip.clone(),
            }
        } else {
            match zone_idx {
                0 => bg_zone_start_flip.clone(),
                1 => bg_zone_purple_flip.clone(),
                _ => bg_zone_black_flip.clone(),
            }
        }
    } else if vivid {
        match zone_idx {
            0 => bg_zone_start_vivid.clone(),
            1 => bg_zone_purple_vivid.clone(),
            _ => bg_zone_black_vivid.clone(),
        }
    } else {
        match zone_idx {
            0 => bg_zone_start.clone(),
            1 => bg_zone_purple.clone(),
            _ => bg_zone_black.clone(),
        }
    };

    if let Some(obj) = c.get_game_object_mut("bg") {
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
            image: image_data.into(),
            color: None,
        });
    }

}
