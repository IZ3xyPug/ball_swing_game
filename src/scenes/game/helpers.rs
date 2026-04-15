use crate::constants::*;

pub fn hook_base_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_HOOK_ZONE1,
        2 => C_HOOK_ZONE2,
        _ => C_HOOK,
    }
}

pub fn hook_near_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_HOOK_NEAR_ZONE1,
        2 => C_HOOK_NEAR_ZONE2,
        _ => C_HOOK_NEAR,
    }
}

pub fn hook_on_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_HOOK_ON_ZONE1,
        2 => C_HOOK_ON_ZONE2,
        _ => C_HOOK_ON,
    }
}

pub fn pad_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_PAD_ZONE1,
        2 => C_PAD_ZONE2,
        _ => C_PAD,
    }
}

pub fn pad_hit_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_PAD_HIT_ZONE1,
        2 => C_PAD_HIT_ZONE2,
        _ => C_PAD_HIT,
    }
}

pub fn spinner_for_zone(zone_idx: usize) -> (u8, u8, u8) {
    match zone_idx {
        1 => C_SPINNER_ZONE1,
        2 => C_SPINNER_ZONE2,
        _ => C_SPINNER,
    }
}
