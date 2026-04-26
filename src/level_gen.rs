//! level_gen.rs — Hop-based level generator
//!
//! Every grab node is one "hop" from the previous node.
//! Rope-reach rules can be intentionally relaxed by setting large
//! HOOK_X_STRIDE_* values in constants.rs.
//!
//! ── Tuning knobs (all in constants.rs) ──────────────────────────────────────
//!   HOOK_X_STRIDE_MIN / HOOK_X_STRIDE_MAX   horizontal step range per hop
//!   HOOK_CLOSE_Y_THRESHOLD                  enforced as minimum |ΔY| per hop
//!   HOOK_Y_MIN / HOOK_Y_MAX                 world Y bounds for grab nodes

#![allow(dead_code)]

use crate::state::{HookSpec, lcg_range};
use crate::constants::*;

/// Generate the next hook as a single hop from (head_x, head_y).
///
/// Updates head_x and head_y to the new position before returning.
pub fn generate_next_hook(
    seed: &mut u64,
    head_x: &mut f32,
    head_y: &mut f32,
    _distance_px: f32,
) -> HookSpec {
    // Horizontal step.
    let dx = lcg_range(seed, HOOK_X_STRIDE_MIN, HOOK_X_STRIDE_MAX);

    // If stride is within rope reach, keep the old Pythagorean dy budget.
    // If stride exceeds rope reach (intentional long-gap mode), use a fixed
    // dy window so hooks still vary vertically instead of flattening out.
    let (min_dy, max_dy) = if dx <= HOOK_MAX_REACH {
        let max_dy = (HOOK_MAX_REACH * HOOK_MAX_REACH - dx * dx).sqrt().max(0.0);
        (HOOK_CLOSE_Y_THRESHOLD.min(max_dy), max_dy)
    } else {
        (HOOK_CLOSE_Y_THRESHOLD, HOOK_CLOSE_Y_THRESHOLD * 2.0)
    };

    let dy_mag = lcg_range(seed, min_dy, max_dy);
    let dy = if lcg_range(seed, 0.0, 1.0) < 0.5 { dy_mag } else { -dy_mag };

    *head_x += dx;
    *head_y = (*head_y + dy).clamp(HOOK_Y_MIN, HOOK_Y_MAX);

    HookSpec { x: *head_x, y: *head_y }
}

