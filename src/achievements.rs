//! Achievements: one table, fed by game events.
//!
//! Every achievement is a row in [`ACHIEVEMENTS`]: a stable id (written into
//! profile files, never rename), a name, a description, a tier, and a
//! trigger. A trigger is either a lifetime-stat predicate, checked whenever
//! the stats might have moved ([`check_achievements`]), or an event the game
//! reports at the moment it happens ([`on_boss_defeated`], [`on_death`],
//! [`on_rare_seen`], ...). Adding one is a row here, plus a line at an event
//! site if it needs a new event.
//!
//! Unlocks go into a QUEUE and the HUD shows them one after another, so
//! several at once are all seen (the old toast showed only the last).
//!
//! The old easy ones (10 coins, 10,000 px, five runs...) are retired: anyone
//! who earned one keeps it, listed at the end as a legacy row; new profiles
//! never see them.

use quartz::*;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

pub const TOTAL_COINS_COLLECTED_VAR: &str = "total_coins_collected";

// ── The toast ────────────────────────────────────────────────────────────────
pub const TOAST_ACTIVE_VAR: &str = "achievement_toast_active";
pub const TOAST_TICKS_VAR: &str = "achievement_toast_ticks";
pub const TOAST_PANEL_NAME: &str = "achievement_toast_panel";
pub const TOAST_TITLE_NAME: &str = "achievement_toast_title";
pub const TOAST_DESC_NAME: &str = "achievement_toast_desc";
pub const TOAST_CHECK_NAME: &str = "achievement_toast_check";
pub const TOAST_TOTAL_TICKS: u32 = 150;
pub const TOAST_RISE_TICKS: u32 = 14;
pub const TOAST_WIDTH: f32 = 1220.0;
pub const TOAST_HEIGHT: f32 = 156.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Easy: shows up in normal play.
    Bronze,
    /// Takes intent.
    Silver,
    /// A real feat.
    Gold,
    /// Hidden until earned.
    Secret,
}

impl Tier {
    pub fn label(self) -> &'static str {
        match self { Tier::Bronze => "BRONZE", Tier::Silver => "SILVER", Tier::Gold => "GOLD", Tier::Secret => "SECRET" }
    }
    /// The tier's colour (titles, medals).
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Tier::Bronze => (222, 160, 105),
            Tier::Silver => (205, 215, 235),
            Tier::Gold => (250, 214, 92),
            Tier::Secret => (200, 150, 255),
        }
    }
}

/// What the stat predicates see: the active profile's lifetime numbers.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub best_distance: f32,
    pub best_distance_normal: f32,
    pub best_distance_casual: f32,
    pub best_run_coins: u64,
    pub bosses_defeated: u32,
    pub boss_kinds: usize,
    pub untouchable_kinds: usize,
    pub runs: u32,
    pub deaths: u32,
    pub flares: u32,
    pub seconds_played: u64,
    pub meta: u64,
    pub all_upgrades_maxed: bool,
    pub hooks_grabbed: u64,
    pub rare_seen: usize,
    pub daily_streak: u32,
    pub dailies_done: u32,
}

#[derive(Clone, Copy)]
pub enum Trigger {
    /// Earned when this holds of the lifetime stats.
    Stat(fn(&Stats) -> bool),
    /// Earned by a game event (see the `on_*` functions); the id is the key.
    Event,
}

pub struct Ach {
    /// Stable save key. Never rename.
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub tier: Tier,
    pub trigger: Trigger,
}

macro_rules! stat { ($e:expr) => { Trigger::Stat($e) }; }

