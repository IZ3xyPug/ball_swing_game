//! death_screen.rs — dying, Terraria-style.
//!
//! The last heart no longer cuts to a separate game-over screen. The run stays
//! on screen and keeps going without the player: a boss that has just killed
//! you carries on moving and attacking. Over it, slowly, a dark veil, then
//! YOU HAVE DIED, then what killed you and the run's numbers, then two small
//! buttons — the old screen's RETRY and MENU.
//!
//! The objects are added to the canvas when the screen begins and removed
//! when the game scene exits, so they never outlive the run.

use quartz::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::objects::ui_text_spec;
use super::helpers::ui_font;

const VEIL: &str = "dth_veil";
const TITLE: &str = "dth_title";
const CAUSE: &str = "dth_cause";
const STATS: &str = "dth_stats";
const RETRY_BTN: &str = "dth_retry_btn";
const RETRY_TXT: &str = "dth_retry_txt";
const MENU_BTN: &str = "dth_menu_btn";
const MENU_TXT: &str = "dth_menu_txt";
const SUM_CARD: &str = "dth_sum_card";
const SUM_BEST: &str = "dth_sum_best";
const SUM_LABELS: [&str; 6] = ["dth_sum_l0", "dth_sum_l1", "dth_sum_l2", "dth_sum_l3", "dth_sum_l4", "dth_sum_l5"];
const SUM_VALUES: [&str; 6] = ["dth_sum_v0", "dth_sum_v1", "dth_sum_v2", "dth_sum_v3", "dth_sum_v4", "dth_sum_v5"];
const ALL: [&str; 23] = [VEIL, TITLE, CAUSE, STATS, RETRY_BTN, RETRY_TXT, MENU_BTN, MENU_TXT,
    SUM_CARD, SUM_BEST,
    SUM_LABELS[0], SUM_LABELS[1], SUM_LABELS[2], SUM_LABELS[3], SUM_LABELS[4], SUM_LABELS[5],
    SUM_VALUES[0], SUM_VALUES[1], SUM_VALUES[2], SUM_VALUES[3], SUM_VALUES[4], SUM_VALUES[5], "dth_sum_head"];
/// The run summary: a card of six numbers under the cause line.
const SUM_W: f32 = 2200.0;
const SUM_H: f32 = 430.0;
const SUM_Y: f32 = VH * 0.535;
const SUM_LABEL_NAMES: [&str; 6] = ["DISTANCE", "TIME", "COINS", "BEST CHAIN", "BOSSES", "PERKS"];

/// Over the game and its HUD, under the shutter and the engine's flash.
const LAYER: i32 = i32::MAX - 10;

const TITLE_TEXT: &str = "YOU HAVE DIED";
const TITLE_PX: f32 = 210.0;
const TITLE_Y: f32 = VH * 0.24;
const CAUSE_Y: f32 = VH * 0.47;
const STATS_Y: f32 = VH * 0.555;
const BTN_SIZE: (f32, f32) = (560.0, 124.0);
const BTN_GAP: f32 = 70.0;
const BTN_Y: f32 = VH * 0.79;
const LABEL_PX: f32 = 62.0;
const RETRY_ACCENT: [u8; 3] = [120, 230, 160];
const MENU_ACCENT: [u8; 3] = [150, 170, 255];

/// When each piece fades in (start tick, ticks to full), so it builds up
/// slowly in order rather than arriving at once.
const VEIL_FADE: (i32, i32) = (0, 100);
const TITLE_FADE: (i32, i32) = (25, 95);
const CAUSE_FADE: (i32, i32) = (80, 70);
const STATS_FADE: (i32, i32) = (105, 70);
const BTN_FADE: (i32, i32) = (140, 70);
/// The buttons take input once they are mostly there.
const INPUT_AT: i32 = BTN_FADE.0 + 30;

fn retry_pos() -> (f32, f32) { (VW * 0.5 - BTN_GAP * 0.5 - BTN_SIZE.0, BTN_Y) }
fn menu_pos() -> (f32, f32) { (VW * 0.5 + BTN_GAP * 0.5, BTN_Y) }

/// Whether the player is dead and the screen is up.
pub fn active(c: &Canvas) -> bool { c.get_bool("death_screen") }

/// Build the title and labels' art now (a background thread at app start),
/// so dying does not stall while it is rendered.
pub fn prewarm() {
    let _ = crate::menu_look::glow_text(TITLE_TEXT, TITLE_PX);
    let _ = crate::menu_look::glow_text("RETRY", LABEL_PX);
    let _ = crate::menu_look::glow_text("MENU", LABEL_PX);
    let _ = veil_img();
}

