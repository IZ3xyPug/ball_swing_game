//! The boss intro card: a short cinematic title as the player arrives in an
//! arena, while they orbit the safe node (nothing attacks yet).
//!
//! Letterbox bars close in; a band of the boss's colour opens across the
//! middle with a bright line along each edge sliding in from opposite sides;
//! the boss's portrait slides in on the left; its name, in glowing letters of
//! its own colour, slides in beside it, with "BOSS 2 OF 7" above, an epithet
//! below and a one-line hint in the lower bar. It holds, then everything
//! collapses away. Grabbing skips it.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

const LAYER: i32 = 170;
/// Frames: in, hold, out, gone.
const OUT_AT: i32 = 196;
const END_AT: i32 = 226;
const BAR_H: f32 = 250.0;
const BAND_H: f32 = 560.0;
const MID_Y: f32 = VH * 0.47;
const PORTRAIT: f32 = 560.0;
const PORTRAIT_X: f32 = VW * 0.25;
const TEXT_X: f32 = VW * 0.38;
const NAME_PX: f32 = 170.0;

const ACTIVE_VAR: &str = "boss_intro_t";

struct Look {
    rgb: [u8; 3],
    epithet: &'static str,
    hint: &'static str,
    art: &'static [u8],
}

fn look(kind: BossKind) -> Look {
    match kind {
        BossKind::Colossus => Look { rgb: [255, 140, 70], epithet: "Titan of Living Rock",
            hint: "Break its hands, then strike the head while you are buffed.",
            art: include_bytes!("../../../../assets/pixellab/colossus_head.png") },
        BossKind::Conductor => Look { rgb: [255, 90, 200], epithet: "Maestro of the Void Symphony",
            hint: "Feel the beat. Strike the core while it is open.",
            art: include_bytes!("../../../../assets/pixellab/conductor_core_a.png") },
        BossKind::GravityWeaver => Look { rgb: [160, 120, 255], epithet: "Spinner of Fallen Skies",
            hint: "Gravity will turn. Cut the spindles, then the loom.",
            art: include_bytes!("../../../../assets/pixellab/weaver_loom.png") },
        BossKind::FlareTitan => Look { rgb: [255, 190, 60], epithet: "The Star That Walks",
            hint: "Shelter from its flares. Hit the vents while you are buffed.",
            art: include_bytes!("../../../../assets/pixellab/titan_core.png") },
        BossKind::Magnetar => Look { rgb: [90, 200, 255], epithet: "Heart of a Dead Star",
            hint: "Ride its pull and push. Break the poles when the starquake hits.",
            art: include_bytes!("../../../../assets/pixellab/magnetar_core.png") },
        BossKind::Serpent => Look { rgb: [120, 255, 170], epithet: "The Coil Between Stars",
            hint: "Cut it down piece by piece, tail first.",
            art: include_bytes!("../../../../assets/pixellab/serpent_head.png") },
        BossKind::SunDevourer => Look { rgb: [255, 80, 60], epithet: "Eater of Light",
            hint: "Smash its generators, then find the core in the dark.",
            art: include_bytes!("../../../../assets/pixellab/devourer_body.png") },
    }
}

const ALL: [BossKind; 7] = [
    BossKind::Colossus, BossKind::Conductor, BossKind::GravityWeaver, BossKind::FlareTitan,
    BossKind::Magnetar, BossKind::Serpent, BossKind::SunDevourer,
];

fn name_image(kind: BossKind) -> Option<Arc<image::RgbaImage>> {
    let l = look(kind);
    let foot = l.rgb.map(|v| ((v as u16 + 255) / 2) as u8);
    crate::menu_look::glow_text_rgb(kind.name(), NAME_PX, l.rgb, foot)
}

/// Render every name card on a background thread (the glow is a few blurs
/// over a wide image), so the first arena does not hitch on it.
pub fn prewarm() {
    static DONE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    DONE.get_or_init(|| {
        std::thread::spawn(|| { for k in ALL { let _ = name_image(k); } });
    });
}

