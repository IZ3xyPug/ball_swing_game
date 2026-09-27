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


/// Register a hit at `pos`, `size` across.
///
/// ## Why hits need their own effect
///
/// Damage used to be reported by a camera flash and a health bar moving. Both
/// are AWAY from the thing that was hit, and the health bar is a slow readout
/// at the top of the screen — so a landed hit and a missed one felt almost the
/// same in the moment, which is the moment that matters in a boss fight.
///
/// Silently does nothing when the pool is empty rather than stealing the
/// oldest burst: the hit that gets dropped in a busy frame is the one the
/// player is least likely to be watching, and cutting a burst short mid-flash
/// looks like a rendering fault.
pub fn spawn_impact(
    c: &mut quartz::Canvas,
    st: &std::sync::Arc<std::sync::Mutex<crate::state::State>>,
    pos: (f32, f32),
    size: f32,
    rgb: (f32, f32, f32),
    taken: bool,
) {
    use crate::constants::*;
    let kind = if taken { IMPACT_KIND_TAKEN } else { IMPACT_KIND_DEALT };
    spawn_pooled_fx(c, st, pos, size * IMPACT_SCALE, rgb, kind, IMPACT_TICKS);
}

/// A sonic pulse at `pos` — the player's own beat feedback.
///
/// Green when a release scores, red when a stack is lost. A PULSE rather than
/// the hit burst: scoring a beat is not an impact, and giving it the same
/// shatter as landing a hit on the boss would make the two read alike in a
/// fight where both happen seconds apart.
pub fn spawn_beat_pulse(
    c: &mut quartz::Canvas,
    st: &std::sync::Arc<std::sync::Mutex<crate::state::State>>,
    pos: (f32, f32),
    hit: bool,
) {
    use crate::constants::*;
    let rgb = if hit { BEAT_HIT_RGB } else { BEAT_MISS_RGB };
    spawn_pooled_fx(c, st, pos, PLAYER_R * 2.0 * BEAT_PULSE_SCALE, rgb,
                    IMPACT_KIND_PULSE, BEAT_PULSE_TICKS);
}

fn spawn_pooled_fx(
    c: &mut quartz::Canvas,
    st: &std::sync::Arc<std::sync::Mutex<crate::state::State>>,
    pos: (f32, f32),
    d: f32,
    rgb: (f32, f32, f32),
    kind: u32,
    ticks: u32,
) {
    let id = {
        let mut s = st.lock().unwrap();
        match s.impact_free.pop() {
            Some(id) => {
                s.impact_live.push((id.clone(), ticks, d, rgb, kind));
                id
            }
            None => return,
        }
    };
    if let Some(o) = c.get_game_object_mut(&id) {
        o.size = (d, d);
        o.position = (pos.0 - d * 0.5, pos.1 - d * 0.5);
        o.visible = true;
    }
}

/// Advance every live hit burst, and hand expired ones back to the pool.
pub fn tick_impacts(
    c: &mut quartz::Canvas,
    st: &std::sync::Arc<std::sync::Mutex<crate::state::State>>,
) {
    use crate::constants::*;
    let live = { st.lock().unwrap().impact_live.clone() };
    if live.is_empty() {
        return;
    }
    let mut keep = Vec::with_capacity(live.len());
    let mut freed = Vec::new();
    for (id, ticks, d, rgb, kind) in live {
        let left = ticks.saturating_sub(1);
        if left == 0 {
            if let Some(o) = c.get_game_object_mut(&id) {
                o.visible = false;
                o.size = (8.0, 8.0);
                o.position = (-9000.0, -9000.0);
            }
            c.clear_effect(&id);
            freed.push(id);
            continue;
        }
        // Age is normalised against whatever this effect's own lifetime was,
        // recovered from the tick it started at — a pulse lives fewer ticks
        // than a burst, and dividing both by the burst's length would make a
        // pulse start at 70% and never reach full brightness.
        let total = if kind == IMPACT_KIND_PULSE { BEAT_PULSE_TICKS } else { IMPACT_TICKS };
        let age = left as f32 / total as f32;
        if kind == IMPACT_KIND_PULSE {
            c.attach_effect(
                &id, Effect::SonicRing { intensity: age },
                crate::scenes::game::fx::lin(rgb), (d, d));
        } else {
            let side = if kind == IMPACT_KIND_TAKEN { ImpactSide::Taken } else { ImpactSide::Dealt };
            c.attach_effect(
                &id, Effect::Impact { age, side },
                crate::scenes::game::fx::lin(rgb), (d, d));
        }
        keep.push((id, left, d, rgb, kind));
    }
    let mut s = st.lock().unwrap();
    s.impact_live = keep;
    s.impact_free.extend(freed);
}

#[cfg(test)]
mod impact_tests {
    use crate::constants::*;

