//! difficulty.rs — the single authority for "how far into the run are we".
//!
//! Everything that is supposed to get harder with distance reads `t` from here
//! instead of dividing `distance` by its own private constant. Before this
//! module existed there were two competing ramps — a 30 000 px hook ramp and a
//! 20 000 px zone step — both of which a player crosses in well under a minute,
//! so the whole game sat at maximum difficulty almost immediately.
//!
//! ── Calibration ─────────────────────────────────────────────────────────────
//! The curve is expressed in world pixels because that is what the generator
//! sees, but it is *authored* in minutes. `ASSUMED_PLAYER_PX_PER_SEC` converts
//! between the two, and it is the one number to retune after a real playtest:
//! time a strong run, divide `distance` by the seconds elapsed, and put the
//! answer here. Nothing else needs to change.

#![allow(dead_code)]

/// Net forward speed (px/second) assumed for a strong player, averaged over a
/// whole run including arcs that briefly travel backwards.
///
/// Reference points: `MOMENTUM_CAP` is 50 px/tick (3000 px/s) at the peak of a
/// swing, and a clean hop chain nets roughly one stride (~640 px) per arc of
/// ~24 ticks, i.e. ~1600 px/s while chaining perfectly. 800 px/s assumes a
/// strong player spends about half the run at that rate and the rest
/// recovering, re-aiming, or handling hazards.
pub const ASSUMED_PLAYER_PX_PER_SEC: f32 = 800.0;

/// Minutes of continuous play a perfect run takes to reach peak difficulty.
/// Boss fights are deliberately excluded — they interrupt the run rather than
/// advancing `distance`, so adding them later does not shift this curve.
pub const DIFFICULTY_FULL_MINUTES: f32 = 60.0;

/// Minutes at the very start that stay at the floor of the curve, so the
/// opening of a run is always the same gentle teaching stretch.
pub const DIFFICULTY_GRACE_MINUTES: f32 = 2.0;

/// One minute of forward progress, in world px.
pub const DIFFICULTY_PX_PER_MINUTE: f32 = ASSUMED_PLAYER_PX_PER_SEC * 60.0;

/// Distance at which the curve reaches 1.0 (≈ 2.88 M px at the defaults).
pub const DIFFICULTY_FULL_DISTANCE: f32 = DIFFICULTY_PX_PER_MINUTE * DIFFICULTY_FULL_MINUTES;

/// Distance before the curve starts moving at all.
pub const DIFFICULTY_GRACE_DISTANCE: f32 = DIFFICULTY_PX_PER_MINUTE * DIFFICULTY_GRACE_MINUTES;

/// Distance one visual zone lasts before cycling to the next. Zones are
/// *texture*, not difficulty — they cycle repeatedly through the run so the
/// backdrop keeps changing, while `difficulty_t` climbs monotonically.
pub const ZONE_CYCLE_MINUTES: f32 = 4.0;
pub const ZONE_CYCLE_DISTANCE: f32 = DIFFICULTY_PX_PER_MINUTE * ZONE_CYCLE_MINUTES;

/// Number of distinct zone looks (normal → purple → black → repeat).
pub const ZONE_COUNT: usize = 3;

/// Progress through the run as 0.0 → 1.0.
///
/// Smoothstep, not linear: the first and last few minutes change slowly and the
/// middle carries most of the ramp. That keeps the opening forgiving and stops
/// the last stretch from spiking into something qualitatively different from
/// the minute before it.
pub fn difficulty_t(distance: f32) -> f32 {
    let span = (DIFFICULTY_FULL_DISTANCE - DIFFICULTY_GRACE_DISTANCE).max(1.0);
    let u = ((distance - DIFFICULTY_GRACE_DISTANCE) / span).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// Which of the `ZONE_COUNT` looks is active. Cycles forever — the previous
/// implementation clamped to the last zone after 40 000 px (~50 s) and the
/// backdrop never changed again for the rest of the run.
pub fn zone_index_for_distance(distance: f32) -> usize {
    if distance <= 0.0 {
        return 0;
    }
    ((distance / ZONE_CYCLE_DISTANCE) as usize) % ZONE_COUNT
}

/// How many times the zone look has cycled. Useful for effects that should
/// escalate on each lap rather than reset with the zone index.
pub fn zone_lap(distance: f32) -> usize {
    if distance <= 0.0 {
        return 0;
    }
    (distance / ZONE_CYCLE_DISTANCE) as usize / ZONE_COUNT
}

/// Interpolate `easy → hard` by the curve. The one call every difficulty-scaled
/// value should go through, so the shape of the ramp lives in exactly one place.
#[inline]
pub fn ramp(distance: f32, easy: f32, hard: f32) -> f32 {
    easy + (hard - easy) * difficulty_t(distance)
}

/// Scale a hazard's spawn gap by the curve.
///
/// Hazard *density* is where most of the "it doesn't get harder" lived: every
/// gap was a fixed constant, so an hour-long run met the same spinner spacing
/// in minute 58 as in minute 2. Gaps shrink toward `HAZARD_GAP_HARD_SCALE` of
/// their authored value, which tightens density without ever letting a gap
/// collapse — the floor is what keeps late game dense rather than impassable.
pub const HAZARD_GAP_HARD_SCALE: f32 = 0.55;

#[inline]
pub fn hazard_gap(distance: f32, gap: f32) -> f32 {
    gap * ramp(distance, 1.0, HAZARD_GAP_HARD_SCALE)
}

/// Scale a hazard gap RANGE, preserving the authored min/max ordering.
#[inline]
pub fn hazard_gap_range(distance: f32, lo: f32, hi: f32) -> (f32, f32) {
    let k = ramp(distance, 1.0, HAZARD_GAP_HARD_SCALE);
    (lo * k, hi * k)
}
