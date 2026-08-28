//! hazards.rs — when each obstacle joins the run, and how hard it gets after.
//!
//! `difficulty.rs` answers "how far into the run are we". This answers "and
//! therefore what is in the world". Before it existed every hazard spawned from
//! the first second at its full authored danger, so minute one and minute fifty
//! differed only in spacing.
//!
//! ── The shape of a run ──────────────────────────────────────────────────────
//! The opening is deliberately generous: dense grab nodes, frequent bounce pads
//! to catch falls, and plenty of high asteroids to swing from if a pad throws
//! you above the node band. Those three SUPPORTS thin out slowly across the
//! run — they never vanish, they just stop being a safety net.
//!
//! Hazards arrive one at a time, each with several minutes to itself before the
//! next joins, so every one is learned in isolation:
//!
//!   min  5  spinners
//!   min 12  gravity wells
//!   min 19  turrets
//!   min 28  asteroids drifting through the play area
//!   min 36  comets
//!   min 44  solar flares
//!
//! Each is introduced RARE and matures toward its authored danger over the
//! following ~18 minutes, so it is never both new and lethal at once. The boss
//! schedule (9/17/26/34/43/51/60) falls just after each introduction, which
//! makes every fight the punctuation on a chapter the player has just learned.
//!
//! ── The floor ───────────────────────────────────────────────────────────────
//! Everything here scales density and danger. NOTHING here scales grab-node
//! reachability: that is the hop envelope's job (`level_gen`), and it is
//! enforced independently, so no combination of these curves can produce a gap
//! the player cannot cross.

#![allow(dead_code)]

use crate::difficulty::DIFFICULTY_PX_PER_MINUTE;

// ── Hazards ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hazard {
    Spinner,
    GravityWell,
    Turret,
    /// Asteroids that drift through the grab-node band, competing for tether
    /// attention and occasionally blocking a line.
    DriftAsteroid,
    Comet,
    SolarFlare,
}

/// When a hazard first appears and when it reaches full danger, as FRACTIONS of
/// the run.
///
/// Fractions, not absolute minutes. The run length is a tuning knob — it was
/// halved on 2026-08-27 after playtest — and absolute minutes would have left
/// every introduction pinned to the old clock, so the whole schedule would have
/// slid to a different part of a shorter run. As fractions the shape is
/// preserved exactly: shortening the run moves every hazard proportionally
/// earlier in distance while keeping its place in the arc.
#[derive(Clone, Copy, Debug)]
pub struct Stage {
    pub hazard: Hazard,
    pub name: &'static str,
    /// Fraction of the run at which this first appears.
    pub introduce_at: f32,
    /// Fraction of the run at which it reaches full danger.
    pub mature_at: f32,
}

impl Stage {
    pub fn introduce_minutes(&self) -> f32 {
        self.introduce_at * crate::difficulty::DIFFICULTY_FULL_MINUTES
    }
    pub fn mature_minutes(&self) -> f32 {
        self.mature_at * crate::difficulty::DIFFICULTY_FULL_MINUTES
    }
    pub fn introduce_distance(&self) -> f32 {
        self.introduce_at * crate::difficulty::DIFFICULTY_FULL_DISTANCE
    }
    pub fn mature_distance(&self) -> f32 {
        self.mature_at * crate::difficulty::DIFFICULTY_FULL_DISTANCE
    }
}

