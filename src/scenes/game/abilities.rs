//! abilities.rs — the equipped active ability, in a run.
//!
//! One ability is equipped in the shop (`profile::ABILITIES`) and taken into
//! every run at the rank owned:
//!
//! * MOMENTUM BURST — a boost bar that refills over time; when full, a press
//!   adds a push along the direction the player is already moving. Ranks push
//!   harder and refill faster.
//! * ALTERNATING SWING — flips the direction of travel (on a rope, the swing
//!   reverses). One use per rank, per run.
//! * OMNI BOOST — a push in any direction you aim. One use per rank, per run.
//!
//! Fired with the ability button (bottom-left, above the pause control, on
//! every platform) or E. Omni aims by dragging away from the button and
//! letting go (a virtual stick), or toward the mouse cursor with E.
//!
//! Run state lives in canvas vars (`ab_*`), set by `start_run` at run start.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

/// Which ability is equipped this run (`ab_kind`).
pub const AB_NONE: i32 = -1;
pub const AB_BURST: i32 = 0;
pub const AB_SWAP: i32 = 1;
pub const AB_OMNI: i32 = 2;

/// The button: (x, y, size), unzoomed screen space. Above the pause control.
pub const AB_BTN: (f32, f32, f32) = (40.0, VH - 540.0, 230.0);

/// Momentum burst: seconds to refill the bar at rank 1, and per rank faster.
const BURST_REFILL_S: f32 = 8.0;
const BURST_REFILL_PER_RANK: f32 = 1.3;
/// Push: this share of the current speed, plus a floor, per rank more.
const BURST_SHARE: f32 = 0.45;
const BURST_SHARE_PER_RANK: f32 = 0.12;
const BURST_FLOOR: f32 = 18.0;
/// Omni boost: the push (px/tick), and how much of the old velocity is kept.
const OMNI_PUSH: f32 = 64.0;
const OMNI_KEEP: f32 = 0.4;
/// Alternating swing keeps this much more speed as it flips.
const SWAP_KICK: f32 = 1.15;
/// OVERDRIVE: after a boost the speed cap is lifted by this factor (per
/// ability, per rank) and eases back over `OVERDRIVE_TICKS`. Without it the
/// boost was clamped to the ordinary cap the same frame it was applied —
/// at a full swing, which is when anyone fires it, it did almost nothing.
pub const OVERDRIVE_TICKS: i32 = 75;
const OVERDRIVE_BURST: f32 = 1.5;
const OVERDRIVE_OMNI: f32 = 1.7;
const OVERDRIVE_SWAP: f32 = 1.25;
const OVERDRIVE_PER_RANK: f32 = 0.12;
/// Swap and Omni win a charge back over time, faster per rank; they hold
/// one more than their rank.
const RECHARGE_S: f32 = 26.0;
const RECHARGE_PER_RANK: f32 = 3.0;
/// How far the stick must be dragged to count as aimed, screen px.
const OMNI_DEADZONE: f32 = 50.0;

const BTN: &str = "ab_btn";
const FILL: &str = "ab_fill";
const ICON: &str = "ab_icon";
const COUNT: &str = "ab_count";
const AIM: &str = "ab_aim";

fn btn_centre() -> (f32, f32) {
    (AB_BTN.0 + AB_BTN.2 * 0.5, AB_BTN.1 + AB_BTN.2 * 0.5)
}

/// Whether an unzoomed screen point is on the button.
pub fn on_button(p: (f32, f32)) -> bool {
    let (cx, cy) = btn_centre();
    let r = AB_BTN.2 * 0.5 + 20.0;
    (p.0 - cx).powi(2) + (p.1 - cy).powi(2) <= r * r
}

/// Whether the button is live (an ability equipped, in a run).
pub fn active(c: &Canvas) -> bool {
    c.is_scene("game") && c.get_i32("ab_kind") >= 0 && c.get_var("ab_kind").is_some()
}

