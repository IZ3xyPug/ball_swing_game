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

const PATTERN_KINDS: usize = 7;

/// The run of nodes being generated: its shape, where it is in it, how long
/// it is, the height it started from, and its spacing.
#[derive(Default)]
struct Pattern { kind: usize, step: usize, len: usize, anchor: f32, dx: f32 }

thread_local! {
    static PATTERN: std::cell::RefCell<Pattern> = std::cell::RefCell::new(Pattern::default());
}

/// Start the pattern sequence afresh (a new run, or a regenerated stretch).
pub fn reset_patterns() {
    PATTERN.with(|p| *p.borrow_mut() = Pattern::default());
}

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

    // ── Pattern ──────────────────────────────────────────────────────────────
    // Nodes come in runs of 4-8 that follow one shape at one steady spacing
    // (a wave, a climb, an arch...), so the line reads as designed rather
    // than random, then the next run picks another. The shape proposes a
    // height; the envelope decides how much of it this hop may take.
    let (kind, step, len, anchor, dx_run) = PATTERN.with(|p| {
        let mut p = p.borrow_mut();
        if p.step >= p.len {
            // Not the same shape twice running.
            let mut k = (lcg_range(seed, 0.0, PATTERN_KINDS as f32) as usize).min(PATTERN_KINDS - 1);
            if k == p.kind { k = (k + 1) % PATTERN_KINDS; }
            p.kind = k;
            p.step = 0;
            p.len = lcg_range(seed, 4.0, 8.99) as usize;
            p.anchor = *head_y;
            p.dx = dx;
        }
        let out = (p.kind, p.step, p.len, p.anchor, p.dx);
        p.step += 1;
        out
    });
    // One spacing for the whole run: its rhythm.
    let dx = dx_run.clamp(lo.min(hi), hi.max(lo));
    let (up_budget, down_budget) = hop_dy_budget(dx);
    let up = up_budget * vert_frac * HOP_REACH_MARGIN;
    let down = down_budget * vert_frac * HOP_REACH_MARGIN;
    let k = step as f32 / (len.max(2) - 1) as f32;          // 0..1 along the run
    let amp = (HOOK_Y_MAX - HOOK_Y_MIN) * 0.30 * (0.6 + 0.4 * vert_frac);
    let mid = (HOOK_Y_MIN + HOOK_Y_MAX) * 0.5;
    // Where the shape wants this node (world y; up is negative).
    // Each run drifts back toward the middle of the band, so the line never
    // parks against the top or the bottom; a climb that starts high turns
    // into a descent (and the other way round), so it has room.
    let anchor = anchor + (mid - anchor) * 0.45 * k;
    let room_up = anchor - HOOK_Y_MIN;
    let room_down = HOOK_Y_MAX - anchor;
    let kind = match kind {
        1 if room_up < amp * 1.2 => 2,
        2 if room_down < amp * 1.2 => 1,
        3 if room_up < amp => 4,
        4 if room_down < amp => 3,
        k => k,
    };
    let target = match kind {
        0 => anchor + amp * (k * std::f32::consts::TAU).sin(),            // wave
        1 => anchor - amp * 1.4 * k,                                       // climb
        2 => anchor + amp * 1.4 * k,                                       // descent
        3 => anchor - amp * (k * std::f32::consts::PI).sin() * 1.3,        // arch
        4 => anchor + amp * (k * std::f32::consts::PI).sin() * 1.3,        // valley
        5 => anchor + (mid - anchor) * 0.15,                               // glide (level)
        _ => anchor + if step % 2 == 0 { -amp * 0.8 } else { amp * 0.8 },  // zigzag
    };
    // Keep the shape on the playable band, then inside this hop's envelope.
    let target = target.clamp(HOOK_Y_MIN + 40.0, HOOK_Y_MAX - 40.0);
    let dy = (target - *head_y).clamp(-up, down);
    *head_x += dx;
    // Clamping to the playable band can only pull the node back toward
    // `head_y`, which is itself always in-band — so it can shorten a hop but
    // never lengthen one past the envelope.
    *head_y = (*head_y + dy).clamp(HOOK_Y_MIN, HOOK_Y_MAX);

    HookSpec { x: *head_x, y: *head_y }
}

#[cfg(test)]
mod pattern_tests {
    use super::*;

    /// `HOOK_PLOT=1`: print a stretch of generated nodes (x y), to look at
    /// the patterns.
    #[test]
    fn plot() {
        if std::env::var("HOOK_PLOT").is_err() { return; }
        reset_patterns();
        let (mut seed, mut x, mut y) = (0x5EED_1234u64 ^ (std::env::var("HOOK_PLOT_SEED").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0)), 0.0f32, 300.0f32);
        let d = std::env::var("HOOK_PLOT_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        for _ in 0..60 {
            let h = generate_next_hook(&mut seed, &mut x, &mut y, d);
            println!("NODE {} {}", h.x, h.y);
        }
    }
}
