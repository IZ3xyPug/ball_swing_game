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
const K_WAVE: usize = 0;
const K_CLIMB: usize = 1;
const K_DESCENT: usize = 2;
const K_ARCH: usize = 3;
const K_VALLEY: usize = 4;
const K_GLIDE: usize = 5;
const K_ZIGZAG: usize = 6;

/// Pacing: what a run of nodes is FOR. Runs cycle through the roles in
/// order, so play rises and falls instead of sitting at one pitch:
///   Breather  wide, gentle, a level line: room to breathe and line up;
///   Build     the curve's own difficulty, any shape;
///   Crux      short and sharp: the longest strides and steepest changes
///             the curve allows here, still always inside the reach;
///   Payoff    a descent that builds speed into one long launch, a trail of
///             coins along the flight. Nailing the crux is paid for here.
/// The cycle tightens with the curve (shorter breathers, longer cruxes late)
/// but keeps its shape, so a late run still has places to recover.
#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub enum Role { #[default] Breather, Build, Crux, Payoff }

/// The run of nodes being generated: its shape, its role, where it is in it,
/// how long it is, the height it started from, and its spacing; and how many
/// builds are left before the next crux.
#[derive(Default)]
struct Pattern { kind: usize, role: Role, step: usize, len: usize, anchor: f32, dx: f32, builds_left: usize, started: bool }

thread_local! {
    static PATTERN: std::cell::RefCell<Pattern> = std::cell::RefCell::new(Pattern::default());
}

/// Start the pattern sequence afresh (a new run, or a regenerated stretch).
pub fn reset_patterns() {
    PATTERN.with(|p| *p.borrow_mut() = Pattern::default());
}

/// The role of the run the last node came from (for tests and tuning).
pub fn current_role() -> Role { PATTERN.with(|p| p.borrow().role) }

fn pick(seed: &mut u64, from: &[usize], not: usize) -> usize {
    let mut k = from[(lcg_range(seed, 0.0, from.len() as f32) as usize).min(from.len() - 1)];
    if k == not && from.len() > 1 { k = from[(from.iter().position(|&x| x == k).unwrap() + 1) % from.len()]; }
    k
}

