use crate::constants::*;
use crate::state::State;

pub fn zone_index_for_distance(distance: f32) -> usize {
    ((distance / ZONE_DISTANCE_STEP) as usize).min(2)
}

pub fn spinner_speed_for_zone(zone_idx: usize) -> f32 {
    let mult = match zone_idx {
        0 => START_ZONE_SPINNER_MULT,
        1 => PURPLE_ZONE_SPINNER_MULT,
        _ => BLACK_ZONE_SPINNER_MULT,
    };
    SPINNER_ROT_SPEED * mult
}

pub fn apply_release_impulse(s: &mut State) {
    // Convert current velocity into tangent space relative to active hook.
    let dx = s.px - s.hook_x;
    let dy = s.py - s.hook_y;
    let dist = (dx * dx + dy * dy).sqrt().max(1.0);
    let nx = dx / dist;
    let ny = dy / dist;
    let tx = -ny;
    let ty = nx;
    let tangent_v = s.vx * tx + s.vy * ty;
    let swing_speed = tangent_v.abs();

    let surge = ((swing_speed - RELEASE_MIN_SWING_SPEED).max(0.0) * RELEASE_SURGE_SCALE)
        .clamp(0.0, RELEASE_SURGE_MAX);
    if surge > 0.0 {
        let dir = if tangent_v.abs() > 0.01 {
            tangent_v.signum()
        } else {
            1.0
        };
        s.vx += tx * surge * dir;
        s.vy += ty * surge * dir;
    }

    // Keep existing launch feel.
    s.vx *= 2.0;
    s.vy *= 2.0;
}

pub fn apply_grab_impulse(s: &mut State, hx: f32, hy: f32) {
    let speed = (s.vx * s.vx + s.vy * s.vy).sqrt();
    if speed >= GRAB_SPIN_DISABLE_SPEED {
        return;
    }

    // Add attach surge along tangent to preserve swing flow.
    let dx = s.px - hx;
    let dy = s.py - hy;
    let inv_dist = 1.0 / (dx * dx + dy * dy).sqrt().max(1.0);
    let nx = dx * inv_dist;
    let ny = dy * inv_dist;
    let tx = -ny;
    let ty = nx;
    let tangent_v = s.vx * tx + s.vy * ty;

    let dir = if tangent_v.abs() > 0.05 {
        tangent_v.signum()
    } else if s.vx.abs() > 0.05 {
        s.vx.signum()
    } else if s.px >= hx {
        1.0
    } else {
        -1.0
    };

    s.vx += tx * GRAB_SURGE * dir * GRAB_SURGE_MULT;
    s.vy += ty * GRAB_SURGE * dir * GRAB_SURGE_MULT;
    let tangent_surge = (tangent_v.abs() * GRAB_TANGENT_SURGE_SCALE)
        .clamp(0.0, GRAB_TANGENT_SURGE_MAX);
    s.vx += tx * tangent_surge * dir * GRAB_SURGE_MULT;
    s.vy += ty * tangent_surge * dir * GRAB_SURGE_MULT;
}
