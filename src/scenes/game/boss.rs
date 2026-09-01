// ── boss.rs — Boss fight logic ────────────────────────────────────────────────
// All logic gated behind boss_active — zero impact on free-roam.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use crate::images::circle_cached;
use super::bootstrap::hook_asteroid_anim_for_spawn;
use super::helpers::center_warp_on_player;
use crate::scenes::game::space_zone::wormhole2_template;

pub fn tick_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Published every frame: several systems (distance tracking, the headless
    // harness, the HUD) need to know an arena is active, and deriving it from
    // boss HP is wrong during entry and victory stasis.
    {
        let active = st.lock().unwrap().boss_active;
        c.set_var("boss_active", active);
    }
    tick_boss_zone_entry(c, st);
    tick_boss_stasis(c, st);
    tick_warp_flash(c, st);

    // Multi-part bosses (Colossus, Serpent) run their own part-driven loop; the
    // single-body bosses dispatch by kind below.
    let is_multi = { let s = st.lock().unwrap(); !s.boss_parts.is_empty() };
    if is_multi {
        // The Serpent is a distinct multi-part fight (head-HP win, tetherable
        // chain); the Colossus uses the shared part loop.
        let kind = { let s = st.lock().unwrap(); s.boss_kind };
        if kind == crate::constants::BossKind::Serpent {
            tick_serpent(c, st);
        } else {
            tick_multi_part_boss(c, st);
        }
    } else {
        let kind = { let s = st.lock().unwrap(); s.boss_kind };
        match kind {
            crate::constants::BossKind::FlareTitan => tick_flare_titan(c, st),
            crate::constants::BossKind::GravityWeaver => tick_gravity_weaver(c, st),
            crate::constants::BossKind::Magnetar => tick_magnetar(c, st),
            crate::constants::BossKind::Conductor => tick_conductor(c, st),
            _ => {
                tick_boss_appearance(c, st);
                tick_boss_movement(c, st);
                tick_boss_asteroid_drift(c, st);
                tick_boss_shooting(c, st);
                tick_boss_darkness(c, st);
                tick_boss_weakpoints(c, st);
                tick_boss_forcefield(c, st);
                tick_generators(c, st);
                tick_barrier(c, st);
                tick_desperation(c, st);
                tick_boss_bolts(c, st);
                tick_boss_bolt_player_collision(c, st);
                tick_boss_player_hits_boss(c, st);
            }
        }
    }

    tick_colossus_meteors(c, st);
    tick_core_vent(c, st);
    tick_clap_wave(c, st);
    tick_boss_hud(c, st);
    tick_boss_indicators(c, st);
    tick_boss_lights(c, st);
    tick_buff_node_elec(c, st);
    tick_arena_walls(c, st);
}

/// Reveal the arena's boundary walls (left/right) so the player can see the
/// play-area limits of the fight. No floor — you can fall to your doom below.
/// The walls extend from high above the highest possible launch up to the bottom
/// of the visible play area, so it never looks like you can hop over them.
fn tick_arena_walls(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let active = { let s = st.lock().unwrap(); s.boss_active && !s.dead };
    let (x1, x2) = arena_bounds(c);
    let top = -9000.0;
    let bottom = 600.0;
    let wall_h = bottom - top;
    let cy = (top + bottom) * 0.5;
    if active {
        if let Some(obj) = c.get_game_object_mut("arena_wall_l") {
            obj.size = (140.0, wall_h);
            obj.position = (x1 - 70.0, cy - wall_h * 0.5);
            obj.visible = true;
        }
        if let Some(obj) = c.get_game_object_mut("arena_wall_r") {
            obj.size = (140.0, wall_h);
            obj.position = (x2 - 70.0, cy - wall_h * 0.5);
            obj.visible = true;
        }
    } else {
        for name in ["arena_wall_l", "arena_wall_r"] {
            if let Some(obj) = c.get_game_object_mut(name) {
                obj.visible = false;
            }
        }
    }
}

/// Total remaining HP across all multi-part parts (drives the HUD + win check).
pub fn boss_total_hp(s: &State) -> i32 {
    s.boss_parts.iter().filter(|p| p.alive).map(|p| p.hp.max(0)).sum()
}

/// Boss-like layout offset for a part by index (Colossus), forming a distinct
/// body pose: the two hands hang at the sides, the torso is the centre, the head
/// sits above. Parts are near-still at idle (a slow, subtle, per-part breathing
/// bob) rather than orbiting, so the body reads as a creature with a lull — and
/// the parts never move in lockstep because each bob has its own phase. When
/// `bob` is false (the body is frozen during an attack) the part sits exactly at
/// its base pose.
fn boss_part_offset(i: u32, phase: f32, bob: bool) -> (f32, f32) {
    let base = match i {
        0 => (-1120.0,  280.0), // hand_l (hangs at the left)
        1 => ( 1120.0,  280.0), // hand_r (hangs at the right)
        2 => (   0.0,    0.0),  // torso (centre)
        _ => (   0.0, -1120.0), // head (above)
    };
    if !bob { return base; }
    // Slow, independent breathing: each part bobs on its own phase, so no two
    // parts move in unison, and the amplitude is small (near-still).
    let bob = (phase * 0.55 + i as f32 * 1.7).sin() * 24.0
            + (phase * 0.30 + i as f32 * 0.9).cos() * 14.0;
    (base.0 + bob * 0.5, base.1 + bob)
}

/// Serpent chain offset: the head leads the body and each segment trails behind
/// it on a travelling sine wave. The amplitude grows toward the tail and the wave
/// phase lags per segment, so the whole body undulates end-to-end like a serpent
/// rather than sitting in a fixed diagonal. Segment 0 is nearest the head.
fn serpent_part_offset(i: f32, phase: f32) -> (f32, f32) {
    // Trail horizontally behind the head with a slight downward cascade.
    let x = -(i + 1.0) * 220.0;
    let base_y = i * 70.0 - 240.0;
    // Wave travels down the chain: tail amplitude is larger and lags further.
    let amp = 70.0 + i * 18.0;
    let wave = (phase - i * 0.9).sin();
    // A little coiling on x too, but mostly a lateral (y) undulation.
    let y = base_y + wave * amp;
    let x_wave = (phase - i * 0.9).cos() * (10.0 + i * 4.0);
    (x + x_wave, y)
}

// ── Segmented-boss helpers (Colossus FSM) ─────────────────────────────────────

fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

fn lerp2(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (lerp(a.0, b.0, t), lerp(a.1, b.1, t))
}

/// Move `from` toward `to` by up to `cap` px per tick for `ticks` ticks, clamped
/// so the part never travels faster than `cap` (the player's momentum cap) and
/// never overshoots the destination. This is what keeps the Colossus's attack
/// lunges fair — the boss moves at the same speed ceiling the player does.
fn capped_toward(from: (f32, f32), to: (f32, f32), ticks: u32, cap: f32) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 0.001 { return to; }
    let traveled = (cap * ticks as f32).min(dist);
    (from.0 + dx / dist * traveled, from.1 + dy / dist * traveled)
}

/// Distance from `p` to the line segment `a`→`b`. Used so the head's gaze beam
/// can hit the player if they stand anywhere along the telegraphed path.
fn point_segment_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let abx = b.0 - a.0;
    let aby = b.1 - a.1;
    let len2 = abx * abx + aby * aby;
    if len2 < 0.0001 { return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt(); }
    let t = (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len2).clamp(0.0, 1.0);
    let cx = a.0 + abx * t;
    let cy = a.1 + aby * t;
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

// ── Beam geometry ────────────────────────────────────────────────────────────

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
    let n = COLOSSUS_BEAM_SEGMENTS;
    (0..=n)
        .map(|i| beam_point(start, end, curve, t_max * i as f32 / n as f32))
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
fn beam_shot_len() -> u32 {
    COLOSSUS_BEAM_TICKS + COLOSSUS_BEAM_GAP_TICKS
}

/// Lay a pool of rectangles along `pts` so a curved beam reads as one
/// continuous band.
///
/// `prefix` names a pool (`colossus_beam_tel_` / `colossus_beam_core_`) of
/// `COLOSSUS_BEAM_SEGMENTS` objects. `rotation_adjusted_offset` keeps a rotated
/// object's rendered centre at `position + size/2`, so positioning each segment
/// by its own midpoint is enough.
fn draw_beam_strip(c: &mut Canvas, prefix: &str, pts: &[(f32, f32)], thickness: f32) {
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
fn hide_beam_strip(c: &mut Canvas, prefix: &str) {
    for i in 0..COLOSSUS_BEAM_SEGMENTS {
        if let Some(obj) = c.get_game_object_mut(&format!("{prefix}_{i}")) {
            obj.visible = false;
        }
    }
}

/// Clamp `to` so it is at most `max` px from `from` — the leash that keeps a
/// part loosely tethered to its home orbit even while attacking.
fn leash_clamp(from: (f32, f32), to: (f32, f32), max: f32) -> (f32, f32) {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let d = (dx * dx + dy * dy).sqrt();
    if d <= max || d < 0.001 { return to; }
    let f = max / d;
    (from.0 + dx * f, from.1 + dy * f)
}

/// Ticks (at the player's momentum cap) it takes a part to lunge from `a` to
/// `b`, so the FSM knows when it has physically arrived at its target.
fn colossus_arrival(a: (f32, f32), b: (f32, f32)) -> u32 {
    let dist = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    (dist / MOMENTUM_CAP).ceil() as u32
}

/// Danger-zone radius for a Colossus part, matching the sized telegraph disc.
fn colossus_zone_r(id: &str) -> f32 {
    match id {
        "hand_l" | "hand_r" => COLOSSUS_HAND_ZONE_R,
        "torso"             => COLOSSUS_TORSO_ZONE_R,
        _                   => COLOSSUS_HEAD_ZONE_R,
    }
}

/// Idle length per part: a base plus a small per-part jitter so parts never
/// attack in lockstep (the pattern director additionally spaces them out).
pub(crate) fn colossus_idle_len(i: usize) -> u32 {
    // The head is timed against its own vulnerability window rather than the
    // shared idle: its next gravity well opens ~1 s after the window closes, so
    // the counter-attack ends cleanly instead of the next attack starting on
    // top of it. Index 3 is the head (see `boss_part_offset`).
    if i == 3 {
        return COLOSSUS_HEAD_VULN_AFTER + COLOSSUS_HEAD_REARM_GAP;
    }
    COLOSSUS_IDLE_TICKS + (i as u32 % 2) * 26 + (i as u32 * 7) % 17
}

// ── The torso's meteor storm ─────────────────────────────────────────────────

/// Build the launch schedule for one storm.
///
/// Two properties do the work, and both were missing from the burst this
/// replaces:
///
///  * SEQUENTIAL. Meteors are 0.5-1.0 s apart, so the storm is a series of
///    dodges the player moves through rather than one instant that they either
///    happened to be clear of or did not. Three simultaneous rocks is a coin
///    flip; five spaced rocks is a skill.
///
///  * ALTERNATING SIDES. The side flips every meteor, so consecutive rocks come
///    from opposite halves of the sky and the player is pushed back and forth
///    instead of settling into one safe corner. Picking each angle at random
///    independently would cluster them — three from the left in a row is a
///    likely draw, and it reads as the boss doing the same thing three times.
///
/// Angles stay in `COLOSSUS_METEOR_ANGLE_MIN..MAX` (above and to the sides),
/// never from below.
pub(crate) fn meteor_storm_schedule(seed: &mut u64) -> Vec<(u32, f32)> {
    let mut out = Vec::with_capacity(COLOSSUS_METEOR_COUNT as usize);
    let mid = (COLOSSUS_METEOR_ANGLE_MIN + COLOSSUS_METEOR_ANGLE_MAX) * 0.5;
    // A short lead-in so the storm does not fire on the same frame the pose
    // lands — the player needs to see the torso commit before the first rock.
    let mut delay = COLOSSUS_METEOR_GAP_MIN;
    for n in 0..COLOSSUS_METEOR_COUNT {
        // Alternate halves, then jitter inside the half.
        let (lo, hi) = if n % 2 == 0 {
            (mid, COLOSSUS_METEOR_ANGLE_MAX)
        } else {
            (COLOSSUS_METEOR_ANGLE_MIN, mid)
        };
        let angle = lcg_range(seed, lo, hi);
        out.push((delay, angle));
        let gap = lcg_range(
            seed,
            COLOSSUS_METEOR_GAP_MIN as f32,
            COLOSSUS_METEOR_GAP_MAX as f32,
        );
        delay += gap
            .round()
            .clamp(COLOSSUS_METEOR_GAP_MIN as f32, COLOSSUS_METEOR_GAP_MAX as f32)
            as u32;
    }
    out
}

/// Launch any meteor whose delay has elapsed.
///
/// Runs unconditionally rather than only while the torso is attacking: a storm
/// already committed should finish even if the torso is destroyed mid-way. The
/// alternative — cancelling the queue on death — makes a kill silently delete
/// hazards already telegraphed on screen, which reads as the warnings lying.
fn tick_colossus_meteors(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let due: Vec<f32> = {
        let mut s = st.lock().unwrap();
        if s.boss_meteor_queue.is_empty() { return; }
        if s.dead || !s.boss_active {
            s.boss_meteor_queue.clear();
            return;
        }
        let mut due = Vec::new();
        for entry in s.boss_meteor_queue.iter_mut() {
            entry.0 = entry.0.saturating_sub(1);
        }
        s.boss_meteor_queue.retain(|(ticks, angle)| {
            if *ticks == 0 { due.push(*angle); false } else { true }
        });
        due
    };
    for angle in due {
        super::spawning::spawn_comet_from_angle(c, st, angle);
    }
}

// ── The torso's core vent ────────────────────────────────────────────────────

/// Draw and resolve the rotating plasma spokes.
///
/// Kept out of the shared part loop: the vent is not a lunge with a landing
/// circle, it is a rotating field around a stationary part, and threading that
/// through the lunge machinery would have meant a special case in every branch
/// of it. It reads the torso's FSM state and owns everything else.
///
/// The spokes damage on CONTACT with a cooldown rather than once per attack: a
/// rotating beam you are standing in would otherwise take every heart in the
/// second it takes to get out.
fn tick_core_vent(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (venting, ticks, buffed, px, py) = {
        let mut s = st.lock().unwrap();
        if s.boss_vent_hit_cooldown > 0 { s.boss_vent_hit_cooldown -= 1; }
        let venting = s.boss_active
            && !s.dead
            && !s.boss_stasis_active
            && torso_attack_for(s.boss_torso_attack) == TorsoAttack::CoreVent
            && s.boss_parts.iter().any(|p| {
                p.id == "torso" && p.alive && !p.shielded && p.state == PartState::Attack
            });
        let ticks = s.boss_parts.iter()
            .find(|p| p.id == "torso")
            .map(|p| p.state_ticks)
            .unwrap_or(0);
        (venting, ticks, s.player_buff > 0, s.px, s.py)
    };

    if !venting {
        for i in 0..COLOSSUS_VENT_SPOKES {
            if let Some(obj) = c.get_game_object_mut(&format!("colossus_vent_{i}")) {
                obj.visible = false;
            }
        }
        return;
    }

    // Where the torso is. The vent radiates from the chest, so the spokes are
    // anchored to the part rather than to the body anchor.
    let Some((tx, ty)) = c.get_game_object("colossus_part_2").map(|o| {
        (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5)
    }) else { return; };

    let spin = ticks as f32 * COLOSSUS_VENT_SPIN;
    let step = 360.0 / COLOSSUS_VENT_SPOKES as f32;
    let half = COLOSSUS_VENT_LENGTH * 0.5;
    let mut hit = false;

    for i in 0..COLOSSUS_VENT_SPOKES {
        let deg = spin + step * i as f32;
        let rad = deg.to_radians();
        let (dx, dy) = (rad.cos(), rad.sin());
        let tip = (tx + dx * COLOSSUS_VENT_LENGTH, ty + dy * COLOSSUS_VENT_LENGTH);
        let mid = (tx + dx * half, ty + dy * half);

        if let Some(obj) = c.get_game_object_mut(&format!("colossus_vent_{i}")) {
            obj.size = (COLOSSUS_VENT_LENGTH, COLOSSUS_VENT_THICKNESS);
            obj.rotation = deg;
            obj.position = (mid.0 - COLOSSUS_VENT_LENGTH * 0.5, mid.1 - COLOSSUS_VENT_THICKNESS * 0.5);
            obj.visible = true;
        }

        // Half the drawn thickness plus the player, as the gaze beam uses — the
        // damaging area is what is on screen.
        if point_segment_dist((px, py), (tx, ty), tip)
            < COLOSSUS_VENT_THICKNESS * 0.5 + PLAYER_R
        {
            hit = true;
        }
    }

    if !hit { return; }
    let on_cooldown = { st.lock().unwrap().boss_vent_hit_cooldown > 0 };
    if on_cooldown { return; }
    { st.lock().unwrap().boss_vent_hit_cooldown = COLOSSUS_VENT_HIT_COOLDOWN; }

    // Thrown clear of the torso, so a hit also solves the problem of being
    // inside the spokes — being hit twice in a row by the same rotation would
    // be the attack punishing the player for its own knockback.
    let dx = px - tx;
    let dy = py - ty;
    let d = (dx * dx + dy * dy).sqrt().max(1.0);
    let push = (dx / d * 62.0, dy / d * 62.0);
    {
        let mut s = st.lock().unwrap();
        s.vx = push.0;
        s.vy = push.1;
        s.hooked = false;
        s.active_hook = String::new();
    }
    c.run(Action::Hide { target: Target::name("rope") });
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.momentum = push;
    }
    c.set_var("boss_knockback_ticks", Value::I32(18));

    if buffed {
        // The buff shields the heart and spends an absorption, as it does for
        // every other Colossus attack.
        let mut s = st.lock().unwrap();
        if s.player_buff > 0 {
            s.buff_absorbs = s.buff_absorbs.saturating_sub(1);
            if s.buff_absorbs == 0 {
                s.player_buff = 0;
                s.buff_timer = 0;
            }
        }
    } else {
        let dead = { st.lock().unwrap().dead };
        if !dead { super::hearts::lose_heart(c, st); }
    }
}

/// Expand and fade the clap's force-wave ring.
fn tick_clap_wave(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (ticks, at) = {
        let mut s = st.lock().unwrap();
        if s.boss_clap_wave == 0 {
            if let Some(obj) = c.get_game_object_mut("colossus_clap_wave") {
                obj.visible = false;
            }
            return;
        }
        s.boss_clap_wave -= 1;
        (s.boss_clap_wave, s.boss_clap_at)
    };
    let t = 1.0 - ticks as f32 / COLOSSUS_CLAP_WAVE_TICKS as f32;
    let r = COLOSSUS_CLAP_WAVE_R * t;
    if let Some(obj) = c.get_game_object_mut("colossus_clap_wave") {
        obj.size = (r * 2.0, r * 2.0);
        obj.position = (at.0 - r, at.1 - r);
        obj.visible = true;
    }
}

// ── Multi-part boss (Colossus / Serpent) ─────────────────────────────────────