/// A dark wash, heavier at the edges and tinged red, so the middle of the
/// fight stays readable through it.
fn veil_img() -> Arc<image::RgbaImage> {
    static IMG: OnceLock<Arc<image::RgbaImage>> = OnceLock::new();
    IMG.get_or_init(|| {
        // Twice the screen each way (see `VEIL_OVER`): the screen is the
        // middle half, and past its edges stays at the edge's darkness.
        let (w, h) = (384u32, 216u32);
        Arc::new(image::RgbaImage::from_fn(w, h, |x, y| {
            let dx = ((x as f32 + 0.5) / w as f32 - 0.5) * 2.0;
            let dy = ((y as f32 + 0.5) / h as f32 - 0.5) * 2.0;
            let r = ((dx * dx + dy * dy).sqrt() / 0.7071).clamp(0.0, 1.0);
            let a = 0.30 + 0.45 * r * r;
            image::Rgba([28, 4, 10, (a * 255.0) as u8])
        }))
    }).clone()
}

/// Each overlay's art, kept to re-tint as it fades (an object's image cannot
/// be read back).
fn arts() -> &'static Mutex<HashMap<&'static str, Arc<image::RgbaImage>>> {
    static ARTS: OnceLock<Mutex<HashMap<&'static str, Arc<image::RgbaImage>>>> = OnceLock::new();
    ARTS.get_or_init(Default::default)
}

fn quad(id: &'static str, size: (f32, f32), pos: (f32, f32), image: Option<Arc<image::RgbaImage>>) -> GameObject {
    let mut b = GameObject::build(id).position(pos.0, pos.1).size(size.0, size.1);
    if let Some(img) = image {
        arts().lock().unwrap().insert(id, img.clone());
        b = b.image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img, color: Some(Color(255, 255, 255, 0)) });
    }
    let mut o = b.layer(LAYER).ignore_zoom().finish();
    // A screen overlay: lights must not brighten it.
    o.unlit = true;
    o.visible = false;
    o
}

/// Put the screen up. `cause` is the line under the title.
pub fn begin(c: &mut Canvas, cause: &str) {
    remove(c);
    c.set_var("death_screen", true);
    c.set_var("death_t", 0i32);
    c.set_var("death_cause_text", Value::Str(cause.to_string()));
    // The view stays where the player died: the player is gone, the fight
    // is not. Nothing follows anything, and the zoom holds.
    if let Some(cam) = c.camera_mut() {
        cam.follow(None);
        cam.zoom_target = cam.zoom;
    }
    c.set_var("go_count_distance", 0i32);
    c.set_var("go_count_distance_vel", 0i32);
    c.set_var("go_count_score", 0i32);
    c.set_var("go_count_score_vel", 0i32);
    c.set_var("go_count_coins", 0i32);
    c.set_var("go_count_coins_vel", 0i32);
    c.set_var("sum_drawn", Value::Str(String::new()));

    // Over-drawn: a window that is not 16:9 shows past the virtual screen,
    // and a screen-sized veil left bands there untinted.
    let mut objs = vec![(VEIL, quad(VEIL, (VW * 2.0, VH * 2.0), (-VW * 0.5, -VH * 0.5), Some(veil_img())))];
    if let Some(t) = crate::menu_look::glow_text(TITLE_TEXT, TITLE_PX) {
        let (w, h) = (t.width() as f32, t.height() as f32);
        objs.push((TITLE, quad(TITLE, (w, h), ((VW - w) * 0.5, TITLE_Y), Some(t))));
    }
    objs.push((CAUSE, quad(CAUSE, (VW * 0.8, 110.0), (VW * 0.1, CAUSE_Y), None)));
    objs.push((STATS, quad(STATS, (VW * 0.8, 100.0), (VW * 0.1, STATS_Y), None)));
    // The run summary card.
    let card = crate::menu_look::glass(SUM_W, SUM_H, [200, 150, 255], false);
    objs.push((SUM_CARD, quad(SUM_CARD, (SUM_W, SUM_H), ((VW - SUM_W) * 0.5, SUM_Y), Some(card.image))));
    objs.push(("dth_sum_head", quad("dth_sum_head", (SUM_W, 60.0), ((VW - SUM_W) * 0.5, SUM_Y + 22.0), None)));
    objs.push((SUM_BEST, quad(SUM_BEST, (SUM_W / 3.0, 50.0), ((VW - SUM_W) * 0.5, SUM_Y + 98.0), None)));
    for i in 0..6 {
        let (col, row) = (i % 3, i / 3);
        let x = (VW - SUM_W) * 0.5 + col as f32 * SUM_W / 3.0;
        let y = SUM_Y + 90.0 + row as f32 * 165.0;
        objs.push((SUM_VALUES[i], quad(SUM_VALUES[i], (SUM_W / 3.0, 90.0), (x, y + 40.0), None)));
        objs.push((SUM_LABELS[i], quad(SUM_LABELS[i], (SUM_W / 3.0, 50.0), (x, y + 120.0), None)));
    }
    for (btn, txt, pos, accent, label) in [
        (RETRY_BTN, RETRY_TXT, retry_pos(), RETRY_ACCENT, "RETRY"),
        (MENU_BTN, MENU_TXT, menu_pos(), MENU_ACCENT, "MENU"),
    ] {
        let glass = crate::menu_look::glass(BTN_SIZE.0, BTN_SIZE.1, accent, true);
        objs.push((btn, quad(btn, BTN_SIZE, pos, Some(glass.image))));
        if let Some(t) = crate::menu_look::glow_text(label, LABEL_PX) {
            let (w, h) = (t.width() as f32, t.height() as f32);
            let at = (pos.0 + (BTN_SIZE.0 - w) * 0.5, pos.1 + (BTN_SIZE.1 - h) * 0.5);
            objs.push((txt, quad(txt, (w, h), at, Some(t))));
        }
    }
    for (id, o) in objs { c.add_game_object(id.to_string(), o); }
}