/// The overlay's objects, for the scene builder (hidden).
pub fn objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    let mut v: Vec<(String, GameObject)> = Vec::new();
    for (id, layer) in [
        ("bi_bar_top", 0), ("bi_bar_bot", 0), ("bi_band", 0), ("bi_line_a", 1), ("bi_line_b", 1),
        ("bi_portrait_glow", 1), ("bi_portrait", 2), ("bi_name", 2), ("bi_kicker", 2), ("bi_epithet", 2), ("bi_hint", 2),
    ] {
        let mut o = GameObject::build(id).size(10.0, 10.0).position(-9000.0, -9000.0).tag("hud").build(ctx);
        o.ignore_zoom = true;
        o.unlit = true;
        o.visible = false;
        o.layer = LAYER + layer;
        v.push((id.to_string(), o));
    }
    v
}

fn band_image(rgb: [u8; 3]) -> Arc<image::RgbaImage> {
    // Wide, so the cover crop keeps its horizontal fade (see the renderer's
    // aspect handling): strongest across the middle, fading up and down and
    // out to the sides.
    let (w, h) = (2048u32, 128u32);
    Arc::new(image::RgbaImage::from_fn(w, h, |x, y| {
        let fx = x as f32 / (w - 1) as f32;
        let fy = (y as f32 / (h - 1) as f32 - 0.5).abs() * 2.0;
        let side = (1.0 - ((fx - 0.5).abs() * 2.0).powf(3.0)).clamp(0.0, 1.0);
        let a = (1.0 - fy * fy) * side * 0.55;
        image::Rgba([rgb[0] / 3, rgb[1] / 3, rgb[2] / 3, (a * 255.0) as u8])
    }))
}

fn line_image(rgb: [u8; 3]) -> Arc<image::RgbaImage> {
    let (w, h) = (2048u32, 16u32);
    Arc::new(image::RgbaImage::from_fn(w, h, |x, y| {
        let fx = x as f32 / (w - 1) as f32;
        let fy = (y as f32 / (h - 1) as f32 - 0.5).abs() * 2.0;
        let side = (1.0 - ((fx - 0.5).abs() * 2.0).powf(2.0)).clamp(0.0, 1.0);
        let a = (1.0 - fy).max(0.0) * side;
        let c = |v: u8| ((v as f32 + 255.0) * 0.5) as u8;
        image::Rgba([c(rgb[0]), c(rgb[1]), c(rgb[2]), (a * 255.0) as u8])
    }))
}

fn glow_disc(rgb: [u8; 3]) -> Arc<image::RgbaImage> {
    let n = 128u32;
    Arc::new(image::RgbaImage::from_fn(n, n, |x, y| {
        let d = (((x as f32 + 0.5) / n as f32 - 0.5).powi(2) + ((y as f32 + 0.5) / n as f32 - 0.5).powi(2)).sqrt() * 2.0;
        let a = (1.0 - d).max(0.0).powf(1.8) * 0.8;
        image::Rgba([rgb[0], rgb[1], rgb[2], (a * 255.0) as u8])
    }))
}

/// The card's images, by object id, so a fade can re-set them with an alpha.
fn imgs() -> &'static Mutex<std::collections::HashMap<String, Arc<image::RgbaImage>>> {
    static I: std::sync::OnceLock<Mutex<std::collections::HashMap<String, Arc<image::RgbaImage>>>> = std::sync::OnceLock::new();
    I.get_or_init(Default::default)
}

fn set_img(c: &mut Canvas, id: &str, img: Arc<image::RgbaImage>, size: (f32, f32)) {
    imgs().lock().unwrap().insert(id.to_string(), img.clone());
    if let Some(o) = c.get_game_object_mut(id) {
        o.size = size;
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img, color: None });
    }
}

fn set_text(c: &mut Canvas, id: &str, t: &str, px: f32, col: Color, width: f32, left: bool) {
    let Some(font) = crate::scenes::game::helpers::ui_font() else { return };
    let s = c.virtual_scale();
    if let Some(o) = c.get_game_object_mut(id) {
        o.size = (width, px * 1.4);
        let spec = if left { crate::objects::ui_text_left_spec(t, &font, px * s, col, width * s) }
                   else { crate::objects::ui_text_spec(t, &font, px * s, col, width * s) };
        o.set_drawable(Box::new(spec));
    }
}