/// Run a multi-part boss fight. Parts live in `s.boss_parts`; the `boss` object
/// is the visual/body anchor. The spec's core loop applies to every multi-part
/// boss:
///  * Parts are shielded until their dependency is destroyed (phase gating).
///  * A buffed player hit near an unshielded part's weakpoint damages that part.
///  * Unprotected body contact costs the player a heart (contact-rule inversion).
///  * When the last part dies the fight is won.
///
/// The distinct attack behaviours per boss (hands, pulses, beams, segments) are
/// layered on top of this loop; this is the shared skeleton they run on.
fn tick_multi_part_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }
    if s.dead { return; }
    // Entry/victory stasis: nothing to run yet.
    if s.boss_stasis_active { return; }

    // Appearance (once): reveal the boss body and set the banner name.
    if !s.boss_spawned {
        s.boss_spawned = true;
        let name = s.boss_kind.name();
        drop(s);
        let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
        let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
        // The `boss` object is an anchor (for the off-screen arrow); the part
        // circles are the visible multi-part body, so don't show the square.
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (spawn_x, spawn_y);
            obj.visible = false;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(200, 60, 220, 255), 1000.0 * sc,
                )));
            }
        }
        s = st.lock().unwrap();
    }

    // Move the boss on a slow lissajous so its parts visibly travel with it.
    // The whole body FREEZES while any part is mid-attack (so the only thing
    // that moves is the attacking part), and stays frozen after the torso's
    // slam until the summoned meteors have fired and cleared.
    let body_still = {
        let any_attacking = s.boss_parts.iter().any(|p| {
            p.alive && !p.shielded && p.state != PartState::Idle
        });
        any_attacking || s.boss_meteor_lock_ticks > 0
    };
    if !body_still {
        s.boss_phase += 0.010;
    }
    {
        let phase = s.boss_phase;
        let x_liss = (phase * 2.1).sin();
        let y_liss = (phase * 1.5 + 0.5).sin();
        let nx = arena_center_x(c) + x_liss * 2600.0;
        let ny = BOSS_Y_CENTER + y_liss * 1360.0;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (nx - BOSS_SIZE * 0.5, ny - BOSS_SIZE * 0.5);
        }
    }

    let phase = s.boss_phase;
    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };
    let mut any_alive = false;

    // ── Colossus: per-part FSM (Idle → Telegraph → Attack → Recover) ──────
    // Each part is an independent body with its own state machine and a leash:
    // it never strays far from its home orbit unless it is attacking. The
    // pattern director (`boss_pattern_cooldown`) stops both hands attacking at
    // once. A part's danger zone is drawn while it Telegraphs so the player can
    // see exactly where it will strike ~1s before it lands.
    #[derive(Clone)]
    struct PartFrame {
        id: &'static str,
        alive: bool,
        shielded: bool,
        weak_open: bool,
        offset: (f32, f32),
        state_ticks: u32,
        zone_visible: bool,
        zone_solid: bool,
        zone_pos: (f32, f32),
        zone_r: f32,
        path_visible: bool,
        path_start: (f32, f32),
        strike_unhook: bool,
        strike_kick: (f32, f32),
        strike_heart: bool,
        strike_consume_absorb: bool,
        strike_big_throw: bool,
        /// This part is the torso, mid meteor storm.
        storm: bool,
        /// This part is the torso, mid core vent.
        vent: bool,
        /// hand_l only: the pair completed a clap this tick.
        clap_wave: bool,
        /// Head only: how far the current beam has swept, 0..1. `None` when no
        /// beam is firing this frame (winding up, or in the gap between shots).
        beam_t: Option<f32>,
        /// Head only: lateral bow of the current beam.
        beam_curve: f32,
    }

    let frames: Vec<PartFrame> = {
        let mut s = st.lock().unwrap();
        if s.boss_pattern_cooldown > 0 { s.boss_pattern_cooldown -= 1; }
        if s.boss_part_invuln_ticks > 0 { s.boss_part_invuln_ticks -= 1; }
        if s.boss_meteor_lock_ticks > 0 { s.boss_meteor_lock_ticks -= 1; }
        let hooked = s.hooked;
        // While the torso is mid-slam, the head rides along with the body, so it
        // moves together with the torso instead of sitting still.
        let mut torso_disp = (0.0_f32, 0.0_f32);
        let mut frames = Vec::with_capacity(s.boss_parts.len());
        for i in 0..s.boss_parts.len() {
            let home = boss_part_offset(i as u32, phase, !body_still);
            s.boss_parts[i].home_offset = home;
            let pid = s.boss_parts[i].id;
            let zone_r = colossus_zone_r(pid);
            // The torso performs BOTH of its attacks from where it stands — a
            // meteor storm calls rocks down, a core vent radiates from the chest
            // — so it never displaces, and the head has nothing to ride. Kept as
            // an explicit zero rather than deleted: the head's offset still adds
            // it, and a silent removal would be a puzzle the next time the torso
            // gains a moving attack.
            if pid == "torso" && s.boss_parts[i].alive {
                torso_disp = (0.0, 0.0);
            }
            if !s.boss_parts[i].alive {
                frames.push(PartFrame {
                    id: pid, alive: false, shielded: s.boss_parts[i].shielded,
                    weak_open: false, offset: home, state_ticks: 0, zone_visible: false, zone_solid: false,
                    zone_pos: (bcx + home.0, bcy + home.1), zone_r,
                    path_visible: false, path_start: (bcx + home.0, bcy + home.1),
                    strike_unhook: false, strike_kick: (0.0, 0.0), strike_heart: false,
                    strike_consume_absorb: false, strike_big_throw: false, storm: false, vent: false, clap_wave: false, beam_t: None, beam_curve: 0.0,
                });
                continue;
            }
            any_alive = true;

            // Shielded parts sit idle and are visible, but never attack nor open.
            if s.boss_parts[i].shielded {
                s.boss_parts[i].weakpoint_open = false;
                s.boss_parts[i].state = PartState::Idle;
                s.boss_parts[i].state_ticks = 0;
                frames.push(PartFrame {
                    id: pid, alive: true, shielded: true, weak_open: false,
                    offset: home, state_ticks: 0, zone_visible: false, zone_solid: false,
                    zone_pos: (bcx + home.0, bcy + home.1), zone_r,
                    path_visible: false, path_start: (bcx + home.0, bcy + home.1),
                    strike_unhook: false, strike_kick: (0.0, 0.0), strike_heart: false,
                    strike_consume_absorb: false, strike_big_throw: false, storm: false, vent: false, clap_wave: false, beam_t: None, beam_curve: 0.0,
                });
                continue;
            }

            // ── FSM advance ──
            let cooldown_ok = s.boss_pattern_cooldown == 0;
            let vx = s.vx;
            let vy = s.vy;
            let mut began_attack = false;
            // Which attack the torso is on. Read BEFORE the FSM may bump it, so
            // the whole frame agrees; `next_torso_attack` applies the bump after.
            let mut torso_storm = torso_attack_for(s.boss_torso_attack) == TorsoAttack::MeteorStorm;
            // Both hands read ONE counter, so a clap is a decision the pair
            // makes rather than two hands happening to agree.
            //
            // And it takes TWO hands. Gated on both being in the fight rather
            // than on the counter alone, which means a hand destroyed mid-clap
            // drops the survivor straight back to lunge rules on the next frame
            // — including its vulnerability window. Left to the counter, a lone
            // hand would keep performing a clap it cannot complete AND keep the
            // clap's "only hittable once home" rule, so killing one hand would
            // have made the other one harder to kill.
            let both_hands_ready = s
                .boss_parts
                .iter()
                .filter(|q| (q.id == "hand_l" || q.id == "hand_r") && q.alive && !q.shielded)
                .count()
                == 2;
            let mut hand_clap =
                both_hands_ready && hand_attack_for(s.boss_hand_attack) == HandAttack::Clap;
            // Both hands lunge at the same speed cap from different distances,
            // so they do not arrive together. The clap is the moment the LATER
            // one lands — the nearer hand waits at the point and the second
            // slams into it, which is what the impact should look like.
            let clap_tick = {
                let arr = |id: &str| {
                    s.boss_parts
                        .iter()
                        .find(|q| q.id == id && q.alive && !q.shielded)
                        .map(|q| colossus_arrival(q.attack_start, q.target))
                };
                match (arr("hand_l"), arr("hand_r")) {
                    (Some(a), Some(b)) => a.max(b),
                    (Some(a), None) | (None, Some(a)) => a,
                    (None, None) => 0,
                }
            };
            let mut next_torso_attack = false;
            let mut next_hand_attack = false;
            let mut begin_clap = false;
            let mut start_storm = false;
            let mut roll_beam_shot = false;
            let mut roll_burst = false;
            {
                let p = &mut s.boss_parts[i];
                match p.state {
                    PartState::Idle => {
                        p.state_ticks += 1;
                        if cooldown_ok && p.state_ticks >= colossus_idle_len(i) {
                            p.state = PartState::Telegraph;
                            p.state_ticks = 0;
                            // Commit the torso to its next attack here, not at
                            // the strike: the wind-up, the pose, the hit and the
                            // vulnerability window all have to agree about which
                            // attack this is, and the telegraph is the first of
                            // them to be drawn.
                            if p.id == "torso" { next_torso_attack = true; }
                            // Same for the hands, except the decision is made
                            // once for the pair — hand_l speaks for both, and a
                            // clap drags hand_r into the same telegraph on the
                            // same tick.
                            if p.id == "hand_l" { next_hand_attack = true; }
                            // The hands aim at (a slight prediction ahead of) the
                            // player's position RIGHT NOW, then the path is locked
                            // — no homing. The lead makes it feel intelligent and,
                            // because the path is telegraphed, readable.
                            //
                            // The torso slams at the player (radial AoE), and the
                            // head aims its gaze beam at the player — both also
                            // lead slightly, and both paths are locked here.
                            let aim = (px + vx * COLOSSUS_ATTACK_LEAD, py + vy * COLOSSUS_ATTACK_LEAD);
                            let home_world = (bcx + home.0, bcy + home.1);
                            let world_target = if p.id == "head" {
                                // The head fires a RAY of fixed length through
                                // the aim point rather than stopping at it, so
                                // the beam sweeps past the player and keeps
                                // going. Stopping at the player made standing
                                // beyond the aim point unconditionally safe.
                                beam_end(home_world, aim)
                            } else {
                                leash_clamp(home_world, aim, COLOSSUS_LEASH)
                            };
                            p.target = (world_target.0 - bcx, world_target.1 - bcy);
                            p.path_start = home_world;
                            began_attack = true;
                        }
                    }
                    PartState::Telegraph => {
                        p.state_ticks += 1;
                        if p.state_ticks >= COLOSSUS_TELEGRAPH_TICKS {
                            p.state = PartState::Attack;
                            p.state_ticks = 0;
                            // Record where the part launches from (its wind-up
                            // position), so the lunge travels at the cap rather
                            // than teleporting to the target.
                            let home = p.home_offset;
                            let tgt = p.target;
                            let dx = home.0 - tgt.0;
                            let dy = home.1 - tgt.1;
                            let d = (dx * dx + dy * dy).sqrt().max(0.001);
                            p.attack_start = (
                                home.0 + dx / d * COLOSSUS_TELEGRAPH_PULL,
                                home.1 + dy / d * COLOSSUS_TELEGRAPH_PULL,
                            );
                            // Re-arm the head's gaze beam so a new sweep can hit.
                            p.beam_hit_done = false;
                            if p.id == "head" { roll_beam_shot = true; roll_burst = true; }
                            if p.id == "torso" && torso_storm { start_storm = true; }
                        }
                    }
                    PartState::Attack => {
                        p.state_ticks += 1;
                        // The head holds its beak while firing the gaze; the hands
                        // and torso hold at the lunge target after arriving (the
                        // torso for longer, so the meteors can clear).
                        // Each beam in the burst re-aims at wherever the player
                        // is NOW, so dodging the first one is the start of the
                        // attack rather than the end of it.
                        if p.id == "head"
                            && p.state_ticks > 0
                            && p.state_ticks % beam_shot_len() == 0
                            && p.state_ticks / beam_shot_len() < p.beam_shots
                        {
                            roll_beam_shot = true;
                        }
                        let duration = if p.id == "head" {
                            p.beam_shots * beam_shot_len()
                        } else if p.id == "torso" {
                            if torso_storm { COLOSSUS_STORM_TICKS } else { COLOSSUS_VENT_TICKS }
                        } else {
                            let arrival = colossus_arrival(p.attack_start, p.target);
                            // Only the hands reach this branch — the head and
                            // the torso both attack from where they stand.
                            arrival + COLOSSUS_HOLD_TICKS
                        };
                        if p.state_ticks >= duration {
                            p.state = PartState::Recover;
                            p.state_ticks = 0;
                        }
                    }
                    PartState::Recover => {
                        p.state_ticks += 1;
                        if p.state_ticks >= COLOSSUS_RECOVER_TICKS {
                            p.state = PartState::Idle;
                            p.state_ticks = 0;
                        }
                    }
                }
            }
            if roll_burst {
                let n = lcg_range(
                    &mut s.seed,
                    COLOSSUS_BEAM_SHOTS_MIN as f32,
                    COLOSSUS_BEAM_SHOTS_MAX as f32 + 0.999,
                ) as u32;
                s.boss_parts[i].beam_shots = n.clamp(COLOSSUS_BEAM_SHOTS_MIN, COLOSSUS_BEAM_SHOTS_MAX);
            }
            if roll_beam_shot {
                // Re-aim at the player's CURRENT position, and roll a fresh
                // curve, so no two beams in a burst are the same problem.
                let straight = lcg_range(&mut s.seed, 0.0, 1.0) > COLOSSUS_BEAM_CURVE_CHANCE;
                let curve = if straight {
                    0.0
                } else {
                    lcg_range(&mut s.seed, -COLOSSUS_BEAM_CURVE_MAX, COLOSSUS_BEAM_CURVE_MAX)
                };
                let start = (bcx + s.boss_parts[i].home_offset.0, bcy + s.boss_parts[i].home_offset.1);
                let end = beam_end(start, (px, py));
                let p = &mut s.boss_parts[i];
                p.beam_curve = curve;
                p.path_start = start;
                p.target = (end.0 - bcx, end.1 - bcy);
                p.beam_hit_done = false;
            }
            if next_torso_attack {
                s.boss_torso_attack = s.boss_torso_attack.wrapping_add(1);
                // Refresh the frame's copy too. Without this the FIRST frame of
                // a telegraph still describes the PREVIOUS attack — the glow,
                // the landing circle and the vulnerability all disagree with the
                // pose for one frame, which is exactly long enough to flicker.
                torso_storm = torso_attack_for(s.boss_torso_attack) == TorsoAttack::MeteorStorm;
            }
            if next_hand_attack {
                s.boss_hand_attack = s.boss_hand_attack.wrapping_add(1);
                hand_clap = both_hands_ready
                    && hand_attack_for(s.boss_hand_attack) == HandAttack::Clap;
                begin_clap = hand_clap;
            }
            if begin_clap {
                // A clap is the one moment the fight suspends its own
                // one-part-at-a-time rule: hand_r is dragged into the same
                // telegraph on the same tick, with the same target, so the two
                // hands wind up on opposite sides of it and arrive together.
                let (target, path_start) = {
                    let l = &s.boss_parts[i];
                    (l.target, l.path_start)
                };
                if let Some(j) = s.boss_parts.iter().position(|q| q.id == "hand_r" && q.alive && !q.shielded) {
                    let r = &mut s.boss_parts[j];
                    r.state = PartState::Telegraph;
                    r.state_ticks = 0;
                    r.target = target;
                    // hand_r starts its wind-up from its OWN home, so the pull
                    // back is mirrored rather than duplicated.
                    r.path_start = (bcx + r.home_offset.0, bcy + r.home_offset.1);
                    let _ = path_start;
                }
            }
            if began_attack {
                s.boss_pattern_cooldown = COLOSSUS_PATTERN_COOLDOWN;
            }
            if start_storm {
                s.boss_meteor_queue = meteor_storm_schedule(&mut s.seed);
                // Hold the body still for the whole storm, so the only things
                // moving on screen are the meteors the player has to read.
                s.boss_meteor_lock_ticks = COLOSSUS_STORM_TICKS + COMET_WARN_TOTAL;
            }

            let (off, weak_open, strike) = {
                let p = &s.boss_parts[i];
                let target = p.target;
                // Parts that attack from where they stand rather than lunging:
                // the head (gravity well + gaze beam from its perch) and the
                // torso while it is calling down a meteor storm. Everything
                // else travels to a telegraphed point.
                let head_stays = p.id == "head" || p.id == "torso";
                let off = match p.state {
                    PartState::Idle => home,
                    PartState::Telegraph => {
                        if head_stays { home } else {
                            // Pull back from the target to visibly wind up.
                            let dx = home.0 - target.0;
                            let dy = home.1 - target.1;
                            let d = (dx * dx + dy * dy).sqrt().max(0.001);
                            (home.0 + dx / d * COLOSSUS_TELEGRAPH_PULL,
                             home.1 + dy / d * COLOSSUS_TELEGRAPH_PULL)
                        }
                    }
                    // The attack lunge and the return both move at the player's
                    // momentum cap, so the boss never out-runs the player.
                    PartState::Attack if head_stays => home,
                    PartState::Attack => capped_toward(p.attack_start, target, p.state_ticks, MOMENTUM_CAP),
                    PartState::Recover if head_stays => home,
                    PartState::Recover => capped_toward(target, home, p.state_ticks, MOMENTUM_CAP),
                };
                // The head rides along with the torso's slam so the whole body
                // moves together when the torso lunges (and sits still otherwise).
                let off = if p.id == "head" {
                    (off.0 + torso_disp.0, off.1 + torso_disp.1)
                } else { off };
                // Weakpoint timing:
                //  * Hands/torso: opens 0.5s AFTER the part arrives at the
                //    telegraphed target (not on arrival), stays open through the
                //    hold, the slow retract, and 1s after getting home.
                //  * Head: NOT vulnerable while it fires the gaze / gravity well;
                //    it opens after (Recover) and for a long post-well window.
                let arrival = if p.id == "head" { 0 } else { colossus_arrival(p.attack_start, target) };
                let vuln_after = if p.id == "head" { COLOSSUS_HEAD_VULN_AFTER } else { COLOSSUS_VULN_AFTER_TICKS };
                // A meteor storm is a pure dodge phase: the torso is not
                // hittable during it, through its recovery, or in the lull
                // after. The window to damage the torso belongs to the SLAM, so
                // the two beats stay distinct — survive one, punish the other.
                let weak_open = if p.id == "torso" {
                    if torso_storm {
                        // A meteor storm is a pure dodge phase: not hittable
                        // during it, through its recovery, or in the lull after.
                        // The window belongs to the vent, so the two beats stay
                        // distinct — survive one, punish the other.
                        false
                    } else {
                        // The core vent is dangerous AND vulnerable at once.
                        // Opens half a second into the rotation (so the wind-up
                        // is not free) and stays open well past the end of the
                        // vent.
                        //
                        // The after-window is measured in TICKS SINCE THE VENT
                        // ENDED, not per FSM state, because it now outlasts the
                        // recovery and runs on into the idle. Expressed per
                        // state it would have been capped at the recovery's
                        // length without anything saying so — the window would
                        // have silently stopped growing when the constant went
                        // past 70.
                        match p.state {
                            PartState::Attack => p.state_ticks >= COLOSSUS_VENT_VULN_DELAY,
                            PartState::Recover => p.state_ticks < COLOSSUS_VENT_VULN_AFTER,
                            PartState::Idle => {
                                COLOSSUS_RECOVER_TICKS + p.state_ticks < COLOSSUS_VENT_VULN_AFTER
                            }
                            _ => false,
                        }
                    }
                } else if p.id == "hand_l" || p.id == "hand_r" {
                    if hand_clap {
                        // After a clap the hands are only hittable once they are
                        // HOME. Not while they are jammed together mid-arena:
                        // the reward for reading a clap should be a clean window
                        // at a known place, not a scramble to the middle of the
                        // arena with everything else still live.
                        p.state == PartState::Idle && p.state_ticks < COLOSSUS_CLAP_VULN_AFTER
                    } else {
                        match p.state {
                            PartState::Attack => p.state_ticks >= arrival + COLOSSUS_ATTACK_VULN_DELAY,
                            PartState::Recover => true,
                            PartState::Idle => p.state_ticks < vuln_after,
                            _ => false,
                        }
                    }
                } else {
                    match p.state {
                        PartState::Attack => {
                            p.id != "head"
                                && p.state_ticks >= arrival + COLOSSUS_ATTACK_VULN_DELAY
                        }
                        PartState::Recover => true,
                        PartState::Idle => p.state_ticks < vuln_after,
                        _ => false,
                    }
                };
                // Strike:
                //  * Hands + torso: the moment the lunge physically reaches the
                //    telegraphed zone (the hit and the motion line up).
                //  * Head: the gaze beam is a travelling sweep handled by the
                //    application loop, so the FSM does not strike for it here.
                let strike = if p.state == PartState::Attack {
                    // During a clap both hands land on the SAME point, so
                    // letting each resolve its own hit would cost two hearts
                    // for one attack. hand_l resolves it for the pair.
                    if head_stays || (hand_clap && p.id == "hand_r") {
                        false
                    } else {
                        let dist = ((target.0 - p.attack_start.0).powi(2)
                                  + (target.1 - p.attack_start.1).powi(2)).sqrt();
                        let now = MOMENTUM_CAP * p.state_ticks as f32 >= dist;
                        let prev = MOMENTUM_CAP * p.state_ticks.saturating_sub(1) as f32 >= dist;
                        // `now && (state_ticks == 1 || !prev)` catches the arrival
                        // even when the target is right next to the launch point
                        // (dist ~ 0, where `!prev` is false from tick 1).
                        now && (p.state_ticks == 1 || !prev)
                    }
                } else { false };
                (off, weak_open, strike)
            };

            {
                let p = &mut s.boss_parts[i];
                p.weakpoint_open = weak_open;
            }

            let zone_pos = (bcx + s.boss_parts[i].target.0, bcy + s.boss_parts[i].target.1);
            // "Storm, and currently performing it". `torso_storm` alone is
            // also true through the lull AFTER a storm (the counter is only
            // bumped on the next telegraph), which is what keeps the weakpoint
            // shut in that lull — but it must not keep the summoning glow lit.
            let performing = matches!(s.boss_parts[i].state, PartState::Telegraph | PartState::Attack);
            let storm_frame = pid == "torso" && torso_storm && performing;
            let vent_frame = pid == "torso" && !torso_storm && performing;
            // Neither torso attack travels, so neither gets a landing circle or
            // a trajectory strip — both would promise an impact that never comes.
            let torso_frame = storm_frame || vent_frame;
            let (zone_visible, zone_solid) = {
                let p = &s.boss_parts[i];
                // A meteor storm has no landing circle: nothing lunges, so a
                // disc on the ground would promise an impact that never comes.
                // Its telegraph is the torso's own summoning glow plus each
                // meteor's two-second warning marker.
                if torso_frame {
                    (false, false)
                } else if p.id == "head" {
                    // The well stays open for the whole attack; the beam is
                    // only "solid" while a shot is actually sweeping, so the
                    // gaps between beams show the charge orb recharging.
                    let firing = p.state == PartState::Attack
                        && (p.state_ticks % beam_shot_len()) < COLOSSUS_BEAM_TICKS;
                    (p.state == PartState::Telegraph || p.state == PartState::Attack, firing)
                } else {
                    (p.state == PartState::Telegraph || p.state == PartState::Attack,
                     p.state == PartState::Attack)
                }
            };
            let (beam_t, beam_curve) = {
                let p = &s.boss_parts[i];
                if p.id == "head" && p.state == PartState::Attack {
                    let shot_t = p.state_ticks % beam_shot_len();
                    if shot_t < COLOSSUS_BEAM_TICKS {
                        (Some(shot_t as f32 / COLOSSUS_BEAM_TICKS as f32), p.beam_curve)
                    } else {
                        (None, p.beam_curve)
                    }
                } else {
                    (None, p.beam_curve)
                }
            };
            // The path telegraph (trajectory) is shown while the part winds up,
            // from where it started the telegraph to where it will strike.
            let (path_visible, path_start) = {
                let p = &s.boss_parts[i];
                // The head's beam stays visible through the whole wind-up AND
                // while it fires (so you can read the full path). Hands + torso
                // only show the path during the wind-up.
                (!torso_frame
                 && (p.state == PartState::Telegraph
                     || (p.id == "head" && p.state == PartState::Attack)),
                 p.path_start)
            };
            let mut strike_unhook = false;
            let mut strike_kick = (0.0f32, 0.0f32);
            let mut strike_heart = false;
            let mut strike_consume_absorb = false;
            let mut strike_big_throw = false;
            if strike {
                // Hit detection:
                //  * Hands/torso: the player is in the radial danger zone.
                //  * Head: the player is on the gaze-beam line (path_start → target).
                let hit = if pid == "head" {
                    let (ax, ay) = (s.boss_parts[i].path_start.0, s.boss_parts[i].path_start.1);
                    point_segment_dist((px, py), (ax, ay), zone_pos) < COLOSSUS_PATH_THICKNESS + PLAYER_R
                } else {
                    (px - zone_pos.0).powi(2) + (py - zone_pos.1).powi(2)
                        < (PLAYER_R + zone_r).powi(2)
                };
                if hit {
                    let d = ((px - zone_pos.0).powi(2) + (py - zone_pos.1).powi(2)).sqrt().max(1.0);
                    // A direct hit throws the player HARD (much further than a
                    // regular body-contact push), so being caught in an attack
                    // is dangerous even when the buff protects your hearts.
                    let power = 78.0;
                    strike_kick = ((px - zone_pos.0) / d * power, (py - zone_pos.1) / d * power);
                    strike_big_throw = true;
                    if buffed {
                        // The buff shields the heart, but you still get flung and
                        // spend one absorption.
                        strike_unhook = hooked;
                        strike_consume_absorb = true;
                    } else {
                        strike_heart = true;
                    }
                }
                // Meteors are the torso's OTHER attack now, queued when the
                // storm begins — not a rider on every slam. See
                // `queue_meteor_storm`.
            }

            frames.push(PartFrame {
                id: pid, alive: true, shielded: false, weak_open, offset: off,
                state_ticks: s.boss_parts[i].state_ticks, zone_visible, zone_solid, zone_pos, zone_r,
                path_visible, path_start,
                strike_unhook, strike_kick, strike_heart, strike_consume_absorb,
                strike_big_throw, storm: storm_frame, vent: vent_frame,
                clap_wave: hand_clap
                    && pid == "hand_l"
                    && s.boss_parts[i].state == PartState::Attack
                    && s.boss_parts[i].state_ticks == clap_tick,
                beam_t, beam_curve,
            });
        }
        frames
    };

    // ── Apply per-part visuals + resolve strikes ──
    for (idx, f) in frames.iter().enumerate() {
        let part_size = colossus_part_size(idx as u32);

        // The head opens a large gravity well during its wind-up and beam, so it
        // drags the player toward the head — and FORCES an untether so the
        // player is dragged in freely instead of just swinging on their rope.
        if f.id == "head" {
            let gx = bcx + f.offset.0;
            let gy = bcy + f.offset.1;
            if f.alive && f.zone_visible {
                // The pull is STRONGEST far from the head (it drags you in from
                // across the arena) and weakens toward the core, so you aren't
                // flung once you're close — you just get dragged in. It is
                // applied to the STATE velocity (not the object's momentum) so
                // the momentum write-back can't wipe it, and it's strong enough
                // at the outer edge to haul you in after you're untethered.
                let dx = px - gx;
                let dy = py - gy;
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                // Steeper ramp than linear: the pull stays strong across most of
                // the well and only falls off in a small region at the core, so
                // it reliably drags you in from the outer edge.
                let strength = COLOSSUS_GRAVITY_STRENGTH * (d / COLOSSUS_GRAVITY_RANGE).clamp(0.0, 1.0).powf(0.35);
                let mut unhooked = false;
                {
                    let mut s = st.lock().unwrap();
                    if strength > 0.01 {
                        s.vx -= dx / d * strength;
                        s.vy -= dy / d * strength;
                    }
                    // Force the untether only for the first ~0.5s of the well, so
                    // the player can re-tether and save themselves from the beam
                    // rather than being sucked to a guaranteed hit.
                    if !f.zone_solid && f.state_ticks < 30 {
                        if s.hooked {
                            s.hooked = false;
                            s.active_hook = String::new();
                            unhooked = true;
                        }
                    }
                }
                if unhooked {
                    c.run(Action::Hide { target: Target::name("rope") });
                }
                // While the well is active, flatten the player's own gravity so
                // the pull wins and they can't free-fall to their doom.
                if let Some(obj) = c.get_game_object_mut("player") {
                    let sign = if obj.gravity < 0.0 { -1.0 } else { 1.0 };
                    obj.gravity = GRAVITY * 0.02 * sign;
                }
                // Gravity well visual.
                if let Some(obj) = c.get_game_object_mut("colossus_well") {
                    obj.position = (gx - COLOSSUS_GRAVITY_RANGE, gy - COLOSSUS_GRAVITY_RANGE);
                    obj.visible = true;
                }
            } else {
                if let Some(obj) = c.get_game_object_mut("colossus_well") {
                    obj.visible = false;
                }
                // The well is over: restore the arena's (reduced) gravity so the
                // player plays normally again.
                if let Some(obj) = c.get_game_object_mut("player") {
                    let sign = if obj.gravity < 0.0 { -1.0 } else { 1.0 };
                    obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE * sign;
                }
            }

            // Charge-up orb: grows at the head while it winds up, so the gaze
            // clearly reads as "about to fire".
            if f.alive && f.zone_visible && !f.zone_solid {
                let charge = (f.state_ticks as f32 / COLOSSUS_TELEGRAPH_TICKS as f32).clamp(0.0, 1.0);
                let r = 70.0 + charge * 120.0;
                if let Some(obj) = c.get_game_object_mut("colossus_charge") {
                    obj.size = (r * 2.0, r * 2.0);
                    obj.position = (gx - r, gy - r);
                    obj.visible = true;
                }
            } else if let Some(obj) = c.get_game_object_mut("colossus_charge") {
                obj.visible = false;
            }

            // Bright beam core: sweeps from the head out along the path as it
            // fires. Drawn as a polyline so a curved beam is drawn by the same
            // code as a straight one.
            if f.alive && f.zone_solid {
                let t = f.beam_t.unwrap_or(0.0);
                let pts = beam_polyline(f.path_start, f.zone_pos, f.beam_curve, t.max(0.001));
                draw_beam_strip(c, "colossus_beam_core", &pts, COLOSSUS_BEAM_THICKNESS * 0.46);
                let front = *pts.last().unwrap_or(&f.path_start);

                // Little contact explosions: as the beam sweeps the path, pops
                // appear at the beam front and quickly grow a few sizes, so it
                // reads as the beam detonating along the telegraphed line.
                {
                    let mut updates: Vec<(String, f32, f32, f32, bool)> = Vec::new();
                    let mut new_pop: Option<(String, f32, f32, u32)> = None;
                    {
                        let mut s = st.lock().unwrap();
                        let mut i = 0;
                        while i < s.beam_explode_live.len() {
                            let (id, x, y, ttl) = s.beam_explode_live[i].clone();
                            let nttl = ttl.saturating_sub(1);
                            if nttl == 0 {
                                s.beam_explode_live.remove(i);
                                updates.push((id, x, y, 0.0, false));
                                continue;
                            }
                            s.beam_explode_live[i].3 = nttl;
                            let growth = (COLOSSUS_BEAM_EXPLODE_TTL as f32 - nttl as f32)
                                / COLOSSUS_BEAM_EXPLODE_TTL as f32;
                            let r = COLOSSUS_BEAM_EXPLODE_R0
                                + growth * (COLOSSUS_BEAM_EXPLODE_R1 - COLOSSUS_BEAM_EXPLODE_R0);
                            updates.push((id, x, y, r, true));
                            i += 1;
                        }
                        // Spawn a new pop at the beam front every few ticks.
                        if f.state_ticks % 3 == 0 {
                            let live_ids: Vec<&String> =
                                s.beam_explode_live.iter().map(|(id, _, _, _)| id).collect();
                            let free = (0..8)
                                .map(|i| format!("colossus_beam_explode_{i}"))
                                .find(|id| !live_ids.contains(&id));
                            if let Some(id) = free {
                                s.beam_explode_live.push((id.clone(), front.0, front.1, COLOSSUS_BEAM_EXPLODE_TTL));
                                new_pop = Some((id, front.0, front.1, COLOSSUS_BEAM_EXPLODE_TTL));
                            }
                        }
                    }
                    for (id, x, y, r, vis) in updates {
                        if let Some(obj) = c.get_game_object_mut(&id) {
                            obj.size = (r * 2.0, r * 2.0);
                            obj.position = (x - r, y - r);
                            obj.visible = vis;
                        }
                    }
                    if let Some((id, x, y, _ttl)) = new_pop {
                        if let Some(obj) = c.get_game_object_mut(&id) {
                            let r = COLOSSUS_BEAM_EXPLODE_R0;
                            obj.size = (r * 2.0, r * 2.0);
                            obj.position = (x - r, y - r);
                            obj.visible = true;
                        }
                    }
                }
            } else {
                hide_beam_strip(c, "colossus_beam_core");
            }
            // When the beam is not firing, also stop the explosion trail.
            if !(f.alive && f.zone_visible) {
                let mut s = st.lock().unwrap();
                for (id, _, _, _) in s.beam_explode_live.drain(..) {
                    if let Some(obj) = c.get_game_object_mut(&id) {
                        obj.visible = false;
                    }
                }
            }
        }

        // Buffed hit on an exposed (unshielded, weakpoint-open) part damages it.
        // The boss has a short invulnerability window after a part is destroyed,
        // so two parts can't be killed back-to-back within the same second.
        if f.alive && !f.shielded && f.weak_open && buffed {
            let sx = bcx + f.offset.0;
            let sy = bcy + f.offset.1;
            let hit_r = colossus_part_hit_r(idx as u32);
            if (px - sx).powi(2) + (py - sy).powi(2) < (PLAYER_R + hit_r).powi(2) {
                let mut s = st.lock().unwrap();
                if s.boss_part_invuln_ticks == 0 {
                    if let Some(p) = s.boss_parts.iter_mut().find(|p| p.id == f.id && p.alive) {
                        p.hp -= 1;
                        if p.hp <= 0 {
                            p.alive = false;
                            s.boss_part_invuln_ticks = COLOSSUS_PART_INVULN_TICKS;
                        }
                        s.buff_hit_flash = 20;
                    }
                }
            }
        }

        // Part body (composite silhouette, sized to the part).
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_part_{idx}")) {            if f.alive {
                let (sx, sy) = (bcx + f.offset.0, bcy + f.offset.1);
                let half = part_size * 0.5;
                obj.size = (part_size, part_size);
                obj.position = (sx - half, sy - half);
                obj.visible = true;
                if f.shielded {
                    obj.set_glow(GlowConfig { color: Color(120, 220, 255, 90), width: 22.0 });
                } else if f.weak_open {
                    // VULNERABLE: a bright, pulsing gold glow — the "hit me now"
                    // cue. Takes priority so the strike window is unmistakable.
                    let pulse = 170 + (((f.zone_pos.0 as i32 / 4) + (f.zone_pos.1 as i32 / 4)).rem_euclid(6) as u8) * 14;
                    obj.set_glow(GlowConfig { color: Color(255, 224, 70, pulse), width: 42.0 });
                } else if f.vent {
                    // Hot orange while the chest is open: the torso is
                    // dangerous here, but it is ALSO the only moment it can be
                    // hurt, so the cue has to say "come here" and "carefully"
                    // at once — which is why it is neither the storm's cold
                    // violet nor the plain strike red.
                    obj.set_glow(GlowConfig { color: Color(255, 170, 80, 220), width: 52.0 });
                } else if f.storm {
                    // Summoning glow: cold violet-white, deliberately NOT the
                    // red-orange of an incoming strike. The two torso attacks
                    // have to be distinguishable during the wind-up, because
                    // one is a beat to dodge and the other is the only beat
                    // where the torso can be hurt — reading it late costs the
                    // player the window entirely.
                    obj.set_glow(GlowConfig { color: Color(190, 150, 255, 200), width: 46.0 });
                } else if f.zone_visible {
                    // Wind-up glow: pulsing red-orange while it commits to the strike.
                    let wide = if f.strike_unhook || f.strike_heart || f.strike_kick.0 != 0.0 || f.strike_kick.1 != 0.0 { 190 } else { 110 };
                    obj.set_glow(GlowConfig { color: Color(255, 80, 30, wide), width: 30.0 });
                } else {
                    obj.clear_glow();
                }
            } else {
                obj.visible = false;
            }
        }

        // Attack-path telegraph: a translucent red strip from where the part
        // started winding up to where it will strike. For the head's gaze beam
        // this strip grows from the head toward a travelling front during the
        // attack, so the player sees the sweep coming and can be off the line.
        if f.id == "head" {
            // The head's path is a curve, so it gets the polyline pool rather
            // than the single rotated strip the lunging parts use. Shown for
            // the whole wind-up and while the beam sweeps, so the player can
            // read the full arc before it is dangerous.
            if f.alive && f.path_visible {
                let pts = beam_polyline(f.path_start, f.zone_pos, f.beam_curve, 1.0);
                draw_beam_strip(c, "colossus_beam_tel", &pts, COLOSSUS_BEAM_THICKNESS);
            } else {
                hide_beam_strip(c, "colossus_beam_tel");
            }
        }
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_path_{idx}")) {
            if f.alive && f.path_visible && f.id != "head" {
                let (ax, ay) = f.path_start;
                let (bx, by) = f.zone_pos;
                let dx = bx - ax;
                let dy = by - ay;
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let deg = dy.atan2(dx).to_degrees();
                let th = COLOSSUS_PATH_THICKNESS;
                let mid = ((ax + bx) * 0.5, (ay + by) * 0.5);
                // `rotation_adjusted_offset` keeps the rendered centre locked at
                // `position + size/2`, so positioning by the strip's centre is
                // enough — the engine handles the rotated AABB compensation.
                obj.size = (len, th);
                obj.rotation = deg;
                obj.position = (mid.0 - len * 0.5, mid.1 - th * 0.5);
                obj.visible = true;
            } else {
                obj.visible = false;
            }
        }

        // Traveling gaze beam hit: the beam front sweeps the path and, when it
        // reaches the player, costs a heart ALWAYS (the buff does not shield the
        // gaze) and throws them hard.
        if f.id == "head" && f.alive && f.zone_solid {
            let t = f.beam_t.unwrap_or(0.0);
            let pts = beam_polyline(f.path_start, f.zone_pos, f.beam_curve, t.max(0.001));
            let front = *pts.last().unwrap_or(&f.path_start);
            let hit_done = {
                let s = st.lock().unwrap();
                s.boss_parts.iter().find(|p| p.id == "head").map(|p| p.beam_hit_done).unwrap_or(true)
            };
            // Against the swept polyline, at HALF the drawn thickness plus the
            // player's radius — the damaging area is what is on screen. The old
            // test used the full path thickness as a radius, so the beam hurt
            // twice as far as it looked.
            if !hit_done && beam_dist((px, py), &pts) < beam_hit_radius() {
                {
                    let mut s = st.lock().unwrap();
                    if let Some(p) = s.boss_parts.iter_mut().find(|p| p.id == "head" && p.alive) {
                        p.beam_hit_done = true;
                    }
                }
                // The gaze always costs a heart.
                let dead = { let s = st.lock().unwrap(); s.dead };
                if !dead { super::hearts::lose_heart(c, st); }
                // And throws the player away from the beam front.
                let dx = px - front.0;
                let dy = py - front.1;
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                let power = 78.0;
                let push = (dx / d * power, dy / d * power);
                let mut s = st.lock().unwrap();
                s.vx = push.0;
                s.vy = push.1;
                drop(s);
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.momentum = push;
                }
                c.set_var("boss_knockback_ticks", Value::I32(20));
            }
        }

        // Vulnerability ring: pulsing gold outline around a hittable part.
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_vuln_{idx}")) {
            if f.alive && !f.shielded && f.weak_open {
                let r = part_size * 0.55;
                let (sx, sy) = (bcx + f.offset.0, bcy + f.offset.1);
                obj.position = (sx - r, sy - r);
                // Pulse the ring's visibility/scale so it throbs.
                let on = ((f.zone_pos.0 as i32 / 3) + (f.zone_pos.1 as i32 / 3)).rem_euclid(5) < 3;
                obj.visible = on;
            } else {
                obj.visible = false;
            }
        }

        // Danger-zone telegraph disc (only while a part telegraphs / strikes).
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_zone_{idx}")) {
            if f.id == "head" {
                // Head: a small targeting reticle that runs the course of the
                // path line just ahead of the beam. During the telegraph the
                // line IS the telegraph, so the reticle only appears as it
                // fires (unlike the hands/torso landing circles).
                if f.alive && f.zone_solid {
                    let t = f.beam_t.unwrap_or(0.0);
                    let front = beam_point(f.path_start, f.zone_pos, f.beam_curve, t);
                    let r = COLOSSUS_BEAM_THICKNESS * 0.34;
                    obj.size = (r * 2.0, r * 2.0);
                    obj.position = (front.0 - r, front.1 - r);
                    obj.visible = true;
                } else {
                    obj.visible = false;
                }
            } else if f.alive && f.zone_visible {
                obj.position = (f.zone_pos.0 - f.zone_r, f.zone_pos.1 - f.zone_r);
                // Zone flickers during the telegraph, then goes solid for the strike.
                let on = f.zone_solid
                    || ((f.zone_pos.0 as i32 / 5) + (f.zone_pos.1 as i32 / 5)).rem_euclid(6) < 4;
                obj.visible = on;
            } else {
                obj.visible = false;
            }
        }

        // Strike effects (unhook / kick / heart) — resolved once per attack.
        if f.strike_unhook {
            let mut s = st.lock().unwrap();
            s.hooked = false;
            s.active_hook = String::new();
            drop(s);
            c.run(Action::Hide { target: Target::name("rope") });
        }
        if f.strike_kick.0 != 0.0 || f.strike_kick.1 != 0.0 {
            let mut s = st.lock().unwrap();
            s.vx = f.strike_kick.0;
            s.vy = f.strike_kick.1;
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum = f.strike_kick;
            }
            // A direct hit throws the player hard: briefly bypass the momentum
            // cap so the throw actually flies rather than being clamped away.
            if f.strike_big_throw {
                c.set_var("boss_knockback_ticks", Value::I32(20));
            }
        }
        if f.strike_heart {
            let dead = { let s = st.lock().unwrap(); s.dead };
            if !dead { super::hearts::lose_heart(c, st); }
        }
        if f.clap_wave {
            // The wave leaves the impact point whether or not the hands caught
            // anyone. Being outside the kill zone is not the same as being
            // unaffected — the throw IS the attack, and it is what turns a
            // dodged clap into a repositioning problem instead of a non-event.
            let (cx, cy) = f.zone_pos;
            let dx = px - cx;
            let dy = py - cy;
            let d = (dx * dx + dy * dy).sqrt().max(1.0);
            {
                let mut s = st.lock().unwrap();
                s.boss_clap_wave = COLOSSUS_CLAP_WAVE_TICKS;
                s.boss_clap_at = (cx, cy);
            }
            if d < COLOSSUS_CLAP_WAVE_R {
                // Falls off to nothing at the edge, so the wave has a readable
                // reach rather than a hard boundary you cannot see.
                let fall = 1.0 - (d / COLOSSUS_CLAP_WAVE_R).clamp(0.0, 1.0);
                let power = COLOSSUS_CLAP_WAVE_POWER * fall;
                let push = (dx / d * power, dy / d * power);
                {
                    let mut s = st.lock().unwrap();
                    s.vx = push.0;
                    s.vy = push.1;
                    s.hooked = false;
                    s.active_hook = String::new();
                }
                c.run(Action::Hide { target: Target::name("rope") });
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.momentum = push;
                }
                // Briefly bypass the momentum cap so the throw actually flies
                // rather than being clamped away on the next frame.
                c.set_var("boss_knockback_ticks", Value::I32(22));
            }
        }
        if f.strike_consume_absorb {
            // The buff ate the hit: spend one absorption. When it runs out the
            // buff ends, so the shield is a limited resource.
            let mut s = st.lock().unwrap();
            if s.player_buff > 0 {
                s.buff_absorbs = s.buff_absorbs.saturating_sub(1);
                if s.buff_absorbs == 0 {
                    s.player_buff = 0;
                    s.buff_timer = 0;
                }
            }
        }
    }

    // Hide the visuals of any destroyed/leashed parts.
    for idx in 0..frames.len() {
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_part_{idx}")) {
            if !frames[idx].alive {
                obj.visible = false;
            }
        }
    }

    // Phase gating: once a dependency is dead, the next part loses its shield.
    // (Colossus: torso unshields when both hands die; head unshields when the
    // torso dies. Serpent segments uncover as the one before dies.)
    {
        let mut s = st.lock().unwrap();
        for i in 0..s.boss_parts.len() {
            let prev_dead = i > 0 && s.boss_parts[..i].iter().all(|p| !p.alive);
            if prev_dead {
                let part = &mut s.boss_parts[i];
                if part.alive && part.shielded {
                    part.shielded = false;
                }
            }
        }
        s.boss_hp = boss_total_hp(&s);
    }

    // Simple shield dome glow while any part is still shielded (the full
    // BIT_ENERGY_DOME shader is layered on later).
    let any_shielded = { let s = st.lock().unwrap(); s.boss_parts.iter().any(|p| p.alive && p.shielded) };
    if let Some(obj) = c.get_game_object_mut("boss") {
        if any_shielded {
            obj.set_glow(GlowConfig { color: Color(120, 220, 255, 90), width: 22.0 });
        } else {
            obj.clear_glow();
        }
    }

    // Contact-rule inversion: touching a part you are NOT currently able to hit
    // (it's shielded or idle/winding up) costs one heart — but not your whole
    // life (the cooldown stops repeated contact from draining every heart in a
    // couple of frames). With the buff it costs no heart, just tears you off.
    // This is a light contact push; the attack STRIKE is the one that throws you
    // hard.
    {
        let mut s = st.lock().unwrap();
        if s.boss_contact_cooldown > 0 { s.boss_contact_cooldown -= 1; }
        let touching = if s.boss_contact_cooldown == 0 && !s.dead {
            frames.iter().enumerate().any(|(i, f)| {
                f.alive && !f.weak_open && {
                    let sx = bcx + f.offset.0;
                    let sy = bcy + f.offset.1;
                    let cr = colossus_part_hit_r(i as u32) + PLAYER_R;
                    (px - sx).powi(2) + (py - sy).powi(2) < cr * cr
                }
            })
        } else { false };
        if touching { s.boss_contact_cooldown = 45; }
        let dead = s.dead;
        drop(s);
        if touching && !dead {
            let d = ((px - bcx).powi(2) + (py - bcy).powi(2)).sqrt().max(1.0);
            let push = ((px - bcx) / d * 34.0, (py - bcy) / d * 34.0);
            let mut s = st.lock().unwrap();
            s.vx = push.0;
            s.vy = push.1;
            if !buffed {
                // No buff: contact costs a heart (see the cooldown — one, not all).
                drop(s);
                super::hearts::lose_heart(c, st);
            } else {
                // Buff shields the heart, but contact still tears you off.
                s.hooked = false;
                s.active_hook = String::new();
                drop(s);
                c.run(Action::Hide { target: Target::name("rope") });
            }
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum = push;
            }
        }
    }

    // Win when no parts are alive.
    if !any_alive {
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
        }
        finish_boss(c, st);
    }
}