/// Generate the next grab node as a single hop from `(head_x, head_y)`,
/// advancing the head to the new position.
///
/// `distance_px` drives the difficulty curve (stride ~0.76-0.95 of the reach,
/// vertical budget 45-100%); the run's `Role` bends that up or down locally.
pub fn generate_next_hook(
    seed: &mut u64,
    head_x: &mut f32,
    head_y: &mut f32,
    distance_px: f32,
) -> HookSpec {
    let t = difficulty_t(distance_px);

    // ── The curve's own stride and vertical budget ──────────────────────────
    let lo = lerp(HOOK_STRIDE_FRAC_EASY_MIN, HOOK_STRIDE_FRAC_HARD_MIN, t) * HOP_REACH_X;
    let hi = lerp(HOOK_STRIDE_FRAC_EASY_MAX, HOOK_STRIDE_FRAC_HARD_MAX, t) * HOP_REACH_X;
    let (lo, hi) = (lo.min(hi), hi.max(lo));
    let vert_frac = lerp(HOOK_VERT_FRAC_EASY, HOOK_VERT_FRAC_HARD, t);
    // The longest stride a node may ever sit at (the crux's and the launch's).
    let reach_max = HOOK_STRIDE_FRAC_HARD_MAX * HOP_REACH_X;

    // ── The run: its role, shape, length and rhythm ─────────────────────────
    let (kind, role, step, len, anchor, dx_run) = PATTERN.with(|p| {
        let mut p = p.borrow_mut();
        if !p.started || p.step >= p.len {
            let first = !p.started;
            p.started = true;
            // The next role in the cycle.
            p.role = if first { Role::Breather } else {
                match p.role {
                    Role::Breather => { p.builds_left = 1 + (lcg_range(seed, 0.0, 1.99 - t) as usize); Role::Build }
                    Role::Build if p.builds_left > 1 => { p.builds_left -= 1; Role::Build }
                    Role::Build => Role::Crux,
                    Role::Crux => Role::Payoff,
                    Role::Payoff => Role::Breather,
                }
            };
            let prev = p.kind;
            let (kind, len, dx) = match p.role {
                // Breathers shorten as the run goes on (5-6 nodes early, 3-4
                // late) but never vanish.
                Role::Breather => (pick(seed, &[K_GLIDE, K_ARCH, K_VALLEY], prev),
                    lcg_range(seed, lerp(5.0, 3.0, t), lerp(6.99, 4.99, t)) as usize,
                    lerp(lo, hi, 0.1)),
                Role::Build => (pick(seed, &[K_WAVE, K_CLIMB, K_DESCENT, K_ARCH, K_VALLEY, K_ZIGZAG], prev),
                    lcg_range(seed, 4.0, 6.99) as usize,
                    lcg_range(seed, lo, hi)),
                // Cruxes lengthen late (3 nodes early, up to 5).
                Role::Crux => (pick(seed, &[K_ZIGZAG, K_CLIMB, K_WAVE], prev),
                    lcg_range(seed, 3.0, lerp(3.99, 5.99, t)) as usize,
                    lerp(hi, reach_max, 0.55)),
                Role::Payoff => (K_DESCENT, 4, lerp(lo, hi, 0.5)),
            };
            p.kind = kind;
            p.len = len.max(2);
            p.step = 0;
            p.anchor = *head_y;
            p.dx = dx;
        }
        let out = (p.kind, p.role, p.step, p.len, p.anchor, p.dx);
        p.step += 1;
        out
    });

    // The payoff's last node is its launch: the longest stride, level with
    // the bottom of the descent (a long gap has no vertical budget left).
    let launch = role == Role::Payoff && step + 1 == len;
    let dx = if launch { reach_max } else { dx_run.clamp(lo.min(reach_max), reach_max) };
    // How much of the vertical budget this run spends: calmer in a breather,
    // all of it in a crux.
    let vf = match role {
        Role::Breather => vert_frac * 0.5,
        Role::Crux => (vert_frac + 0.3).min(1.0),
        _ => vert_frac,
    };
    let (up_budget, down_budget) = hop_dy_budget(dx);
    let up = up_budget * vf * HOP_REACH_MARGIN;
    let down = down_budget * vf * HOP_REACH_MARGIN;
    let k = step as f32 / (len.max(2) - 1) as f32;          // 0..1 along the run
    let amp = (HOOK_Y_MAX - HOOK_Y_MIN) * 0.30 * (0.6 + 0.4 * vf);
    let mid = (HOOK_Y_MIN + HOOK_Y_MAX) * 0.5;
    // Where the shape wants this node (world y; up is negative). Each run
    // drifts back toward the middle of the band, so the line never parks
    // against the top or the bottom; a climb that starts high turns into a
    // descent (and the other way round), so it has room.
    let anchor = anchor + (mid - anchor) * 0.45 * k;
    let room_up = anchor - HOOK_Y_MIN;
    let room_down = HOOK_Y_MAX - anchor;
    let kind = match kind {
        K_CLIMB if room_up < amp * 1.2 => K_DESCENT,
        // A payoff always gets its drop: from high up if it has to.
        K_DESCENT if room_down < amp * 1.2 && role != Role::Payoff => K_CLIMB,
        K_ARCH if room_up < amp => K_VALLEY,
        K_VALLEY if room_down < amp => K_ARCH,
        k => k,
    };
    let target = if launch {
        *head_y
    } else {
        match kind {
            K_WAVE => anchor + amp * (k * std::f32::consts::TAU).sin(),
            K_CLIMB => anchor - amp * 1.4 * k,
            K_DESCENT => anchor + amp * 1.4 * k,
            K_ARCH => anchor - amp * (k * std::f32::consts::PI).sin() * 1.3,
            K_VALLEY => anchor + amp * (k * std::f32::consts::PI).sin() * 1.3,
            K_GLIDE => anchor + (mid - anchor) * 0.15,
            _ => anchor + if step % 2 == 0 { -amp * 0.8 } else { amp * 0.8 },
        }
    };
    // Keep the shape on the playable band, then inside this hop's envelope.
    let target = target.clamp(HOOK_Y_MIN + 40.0, HOOK_Y_MAX - 40.0);
    let dy = (target - *head_y).clamp(-up, down);
    *head_x += dx;
    // Clamping to the playable band can only pull the node back toward
    // `head_y`, which is itself always in-band — so it can shorten a hop but
    // never lengthen one past the envelope.
    *head_y = (*head_y + dy).clamp(HOOK_Y_MIN, HOOK_Y_MAX);

    HookSpec { x: *head_x, y: *head_y, launch }
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
            println!("NODE {} {} {:?}{}", h.x, h.y, current_role(), if h.launch { " LAUNCH" } else { "" });
        }
    }
}

#[cfg(test)]
mod pacing_tests {
    use super::*;
    use crate::difficulty::DIFFICULTY_FULL_DISTANCE;

    /// Every role turns up, in the cycle's order, early and late in a run;
    /// every hop stays reachable; and each payoff ends in exactly one launch.
    #[test]
    fn the_runs_cycle_through_their_roles_and_stay_reachable() {
        for d in [0.0, DIFFICULTY_FULL_DISTANCE * 0.5, DIFFICULTY_FULL_DISTANCE] {
            reset_patterns();
            let (mut seed, mut x, mut y) = (0xC0FFEEu64, 0.0f32, (HOOK_Y_MIN + HOOK_Y_MAX) * 0.5);
            let mut seen = Vec::new();
            let mut launches = 0;
            for _ in 0..400 {
                let (px, py) = (x, y);
                let h = generate_next_hook(&mut seed, &mut x, &mut y, d);
                assert!(hop_is_reachable(h.x - px, h.y - py), "unreachable hop at d={d}: ({px},{py}) -> ({},{})", h.x, h.y);
                let r = current_role();
                if seen.last() != Some(&r) { seen.push(r); }
                if h.launch { launches += 1; assert_eq!(r, Role::Payoff); }
            }
            for r in [Role::Breather, Role::Build, Role::Crux, Role::Payoff] { assert!(seen.contains(&r), "{r:?} never came at d={d}"); }
            let payoffs = seen.iter().filter(|r| **r == Role::Payoff).count();
            assert!(launches >= payoffs.saturating_sub(1) && launches <= payoffs, "{launches} launches for {payoffs} payoffs");
            // Order: a crux is always followed by a payoff, a payoff by a breather.
            for w in seen.windows(2) {
                if w[0] == Role::Crux { assert_eq!(w[1], Role::Payoff); }
                if w[0] == Role::Payoff { assert_eq!(w[1], Role::Breather); }
            }
        }
    }
}
