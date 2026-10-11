// ── profile.rs — multi-slot player profiles (meta + permanent upgrades) ──────
//
// Each profile is a small text file `saves/profile_<N>.txt` (no serde). It holds
// the per-player state that persists across runs: tutorial completion, unlocked
// achievements, the meta (and future premium) currency balances, and permanent
// upgrades bought with meta. Profiles are selected before the menu (see
// `menu::build_profile_scene`), and the chosen slot becomes the active profile
// behind `profile()`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// How many profile slots are available.
pub const SLOT_COUNT: usize = 4;

/// Directory saves are read from / written to: the platform's app-data
/// folder (see `default_saves_dir`). Redirected in the headless driver so
/// automated runs never clobber the real save slots.
static SAVES_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Override the saves directory (e.g. a temp dir for headless runs). Only the
/// first call wins.
pub fn set_saves_dir(dir: &str) {
    let _ = SAVES_DIR.set(PathBuf::from(dir));
}

fn saves_dir() -> &'static PathBuf {
    SAVES_DIR.get_or_init(default_saves_dir)
}

/// Where a player's saves live: the platform's own place for app data, not
/// the folder the game happened to be launched from. A relative `saves/`
/// landed in the player's home folder when the game was double-clicked on
/// a Mac, and anywhere at all (maybe somewhere unwritable) on Windows.
///   * macOS: ~/Library/Application Support/BallSwing/saves
///   * Windows: %APPDATA%\BallSwing\saves
///   * Linux: $XDG_DATA_HOME/BallSwing/saves, else ~/.local/share/BallSwing/saves
///   * anything else (Android): `saves`, as before.
/// Saves already in a relative `saves/` (where earlier builds wrote them)
/// are copied over the first time, so nobody loses their slots.
fn default_saves_dir() -> PathBuf {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty());
    let base: Option<PathBuf> = if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library").join("Application Support"))
    } else if cfg!(target_os = "windows") {
        env("APPDATA")
    } else if cfg!(target_os = "linux") {
        env("XDG_DATA_HOME").or_else(|| env("HOME").map(|h| h.join(".local").join("share")))
    } else {
        None
    };
    let Some(base) = base else { return PathBuf::from("saves") };
    let dir = base.join("BallSwing").join("saves");
    migrate_old_saves(Path::new("saves"), &dir);
    dir
}