// ── Flare Titan (reuses the solar-flare mechanic) ────────────────────────────

/// Mark a few of the arena tether nodes as shielded so they are shelter during
/// the Flare Titan's flares (timed-release, like the world's flare system).
fn ensure_arena_shelter_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let ids: Vec<String> = st.lock().unwrap().live_hooks.clone();
    for (i, id) in ids.iter().enumerate() {
        if i % 4 == 0 {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.tags.retain(|t| t != SHIELD_HOOK_TAG);
                obj.tags.push(SHIELD_HOOK_TAG.into());
                obj.set_glow(GlowConfig { color: Color(255, 200, 80, 255), width: 14.0 });
            }
        }
    }
}

/// The Flare Titan: a single-body boss whose fight is the flare loop. Telegraph
/// → flare (tether a shielded node or lose a heart; tethering grants Solar
/// Charge) → weakpoint window (core vents; only a buffed hit hurts it, for 4s)
/// → repeat. Touching the body costs a heart.
fn tick_flare_titan(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }
    if s.dead { return; }
    if s.boss_stasis_active { return; }

    // Appearance (once).
    if !s.boss_spawned {
        s.boss_spawned = true;
        let name = s.boss_kind.name();
        drop(s);
        let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
        let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (spawn_x, spawn_y);
            obj.visible = true;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(255, 170, 60, 255), 1000.0 * sc,
                )));
            }
        }
        ensure_arena_shelter_nodes(c, st);
        s = st.lock().unwrap();
    }

    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };

    // ── Flare cycle ──
    // idle → warning (flare_warn) → active (flare_active) → weakpoint window
    // (boss_flare_window_ticks).
    let mut unhook_flare = false;
    {
        let mut s = st.lock().unwrap();
        if s.boss_flare_window_ticks > 0 {
            s.boss_flare_window_ticks -= 1;
        } else if s.flare_active {
            s.flare_active_ticks = s.flare_active_ticks.saturating_sub(1);
            s.flare_damage_timer = s.flare_damage_timer.saturating_sub(1);
            let sheltered = super::solar::player_is_sheltered(c, &s);
            // Tethering a shielded node grants / refreshes Solar Charge.
            if sheltered {
                s.player_buff = 1;
                s.buff_timer = 600;
                s.buff_absorbs = 3;
            }
            if s.flare_damage_timer == 0 {
                s.flare_damage_timer = FLARE_TITAN_DAMAGE_INTERVAL;
                if !sheltered {
                    unhook_flare = s.hooked;
                }
            }
            if s.flare_active_ticks == 0 {
                s.flare_active = false;
                s.boss_flare_window_ticks = FLARE_TITAN_WINDOW_TICKS;
            }
        } else if s.flare_warn > 0 {
            s.flare_warn -= 1;
            c.set_var("flare_warning", s.flare_warn > 0);
            if s.flare_warn == 0 {
                s.flare_active = true;
                s.flare_active_ticks = FLARE_TITAN_ACTIVE_TICKS;
                s.flare_damage_timer = FLARE_TITAN_DAMAGE_INTERVAL;
                c.set_var("flare_active", true);
                c.set_var("flare_warning", false);
            }
        } else if s.flare_cooldown == 0 {
            s.flare_cooldown = FLARE_TITAN_INTERVAL;
            s.flare_warn = FLARE_TITAN_TELEGRAPH_TICKS;
        } else {
            s.flare_cooldown -= 1;
        }
    }

    if unhook_flare {
        let mut s = st.lock().unwrap();
        s.hooked = false;
        s.active_hook = String::new();
        drop(s);
        c.run(Action::Hide { target: Target::name("rope") });
        super::hearts::lose_heart(c, st);
    }

    // ── Weakpoint window: a buffed hit on the core damages the boss ──
    let window_open = { let s = st.lock().unwrap(); s.boss_flare_window_ticks > 0 };
    if window_open && buffed && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + 320.0).powi(2) {
        let mut s = st.lock().unwrap();
        s.boss_hp = (s.boss_hp - 4).max(0);
        s.buff_hit_flash = 20;
        let hp = s.boss_hp;
        drop(s);
        if hp <= 0 {
            if let Some(obj) = c.get_game_object_mut("boss") {
                obj.visible = false;
                obj.position = (-6000.0, -6000.0);
            }
            finish_boss(c, st);
        }
    }

    // ── Contact-rule inversion ──
    {
        let s = st.lock().unwrap();
        let touching = !s.dead
            && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + BOSS_SIZE * 0.5).powi(2);
        drop(s);
        if touching {
            super::hearts::lose_heart(c, st);
        }
    }
}

