//! The sky above the normal view, and the nebula that lives in it.
//!
//! The sky (`bg`) zooms out with the camera (see `sky_scale` and
//! `tick_background`), anchored at the screen's bottom edge, so zooming out
//! shows more sky above the usual view. This module supplies that sky: a dark
//! base (`bg_sky_ext`) and the same stars (`bg_sky_ext_stars`), continuing the
//! sky image past its top edge. The nebula sits in it, in sky coordinates,
//! above the normal view: hidden at normal zoom, it comes into view as the sky
//! zooms out and shrinks with the sky, like everything else in it. Like the
//! planets it slides sideways only as the player travels forward (never
//! backward, never on its own), slowly, as something very far away; two copies
//! side by side wrap seamlessly.
//!
//! Everything is placed in the sky image's own units (the image is VW x VH;
//! row 0 is its top edge, negative rows are above it) and mapped to the
//! screen through `bg`'s current rect, so it all zooms as one scene.
//!
//! The nebula is the backmost thing in the sky: the static stars are baked
//! into the sky image, so a transparent copy of them (`bg_star_overlay`) is
//! drawn over the nebula, lined up with the sky, as the extension's stars are.
//!
//! F cycles which set is shown (a look-comparison toggle), except while the
//! cannon's hyper-transit prompt is up, where F means "accept".

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;

/// The sets F cycles through, in order; the last is none. Each image is a
/// transparent 1920x1080 overlay (half the virtual screen), stretched to fit.
/// How a set's image is laid into the sky.
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum Fit {
    /// A wide strip at its own proportions, its bottom edge at
    /// `NEBULA_STRIP_BOTTOM_ROW`: shaped to the band of sky a zoom-out reveals.
    Strip,
    /// A full 16:9 layer whose clouds (8%..60% of its height) are stretched
    /// between `NEBULA_CLOUDS_TOP_ROW` and `NEBULA_CLOUDS_BOTTOM_ROW`.
    Band,
}

/// The one nebula, and "off" (F toggles between them). The others that
/// were tried live in `assets/nebula/old/`.
pub const NEBULA_SETS: [(&str, Option<&[u8]>, Fit); 2] = [
    ("mixed", Some(include_bytes!("../../../assets/nebula/mixed.png")), Fit::Band),
    ("off",   None, Fit::Band),
];
const NEBULA_SET_VAR: &str = "nebula_set";
/// The sky a run starts with: "mixed".
const NEBULA_DEFAULT: i32 = 0;

/// The set showing (F cycles it).
fn nebula_set(c: &Canvas) -> usize {
    match c.get_var(NEBULA_SET_VAR) {
        Some(Value::I32(i)) => (i.max(0) as usize).min(NEBULA_SETS.len() - 1),
        _ => NEBULA_DEFAULT as usize,
    }
}
const NEBULA_IDS: [&str; 2] = ["nebula", "nebula_b"];
const STAR_OVERLAY_ID: &str = "bg_star_overlay";
const EXT_ID: &str = "bg_sky_ext";
const EXT_STARS_ID: &str = "bg_sky_ext_stars";

/// Where the clouds sit, in sky-image rows (negative is above the image's top
/// edge). At normal zoom the screen's top edge is at row ~390 (see
/// `sky_rect`), so everything above that is out of sight; at the deepest sky
/// zoom-out the screen reaches up to row ~-270. Fit to that band so most of
/// the clouds can be seen (a taller layer showed only its bottom third); the
/// top runs a little past it. Near the image's own proportions.
pub const NEBULA_CLOUDS_TOP_ROW: f32 = -800.0;
pub const NEBULA_CLOUDS_BOTTOM_ROW: f32 = 300.0;
/// Sideways drift, in sky-image px per px of forward player travel. The
/// spiral galaxy, the farthest planet, moves ~0.017 screen px per px; the
/// nebulas are farther still.
pub const NEBULA_PARALLAX: f32 = 0.012;
/// Width, as a multiple of the sky image's (centred).
pub const NEBULA_WIDTH: f32 = 1.15;
/// Where a `Fit::Strip` set's bottom edge sits, in sky rows: just above the
/// screen's top edge at normal zoom (row ~390), so it is out of sight until
/// the sky zooms out; the strip's clouds then reach up to about row -270,
/// the top of the deepest zoom-out.
pub const NEBULA_STRIP_BOTTOM_ROW: f32 = 360.0;
/// Where the clouds start and fade out, as fractions of the nebula image's
/// height (the generator places them between these).
const CLOUDS_START: f32 = 0.08;
const CLOUDS_END: f32 = 0.60;
/// How far the extension reaches above the sky image, in rows (more than the
/// deepest zoom-out reveals).
const EXT_ROWS: f32 = VH;