/// Copy `profile_*.txt` from `old` into `new`, once: only when `new` holds
/// no profiles yet. Copied, not moved, so an older build still finds its own.
fn migrate_old_saves(old: &Path, new: &Path) {
    let has_profiles = |d: &Path| std::fs::read_dir(d).map(|rd| rd.flatten().any(|e| {
        e.file_name().to_string_lossy().starts_with("profile_")
    })).unwrap_or(false);
    if has_profiles(new) || !has_profiles(old) { return; }
    if std::fs::create_dir_all(new).is_err() { return; }
    if let Ok(rd) = std::fs::read_dir(old) {
        for e in rd.flatten() {
            let name = e.file_name();
            if name.to_string_lossy().starts_with("profile_") {
                let _ = std::fs::copy(e.path(), new.join(&name));
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlayerProfile {
    /// Display name for the slot.
    pub name: String,
    /// Whether the player has already clicked through the tutorial.
    pub tutorial_done: bool,
    /// Special currency earned at end of each run; spent on permanent upgrades.
    pub meta_currency: u64,
    /// Reserved for a future premium currency (kept here so saves are stable).
    pub premium_currency: u64,
    /// Permanent extra hearts owned. Predates `perm_levels` and keeps its own
    /// save key so existing profiles do not lose what they already bought;
    /// `upgrade_level("heart")` reads through to it.
    pub permanent_extra_hearts: u32,
    /// Every other permanent upgrade, as (id, ranks owned). A Vec rather than a
    /// map so the save file has a stable order and diffs cleanly.
    pub perm_levels: Vec<(String, u32)>,
    /// Achievement ids already unlocked (so they don't re-unlock).
    pub achievements: Vec<String>,
    /// Selected cosmetics (char / rope / bg / trail), persisted per profile.
    pub cosmetic_char: u32,
    pub cosmetic_rope: u32,
    pub cosmetic_bg: u32,
    pub cosmetic_trail: u32,
    /// Rope STYLE (cosmetics::ROPE_STYLES index); `cosmetic_rope` is its
    /// colour, chosen separately.
    pub cosmetic_rope_style: u32,
    /// Premium looks bought on this profile, as keys ("rope:helix",
    /// "cat:ginger"). Free items are never listed.
    pub owned_cosmetics: Vec<String>,
    /// Lifetime stats, tracked per profile for the Stats screen.
    pub best_distance: u32,
    pub total_coins: u64,
    pub deaths: u32,
    pub bosses_defeated: u32,
    pub runs_played: u32,
    pub hooks_grabbed: u64,
    pub powerups_collected: u32,
    pub time_played_seconds: u64,
    /// Per-mode best distance (px) and run counts, for mode-specific stats.
    pub best_distance_casual: u32,
    pub best_distance_normal: u32,
    pub best_distance_bossrush: u32,
    pub runs_casual: u32,
    pub runs_normal: u32,
    pub runs_bossrush: u32,
    /// Solar flares lived through (the run's own flares), lifetime.
    pub flares_survived: u32,
    /// The active ability taken into runs (an `ABILITIES` id), or empty.
    /// Ranks owned of each ability live in `perm_levels` beside the upgrades.
    pub equipped_ability: String,
    /// Most coins held at the end of one run.
    pub best_run_coins: u64,
    /// Boss slugs defeated at least once, and those beaten without losing a
    /// heart (achievements).
    pub boss_kinds_defeated: Vec<String>,
    pub untouchable_kinds: Vec<String>,
    /// Rare decor seen (`DecorKind::name`s).
    pub rare_seen: Vec<String>,
    /// Daily reward: the day (days since the Unix epoch, local clock) it was
    /// last claimed, and the run of consecutive days.
    pub daily_last_claim: i64,
    pub daily_streak: u32,
    /// Daily challenge: the day last completed, and how many in all.
    pub daily_done_day: i64,
    pub dailies_done: u32,
    /// Best distance in today's daily challenge, and the day it is for.
    pub daily_best: u32,
    pub daily_best_day: i64,
    /// Graphics quality: 0 for the platform's default, else 1 + a
    /// `graphics::Quality` index.
    pub graphics_quality: i32,
    /// Effect switches turned OFF in settings (`graphics::FX_*` bits).
    pub fx_off: u32,
}

/// A comma list from the save file.
fn list(v: &str) -> Vec<String> {
    v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

impl PlayerProfile {
    fn path(idx: usize) -> PathBuf {
        saves_dir().join(format!("profile_{idx}.txt"))
    }

    fn load(idx: usize) -> Self {
        let mut p = PlayerProfile::default();
        p.name = format!("Player {}", idx + 1);
        if let Ok(text) = std::fs::read_to_string(Self::path(idx)) {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let k = k.trim();
                    let v = v.trim();
                    match k {
                        "name" => {
                            // Keep the "Player N" default when a stale save has
                            // an empty name, so the slot never shows blank.
                            if !v.is_empty() { p.name = v.to_string(); }
                        }
                        "tutorial_done" => p.tutorial_done = v == "1" || v.eq_ignore_ascii_case("true"),
                        "meta_currency" => p.meta_currency = v.parse().unwrap_or(0),
                        "premium_currency" => p.premium_currency = v.parse().unwrap_or(0),
                        "permanent_extra_hearts" => p.permanent_extra_hearts = v.parse().unwrap_or(0),
                        // perm_<id>=<level>. Unknown ids are dropped on load
                        // rather than kept, so retiring an upgrade cleans up.
                        _ if k.starts_with("perm_") => {
                            let id = &k[5..];
                            if upgrade_by_id(id).is_some() || ability_by_id(id).is_some() {
                                p.perm_levels.push((id.to_string(), v.parse().unwrap_or(0)));
                            }
                        }
                        "achievements" => {
                            p.achievements = v.split(',').map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        "cosmetic_char" => p.cosmetic_char = v.parse().unwrap_or(0),
                        "cosmetic_rope" => p.cosmetic_rope = v.parse().unwrap_or(0),
                        "cosmetic_bg" => p.cosmetic_bg = v.parse().unwrap_or(0),
                        "cosmetic_trail" => p.cosmetic_trail = v.parse().unwrap_or(0),
                        "cosmetic_rope_style" => p.cosmetic_rope_style = v.parse().unwrap_or(0),
                        "owned_cosmetics" => {
                            p.owned_cosmetics = v.split(',').map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        "best_distance" => p.best_distance = v.parse().unwrap_or(0),
                        "total_coins" => p.total_coins = v.parse().unwrap_or(0),
                        "deaths" => p.deaths = v.parse().unwrap_or(0),
                        "bosses_defeated" => p.bosses_defeated = v.parse().unwrap_or(0),
                        "runs_played" => p.runs_played = v.parse().unwrap_or(0),
                        "hooks_grabbed" => p.hooks_grabbed = v.parse().unwrap_or(0),
                        "powerups_collected" => p.powerups_collected = v.parse().unwrap_or(0),
                        "time_played_seconds" => p.time_played_seconds = v.parse().unwrap_or(0),
                        "best_distance_casual" => p.best_distance_casual = v.parse().unwrap_or(0),
                        "best_distance_normal" => p.best_distance_normal = v.parse().unwrap_or(0),
                        "best_distance_bossrush" => p.best_distance_bossrush = v.parse().unwrap_or(0),
                        "runs_casual" => p.runs_casual = v.parse().unwrap_or(0),
                        "runs_normal" => p.runs_normal = v.parse().unwrap_or(0),
                        "runs_bossrush" => p.runs_bossrush = v.parse().unwrap_or(0),
                        "flares_survived" => p.flares_survived = v.parse().unwrap_or(0),
                        "equipped_ability" => p.equipped_ability = v.to_string(),
                        "best_run_coins" => p.best_run_coins = v.parse().unwrap_or(0),
                        "boss_kinds_defeated" => p.boss_kinds_defeated = list(v),
                        "untouchable_kinds" => p.untouchable_kinds = list(v),
                        "rare_seen" => p.rare_seen = list(v),
                        "daily_last_claim" => p.daily_last_claim = v.parse().unwrap_or(0),
                        "daily_streak" => p.daily_streak = v.parse().unwrap_or(0),
                        "daily_done_day" => p.daily_done_day = v.parse().unwrap_or(0),
                        "dailies_done" => p.dailies_done = v.parse().unwrap_or(0),
                        "daily_best" => p.daily_best = v.parse().unwrap_or(0),
                        "daily_best_day" => p.daily_best_day = v.parse().unwrap_or(0),
                        "graphics_quality" => p.graphics_quality = v.parse().unwrap_or(0),
                        "fx_off" => p.fx_off = v.parse().unwrap_or(0),
                        _ => {}
                    }
                }
            }
        }
        p
    }

    fn save(&self, idx: usize) {
        let path = Self::path(idx);
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(dir);
            }
        }
        let mut body = format!(
            "# FlowMake ball_swing profile slot {}\nname={}\ntutorial_done={}\nmeta_currency={}\npremium_currency={}\npermanent_extra_hearts={}\nachievements={}\ncosmetic_char={}\ncosmetic_rope={}\ncosmetic_bg={}\ncosmetic_trail={}\nbest_distance={}\ntotal_coins={}\ndeaths={}\nbosses_defeated={}\nruns_played={}\nhooks_grabbed={}\npowerups_collected={}\ntime_played_seconds={}\nbest_distance_casual={}\nbest_distance_normal={}\nbest_distance_bossrush={}\nruns_casual={}\nruns_normal={}\nruns_bossrush={}\n",
            idx, self.name,
            if self.tutorial_done { "1" } else { "0" },
            self.meta_currency, self.premium_currency, self.permanent_extra_hearts,
            self.achievements.join(","),
            self.cosmetic_char, self.cosmetic_rope, self.cosmetic_bg, self.cosmetic_trail,
            self.best_distance, self.total_coins, self.deaths, self.bosses_defeated,
            self.runs_played, self.hooks_grabbed, self.powerups_collected, self.time_played_seconds,
            self.best_distance_casual, self.best_distance_normal, self.best_distance_bossrush,
            self.runs_casual, self.runs_normal, self.runs_bossrush,
        );
        body.push_str(&format!(
            "cosmetic_rope_style={}\nowned_cosmetics={}\nflares_survived={}\nequipped_ability={}\n",
            self.cosmetic_rope_style,
            self.owned_cosmetics.join(","),
            self.flares_survived,
            self.equipped_ability,
        ));
        body.push_str(&format!(
            "best_run_coins={}\nboss_kinds_defeated={}\nuntouchable_kinds={}\nrare_seen={}\ndaily_last_claim={}\ndaily_streak={}\ndaily_done_day={}\ndailies_done={}\ndaily_best={}\ndaily_best_day={}\n",
            self.best_run_coins,
            self.boss_kinds_defeated.join(","),
            self.untouchable_kinds.join(","),
            self.rare_seen.join(","),
            self.daily_last_claim, self.daily_streak,
            self.daily_done_day, self.dailies_done,
            self.daily_best, self.daily_best_day,
        ));
        body.push_str(&format!("graphics_quality={}\nfx_off={}\n", self.graphics_quality, self.fx_off));
        for a in ABILITIES {
            let level = self.upgrade_level(a.id);
            if level > 0 {
                body.push_str(&format!("perm_{}={}\n", a.id, level));
            }
        }
        // Written in table order, not insertion order, so the file is stable.
        for u in PERM_UPGRADES {
            if u.id == "heart" {
                continue;
            }
            let level = self.upgrade_level(u.id);
            if level > 0 {
                body.push_str(&format!("perm_{}={}\n", u.id, level));
            }
        }
        if let Ok(mut f) = std::fs::File::create(&path) {
            let _ = f.write_all(body.as_bytes());
        }
    }
}

// ── Active profile ────────────────────────────────────────────────────────────

static ACTIVE_PROFILE: OnceLock<Arc<Mutex<PlayerProfile>>> = OnceLock::new();
static ACTIVE_INDEX: OnceLock<Mutex<usize>> = OnceLock::new();

/// The active (selected) profile. Callers mutate this directly.
pub fn profile() -> Arc<Mutex<PlayerProfile>> {
    ACTIVE_PROFILE
        .get_or_init(|| Arc::new(Mutex::new(PlayerProfile::default())))
        .clone()
}

/// Index of the currently active profile slot.
pub fn active_index() -> usize {
    *ACTIVE_INDEX.get_or_init(|| Mutex::new(0)).lock().unwrap()
}

/// Select a slot: copy its data into the active profile and record the index.
pub fn select_profile(idx: usize) {
    let idx = idx.min(SLOT_COUNT - 1);
    let p = PlayerProfile::load(idx);
    *profile().lock().unwrap() = p;
    *ACTIVE_INDEX.get_or_init(|| Mutex::new(0)).lock().unwrap() = idx;
    crate::graphics::load_from_profile();
}

/// Number of profile slots.
pub fn slot_count() -> usize {
    SLOT_COUNT
}

/// All profile slots (for the selection screen).
pub fn all_profiles() -> Vec<PlayerProfile> {
    (0..SLOT_COUNT).map(PlayerProfile::load).collect()
}

/// Whether a slot has ever been saved (has data on disk).
pub fn slot_exists(idx: usize) -> bool {
    PlayerProfile::path(idx).exists()
}

/// Delete a profile slot: remove its save file and, if it was the active slot,
/// reset the in-memory active profile so the next run starts fresh. Returns true
/// if a save file was actually removed.
pub fn delete_profile(idx: usize) -> bool {
    let idx = idx.min(SLOT_COUNT - 1);
    let path = PlayerProfile::path(idx);
    let existed = path.exists();
    let _ = std::fs::remove_file(&path);
    if active_index() == idx {
        *profile().lock().unwrap() = PlayerProfile::default();
    }
    existed
}

/// Persist the active profile to its slot.
pub fn save_profile() {
    let idx = active_index();
    let g = profile();
    g.lock().unwrap().save(idx);
}

/// Record a finished run's lifetime stats on the active profile and persist.
/// `mode_idx` is the GameMode::index() of the run (0 casual, 1 normal, 2 boss rush).
pub fn record_run(mode_idx: i32, distance_px: f32, coins_on_hand: u64, seconds: u64) {
    let g = profile();
    let mut p = g.lock().unwrap();
    let d = distance_px.max(0.0) as u32;
    if d > p.best_distance { p.best_distance = d; }
    match mode_idx {
        0 => { if d > p.best_distance_casual { p.best_distance_casual = d; } p.runs_casual += 1; }
        2 => { if d > p.best_distance_bossrush { p.best_distance_bossrush = d; } p.runs_bossrush += 1; }
        _ => { if d > p.best_distance_normal { p.best_distance_normal = d; } p.runs_normal += 1; }
    }
    p.total_coins = p.total_coins.saturating_add(coins_on_hand);
    p.best_run_coins = p.best_run_coins.max(coins_on_hand);
    p.runs_played += 1;
    p.time_played_seconds = p.time_played_seconds.saturating_add(seconds);
    p.save(active_index());
}

/// Increment the active profile's death counter.
pub fn record_death() {
    let g = profile();
    let mut p = g.lock().unwrap();
    p.deaths += 1;
    p.save(active_index());
}

/// Count one solar flare survived; returns the new lifetime total.
pub fn record_flare_survived() -> u32 {
    let g = profile();
    let mut p = g.lock().unwrap();
    p.flares_survived = p.flares_survived.saturating_add(1);
    p.save(active_index());
    p.flares_survived
}

/// Increment the active profile's boss-defeated counter.
pub fn record_boss_defeated() {
    let g = profile();
    let mut p = g.lock().unwrap();
    p.bosses_defeated += 1;
    p.save(active_index());
}

// ── Meta / upgrades (operate on the active profile) ─────────────────────────

/// Award meta currency (called at end of a run) and persist.
pub fn award_meta_currency(amount: u64) {
    let g = profile();
    {
        let mut p = g.lock().unwrap();
        p.meta_currency = p.meta_currency.saturating_add(amount);
    }
    g.lock().unwrap().save(active_index());
}

// ── Achievement helpers ───────────────────────────────────────────────────────

/// Save the currently-selected cosmetics to the active profile.
pub fn save_cosmetics(char: u32, rope: u32, bg: u32, trail: u32) {
    let g = profile();
    let mut p = g.lock().unwrap();
    p.cosmetic_char = char;
    p.cosmetic_rope = rope;
    p.cosmetic_bg = bg;
    p.cosmetic_trail = trail;
    p.save(active_index());
}

/// Save the selected rope STYLE (its colour goes through `save_cosmetics`).
pub fn save_rope_style(style: u32) {
    let g = profile();
    let mut p = g.lock().unwrap();
    p.cosmetic_rope_style = style;
    p.save(active_index());
}

/// Whether the active profile owns the premium look `key`.
pub fn owns_cosmetic(key: &str) -> bool {
    let g = profile();
    let owned = g.lock().unwrap().owned_cosmetics.iter().any(|k| k == key);
    owned
}

/// Buy `key` for `cost` META on the active profile. `Err(shortfall)` when
/// the balance is short; already owned is `Ok` and charges nothing.
pub fn buy_cosmetic(key: &str, cost: u64) -> Result<(), u64> {
    let g = profile();
    let mut p = g.lock().unwrap();
    if p.owned_cosmetics.iter().any(|k| k == key) {
        return Ok(());
    }
    if p.meta_currency < cost {
        return Err(cost - p.meta_currency);
    }
    p.meta_currency -= cost;
    p.owned_cosmetics.push(key.to_string());
    p.save(active_index());
    Ok(())
}

/// Record an achievement as unlocked on the active profile (no-op if present).
pub fn unlock_achievement_on_profile(id: &str) {
    let g = profile();
    let mut p = g.lock().unwrap();
    if !p.achievements.iter().any(|a| a == id) {
        p.achievements.push(id.to_string());
    }
    p.save(active_index());
}

/// Whether the active profile has already unlocked this achievement.
pub fn profile_has_achievement(id: &str) -> bool {
    let g = profile();
    let has = g.lock().unwrap().achievements.iter().any(|a| a == id);
    has
}

// ── Permanent upgrades (the meta / roguelike loop) ───────────────────────────
//
// One table drives everything: the shop cards, the cost curve, the save format
// and the per-run application in `build_scene`. Adding an upgrade is one row
// here plus one arm in `apply_permanent_upgrades`.
//
// Costs are exponential in the number already owned, so early ranks are
// reachable inside a few runs and the last rank is a long-term goal. `max`
// keeps every upgrade from becoming mandatory: a fully-bought profile is
// stronger, never invincible.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PermUpgrade {
    /// Stable save key. Never rename — it is written into profile files.
    pub id: &'static str,
    pub name: &'static str,
    /// One line, shown under the card. Says what the rank does, in the units
    /// the player sees.
    pub blurb: &'static str,
    pub max: u32,
    pub base_cost: u64,
    pub growth: f32,
    /// Card colour in the shop carousel.
    pub color: (u8, u8, u8),
}

pub const PERM_UPGRADES: &[PermUpgrade] = &[
    PermUpgrade {
        id: "heart",
        name: "VITALITY",
        blurb: "+1 HEART EACH RUN",
        max: 4,
        base_cost: 150,
        growth: 2.1,
        color: (232, 72, 92),
    },
    PermUpgrade {
        id: "reach",
        name: "LONG LINE",
        blurb: "+6% TETHER REACH",
        max: 5,
        base_cost: 120,
        growth: 1.85,
        color: (96, 196, 255),
    },
    PermUpgrade {
        id: "momentum",
        name: "FLYWHEEL",
        blurb: "+4% TOP SPEED",
        max: 5,
        base_cost: 130,
        growth: 1.85,
        color: (255, 176, 64),
    },
    PermUpgrade {
        id: "magnet",
        name: "MAGNETISM",
        blurb: "+18% COIN PICKUP RANGE",
        max: 4,
        base_cost: 90,
        growth: 1.7,
        color: (196, 128, 255),
    },
    PermUpgrade {
        id: "purse",
        name: "SEED FUNDS",
        blurb: "START EACH RUN WITH 25 COINS",
        max: 4,
        base_cost: 110,
        growth: 1.8,
        color: (255, 226, 96),
    },
    PermUpgrade {
        id: "flareward",
        name: "SUNPROOFING",
        blurb: "SHRUG OFF 1 FLARE TICK PER FLARE",
        max: 3,
        base_cost: 260,
        growth: 2.4,
        color: (255, 138, 48),
    },
    PermUpgrade {
        id: "secondwind",
        name: "SECOND WIND",
        blurb: "SURVIVE LOSING YOUR LAST HEART ONCE",
        max: 2,
        base_cost: 400,
        growth: 2.6,
        color: (128, 255, 188),
    },
];

// ── Abilities (active powers, one equipped at a time) ────────────────────────
//
// Bought in the shop's ABILITIES category: rank 1 unlocks one, each further
// rank upgrades it. One is equipped and taken into every run, where the
// ability button (or E) fires it — see `scenes::game::abilities`.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ability {
    /// Stable save key (written as `perm_<id>`). Never rename.
    pub id: &'static str,
    pub name: &'static str,
    /// What it does, one line, for the shop.
    pub blurb: &'static str,
    /// What each upgrade rank adds, one line.
    pub per_rank: &'static str,
    pub max: u32,
    /// Cost of rank 1 (the unlock); rank n costs `unlock * growth^(n-1)`.
    pub unlock_cost: u64,
    pub growth: f32,
    pub color: (u8, u8, u8),
}

/// In price order: Momentum Burst < Alternating Swing < Omni Boost.
pub const ABILITIES: &[Ability] = &[
    Ability {
        id: "ab_burst", name: "MOMENTUM BURST",
        blurb: "BOOST YOUR CURRENT MOMENTUM WHEN THE BAR IS FULL",
        per_rank: "STRONGER PUSH, FASTER BAR",
        max: 4, unlock_cost: 500, growth: 1.6, color: (255, 150, 60),
    },
    Ability {
        id: "ab_swap", name: "ALTERNATING SWING",
        blurb: "FLIP THE DIRECTION YOU'RE SWINGING",
        per_rank: "+1 USE PER RUN",
        max: 4, unlock_cost: 850, growth: 1.6, color: (90, 220, 200),
    },
    Ability {
        id: "ab_omni", name: "OMNI BOOST",
        blurb: "BOOST IN ANY DIRECTION YOU AIM",
        per_rank: "+1 USE PER RUN",
        max: 4, unlock_cost: 1300, growth: 1.6, color: (190, 120, 255),
    },
];

pub fn ability_by_id(id: &str) -> Option<&'static Ability> {
    ABILITIES.iter().find(|a| a.id == id)
}