// ── Gravity Weaver (uses the space_rift gravity inversion) ───────────────────

/// The Gravity Weaver: a single-body boss that periodically INVERTS the world
/// (flips `gravity_dir`, so the arena's ceiling becomes the floor). Tether nodes
/// exist on both sides, so the player keeps swinging as the world turns over.
/// The boss's core opens for a short window right after each flip; a buffed hit
/// in that window damages it. Touching the body costs a heart.
fn tick_gravity_weaver(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }
    if s.dead { return; }
    if s.boss_stasis_active { return; }

    if !s.boss_spawned {
        s.boss_spawned = true;
        let name = s.boss_kind.name();
        drop(s);
        let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
        let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (spawn_x, spawn_y);
            obj.visible = true;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(120, 210, 255, 255), 1000.0 * sc,
                )));
            }
        }
        s = st.lock().unwrap();
    }

    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };

    // ── Flip cycle ──
    // weakpoint window open (after a flip) → countdown → flip + open window.
    let mut did_flip = false;
    {
        let mut s = st.lock().unwrap();
        if s.boss_flare_window_ticks > 0 {
            s.boss_flare_window_ticks -= 1;
        } else if s.boss_gravity_flip_ticks > 0 {
            s.boss_gravity_flip_ticks -= 1;
        } else {
            // Invert the world (like a space_rift).
            s.gravity_dir = -s.gravity_dir;
            s.boss_flare_window_ticks = GRAVITY_WEAVER_WINDOW_TICKS;
            s.boss_gravity_flip_ticks = GRAVITY_WEAVER_FLIP_INTERVAL;
            did_flip = true;
            // Keep the arena nodes / player oriented: flip the player's gravity.
            let gdir = s.gravity_dir;
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.gravity = GRAVITY * gdir;
            }
            drop(s);
            if let Some(cam) = c.camera_mut() {
                cam.flash_with(Color(120, 200, 255, 120), 0.35, FlashMode::Pulse, FlashEase::Sharp, 0.8, 0.0);
            }
            s = st.lock().unwrap();
        }
        let _ = did_flip;
    }

    // ── Weakpoint window: buffed hit on the core damages the boss ──
    let window_open = { let s = st.lock().unwrap(); s.boss_flare_window_ticks > 0 };
    if window_open && buffed && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + 320.0).powi(2) {
        let mut s = st.lock().unwrap();
        s.boss_hp = (s.boss_hp - 2).max(0);
        s.buff_hit_flash = 20;
        let hp = s.boss_hp;
        drop(s);
        if hp <= 0 {
            if let Some(obj) = c.get_game_object_mut("boss") {
                obj.visible = false;
                obj.position = (-6000.0, -6000.0);
            }
            finish_boss(c, st);
        }
    }

    // ── Contact-rule inversion ──
    {
        let s = st.lock().unwrap();
        let touching = !s.dead
            && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + BOSS_SIZE * 0.5).powi(2);
        drop(s);
        if touching {
            super::hearts::lose_heart(c, st);
        }
    }
}

// ── Magnetar (uses the gravity-well / magnet pull) ───────────────────────────

/// The Magnetar: a single-body boss that pulses a strong gravity attraction,
/// dragging the player's rope toward it. The player resists by grabbing a
/// shielded node or letting go and swinging against the pull. While over-charged
/// (venting) the core is the weakpoint; a buffed hit damages it. Touching the
/// body costs a heart.
fn tick_magnetar(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }
    if s.dead { return; }
    if s.boss_stasis_active { return; }

    if !s.boss_spawned {
        s.boss_spawned = true;
        let name = s.boss_kind.name();
        drop(s);
        let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
        let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (spawn_x, spawn_y);
            obj.visible = true;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(200, 120, 255, 255), 1000.0 * sc,
                )));
            }
        }
        s = st.lock().unwrap();
    }

    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };

    // ── Charge cycle ──
    // idle (flare_cooldown) → pull window (boss_pull_ticks) → weakpoint window.
    let mut pulling = false;
    {
        let mut s = st.lock().unwrap();
        if s.boss_flare_window_ticks > 0 {
            s.boss_flare_window_ticks -= 1;
        } else if s.boss_pull_ticks > 0 {
            s.boss_pull_ticks -= 1;
            pulling = true;
            if s.boss_pull_ticks == 0 {
                s.boss_flare_window_ticks = MAGNETAR_WINDOW_TICKS;
            }
        } else if s.flare_cooldown == 0 {
            s.flare_cooldown = MAGNETAR_PULL_INTERVAL;
            s.boss_pull_ticks = MAGNETAR_PULL_TICKS;
        } else {
            s.flare_cooldown -= 1;
        }
    }

    // The pull: drag the player toward the boss's core.
    if pulling {
        let dx = px - bcx;
        let dy = py - bcy;
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let strength = 0.9 * (1.0 - (d / 2600.0).clamp(0.0, 1.0));
        if strength > 0.01 {
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum.0 -= dx / d * strength;
                obj.momentum.1 -= dy / d * strength;
            }
        }
    }

    // ── Weakpoint window: buffed hit on the core damages the boss ──
    let window_open = { let s = st.lock().unwrap(); s.boss_flare_window_ticks > 0 };
    if window_open && buffed && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + 320.0).powi(2) {
        let mut s = st.lock().unwrap();
        s.boss_hp = (s.boss_hp - 2).max(0);
        s.buff_hit_flash = 20;
        let hp = s.boss_hp;
        drop(s);
        if hp <= 0 {
            if let Some(obj) = c.get_game_object_mut("boss") {
                obj.visible = false;
                obj.position = (-6000.0, -6000.0);
            }
            finish_boss(c, st);
        }
    }

    // ── Contact-rule inversion ──
    {
        let s = st.lock().unwrap();
        let touching = !s.dead
            && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + BOSS_SIZE * 0.5).powi(2);
        drop(s);
        if touching {
            super::hearts::lose_heart(c, st);
        }
    }
}

// ── Conductor (rhythm / Resonance) ───────────────────────────────────────────

