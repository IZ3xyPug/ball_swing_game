use quartz::*;

pub const GOLD_MASTER_TITLE: &str = "gold master!";
pub const GOLD_MASTER_DESCRIPTION: &str = "collect 10 coins";

pub const GOLD_MASTER_UNLOCKED_VAR: &str = "achievement_gold_master_unlocked";
pub const TOTAL_COINS_COLLECTED_VAR: &str = "total_coins_collected";
pub const GOLD_MASTER_TOAST_ACTIVE_VAR: &str = "achievement_gold_master_toast_active";
pub const GOLD_MASTER_TOAST_TICKS_VAR: &str = "achievement_gold_master_toast_ticks";

pub const GOLD_MASTER_TOAST_PANEL_NAME: &str = "achievement_gold_master_toast_panel";
pub const GOLD_MASTER_TOAST_TITLE_NAME: &str = "achievement_gold_master_toast_title";
pub const GOLD_MASTER_TOAST_DESC_NAME: &str = "achievement_gold_master_toast_desc";
pub const GOLD_MASTER_TOAST_CHECK_NAME: &str = "achievement_gold_master_toast_check";

pub const GOLD_MASTER_CARD_PANEL_NAME: &str = "achievement_gold_master_card_panel";
pub const GOLD_MASTER_CARD_TITLE_NAME: &str = "achievement_gold_master_card_title";
pub const GOLD_MASTER_CARD_DESC_NAME: &str = "achievement_gold_master_card_desc";
pub const GOLD_MASTER_CARD_CHECK_NAME: &str = "achievement_gold_master_card_check";

pub const GOLD_MASTER_TOAST_TOTAL_TICKS: u32 = 120;
pub const GOLD_MASTER_TOAST_RISE_TICKS: u32 = 14;

pub const GOLD_MASTER_TOAST_WIDTH: f32 = 1220.0;
pub const GOLD_MASTER_TOAST_HEIGHT: f32 = 156.0;
pub const GOLD_MASTER_CARD_WIDTH: f32 = 1520.0;
pub const GOLD_MASTER_CARD_HEIGHT: f32 = 196.0;

pub fn gold_master_unlocked(c: &Canvas) -> bool {
    matches!(c.get_var(GOLD_MASTER_UNLOCKED_VAR), Some(Value::Bool(true)))
}

pub fn gold_master_toast_active(c: &Canvas) -> bool {
    matches!(c.get_var(GOLD_MASTER_TOAST_ACTIVE_VAR), Some(Value::Bool(true)))
}

pub fn gold_master_toast_ticks(c: &Canvas) -> u32 {
    match c.get_var(GOLD_MASTER_TOAST_TICKS_VAR) {
        Some(Value::I32(v)) => v.max(0) as u32,
        _ => 0,
    }
}

pub fn clear_gold_master_toast(c: &mut Canvas) {
    c.set_var(GOLD_MASTER_TOAST_ACTIVE_VAR, false);
    c.set_var(GOLD_MASTER_TOAST_TICKS_VAR, 0i32);
}

pub fn trigger_gold_master_unlock(c: &mut Canvas) {
    c.set_var(GOLD_MASTER_UNLOCKED_VAR, true);
    c.set_var(GOLD_MASTER_TOAST_ACTIVE_VAR, true);
    c.set_var(GOLD_MASTER_TOAST_TICKS_VAR, 0i32);
    c.set_var("achievement_toast_title", GOLD_MASTER_TITLE.to_string());
    c.set_var("achievement_toast_desc", GOLD_MASTER_DESCRIPTION.to_string());
    // Persist the unlock on the active profile so it doesn't re-trigger.
    crate::profile::unlock_achievement_on_profile("gold_master");
}

pub fn maybe_unlock_gold_master(c: &mut Canvas, total_coins: i32) -> bool {
    if total_coins >= 10 && !gold_master_unlocked(c) {
        trigger_gold_master_unlock(c);
        true
    } else {
        false
    }
}

// ── Extra achievements (grounded in mechanics, awarded from lifetime stats) ──
pub const ACH_COIN_HUNTER:    &str = "coin_hunter";   // 100 lifetime coins
pub const ACH_DIST_10K:       &str = "distance_10k";  // best distance 10,000 px
pub const ACH_DIST_25K:       &str = "distance_25k";  // best distance 25,000 px
pub const ACH_BOSS_SLAYER:    &str = "boss_slayer";   // defeat a boss
pub const ACH_SPACE_CADET:    &str = "space_cadet";   // reach the space zone
pub const ACH_FIVE_RUNS:      &str = "five_runs";     // 5 runs played
pub const ACH_MARATHON:       &str = "marathon";      // 50,000 px best distance
pub const ACH_CASUAL_10:      &str = "casual_10";     // 10 casual runs
pub const ACH_BOSS_RUSH_5:    &str = "boss_rush_5";   // 5 boss-rush runs

