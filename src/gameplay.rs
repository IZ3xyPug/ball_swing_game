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

/// Compute the release impulse given current velocity + hook position.
/// Returns the new (vx, vy).
pub fn compute_release_impulse(
    px: f32, py: f32,
    vx: f32, vy: f32,
    hook_x: f32, hook_y: f32,
) -> (f32, f32) {
    let dx = px - hook_x;
    let dy = py - hook_y;
    let dist = (dx * dx + dy * dy).sqrt().max(1.0);
    let nx = dx / dist;
    let ny = dy / dist;
    let tx = -ny;
    let ty = nx;
    let tangent_v = vx * tx + vy * ty;
    let swing_speed = tangent_v.abs();

    let mut nvx = vx;
    let mut nvy = vy;

    let surge = ((swing_speed - RELEASE_MIN_SWING_SPEED).max(0.0) * RELEASE_SURGE_SCALE)
        .clamp(0.0, RELEASE_SURGE_MAX);
    if surge > 0.0 {
        let dir = if tangent_v.abs() > 0.01 {
            tangent_v.signum()
        } else {
            1.0
        };
        nvx += tx * surge * dir;
        nvy += ty * surge * dir;
    }

    // Keep existing launch feel.
    nvx *= 2.0;
    nvy *= 2.0;
    nvy *= RELEASE_VERTICAL_BOOST;
    (nvx, nvy)
}

/// Compute the grab impulse given current velocity + hook center position.
/// Returns the new (vx, vy).
pub fn compute_grab_impulse(
    px: f32, py: f32,
    vx: f32, vy: f32,
    hx: f32, hy: f32,
) -> (f32, f32) {
    let speed = (vx * vx + vy * vy).sqrt();
    if speed >= GRAB_SPIN_DISABLE_SPEED {
        return (vx, vy);
    }

    let dx = px - hx;
    let dy = py - hy;
    let inv_dist = 1.0 / (dx * dx + dy * dy).sqrt().max(1.0);
    let nx = dx * inv_dist;
    let ny = dy * inv_dist;
    let tx = -ny;
    let ty = nx;
    let tangent_v = vx * tx + vy * ty;

    let dir = if tangent_v.abs() > 0.05 {
        tangent_v.signum()
    } else if vx.abs() > 0.05 {
        vx.signum()
    } else if px >= hx {
        1.0
    } else {
        -1.0
    };

    let mut nvx = vx;
    let mut nvy = vy;

    nvx += tx * GRAB_SURGE * dir * GRAB_SURGE_MULT;
    nvy += ty * GRAB_SURGE * dir * GRAB_SURGE_MULT;
    let tangent_surge = (tangent_v.abs() * GRAB_TANGENT_SURGE_SCALE)
        .clamp(0.0, GRAB_TANGENT_SURGE_MAX);
    nvx += tx * tangent_surge * dir * GRAB_SURGE_MULT;
    nvy += ty * tangent_surge * dir * GRAB_SURGE_MULT;
    nvy *= GRAB_VERTICAL_BOOST;
    (nvx, nvy)
}

/// Legacy wrappers that modify State directly (used by custom events).
pub fn apply_release_impulse(s: &mut State) {
    let (nvx, nvy) = compute_release_impulse(s.px, s.py, s.vx, s.vy, s.hook_x, s.hook_y);
    s.vx = nvx;
    s.vy = nvy;
}

pub fn apply_grab_impulse(s: &mut State, hx: f32, hy: f32) {
    let (nvx, nvy) = compute_grab_impulse(s.px, s.py, s.vx, s.vy, hx, hy);
    s.vx = nvx;
    s.vy = nvy;
}
