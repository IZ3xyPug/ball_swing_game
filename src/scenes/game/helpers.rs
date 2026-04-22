use crate::constants::*;
use crate::images::{circle_cached, asteroid_hook_image_cached};
use quartz::{Image, ShapeType};

/// Hook image using circle_cached — keeps hooks in the same
/// render batch as other Rectangle objects to avoid z-order artifacts.
pub fn hook_img(r: u8, g: u8, b: u8) -> Image {
    Image {
        shape: ShapeType::Ellipse(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
        image: circle_cached(HOOK_R as u32, r, g, b),
        color: None,
    }
}

/// Asteroid-skinned hook image for the asteroid-hooks toggle mode.
pub fn hook_asteroid_img() -> Image {
    Image {
        shape: ShapeType::Rectangle(0.0, (HOOK_R * 2.0, HOOK_R * 2.0), 0.0),
        image: asteroid_hook_image_cached(),
        color: None,
    }
}

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