/// Per-scene-visit state, kept by the game tick.
#[derive(Default, Clone)]
pub struct NebulaTick {
    f_was_down: bool,
    shown: Option<usize>,
    /// The (flipped, dark) the extension and overlays were last set up for.
    sky_look: Option<(bool, bool)>,
    /// How far the nebula has slid left, in sky-image px, 0..its width.
    scroll: f32,
    prev_px: Option<f32>,
    /// The current set's image height / width.
    image_aspect: f32,
    /// The strength last applied (quantised), so it is re-tinted only on change.
    alpha_q: Option<i32>,
}

/// The decoded image for one set. Only the current set is kept, so the others
/// are not sitting in memory; the Arc stays the same while the set does, so
/// the texture is uploaded once per switch.
fn set_image(i: usize) -> Option<Arc<image::RgbaImage>> {
    static CURRENT: OnceLock<Mutex<Option<(usize, Arc<image::RgbaImage>)>>> = OnceLock::new();
    let bytes = NEBULA_SETS[i].1?;
    let mut cur = CURRENT.get_or_init(|| Mutex::new(None)).lock().unwrap();
    if let Some((j, img)) = cur.as_ref() {
        if *j == i { return Some(img.clone()); }
    }
    let img = Arc::new(image::load_from_memory(bytes).ok()?.to_rgba8());
    *cur = Some((i, img.clone()));
    Some(img)
}

/// Rows of the sky image that carry stars: the pure starfield plus the blend
/// into the horizon gradient (see `composite_starfield_gradient`, split 0.56,
/// blend h/8 either side).
fn star_rows(h: u32) -> (u32, u32, u32) {
    let split = (h as f32 * 0.56) as u32;
    let blend = h / 8;
    (split.saturating_sub(blend), split + blend, h)
}

/// The sky's own stars, alone on a transparent image, cropped to the rows that
/// have them and faded through the horizon blend exactly as the sky fades
/// them. `flipped` matches the upside-down sky.
fn star_overlay(flipped: bool) -> Arc<image::RgbaImage> {
    static UP: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    static DOWN: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    let build = || {
        let (w, h) = (VW as u32, VH as u32);
        let field = quartz::star_field(w, h, STARFIELD_STAR_COUNT, 0xCAFE_BABE);
        let (pure_end, blend_end, _) = star_rows(h);
        let mut img = image::RgbaImage::new(w, blend_end);
        for y in 0..blend_end {
            let weight = if y < pure_end { 1.0 } else {
                1.0 - (y - pure_end) as f32 / (blend_end - pure_end) as f32
            };
            for x in 0..w {
                let p = field.image.get_pixel(x, y);
                // The field's empty sky is (5, 5, 15); anything brighter is star.
                if p[0] < 20 && p[1] < 20 && p[2] < 25 { continue; }
                img.put_pixel(x, y, image::Rgba([p[0], p[1], p[2], (255.0 * weight) as u8]));
            }
        }
        img
    };
    if flipped {
        DOWN.get_or_init(|| Arc::new(crate::images::flip_image_vertical(&star_overlay(false)))).clone()
    } else {
        UP.get_or_init(|| Arc::new(build())).clone()
    }
}