pub fn active(c: &Canvas) -> bool {
    matches!(c.get_var(ACTIVE_VAR), Some(Value::I32(t)) if t > 0)
}

/// The player arrived in an arena: start the card.
pub fn begin(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (kind, index) = { let s = st.lock().unwrap(); (s.boss_kind, s.boss_index) };
    let mode = crate::mode::current_mode(c);
    let total = crate::mode::boss_count(mode).max(1);
    let l = look(kind);
    c.set_var(ACTIVE_VAR, 1i32);
    let bar = Arc::new(image::RgbaImage::from_pixel(4, 4, image::Rgba([4, 2, 10, 240])));
    set_img(c, "bi_bar_top", bar.clone(), (VW * 1.2, BAR_H));
    set_img(c, "bi_bar_bot", bar, (VW * 1.2, BAR_H));
    set_img(c, "bi_band", band_image(l.rgb), (VW, BAND_H));
    let line = line_image(l.rgb);
    set_img(c, "bi_line_a", line.clone(), (VW, 10.0));
    set_img(c, "bi_line_b", line, (VW, 10.0));
    set_img(c, "bi_portrait_glow", glow_disc(l.rgb), (PORTRAIT * 1.7, PORTRAIT * 1.7));
    if let Some(art) = crate::scenes::game::helpers::pl_image_cached(l.art, PORTRAIT) {
        set_img(c, "bi_portrait", art, (PORTRAIT, PORTRAIT));
    }
    match name_image(kind) {
        Some(img) => {
            let (w, h) = (img.width() as f32, img.height() as f32);
            set_img(c, "bi_name", img, (w, h));
        }
        None => set_text(c, "bi_name", kind.name(), NAME_PX * 0.7, Color(255, 255, 255, 255), VW * 0.6, true),
    }
    let kicker = if crate::mode::is_final_boss(mode, index) {
        "WARNING   \u{00b7}   FINAL BOSS".to_string()
    } else {
        format!("WARNING   \u{00b7}   BOSS {} OF {}", index + 1, total)
    };
    let [r, g, b] = l.rgb;
    set_text(c, "bi_kicker", &kicker, 44.0, Color(r, g, b, 255), VW * 0.6, true);
    set_text(c, "bi_epithet", l.epithet, 52.0, Color(235, 228, 255, 240), VW * 0.6, true);
    set_text(c, "bi_hint", l.hint, 40.0, Color(215, 210, 240, 230), VW, false);
    // The arena's own "tether to begin" waits for the card.
    if let Some(o) = c.get_game_object_mut("start_prompt_text") { o.visible = false; }
    if let Some(cam) = c.camera_mut() {
        cam.flash_with(Color(r, g, b, 120), 0.35, FlashMode::Pulse, FlashEase::Sharp, 0.7, 0.0);
    }
}

/// Jump to the card's exit (a grab while it is up).
pub fn skip(c: &mut Canvas) {
    let t = c.get_i32(ACTIVE_VAR);
    if t > 0 && t < OUT_AT { c.set_var(ACTIVE_VAR, OUT_AT); }
}

fn ease_out(k: f32) -> f32 { 1.0 - (1.0 - k.clamp(0.0, 1.0)).powi(3) }
fn ease_in(k: f32) -> f32 { k.clamp(0.0, 1.0).powi(3) }
/// 0 -> 1 over [a, b] (eased in), and back to 0 over the exit.
fn ramp(t: i32, a: i32, b: i32) -> f32 {
    let inn = ease_out((t - a) as f32 / (b - a).max(1) as f32);
    let out = 1.0 - ease_in((t - OUT_AT) as f32 / (END_AT - OUT_AT - 6) as f32);
    inn.min(out)
}

/// Show `id` at `alpha`: an image object is re-set with that alpha (text
/// objects just show or hide; their slide carries the entrance).
fn fade(c: &mut Canvas, id: &str, alpha: f32) {
    let img = imgs().lock().unwrap().get(id).cloned();
    if let Some(o) = c.get_game_object_mut(id) {
        o.visible = alpha > 0.02;
        if let (Some(img), true) = (img, o.visible) {
            let size = o.size;
            o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img,
                color: Some(Color(255, 255, 255, (alpha.clamp(0.0, 1.0) * 255.0) as u8)) });
        }
    }
}