/// The Conductor: a single-body boss fought to a beat. Releasing your tether
/// within a few frames of a beat (the release window) earns one stack of
/// Resonance; at three stacks the weakpoint arms and a buffed hit damages the
/// boss. Missing a beat costs a stack. Touching the body costs a heart.
fn tick_conductor(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }
    if s.dead { return; }
    if s.boss_stasis_active { return; }

    if !s.boss_spawned {
        s.boss_spawned = true;
        let name = s.boss_kind.name();
        drop(s);
        let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
        let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (spawn_x, spawn_y);
            obj.visible = true;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(90, 220, 200, 255), 1000.0 * sc,
                )));
            }
        }
        s = st.lock().unwrap();
        s.boss_beat_ticks = s.boss_beat_interval;
    }

    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };

    // Detect a release edge (hooked last tick, free now) and open the window.
    let mut beat_hit = false;
    {
        let mut s = st.lock().unwrap();
        let hooked = s.hooked;
        if s.boss_was_hooked && !hooked {
            s.boss_release_window = CONDUCTOR_RELEASE_WINDOW;
        }
        s.boss_was_hooked = hooked;
        if s.boss_release_window > 0 {
            s.boss_release_window -= 1;
        }

        // Advance the beat.
        if s.boss_beat_ticks > 0 {
            s.boss_beat_ticks -= 1;
        }
        if s.boss_beat_ticks == 0 {
            // Beat landed. On-beat release → Resonance stack; otherwise lose one.
            if s.boss_release_window > 0 {
                s.boss_resonance = (s.boss_resonance + 1).min(CONDUCTOR_RESONANCE_REQUIRED);
            } else if s.boss_resonance > 0 {
                s.boss_resonance -= 1;
            }
            // Phase two: faster bar once below half HP.
            let speedup = s.boss_hp <= BOSS_MAX_HP / 2;
            s.boss_beat_interval = if speedup { 30 } else { 36 };
            s.boss_beat_ticks = s.boss_beat_interval;
            // At full Resonance, arm the weakpoint window.
            if s.boss_resonance >= CONDUCTOR_RESONANCE_REQUIRED {
                s.boss_flare_window_ticks = 180;
            }
            beat_hit = true;
        }
    }

    // ── Weakpoint window (armed) → buffed hit damages the boss ──
    let window_open = { let s = st.lock().unwrap(); s.boss_flare_window_ticks > 0 && s.boss_resonance >= CONDUCTOR_RESONANCE_REQUIRED };
    if window_open && buffed && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + 320.0).powi(2) {
        let mut s = st.lock().unwrap();
        s.boss_hp = (s.boss_hp - 2).max(0);
        s.buff_hit_flash = 20;
        s.boss_resonance = 0; // spent: rebuild the stacks.
        let hp = s.boss_hp;
        drop(s);
        if hp <= 0 {
            if let Some(obj) = c.get_game_object_mut("boss") {
                obj.visible = false;
                obj.position = (-6000.0, -6000.0);
            }
            finish_boss(c, st);
        }
    }

    // ── Contact-rule inversion ──
    {
        let s = st.lock().unwrap();
        let touching = !s.dead
            && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + BOSS_SIZE * 0.5).powi(2);
        drop(s);
        if touching {
            super::hearts::lose_heart(c, st);
        }
    }
}

// ── Serpent (tetherable chain) ───────────────────────────────────────────────

/// The Serpent: a multi-part boss whose body IS the level. Eight segments trail
/// the head in a chain; a buffed hit destroys a segment, which shortens + speeds
/// the body. The head is invulnerable until every segment is gone, then it
/// detaches and is the only target. Touching a live segment or the head costs a
/// heart.
fn tick_serpent(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }
    if s.dead { return; }
    if s.boss_stasis_active { return; }

    if !s.boss_spawned {
        s.boss_spawned = true;
        let name = s.boss_kind.name();
        drop(s);
        let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
        let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (spawn_x, spawn_y);
            obj.visible = true;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(130, 255, 170, 255), 1000.0 * sc,
                )));
            }
        }
        s = st.lock().unwrap();
    }

    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    // Advance the serpent's undulation: the same phase drives both the visuals
    // and the hit-boxes so a segment you see is exactly where it can be hit.
    s.boss_phase += 0.035;
    let phase = s.boss_phase;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };

    // Reveal the chain of segment visuals (each on its travelling-wave offset).
    {
        let s = st.lock().unwrap();
        for i in 0..s.boss_parts.len() {
            let alive = s.boss_parts[i].alive;
            let (ox, oy) = serpent_part_offset(i as f32, phase);
            let sx = bcx + ox;
            let sy = bcy + oy;
            if let Some(obj) = c.get_game_object_mut(&format!("serpent_part_{i}")) {
                if alive {
                    obj.position = (sx - 65.0, sy - 65.0);
                    obj.visible = true;
                } else {
                    obj.visible = false;
                }
            }
        }
    }

    // ── Destroy segments ──
    let mut seg_positions = Vec::new();
    let mut hit_seg = false;
    {
        let mut s = st.lock().unwrap();
        for i in 0..s.boss_parts.len() {
            if !s.boss_parts[i].alive { continue; }
            // Segment trails behind the head on the same travelling wave.
            let (ox, oy) = serpent_part_offset(i as f32, phase);
            let sx = bcx + ox;
            let sy = bcy + oy;
            seg_positions.push((sx, sy));
            // A buffed hit near an exposed segment damages it (the player swings
            // at the near/exposed face).
            if buffed && (px - sx).powi(2) + (py - sy).powi(2) < (PLAYER_R + 200.0).powi(2) {
                let p = &mut s.boss_parts[i];
                p.hp -= 1;
                if p.hp <= 0 { p.alive = false; }
                hit_seg = true;
            }
        }
        if hit_seg {
            s.buff_hit_flash = 20;
        }
    }

    // ── Head is only hittable once every segment is destroyed ──
    let all_dead = { let s = st.lock().unwrap(); s.boss_parts.iter().all(|p| !p.alive) };
    if all_dead && buffed && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + 320.0).powi(2) {
        let mut s = st.lock().unwrap();
        s.boss_hp = (s.boss_hp - 1).max(0);
        s.buff_hit_flash = 20;
    }

    // ── Contact-rule inversion ──
    {
        let s = st.lock().unwrap();
        let mut touching = !s.dead
            && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + BOSS_SIZE * 0.5).powi(2);
        if !touching {
            for (sx, sy) in &seg_positions {
                if (px - *sx).powi(2) + (py - *sy).powi(2) < (PLAYER_R + 200.0).powi(2) {
                    touching = true;
                    break;
                }
            }
        }
        drop(s);
        if touching {
            super::hearts::lose_heart(c, st);
        }
    }

    // ── Win when the head is destroyed ──
    let head_dead = { let s = st.lock().unwrap(); s.boss_hp <= 0 };
    if head_dead {
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
        }
        finish_boss(c, st);
    }
}

/// Push a small electricity effect over each buff tether node so they read as
/// distinct from the regular grab nodes (the same round electricity the player
/// gets while buffed).
/// The electric aura on a buff node.
///
/// ATTACHED to each node rather than pushed as a screen overlay. A pushed mega
/// sprite is drawn in a pass that runs after every other renderer, so it sat in
/// front of the arena asteroids even when an asteroid was between the camera
/// and the node — the aura read as being in a different plane from the node it
/// belongs to. Attaching it makes the sprite part of the node's own draw, so it
/// takes the node's layer and depth and is occluded by exactly what occludes
/// the node.
///
/// The player's own buff dome stays a pushed overlay (see `solar.rs`): that one
/// genuinely must never be hidden by scenery.
///
/// Attached sprites persist until cleared, so every id touched last frame that
/// is no longer a buff node has to be cleared explicitly — a recycled pool slot
/// would otherwise keep wearing an aura for something it is no longer.
fn tick_buff_node_elec(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (candidates, previous) = {
        let s = st.lock().unwrap();
        (s.live_hooks.clone(), s.buff_fx_attached.clone())
    };

    let mut attached: Vec<String> = Vec::new();
    for id in &candidates {
        let Some(obj) = c.get_game_object(id) else { continue; };
        if !obj.visible || !obj.tags.iter().any(|t| t == BUFF_HOOK_TAG) { continue; }
        let cx = obj.position.0 + obj.size.0 * 0.5;
        let cy = obj.position.1 + obj.size.1 * 0.5;
        super::fx::attach_electric_fx(
            c, id, (cx, cy),
            (HOOK_R * 2.6, HOOK_R * 2.6),
            (0.6, 0.95, 1.0, 0.7),
        );
        attached.push(id.clone());
    }

    for id in &previous {
        if !attached.contains(id) {
            super::fx::clear_object_fx(c, id);
        }
    }

    st.lock().unwrap().buff_fx_attached = attached;
}

/// Position the weakpoint marker rings on the boss body, visible only while the
/// boss is up, so players can see where to land buffed hits.
fn tick_boss_weakpoints(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let active = {
        let s = st.lock().unwrap();
        s.boss_active && s.boss_spawned && s.boss_hp > 0
    };
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    for (i, (ox, oy)) in BOSS_WEAKPOINT_OFFSETS.iter().enumerate() {
        let id = format!("boss_weak_{i}");
        let Some(obj) = c.get_game_object_mut(&id) else { continue; };
        if active {
            let r = BOSS_WEAKPOINT_R;
            obj.position = (bcx + ox - r, bcy + oy - r);
            obj.visible = true;
        } else {
            obj.visible = false;
        }
    }
}

// ── Boss darkness attack (uses the quartz lighting system) ───────────────────

/// The Sun Eater's darkness attack.
///
/// Runs the SAME night-mode post pass and player lamp chain the eclipse
/// approach uses (`eclipse::begin_night_mode`), at the `boss_dark` preset.
/// Previously this only dropped the ambient — but quartz lighting is
/// multiplicative, so an ambient of 0.06 with no light source in the world is
/// not a dark room, it is a blank screen: there is nothing for the lamp to
/// restore because there is no lamp. The attack was unreadable and unfair for
/// exactly that reason.
///
/// With the shared entry point the player becomes the light source for three
/// seconds, the node markers stay lit so the route is still findable, and the
/// vignette closes in — the attack takes away the room, not the game.
fn tick_boss_darkness(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 { return; }

    if s.boss_dark_active {
        s.boss_dark_ticks = s.boss_dark_ticks.saturating_sub(1);
        let ending = s.boss_dark_ticks == 0;
        if ending {
            s.boss_dark_active = false;
            s.boss_dark_cooldown = BOSS_DARK_INTERVAL;
        }
        drop(s);
        if ending {
            c.set_var("boss_darkness", false);
            super::eclipse::end_night_mode(c);
            super::eclipse::kill_node_lights(c);
            if c.has_lighting() {
                c.set_ambient(Color(255, 255, 255, 255), 1.0);
            }
        } else {
            // Hold the markers lit for the whole attack. They are attached
            // per pool slot, so this only has to match `enabled` to `visible`.
            super::eclipse::drive_node_lights(c, ECLIPSE_NODE_LIGHT_INTENSITY);
        }
    } else {
        s.boss_dark_cooldown = s.boss_dark_cooldown.saturating_sub(1);
        let starting = s.boss_dark_cooldown == 0;
        if starting {
            s.boss_dark_active = true;
            s.boss_dark_ticks = BOSS_DARK_DURATION;
        }
        drop(s);
        if starting {
            c.set_var("boss_darkness", true);
            super::eclipse::ensure_node_lights(c);
            super::eclipse::begin_night_mode(c, super::eclipse::NightPost::boss_dark());
            super::eclipse::drive_node_lights(c, ECLIPSE_NODE_LIGHT_INTENSITY);
            if c.has_lighting() {
                c.set_ambient(Color(10, 10, 25, 255), BOSS_DARK_AMBIENT);
            }
        }
    }
}

// ── Zone entry + arena clear ──────────────────────────────────────────────────

// ── Arena placement ──────────────────────────────────────────────────────────
//
// The arena used to be two constants, so a run could only ever hold one fight.
// It is now a per-fight region recorded on the canvas at entry, which is what
// lets `mode::boss_trigger_distance` schedule several. The player is warped in
// and out, so the region can sit anywhere — it does not have to be reachable by
// swinging, and successive arenas simply step further along the X axis.

/// Centre of the arena for the fight currently being set up.
///
/// `BOSS_ARENA_CENTER_X` is derived from the ORIGINAL fixed arena constants and
/// is now wrong for every fight: when arenas became relocatable the walls moved
/// and this did not, so the warp destination, the boss spawn and the boss's
/// movement centre all stayed at the old 27 000 while the walls sat millions of
/// pixels away. Use this instead.
fn arena_center_x(c: &Canvas) -> f32 {
    let (x1, x2) = arena_bounds(c);
    (x1 + x2) * 0.5
}

/// Left and right walls of the arena for the fight currently being set up.
fn arena_bounds(c: &Canvas) -> (f32, f32) {
    let x1 = match c.get_var("boss_arena_x1") {
        Some(Value::F32(v)) => v,
        Some(Value::F64(v)) => v as f32,
        _ => BOSS_ZONE_X1,
    };
    let x2 = match c.get_var("boss_arena_x2") {
        Some(Value::F32(v)) => v,
        Some(Value::F64(v)) => v as f32,
        _ => BOSS_ZONE_X2,
    };
    (x1, x2)
}

/// Place the arena for fight `index`. Successive arenas are laid end to end
/// with a wide gap, far past anything the normal generator will ever reach.
fn place_arena(c: &mut Canvas, index: u32) {
    let stride = (BOSS_ZONE_X2 - BOSS_ZONE_X1) + BOSS_ARENA_GAP;
    let x1 = crate::mode::BOSS_ARENA_ORIGIN_X + stride * index as f32;
    let x2 = x1 + (BOSS_ZONE_X2 - BOSS_ZONE_X1);
    c.set_var("boss_arena_x1", Value::F32(x1));
    c.set_var("boss_arena_x2", Value::F32(x2));
}

fn tick_boss_zone_entry(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Which fights this run has, and where, is the mode's business.
    let mode = crate::mode::current_mode(c);
    // `boss_mode_active` remains an override so the debug/test harness can force
    // a fight in any mode.
    let forced_mode = matches!(c.get_var("boss_mode_active"), Some(Value::Bool(true)));
    if !mode.has_bosses() && !forced_mode { return; }

    let mut s = st.lock().unwrap();
    // The distance that triggers the NEXT fight. `None` means this run has no
    // fights left — the schedule is exhausted, so the boss flow is done.
    let threshold = match crate::mode::boss_trigger_distance(mode, s.boss_index) {
        Some(d) => SPAWN_X + d,
        None if forced_mode => BOSS_THRESHOLD_X,
        None => return,
    };
    // Only fire in normal game, not space mode.
    if s.in_space_mode || s.space_launch_active { return; }
    if s.dead { return; }

    // Spawn the pre-portal approach grapple nodes once, as the player nears the
    // boss threshold, so there is always a swing path up to the portal. Drop the
    // lock first because the spawner takes its own.
    if !s.boss_active && !s.boss_approach_nodes_spawned
        && s.px >= threshold - BOSS_APPROACH_RANGE
    {
        drop(s);
        spawn_boss_approach_nodes(c, st, threshold);
        s = st.lock().unwrap();
    }

    // Boss entry: reach the threshold, or be force-warped (debug/test harness).
    // Safe read: `get_bool` panics if the var is unset in a normal boss run.
    let force = matches!(c.get_var("force_boss_warp"), Some(Value::Bool(true)));
    if !s.boss_active && (s.px >= threshold || force) {
        // Remember where the level was left, so victory returns the player to
        // the run rather than stranding them in the arena's stretch of X.
        s.boss_return_x = s.px;
        s.boss_return_y = s.py;
        s.boss_approach_nodes_spawned = false; // re-arm for the next fight
        drop(s);
        c.set_var("boss_defeated_this_fight", false);
        let mut s = st.lock().unwrap();
        let index = s.boss_index;
        drop(s);
        place_arena(c, index);
        let mut s = st.lock().unwrap();
        s.boss_active = true;
        // A debug override lets the headless harness validate the existing
        // Sun Devourer fight regardless of which roster slot is being warped to
        // (once the new bosses land, index 0 is the Colossus).
        s.boss_kind = if matches!(c.get_var("debug_boss_kind_sundev"), Some(Value::Bool(true))) {
            crate::constants::BossKind::SunDevourer
        } else {
            crate::constants::boss_kind_for_index(index)
        };
        s.boss_parts = crate::constants::boss_parts_for_kind(s.boss_kind);
        s.boss_cleared = false;
        s.boss_entry_ticks = 0;
        s.boss_phase = 0.0;
        s.boss_hp = BOSS_MAX_HP;
        s.boss_shoot_timer = BOSS_SHOOT_INTERVAL;
        drop(s);
        if let Some(obj) = c.get_game_object_mut("boss_hp_bar") {
            obj.visible = true;
        }
        return;
    }

    if !s.boss_active { return; }

    // One-shot arena clear: remove all hooks/pads/coins/turrets/bullets/etc.
    if !s.boss_cleared {
        s.boss_cleared = true;

        // --- hooks ---
        let hooks: Vec<String> = s.live_hooks.drain(..).collect();
        s.spawn_animations.retain(|a| !hooks.contains(&a.id));
        for id in &hooks { s.pool_free.push(id.clone()); }

        // --- pads (also hide thruster) ---
        let pads: Vec<String> = s.pad_live.drain(..).collect();
        for id in &pads { s.pad_free.push(id.clone()); }

        // --- spinners ---
        let spinners: Vec<String> = s.spinner_live.drain(..).collect();
        for id in &spinners { s.spinner_free.push(id.clone()); }

        // --- coins ---
        let coins: Vec<String> = s.coin_live.drain(..).collect();
        for id in &coins { s.coin_free.push(id.clone()); }

        // --- flips ---
        let flips: Vec<String> = s.flip_live.drain(..).collect();
        for id in &flips { s.flip_free.push(id.clone()); }

        // --- score x2 ---
        let sx2: Vec<String> = s.score_x2_live.drain(..).collect();
        for id in &sx2 { s.score_x2_free.push(id.clone()); }

        // --- zero-g ---
        let zg: Vec<String> = s.zero_g_live.drain(..).collect();
        for id in &zg { s.zero_g_free.push(id.clone()); }

        // --- gates ---
        let gates: Vec<String> = s.gate_live.drain(..).collect();
        for id in &gates { s.gate_free.push(id.clone()); }

        // --- gravity wells ---
        let gwells: Vec<String> = s.gwell_live.drain(..).collect();
        s.gwell_timers.retain(|(gid, _, _)| !gwells.contains(gid));
        for id in &gwells { s.gwell_free.push(id.clone()); }

        // --- turrets + bullets ---
        let turrets: Vec<String> = s.turret_live.drain(..).collect();
        s.turret_timers.retain(|(gid, _)| !turrets.contains(gid));
        for id in &turrets { s.turret_free.push(id.clone()); }
        let bullets: Vec<(String, f32, f32, u32)> = s.bullet_live.drain(..).collect();
        for (id, _, _, _) in &bullets { s.bullet_free.push(id.clone()); }

        // --- rocket pads ---
        let rpads: Vec<String> = s.rocket_pad_live.drain(..).collect();
        for id in &rpads { s.rocket_pad_free.push(id.clone()); }

        // --- floating world asteroids outside arena ---
        // Remove all existing floating asteroid GIFs when entering boss zone.
        let boss_asteroid_ids = s.boss_asteroids.clone();
        let mut world_asteroids: Vec<String> = s.space_asteroid_live.drain(..).collect();
        world_asteroids.retain(|id| !boss_asteroid_ids.contains(id));
        for id in &world_asteroids { s.space_asteroid_free.push(id.clone()); }

        // Register boss asteroids as live so collision systems treat them
        // exactly like regular floating asteroids.
        for id in &boss_asteroid_ids {
            if !s.space_asteroid_live.contains(id) {
                s.space_asteroid_live.push(id.clone());
            }
        }

        // Kill any in-flight spawn animations for all cleared objects so
        // tick_spawn_animations cannot make them visible again after the clear.
        s.spawn_animations.retain(|a| {
            !spinners.contains(&a.id)
                && !pads.contains(&a.id)
                && !coins.contains(&a.id)
                && !flips.contains(&a.id)
                && !sx2.contains(&a.id)
                && !zg.contains(&a.id)
                && !gates.contains(&a.id)
                && !gwells.contains(&a.id)
                && !turrets.contains(&a.id)
                && !rpads.contains(&a.id)
                && !world_asteroids.contains(&a.id)
        });

        // Collect all ids to hide.
        let mut all_hide: Vec<String> = hooks;
        all_hide.extend(pads.iter().cloned());
        // Also hide pad thrusters (named pad_X_thruster)
        let thr_ids: Vec<String> = pads.iter().map(|n| format!("{n}_thruster")).collect();
        all_hide.extend(thr_ids);
        all_hide.extend(spinners);
        all_hide.extend(coins);
        all_hide.extend(flips);
        all_hide.extend(sx2);
        all_hide.extend(zg);
        all_hide.extend(gates.iter().flat_map(|g| [format!("{g}_top"), format!("{g}_bot")]));
        all_hide.extend(gwells);
        all_hide.extend(turrets);
        all_hide.extend(bullets.iter().map(|(id, _, _, _)| id.clone()));
        all_hide.extend(rpads);
        all_hide.extend(world_asteroids.clone());

        drop(s);

        for id in &all_hide {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.visible = false;
                obj.position = (-3000.0, -3000.0);
                obj.momentum = (0.0, 0.0);
            }
        }

        // The player has crossed into the boss arena — hide the threshold marker.
        if let Some(obj) = c.get_game_object_mut("boss_threshold_marker") {
            obj.visible = false;
        }
        if let Some(obj) = c.get_game_object_mut("boss_marker_arrow") {
            obj.visible = false;
        }

        // Apply reduced gravity to player for the boss zone.
        if let Some(obj) = c.get_game_object_mut("player") {
            // Preserve sign (flipped gravity support).
            let cur = obj.gravity;
            let sign = if cur < 0.0 { -1.0_f32 } else { 1.0_f32 };
            obj.gravity = sign * GRAVITY * BOSS_GRAVITY_SCALE;
        }

        // Spawn boss-zone asteroids immediately on entry.
        place_boss_asteroids(c, &boss_asteroid_ids);
        // Spawn climbable tether nodes across the arena so the player can
        // swing up to reach the upper-sky boss.
        spawn_arena_tether_nodes(c, st);
        // Warp the player into the arena with a wormhole-style flash.
        warp_player_into_arena(c, st);
        // Barrier + generators are the Sun Devourer's finale set-dressing only;
        // no other roster boss gets them.
        if st.lock().unwrap().boss_kind == crate::constants::BossKind::SunDevourer {
            spawn_generators_and_barrier(c, st);
        }
        // Hold the player in a stasis orbit around a safe tether node so they
        // can get their bearings before the battle starts. Tether to a node to
        // begin (see `tick_boss_stasis`).
        enter_boss_stasis(c, st);
        return;
    }

    // Clamp player inside boss zone while boss is alive.
    if s.boss_hp > 0 {
        let (zx1, zx2) = arena_bounds(c);
        let half = PLAYER_R;
        if s.px < zx1 + half {
            s.px = zx1 + half;
            s.vx = s.vx.max(0.0);
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                if obj.position.0 < zx1 + half - PLAYER_R {
                    obj.position.0 = zx1 + half - PLAYER_R;
                }
                if obj.momentum.0 < 0.0 { obj.momentum.0 = 0.0; }
            }
        } else if s.px > zx2 - half {
            s.px = zx2 - half;
            s.vx = s.vx.min(0.0);
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                if obj.position.0 > zx2 - half - PLAYER_R {
                    obj.position.0 = zx2 - half - PLAYER_R;
                }
                if obj.momentum.0 > 0.0 { obj.momentum.0 = 0.0; }
            }
        }
    }
}

