//! level_gen.rs — hop-based grab-node generator.
//!
//! Every grab node is one "hop" from the previous node, and every hop is
//! required to land inside the reach envelope described in `constants.rs`
//! (see "Generation — Rope Reach Rules"). The generator is only half of that
//! guarantee: `spawning::spawn_hooks` moves nodes in Y to dodge spinners, pads
//! and gravity wells, so it re-clamps against the same envelope using the
//! helpers below. Both sides share `hop_dy_budget`, so there is exactly one
//! definition of "reachable" in the codebase.
//!
//! ── Tuning knobs ────────────────────────────────────────────────────────────
//!   constants.rs  HOP_REACH_X / _UP / _DOWN     shape of the envelope
//!   constants.rs  HOOK_STRIDE_FRAC_*            stride, as a fraction of it
//!   constants.rs  HOOK_VERT_FRAC_*              how much vertical budget to use
//!   difficulty.rs ASSUMED_PLAYER_PX_PER_SEC     how long the ramp takes
//!   constants.rs  HOOK_Y_MIN / HOOK_Y_MAX       world Y band for grab nodes

#![allow(dead_code)]

use crate::constants::*;
use crate::difficulty::difficulty_t;
use crate::state::{HookSpec, lcg_range};

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ── Hop envelope ─────────────────────────────────────────────────────────────

/// Vertical budget `(up, down)` available at a horizontal offset of `dx`.
///
/// Y grows downward in world space, so `up` bounds a negative `dy` and `down`
/// bounds a positive one. Both collapse to 0 as `dx` approaches `HOP_REACH_X`.
#[inline]
pub fn hop_dy_budget(dx: f32) -> (f32, f32) {
    let fx = (dx.abs() / HOP_REACH_X).clamp(0.0, 1.0);
    let k = (1.0 - fx * fx).max(0.0).sqrt();
    (HOP_REACH_UP * k, HOP_REACH_DOWN * k)
}

/// True when a hop of `(dx, dy)` is inside the envelope.
#[inline]
pub fn hop_is_reachable(dx: f32, dy: f32) -> bool {
    if dx.abs() > HOP_REACH_X {
        return false;
    }
    let (up, down) = hop_dy_budget(dx);
    if dy < 0.0 { -dy <= up } else { dy <= down }
}

/// Clamp `y` so the hop from `(prev_x, prev_y)` to `(x, y)` is inside the
/// envelope, with `HOP_REACH_MARGIN` of headroom against float error.
///
/// This is the backstop the spawner calls after hazard avoidance. It only ever
/// pulls a node *toward* the previous node's height, so it cannot introduce a
/// new overlap with something the avoidance pass had already cleared in the
/// opposite direction — it can only undo an over-correction.
#[inline]
pub fn clamp_into_envelope(prev_x: f32, prev_y: f32, x: f32, y: f32) -> f32 {
    let (up, down) = hop_dy_budget(x - prev_x);
    let lo = prev_y - up * HOP_REACH_MARGIN;
    let hi = prev_y + down * HOP_REACH_MARGIN;
    y.clamp(lo, hi)
}

/// Largest `x` that is still reachable from `prev_x`, used when avoidance wants
/// to move a node horizontally instead of vertically. Pulling a node back in X
/// *widens* its vertical budget, which is usually the cheaper way out of a
/// blocked placement.
#[inline]
pub fn max_reachable_x(prev_x: f32) -> f32 {
    prev_x + HOP_REACH_X * HOP_REACH_MARGIN
}

// ── Starter layout ───────────────────────────────────────────────────────────

pub const STARTER_HOOK_COUNT: usize = 6;