fn place(c: &mut Canvas, id: &str, center: (f32, f32), alpha: f32) {
    if let Some(o) = c.get_game_object_mut(id) {
        o.position = (center.0 - o.size.0 * 0.5, center.1 - o.size.1 * 0.5);
    }
    fade(c, id, alpha);
}

fn place_left(c: &mut Canvas, id: &str, left: f32, center_y: f32, alpha: f32) {
    if let Some(o) = c.get_game_object_mut(id) {
        o.position = (left, center_y - o.size.1 * 0.5);
    }
    fade(c, id, alpha);
}

/// Every frame.
pub fn tick(c: &mut Canvas) {
    let t = c.get_i32(ACTIVE_VAR);
    if t <= 0 { return; }
    let t = t + 1;
    if t >= END_AT {
        c.set_var(ACTIVE_VAR, 0i32);
        hide(c);
        if let Some(o) = c.get_game_object_mut("start_prompt_text") { o.visible = true; }
        return;
    }
    c.set_var(ACTIVE_VAR, t);

    // Letterbox bars.
    let bars = ramp(t, 0, 16);
    place(c, "bi_bar_top", (VW * 0.5, -BAR_H * 0.5 + BAR_H * bars), 1.0);
    place(c, "bi_bar_bot", (VW * 0.5, VH + BAR_H * 0.5 - BAR_H * bars), 1.0);
    // The band opens from the middle.
    let band = ramp(t, 4, 26);
    if let Some(o) = c.get_game_object_mut("bi_band") {
        let w = VW * band.max(0.001);
        let h = BAND_H * (0.35 + 0.65 * band);
        o.size = (w, h);
        o.update_image_shape();
    }
    place(c, "bi_band", (VW * 0.5, MID_Y), band);
    // Bright lines along its edges, sliding in from opposite sides.
    let lines = ramp(t, 8, 30);
    let edge = BAND_H * 0.5 * (0.35 + 0.65 * band);
    place(c, "bi_line_a", (VW * 0.5 - (1.0 - lines) * VW, MID_Y - edge), lines);
    place(c, "bi_line_b", (VW * 0.5 + (1.0 - lines) * VW, MID_Y + edge), lines);
    // The portrait slides in from the left, a slow breathing glow behind it.
    let p = ramp(t, 12, 36);
    let px = PORTRAIT_X - (1.0 - p) * 500.0;
    let breathe = 1.0 + 0.04 * (t as f32 * 0.08).sin();
    if let Some(o) = c.get_game_object_mut("bi_portrait_glow") {
        let d = PORTRAIT * 1.7 * breathe;
        o.size = (d, d);
        o.update_image_shape();
    }
    place(c, "bi_portrait_glow", (px, MID_Y), p * 0.9);
    place(c, "bi_portrait", (px, MID_Y), p);
    // The words, from the right, one after another.
    let k = ramp(t, 18, 38);
    place_left(c, "bi_kicker", TEXT_X + (1.0 - k) * 300.0, MID_Y - 190.0, k);
    let n = ramp(t, 14, 38);
    place_left(c, "bi_name", TEXT_X - 50.0 + (1.0 - n) * 420.0, MID_Y - 30.0, n);
    let e = ramp(t, 24, 44);
    place_left(c, "bi_epithet", TEXT_X + (1.0 - e) * 300.0, MID_Y + 130.0, e);
    let h = ramp(t, 32, 52);
    place(c, "bi_hint", (VW * 0.5, VH - BAR_H * 0.5 * bars - 10.0 + (1.0 - h) * 40.0), h);
}

pub fn hide(c: &mut Canvas) {
    for id in ["bi_bar_top", "bi_bar_bot", "bi_band", "bi_line_a", "bi_line_b", "bi_portrait_glow",
               "bi_portrait", "bi_name", "bi_kicker", "bi_epithet", "bi_hint"] {
        if let Some(o) = c.get_game_object_mut(id) { o.visible = false; }
    }
}

/// Clear it (a new run, or leaving the game).
pub fn reset(c: &mut Canvas) {
    c.set_var(ACTIVE_VAR, 0i32);
    hide(c);
}