fn place_boss_asteroids(c: &mut Canvas, asteroid_ids: &[String]) {
    let (zx1, zx2) = arena_bounds(c);
    let anim = hook_asteroid_anim_for_spawn();
    let zone_w = zx2 - zx1;
    const Y_TOP:  f32 = -3500.0;
    const Y_BOT:  f32 =  1500.0;

    // Deterministic hash RNG for stable-but-arbitrary placement.
    fn hash01(mut x: u32) -> f32 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x as f32) / (u32::MAX as f32)
    }

    // Build a blue-noise-ish point set so spacing stays wide while looking random.
    let mut points: Vec<(f32, f32)> = Vec::new();
    let min_sep = 1120.0;
    let min_sep2 = min_sep * min_sep;
    let mut seed = 0xC0FFEE_u32;
    for _ in 0..600 {
        if points.len() >= asteroid_ids.len() { break; }
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let rx = hash01(seed);
        let ry = hash01(seed ^ 0x9E37_79B9);
        let x = zx1 + rx * zone_w;
        let y = Y_TOP + ry * (Y_BOT - Y_TOP);
        if points.iter().all(|(px, py)| {
            let dx = x - *px;
            let dy = y - *py;
            dx * dx + dy * dy >= min_sep2
        }) {
            points.push((x, y));
        }
    }

    // Fallback: if rejection sampling under-fills, top up with sparse jittered rows.
    if points.len() < asteroid_ids.len() {
        let need = asteroid_ids.len() - points.len();
        for i in 0..need {
            let t = (i as f32 + 0.5) / need as f32;
            let x = zx1 + zone_w * t + ((i as f32 * 173.0) % 500.0 - 250.0);
            let y = Y_TOP + (Y_BOT - Y_TOP) * ((i as f32 * 0.618_033_95) % 1.0)
                + ((i as f32 * 97.0) % 280.0 - 140.0);
            points.push((x.clamp(zx1, zx2), y.clamp(Y_TOP, Y_BOT)));
        }
    }

    for (i, id) in asteroid_ids.iter().enumerate() {
        let (ax, ay) = points[i.min(points.len().saturating_sub(1))];
        let dvx = ((i as f32 * 0.7 + 0.4) % 1.0 - 0.5) * 0.6;
        let dvy = ((i as f32 * 1.3 + 0.1) % 1.0 - 0.5) * 0.3;

        if let Some(obj) = c.get_game_object_mut(id) {
            obj.position = (ax - obj.size.0 * 0.5, ay - obj.size.1 * 0.5);
            obj.momentum = (dvx, dvy);
            obj.visible = true;
            if let Some(anim_ref) = &anim {
                obj.set_animation(anim_ref.clone());
            }
        }
    }
}

/// Activate a single arena tether node (image, tags, glow) and register it live.
fn activate_arena_tether_node(c: &mut Canvas, s: &mut State, id: String, hx: f32, hy: f32, is_buff: bool) {
    s.live_hooks.push(id.clone());
    if let Some(obj) = c.get_game_object_mut(&id) {
        obj.visible = true;
        obj.position = (hx - HOOK_R, hy - HOOK_R);
        obj.size = (HOOK_R * 2.0, HOOK_R * 2.0);
        obj.gravity = 0.0;
        obj.momentum = (0.0, 0.0);
        obj.rotation_momentum = 0.0;
        obj.collision_mode = CollisionMode::NonPlatform;
        obj.tags.retain(|t| t != "arena_node" && t != BUFF_HOOK_TAG);
        obj.tags.push("arena_node".into());
        if !obj.tags.iter().any(|t| t == "hook") {
            obj.tags.push("hook".into());
        }
        if is_buff {
            obj.tags.push(BUFF_HOOK_TAG.into());
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
                image: circle_cached(HOOK_R as u32, C_BUFF_HOOK.0, C_BUFF_HOOK.1, C_BUFF_HOOK.2),
                color: None,
            });
            obj.set_glow(GlowConfig { color: Color(110, 230, 255, 255), width: 16.0 });
        } else {
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
                image: circle_cached(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2),
                color: None,
            });
            obj.clear_glow();
        }
        obj.clear_highlight();
    } else {
        s.live_hooks.retain(|n| n != &id);
        s.pool_free.push(id);
    }
}

/// Spawn a small grid of climbable grab nodes spanning the arena X and the
/// upper-sky boss Y band (+300 down to −3600), so the player can swing up to
/// reach the boss at y ≈ −2500. Adds staggered gap-fill nodes between the main
/// columns so there are no large horizontal gaps.
fn spawn_arena_tether_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (zx1, zx2) = arena_bounds(c);
    let mut s = st.lock().unwrap();
    let zone_w = zx2 - zx1;
    let cols = 5;
    let rows = 6;
    let total = cols * rows;
    for i in 0..total {
        let Some(id) = s.pool_free.pop() else { break; };
        let col = i % cols;
        let row = i / cols;
        let frac = row as f32 / (rows - 1).max(1) as f32;
        let hy = 300.0 - frac * 4400.0; // +300 … −4100
        let hx = zx1 + zone_w * (0.5 + (col as f32 - (cols as f32 - 1.0) * 0.5) * 0.22);
        // Every third node is a buff node so the player can get a buff mid-fight.
        activate_arena_tether_node(c, &mut *s, id, hx, hy, i % 3 == 0);
    }

    // Staggered gap-fill nodes in the wide gaps between the main columns:
    // one near the bottom, one at mid height, one nearer the top.
    let gap_fracs = [0.17, 0.39, 0.61, 0.83];
    let gap_ys = [-3600.0, -2000.0, 500.0];
    for gf in gap_fracs {
        let gx = zx1 + zone_w * gf;
        for &gy in &gap_ys {
            let Some(id) = s.pool_free.pop() else { break; };
            activate_arena_tether_node(c, &mut *s, id, gx, gy, false);
        }
    }
}

/// Reveal the boss threshold marker once the player approaches the portal. The
/// procedural hooks now cover the approach densely (after the stride tightening
/// in constants.rs), so no extra approach nodes are placed here.
fn spawn_boss_approach_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>, threshold: f32) {
    let mut s = st.lock().unwrap();
    if s.boss_approach_nodes_spawned {
        return;
    }
    s.boss_approach_nodes_spawned = true;
    drop(s);

    // Reveal the huge black-hole threshold marker so the player knows they are
    // heading into something special — and MOVE IT to the fight that is
    // actually coming.
    //
    // Both objects were built once at `BOSS_THRESHOLD_X` (20 000), which was
    // the trigger back when a run held exactly one fight. The trigger is now
    // scheduled, so by the time this revealed them they were hundreds of
    // thousands of pixels behind the player and the teleport arrived with no
    // warning at all.
    let d = BOSS_MARKER_D;
    if let Some(obj) = c.get_game_object_mut("boss_threshold_marker") {
        obj.position = (threshold - d * 0.5, BOSS_MARKER_Y - d * 0.5);
        obj.visible = true;
    }
    let asz = BOSS_MARKER_ARROW_D;
    if let Some(obj) = c.get_game_object_mut("boss_marker_arrow") {
        obj.position = (threshold - asz * 0.5, (BOSS_MARKER_Y - 900.0) - asz * 0.5);
        obj.visible = true;
    }
}

/// Warp the player into the boss arena (wormhole-style flash) at the bottom of
/// the tether grid, so the fight is entered cleanly rather than the player
/// having to climb from the normal zone.
fn warp_player_into_arena(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let warp_x = arena_center_x(c);
    let warp_y = 200.0;
    {
        let mut s = st.lock().unwrap();
        s.px = warp_x;
        s.py = warp_y;
        s.vx = 0.0;
        s.vy = 0.0;
        s.hooked = false;
        s.active_hook = String::new();
        s.hook_x = warp_x;
        s.hook_y = warp_y;
        s.rope_len = RESPAWN_ORBIT_R;
        s.cannon_captured = false;
        s.in_space_mode = false;
    }
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (warp_x - PLAYER_R, warp_y - PLAYER_R);
        obj.momentum = (0.0, 0.0);
        obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE;
        obj.visible = true;
    }
    c.run(Action::Hide { target: Target::name("rope") });
    c.set_var("rope_visible_at_pause", false);
    // Show the wormhole gif over the player during the warp — a defined-size
    // portal so the opening reads as a visible wormhole (not an invisible
    // full-screen smear). tick_warp_flash re-centres it on the player.
    if let Some(obj) = c.get_game_object_mut("warp_flash") {
        obj.set_animation(wormhole2_template());
        obj.size = (BOSS_WORMHOLE_D, BOSS_WORMHOLE_D);
        obj.position = (warp_x - BOSS_WORMHOLE_D * 0.5, warp_y - BOSS_WORMHOLE_D * 0.5);
        obj.visible = true;
    }
    c.set_var("warp_flash_ticks", 40i32);
    if let Some(cam) = c.camera_mut() {
        cam.flash_with(
            Color(160, 160, 255, 130),
            0.40,
            FlashMode::Pulse,
            FlashEase::Sharp,
            0.9,
            0.0,
        );
    }
}

/// Enter a stasis orbit around a safe tether node after teleporting into the
/// boss arena. The battle stays inactive while the player orbits so they can
/// get their bearings; tethering to a node (grabbing it) ends the stasis and
/// starts the fight (see `tick_boss_stasis`).
fn enter_boss_stasis(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (warp_x, warp_y) = (arena_center_x(c), 200.0);
    let mut s = st.lock().unwrap();
    s.boss_stasis_active = true;
    s.boss_stasis_ticks = 0;
    s.boss_stasis_hook = String::new();

    // Pick the tether node nearest the warp point as the "safe node".
    let mut best: Option<(f32, f32, f32, String)> = None;
    for id in &s.live_hooks {
        if let Some(obj) = c.get_game_object(id) {
            let hcx = obj.position.0 + obj.size.0 * 0.5;
            let hcy = obj.position.1 + obj.size.1 * 0.5;
            let d2 = (hcx - warp_x).powi(2) + (hcy - warp_y).powi(2);
            if best.as_ref().map(|(bd, _, _, _)| d2 < *bd).unwrap_or(true) {
                best = Some((d2, hcx, hcy, id.clone()));
            }
        }
    }
    let (_, hx, hy, hook_id) = best.unwrap_or((0.0, warp_x, warp_y, String::new()));
    s.boss_stasis_hook = hook_id;

    // Position the player at the top of the orbit around the safe node.
    s.hook_x = hx;
    s.hook_y = hy;
    s.px = hx;
    s.py = hy - RESPAWN_ORBIT_R;
    s.vx = 0.0;
    s.vy = 0.0;
    s.hooked = false;
    s.active_hook = String::new();
    s.rope_len = RESPAWN_ORBIT_R;
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (hx - PLAYER_R, (hy - RESPAWN_ORBIT_R) - PLAYER_R);
        obj.momentum = (0.0, 0.0);
        obj.visible = true;
    }

    // Show a prompt telling the player to tether to begin.
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
        let scale = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
            obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                "TETHER TO A NODE TO BEGIN",
                &font,
                52.0 * scale,
                Color(200, 235, 255, 255),
                1300.0 * scale,
            )));
            obj.visible = true;
        }
    }
}

/// Drive the boss stasis orbit each tick. While the player is in stasis, keep
/// them circling the safe node so they can survey the arena; the moment they
/// tether (grab a node) the stasis ends and the battle starts. A debug var
/// (`debug_boss_stasis_down`) bypasses the tether for headless validation.
fn tick_boss_stasis(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_stasis_active { return; }

    // The player tethered to a node (or a debug bypass): end stasis, start.
    let force_down = matches!(c.get_var("debug_boss_stasis_down"), Some(Value::Bool(true)));
    if s.hooked || force_down {
        let victory = s.boss_hp <= 0;
        s.boss_stasis_active = false;
        s.boss_stasis_hook.clear();
        if !victory {
            // Make the boss spawn on the next appearance tick.
            s.boss_entry_ticks = BOSS_ENTRY_DELAY_TICKS;
        }
        drop(s);
        if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
            obj.visible = false;
        }
        if victory {
            complete_boss_finish(c, st);
        }
        return;
    }

    // Otherwise hold the player in a slow counter-clockwise orbit around the
    // safe node so they can look around before deciding to tether.
    let ticks = s.boss_stasis_ticks;
    s.boss_stasis_ticks = ticks + 1;
    let (hx, hy) = (s.hook_x, s.hook_y);
    const ORBIT_OMEGA: f32 = 0.038;
    let theta = -std::f32::consts::FRAC_PI_2 - ORBIT_OMEGA * ticks as f32;
    let px = hx + RESPAWN_ORBIT_R * theta.cos();
    let py = hy + RESPAWN_ORBIT_R * theta.sin();
    s.px = px;
    s.py = py;
    s.vx = 0.0;
    s.vy = 0.0;
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (px - PLAYER_R, py - PLAYER_R);
        obj.momentum = (0.0, 0.0);
    }
}

/// After a fall in the boss arena, reset the player into the stasis orbit while
/// preserving boss progress (HP, generators, final phase). The boss is unspawned
/// so it stops attacking during the orbit; it re-appears once the player tethers
/// to resume the fight.
pub fn reset_boss_after_fall(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let mut s = st.lock().unwrap();
        s.boss_spawned = false;
        s.boss_stasis_active = true;
        // Clear any active darkness so the stasis orbit is visible.
        s.boss_dark_active = false;
        s.boss_dark_ticks = 0;
    }
    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.visible = false;
        obj.position = (-6000.0, -6000.0);
        obj.momentum = (0.0, 0.0);
    }
    if c.has_lighting() {
        c.set_ambient(Color(255, 255, 255, 255), 1.0);
    }
    // Re-enter the stasis orbit (positions/orbits the player, shows the prompt).
    enter_boss_stasis(c, st);
}

/// Keep the boss, its bolts, and the arena tether nodes lit so they remain
/// faintly visible (and add ambience) during the darkness phase. Lights are
/// attached to their objects so they follow automatically; bolts are only lit
/// while active, and the boss light only while it is spawned.
fn tick_boss_lights(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    if !c.has_lighting() { return; }
    let (active, spawned) = {
        let s = st.lock().unwrap();
        (s.boss_active, s.boss_spawned)
    };
    if !active {
        return;
    }

    // Boss: faint purple light (no shadow cast — small ambient fill).
    if c.get_light("boss_light").is_none() {
        let mut ls = LightSource::new("boss_light", (0.0, 0.0), Color(170, 90, 230, 255), 1500.0, 0.32);
        ls.casts_shadows = false;
        c.add_light(ls);
        c.attach_light("boss_light", "boss", (0.0, 0.0));
    }
    c.set_light_enabled("boss_light", spawned);

    // Bolts: faint orange light, lit only while the bolt is visible.
    for i in 0..BOSS_BOLT_POOL_SIZE {
        let lid = format!("boss_bolt_light_{i}");
        if c.get_light(&lid).is_none() {
            let mut ls = LightSource::new(lid.clone(), (0.0, 0.0), Color(255, 120, 30, 255), 620.0, 0.5);
            ls.casts_shadows = false;
            c.add_light(ls);
            c.attach_light(&lid, &format!("boss_bolt_{i}"), (0.0, 0.0));
        }
        let vis = c.get_game_object(&format!("boss_bolt_{i}")).map(|o| o.visible).unwrap_or(false);
        c.set_light_enabled(&lid, vis);
    }

    // Arena tether nodes: faint cyan light so the player can navigate in the dark.
    let hooks: Vec<String> = {
        let s = st.lock().unwrap();
        s.live_hooks.clone()
    };
    for id in &hooks {
        let lid = format!("boss_node_light_{id}");
        if c.get_light(&lid).is_none() {
            let mut ls = LightSource::new(lid.clone(), (0.0, 0.0), Color(110, 230, 255, 255), 520.0, 0.45);
            ls.casts_shadows = false;
            c.add_light(ls);
            c.attach_light(&lid, id, (0.0, 0.0));
        }
        c.set_light_enabled(&lid, true);
    }
}

