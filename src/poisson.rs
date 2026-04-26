//! Poisson-disk sampler for organic 2-D procedural placement.
//!
//! Bridges Bridson's algorithm to the game's LCG RNG.  The sampler keeps a
//! rolling spatial grid; call `evict_before(min_x)` every tick to discard
//! points that have scrolled off-screen.  `sample_y` picks a Y coordinate
//! that satisfies the minimum-distance constraint against all retained points,
//! falling back to the first candidate rather than stalling if no ideal slot
//! is available.

use std::collections::HashMap;

// ── Inline LCG (avoids a circular dependency with state.rs) ──────────────────

#[inline]
fn lcg_f(s: &mut u64) -> f32 {
    *s = s.wrapping_mul(6_364_136_223_846_793_005)
          .wrapping_add(1_442_695_040_888_963_407);
    let hi = (*s >> 32) as u32;
    (hi as f32) / (u32::MAX as f32)
}

#[inline]
fn lcg_range_f(s: &mut u64, lo: f32, hi: f32) -> f32 {
    lo + lcg_f(s) * (hi - lo)
}

// ── PoissonSampler ────────────────────────────────────────────────────────────

/// Keeps a set of 2-D points where every pair satisfies a minimum Euclidean
/// distance.  The grid cell size is set to `min_dist / √2` so that each cell
/// holds at most one point; neighbour lookups only require checking a 5×5
/// cell window.
#[derive(Clone, Debug)]
pub struct PoissonSampler {
    points: Vec<(f32, f32)>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    cell_size: f32,
    min_dist_sq: f32,
}

impl PoissonSampler {
    /// Create a new sampler that enforces `min_dist` separation between all
    /// registered points.
    pub fn new(min_dist: f32) -> Self {
        let cell_size = min_dist / std::f32::consts::SQRT_2;
        Self {
            points: Vec::new(),
            grid: HashMap::new(),
            cell_size,
            min_dist_sq: min_dist * min_dist,
        }
    }

    /// Remove all points whose X coordinate is less than `min_x` and rebuild
    /// the grid.  Call this once per tick with `player_x - EVICT_MARGIN` to
    /// prevent unbounded memory growth.
    pub fn evict_before(&mut self, min_x: f32) {
        if self.points.is_empty() {
            return;
        }
        let had_old = self.points.iter().any(|(x, _)| *x < min_x);
        if !had_old {
            return;
        }
        self.points.retain(|(x, _)| *x >= min_x);
        self.grid.clear();
        for (i, &(x, y)) in self.points.iter().enumerate() {
            let cx = (x / self.cell_size) as i32;
            let cy = (y / self.cell_size) as i32;
            self.grid.entry((cx, cy)).or_default().push(i);
        }
    }

    /// Register an (x, y) point unconditionally (used for externally-placed
    /// objects like hooks, so they count against future placements).
    pub fn add(&mut self, x: f32, y: f32) {
        let i = self.points.len();
        self.points.push((x, y));
        let cx = (x / self.cell_size) as i32;
        let cy = (y / self.cell_size) as i32;
        self.grid.entry((cx, cy)).or_default().push(i);
    }

    /// Try up to `k` random Y values (drawn from `[y_min, y_max]` via the
    /// shared LCG `seed`) to find one that clears `min_dist` from every
    /// nearby registered point.  The accepted Y is registered and returned.
    /// If no candidate passes, the *first* drawn value is registered anyway
    /// (Bridson fallback — prevents starvation in dense regions).
    pub fn sample_y(
        &mut self,
        seed: &mut u64,
        x: f32,
        y_min: f32,
        y_max: f32,
        k: usize,
    ) -> f32 {
        let first = lcg_range_f(seed, y_min, y_max);
        if self.is_valid(x, first) {
            self.add(x, first);
            return first;
        }
        for _ in 1..k {
            let y = lcg_range_f(seed, y_min, y_max);
            if self.is_valid(x, y) {
                self.add(x, y);
                return y;
            }
        }
        // Fallback: nowhere valid — register and accept the first candidate.
        self.add(x, first);
        first
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn is_valid(&self, x: f32, y: f32) -> bool {
        let cx = (x / self.cell_size) as i32;
        let cy = (y / self.cell_size) as i32;
        for dx in -2..=2_i32 {
            for dy in -2..=2_i32 {
                if let Some(indices) = self.grid.get(&(cx + dx, cy + dy)) {
                    for &i in indices {
                        let (px, py) = self.points[i];
                        let d2 = (x - px) * (x - px) + (y - py) * (y - py);
                        if d2 < self.min_dist_sq {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }
}
