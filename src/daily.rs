//! The daily reward and the daily challenge.
//!
//! DAILY REWARD: once a day, META from a seven-day calendar that climbs to a
//! big day-seven prize. Claiming on consecutive days walks the calendar;
//! missing a day starts it over; after day seven it loops.
//!
//! DAILY CHALLENGE: one run a day that is the same for everyone. The world is
//! generated from the day's seed, one modifier changes the rules, and there
//! is a distance to reach. Reaching it pays META once per day; trying again
//! only chases a better distance.
//!
//! Days are counted in UTC (days since the Unix epoch): the standard library
//! has no time zone, and one clock for everyone is what makes the challenge
//! the same challenge.

use quartz::*;

/// META for each day of the calendar, day one first.
pub const REWARD_CALENDAR: [u64; 7] = [50, 75, 100, 150, 200, 300, 600];

/// Set by the daily page's PLAY: the next run (and its retries) is the daily
/// challenge. The menu clears it.
pub const DAILY_PENDING_VAR: &str = "daily_pending";
/// True during a daily-challenge run.
pub const DAILY_ACTIVE_VAR: &str = "daily_active";

/// Today, as days since the Unix epoch (UTC).
pub fn today() -> i64 {
    // `DAILY_DAY=<n>` pins the day, for testing the calendar and rotation.
    if let Some(d) = std::env::var("DAILY_DAY").ok().and_then(|v| v.parse().ok()) { return d; }
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64).unwrap_or(0)
}

/// Seconds until the next day begins.
pub fn secs_to_tomorrow() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs()).unwrap_or(0);
    86_400 - now % 86_400
}

// ── Reward ───────────────────────────────────────────────────────────────────

/// Where the calendar stands: (the day to claim next, 0..7; claimed today).
pub fn reward_state() -> (usize, bool) {
    let g = crate::profile::profile();
    let p = g.lock().unwrap();
    let day = today();
    if p.daily_last_claim == day {
        // Claimed: the streak already counts today.
        (((p.daily_streak.max(1) - 1) % 7) as usize, true)
    } else if p.daily_last_claim == day - 1 {
        ((p.daily_streak % 7) as usize, false)
    } else {
        // Missed a day (or never claimed): back to day one.
        (0, false)
    }
}

/// Claim today's reward. `Some(meta)` if it was there to claim.
pub fn claim_reward() -> Option<u64> {
    let (slot, claimed) = reward_state();
    if claimed { return None; }
    let meta = REWARD_CALENDAR[slot];
    {
        let g = crate::profile::profile();
        let mut p = g.lock().unwrap();
        let day = today();
        p.daily_streak = if p.daily_last_claim == day - 1 { p.daily_streak + 1 } else { 1 };
        p.daily_last_claim = day;
        p.meta_currency = p.meta_currency.saturating_add(meta);
    }
    crate::profile::save_profile();
    Some(meta)
}

// ── Challenge ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Modifier {
    /// The rope reaches 30% further.
    LongRope,
    /// The rope is 15% shorter.
    ShortRope,
    /// One heart, no second chances.
    GlassCat,
    /// The momentum cap is 25% higher.
    SpeedDemon,
    /// Coins fly to you from twice as far.
    MagnetPaws,
    /// Two extra hearts, but a longer road.
    ThickSkin,
}

