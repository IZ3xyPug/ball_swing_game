use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::images::gwell_ring_cached;
use crate::state::*;

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
                let ring_img = gwell_ring_cached(
                    visual_r,
                    C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2,
                    GWELL_RING_COUNT, 200.0,
                );
                let d = visual_r * 2.0;
                obj.set_image(Image {
                    shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                    image: ring_img,
                    color: None,
                });
                obj.set_glow(GlowConfig {
                    color: Color(C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2, 200),
                    width: 14.0,
                });
            } else {
                obj.planet_radius = None;
                let ring_img = gwell_ring_cached(
                    visual_r,
                    C_GWELL_DORMANT.0, C_GWELL_DORMANT.1, C_GWELL_DORMANT.2,
                    GWELL_RING_COUNT, 80.0,
                );
                let d = visual_r * 2.0;
                obj.set_image(Image {
                    shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
                    image: ring_img,
                    color: None,
                });
                obj.set_glow(GlowConfig {
                    color: Color(C_GWELL_DORMANT.0, C_GWELL_DORMANT.1, C_GWELL_DORMANT.2, 60),
                    width: 6.0,
                });
            }
        }
    }

    // Visual pulse for active wells — modulate glow width/alpha.
    for (id, _, active) in &timers {
        if !active { continue; }
        if let Some(obj) = c.get_game_object_mut(id) {
            let t = frame as f32 * GWELL_PULSE_SPEED;
            let pulse = GWELL_PULSE_MIN + (1.0 - GWELL_PULSE_MIN) * ((t.sin() + 1.0) * 0.5);
            let glow_w = 14.0 * pulse;
            obj.set_glow(GlowConfig {
                color: Color(C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2, (200.0 * pulse) as u8),
                width: glow_w,
            });
        }
    }
}