/// Take the screen down (leaving the run, either way).
pub fn remove(c: &mut Canvas) {
    c.set_var("death_screen", false);
    for id in ALL {
        if c.get_game_object(id).is_some() { c.remove_game_object(id); }
    }
}

/// 0..1 for a fade window at tick `t`, eased.
fn fade(t: i32, (start, len): (i32, i32)) -> f32 {
    let k = ((t - start) as f32 / len as f32).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn set_alpha(c: &mut Canvas, id: &str, a: f32, tint: (u8, u8, u8)) {
    let Some(img) = arts().lock().unwrap().get(id).cloned() else { return; };
    let Some(o) = c.get_game_object_mut(id) else { return; };
    o.visible = a > 0.002;
    let size = o.size;
    o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img,
        color: Some(Color(tint.0, tint.1, tint.2, (a * 255.0) as u8)) });
}

fn set_text(c: &mut Canvas, id: &str, text: &str, px: f32, a: f32, rgb: (u8, u8, u8)) {
    let Some(font) = ui_font() else { return; };
    let s = c.virtual_scale();
    let Some(o) = c.get_game_object_mut(id) else { return; };
    o.visible = a > 0.002;
    if o.visible {
        let w = o.size.0;
        o.set_drawable(Box::new(ui_text_spec(text, &font, px * s, Color(rgb.0, rgb.1, rgb.2, (a * 255.0) as u8), w * s)));
    }
}

