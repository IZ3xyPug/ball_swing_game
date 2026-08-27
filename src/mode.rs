//! mode.rs — the three ways to play, and where bosses sit in each.
//!
//! Before this existed there was one mode with a `boss_mode_active` flag and a
//! single hardcoded `BOSS_THRESHOLD_X`, so a run had at most one boss fight and
//! there was no way to play without them.
//!
//!   Casual    — the normal zone, forever, no bosses. Tracks furthest distance.
//!   Normal    — the core loop: the difficulty curve, bosses on a schedule, a
//!               final boss at the top of the curve, space-zone launch pads.
//!   Boss Rush — short link sections between fights, scored on total time.
//!
//! Boss pacing is expressed in MINUTES and converted through the same
//! `ASSUMED_PLAYER_PX_PER_SEC` the difficulty curve uses, so retuning the
//! player's assumed speed moves the curve and the boss schedule together
//! instead of silently desynchronising them.

#![allow(dead_code)]

use crate::difficulty::{DIFFICULTY_FULL_DISTANCE, DIFFICULTY_PX_PER_MINUTE};

// ── Mode ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    Casual,
    Normal,
    BossRush,
}

impl Default for GameMode {
    fn default() -> Self {
        GameMode::Normal
    }
}

impl GameMode {
    /// Stable index used in canvas vars and the profile. Never renumber — it is
    /// persisted alongside best-distance and best-time records.
    pub const fn index(self) -> i32 {
        match self {
            GameMode::Casual => 0,
            GameMode::Normal => 1,
            GameMode::BossRush => 2,
        }
    }

    pub const fn from_index(i: i32) -> Self {
        match i {
            0 => GameMode::Casual,
            2 => GameMode::BossRush,
            _ => GameMode::Normal,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            GameMode::Casual => "CASUAL",
            GameMode::Normal => "NORMAL",
            GameMode::BossRush => "BOSS RUSH",
        }
    }

    pub const fn blurb(self) -> &'static str {
        match self {
            GameMode::Casual => "ENDLESS SWINGING, NO BOSSES \u{2022} CHASING DISTANCE",
            GameMode::Normal => "THE FULL RUN \u{2022} RISING DIFFICULTY, BOSSES, A FINAL FIGHT",
            GameMode::BossRush => "EVERY BOSS, BACK TO BACK \u{2022} SCORED ON TIME",
        }
    }

    /// Whether bosses appear at all.
    pub const fn has_bosses(self) -> bool {
        !matches!(self, GameMode::Casual)
    }

    /// Whether the run ever ends on its own. Casual is endless by design — the
    /// score is how far you got before you ran out of hearts.
    pub const fn has_ending(self) -> bool {
        !matches!(self, GameMode::Casual)
    }

    /// Whether rocket pads can launch the player into the space zone.
    /// Boss Rush stays on the ground: a timed run should not branch into a
    /// bonus area whose length the player does not control.
    pub const fn allows_space_zone(self) -> bool {
        !matches!(self, GameMode::BossRush)
    }

    /// Whether the run pays meta currency. Casual pays at a reduced rate rather
    /// than nothing — a mode that funds no progress reads as a dead end, and
    /// the reduction is what keeps Normal the efficient way to earn.
    pub const fn meta_multiplier(self) -> f32 {
        match self {
            GameMode::Casual => 0.35,
            GameMode::Normal => 1.0,
            GameMode::BossRush => 0.85,
        }
    }

    /// What a leaderboard for this mode would rank.
    pub fn record_kind(self) -> RecordKind {
        match self {
            GameMode::Casual => RecordKind::FurthestDistance,
            GameMode::Normal => RecordKind::HighScore,
            GameMode::BossRush => RecordKind::FastestTime,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    FurthestDistance,
    HighScore,
    FastestTime,
}

// ── Boss schedule ────────────────────────────────────────────────────────────

/// How many bosses a full run contains — the size of the roster.
///
/// This is the number the schedule is DERIVED from, not the other way round.
/// An independent "minutes between fights" constant disagreed with the roster
/// as soon as either moved: it produced eight fights for a seven-boss roster,
/// with the seventh crammed four minutes before the finale.
pub const BOSS_ROSTER_SIZE: u32 = 7;

/// Average minutes of clean play between fights in Normal, for reference.
/// Derived, so it cannot drift from the schedule.
pub const BOSS_INTERVAL_MINUTES: f32 =
    crate::difficulty::DIFFICULTY_FULL_MINUTES / BOSS_ROSTER_SIZE as f32;

/// Distance the player swings between fights in Boss Rush — a link section, not
/// a stretch of level. ~8 s of travel: long enough to rebuild momentum and read
/// the next arena, short enough that the mode is about the fights.
pub const BOSS_RUSH_LINK_DISTANCE: f32 = 6_400.0;

/// World X at which boss number `index` (0-based) begins, or `None` when this
/// mode has no such fight.
///
/// Normal spreads the roster evenly across the difficulty curve with the last
/// fight pinned exactly to the top of it, so the finale always lands where the
/// ramp ends however the roster or the curve is later retuned.
pub fn boss_trigger_distance(mode: GameMode, index: u32) -> Option<f32> {
    if index >= BOSS_ROSTER_SIZE {
        return None;
    }
    match mode {
        GameMode::Casual => None,
        GameMode::Normal => {
            let step = DIFFICULTY_FULL_DISTANCE / BOSS_ROSTER_SIZE as f32;
            Some(step * (index as f32 + 1.0))
        }
        GameMode::BossRush => Some(BOSS_RUSH_LINK_DISTANCE * (index as f32 + 1.0)),
    }
}

/// Where the first boss arena is carved out.
///
/// DERIVED from the curve rather than written as a literal: arenas must sit
/// past everything the generator will ever produce, and a hand-picked constant
/// silently stopped being far enough the moment the curve was lengthened.
/// The factor of four leaves room for an over-running Casual player too, even
/// though Casual never opens an arena.
pub const BOSS_ARENA_ORIGIN_X: f32 = DIFFICULTY_FULL_DISTANCE * 4.0;

/// Whether `index` is the run's last fight.
pub fn is_final_boss(mode: GameMode, index: u32) -> bool {
    boss_trigger_distance(mode, index).is_some()
        && boss_trigger_distance(mode, index + 1).is_none()
}

/// Total fights in a full run of `mode`.
pub fn boss_count(mode: GameMode) -> u32 {
    let mut n = 0;
    while boss_trigger_distance(mode, n).is_some() {
        n += 1;
        if n > 64 {
            break; // schedule is malformed; refuse to spin
        }
    }
    n
}

// ── Canvas plumbing ──────────────────────────────────────────────────────────

pub const MODE_VAR: &str = "game_mode";

pub fn current_mode(c: &quartz::Canvas) -> GameMode {
    match c.get_var(MODE_VAR) {
        Some(quartz::Value::I32(i)) => GameMode::from_index(i),
        _ => GameMode::default(),
    }
}

pub fn set_mode(c: &mut quartz::Canvas, mode: GameMode) {
    c.set_var(MODE_VAR, quartz::Value::I32(mode.index()));
}