/// Introduction order. Kept as a table so the pacing of a whole run is legible
/// in one place, and so a test can assert the gaps between introductions.
/// Introduction order, as fractions of a run. The comment column is where each
/// lands on the shipped 80-minute curve, and which boss slot it precedes —
/// every hazard gets solo time before the fight that closes its chapter.
pub const STAGES: &[Stage] = &[
    // frac                                                    ~min   before boss
    Stage { hazard: Hazard::Spinner,       name: "SPINNERS",      introduce_at: 0.083, mature_at: 0.367 }, //  6.7   1
    Stage { hazard: Hazard::GravityWell,   name: "GRAVITY WELLS", introduce_at: 0.200, mature_at: 0.533 }, // 16.0   2
    Stage { hazard: Hazard::Turret,        name: "TURRETS",       introduce_at: 0.317, mature_at: 0.667 }, // 25.3   3
    Stage { hazard: Hazard::DriftAsteroid, name: "ASTEROIDS",     introduce_at: 0.467, mature_at: 0.800 }, // 37.3   4
    Stage { hazard: Hazard::Comet,         name: "COMETS",        introduce_at: 0.600, mature_at: 0.900 }, // 48.0   5
    Stage { hazard: Hazard::SolarFlare,    name: "SOLAR FLARES",  introduce_at: 0.733, mature_at: 0.967 }, // 58.7   6
];

pub fn stage(h: Hazard) -> &'static Stage {
    STAGES
        .iter()
        .find(|s| s.hazard == h)
        .expect("every Hazard variant must have a Stage")
}

/// Smoothstep, matching the difficulty curve's easing so introductions feel
/// like part of the same ramp rather than a separate schedule bolted on.
#[inline]
fn smoothstep(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// Has this hazard joined the run yet?
pub fn hazard_active(distance: f32, h: Hazard) -> bool {
    distance >= stage(h).introduce_distance()
}

/// How far along its own maturity curve a hazard is, 0.0 → 1.0.
///
/// 0.0 on the frame it is introduced (so it arrives at its *rarest*, not its
/// authored density) and 1.0 from `mature_minutes` onward.
pub fn hazard_intensity(distance: f32, h: Hazard) -> f32 {
    let s = stage(h);
    let lo = s.introduce_distance();
    let hi = s.mature_distance();
    if distance <= lo {
        return 0.0;
    }
    smoothstep((distance - lo) / (hi - lo).max(1.0))
}

/// Interpolate a hazard-scaled value across its maturity curve.
#[inline]
pub fn hazard_ramp(distance: f32, h: Hazard, at_introduction: f32, at_maturity: f32) -> f32 {
    at_introduction + (at_maturity - at_introduction) * hazard_intensity(distance, h)
}

/// Scale a spawn-gap range by a hazard's maturity.
///
/// `rare` is a multiplier above 1.0 (wider gaps → fewer of them) applied at
/// introduction; `dense` is below 1.0 at maturity. A hazard therefore shows up
/// as an occasional curiosity long before it becomes a constant pressure.
#[inline]
pub fn hazard_gap_range(distance: f32, h: Hazard, lo: f32, hi: f32, rare: f32, dense: f32) -> (f32, f32) {
    let k = hazard_ramp(distance, h, rare, dense);
    (lo * k, hi * k)
}

// ── Supports ─────────────────────────────────────────────────────────────────

/// The things that make the early game forgiving. They thin out; they never go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    /// Bounce pads — the anti-fall net.
    Pad,
    /// Asteroids high above the node band, there to be swung from when a pad or
    /// a cannon throws the player out of the normal band.
    HighAsteroid,
}

#[derive(Clone, Copy, Debug)]
pub struct SupportCurve {
    pub support: Support,
    pub name: &'static str,
    /// Fraction of the run spent at full generosity before thinning starts.
    pub full_at: f32,
    /// Fraction of the run at which thinning is complete.
    pub thin_at: f32,
    /// Fraction of the opening density that remains at the end. Never 0 — a
    /// support that disappears turns a difficulty curve into a wall.
    pub floor: f32,
}

pub const SUPPORTS: &[SupportCurve] = &[
    SupportCurve { support: Support::Pad,          name: "BOUNCE PADS",    full_at: 0.083, thin_at: 0.667, floor: 0.45 },
    SupportCurve { support: Support::HighAsteroid, name: "HIGH ASTEROIDS", full_at: 0.133, thin_at: 0.750, floor: 0.55 },
];