impl NebulaTick {
    /// Re-apply every image next tick. The tick outlives the scene (it is
    /// registered once), but re-entering the game scene rebuilds its objects
    /// blank, so images applied before would otherwise never come back.
    pub fn refresh(&mut self) {
        self.shown = None;
        self.sky_look = None;
    }
}

fn solid_arc(rgba: [u8; 4]) -> Arc<image::RgbaImage> {
    Arc::new(image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba)))
}

/// The extension's base: the star field's empty sky, or the dark-mode sky.
fn ext_base(dark: bool) -> Arc<image::RgbaImage> {
    static NIGHT: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    static DARK: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    if dark { DARK.get_or_init(|| solid_arc([4, 4, 8, 255])).clone() }
    else { NIGHT.get_or_init(|| solid_arc([5, 5, 15, 255])).clone() }
}

/// In draw order: the extension's base, the nebula, then the sky's stars and
/// the extension's stars over it. Screen space like the rest of the backdrop.
pub fn nebula_objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    [EXT_ID, NEBULA_IDS[0], NEBULA_IDS[1], STAR_OVERLAY_ID, EXT_STARS_ID].iter().map(|id| {
        let mut obj = GameObject::new_rect(ctx, id.to_string(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0), image: crate::images::solid(0, 0, 0, 0).into(), color: None }),
            (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
        obj.ignore_zoom = true;
        obj.visible = false;
        (id.to_string(), obj)
    }).collect()
}

fn set_look(c: &mut Canvas, id: &str, img: Arc<image::RgbaImage>) {
    let (w, h) = (img.width() as f32, img.height() as f32);
    if let Some(obj) = c.get_game_object_mut(id) {
        obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img, color: None });
    }
}

fn place(c: &mut Canvas, id: &str, rect: (f32, f32, f32, f32), visible: bool) {
    if let Some(obj) = c.get_game_object_mut(id) {
        obj.position = (rect.0.round(), rect.1.round());
        obj.size = (rect.2.round(), rect.3.round());
        obj.visible = visible;
    }
}