/// Every achievement, in the order the achievements page lists them.
pub const ACHIEVEMENTS: &[Ach] = &[
    // ── Milestones ──
    Ach { id: "litter_box", name: "Out of the Litter Box", desc: "Travel 100,000 px in one run.", tier: Tier::Bronze, trigger: stat!(|s| s.best_distance >= 100_000.0) },
    Ach { id: "commuter_cat", name: "Commuter Cat", desc: "Travel 360,000 px in one run.", tier: Tier::Silver, trigger: stat!(|s| s.best_distance >= 360_000.0) },
    Ach { id: "long_tail", name: "Long Tail", desc: "Travel 720,000 px in one run.", tier: Tier::Gold, trigger: stat!(|s| s.best_distance >= 720_000.0) },
    Ach { id: "top_of_curve", name: "Top of the Curve", desc: "Reach peak difficulty (1,440,000 px) in Normal.", tier: Tier::Gold, trigger: stat!(|s| s.best_distance_normal >= 1_440_000.0) },
    Ach { id: "pawsitively_endless", name: "Pawsitively Endless", desc: "Travel 2,000,000 px in one Casual run.", tier: Tier::Gold, trigger: stat!(|s| s.best_distance_casual >= 2_000_000.0) },
    Ach { id: "space_cadet", name: "Space Cadet", desc: "Reach the space zone.", tier: Tier::Bronze, trigger: Trigger::Event },
    Ach { id: "hairball_of_gold", name: "Hairball of Gold", desc: "Collect 300 coins in a single run.", tier: Tier::Silver, trigger: stat!(|s| s.best_run_coins >= 300) },
    Ach { id: "frequent_flyer", name: "Frequent Flyer", desc: "Play 100 runs.", tier: Tier::Silver, trigger: stat!(|s| s.runs >= 100) },
    Ach { id: "flares_10", name: "Weathered", desc: "Survive 10 solar flares.", tier: Tier::Bronze, trigger: stat!(|s| s.flares >= 10) },
    Ach { id: "grabby_paws", name: "Grabby Paws", desc: "Grab 10,000 nodes, lifetime.", tier: Tier::Silver, trigger: stat!(|s| s.hooks_grabbed >= 10_000) },
    Ach { id: "zoomies", name: "Zoomies", desc: "Play for three hours, lifetime.", tier: Tier::Silver, trigger: stat!(|s| s.seconds_played >= 3 * 3600) },
    // ── Bosses ──
    Ach { id: "boss_slayer", name: "Boss Slayer", desc: "Defeat any boss.", tier: Tier::Bronze, trigger: stat!(|s| s.bosses_defeated >= 1) },
    Ach { id: "whole_menagerie", name: "The Whole Menagerie", desc: "Defeat all seven bosses, lifetime.", tier: Tier::Gold, trigger: stat!(|s| s.boss_kinds >= 7) },
    Ach { id: "untouchable", name: "Untouchable", desc: "Defeat a boss without losing a heart.", tier: Tier::Silver, trigger: Trigger::Event },
    Ach { id: "clean_sweep", name: "Clean Sweep", desc: "Be Untouchable against three different bosses.", tier: Tier::Gold, trigger: stat!(|s| s.untouchable_kinds >= 3) },
    Ach { id: "speed_date", name: "Speed Date", desc: "Defeat a boss within 60 seconds of it appearing.", tier: Tier::Gold, trigger: Trigger::Event },
    Ach { id: "hat_trick", name: "Hat Trick", desc: "Defeat three bosses in one run.", tier: Tier::Silver, trigger: Trigger::Event },
    // ── Ways to die ──
    Ach { id: "die_sun", name: "Sunburnt", desc: "Fly too close to the sun.", tier: Tier::Bronze, trigger: Trigger::Event },
    Ach { id: "die_oxygen", name: "Running on Fumes", desc: "Run out of oxygen.", tier: Tier::Bronze, trigger: Trigger::Event },
    Ach { id: "die_fall", name: "Gravity's Lap", desc: "Fall to the danger floor.", tier: Tier::Bronze, trigger: Trigger::Event },
    Ach { id: "slain_by_boss", name: "Boss Fight Club", desc: "Lose your last heart in a boss fight.", tier: Tier::Bronze, trigger: Trigger::Event },
    Ach { id: "floor_is_lava", name: "Floor Is Lava", desc: "Die within 20 seconds of starting a run.", tier: Tier::Secret, trigger: Trigger::Event },
    Ach { id: "premature_celebration", name: "Premature Celebration", desc: "Lose a heart within 3 seconds of defeating a boss.", tier: Tier::Secret, trigger: Trigger::Event },
    Ach { id: "cat_astrophe", name: "Cat-astrophe", desc: "Lose three hearts within 5 seconds.", tier: Tier::Silver, trigger: Trigger::Event },
    Ach { id: "so_close", name: "So Close", desc: "Die while a boss's eclipse is falling.", tier: Tier::Silver, trigger: Trigger::Event },
    Ach { id: "quack_attack", name: "Quack Attack", desc: "Die while the rubber duck floats by.", tier: Tier::Secret, trigger: Trigger::Event },
    Ach { id: "ninety_nine_lives", name: "Ninety-Nine Lives", desc: "Die 99 times, lifetime.", tier: Tier::Silver, trigger: stat!(|s| s.deaths >= 99) },
    // ── Sightseeing ──
    Ach { id: "seen_duck", name: "Quack in Space", desc: "Spot the rubber duck.", tier: Tier::Secret, trigger: Trigger::Event },
    Ach { id: "seen_whale", name: "Whale Hello There", desc: "Spot the space whale.", tier: Tier::Secret, trigger: Trigger::Event },
    Ach { id: "seen_ufo", name: "We Come in Peace", desc: "Spot the UFO.", tier: Tier::Secret, trigger: Trigger::Event },
    Ach { id: "seen_astronaut", name: "Spacewalk", desc: "Spot the drifting astronaut.", tier: Tier::Secret, trigger: Trigger::Event },
    Ach { id: "seen_jelly", name: "Jelly Belly", desc: "Spot the space jellyfish.", tier: Tier::Secret, trigger: Trigger::Event },
    Ach { id: "tourist_trap", name: "Tourist Trap", desc: "Spot all five oddities, lifetime.", tier: Tier::Silver, trigger: stat!(|s| s.rare_seen >= 5) },
    Ach { id: "aurora_chaser", name: "Aurora Chaser", desc: "Swing under the northern lights for a whole display.", tier: Tier::Bronze, trigger: Trigger::Event },
    // ── Economy ──
    Ach { id: "fat_cat", name: "Fat Cat", desc: "Hold 3,000 META at once.", tier: Tier::Silver, trigger: stat!(|s| s.meta >= 3_000) },
    Ach { id: "fully_loaded", name: "Fully Loaded", desc: "Max every permanent upgrade.", tier: Tier::Gold, trigger: stat!(|s| s.all_upgrades_maxed) },
    Ach { id: "retail_therapy", name: "Retail Therapy", desc: "Take five upgrades in one run.", tier: Tier::Silver, trigger: Trigger::Event },
    // ── Habits ──
    Ach { id: "graduate", name: "Graduate", desc: "Finish the tutorial.", tier: Tier::Bronze, trigger: Trigger::Event },
    Ach { id: "daily_grind", name: "Daily Grind", desc: "Complete a daily challenge.", tier: Tier::Bronze, trigger: stat!(|s| s.dailies_done >= 1) },
    Ach { id: "seven_dailies", name: "Clockwork", desc: "Complete seven daily challenges.", tier: Tier::Silver, trigger: stat!(|s| s.dailies_done >= 7) },
    Ach { id: "creature_of_habit", name: "Creature of Habit", desc: "Claim the daily reward seven days in a row.", tier: Tier::Silver, trigger: stat!(|s| s.daily_streak >= 7) },
];