/// Every game frame while dead: fade the pieces in, count the stats up.
pub fn tick(c: &mut Canvas) {
    if !active(c) { return; }
    let t = c.get_i32("death_t") + 1;
    c.set_var("death_t", t);

    // Each piece is re-tinted only while it is fading.
    let fading = |(start, len): (i32, i32)| t >= start && t <= start + len + 1;
    if fading(VEIL_FADE) { set_alpha(c, VEIL, fade(t, VEIL_FADE), (255, 255, 255)); }
    // The title settles down into place as it fades in.
    if fading(TITLE_FADE) {
        let ta = fade(t, TITLE_FADE);
        set_alpha(c, TITLE, ta, (255, 120, 120));
        if let Some(o) = c.get_game_object_mut(TITLE) {
            o.position.1 = TITLE_Y - 40.0 * (1.0 - ta);
        }
    }

    let cause = match c.get_var("death_cause_text") { Some(Value::Str(s)) => s, _ => String::new() };
    if fading(CAUSE_FADE) {
        set_text(c, CAUSE, &cause, 64.0, fade(t, CAUSE_FADE), (255, 215, 215));
    }

    // The run summary: the card fades in, the numbers count up.
    let sa = fade(t, STATS_FADE);
    if fading(STATS_FADE) { set_alpha(c, SUM_CARD, sa, (255, 255, 255)); }
    if t >= STATS_FADE.0 {
        let target_distance = c.get_f32("last_distance").round() as i32;
        let target_coins = c.get_i32("last_coins").max(0);
        let d = crate::menu::gameover_step_countup(c, "go_count_distance", "go_count_distance_vel", target_distance);
        let k = crate::menu::gameover_step_countup(c, "go_count_coins", "go_count_coins_vel", target_coins);
        let secs = c.get_i32("sum_secs").max(0);
        let values = [
            format!("{} px", thousands(d.max(0) as u64)),
            format!("{}:{:02}", secs / 60, secs % 60),
            thousands(k.max(0) as u64),
            format!("{}", c.get_i32("sum_chain").max(0)),
            format!("{}", c.get_i32("sum_bosses").max(0)),
            format!("{}", c.get_i32("sum_perks").max(0)),
        ];
        // The text only changes while counting; redraw when it does.
        let key = values.join("|");
        let drawn = match c.get_var("sum_drawn") { Some(Value::Str(s)) => s, _ => String::new() };
        if key != drawn || fading(STATS_FADE) {
            c.set_var("sum_drawn", Value::Str(key));
            set_text(c, "dth_sum_head", "RUN SUMMARY", 40.0, sa, (205, 180, 255));
            for i in 0..6 {
                set_text(c, SUM_VALUES[i], &values[i], 70.0, sa, (250, 245, 255));
                set_text(c, SUM_LABELS[i], SUM_LABEL_NAMES[i], 32.0, sa * 0.85, (190, 175, 230));
            }
            if c.get_bool("sum_new_best") {
                set_text(c, SUM_BEST, "NEW BEST!", 34.0, sa, (255, 220, 110));
            }
        }
    }

    if fading(BTN_FADE) {
        let ba = fade(t, BTN_FADE);
        for id in [RETRY_BTN, RETRY_TXT, MENU_BTN, MENU_TXT] {
            set_alpha(c, id, ba, (255, 255, 255));
        }
    }
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    out
}

/// Whether the buttons are up and taking input.
fn ready(c: &Canvas) -> bool {
    active(c) && c.is_scene("game") && c.get_i32("death_t") >= INPUT_AT
        && !crate::transition::busy(c)
}

fn retry(c: &mut Canvas) {
    // The game scene's exit takes the screen down.
    c.load_scene("game");
}

fn to_menu(c: &mut Canvas) {
    // The shutter closes over the screen as it is; the game scene's exit
    // takes the screen down.
    crate::transition::go_slow(c, "menu");
}

fn in_rect(p: (f32, f32), pos: (f32, f32), size: (f32, f32)) -> bool {
    p.0 >= pos.0 && p.0 <= pos.0 + size.0 && p.1 >= pos.1 && p.1 <= pos.1 + size.1
}

/// Clicks and keys. Registered once at app start, like the pause menu's,
/// because a press handler registered in the game scene's `on_enter` never
/// receives presses in the live window.
pub fn register_handlers(canvas: &mut Canvas) {
    if canvas.get_bool("death_handlers_registered") { return; }
    canvas.set_var("death_handlers_registered", true);
    canvas.on_mouse_press(|c, btn, pos| {
        if btn != MouseButton::Left || !ready(c) { return; }
        // The overlay is `ignore_zoom`: compare in its space.
        let p = c.input_to_unzoomed(pos);
        if in_rect(p, retry_pos(), BTN_SIZE) {
            retry(c);
        } else if in_rect(p, menu_pos(), BTN_SIZE) {
            to_menu(c);
        }
    });
    canvas.on_mouse_move(|c, pos| {
        if !ready(c) { return; }
        let p = c.input_to_unzoomed(pos);
        for (id, at) in [(RETRY_BTN, retry_pos()), (MENU_BTN, menu_pos())] {
            let over = in_rect(p, at, BTN_SIZE);
            if let Some(o) = c.get_game_object_mut(id) {
                if over { o.set_tint(Color(255, 255, 255, 70)); } else { o.clear_highlight(); }
            }
        }
    });
    canvas.on_key_press(|c, key| {
        if !ready(c) { return; }
        match key {
            Key::Named(NamedKey::Space) | Key::Named(NamedKey::Enter) => retry(c),
            Key::Named(NamedKey::Escape) => to_menu(c),
            _ => {}
        }
    });
}
