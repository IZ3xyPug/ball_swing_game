//! The OCEAN WORLD's sea, moving.
//!
//! The sky picture holds the sea still; over it this lays the sea's band
//! (from just under the horizon to the bottom), one frame after another
//! through a loop of its swell (`examples/make_planets.rs` renders them, and
//! frame 0 is the still picture, so the two agree). The frames are tinted
//! towards the background's colour as the sky's planet is, and placed exactly
//! where the band sits in the sky picture as the sky object draws it.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;

const ID: &str = "ocean_water";
/// The band's first row in the sky picture (see the generator).
const BAND_TOP: f32 = 1790.0;
/// Ticks each frame shows (16 frames: a loop of about 1.3 s).
const TICKS_PER_FRAME: u32 = 5;

macro_rules! frames { ($($n:literal),*) => { [$(include_bytes!(concat!("../../../assets/planets/ocean_water_", $n, ".jpg")) as &[u8]),*] }; }
const FRAMES: [&[u8]; 16] = frames!("00", "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "14", "15");

type Frames = Arc<Vec<Arc<image::RgbaImage>>>;

/// The frames for a background colour, decoded and tinted off the main
/// thread; `None` until they are ready.
fn frames_for(bg: (u8, u8, u8), tint: f32) -> Option<Frames> {
    static READY: OnceLock<Mutex<Option<((u8, u8, u8), Frames)>>> = OnceLock::new();
    static BUILDING: Mutex<Option<(u8, u8, u8)>> = Mutex::new(None);
    let ready = READY.get_or_init(|| Mutex::new(None));
    if let Some((k, f)) = ready.lock().unwrap().as_ref() {
        if *k == bg { return Some(f.clone()); }
    }
    let mut b = BUILDING.lock().unwrap();
    if *b != Some(bg) {
        *b = Some(bg);
        std::thread::spawn(move || {
            let frames: Vec<Arc<image::RgbaImage>> = FRAMES.iter().filter_map(|bytes| {
                let mut img = image::load_from_memory(bytes).ok()?.to_rgba8();
                for p in img.pixels_mut() {
                    p[0] = (p[0] as f32 * (1.0 - tint) + bg.0 as f32 * tint) as u8;
                    p[1] = (p[1] as f32 * (1.0 - tint) + bg.1 as f32 * tint) as u8;
                    p[2] = (p[2] as f32 * (1.0 - tint) + bg.2 as f32 * tint) as u8;
                }
                Some(Arc::new(img))
            }).collect();
            *READY.get_or_init(|| Mutex::new(None)).lock().unwrap() = Some((bg, Arc::new(frames)));
        });
    }
    None
}

/// Start decoding ahead of the run.
pub fn prewarm(bg: (u8, u8, u8)) { let _ = frames_for(bg, crate::sky::planet_tint(ocean())); }

/// The OCEAN WORLD's place in `sky::PLANETS` (looked up by id, so the list
/// can be reordered).
fn ocean() -> usize { crate::sky::planet_by_id("ocean").unwrap_or(usize::MAX) }

pub fn objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    let mut o = GameObject::new_rect(ctx, ID.to_string(),
        Some(Image { shape: ShapeType::Rectangle(0.0, (8.0, 8.0), 0.0), image: crate::images::solid(0, 0, 0, 0).into(), color: None }),
        (8.0, 8.0), (-9000.0, -9000.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    o.ignore_zoom = true;
    o.visible = false;
    vec![(ID.to_string(), o)]
}

pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (flipped, off) = {
        let s = st.lock().unwrap();
        (s.gravity_dir < 0.0, s.in_space_mode || s.space_launch_active || s.dark_mode)
    };
    let shown = c.get_i32("sky_planet_shown").max(0) as usize;
    let bg_sel = (c.get_i32("player_bg_selected").max(0) as usize).min(crate::shop::SHOP_BG_COLORS.len() - 1);
    let frames = (shown == ocean() && !off && !c.get_bool("bg_vivid"))
        .then(|| frames_for(crate::shop::SHOP_BG_COLORS[bg_sel], crate::sky::planet_tint(ocean()))).flatten();
    let Some((ox, oy, sc)) = super::background::sky_picture_rect(c) else { return };
    let Some(frames) = frames.filter(|f| !f.is_empty()) else {
        if let Some(o) = c.get_game_object_mut(ID) { o.visible = false; }
        return;
    };
    let t = c.get_i32("ocean_water_t").wrapping_add(1);
    c.set_var("ocean_water_t", t);
    let img = frames[(t as u32 / TICKS_PER_FRAME) as usize % frames.len()].clone();
    // Exactly where the sky draws the picture's rows (see `sky_picture_rect`).
    let (r0, r1) = if flipped { (0.0, VH - BAND_TOP) } else { (BAND_TOP, VH) };
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    if let Some(o) = c.get_game_object_mut(ID) {
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, (iw, ih), 0.0), image: img, color: None });
        o.position = (ox, oy + r0 * sc);
        o.size = (VW * sc, (r1 - r0) * sc);
        o.rotation = if flipped { 180.0 } else { 0.0 };
        o.visible = true;
    }
}