/// Retired ids, with their old names: shown at the end of the page, only to
/// profiles that earned them.
pub const LEGACY: &[(&str, &str, &str)] = &[
    ("gold_master", "Gold Master!", "Collect 10 coins. (retired)"),
    ("coin_hunter", "Coin Hunter", "Collect 100 coins. (retired)"),
    ("distance_10k", "Marathoner", "Reach 10,000 px in a run. (retired)"),
    ("distance_25k", "Quarter-Century", "Reach 25,000 px in a run. (retired)"),
    ("marathon", "Marathon", "Reach 50,000 px in a run. (retired)"),
    ("five_runs", "Regular", "Play 5 runs. (retired)"),
    ("casual_10", "Casual Regular", "Play 10 Casual runs. (retired)"),
    ("boss_rush_5", "Rush Regular", "Play 5 Boss Rush runs. (retired)"),
];

pub fn by_id(id: &str) -> Option<&'static Ach> {
    ACHIEVEMENTS.iter().find(|a| a.id == id)
}

/// What the achievements page lists: every live row, then the legacy rows
/// this profile earned. (id, title, description, tier).
pub(crate) fn page_rows() -> Vec<(&'static str, &'static str, &'static str, Tier)> {
    let mut v: Vec<_> = ACHIEVEMENTS.iter().map(|a| (a.id, a.name, a.desc, a.tier)).collect();
    for &(id, name, desc) in LEGACY {
        if crate::profile::profile_has_achievement(id) {
            v.push((id, name, desc, Tier::Bronze));
        }
    }
    v
}

