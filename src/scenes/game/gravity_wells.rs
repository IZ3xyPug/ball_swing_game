use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;

static GWELLON_TEMPLATE:  OnceLock<AnimatedSprite> = OnceLock::new();
static GWELLOFF_TEMPLATE: OnceLock<AnimatedSprite> = OnceLock::new();

fn gwellon_template() -> AnimatedSprite {
    GWELLON_TEMPLATE.get_or_init(|| {
        AnimatedSprite::new(include_bytes!("../../../assets/gwellon.gif"), (256.0, 256.0), GWELL_FPS)
            .expect("gwellon.gif decode")
    }).clone()
}
fn gwelloff_template() -> AnimatedSprite {
    GWELLOFF_TEMPLATE.get_or_init(|| {
        AnimatedSprite::new(include_bytes!("../../../assets/gwelloff.gif"), (256.0, 256.0), GWELL_FPS)
            .expect("gwelloff.gif decode")
    }).clone()
}

/// Tick the gravity-well on/off cycle and visual pulse.
pub fn tick_gravity_wells(c: &mut Canvas, st: &Arc<Mutex<State>>, frame: u32) {
    let mut s = st.lock().unwrap();
    let mut toggle_ids: Vec<(String, bool)> = Vec::new();

    for (id, remaining, active) in s.gwell_timers.iter_mut() {
        if *remaining > 0 {
            *remaining -= 1;
        }
        if *remaining == 0 {
            *active = !*active;
            *remaining = if *active { GWELL_ON_TICKS } else { GWELL_OFF_TICKS };
            toggle_ids.push((id.clone(), *active));
        }
    }

    let timers = s.gwell_timers.clone();
    drop(s);

    // Apply toggles: swap between active ring image and dormant ring image.
    for (id, now_active) in &toggle_ids {
        if let Some(obj) = c.get_game_object_mut(id) {
            let visual_r = obj.size.0 * 0.5;
            if *now_active {
                obj.planet_radius = Some(obj.planet_radius.unwrap_or(GWELL_RADIUS_MIN));
                let d = visual_r * 2.0;
                obj.size = (d, d);
                obj.set_animation(gwellon_template());
            } else {
                obj.planet_radius = None;
                let d = visual_r * 2.0;
                obj.size = (d, d);
                obj.set_animation(gwelloff_template());
            }
        }
    }

    // Disconnect player from grab node when close to an active well center.
    let s = st.lock().unwrap();
    let hooked = s.hooked;
    let px = s.px;
    let py = s.py;
    let asteroid_ids = s.space_asteroid_live.clone();
    drop(s);

    if hooked {
        for (id, _, active) in &timers {
            if !*active { continue; }
            if let Some(obj) = c.get_game_object(id) {
                let well_cx = obj.position.0 + obj.size.0 * 0.5;
                let well_cy = obj.position.1 + obj.size.1 * 0.5;
                let pr = obj.planet_radius.unwrap_or(0.0);
                if pr <= 0.0 { continue; }
                let disconnect_r = pr * GWELL_DISCONNECT_FRAC;
                let dx = px - well_cx;
                let dy = py - well_cy;
                if dx * dx + dy * dy < disconnect_r * disconnect_r {
                    c.run(Action::Custom { name: "do_release".into() });
                    break;
                }
            }
        }
    }

    // ── Pull live asteroids toward active gravity wells ───────────────────
    for ast_id in &asteroid_ids {
        let ast_snap = {
            if let Some(obj) = c.get_game_object(ast_id) {
                if !obj.visible { continue; }
                Some((
                    obj.position.0 + obj.size.0 * 0.5,
                    obj.position.1 + obj.size.1 * 0.5,
                    obj.momentum.0,
                    obj.momentum.1,
                ))
            } else { None }
        };
        let Some((ax, ay, amx, amy)) = ast_snap else { continue };

        let mut force_x = 0.0f32;
        let mut force_y = 0.0f32;

        for (id, _, active) in &timers {
            if !*active { continue; }
            if let Some(obj) = c.get_game_object(id) {
                let pr = obj.planet_radius.unwrap_or(0.0);
                if pr <= 0.0 { continue; }
                let strength_scale = obj.gravity_strength.max(GWELL_STRENGTH_MIN);
                let well_cx = obj.position.0 + obj.size.0 * 0.5;
                let well_cy = obj.position.1 + obj.size.1 * 0.5;
                let dx = well_cx - ax;
                let dy = well_cy - ay;
                let dist2 = dx * dx + dy * dy;
                if dist2 < pr * pr {
                    let dist = dist2.sqrt().max(1.0);
                    let strength = strength_scale * (1.0 - dist / pr);
                    force_x += dx / dist * strength;
                    force_y += dy / dist * strength;
                }
            }
        }

        if force_x.abs() > 0.001 || force_y.abs() > 0.001 {
            if let Some(obj) = c.get_game_object_mut(ast_id) {
                obj.momentum.0 = amx + force_x;
                obj.momentum.1 = amy + force_y;
            }
        }
    }
}