/// Cost of the NEXT rank of `a` given the ranks owned; `None` when maxed.
pub fn ability_cost(a: &Ability, owned: u32) -> Option<u64> {
    if owned >= a.max { return None; }
    Some((a.unlock_cost as f64 * (a.growth as f64).powi(owned as i32)).round() as u64)
}

/// Ranks owned of an ability (0 = locked).
pub fn ability_level(id: &str) -> u32 {
    let g = profile();
    let l = g.lock().unwrap().upgrade_level(id);
    l
}

/// The equipped ability and its rank, if one is equipped and owned.
pub fn equipped_ability() -> Option<(&'static Ability, u32)> {
    let g = profile();
    let p = g.lock().unwrap();
    let a = ability_by_id(&p.equipped_ability)?;
    let rank = p.upgrade_level(a.id);
    (rank > 0).then_some((a, rank))
}

/// Buy the next rank of `id` (rank 1 unlocks it). Unlocking also equips it.
/// `Ok(new_rank)` or the reason for the shop to show.
pub fn buy_ability_rank(id: &str) -> Result<u32, String> {
    let Some(a) = ability_by_id(id) else { return Err("UNKNOWN ABILITY".into()) };
    let g = profile();
    let mut p = g.lock().unwrap();
    let owned = p.upgrade_level(id);
    let Some(cost) = ability_cost(a, owned) else { return Err("ALREADY MAXED".into()) };
    if p.meta_currency < cost {
        return Err(format!("NEED {} MORE META", cost - p.meta_currency));
    }
    p.meta_currency -= cost;
    p.set_upgrade_level(id, owned + 1);
    if owned == 0 { p.equipped_ability = id.to_string(); }
    p.save(active_index());
    Ok(owned + 1)
}