// ── Unlocking ────────────────────────────────────────────────────────────────

fn queue() -> &'static Mutex<VecDeque<(String, String, Tier)>> {
    static Q: OnceLock<Mutex<VecDeque<(String, String, Tier)>>> = OnceLock::new();
    Q.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Earn `id` (no-op if already earned): persist it and queue its toast.
pub fn unlock(id: &str) -> bool {
    let Some(a) = by_id(id) else { return false };
    if crate::profile::profile_has_achievement(id) { return false; }
    crate::profile::unlock_achievement_on_profile(id);
    log::info!("achievement: {}", a.name);
    queue().lock().unwrap().push_back((a.name.to_string(), a.desc.to_string(), a.tier));
    true
}

/// Queue a toast that is not an achievement (a daily completed, ...).
pub fn toast(title: &str, desc: &str, tier: Tier) {
    queue().lock().unwrap().push_back((title.to_string(), desc.to_string(), tier));
}

/// The next queued toast, for the HUD.
pub fn next_toast() -> Option<(String, String, Tier)> {
    queue().lock().unwrap().pop_front()
}

pub fn toast_active(c: &Canvas) -> bool {
    matches!(c.get_var(TOAST_ACTIVE_VAR), Some(Value::Bool(true)))
}

pub fn toast_ticks(c: &Canvas) -> u32 {
    match c.get_var(TOAST_TICKS_VAR) { Some(Value::I32(v)) => v.max(0) as u32, _ => 0 }
}

pub fn clear_toast(c: &mut Canvas) {
    c.set_var(TOAST_ACTIVE_VAR, false);
    c.set_var(TOAST_TICKS_VAR, 0i32);
}

/// The lifetime stats, with the current (not yet recorded) run folded in.
fn stats(run_coins: u64, run_distance: f32) -> Stats {
    let g = crate::profile::profile();
    let p = g.lock().unwrap();
    let all_maxed = crate::profile::PERM_UPGRADES.iter().all(|u| p.upgrade_level(u.id) >= u.max);
    Stats {
        best_distance: (p.best_distance as f32).max(run_distance),
        best_distance_normal: p.best_distance_normal as f32,
        best_distance_casual: p.best_distance_casual as f32,
        best_run_coins: p.best_run_coins.max(run_coins),
        bosses_defeated: p.bosses_defeated,
        boss_kinds: p.boss_kinds_defeated.len(),
        untouchable_kinds: p.untouchable_kinds.len(),
        runs: p.runs_played,
        deaths: p.deaths,
        flares: p.flares_survived,
        seconds_played: p.time_played_seconds,
        meta: p.meta_currency,
        all_upgrades_maxed: all_maxed,
        hooks_grabbed: p.hooks_grabbed + run().grabs,
        rare_seen: p.rare_seen.len(),
        daily_streak: p.daily_streak,
        dailies_done: p.dailies_done,
    }
}