/// Set this run's ability from the profile. Called at run start.
pub fn start_run(c: &mut Canvas) {
    let eq = crate::profile::equipped_ability();
    let (kind, rank) = match eq {
        Some((a, r)) => (match a.id { "ab_burst" => AB_BURST, "ab_swap" => AB_SWAP, "ab_omni" => AB_OMNI, _ => AB_NONE }, r as i32),
        None => (AB_NONE, 0),
    };
    c.set_var("ab_kind", kind);
    c.set_var("ab_rank", rank);
    // Burst starts full; the others start full of charges (rank + 1).
    c.set_var("ab_bar", 1.0f32);
    c.set_var("ab_charges", if rank > 0 { rank + 1 } else { 0 });
    c.set_var("ab_recharge", 0i32);
    c.set_var("ab_overdrive_ticks", 0i32);
    c.set_var("ab_aiming", false);
    c.set_var("ab_flash", 0i32);
    c.set_var("ab_last_fill", -1i32);
    c.set_var("ab_count_shown", -2i32);
}

// ── Art (built once) ─────────────────────────────────────────────────────────

fn disc(n: u32, rgba: [u8; 4], ring: [u8; 4], ring_w: f32) -> image::RgbaImage {
    let r = n as f32 * 0.5;
    image::RgbaImage::from_fn(n, n, |x, y| {
        let d = ((x as f32 + 0.5 - r).powi(2) + (y as f32 + 0.5 - r).powi(2)).sqrt();
        let edge = (r - d).clamp(0.0, 1.0);
        if d > r - ring_w { image::Rgba([ring[0], ring[1], ring[2], (ring[3] as f32 * edge) as u8]) }
        else { image::Rgba([rgba[0], rgba[1], rgba[2], rgba[3]]) }
    })
}

fn base_img() -> Arc<image::RgbaImage> {
    static IMG: std::sync::OnceLock<Arc<image::RgbaImage>> = std::sync::OnceLock::new();
    IMG.get_or_init(|| Arc::new(disc(128, [20, 14, 44, 150], [200, 180, 255, 220], 5.0))).clone()
}

/// The burst's fill, in 24 steps (cached: an image per frame would re-upload).
fn fill_img(step: usize, ready: bool) -> Arc<image::RgbaImage> {
    static IMGS: std::sync::OnceLock<Vec<[Arc<image::RgbaImage>; 2]>> = std::sync::OnceLock::new();
    IMGS.get_or_init(|| (0..=24).map(|k| {
        let make = |col: [u8; 3]| {
            let n = 128u32;
            let r = n as f32 * 0.5 - 7.0;
            let level = n as f32 - 7.0 - (k as f32 / 24.0) * (n as f32 - 14.0);
            Arc::new(image::RgbaImage::from_fn(n, n, |x, y| {
                let d = ((x as f32 + 0.5 - 64.0).powi(2) + (y as f32 + 0.5 - 64.0).powi(2)).sqrt();
                if d <= r && y as f32 >= level { image::Rgba([col[0], col[1], col[2], 150]) } else { image::Rgba([0, 0, 0, 0]) }
            }))
        };
        [make([255, 150, 60]), make([255, 210, 120])]
    }).collect())[step.min(24)][ready as usize].clone()
}

/// The icon for each ability, white on clear.
pub fn icon_img(kind: i32) -> Arc<image::RgbaImage> {
    static IMGS: std::sync::OnceLock<Vec<Arc<image::RgbaImage>>> = std::sync::OnceLock::new();
    IMGS.get_or_init(|| (0..3).map(|k| {
        let n = 128u32;
        let mut img = image::RgbaImage::new(n, n);
        let mut dot = |x: f32, y: f32, w: f32| {
            for yy in (y - w) as i32..=(y + w) as i32 { for xx in (x - w) as i32..=(x + w) as i32 {
                if xx >= 0 && yy >= 0 && (xx as u32) < n && (yy as u32) < n
                    && ((xx as f32 - x).powi(2) + (yy as f32 - y).powi(2)) <= w * w {
                    img.put_pixel(xx as u32, yy as u32, image::Rgba([255, 255, 255, 240]));
                }
            }}
        };
        let mut line = |a: (f32, f32), b: (f32, f32), w: f32| {
            let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) as i32).max(1);
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                dot(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, w);
            }
        };
        match k {
            // Burst: a double chevron, pointing ahead.
            0 => for dx in [0.0, 26.0] {
                line((38.0 + dx, 36.0), (62.0 + dx, 64.0), 6.0);
                line((62.0 + dx, 64.0), (38.0 + dx, 92.0), 6.0);
            },
            // Swap: two arrows, opposite ways.
            1 => {
                line((30.0, 48.0), (98.0, 48.0), 5.0); line((98.0, 48.0), (82.0, 34.0), 5.0); line((98.0, 48.0), (82.0, 62.0), 5.0);
                line((98.0, 82.0), (30.0, 82.0), 5.0); line((30.0, 82.0), (46.0, 68.0), 5.0); line((30.0, 82.0), (46.0, 96.0), 5.0);
            }
            // Omni: four arrow heads round a centre.
            _ => {
                let c = 64.0;
                for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                    let tip = (c + dx * 44.0, c + dy * 44.0);
                    line((c + dx * 14.0, c + dy * 14.0), tip, 5.0);
                    let (px, py) = (-dy, dx);
                    line(tip, (tip.0 - dx * 16.0 + px * 14.0, tip.1 - dy * 16.0 + py * 14.0), 5.0);
                    line(tip, (tip.0 - dx * 16.0 - px * 14.0, tip.1 - dy * 16.0 - py * 14.0), 5.0);
                }
                dot(c, c, 8.0);
            }
        }
        Arc::new(img)
    }).collect())[kind.clamp(0, 2) as usize].clone()
}

