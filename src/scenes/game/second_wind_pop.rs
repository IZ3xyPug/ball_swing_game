//! SECOND WIND's save, made loud: like Minecraft's totem of undying, a big
//! heart pops up the middle of the screen — it shoots up from below with a
//! squash-and-stretch bounce and a wobble, hangs there with sparks bursting
//! out round it, then swells and fades away. Screen-space (HUD) objects,
//! driven by one counter on an always-running tick: a fall's respawn pauses
//! the game until the next grab, and the pop has to play from the moment of
//! the save, not from that grab.

use quartz::*;
use std::sync::Arc;

use crate::constants::*;

const VAR: &str = "sw_pop_t";
/// How long the whole pop lasts, in frames.
const LEN: i32 = 110;
const RISE: i32 = 20;
const HOLD_END: i32 = 70;
const HEART: (f32, f32) = (1000.0, 923.0);
const SPARKS: usize = 26;
const SPARK: f32 = 44.0;
const LAYER: i32 = 130;

fn heart_img() -> Arc<image::RgbaImage> {
    static IMG: std::sync::OnceLock<Arc<image::RgbaImage>> = std::sync::OnceLock::new();
    IMG.get_or_init(|| Arc::new(crate::images::big_heart_img(26 * 12, 24 * 12))).clone()
}

fn spark_img() -> Arc<image::RgbaImage> {
    static IMG: std::sync::OnceLock<Arc<image::RgbaImage>> = std::sync::OnceLock::new();
    IMG.get_or_init(|| Arc::new(image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255])))).clone()
}

fn obj(ctx: &mut Context, id: &str, img: Arc<image::RgbaImage>, size: (f32, f32), layer: i32) -> GameObject {
    let mut o = GameObject::new_rect(ctx, id.to_string().into(),
        Some(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img, color: None }),
        size, (-9000.0, -9000.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0);
    // In the world, not the HUD: it scales with the camera's zoom, centred
    // on the view (a HUD object drifted right when zoomed far in).
    o.ignore_zoom = false;
    o.unlit = true;
    o.visible = false;
    o.layer = layer;
    o
}

/// The pop's objects, for the scene builder (hidden).
pub fn objects(ctx: &mut Context) -> Vec<(String, GameObject)> {
    let mut v = vec![
        ("sw_pop_heart".to_string(), obj(ctx, "sw_pop_heart",
            heart_img(), HEART, LAYER + 2)),
    ];
    for i in 0..SPARKS {
        let id = format!("sw_pop_spark_{i}");
        v.push((id.clone(), obj(ctx, &id, spark_img(), (SPARK, SPARK), LAYER + 1)));
    }
    v
}

/// SECOND WIND just saved the run: start the pop.
pub fn trigger(c: &mut Canvas) {
    c.set_var(VAR, 1i32);
    if !c.get_bool("sw_pop_registered") {
        c.set_var("sw_pop_registered", true);
        c.on_update_always(|c| tick(c));
    }
}

/// Size, place, turn and tint one piece. The image Arc is the same every
/// frame, so the texture is uploaded once; the fade is only a tint.
fn place(c: &mut Canvas, id: &str, img: Arc<image::RgbaImage>, centre: (f32, f32), size: (f32, f32), rot: f32, tint: Color) {
    if let Some(o) = c.get_game_object_mut(id) {
        o.size = size;
        o.position = (centre.0 - size.0 * 0.5, centre.1 - size.1 * 0.5);
        o.rotation = rot;
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img, color: Some(tint) });
        o.visible = true;
    }
}

fn hide(c: &mut Canvas) {
    for id in ["sw_pop_heart"] {
        if let Some(o) = c.get_game_object_mut(id) { o.visible = false; }
    }
    for i in 0..SPARKS {
        if let Some(o) = c.get_game_object_mut(&format!("sw_pop_spark_{i}")) { o.visible = false; }
    }
}

/// How fast the world runs `t` frames into the pop: slowed hard as the heart
/// lands, easing back to full speed by the time it fades.
const SLOW_END: i32 = 75;
fn slow_rate(t: i32) -> f32 {
    if t >= SLOW_END { return 1.0; }
    let u = t as f32 / SLOW_END as f32;
    0.35 + 0.65 * u * u
}

/// Slow motion, by letting the world advance only some frames (the engine has
/// no time scale): the world is paused on the frames it sits out. Stands
/// aside for anything else holding the game — the pause menu, a hit-stop, a
/// transition, a fall's respawn standby — and never leaves its pause behind.
fn slow_time(c: &mut Canvas, t: i32) {
    if c.get_bool("sw_slow_paused") {
        c.set_var("sw_slow_paused", false);
        // A hit-stop that began meanwhile now owns the pause.
        if c.get_i32("juice_hitstop") <= 0 { c.resume(); }
    }
    if t >= SLOW_END { return; }
    let busy = matches!(c.get_var("pause_menu_open"), Some(Value::Bool(true)))
        || c.get_bool("game_paused") || c.get_i32("juice_hitstop") > 0
        || crate::transition::busy(c) || c.is_paused();
    if busy { return; }
    let acc = c.get_f32("sw_slow_acc") + slow_rate(t);
    if acc >= 1.0 {
        c.set_var("sw_slow_acc", acc - 1.0);
    } else {
        c.set_var("sw_slow_acc", acc);
        c.set_var("sw_slow_paused", true);
        c.pause();
        // Headless check: `SW_SLOW_LOG=1` counts the frames sat out.
        if std::env::var("SW_SLOW_LOG").is_ok() { eprintln!("SW_SLOW skip t={t}"); }
    }
}