/// Count down and hide the boss-arena wormhole warp overlay. The cannon
/// fast-travel warp manages its own two-phase overlay via `cannon_warp_phase`,
/// so skip it here.
fn tick_warp_flash(c: &mut Canvas, _st: &Arc<Mutex<State>>) {
    if matches!(c.get_var("cannon_warp_phase"), Some(Value::I32(v)) if v > 0) {
        return;
    }
    let t = match c.get_var("warp_flash_ticks") {
        Some(Value::I32(v)) => v,
        _ => 0,
    };
    if t <= 0 {
        return;
    }
    // Keep the warp origin centred on the player so the speed lines converge on
    // the ball, not the middle of the screen.
    center_warp_on_player(c);
    c.set_var("warp_flash_ticks", t - 1);
    if t - 1 <= 0 {
        if let Some(obj) = c.get_game_object_mut("warp_flash") {
            obj.visible = false;
        }
    }
}

// ── Last-boss: barrier + generators + bait-and-bail ──────────────────────────

/// Position the barrier and generator nodes across the arena.
fn spawn_generators_and_barrier(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (zx1, zx2) = arena_bounds(c);
    let zone_w = zx2 - zx1;
    {
        let mut s = st.lock().unwrap();
        s.boss_generators.clear();
        s.boss_generator_hp.clear();
        for _ in 0..BOSS_GENERATOR_COUNT {
            s.boss_generators.push(String::new());
            s.boss_generator_hp.push(BOSS_GENERATOR_HP);
        }
        s.boss_barrier_up = true;
        s.boss_final_phase = false;
        s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH;
        s.boss_lunge_ticks = 0;
        s.boss_lunge_target = (0.0, 0.0);
    }
    for i in 0..BOSS_GENERATOR_COUNT {
        let id = format!("boss_gen_{i}");
        let frac = i as f32 / (BOSS_GENERATOR_COUNT - 1).max(1) as f32;
        let gx = zx1 + zone_w * (0.15 + frac * 0.7);
        let gy = -700.0 - frac * 2200.0;
        {
            let mut s = st.lock().unwrap();
            if i < s.boss_generators.len() {
                s.boss_generators[i] = id.clone();
            }
        }
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.visible = true;
            obj.position = (gx - BOSS_GENERATOR_R, gy - BOSS_GENERATOR_R);
            obj.momentum = (0.0, 0.0);
            obj.rotation_momentum = 0.0;
        }
    }
    if let Some(obj) = c.get_game_object_mut("boss_barrier") {
        obj.position = (zx1, BOSS_BARRIER_Y);
        obj.visible = true;
    }
}

/// Damage generators (buffed player hits, or the boss crashing into them), and
/// drop the barrier once all are down (entering the final bait-and-bail phase).
/// Show the boss forcefield ring while the generators are still up, and the
/// arena boundary forcefield while the boss fight is active.
fn tick_boss_forcefield(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (active, spawned, generators_up, hp) = {
        let s = st.lock().unwrap();
        let gens_up = !s.boss_generator_hp.is_empty() && s.boss_generator_hp.iter().any(|&hp| hp > 0);
        (s.boss_active, s.boss_spawned, gens_up, s.boss_hp)
    };
    let fighting = active && hp > 0;

    for id in ["boss_boundary_b", "boss_boundary_t", "boss_boundary_l", "boss_boundary_r"] {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = fighting;
        }
    }

    let boss_center = c.get_game_object("boss")
        .map(|o| (o.position.0 + BOSS_SIZE * 0.5, o.position.1 + BOSS_SIZE * 0.5));

    if let Some(obj) = c.get_game_object_mut("boss_forcefield") {
        if fighting && spawned && generators_up {
            if let Some((bcx, bcy)) = boss_center {
                let d = BOSS_SIZE * 1.5;
                obj.position = (bcx - d * 0.5, bcy - d * 0.5);
                obj.size = (d, d);
                obj.update_image_shape();
                obj.visible = true;
            } else {
                obj.visible = false;
            }
        } else {
            obj.visible = false;
        }
    }
}

fn tick_generators(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (px, py, buffed, boss_pos) = {
        let s = st.lock().unwrap();
        if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 {
            return;
        }
        (
            s.px,
            s.py,
            s.player_buff > 0,
            c.get_game_object("boss").map(|o| o.position).unwrap_or((-9999.0, -9999.0)),
        )
    };
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;

    let gens: Vec<String> = st.lock().unwrap().boss_generators.clone();
    let mut damaged: Vec<usize> = Vec::new();
    for (i, id) in gens.iter().enumerate() {
        let Some(obj) = c.get_game_object(id) else { continue; };
        if !obj.visible {
            continue;
        }
        let gx = obj.position.0 + obj.size.0 * 0.5;
        let gy = obj.position.1 + obj.size.1 * 0.5;
        if buffed {
            let dx = px - gx;
            let dy = py - gy;
            let r = PLAYER_R + BOSS_GENERATOR_R;
            if dx * dx + dy * dy < r * r {
                damaged.push(i);
                continue;
            }
        }
        // Boss crashing into the generator damages it (the "lure" path).
        let dx = bcx - gx;
        let dy = bcy - gy;
        let r = BOSS_SIZE * 0.5 + BOSS_GENERATOR_R;
        if dx * dx + dy * dy < r * r {
            damaged.push(i);
        }
    }
    if damaged.is_empty() {
        return;
    }
    {
        let mut s = st.lock().unwrap();
        for i in damaged {
            if i < s.boss_generator_hp.len() {
                s.boss_generator_hp[i] -= 1;
            }
        }
    }
    let mut all_down = true;
    {
        let mut s = st.lock().unwrap();
        for i in 0..s.boss_generator_hp.len() {
            if s.boss_generator_hp[i] <= 0 {
                if let Some(id) = s.boss_generators.get(i).cloned() {
                    if let Some(obj) = c.get_game_object_mut(&id) {
                        obj.visible = false;
                    }
                }
            } else {
                all_down = false;
            }
        }
    }
    if all_down {
        let mut s = st.lock().unwrap();
        s.boss_barrier_up = false;
        s.boss_final_phase = true;
        drop(s);
        if let Some(obj) = c.get_game_object_mut("boss_barrier") {
            obj.visible = false;
        }
    }
}

/// While the barrier is up, clamp the player (and boss) from crossing into the
/// sun side of the arena.
fn tick_barrier(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_barrier_up {
        return;
    }
    if s.py < BOSS_BARRIER_Y {
        s.py = BOSS_BARRIER_Y;
        if s.vy < 0.0 {
            s.vy = 0.0;
        }
        drop(s);
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.position.1 = BOSS_BARRIER_Y - PLAYER_R;
            if obj.momentum.1 < 0.0 {
                obj.momentum.1 = 0.0;
            }
        }
        return;
    }
    // Keep the boss on the safe side of the barrier too.
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((0.0, 0.0));
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    if bcy < BOSS_BARRIER_Y {
        drop(s);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position.1 = BOSS_BARRIER_Y - BOSS_SIZE * 0.5;
        }
    }
}

/// Final phase: the boss periodically lunges at where the player *was* (a
/// telegraphed bait), which is a dodge test and a window to counter-attack.
///
/// The lunge used to KILL the boss outright if it carried past the sun line —
/// the "bait-and-bail" finisher from the original design. That is gone: the sun
/// is no longer part of this fight, so the only way to end it is to bring the
/// barrier down by destroying both generators and then damage the boss with a
/// buffed weakpoint hit. The lunge is now purely an attack the player dodges.
fn tick_desperation(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (final_phase, active, spawned, hp) = {
        let s = st.lock().unwrap();
        (s.boss_final_phase, s.boss_active, s.boss_spawned, s.boss_hp)
    };
    if !final_phase || !active || !spawned || hp <= 0 {
        return;
    }

    let mut s = st.lock().unwrap();
    if s.boss_lunge_telegraph > 0 {
        s.boss_lunge_telegraph -= 1;
        if s.boss_lunge_telegraph == 0 {
            // Lock target to the player's current position (the bait).
            s.boss_lunge_target = (s.px, s.py);
            s.boss_lunge_ticks = 90;
        }
        drop(s);
        return;
    }
    if s.boss_lunge_ticks > 0 {
        s.boss_lunge_ticks -= 1;
        let (tx, ty) = s.boss_lunge_target;
        let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((0.0, 0.0));
        let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
        let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
        let dx = tx - bcx;
        let dy = ty - bcy;
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let spd = 42.0;
        let nvx = dx / d * spd;
        let nvy = dy / d * spd;
        let nx = bcx + nvx;
        let ny = bcy + nvy;
        s.boss_vx = nvx;
        s.boss_vy = nvy;
        let done = s.boss_lunge_ticks == 0;
        drop(s);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (nx - BOSS_SIZE * 0.5, ny - BOSS_SIZE * 0.5);
        }
        // NOTE: no sun-line kill here any more. The boss overshooting the top
        // of the arena used to end the fight instantly, which short-circuited
        // the generators-then-weakpoint loop that is now the whole fight.
        // Clamp instead, so a lunge cannot carry it out of the arena.
        let ny = ny.max(BOSS_ARENA_TOP_Y);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position.1 = ny - BOSS_SIZE * 0.5;
        }
        if done {
            let mut s = st.lock().unwrap();
            s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH;
        }
        return;
    }
    s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH.max(s.boss_lunge_telegraph);
}

/// Boss defeated: award a chunk of meta currency, hide the boss/generators, and
/// drop the player back into a stasis orbit with a congratulations message. The
/// actual end-of-fight cleanup (rewind frontiers, `boss_active=false`) happens
/// once the player tethers out of the victory stasis (`complete_boss_finish`).
fn finish_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let generator_ids;
    {
        let mut s = st.lock().unwrap();
        s.boss_hp = 0;
        s.boss_spawned = false;
        s.boss_stasis_active = true;
        s.boss_stasis_hook.clear();
        s.boss_entry_ticks = 0;
        s.boss_barrier_up = false;
        s.boss_final_phase = false;
        s.boss_lunge_ticks = 0;
        s.boss_lunge_telegraph = BOSS_LUNGE_TELEGRAPH;
        s.boss_killed = true;
        // A darkness attack in flight when the boss dies would otherwise leave
        // the night-mode post pass, the lamp and the dark ambient running for
        // the rest of the run — the post override is a single global slot and
        // nothing else would ever clear it.
        s.boss_dark_active = false;
        s.boss_dark_ticks = 0;
        s.boss_dark_cooldown = BOSS_DARK_INTERVAL;
        generator_ids = s.boss_generators.clone();
    }
    c.set_var("boss_darkness", false);
    super::eclipse::end_night_mode(c);
    super::eclipse::kill_node_lights(c);
    if c.has_lighting() {
        c.set_ambient(Color(255, 255, 255, 255), 1.0);
    }

    // Award meta currency for the permanent-roguelike upgrade pool, and coins
    // (on-hand) so a boss kill funds an in-run upgrade in the next link section.
    crate::profile::award_meta_currency(META_BOSS_REWARD);
    crate::profile::record_boss_defeated();
    {
        let mut s = st.lock().unwrap();
        s.coin_count = s.coin_count.saturating_add(BOSS_COIN_REWARD);
    }
    // Direct defeat signal. `boss_mode_cleared` now means "no fights left this
    // run", which is only true of the last one, so it is the wrong thing for a
    // per-fight check to read.
    c.set_var("boss_defeated_this_fight", true);

    for id in generator_ids {
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
        }
    }
    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.visible = false;
        obj.position = (-6000.0, -6000.0);
        obj.momentum = (0.0, 0.0);
    }
    if let Some(obj) = c.get_game_object_mut("boss_barrier") {
        obj.visible = false;
    }
    if c.has_lighting() {
        c.set_ambient(Color(255, 255, 255, 255), 1.0);
    }

    // Drop the player into the victory stasis orbit, then set the congrats text.
    enter_boss_stasis(c, st);
    let boss_name = st.lock().unwrap().boss_kind.name();
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
        let scale = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("start_prompt_text") {
            obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                &format!("CONGRATULATIONS! You defeated {boss_name}!  +{META_BOSS_REWARD} META"),
                &font,
                46.0 * scale,
                Color(255, 230, 140, 255),
                1400.0 * scale,
            )));
            obj.visible = true;
        }
    }
}

/// Complete the end-of-fight cleanup once the player tethers out of the victory
/// stasis: rewind spawn frontiers and hand control back to normal play.
fn complete_boss_finish(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Put the player back where the level was left. Without this they stay at
    // the arena's own stretch of X — fine when a run held exactly one fight and
    // ended there, fatal now that a run holds several.
    let (rx, ry) = {
        let s = st.lock().unwrap();
        (s.boss_return_x, s.boss_return_y)
    };
    {
        let mut s = st.lock().unwrap();
        s.px = rx;
        s.py = ry;
        s.vx = 0.0;
        s.vy = 0.0;
        s.hooked = false;
        s.active_hook.clear();
        s.hook_x = rx;
        s.hook_y = ry;
    }
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (rx - PLAYER_R, ry - PLAYER_R);
        obj.momentum = (0.0, 0.0);
    }
    if let Some(cam) = c.camera_mut() {
        cam.position = (rx - VW * 0.5, ry - VH * 0.5);
        cam.snap_zoom(1.0);
    }

    {
        let mut s = st.lock().unwrap();
        s.boss_active = false;
        s.boss_spawned = false;
        s.boss_cleared = false;
        let backfill_x = s.px - GEN_AHEAD * 0.35;
        s.rightmost_x = s.rightmost_x.min(backfill_x);
        s.pad_rightmost = s.pad_rightmost.min(backfill_x);
        s.spinner_rightmost = s.spinner_rightmost.min(backfill_x);
        s.coin_rightmost = s.coin_rightmost.min(backfill_x);
        s.flip_rightmost = s.flip_rightmost.min(backfill_x);
        s.score_x2_rightmost = s.score_x2_rightmost.min(backfill_x);
        s.zero_g_rightmost = s.zero_g_rightmost.min(backfill_x);
        s.gate_rightmost = s.gate_rightmost.min(backfill_x);
        s.gwell_rightmost = s.gwell_rightmost.min(backfill_x);
        s.turret_rightmost = s.turret_rightmost.min(backfill_x);
        s.rocket_pad_rightmost = s.rocket_pad_rightmost.min(backfill_x);
        s.upgrade_rightmost = s.upgrade_rightmost.min(backfill_x);
        // ALWAYS rebuild the pending hook queue from the backfilled position.
        // Leftover pre-arena hooks (when `pending` wasn't empty) sit at the
        // arena's stretch of X, which left a barren link section after the boss
        // that the player couldn't swing through until they died once.
        s.pending.clear();
        let mut seed = s.seed;
        let mut gen_head_x = s.gen_head_x.min(backfill_x);
        let mut gen_head_y = s.gen_head_y;
        let batch = gen_hook_batch(&mut seed, backfill_x, &mut gen_head_x, &mut gen_head_y, s.distance);
        s.seed = seed;
        s.gen_head_x = gen_head_x;
        s.gen_head_y = gen_head_y;
        s.pending.extend(batch);
    }
    if let Some(obj) = c.get_game_object_mut("boss_hp_bar") {
        obj.visible = false;
    }
    if c.has_lighting() {
        c.set_light_enabled("boss_light", false);
        for i in 0..BOSS_BOLT_POOL_SIZE {
            c.set_light_enabled(&format!("boss_bolt_light_{i}"), false);
        }
    }

    // Advance the schedule. A run holds several fights now, so clearing one
    // arms the next rather than ending the boss flow for good — the schedule
    // running out is what ends it (`boss_trigger_distance` returns None).
    let mode = crate::mode::current_mode(c);
    let (index, was_final) = {
        let mut s = st.lock().unwrap();
        let was_final = crate::mode::is_final_boss(mode, s.boss_index);
        s.boss_index = s.boss_index.saturating_add(1);
        (s.boss_index, was_final)
    };
    c.set_var("boss_index", Value::I32(index as i32));
    c.set_var("boss_mode_cleared", was_final);
    c.set_var("bosses_defeated", Value::I32(index as i32));
}

// ── Boss appearance after delay ───────────────────────────────────────────────

fn tick_boss_appearance(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || s.boss_spawned || s.boss_stasis_active { return; }

    s.boss_entry_ticks = s.boss_entry_ticks.saturating_add(1);
    if s.boss_entry_ticks < BOSS_ENTRY_DELAY_TICKS { return; }

    s.boss_spawned = true;

    // Spawn phase starts from the top of the lissajous sweep.
    s.boss_phase = std::f32::consts::FRAC_PI_2;
    let boss_name = s.boss_kind.name();
    drop(s);

    // Show this boss's name on the banner (set once, at spawn).
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
        let sc = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut("boss_name_text") {
            obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                boss_name, &font, 42.0 * sc, Color(200, 60, 220, 255), 1000.0 * sc,
            )));
        }
    }

    // Place boss at its initial lissajous position (top-center).
    let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
    let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.position = (spawn_x, spawn_y);
        obj.visible = true;
    }
}

// ── Boss movement — lissajous figure-8 ───────────────────────────────────────
// Horizontal: A·sin(phase)         → sweeps full arena width
// Vertical:   B·sin(2·phase + π/4) → two vertical cycles per horizontal sweep
// The asymmetric phase offset makes it feel less mechanical.