fn hud_quad(name: &str, size: (f32, f32), img: Arc<image::RgbaImage>, layer: i32) -> GameObject {
    let mut o = GameObject::build(name)
        .position(AB_BTN.0, AB_BTN.1)
        .size(size.0, size.1)
        .image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img, color: None })
        .layer(layer)
        .ignore_zoom()
        .finish();
    o.visible = false;
    o.unlit = true;
    o
}

fn clear_px() -> Arc<image::RgbaImage> {
    static P: std::sync::OnceLock<Arc<image::RgbaImage>> = std::sync::OnceLock::new();
    P.get_or_init(|| Arc::new(image::RgbaImage::new(1, 1))).clone()
}

/// Create the HUD pieces the first time they are needed in this scene.
fn ensure_hud(c: &mut Canvas) {
    if c.get_game_object(BTN).is_some() { return; }
    let d = AB_BTN.2;
    c.add_game_object(BTN.into(), hud_quad(BTN, (d, d), base_img(), 9_500));
    c.add_game_object(FILL.into(), hud_quad(FILL, (d, d), fill_img(0, false), 9_501));
    c.add_game_object(ICON.into(), hud_quad(ICON, (d, d), icon_img(0), 9_502));
    let mut count = GameObject::build(COUNT).position(AB_BTN.0 + d - 70.0, AB_BTN.1 + d - 64.0)
        .size(110.0, 60.0).layer(9_503).ignore_zoom().finish();
    count.visible = false;
    c.add_game_object(COUNT.into(), count);
    let aim = solid_px([230, 210, 255, 220]);
    c.add_game_object(AIM.into(), hud_quad(AIM, (12.0, 12.0), aim, 9_499));
}

fn solid_px(rgba: [u8; 4]) -> Arc<image::RgbaImage> {
    Arc::new(image::RgbaImage::from_pixel(1, 1, image::Rgba(rgba)))
}

// ── Firing ───────────────────────────────────────────────────────────────────