/// Earn every stat achievement the stats now satisfy. `run_coins` and
/// `run_distance` add the current run so they can pop mid-run; pass (0, 0.0)
/// once the run is recorded.
pub fn check_achievements(_c: &mut Canvas, run_coins: u64, run_distance: f32) {
    let s = stats(run_coins, run_distance);
    for a in ACHIEVEMENTS {
        if let Trigger::Stat(f) = a.trigger {
            if f(&s) { unlock(a.id); }
        }
    }
}

// ── Events ───────────────────────────────────────────────────────────────────
//
// What one run has seen so far, for the event achievements.

#[derive(Default, Clone)]
struct RunTrack {
    /// Frames since the run began.
    tick: u64,
    hearts_prev: Option<i32>,
    /// Frames at which hearts were lost (recent ones).
    losses: Vec<u64>,
    /// The fight in progress: when it began and the hearts lost in it.
    fight_start: Option<u64>,
    fight_losses: u32,
    last_boss_kill: Option<u64>,
    bosses: u32,
    grabs: u64,
    upgrades: u32,
    duck_seen_at: Option<u64>,
    aurora_frames: u32,
}

fn track() -> &'static Mutex<RunTrack> {
    static T: OnceLock<Mutex<RunTrack>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(RunTrack::default()))
}

fn run() -> RunTrack { track().lock().unwrap().clone() }

/// A run is starting.
pub fn on_run_start() {
    *track().lock().unwrap() = RunTrack::default();
}

/// Every game frame (alive): watches hearts and fight starts, so no attack or
/// boss has to report them itself.
pub fn tick(c: &mut Canvas, boss_spawned: bool, aurora_up: bool) {
    let hearts = c.get_i32("hearts");
    let mut earn: Vec<&str> = Vec::new();
    {
        let mut t = track().lock().unwrap();
        t.tick += 1;
        let now = t.tick;
        if let Some(prev) = t.hearts_prev {
            if hearts < prev {
                for _ in 0..(prev - hearts) {
                    t.losses.push(now);
                    if t.fight_start.is_some() { t.fight_losses += 1; }
                }
                if t.last_boss_kill.is_some_and(|k| now - k <= 180) { earn.push("premature_celebration"); }
            }
        }
        t.hearts_prev = Some(hearts);
        t.losses.retain(|&f| now - f <= 300);
        if t.losses.len() >= 3 { earn.push("cat_astrophe"); }
        match (boss_spawned, t.fight_start) {
            (true, None) => { t.fight_start = Some(now); t.fight_losses = 0; }
            (false, Some(_)) => t.fight_start = None,
            _ => {}
        }
        t.aurora_frames = if aurora_up { t.aurora_frames + 1 } else { 0 };
        // A display runs 35-55 s; 30 s under one is "a whole display".
        if t.aurora_frames >= 30 * 60 { earn.push("aurora_chaser"); }
    }
    for id in earn { unlock(id); }
}

/// The player grabbed a node.
pub fn on_grab() {
    track().lock().unwrap().grabs += 1;
}

/// An upgrade was taken (a ring in the run, or a reward pick).
pub fn on_upgrade_taken() {
    let n = { let mut t = track().lock().unwrap(); t.upgrades += 1; t.upgrades };
    if n >= 5 { unlock("retail_therapy"); }
}

/// The player reached space.
pub fn award_space_cadet(_c: &mut Canvas) {
    unlock("space_cadet");
}