/// Equip an owned ability.
pub fn equip_ability(id: &str) {
    let g = profile();
    let mut p = g.lock().unwrap();
    if p.upgrade_level(id) > 0 {
        p.equipped_ability = id.to_string();
        p.save(active_index());
    }
}

pub fn upgrade_by_id(id: &str) -> Option<&'static PermUpgrade> {
    PERM_UPGRADES.iter().find(|u| u.id == id)
}

/// Cost of the NEXT rank of `u` given how many are already owned.
/// Returns `None` when the upgrade is maxed.
pub fn upgrade_cost(u: &PermUpgrade, owned: u32) -> Option<u64> {
    if owned >= u.max {
        return None;
    }
    Some((u.base_cost as f64 * (u.growth as f64).powi(owned as i32)).round() as u64)
}

impl PlayerProfile {
    /// Ranks owned of a permanent upgrade.
    pub fn upgrade_level(&self, id: &str) -> u32 {
        // `heart` predates the table and has its own save key, so it keeps
        // reading from the old field — otherwise every existing profile would
        // silently lose the hearts it had already bought.
        if id == "heart" {
            return self.permanent_extra_hearts;
        }
        self.perm_levels
            .iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| *v)
            .unwrap_or(0)
    }

    fn set_upgrade_level(&mut self, id: &str, level: u32) {
        if id == "heart" {
            self.permanent_extra_hearts = level;
            return;
        }
        if let Some(entry) = self.perm_levels.iter_mut().find(|(k, _)| k == id) {
            entry.1 = level;
        } else {
            self.perm_levels.push((id.to_string(), level));
        }
    }
}