/// Fire the equipped ability. `aim`: a direction for Omni (any length).
/// Returns whether it fired.
pub fn fire(c: &mut Canvas, st: &Arc<Mutex<State>>, aim: Option<(f32, f32)>) -> bool {
    if !active(c) || crate::scenes::game::helpers::is_game_paused(c) { return false; }
    {
        let s = st.lock().unwrap();
        if s.dead || crate::scenes::game::boss::phase_cine_active(&s) || s.boss_stasis_active { return false; }
    }
    let kind = c.get_i32("ab_kind");
    let rank = c.get_i32("ab_rank").max(1) as f32;
    // The player's velocity, from the engine (the source of truth between ticks).
    let (vx, vy) = c.get_game_object("player").map_or((0.0, 0.0), |o| o.momentum);
    let (nvx, nvy, colour) = match kind {
        AB_BURST => {
            if c.get_f32("ab_bar") < 1.0 { return false; }
            c.set_var("ab_bar", 0.0f32);
            let speed = (vx * vx + vy * vy).sqrt();
            let (dx, dy) = if speed > 1.0 { (vx / speed, vy / speed) } else { (1.0, 0.0) };
            let push = speed * (BURST_SHARE + BURST_SHARE_PER_RANK * (rank - 1.0)) + BURST_FLOOR;
            (vx + dx * push, vy + dy * push, (1.0, 0.6, 0.25))
        }
        AB_SWAP => {
            if c.get_i32("ab_charges") <= 0 { return false; }
            c.set_var("ab_charges", c.get_i32("ab_charges") - 1);
            (-vx * SWAP_KICK, -vy * SWAP_KICK, (0.35, 0.9, 0.8))
        }
        AB_OMNI => {
            let Some((ax, ay)) = aim else { return false };
            let len = (ax * ax + ay * ay).sqrt();
            if len < 1e-3 || c.get_i32("ab_charges") <= 0 { return false; }
            c.set_var("ab_charges", c.get_i32("ab_charges") - 1);
            // Omni lets go of the rope: a boost the rope then strips away
            // would not go where it was aimed.
            if st.lock().unwrap().hooked {
                c.run(Action::Custom { name: "do_release".into() });
            }
            let (vx, vy) = c.get_game_object("player").map_or((vx, vy), |o| o.momentum);
            (vx * OMNI_KEEP + ax / len * OMNI_PUSH, vy * OMNI_KEEP + ay / len * OMNI_PUSH, (0.75, 0.5, 1.0))
        }
        _ => return false,
    };
    {
        let mut s = st.lock().unwrap();
        s.vx = nvx;
        s.vy = nvy;
        s.run_stats.abilities += 1;
    }
    let lift = match kind { AB_BURST => OVERDRIVE_BURST, AB_OMNI => OVERDRIVE_OMNI, _ => OVERDRIVE_SWAP }
        + OVERDRIVE_PER_RANK * (rank - 1.0);
    c.set_var("ab_overdrive_ticks", OVERDRIVE_TICKS);
    c.set_var("ab_overdrive_mult", lift);
    if let Some(o) = c.get_game_object_mut("player") { o.momentum = (nvx, nvy); }
    // Feedback: a burst ring at the player, a small kick, the button flashes.
    let at = { let s = st.lock().unwrap(); (s.px, s.py) };
    crate::scenes::game::boss::common::spawn_fx_ring(c, st, at, PLAYER_R * 6.0, colour, false);
    if let Some(cam) = c.camera_mut() {
        cam.shake(14.0, 0.15);
        cam.zoom_punch(0.025, 0.15);
    }
    c.set_var("ab_flash", 12i32);
    true
}

/// Fire from a key (E): Omni aims at the mouse cursor.
pub fn fire_from_key(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let aim = if c.get_i32("ab_kind") == AB_OMNI {
        let player = c.get_game_object("player")
            .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5));
        match (player, c.mouse_position()) {
            (Some(p), Some(m)) => {
                let ps = c.world_to_screen(p);
                let ms = c.input_to_unzoomed(m);
                Some((ms.0 - ps.0, ms.1 - ps.1))
            }
            // No cursor (a phone): straight up.
            _ => Some((0.0, -1.0)),
        }
    } else { None };
    fire(c, st, aim);
}

// ── Input: the button ────────────────────────────────────────────────────────