impl Modifier {
    pub const ALL: [Modifier; 6] = [
        Modifier::LongRope, Modifier::GlassCat, Modifier::SpeedDemon,
        Modifier::ShortRope, Modifier::MagnetPaws, Modifier::ThickSkin,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Modifier::LongRope => "Long Rope",
            Modifier::ShortRope => "Short Rope",
            Modifier::GlassCat => "Glass Cat",
            Modifier::SpeedDemon => "Speed Demon",
            Modifier::MagnetPaws => "Magnet Paws",
            Modifier::ThickSkin => "Thick Skin",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Modifier::LongRope => "Your rope reaches 30% further.",
            Modifier::ShortRope => "Your rope is 15% shorter.",
            Modifier::GlassCat => "One heart. No second wind.",
            Modifier::SpeedDemon => "Your top speed is 25% higher.",
            Modifier::MagnetPaws => "Coins fly to you from twice as far.",
            Modifier::ThickSkin => "Two extra hearts, but a longer road.",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Challenge {
    pub day: i64,
    pub modifier: Modifier,
    /// Distance to reach, px.
    pub goal: f32,
    /// META for reaching it (once a day).
    pub reward: u64,
    pub seed: u64,
}

/// The challenge for `day`. Pure, so everyone gets the same one.
pub fn challenge_for(day: i64) -> Challenge {
    let mut h = (day as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xDA11_C4A1_1E46_E5ED;
    h ^= h >> 31; h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9); h ^= h >> 29;
    let modifier = Modifier::ALL[(day.rem_euclid(Modifier::ALL.len() as i64)) as usize];
    // 1.5 to 3 minutes of travel (~18,000 px a minute), in steps of 5,000.
    let base = 27_000.0 + ((h >> 8) % 4) as f32 * 5_000.0;
    let goal = match modifier {
        Modifier::ThickSkin => base + 15_000.0,
        Modifier::GlassCat => base - 7_000.0,
        _ => base,
    };
    Challenge { day, modifier, goal, reward: 250, seed: h | 1 }
}

pub fn today_challenge() -> Challenge { challenge_for(today()) }

/// Whether today's challenge is already done.
pub fn done_today() -> bool {
    let g = crate::profile::profile();
    let p = g.lock().unwrap();
    p.daily_done_day == today()
}

/// Best distance in today's challenge.
pub fn best_today() -> u32 {
    let g = crate::profile::profile();
    let p = g.lock().unwrap();
    if p.daily_best_day == today() { p.daily_best } else { 0 }
}

pub fn is_daily_run(c: &Canvas) -> bool {
    c.get_bool(DAILY_ACTIVE_VAR)
}

/// A run is starting: is it the daily? (Reads the pending flag the daily
/// page set; retries keep it until the menu clears it.)
pub fn begin_run(c: &mut Canvas) -> Option<Challenge> {
    // `DAILY_RUN=1` makes a headless run the daily, for testing.
    let on = matches!(c.get_var(DAILY_PENDING_VAR), Some(Value::Bool(true))) || std::env::var("DAILY_RUN").is_ok();
    c.set_var(DAILY_ACTIVE_VAR, on);
    c.set_var("daily_goal_announced", false);
    on.then(today_challenge)
}

/// Every frame of a daily run: completes the challenge when the goal is
/// reached. Returns the META paid, the frame it was.
pub fn tick(c: &mut Canvas, distance: f32) -> Option<u64> {
    if !is_daily_run(c) || done_today() { return None; }
    let ch = today_challenge();
    if distance < ch.goal { return None; }
    {
        let g = crate::profile::profile();
        let mut p = g.lock().unwrap();
        p.daily_done_day = ch.day;
        p.dailies_done += 1;
        p.meta_currency = p.meta_currency.saturating_add(ch.reward);
    }
    crate::profile::save_profile();
    crate::achievements::toast("Daily Challenge Complete!",
        &format!("{}  \u{00b7}  +{} META", ch.modifier.name(), ch.reward), crate::achievements::Tier::Gold);
    crate::achievements::check_achievements(c, 0, 0.0);
    Some(ch.reward)
}

/// A daily run ended at `distance`: keep today's best.
pub fn end_run(c: &Canvas, distance: f32) {
    if !is_daily_run(c) { return; }
    let day = today();
    {
        let g = crate::profile::profile();
        let mut p = g.lock().unwrap();
        if p.daily_best_day != day { p.daily_best_day = day; p.daily_best = 0; }
        p.daily_best = p.daily_best.max(distance.max(0.0) as u32);
    }
    crate::profile::save_profile();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_is_the_same_for_a_day_and_changes_between_days() {
        let a = challenge_for(20_000);
        let b = challenge_for(20_000);
        assert_eq!((a.seed, a.goal, a.modifier), (b.seed, b.goal, b.modifier));
        let c = challenge_for(20_001);
        assert_ne!(a.seed, c.seed);
        assert_ne!(a.modifier, c.modifier);
        for d in 0..60 {
            let ch = challenge_for(20_000 + d);
            assert!(ch.goal >= 20_000.0 && ch.goal <= 60_000.0, "{}", ch.goal);
        }
    }
}
