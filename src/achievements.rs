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
}

pub fn maybe_unlock_gold_master(c: &mut Canvas, total_coins: i32) -> bool {
    if total_coins >= 10 && !gold_master_unlocked(c) {
        trigger_gold_master_unlock(c);
        true
    } else {
        false
    }
}