/// Attempt to buy one rank of `id` with meta currency.
/// Returns `Ok(new_level)`, or `Err(reason)` for the shop to display.
pub fn buy_permanent_upgrade(id: &str) -> Result<u32, &'static str> {
    let Some(u) = upgrade_by_id(id) else { return Err("UNKNOWN UPGRADE") };
    let g = profile();
    let mut p = g.lock().unwrap();
    let owned = p.upgrade_level(id);
    let Some(cost) = upgrade_cost(u, owned) else { return Err("ALREADY MAXED") };
    if p.meta_currency < cost {
        return Err("NOT ENOUGH META");
    }
    p.meta_currency -= cost;
    let next = owned + 1;
    p.set_upgrade_level(id, next);
    p.save(active_index());
    Ok(next)
}

/// Snapshot of every owned rank, for applying at run start.
pub fn permanent_levels() -> Vec<(&'static str, u32)> {
    let g = profile();
    let p = g.lock().unwrap();
    PERM_UPGRADES.iter().map(|u| (u.id, p.upgrade_level(u.id))).collect()
}

/// Resolved permanent bonuses for a fresh run.
///
/// Computed once at run start so a run plays by the ranks it began with, and so
/// gameplay never has to lock the profile mutex mid-frame.
#[derive(Clone, Copy, Debug)]
pub struct PermBonuses {
    pub extra_hearts: i32,
    pub reach_mult: f32,
    pub momentum_mult: f32,
    pub magnet_mult: f32,
    pub start_coins: u32,
    pub flare_wards: u32,
    pub free_respawns: u32,
}