/// Title + description for each extra achievement id.
pub(crate) fn achi_def(id: &str) -> Option<(&'static str, &'static str)> {
    Some(match id {
        ACH_COIN_HUNTER  => ("Coin Hunter", "Collect 100 coins (lifetime)."),
        ACH_DIST_10K     => ("Marathoner", "Reach 10,000 px in a single run."),
        ACH_DIST_25K     => ("Quarter-Century", "Reach 25,000 px in a single run."),
        ACH_BOSS_SLAYER  => ("Boss Slayer", "Defeat The Sun Devourer."),
        ACH_SPACE_CADET  => ("Space Cadet", "Reach the space zone."),
        ACH_FIVE_RUNS    => ("Regular", "Play 5 runs."),
        ACH_MARATHON     => ("Marathon", "Reach 50,000 px in a single run."),
        ACH_CASUAL_10    => ("Casual Regular", "Play 10 Casual runs."),
        ACH_BOSS_RUSH_5  => ("Rush Regular", "Play 5 Boss Rush runs."),
        _ => return None,
    })
}

/// Mark an achievement unlocked, show its toast, and persist on the profile.
pub fn trigger_achievement(c: &mut Canvas, id: &str, title: &str, desc: &str) {
    let title_id = "achievement_toast_title";
    let desc_id  = "achievement_toast_desc";
    let check_id = "achievement_toast_check";
    // Toast text is set by reading these vars in the HUD tick; store them for
    // the current unlock so build_scene's toast can show them.
    c.set_var("achievement_toast_title", title.to_string());
    c.set_var("achievement_toast_desc", desc.to_string());
    c.set_var(GOLD_MASTER_UNLOCKED_VAR, true); // reuse the "unlocked" gate
    c.set_var(GOLD_MASTER_TOAST_ACTIVE_VAR, true);
    c.set_var(GOLD_MASTER_TOAST_TICKS_VAR, 0i32);
    let _ = (title_id, desc_id, check_id); // names kept for clarity
    crate::profile::unlock_achievement_on_profile(id);
}

/// Award any extra achievements that the active profile's lifetime stats now
/// satisfy. `run_coins`/`run_distance` add the *current* (not yet recorded) run
/// so achievements can pop mid-run, exactly like the gold-master toast. Pass
/// (0, 0.0) when the run has already been recorded (e.g. on death).
pub fn check_achievements(c: &mut Canvas, run_coins: u64, run_distance: f32) {
    let (lifetime_coins, lifetime_best, bosses, runs, casual_runs, boss_rush_runs) = {
        let g = crate::profile::profile();
        let p = g.lock().unwrap();
        (p.total_coins, p.best_distance, p.bosses_defeated, p.runs_played,
         p.runs_casual, p.runs_bossrush)
    };
    let coins = lifetime_coins.saturating_add(run_coins);
    let best = (lifetime_best as f32).max(run_distance);
    let mut to_unlock: Vec<&str> = Vec::new();
    if coins >= 100 { to_unlock.push(ACH_COIN_HUNTER); }
    if best >= 10_000.0 { to_unlock.push(ACH_DIST_10K); }
    if best >= 25_000.0 { to_unlock.push(ACH_DIST_25K); }
    if best >= 50_000.0 { to_unlock.push(ACH_MARATHON); }
    if bosses > 0 { to_unlock.push(ACH_BOSS_SLAYER); }
    if runs >= 5 { to_unlock.push(ACH_FIVE_RUNS); }
    if casual_runs >= 10 { to_unlock.push(ACH_CASUAL_10); }
    if boss_rush_runs >= 5 { to_unlock.push(ACH_BOSS_RUSH_5); }
    for id in to_unlock {
        if !crate::profile::profile_has_achievement(id) {
            if let Some((title, desc)) = achi_def(id) {
                trigger_achievement(c, id, title, desc);
            }
        }
    }
}

/// Award the space-cadet achievement (called at space entry).
pub fn award_space_cadet(c: &mut Canvas) {
    if !crate::profile::profile_has_achievement(ACH_SPACE_CADET) {
        let (title, desc) = achi_def(ACH_SPACE_CADET).unwrap();
        trigger_achievement(c, ACH_SPACE_CADET, title, desc);
    }
}

// ── "Funny ways to die" achievements ─────────────────────────────────────────
pub const ACH_DIE_SUN:     &str = "die_sun";     // die to the sun
pub const ACH_DIE_OXYGEN:  &str = "die_oxygen";  // die to oxygen
pub const ACH_DIE_FALL:    &str = "die_fall";    // die to the danger floor

fn die_achi(cause: &str) -> (&'static str, &'static str, &'static str) {
    match cause {
        "sun"     => (ACH_DIE_SUN, "Sunburnt", "Die to the sun."),
        "oxygen"  => (ACH_DIE_OXYGEN, "Running on Fumes", "Die to low oxygen."),
        _         => (ACH_DIE_FALL, "Gravity's Lap", "Die to the danger floor."),
    }
}

/// Award a death-cause achievement (called from the death flow).
pub fn record_death_cause(c: &mut Canvas, cause: &str) {
    let (id, title, desc) = die_achi(cause);
    if !crate::profile::profile_has_achievement(id) {
        trigger_achievement(c, id, title, desc);
    }
}