    /// The pool must be big enough for the worst honest case: the Colossus can
    /// lose a part while two more are striking and the player is being hit.
    #[test]
    fn the_pool_covers_a_busy_frame() {
        assert!(IMPACT_POOL_SIZE >= 4,
                "a pool of {IMPACT_POOL_SIZE} will drop hits in a busy frame, \
                 and a hit that does not draw teaches the player it missed");
    }

    /// Short enough to read as an instant. Much past a third of a second and a
    /// burst stops being an event and starts being a state the part is in.
    #[test]
    fn a_burst_is_brief() {
        assert!(IMPACT_TICKS <= 30,
                "{IMPACT_TICKS} ticks is over half a second — too long to read \
                 as the moment of impact");
        assert!(IMPACT_TICKS >= 8,
                "{IMPACT_TICKS} ticks may not survive a dropped frame");
    }

    /// Drawn bigger than the thing it happened to, or the burst hides inside
    /// the sprite it is meant to be confirming.
    #[test]
    fn a_burst_is_bigger_than_what_was_hit() {
        assert!(IMPACT_SCALE > 1.0, "impacts draw no larger than the target");
    }

    /// Damage TAKEN is red and nothing else in the fight's state vocabulary
    /// is, so it cannot be mistaken for a wind-up warning at a glance.
    #[test]
    fn taken_damage_is_unmistakable() {
        let (r, g, b) = IMPACT_TAKEN_RGB;
        assert!(r > 0.8 && g < 0.4 && b < 0.4, "taken damage is not clearly red");
        // And clearly distinct from the colour a hit DEALT uses.
        let (dr, dg, db) = IMPACT_DEALT_RGB;
        let dist = (r - dr).abs() + (g - dg).abs() + (b - db).abs();
        assert!(dist > 0.8,
                "dealt and taken damage are too close in colour ({dist:.2})");
    }
}

// ── Solid parts and hit feel ────────────────────────────────────────────────
//
// A boss part is SOLID: the player cannot pass through it, and touching it
// throws them back off its surface. And a hit that lands is FELT: the part
// flashes and shakes, the player hangs at the contact point for a few
// frames (hit-stop), the camera kicks, and they rebound. Before this, a
// buffed hit on an open part passed straight through it with a burst at a
// distance — nothing said "that connected", and nothing stopped the player
// sailing into a closed part except the heart it cost.

/// Resolve the player against a boss part's collision circle (`r` around
/// `centre`): pushed out to its surface, inward speed reflected with
/// `BOSS_PART_RESTITUTION`, at least `BOSS_PART_MIN_BOUNCE` outward, and off
/// the rope (the rope would pull them straight back in). Returns the
/// outward normal if they touched this frame.
pub(crate) fn bounce_off_part(
    c: &mut Canvas, st: &Arc<Mutex<State>>, centre: (f32, f32), r: f32,
) -> Option<(f32, f32)> {
    let (px, py, vx, vy, hooked, holding) = {
        let s = st.lock().unwrap();
        (s.px, s.py, s.vx, s.vy, s.hooked, s.hitstop_ticks > 0)
    };
    // Hanging in a hit-stop: already resolved, and moving them now would
    // tug the freeze.
    if holding {
        return None;
    }
    let reach = r + PLAYER_R;
    let (dx, dy) = (px - centre.0, py - centre.1);
    let d2 = dx * dx + dy * dy;
    if d2 >= reach * reach {
        return None;
    }
    let d = d2.sqrt();
    // Dead centre (a pinned test, or a part spawning on the player): out
    // the way they were moving, or up.
    let n = if d > 0.001 {
        (dx / d, dy / d)
    } else {
        let sp = (vx * vx + vy * vy).sqrt();
        if sp > 0.001 { (-vx / sp, -vy / sp) } else { (0.0, -1.0) }
    };
    let out = (centre.0 + n.0 * (reach + 1.0), centre.1 + n.1 * (reach + 1.0));
    let vn = vx * n.0 + vy * n.1;
    let (mut nvx, mut nvy) = (vx, vy);
    if vn < 0.0 {
        nvx -= (1.0 + BOSS_PART_RESTITUTION) * vn * n.0;
        nvy -= (1.0 + BOSS_PART_RESTITUTION) * vn * n.1;
    }
    let away = nvx * n.0 + nvy * n.1;
    if away < BOSS_PART_MIN_BOUNCE {
        let add = BOSS_PART_MIN_BOUNCE - away;
        nvx += n.0 * add;
        nvy += n.1 * add;
    }
    {
        let mut s = st.lock().unwrap();
        s.px = out.0;
        s.py = out.1;
        s.vx = nvx;
        s.vy = nvy;
        if hooked {
            s.hooked = false;
            s.active_hook = String::new();
        }
    }
    if hooked {
        c.run(Action::Hide { target: Target::name("rope") });
    }
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (out.0 - PLAYER_R, out.1 - PLAYER_R);
        obj.momentum = (nvx, nvy);
    }
    // Let the rebound exceed the momentum cap for a moment, or it is
    // clamped away before it reads.
    c.set_var("boss_knockback_ticks", Value::I32(10));
    Some(n)
}