impl Default for PermBonuses {
    fn default() -> Self {
        Self {
            extra_hearts: 0,
            reach_mult: 1.0,
            momentum_mult: 1.0,
            magnet_mult: 1.0,
            start_coins: 0,
            flare_wards: 0,
            free_respawns: 0,
        }
    }
}

/// Per-rank strengths. Kept beside the table they belong to, so a blurb and the
/// number it promises cannot drift apart in different files.
const REACH_PER_RANK: f32 = 0.06;
const MOMENTUM_PER_RANK: f32 = 0.04;
const MAGNET_PER_RANK: f32 = 0.18;
const COINS_PER_RANK: u32 = 25;

pub fn permanent_bonuses() -> PermBonuses {
    let g = profile();
    let p = g.lock().unwrap();
    let lvl = |id: &str| p.upgrade_level(id) as f32;
    PermBonuses {
        extra_hearts: p.upgrade_level("heart") as i32,
        reach_mult: 1.0 + REACH_PER_RANK * lvl("reach"),
        momentum_mult: 1.0 + MOMENTUM_PER_RANK * lvl("momentum"),
        magnet_mult: 1.0 + MAGNET_PER_RANK * lvl("magnet"),
        start_coins: p.upgrade_level("purse") * COINS_PER_RANK,
        flare_wards: p.upgrade_level("flareward"),
        free_respawns: p.upgrade_level("secondwind"),
    }
}

#[cfg(test)]
mod saves_dir_tests {
    use super::*;

    /// Old relative saves are copied into the new folder once, and never
    /// over slots that are already there.
    #[test]
    fn old_saves_are_copied_over_once() {
        let root = std::env::temp_dir().join(format!("bs_migrate_{}", std::process::id()));
        let (old, new) = (root.join("old"), root.join("new"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("profile_0.txt"), "name=A").unwrap();
        std::fs::write(old.join("notes.txt"), "x").unwrap();
        migrate_old_saves(&old, &new);
        assert_eq!(std::fs::read_to_string(new.join("profile_0.txt")).unwrap(), "name=A");
        assert!(!new.join("notes.txt").exists(), "only profiles are copied");
        assert!(old.join("profile_0.txt").exists(), "copied, not moved");
        // A second run with newer saves in place leaves them alone.
        std::fs::write(new.join("profile_0.txt"), "name=B").unwrap();
        migrate_old_saves(&old, &new);
        assert_eq!(std::fs::read_to_string(new.join("profile_0.txt")).unwrap(), "name=B");
        let _ = std::fs::remove_dir_all(&root);
    }
}