/// Register the button's press/move/release once. Returns nothing; the game's
/// own swing handler skips a press that lands on the button (`on_button`).
pub fn register_input(canvas: &mut Canvas) {
    if canvas.get_bool("ab_input_registered") { return; }
    canvas.set_var("ab_input_registered", true);
    canvas.on_mouse_press(|c, btn, pos| {
        // On a computer, a click anywhere (either button) fires the equipped
        // ability; the mouse no longer swings (SPACE does). Omni Boost aims
        // from the cat toward the cursor, as the E key does. The button stays
        // for touch screens. Not while a menu or a choice is up: those clicks
        // are theirs.
        if (btn == MouseButton::Left || btn == MouseButton::Right)
            && !crate::constants::mobile_controls_enabled() && active(c)
        {
            if click_is_for_ui(c) { return; }
            if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
                fire_from_key(c, &st);
            }
            return;
        }
        if btn != MouseButton::Left || !active(c) { return; }
        if matches!(c.get_var("pause_menu_open"), Some(Value::Bool(true))) { return; }
        let p = c.input_to_unzoomed(pos);
        if !on_button(p) { return; }
        c.set_var("mouse_left_held", false);
        if c.get_i32("ab_kind") == AB_OMNI {
            if c.get_i32("ab_charges") > 0 {
                c.set_var("ab_aiming", true);
                c.set_var("ab_aim_x", p.0);
                c.set_var("ab_aim_y", p.1);
            }
            return;
        }
        if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
            fire(c, &st, None);
        }
    });
    canvas.on_mouse_move(|c, pos| {
        if !active(c) || !c.get_bool("ab_aiming") { return; }
        let p = c.input_to_unzoomed(pos);
        c.set_var("ab_aim_x", p.0);
        c.set_var("ab_aim_y", p.1);
    });
    canvas.on_mouse_release(|c, _btn, pos| {
        if !active(c) || !c.get_bool("ab_aiming") { return; }
        c.set_var("ab_aiming", false);
        let p = c.input_to_unzoomed(pos);
        let (cx, cy) = btn_centre();
        let aim = (p.0 - cx, p.1 - cy);
        if (aim.0 * aim.0 + aim.1 * aim.1).sqrt() < OMNI_DEADZONE { return; }
        if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
            fire(c, &st, Some(aim));
        }
        // A press that started on the button must not leave the swing held.
        c.set_var("mouse_left_held", false);
    });
}

/// Whether a click belongs to something on screen rather than the ability:
/// the pause menu, the start prompt, an upgrade node's choices, the boss
/// reward cards, the cannon's fast-travel prompt, the death screen.
fn click_is_for_ui(c: &Canvas) -> bool {
    if c.get_bool("game_paused") || c.is_paused() || c.get_bool("start_prompt_active") { return true; }
    if matches!(c.get_var("pause_menu_open"), Some(Value::Bool(true))) { return true; }
    if crate::scenes::game::perks::reward_pending(c) || crate::scenes::game::death_screen::active(c) { return true; }
    if let Some(st) = crate::scenes::game::build_scene::run_state().lock().unwrap().as_ref().cloned() {
        let s = st.lock().unwrap();
        if s.upgrade_dialogue_active || (s.cannon_ft_prompt && s.cannon_captured) || s.dead { return true; }
    }
    false
}

// ── Per frame ────────────────────────────────────────────────────────────────

/// Refill the burst bar and draw the button. Every game tick.
/// Hide the button and everything with it (leaving the game). It is added to
/// the canvas at run start, not to the game scene, so the scene leaving does
/// not take it along: without this it stayed on the menu.
pub fn hide(c: &mut Canvas) {
    for n in [BTN, FILL, ICON, COUNT, AIM] {
        if let Some(o) = c.get_game_object_mut(n) { o.visible = false; }
    }
}

pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    ensure_hud(c);
    let kind = c.get_i32("ab_kind");
    let (dead, cine) = { let s = st.lock().unwrap(); (s.dead, crate::scenes::game::boss::phase_cine_active(&s)) };
    let show = c.get_var("ab_kind").is_some() && kind >= 0 && !dead && !cine
        && !matches!(c.get_var("pause_menu_open"), Some(Value::Bool(true)));
    for n in [BTN, FILL, ICON, COUNT] {
        if let Some(o) = c.get_game_object_mut(n) { o.visible = show && (n != FILL || kind == AB_BURST); }
    }
    if !show {
        if let Some(o) = c.get_game_object_mut(AIM) { o.visible = false; }
        return;
    }
    let rank = c.get_i32("ab_rank").max(1) as f32;
    // Swap / Omni: a charge comes back over time, up to rank + 1.
    if kind != AB_BURST && c.get_i32("ab_charges") < rank as i32 + 1 {
        let secs = (RECHARGE_S - RECHARGE_PER_RANK * (rank - 1.0)).max(8.0);
        let t = c.get_i32("ab_recharge") + 1;
        if t as f32 >= secs * 60.0 {
            c.set_var("ab_recharge", 0i32);
            c.set_var("ab_charges", c.get_i32("ab_charges") + 1);
        } else {
            c.set_var("ab_recharge", t);
        }
    }
    // Burst: the bar refills.
    if kind == AB_BURST {
        let secs = (BURST_REFILL_S - BURST_REFILL_PER_RANK * (rank - 1.0)).max(2.0);
        let bar = (c.get_f32("ab_bar") + 1.0 / (secs * 60.0)).min(1.0);
        c.set_var("ab_bar", bar);
        let step = (bar * 24.0).floor() as i32;
        let ready = bar >= 1.0;
        let key = step * 2 + ready as i32;
        if c.get_i32("ab_last_fill") != key {
            c.set_var("ab_last_fill", key);
            let d = AB_BTN.2;
            if let Some(o) = c.get_game_object_mut(FILL) {
                o.set_image(Image { shape: ShapeType::Rectangle(0.0, (d, d), 0.0), image: fill_img(step as usize, ready), color: None });
            }
        }
    }
    // The icon: dimmed when there is nothing to fire, flashing when fired.
    let flash = c.get_i32("ab_flash");
    if flash > 0 { c.set_var("ab_flash", flash - 1); }
    let usable = match kind { AB_BURST => c.get_f32("ab_bar") >= 1.0, _ => c.get_i32("ab_charges") > 0 };
    let d = AB_BTN.2;
    let alpha = if usable { 255 } else { 90 };
    let pulse = if usable && kind == AB_BURST { 1.0 + 0.05 * (c.get_i32("ab_pulse_t") as f32 * 0.12).sin() } else { 1.0 };
    c.set_var("ab_pulse_t", c.get_i32("ab_pulse_t") + 1);
    let sz = d * pulse * if flash > 0 { 1.12 } else { 1.0 };
    // The ability's animated emblem (the one on its shop card), bright when
    // it can be fired, dim when it cannot.
    if let Some(o) = c.get_game_object_mut(ICON) {
        o.size = (sz, sz);
        o.position = (AB_BTN.0 - (sz - d) * 0.5, AB_BTN.1 - (sz - d) * 0.5);
        o.set_image(Image { shape: ShapeType::Rectangle(0.0, (sz, sz), 0.0), image: clear_px(), color: None });
    }
    let _ = alpha;
    if let Some(a) = crate::profile::ABILITIES.get(kind.max(0) as usize) {
        let (r, g, b) = a.color;
        let t = c.get_i32("ab_pulse_t") as f32 / 60.0;
        c.attach_effect(ICON, Effect::ShopIcon { icon: 7 + kind.max(0) as u32, time: t, intensity: if usable { 1.0 } else { 0.35 } },
            EffectColor::srgb8(r, g, b), (sz * 1.15, sz * 1.15));
    }
    // Uses left (not for the burst, whose bar says it).
    if let Some(font) = crate::scenes::game::helpers::ui_font() {
        let shown = if kind == AB_BURST { -1 } else { c.get_i32("ab_charges") };
        if c.get_i32("ab_count_shown") != shown || c.get_var("ab_count_shown").is_none() {
            c.set_var("ab_count_shown", shown);
            let text = if shown < 0 { String::new() } else { format!("x{shown}") };
            let s = c.virtual_scale();
            if let Some(o) = c.get_game_object_mut(COUNT) {
                o.set_drawable(Box::new(crate::objects::ui_text_spec(&text, &font, 40.0 * s, Color(255, 255, 255, 240), 110.0 * s)));
            }
        }
    }
    // Omni's stick: a line from the button to the finger while aiming.
    let aiming = kind == AB_OMNI && c.get_bool("ab_aiming");
    if let Some(o) = c.get_game_object_mut(AIM) { o.visible = aiming; }
    if aiming {
        let (cx, cy) = btn_centre();
        let (ax, ay) = (c.get_f32("ab_aim_x") - cx, c.get_f32("ab_aim_y") - cy);
        let len = (ax * ax + ay * ay).sqrt().min(420.0);
        if len > 1.0 {
            let ang = ay.atan2(ax);
            if let Some(o) = c.get_game_object_mut(AIM) {
                o.size = (len, 12.0);
                o.position = (cx + ang.cos() * len * 0.5 - len * 0.5, cy + ang.sin() * len * 0.5 - 6.0);
                o.rotation = ang.to_degrees();
                o.set_image(Image { shape: ShapeType::Rectangle(0.0, (len, 12.0), 0.0), image: solid_px([230, 210, 255, 220]), color: None });
            }
        }
    }
}
