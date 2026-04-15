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
) {
    let s = st.lock().unwrap();
    let zone_idx = zone_index_for_distance(s.distance);
    let dark = s.dark_mode;
    let flipped = s.gravity_dir < 0.0;
    drop(s);
    let vivid = matches!(c.get_var("bg_vivid"), Some(Value::Bool(true)));

    let key = (dark, zone_idx, vivid, flipped);
    if *prev_bg_theme == Some(key) { return; }
    *prev_bg_theme = Some(key);

    let image_data: image::RgbaImage = if dark {
        // Dark mode: solid near-black
        let mut img = image::RgbaImage::new(4, 4);
        for py in 0..4 { for px in 0..4 { img.put_pixel(px, py, image::Rgba([4, 4, 8, 255])); } }
        img
    } else if flipped {
        // Gravity inverted — use vertically flipped backgrounds
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