/// A boss just died. `kind` is its `BossKind::slug`.
pub fn on_boss_defeated(kind: &str) {
    let (losses, secs, bosses) = {
        let mut t = track().lock().unwrap();
        let secs = t.fight_start.map(|s| (t.tick - s) as f32 / 60.0).unwrap_or(f32::MAX);
        let losses = t.fight_losses;
        t.fight_start = None;
        t.last_boss_kill = Some(t.tick);
        t.bosses += 1;
        (losses, secs, t.bosses)
    };
    {
        let g = crate::profile::profile();
        let mut p = g.lock().unwrap();
        if !p.boss_kinds_defeated.iter().any(|k| k == kind) { p.boss_kinds_defeated.push(kind.to_string()); }
        if losses == 0 && !p.untouchable_kinds.iter().any(|k| k == kind) { p.untouchable_kinds.push(kind.to_string()); }
        drop(p);
        crate::profile::save_profile();
    }
    if losses == 0 { unlock("untouchable"); }
    if secs <= 60.0 { unlock("speed_date"); }
    if bosses >= 3 { unlock("hat_trick"); }
}

/// The run ended. `cause` is "sun", "oxygen" or "fall"; `in_fight` when a
/// boss was up; `eclipse` when one was on its way.
pub fn on_death(c: &mut Canvas, cause: &str, in_fight: bool, eclipse: bool) {
    let t = run();
    unlock(match cause { "sun" => "die_sun", "oxygen" => "die_oxygen", _ => "die_fall" });
    if in_fight { unlock("slain_by_boss"); }
    if eclipse && !in_fight { unlock("so_close"); }
    if t.tick <= 20 * 60 { unlock("floor_is_lava"); }
    if t.duck_seen_at.is_some_and(|d| t.tick - d <= 30 * 60) { unlock("quack_attack"); }
    // Bank the run's grabs on the profile.
    {
        let g = crate::profile::profile();
        let mut p = g.lock().unwrap();
        p.hooks_grabbed = p.hooks_grabbed.saturating_add(t.grabs);
    }
    track().lock().unwrap().grabs = 0;
    crate::profile::save_profile();
    check_achievements(c, 0, 0.0);
}

/// A rare piece of decor came into view (its `DecorKind::name`).
pub fn on_rare_seen(name: &str) {
    let id = match name {
        "rubber duck" => "seen_duck",
        "space whale" => "seen_whale",
        "UFO" => "seen_ufo",
        "astronaut" => "seen_astronaut",
        "space jellyfish" => "seen_jelly",
        _ => return,
    };
    if id == "seen_duck" { let mut t = track().lock().unwrap(); t.duck_seen_at = Some(t.tick); }
    {
        let g = crate::profile::profile();
        let mut p = g.lock().unwrap();
        if !p.rare_seen.iter().any(|k| k == name) { p.rare_seen.push(name.to_string()); }
    }
    crate::profile::save_profile();
    unlock(id);
    if stats(0, 0.0).rare_seen >= 5 { unlock("tourist_trap"); }
}

/// A solar flare ended with the player alive.
pub fn record_flare_survived(c: &mut Canvas) {
    crate::profile::record_flare_survived();
    check_achievements(c, 0, 0.0);
}

/// The tutorial was finished.
pub fn on_tutorial_done() {
    unlock("graduate");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_legacy_ids_are_not_reused() {
        let mut seen = std::collections::HashSet::new();
        for a in ACHIEVEMENTS {
            assert!(seen.insert(a.id), "duplicate id {}", a.id);
            assert!(!a.name.is_empty() && !a.desc.is_empty(), "{}", a.id);
        }
        for (id, _, _) in LEGACY {
            assert!(!seen.contains(id), "legacy id {id} reused");
        }
    }

    #[test]
    fn stat_triggers_fire_at_their_thresholds() {
        let mut s = Stats::default();
        let earned = |s: &Stats| -> Vec<&str> {
            ACHIEVEMENTS.iter().filter_map(|a| match a.trigger { Trigger::Stat(f) if f(s) => Some(a.id), _ => None }).collect()
        };
        assert!(earned(&s).is_empty(), "a fresh profile earns nothing");
        s.best_distance = 100_000.0;
        assert_eq!(earned(&s), vec!["litter_box"]);
        s.bosses_defeated = 1;
        assert!(earned(&s).contains(&"boss_slayer"));
    }
}