impl SupportCurve {
    pub fn full_distance(&self) -> f32 {
        self.full_at * crate::difficulty::DIFFICULTY_FULL_DISTANCE
    }
    pub fn thin_distance(&self) -> f32 {
        self.thin_at * crate::difficulty::DIFFICULTY_FULL_DISTANCE
    }
    pub fn full_minutes(&self) -> f32 {
        self.full_at * crate::difficulty::DIFFICULTY_FULL_MINUTES
    }
}

pub fn support_curve(s: Support) -> &'static SupportCurve {
    SUPPORTS
        .iter()
        .find(|c| c.support == s)
        .expect("every Support variant must have a SupportCurve")
}

/// Remaining density of a support, 1.0 → its floor.
pub fn support_level(distance: f32, s: Support) -> f32 {
    let c = support_curve(s);
    let lo = c.full_distance();
    let hi = c.thin_distance();
    if distance <= lo {
        return 1.0;
    }
    let u = smoothstep((distance - lo) / (hi - lo).max(1.0));
    1.0 + (c.floor - 1.0) * u
}

/// Scale a support's spawn-gap range. Gaps grow as the support thins, which is
/// the inverse of `hazard_gap_range`.
#[inline]
pub fn support_gap_range(distance: f32, s: Support, lo: f32, hi: f32) -> (f32, f32) {
    let k = 1.0 / support_level(distance, s).max(0.05);
    (lo * k, hi * k)
}

// ── Reporting ────────────────────────────────────────────────────────────────

/// Hazards live at `distance`, newest first. For the HUD and for the headless
/// harness, which asserts that a hazard never spawns before its introduction.
pub fn active_hazards(distance: f32) -> Vec<&'static Stage> {
    let mut v: Vec<&'static Stage> = STAGES
        .iter()
        .filter(|s| hazard_active(distance, s.hazard))
        .collect();
    v.reverse();
    v
}

/// The next hazard due, and how many minutes away it is.
pub fn next_hazard(distance: f32) -> Option<(&'static Stage, f32)> {
    STAGES
        .iter()
        .find(|s| !hazard_active(distance, s.hazard))
        .map(|s| {
            let at = s.introduce_distance();
            (s, (at - distance) / DIFFICULTY_PX_PER_MINUTE)
        })
}

// ── Per-hazard derived values ────────────────────────────────────────────────

/// Ticks between turret shots at this point in the run.
///
/// A turret introduced at minute 19 fires half again as slowly as its authored
/// interval, so its first appearances are readable; by minute 40 it fires
/// faster than the authored value. `phase` is the turret's own escalation and
/// still applies on top.
pub fn turret_shoot_interval(distance: f32, base_ticks: u32) -> u32 {
    let k = hazard_ramp(distance, Hazard::Turret, 1.55, 0.78);
    ((base_ticks as f32) * k).round().max(30.0) as u32
}

/// How many comets arrive in one wave.
///
/// One at introduction, up to three back to back at maturity — the "more than
/// one comet in a row" escalation, expressed so the count can never jump
/// straight from one to three.
pub fn comet_burst_count(distance: f32) -> u32 {
    let n = hazard_ramp(distance, Hazard::Comet, 1.0, 3.0);
    n.round().clamp(1.0, 3.0) as u32
}

/// Ticks between comet waves, and the chance a due wave actually fires.
pub fn comet_interval(distance: f32, base_ticks: u32) -> u32 {
    let k = hazard_ramp(distance, Hazard::Comet, 2.6, 0.75);
    ((base_ticks as f32) * k).round().max(60.0) as u32
}

pub fn comet_fire_chance(distance: f32) -> f32 {
    hazard_ramp(distance, Hazard::Comet, 0.28, 0.85)
}

/// Fraction of main-zone asteroid spawns that are placed IN the grab-node band
/// rather than high above it.
///
/// Zero until minute 28. High asteroids are a support the player swings from;
/// these are the same objects used against them, sharing a lane with the nodes
/// so they compete for tether attention and sometimes block a line.
pub fn drift_asteroid_share(distance: f32) -> f32 {
    hazard_ramp(distance, Hazard::DriftAsteroid, 0.0, 0.45)
}
