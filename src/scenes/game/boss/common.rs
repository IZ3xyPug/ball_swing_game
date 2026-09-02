use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use crate::images::circle_cached;
use crate::scenes::game::bootstrap::hook_asteroid_anim_for_spawn;
use crate::scenes::game::helpers::center_warp_on_player;
use crate::scenes::game::space_zone::wormhole2_template;
#[allow(unused_imports)]
use super::*;

/// Total remaining HP across all multi-part parts (drives the HUD + win check).
pub fn boss_total_hp(s: &State) -> i32 {
    s.boss_parts.iter().filter(|p| p.alive).map(|p| p.hp.max(0)).sum()
}

pub(crate) fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

pub(crate) fn lerp2(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (lerp(a.0, b.0, t), lerp(a.1, b.1, t))
}

/// Move `from` toward `to` by up to `cap` px per tick for `ticks` ticks, clamped
/// so the part never travels faster than `cap` (the player's momentum cap) and
/// never overshoots the destination. This is what keeps the Colossus's attack
/// lunges fair — the boss moves at the same speed ceiling the player does.
pub(crate) fn capped_toward(from: (f32, f32), to: (f32, f32), ticks: u32, cap: f32) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 0.001 { return to; }
    let traveled = (cap * ticks as f32).min(dist);
    (from.0 + dx / dist * traveled, from.1 + dy / dist * traveled)
}