fn tick_boss_movement(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (zx1, zx2) = arena_bounds(c);
    let (cur_x, cur_y) = if let Some(obj) = c.get_game_object("boss") {
        (obj.position.0 + BOSS_SIZE * 0.5, obj.position.1 + BOSS_SIZE * 0.5)
    } else {
        (arena_center_x(c), BOSS_Y_CENTER)
    };

    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 { return; }

    let px = s.px;
    let py = s.py;

    // Smooth Lissajous 3:2 figure — natural, coherent looping pattern.
    s.boss_phase += BOSS_PHASE_X_SPEED * 3.2;
    let phase = s.boss_phase;

    // Lissajous curve: x = sin(3*phase), y = sin(2*phase + offset)
    // This creates a smooth 3-loop pattern that visits all quadrants naturally
    let x_liss = (phase * 3.0).sin();
    let y_liss = (phase * 2.0 + 0.5).sin();

    // Map to arena bounds: X = [-1,1] → [20000, 34000], Y = [-1,1] → [-6000, 2400]
    let tx_base = arena_center_x(c) + x_liss * BOSS_ARENA_HALF_W * 0.95;
    let y_min = -6000.0;
    let y_max = 2400.0;
    let y_center = (y_min + y_max) * 0.5;
    let y_half_range = (y_max - y_min) * 0.5;
    let ty_base = y_center + y_liss * y_half_range * 0.92;

    // ── Player proximity avoidance: dynamic steering away when threatened ──
    let pdx = cur_x - px;
    let pdy = cur_y - py;
    let player_dist2 = pdx * pdx + pdy * pdy;
    let danger_radius = 1200.0; // activation distance
    let danger_radius2 = danger_radius * danger_radius;

    let (tx_final, ty_final) = if player_dist2 < danger_radius2 {
        // Player too close: steer boss away with proportional force
        let threat_factor = (1.0 - (player_dist2.sqrt() / danger_radius)).max(0.0).powi(2);
        let threat_mul = 0.7 * threat_factor; // max 70% steering influence
        
        // Escape direction: away from player
        let escape_dist = player_dist2.sqrt().max(1.0);
        let escape_dx = pdx / escape_dist;
        let escape_dy = pdy / escape_dist;
        
        // Guide to opposite corner/edge
        let opposite_corner_x = if escape_dx > 0.0 {
            zx1 + BOSS_SIZE * 0.5
        } else {
            zx2 - BOSS_SIZE * 0.5
        };
        let opposite_corner_y = if escape_dy > 0.0 {
            y_min
        } else {
            y_max
        };
        
        // Blend base pattern with escape direction
        let tx_escape = opposite_corner_x * threat_mul + tx_base * (1.0 - threat_mul);
        let ty_escape = opposite_corner_y * threat_mul + ty_base * (1.0 - threat_mul);
        (tx_escape, ty_escape)
    } else {
        (tx_base, ty_base)
    };

    let dx = tx_final - cur_x;
    let dy = ty_final - cur_y;
    let d = (dx * dx + dy * dy).sqrt().max(1.0);
    
    // Smooth speed modulation based on phase — faster in straights, slower at turns
    let speed_mod = 0.3 + 0.7 * (phase * 0.5).cos().abs();
    let seek_speed = 22.0 * speed_mod; // smooth variable speed
    let desired_vx = dx / d * seek_speed;
    let desired_vy = dy / d * seek_speed;

    // Very smooth velocity transitions to eliminate jerky direction changes
    s.boss_vx += (desired_vx - s.boss_vx) * 0.18;
    s.boss_vy += (desired_vy - s.boss_vy) * 0.18;

    let max_speed = 38.0; // moderate speed for smooth curves
    let sp = (s.boss_vx * s.boss_vx + s.boss_vy * s.boss_vy).sqrt();
    if sp > max_speed {
        let k = max_speed / sp;
        s.boss_vx *= k;
        s.boss_vy *= k;
    }

    let mut nx = cur_x + s.boss_vx;
    let mut ny = cur_y + s.boss_vy;

    let x_min = zx1 + BOSS_SIZE * 0.5;
    let x_max = zx2 - BOSS_SIZE * 0.5;
    if nx < x_min {
        nx = x_min;
        s.boss_vx = s.boss_vx.abs() * 0.65;
    } else if nx > x_max {
        nx = x_max;
        s.boss_vx = -s.boss_vx.abs() * 0.65;
    }

    let boundary_y_min = -6000.0;
    let boundary_y_max = 2400.0;
    if ny < boundary_y_min {
        ny = boundary_y_min;
        s.boss_vy = s.boss_vy.abs() * 0.75;
    } else if ny > boundary_y_max {
        ny = boundary_y_max;
        s.boss_vy = -s.boss_vy.abs() * 0.9;
    }
    drop(s);

    if let Some(obj) = c.get_game_object_mut("boss") {
        obj.position = (nx - BOSS_SIZE * 0.5, ny - BOSS_SIZE * 0.5);
    }
}

// ── Drift boss arena asteroids ────────────────────────────────────────────────

fn tick_boss_asteroid_drift(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (zx1, zx2) = arena_bounds(c);
    let s = st.lock().unwrap();
    if !s.boss_active { return; }
    let ids = s.boss_asteroids.clone();
    drop(s);

    // The engine applies momentum to position automatically.
    // This function only bounces asteroids off the arena boundaries.
    let y_min = -3500.0;
    let y_max =  1500.0;

    for id in &ids {
        if let Some(obj) = c.get_game_object_mut(id) {
            // Bounce off arena X walls.
            if obj.position.0 < zx1 {
                obj.momentum.0 = obj.momentum.0.abs();
                obj.position.0 = zx1;
            } else if obj.position.0 + obj.size.0 > zx2 {
                obj.momentum.0 = -obj.momentum.0.abs();
                obj.position.0 = zx2 - obj.size.0;
            }
            // Bounce off Y limits.
            if obj.position.1 < y_min {
                obj.momentum.1 = obj.momentum.1.abs();
                obj.position.1 = y_min;
            } else if obj.position.1 + obj.size.1 > y_max {
                obj.momentum.1 = -obj.momentum.1.abs();
                obj.position.1 = y_max - obj.size.1;
            }
        }
    }
}

// ── Boss shoots bolts at player ───────────────────────────────────────────────

fn tick_boss_shooting(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 { return; }

    if s.boss_shoot_timer > 0 {
        s.boss_shoot_timer -= 1;
        return;
    }
    s.boss_shoot_timer = BOSS_SHOOT_INTERVAL;

    let bolt_id = match s.boss_bolt_free.pop() {
        Some(id) => id,
        None => return,
    };

    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-9999.0, -9999.0));
    let boss_cx = boss_pos.0 + BOSS_SIZE * 0.5;
    let boss_cy = boss_pos.1 + BOSS_SIZE * 0.5;

    let px = s.px;
    let py = s.py;
    let dx = px - boss_cx;
    let dy = py - boss_cy;
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let vx = dx / len * BOSS_BOLT_SPEED;
    let vy = dy / len * BOSS_BOLT_SPEED;

    s.boss_bolt_live.push((bolt_id.clone(), vx, vy, BOSS_BOLT_LIFETIME));
    drop(s);

    if let Some(obj) = c.get_game_object_mut(&bolt_id) {
        obj.position = (boss_cx - BOSS_BOLT_W * 0.5, boss_cy - BOSS_BOLT_H * 0.5);
        obj.visible = true;
    }
}

// ── Move boss bolts ───────────────────────────────────────────────────────────

fn tick_boss_bolts(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let recycle: Vec<String>;
    let move_list: Vec<(String, f32, f32)>;
    {
        let mut s = st.lock().unwrap();
        if !s.boss_active { return; }

        let mut rc: Vec<String> = Vec::new();
        let mut mv: Vec<(String, f32, f32)> = Vec::new();
        for (name, vx, vy, ttl) in &mut s.boss_bolt_live {
            mv.push((name.clone(), *vx, *vy));
            if *ttl > 0 { *ttl -= 1; }
            if *ttl == 0 { rc.push(name.clone()); }
        }
        for id in &rc {
            s.boss_bolt_live.retain(|(n, _, _, _)| n != id);
            s.boss_bolt_free.push(id.clone());
        }
        recycle = rc;
        move_list = mv;
    }

    for (name, vx, vy) in &move_list {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.0 += vx;
            obj.position.1 += vy;
        }
    }

    for id in &recycle {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-7000.0, -7000.0);
        }
    }
}

// ── Boss bolt hits player ─────────────────────────────────────────────────────

fn tick_boss_bolt_player_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || s.dead { return; }

    let px = s.px;
    let py = s.py;
    let hit_r = PLAYER_R + BOSS_BOLT_W.max(BOSS_BOLT_H) * 0.5 + 4.0;

    let live_snapshot: Vec<String> = s.boss_bolt_live.iter().map(|(n, _, _, _)| n.clone()).collect();
    let mut hit_ids: Vec<String> = Vec::new();

    for name in &live_snapshot {
        if let Some(obj) = c.get_game_object(name) {
            let bcx = obj.position.0 + BOSS_BOLT_W * 0.5;
            let bcy = obj.position.1 + BOSS_BOLT_H * 0.5;
            let dx = px - bcx;
            let dy = py - bcy;
            if dx * dx + dy * dy < hit_r * hit_r {
                hit_ids.push(name.clone());
            }
        }
    }

    if hit_ids.is_empty() { return; }

    for id in &hit_ids {
        s.boss_bolt_live.retain(|(n, _, _, _)| n != id);
        s.boss_bolt_free.push(id.clone());
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-7000.0, -7000.0);
        }
    }

    // A boss bolt normally breaks the tether and costs a heart. While buffed,
    // the buff absorbs up to BUFF_ABSORB_MAX hits with no damage; after the last
    // one it ends early. Refreshing the buff resets the absorb count.
    let absorb = s.player_buff > 0 && s.buff_absorbs > 0;
    if absorb {
        s.buff_absorbs -= 1;
        s.buff_hit_flash = 6;
        if s.buff_absorbs == 0 {
            s.player_buff = 0;
            s.buff_timer = 0;
        }
        drop(s);
        // Brief cyan flash so the absorb reads clearly.
        if let Some(cam) = c.camera_mut() {
            cam.flash_with(Color(110, 230, 255, 200), 0.25, FlashMode::Pulse, FlashEase::Sharp, 0.7, 0.0);
        }
        return;
    }

    // A boss bolt breaks the tether and costs a heart.
    let hooked = s.hooked;
    if hooked {
        s.hooked = false;
        s.active_hook = String::new();
    }
    drop(s);
    if hooked {
        c.run(Action::Hide { target: Target::name("rope") });
    }
    let _over = super::hearts::lose_heart(c, st);
    // Restore light boss gravity after the hit.
    if let Some(obj) = c.get_game_object_mut("player") {
        let gdir = st.lock().unwrap().gravity_dir;
        obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE * gdir;
    }
}

// ── Player hits boss body ─────────────────────────────────────────────────────

fn tick_boss_player_hits_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active || !s.boss_spawned || s.boss_hp <= 0 { return; }

    let px = s.px;
    let py = s.py;

    let boss_pos = c.get_game_object("boss").map(|o| o.position);
    let Some(bpos) = boss_pos else { return };

    let bcx = bpos.0 + BOSS_SIZE * 0.5;
    let bcy = bpos.1 + BOSS_SIZE * 0.5;
    let hit_r = PLAYER_R + BOSS_SIZE * 0.5;

    let dx = px - bcx;
    let dy = py - bcy;
    if dx * dx + dy * dy >= hit_r * hit_r { return; }

    // ── Buffed weakpoint hit damages the boss; unprotected body contact ─────
    // knocks the player back and disconnects the tether (no boss damage).
    let buffed = s.player_buff > 0;
    let near_weakpoint = BOSS_WEAKPOINT_OFFSETS.iter().any(|(wx, wy)| {
        let wx = bcx + wx;
        let wy = bcy + wy;
        let ddx = px - wx;
        let ddy = py - wy;
        ddx * ddx + ddy * ddy < BOSS_WEAKPOINT_R * BOSS_WEAKPOINT_R
    });

    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let nx = dx / len;
    let ny = dy / len;

    // Boss is forcefield-protected until ALL generators are destroyed.
    // (A debug var can force it down for headless weakpoint validation.)
    let forcefield_debug = matches!(c.get_var("debug_boss_forcefield_down"), Some(Value::Bool(true)));
    let generators_down = forcefield_debug
        || (!s.boss_generator_hp.is_empty() && s.boss_generator_hp.iter().all(|&hp| hp <= 0));

    let mut contact_damage = false;
    let (nwx, nwy, did_unhook) = if buffed && near_weakpoint && generators_down {
        // Buffed weakpoint hit with the forcefield down: damage the boss.
        s.boss_hp -= 1;
        s.buff_hit_flash = 20;
        (nx * 26.0, ny * 26.0, false)
    } else {
        // Unbuffed contact with the boss body COSTS a heart, it does not merely
        // bounce. Touching the thing you are fighting has to be a mistake, or
        // the buff is only a damage tool and never a defensive decision — and
        // the fight degenerates into riding the boss for free.
        let unhook = s.hooked;
        if unhook {
            s.hooked = false;
            s.active_hook = String::new();
        }
        if !buffed {
            contact_damage = true;
        }
        (nx * 34.0, ny * 34.0, unhook)
    };
    s.vx = nwx;
    s.vy = nwy;

    let hp = s.boss_hp;
    let asteroid_ids = s.boss_asteroids.clone();
    drop(s);

    if let Some(obj) = c.get_game_object_mut("player") {
        obj.momentum.0 = nwx;
        obj.momentum.1 = nwy;
    }
    if did_unhook {
        c.run(Action::Hide { target: Target::name("rope") });
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE;
        }
    }

    if contact_damage {
        // Applied after the State lock is dropped — `lose_heart` takes it too.
        super::hearts::lose_heart(c, st);
    }

    if hp <= 0 {
        // Recycle the objects the fight owns, then hand off to the real victory
        // flow.
        //
        // This used to be an inline copy of the end-of-fight teardown, and it
        // had drifted from `finish_boss`: it awarded no meta currency, showed no
        // victory stasis, and never advanced `boss_index` — so with the sun-line
        // finisher removed, killing the boss by weakpoint damage (the only way
        // now, and the intended one) would have paid nothing and re-armed the
        // SAME fight at the same threshold.
        let asteroid_ids2 = asteroid_ids.clone();
        {
            let mut s2 = st.lock().unwrap();
            let live: Vec<String> = s2.boss_bolt_live.iter().map(|(n, _, _, _)| n.clone()).collect();
            for id in &live {
                s2.boss_bolt_live.retain(|(n, _, _, _)| n != id);
                s2.boss_bolt_free.push(id.clone());
            }
            s2.space_asteroid_live.retain(|id| !asteroid_ids2.contains(id));
            s2.boss_dark_active = false;
            s2.boss_dark_ticks = 0;
            s2.boss_dark_cooldown = BOSS_DARK_INTERVAL;
            drop(s2);
            for id in &live {
                if let Some(obj) = c.get_game_object_mut(id) {
                    obj.visible = false;
                    obj.position = (-7000.0, -7000.0);
                }
            }
        }
        for id in &asteroid_ids {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.visible = false;
                obj.position = (-8000.0, -8000.0);
                obj.momentum = (0.0, 0.0);
            }
        }
        // Restore player gravity (the arena runs at a reduced scale).
        if let Some(obj) = c.get_game_object_mut("player") {
            let cur = obj.gravity;
            let sign = if cur < 0.0 { -1.0_f32 } else { 1.0_f32 };
            obj.gravity = sign * GRAVITY;
        }

        // Meta reward, victory stasis, and — via `complete_boss_finish` when the
        // player tethers out — the frontier rewind and the schedule advance that
        // arms the next fight.
        finish_boss(c, st);
    }
}

// ── Boss HP bar HUD update ────────────────────────────────────────────────────

fn tick_boss_hud(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }

    let hp    = s.boss_hp;
    c.set_var("boss_hp", Value::I32(hp));
    let dirty = hp != s.hud_last_boss_hp;
    if !dirty { return; }
    s.hud_last_boss_hp = hp;
    drop(s);

    let fill = (hp as f32 / BOSS_MAX_HP as f32).clamp(0.0, 1.0);
    let w = BOSS_HP_BAR_W as u32;
    let h = BOSS_HP_BAR_H as u32;
    let fill_px = (fill * w as f32).round() as u32;

    let mut img = image::RgbaImage::new(w, h);
    for row in 0..h {
        for col in 0..w {
            let color = if col < fill_px {
                image::Rgba([C_BOSS_HP_FILL.0, C_BOSS_HP_FILL.1, C_BOSS_HP_FILL.2, 255])
            } else {
                image::Rgba([C_BOSS_HP_BG.0, C_BOSS_HP_BG.1, C_BOSS_HP_BG.2, 200])
            };
            img.put_pixel(col, row, color);
        }
    }

    if let Some(obj) = c.get_game_object_mut("boss_hp_bar") {
        obj.set_image(Image { shape: ShapeType::Rectangle(0.0, (BOSS_HP_BAR_W, BOSS_HP_BAR_H), 0.0), image: img.into(), color: None });
        obj.visible = fill > 0.0;
    }
}

// ── Off-screen objective indicators ──────────────────────────────────────────

/// Place a HUD arrow at the screen edge pointing at a world position. If the
/// world position is on screen the arrow is hidden. `cleft`/`ctop`/`zoom` are
/// the camera's world-space top-left and zoom.
fn place_off_arrow(c: &mut Canvas, id: &str, wx: f32, wy: f32, cleft: f32, ctop: f32, zoom: f32) {
    let sx = (wx - cleft) * zoom;
    let sy = (wy - ctop) * zoom;
    let on_screen = sx >= 0.0 && sx <= VW && sy >= 0.0 && sy <= VH;
    let Some(obj) = c.get_game_object_mut(id) else { return; };
    if on_screen {
        obj.visible = false;
        return;
    }
    // Aim from screen centre toward the target.
    let dx = sx - VW * 0.5;
    let dy = sy - VH * 0.5;
    obj.rotation = dy.atan2(dx).to_degrees();
    // Clamp to the viewport rim so the arrow sits at the edge.
    let margin = 90.0;
    obj.position = (sx.clamp(margin, VW - margin), sy.clamp(margin, VH - margin));
    obj.visible = true;
}

/// Off-screen indicators during the fight: the boss-name banner, an edge arrow
/// pointing at the boss, and (only while the player is buffed) edge arrows
/// pointing at any off-screen generators that still have HP.
fn tick_boss_indicators(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (active, spawned, buff, gens_down) = {
        let s = st.lock().unwrap();
        let gens_down = !s.boss_generator_hp.is_empty() && s.boss_generator_hp.iter().all(|&hp| hp <= 0);
        (s.boss_active, s.boss_spawned, s.player_buff, gens_down)
    };

    // Hide everything when the fight is not running.
    if !active || !spawned {
        if let Some(obj) = c.get_game_object_mut("boss_name_text") { obj.visible = false; }
        if let Some(obj) = c.get_game_object_mut("boss_off_arrow") { obj.visible = false; }
        for i in 0..BOSS_GENERATOR_COUNT {
            if let Some(obj) = c.get_game_object_mut(&format!("gen_arrow_{i}")) { obj.visible = false; }
        }
        return;
    }

    let (cleft, ctop, zoom) = c.camera()
        .map(|cam| (cam.position.0, cam.position.1, cam.zoom.max(0.1)))
        .unwrap_or((0.0, 0.0, 1.0));

    // Boss-name banner: visible for the length of the fight.
    if let Some(obj) = c.get_game_object_mut("boss_name_text") { obj.visible = true; }

    // Arrow pointing at the boss when it is off-screen.
    if let Some(boss) = c.get_game_object("boss") {
        let bx = boss.position.0 + BOSS_SIZE * 0.5;
        let by = boss.position.1 + BOSS_SIZE * 0.5;
        place_off_arrow(c, "boss_off_arrow", bx, by, cleft, ctop, zoom);
    } else {
        if let Some(obj) = c.get_game_object_mut("boss_off_arrow") { obj.visible = false; }
    }

    // Generator objective arrows, only while the player is buffed and any
    // generator is still alive to destroy.
    let show_gens = buff > 0 && !gens_down;
    for i in 0..BOSS_GENERATOR_COUNT {
        let id = format!("gen_arrow_{i}");
        if show_gens {
            if let Some(gen) = c.get_game_object(&format!("boss_gen_{i}")) {
                let gw = BOSS_GENERATOR_R * 2.0;
                place_off_arrow(c, &id, gen.position.0 + gw * 0.5, gen.position.1 + gw * 0.5, cleft, ctop, zoom);
            } else if let Some(obj) = c.get_game_object_mut(&id) {
                obj.visible = false;
            }
        } else if let Some(obj) = c.get_game_object_mut(&id) {
            obj.visible = false;
        }
    }
}