/// A hit landed on `part_id` at `at`: burst, white flash, camera kick, and a
/// hit-stop — the part shakes and the player hangs where they are, then
/// rebounds at no less than `HIT_REBOUND_SPEED` along `normal` (outward
/// from the part). Call AFTER `bounce_off_part`, so the hang is at the
/// surface and the rebound starts from the bounce.
pub(crate) fn land_hit(
    c: &mut Canvas, st: &Arc<Mutex<State>>, part_id: &str,
    at: (f32, f32), size: f32, rgb: (f32, f32, f32), normal: (f32, f32),
) {
    spawn_impact(c, st, at, size, rgb, false);
    crate::scenes::game::fx::flash_hit(c, part_id);
    if let Some(cam) = c.camera_mut() {
        cam.shake(HIT_SHAKE_INTENSITY, HIT_SHAKE_SECS);
    }
    let mut s = st.lock().unwrap();
    let (mut vx, mut vy) = (s.vx, s.vy);
    let away = vx * normal.0 + vy * normal.1;
    if away < HIT_REBOUND_SPEED {
        vx += normal.0 * (HIT_REBOUND_SPEED - away);
        vy += normal.1 * (HIT_REBOUND_SPEED - away);
    }
    s.hitstop_ticks = HITSTOP_TICKS;
    s.hitstop_pos = (s.px, s.py);
    s.hitstop_vel = (vx, vy);
    s.hitstop_target = part_id.to_owned();
}

/// Hold the player through a hit-stop, and release the rebound on its last
/// frame. Run before the fight each tick; true while holding.
pub(crate) fn tick_hitstop_hold(c: &mut Canvas, st: &Arc<Mutex<State>>) -> bool {
    let (ticks, pos, vel) = {
        let mut s = st.lock().unwrap();
        if s.hitstop_ticks == 0 {
            return false;
        }
        s.hitstop_ticks -= 1;
        (s.hitstop_ticks, s.hitstop_pos, s.hitstop_vel)
    };
    let (m, p) = if ticks > 0 { ((0.0, 0.0), pos) } else { (vel, pos) };
    {
        let mut s = st.lock().unwrap();
        s.px = p.0;
        s.py = p.1;
        s.vx = m.0;
        s.vy = m.1;
    }
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (p.0 - PLAYER_R, p.1 - PLAYER_R);
        obj.momentum = m;
    }
    if ticks == 0 {
        c.set_var("boss_knockback_ticks", Value::I32(12));
    }
    ticks > 0
}

/// Shake the struck part during a hit-stop. Run AFTER the fight has placed
/// its parts this tick, or the placement erases the shake.
pub(crate) fn tick_hitstop_shake(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (ticks, target) = {
        let s = st.lock().unwrap();
        (s.hitstop_ticks, s.hitstop_target.clone())
    };
    if ticks == 0 || target.is_empty() {
        return;
    }
    // Alternating, decaying: a knock, not a wobble.
    let k = ticks as f32 / HITSTOP_TICKS as f32;
    let sgn = if ticks % 2 == 0 { 1.0 } else { -1.0 };
    if let Some(obj) = c.get_game_object_mut(&target) {
        obj.position.0 += sgn * HITSTOP_JITTER * k;
        obj.position.1 -= sgn * HITSTOP_JITTER * 0.5 * k;
    }
}

#[cfg(test)]
mod solid_part_tests {
    /// The bounce maths, extracted: reflect the inward speed, keep a minimum
    /// outward speed.
    fn bounce(v: (f32, f32), n: (f32, f32), e: f32, min: f32) -> (f32, f32) {
        let vn = v.0 * n.0 + v.1 * n.1;
        let (mut x, mut y) = v;
        if vn < 0.0 {
            x -= (1.0 + e) * vn * n.0;
            y -= (1.0 + e) * vn * n.1;
        }
        let away = x * n.0 + y * n.1;
        if away < min {
            x += n.0 * (min - away);
            y += n.1 * (min - away);
        }
        (x, y)
    }

    #[test]
    fn a_head_on_touch_rebounds_and_a_graze_still_leaves() {
        // Head-on at 40 into a surface facing +x: back out at 22 (0.55).
        let (x, _) = bounce((-40.0, 0.0), (1.0, 0.0), 0.55, 16.0);
        assert!((x - 22.0).abs() < 1e-3, "{x}");
        // Sliding along the surface: tangent kept, pushed off at the minimum.
        let (x, y) = bounce((0.0, 30.0), (1.0, 0.0), 0.55, 16.0);
        assert!((x - 16.0).abs() < 1e-3 && (y - 30.0).abs() < 1e-3, "({x}, {y})");
    }
}