/// Distance from `p` to the line segment `a`→`b`. Used so the head's gaze beam
/// can hit the player if they stand anywhere along the telegraphed path.
pub(crate) fn point_segment_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let abx = b.0 - a.0;
    let aby = b.1 - a.1;
    let len2 = abx * abx + aby * aby;
    if len2 < 0.0001 { return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt(); }
    let t = (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len2).clamp(0.0, 1.0);
    let cx = a.0 + abx * t;
    let cy = a.1 + aby * t;
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// A point on the gaze beam at parameter `t` (0 = at the head, 1 = the far end).
///
/// A quadratic bezier whose control point is offset perpendicular to the chord,
/// so `curve == 0.0` collapses to a straight line and every caller — drawing,
/// hit testing, the travelling core — uses this one function for both. Keeping
/// a straight beam as a special case would have let the drawn path and the
/// damaging path disagree, which on a beam this wide is the difference between
/// a fair attack and an unreadable one.
pub(crate) fn beam_point(start: (f32, f32), end: (f32, f32), curve: f32, t: f32) -> (f32, f32) {
    if curve.abs() < 0.0001 {
        return lerp2(start, end, t);
    }
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let (nx, ny) = (-dy / len, dx / len);
    let mid = ((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5);
    let ctrl = (mid.0 + nx * curve * len, mid.1 + ny * curve * len);
    let u = 1.0 - t;
    (
        u * u * start.0 + 2.0 * u * t * ctrl.0 + t * t * end.0,
        u * u * start.1 + 2.0 * u * t * ctrl.1 + t * t * end.1,
    )
}

/// Sample the beam from the head out to `t_max` as a polyline.
pub(crate) fn beam_polyline(start: (f32, f32), end: (f32, f32), curve: f32, t_max: f32) -> Vec<(f32, f32)> {
    beam_polyline_range(start, end, curve, 0.0, t_max)
}

/// The beam between two parameters along it. `t0 == 0.0` is the head, `t1 ==
/// 1.0` the far end.
///
/// The telegraph uses this to draw only the stretch AHEAD of the sweep: the
/// part already passed is covered by the bright core, so drawing the full ray
/// under it was a second full-length translucent quad for nothing. It also
/// reads better — the telegraph is "where this is about to reach", and it is
/// consumed as the beam travels.
pub(crate) fn beam_polyline_range(
    start: (f32, f32),
    end: (f32, f32),
    curve: f32,
    t0: f32,
    t1: f32,
) -> Vec<(f32, f32)> {
    let (t0, t1) = (t0.clamp(0.0, 1.0), t1.clamp(0.0, 1.0));
    if t1 <= t0 {
        return Vec::new();
    }
    // A straight beam is exactly one quad, whatever range of it is drawn.
    if curve.abs() < 0.0001 {
        return vec![
            beam_point(start, end, 0.0, t0),
            beam_point(start, end, 0.0, t1),
        ];
    }
    let n = COLOSSUS_BEAM_SEGMENTS;
    (0..=n)
        .map(|i| beam_point(start, end, curve, t0 + (t1 - t0) * i as f32 / n as f32))
        .collect()
}

/// Distance from `p` to the beam, as drawn. Segment-wise against the same
/// polyline the renderer uses, so a curved beam damages where it looks like it
/// does.
pub(crate) fn beam_dist(p: (f32, f32), pts: &[(f32, f32)]) -> f32 {
    pts.windows(2)
        .map(|w| point_segment_dist(p, w[0], w[1]))
        .fold(f32::MAX, f32::min)
}

/// Half-width of the beam's damaging area. The drawn thickness, not a hidden
/// margin on top of it.
pub(crate) fn beam_hit_radius() -> f32 {
    COLOSSUS_BEAM_THICKNESS * 0.5 + PLAYER_R
}

/// The far end of a beam aimed from `start` through `aim`: a ray of fixed
/// length, so the beam does not politely stop at the player.
pub(crate) fn beam_end(start: (f32, f32), aim: (f32, f32)) -> (f32, f32) {
    let dx = aim.0 - start.0;
    let dy = aim.1 - start.1;
    let d = (dx * dx + dy * dy).sqrt();
    if d < 1.0 {
        return (start.0 + COLOSSUS_BEAM_LENGTH, start.1);
    }
    (
        start.0 + dx / d * COLOSSUS_BEAM_LENGTH,
        start.1 + dy / d * COLOSSUS_BEAM_LENGTH,
    )
}

/// Length of one beam in the burst: the sweep plus the pause after it.
pub(crate) fn beam_shot_len() -> u32 {
    COLOSSUS_BEAM_TICKS + COLOSSUS_BEAM_GAP_TICKS
}

/// Lay a pool of rectangles along `pts` so a curved beam reads as one
/// continuous band.
///
/// `prefix` names a pool (`colossus_beam_tel_` / `colossus_beam_core_`) of
/// `COLOSSUS_BEAM_SEGMENTS` objects. `rotation_adjusted_offset` keeps a rotated
/// object's rendered centre at `position + size/2`, so positioning each segment
/// by its own midpoint is enough.
pub(crate) fn draw_beam_strip(c: &mut Canvas, prefix: &str, pts: &[(f32, f32)], thickness: f32) {
    for i in 0..COLOSSUS_BEAM_SEGMENTS {
        let name = format!("{prefix}_{i}");
        let Some(seg) = pts.get(i).zip(pts.get(i + 1)) else {
            if let Some(obj) = c.get_game_object_mut(&name) { obj.visible = false; }
            continue;
        };
        let ((ax, ay), (bx, by)) = (*seg.0, *seg.1);
        let dx = bx - ax;
        let dy = by - ay;
        // Overlap each segment slightly so the joints of a curve do not show as
        // notches along the edge of the band.
        let len = (dx * dx + dy * dy).sqrt().max(1.0) + thickness * 0.25;
        let deg = dy.atan2(dx).to_degrees();
        let mid = ((ax + bx) * 0.5, (ay + by) * 0.5);
        if let Some(obj) = c.get_game_object_mut(&name) {
            obj.size = (len, thickness);
            obj.rotation = deg;
            obj.position = (mid.0 - len * 0.5, mid.1 - thickness * 0.5);
            obj.visible = true;
        }
    }
}

/// Hide every segment of a beam strip pool.
pub(crate) fn hide_beam_strip(c: &mut Canvas, prefix: &str) {
    for i in 0..COLOSSUS_BEAM_SEGMENTS {
        if let Some(obj) = c.get_game_object_mut(&format!("{prefix}_{i}")) {
            obj.visible = false;
        }
    }
}

/// Clamp `to` so it is at most `max` px from `from` — the leash that keeps a
/// part loosely tethered to its home orbit even while attacking.
pub(crate) fn leash_clamp(from: (f32, f32), to: (f32, f32), max: f32) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let d = (dx * dx + dy * dy).sqrt();
    if d <= max || d < 0.001 { return to; }
    let f = max / d;
    (from.0 + dx * f, from.1 + dy * f)
}