/// The fixed opening sequence, shared by `bootstrap.rs` (which creates the
/// objects) and `build_scene.rs` (which seeds `State` from it). These used to
/// be two copy-pasted tables with a "must match bootstrap.rs" comment between
/// them.
///
/// The old table stepped 1250 px per node — 74% past a full rope length — and
/// put two of its four nodes below `HOOK_Y_MAX`, so the first thing a player
/// ever saw was the least reachable layout in the game. Every headless episode
/// died inside it. This sequence uses easy-end strides and stays in the band.
pub fn starter_hooks() -> [(f32, f32); STARTER_HOOK_COUNT] {
    const STEP: f32 = HOP_REACH_X * 0.64; // ~461 px — the easy end of the ramp
    [
        (START_HOOK_X,             START_HOOK_Y),
        (START_HOOK_X + STEP,      START_HOOK_Y - 100.0),
        (START_HOOK_X + STEP * 2.0, START_HOOK_Y + 30.0),
        (START_HOOK_X + STEP * 3.0, START_HOOK_Y - 150.0),
        (START_HOOK_X + STEP * 4.0, START_HOOK_Y + 70.0),
        (START_HOOK_X + STEP * 5.0, START_HOOK_Y - 70.0),
    ]
}

// ── Generator ────────────────────────────────────────────────────────────────

/// Generate the next grab node as a single hop from `(head_x, head_y)`,
/// advancing the head to the new position.
///
/// `distance_px` drives the difficulty curve: stride grows from ~58% to ~97% of
/// the horizontal reach, and the share of the vertical budget a hop may spend
/// grows from 45% to 100%, over `difficulty::DIFFICULTY_FULL_DISTANCE`.
pub fn generate_next_hook(
    seed: &mut u64,
    head_x: &mut f32,
    head_y: &mut f32,
    distance_px: f32,
) -> HookSpec {
    let t = difficulty_t(distance_px);

    // ── Horizontal stride ────────────────────────────────────────────────────
    // Both ends are interpolated, so `lo <= hi` holds for every t. The previous
    // implementation added a flat bonus to the minimum and then clamped only
    // the maximum, so past a certain distance the minimum overtook the maximum
    // and `lcg_range` ran with its bounds inverted.
    let lo = lerp(HOOK_STRIDE_FRAC_EASY_MIN, HOOK_STRIDE_FRAC_HARD_MIN, t) * HOP_REACH_X;
    let hi = lerp(HOOK_STRIDE_FRAC_EASY_MAX, HOOK_STRIDE_FRAC_HARD_MAX, t) * HOP_REACH_X;
    let dx = lcg_range(seed, lo.min(hi), hi.max(lo));

    // ── Vertical offset ──────────────────────────────────────────────────────
    // The budget is whatever the envelope allows at this dx, scaled by how far
    // into the run we are. Direction is picked first so the magnitude is drawn
    // against the budget that direction actually has — up and down differ.
    let vert_frac = lerp(HOOK_VERT_FRAC_EASY, HOOK_VERT_FRAC_HARD, t);
    let (up_budget, down_budget) = hop_dy_budget(dx);
    // HOP_REACH_MARGIN keeps a proposal off the exact boundary. Without it the
    // top of the range lands on the envelope edge, and the strict check that
    // guards this (in tests, and in the spawner's backstop) sees a hop fail by
    // a fraction of a pixel of float error.
    let up = up_budget * vert_frac * HOP_REACH_MARGIN;
    let down = down_budget * vert_frac * HOP_REACH_MARGIN;

    let mut go_down = lcg_range(seed, 0.0, 1.0) < 0.5;
    // Anti-stacking: consecutive nodes should differ vertically by at least
    // HOOK_CLOSE_Y_THRESHOLD where the budget can afford it. If the chosen
    // direction cannot, take the other one rather than flattening the hop.
    let min_dy = HOOK_CLOSE_Y_THRESHOLD.min(up.max(down));
    if (if go_down { down } else { up }) < min_dy {
        go_down = !go_down;
    }
    let budget = if go_down { down } else { up };
    let dy_mag = lcg_range(seed, min_dy.min(budget), budget);
    let dy = if go_down { dy_mag } else { -dy_mag };

    *head_x += dx;
    // Clamping to the playable band can only pull the node back toward
    // `head_y`, which is itself always in-band — so it can shorten a hop but
    // never lengthen one past the envelope.
    *head_y = (*head_y + dy).clamp(HOOK_Y_MIN, HOOK_Y_MAX);

    HookSpec { x: *head_x, y: *head_y }
}