pub fn tick_nebula(c: &mut Canvas, st: &Arc<Mutex<State>>, t: &mut NebulaTick) {
    let (px, cannon_prompt, dark_sky, flipped, no_sky, boss) = {
        let s = st.lock().unwrap();
        (s.px, s.cannon_ft_prompt && s.cannon_captured, s.dark_mode, s.gravity_dir < 0.0,
         s.in_space_mode || s.space_launch_active, s.boss_active)
    };

    let f_now = c.key("f");
    if f_now && !t.f_was_down && !cannon_prompt {
        let next = (nebula_set(c) + 1) % NEBULA_SETS.len();
        c.set_var(NEBULA_SET_VAR, next as i32);
        log::info!("nebula: {}", NEBULA_SETS[next].0);
    }
    t.f_was_down = f_now;

    let set = nebula_set(c);
    if t.shown != Some(set) {
        t.shown = Some(set);
        if let Some(img) = set_image(set) {
            t.image_aspect = img.height() as f32 / img.width() as f32;
            for id in NEBULA_IDS { set_look(c, id, img.clone()); }
            t.alpha_q = None;
        }
    }
    if t.sky_look != Some((flipped, dark_sky)) {
        t.sky_look = Some((flipped, dark_sky));
        set_look(c, EXT_ID, ext_base(dark_sky));
        set_look(c, STAR_OVERLAY_ID, star_overlay(flipped));
        // The extension's stars: the sky's own, turned so their pure rows sit
        // against the sky image and the faded ones reach away from it.
        set_look(c, EXT_STARS_ID, star_overlay(!flipped));
    }

    // The sky's rect this frame, set by `tick_background` just before.
    let Some(((x, y), (w, h))) = c.get_game_object("bg").map(|o| (o.position, o.size)) else { return };
    let (sx, sy) = (w / VW, h / VH);
    // A sky-image row range mapped to the screen. Flipped, the sky image is
    // upside down and the extension is below it, so rows mirror about it.
    let rows = |top: f32, bottom: f32| -> (f32, f32) {
        if flipped { (y + (VH - bottom) * sy, (bottom - top) * sy) } else { (y + top * sy, (bottom - top) * sy) }
    };

    let (_, star_rows_end, _) = star_rows(VH as u32);
    let overlay_rows = star_rows_end as f32;
    let (ey, eh) = rows(-EXT_ROWS, 0.0);
    place(c, EXT_ID, (x, ey, w, eh), !no_sky);
    let (sy_, sh) = rows(-overlay_rows, 0.0);
    place(c, EXT_STARS_ID, (x, sy_, w, sh), !no_sky && !dark_sky);

    // Flipped, the whole sky is upside down; the nebula goes with it (its
    // rect mirrors in `rows`, and its image turns half a circle below).
    let show = !no_sky && !boss && NEBULA_SETS[set].1.is_some();
    // The sky's own stars over the nebula, where it overlaps the sky image.
    let (oy, oh) = rows(0.0, overlay_rows);
    place(c, STAR_OVERLAY_ID, (x, oy, w, oh), show && !dark_sky);

    let layer_w = VW * NEBULA_WIDTH;
    let (layer_top, layer_h) = match NEBULA_SETS[set].2 {
        Fit::Strip => {
            let h = layer_w * t.image_aspect;
            (NEBULA_STRIP_BOTTOM_ROW - h, h)
        }
        Fit::Band => {
            let h = (NEBULA_CLOUDS_BOTTOM_ROW - NEBULA_CLOUDS_TOP_ROW) / (CLOUDS_END - CLOUDS_START);
            (NEBULA_CLOUDS_TOP_ROW - h * CLOUDS_START, h)
        }
    };
    // Forward travel only, as the planets. A jump of 2,000 px or more is a
    // warp or respawn, not travel.
    let dx = t.prev_px.map_or(0.0, |p| px - p);
    t.prev_px = Some(px);
    let forward = if dx.abs() > 2_000.0 { 0.0 } else { dx.max(0.0) };
    t.scroll = (t.scroll + forward * NEBULA_PARALLAX).rem_euclid(layer_w);
    let (ny, nh) = rows(layer_top, layer_top + layer_h);
    let left = (VW - layer_w) / 2.0 - t.scroll;
    for (n, id) in NEBULA_IDS.iter().enumerate() {
        let u = left + n as f32 * layer_w;
        place(c, id, (x + u * sx, ny, layer_w * sx, nh), show);
        // Upside down with the sky. Half a turn also mirrors it left-right,
        // which a nebula does not mind, and a mirrored seam still tiles.
        if let Some(obj) = c.get_game_object_mut(id) { obj.rotation = if flipped { 180.0 } else { 0.0 }; }
    }

    // Stronger the higher you climb: faint low down, full strength by the
    // deepest zoom-out (the sky's scale, 1 at normal zoom). Re-tinted only
    // when the strength visibly changes.
    if show {
        let s_min = sky_scale(SKY_ZOOM_FLOOR);
        let climbed = ((1.0 - sx / (sky_rect(1.0, flipped).2 / VW)) / (1.0 - s_min).max(1e-3)).clamp(0.0, 1.0);
        let strength = NEBULA_LOW_ALPHA + (1.0 - NEBULA_LOW_ALPHA) * climbed * climbed.sqrt();
        let q = (strength * 40.0).round() as i32;
        if t.alpha_q != Some(q) {
            t.alpha_q = Some(q);
            if let Some(img) = set_image(set) {
                let a = (q as f32 / 40.0 * 255.0) as u8;
                let (w, h) = (img.width() as f32, img.height() as f32);
                for id in NEBULA_IDS {
                    if let Some(obj) = c.get_game_object_mut(id) {
                        obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (w, h), 0.0), image: img.clone(),
                            color: Some(Color(255, 255, 255, a)) });
                    }
                }
            }
        }
    }
}

/// The nebula's strength low down (it rises to full as the sky zooms out).
pub const NEBULA_LOW_ALPHA: f32 = 0.45;