/// Every frame, paused or not (registered by the first `trigger`).
fn tick(c: &mut Canvas) {
    let t = match c.get_var(VAR) { Some(Value::I32(t)) => t, _ => 0 };
    if t <= 0 { return; }
    if t > LEN || !c.is_scene("game") {
        c.set_var(VAR, 0i32);
        if c.get_bool("sw_slow_paused") {
            c.set_var("sw_slow_paused", false);
            if c.get_i32("juice_hitstop") <= 0 && c.is_scene("game") { c.resume(); }
        }
        hide(c);
        return;
    }
    c.set_var(VAR, t + 1);
    slow_time(c, t);
    let tf = t as f32;
    // The middle of the view, in world units, and how big to draw it there:
    // the heart grows and shrinks with the zoom, but only so far — zoomed in
    // near the ground it would fill the screen.
    let (centre, z) = c.camera().map_or(((VW * 0.5, VH * 0.45), 1.0), |cam| {
        let z = cam.zoom.max(0.01);
        ((cam.position.0 + VW * 0.5 / z, cam.position.1 + VH * 0.45 / z), z)
    });
    let k = z.clamp(0.5, 0.9) / z;

    // The heart: up from below with an overshoot, a squash-and-stretch
    // bounce as it lands (always its full shape), a wobble, then a swell as
    // it fades.
    let (scale, lift, alpha) = if t < RISE {
        let u = tf / RISE as f32;
        let back = 1.0 + 2.7 * (u - 1.0).powi(3) + 1.7 * (u - 1.0).powi(2);
        (0.15 + 0.95 * back, (1.0 - u).powi(2) * 620.0, 1.0)
    } else if t < HOLD_END {
        (1.0 + 0.04 * (tf * 0.35).sin(), 0.0, 1.0)
    } else {
        let u = (tf - HOLD_END as f32) / (LEN - HOLD_END) as f32;
        (1.0 + 0.9 * u * u, -120.0 * u, (1.0 - u).powf(1.4))
    };
    let (scale, lift) = (scale * k, lift * k);
    // Squash and stretch: tall while rising, wide as it lands, settling.
    let since_land = (tf - RISE as f32).max(0.0);
    let squash = if t < RISE { -0.12 * (1.0 - tf / RISE as f32) }
        else { 0.14 * (-since_land / 9.0).exp() * (since_land * 0.45).cos() };
    let (sx, sy) = (1.0 + squash, 1.0 - squash);
    let wobble = 9.0 * (tf * 0.3).sin() * (1.0 - tf / LEN as f32);
    // Slightly see-through, so the fight under it still reads.
    let a = (alpha * 0.78 * 255.0) as u8;
    place(c, "sw_pop_heart", heart_img(), (centre.0, centre.1 + lift),
        (HEART.0 * scale * sx, HEART.1 * scale * sy), wobble, Color(255, 255, 255, a));

    // Sparks: burst out of the heart as it lands, slow, drift and fall,
    // tumbling; green and gold like the totem's, with a few of the heart's red.
    let since = (tf - RISE as f32 * 0.6).max(0.0);
    for i in 0..SPARKS {
        let id = format!("sw_pop_spark_{i}");
        if since <= 0.0 {
            if let Some(o) = c.get_game_object_mut(&id) { o.visible = false; }
            continue;
        }
        let fi = i as f32;
        let h = ((fi * 12.9898).sin() * 43758.547).fract().abs();
        let ang = fi * 2.399 + h;
        let speed = 16.0 + 22.0 * h;
        let travel = speed * 30.0 * (1.0 - (-since / 30.0).exp()) * k;
        let p = (centre.0 + ang.cos() * travel, centre.1 + ang.sin() * travel * 0.8 + 0.06 * since * since * k);
        let life = (1.0 - since / (LEN as f32 - RISE as f32 * 0.6)).clamp(0.0, 1.0);
        let col = match i % 3 { 0 => (110, 255, 150), 1 => (255, 220, 90), _ => (240, 70, 80) };
        let sz = SPARK * (0.6 + 0.8 * h) * (0.4 + 0.6 * life) * k;
        place(c, &id, spark_img(), p, (sz, sz), since * (8.0 + 10.0 * h) + fi * 20.0,
            Color(col.0, col.1, col.2, (life.powf(0.7) * 255.0) as u8));
    }
}
