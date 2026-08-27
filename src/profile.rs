// ── profile.rs — multi-slot player profiles (meta + permanent upgrades) ──────
//
// Each profile is a small text file `saves/profile_<N>.txt` (no serde). It holds
// the per-player state that persists across runs: tutorial completion, unlocked
// achievements, the meta (and future premium) currency balances, and permanent
// upgrades bought with meta. Profiles are selected before the menu (see
// `menu::build_profile_scene`), and the chosen slot becomes the active profile
// behind `profile()`.

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

/// How many profile slots are available.
pub const SLOT_COUNT: usize = 4;

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
    /// Permanent extra hearts owned (persists across restarts; grinded for).
    pub permanent_extra_hearts: u32,
    /// Achievement ids already unlocked (so they don't re-unlock).
    pub achievements: Vec<String>,
    /// Selected cosmetics (char / rope / bg / trail), persisted per profile.
    pub cosmetic_char: u32,
    pub cosmetic_rope: u32,
    pub cosmetic_bg: u32,
    pub cosmetic_trail: u32,
}

impl PlayerProfile {
    fn path(idx: usize) -> PathBuf {
        PathBuf::from(format!("saves/profile_{idx}.txt"))
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
                        "name" => p.name = v.to_string(),
                        "tutorial_done" => p.tutorial_done = v == "1" || v.eq_ignore_ascii_case("true"),
                        "meta_currency" => p.meta_currency = v.parse().unwrap_or(0),
                        "premium_currency" => p.premium_currency = v.parse().unwrap_or(0),
                        "permanent_extra_hearts" => p.permanent_extra_hearts = v.parse().unwrap_or(0),
                        "achievements" => {
                            p.achievements = v.split(',').map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        "cosmetic_char" => p.cosmetic_char = v.parse().unwrap_or(0),
                        "cosmetic_rope" => p.cosmetic_rope = v.parse().unwrap_or(0),
                        "cosmetic_bg" => p.cosmetic_bg = v.parse().unwrap_or(0),
                        "cosmetic_trail" => p.cosmetic_trail = v.parse().unwrap_or(0),
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
        let body = format!(
            "# FlowMake ball_swing profile slot {}\nname={}\ntutorial_done={}\nmeta_currency={}\npremium_currency={}\npermanent_extra_hearts={}\nachievements={}\ncosmetic_char={}\ncosmetic_rope={}\ncosmetic_bg={}\ncosmetic_trail={}\n",
            idx, self.name,
            if self.tutorial_done { "1" } else { "0" },
            self.meta_currency, self.premium_currency, self.permanent_extra_hearts,
            self.achievements.join(","),
            self.cosmetic_char, self.cosmetic_rope, self.cosmetic_bg, self.cosmetic_trail,
        );
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

/// Persist the active profile to its slot.
pub fn save_profile() {
    let idx = active_index();
    let g = profile();
    g.lock().unwrap().save(idx);
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

/// Current permanent-extra-heart price in meta currency (exponential).
pub fn permanent_heart_cost(owned: u32) -> u64 {
    (crate::constants::UPGRADE_PERM_HEART_BASE as f64
        * (crate::constants::UPGRADE_PERM_HEART_GROWTH as f64).powi(owned as i32))
        .round() as u64
}

/// Attempt to buy one permanent extra heart with meta currency. Returns true on
/// success (deducts, increments owned, persists).
pub fn buy_permanent_heart() -> bool {
    let g = profile();
    let mut p = g.lock().unwrap();
    let cost = permanent_heart_cost(p.permanent_extra_hearts);
    if p.meta_currency < cost {
        return false;
    }
    p.meta_currency -= cost;
    p.permanent_extra_hearts += 1;
    p.save(active_index());
    true
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
