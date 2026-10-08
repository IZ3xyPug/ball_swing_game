//! seed_funds.rs — the SEED FUNDS upgrade, seen being paid.
//!
//! The upgrade starts a run with coins in hand. Starting the counter already
//! at 25 said nothing — it looked like any other run — so the coins are paid
//! IN on the start screen instead: a beat after it appears, the counter
//! climbs from 0 and a gold "+25" rises beside it. Launching early pays the
//! rest at once, so the coins never depend on waiting.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::state::State;

const POP: &str = "seed_pop";
/// Ticks into the start screen before the coins arrive.
const DELAY: i32 = 35;
/// Ticks the counter takes to climb.
const CLIMB: u32 = 30;
/// The "+N": rises this far and lasts this long.
const POP_RISE: f32 = 70.0;
const POP_TICKS: i32 = 80;
/// Just under the coin counter (`coin_counter`, 640x168 at 26,24).
const POP_AT: (f32, f32) = (230.0, 200.0);
const POP_PX: f32 = 96.0;

/// A new run: `coins` to pay in on the start screen.
pub fn begin(c: &mut Canvas, coins: u32) {
    c.set_var("seed_pending", coins as i32);
    c.set_var("seed_total", coins as i32);
    c.set_var("seed_t", 0i32);
    c.set_var("seed_pop_t", -1i32);
    if let Some(o) = c.get_game_object_mut(POP) { o.visible = false; }
}

/// Every game frame, start screen included.
pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let pending = c.get_i32("seed_pending");
    let pop_t = c.get_i32("seed_pop_t");
    if pending <= 0 && pop_t < 0 { return; }
    let t = c.get_i32("seed_t") + 1;
    c.set_var("seed_t", t);
    let waiting = c.get_bool("start_prompt_active");

    if pending > 0 && (t >= DELAY || !waiting) {
        let total = c.get_i32("seed_total").max(1);
        let give = if waiting { ((total as u32 + CLIMB - 1) / CLIMB).max(1) as i32 } else { pending };
        let give = give.min(pending);
        st.lock().unwrap().coin_count += give as u32;
        c.set_var("seed_pending", pending - give);
        // The HUD is not refreshed on the start screen; the counter climbing
        // is the point, so refresh it here.
        if waiting { super::hud_update::tick_hud(c, st); }
        if pop_t < 0 { show_pop(c, total); c.set_var("seed_pop_t", 0i32); }
    }

    let pop_t = c.get_i32("seed_pop_t");
    if pop_t >= 0 {
        let k = pop_t as f32 / POP_TICKS as f32;
        let a = if k < 0.5 { 1.0 } else { (1.0 - (k - 0.5) / 0.5).max(0.0) };
        // A small overshoot on the way in, so it POPS.
        let s = if pop_t < 8 { 0.6 + 0.6 * (pop_t as f32 / 8.0) } else { 1.2 - 0.2 * ((pop_t - 8) as f32 / 8.0).min(1.0) };
        let (w, h) = (c.get_f32("seed_pop_w"), c.get_f32("seed_pop_h"));
        let art = crate::menu_look::glow_text(&format!("+{}", c.get_i32("seed_total")), POP_PX);
        if let Some(o) = c.get_game_object_mut(POP) {
            o.size = (w * s, h * s);
            o.position = (POP_AT.0, POP_AT.1 - POP_RISE * k - (h * s - h) * 0.5);
            o.visible = a > 0.01;
            if let Some(img) = art {
                o.set_image(Image { shape: ShapeType::Rectangle(0.0, o.size, 0.0), image: img,
                    color: Some(Color(255, 215, 90, (a * 255.0) as u8)) });
            }
        }
        if pop_t >= POP_TICKS {
            c.set_var("seed_pop_t", -1i32);
            if let Some(o) = c.get_game_object_mut(POP) { o.visible = false; }
        } else {
            c.set_var("seed_pop_t", pop_t + 1);
        }
    }
}

fn show_pop(c: &mut Canvas, total: i32) {
    let Some(img) = crate::menu_look::glow_text(&format!("+{total}"), POP_PX) else { return };
    let size = (img.width() as f32, img.height() as f32);
    if c.get_game_object(POP).is_none() {
        let mut o = GameObject::build(POP).position(POP_AT.0, POP_AT.1).size(size.0, size.1)
            .image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: img, color: None })
            .layer(101).ignore_zoom().finish();
        o.unlit = true;
        o.gravity = 0.0;
        c.add_game_object(POP.into(), o);
    }
    // The art's own size, kept for the pop's scaling.
    c.set_var("seed_pop_w", size.0);
    c.set_var("seed_pop_h", size.1);
    if let Some(o) = c.get_game_object_mut(POP) {
        o.size = size;
        o.visible = true;
    }
}
