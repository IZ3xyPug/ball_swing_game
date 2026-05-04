use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::zone_index_for_distance;
use crate::hud::*;
use crate::images::*;
use crate::state::*;

pub fn tick_hud(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let distance = s.distance;
    let zone_idx = zone_index_for_distance(distance);
    let zone_start = zone_idx as f32 * ZONE_DISTANCE_STEP;
    let dist_fill = ((distance - zone_start) / ZONE_DISTANCE_STEP).clamp(0.0, 1.0);
    let coins = s.coin_count;
    let score = s.score;
    let py = s.py;
    let px = s.px;
    let gravity_dir = s.gravity_dir;
    let ticks = s.ticks;

    // In flipped gravity the floor is at y=VH (ceiling is y=0). Negate and
    // shift so 0 = floor (py=VH), positive = going "up" toward ceiling (py→0).
    let display_py = if gravity_dir < 0.0 { VH - py } else { py };

    // Quantize for dirty checks (sign-aware so display refreshes on flip)
    let q_dist_fill = (dist_fill * 1000.0) as u32;
    let q_py        = display_py as i32;
    let q_px        = px as i32;

    let _dirty_dist    = q_dist_fill     != s.hud_last_dist_fill;
    let dirty_coins    = coins           != s.hud_last_coins;
    let dirty_py       = q_py            != s.hud_last_py;
    let dirty_px       = q_px            != s.hud_last_px;
    let dirty_score    = score           != s.hud_last_score;

    let previous_coins = s.hud_last_coins;
    let initialized = previous_coins != u32::MAX;
    let coin_gained = initialized && coins > previous_coins;
    if !initialized && coins == 0 {
        s.hud_coin_alpha = 0;
        s.hud_coin_fade_ticks = u32::MAX;
    } else if !initialized || coin_gained {
        s.hud_coin_fade_ticks = 0;
        s.hud_coin_alpha = 255;
    } else {
        s.hud_coin_fade_ticks = s.hud_coin_fade_ticks.saturating_add(1);
        const COIN_HUD_HOLD_TICKS: u32 = 45;
        const COIN_HUD_FADE_TICKS: u32 = 300;
        const COIN_HUD_ALPHA_MIN: u8 = 0;

        let target_alpha = if s.hud_coin_fade_ticks <= COIN_HUD_HOLD_TICKS {
            255
        } else {
            let t = (s.hud_coin_fade_ticks - COIN_HUD_HOLD_TICKS).min(COIN_HUD_FADE_TICKS);
            let k = 1.0 - (t as f32 / COIN_HUD_FADE_TICKS as f32);
            let min_a = COIN_HUD_ALPHA_MIN as f32;
            (min_a + (255.0 - min_a) * k).round() as u8
        };
        s.hud_coin_alpha = target_alpha;
    }
    let coin_alpha = s.hud_coin_alpha;
    let dirty_alpha = coin_alpha != s.hud_last_coin_alpha;

    // Rebuild base image whenever coin count changes
    if dirty_coins || s.hud_coin_base_img.is_none() {
        s.hud_coin_base_img = Some(coin_counter_img(coins));
    }

    // Build alpha-applied image when coin count or alpha changes
    let update_coin_img = if dirty_coins || dirty_alpha {
        let base = s.hud_coin_base_img.as_ref().unwrap();
        let mut img = base.clone();
        if coin_alpha < 255 {
            for pixel in img.pixels_mut() {
                pixel[3] = ((pixel[3] as u32 * coin_alpha as u32) / 255) as u8;
            }
        }
        Some(img)
    } else {
        None
    };

    // Update tracking
    s.hud_last_dist_fill    = q_dist_fill;
    s.hud_last_coins        = coins;
    s.hud_last_py           = q_py;
    s.hud_last_px           = q_px;
    s.hud_last_score        = score;
    s.hud_last_coin_alpha   = coin_alpha;
    let in_space = s.in_space_mode;
    drop(s);

    // Distance progress bar — removed from in-game HUD
    if let Some(obj) = c.get_game_object_mut("dist_bar") {
        obj.visible = false;
    }

    // Coin counter
    if let Some(obj) = c.get_game_object_mut("coin_counter") {
        obj.position = (26.0, 24.0);
        obj.visible = coin_alpha > 0;
        if let Some(img) = update_coin_img {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (640.0, 168.0), 0.0),
                image: img.into(),
                color: None,
            });
        }
    }

    // Score counter (top-right)
    if let Some(obj) = c.get_game_object_mut("score_counter") {
        obj.position = (VW - 450.0, 40.0);
        if dirty_score {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (420.0, 98.0), 0.0),
                image: score_counter_img(score).into(),
                color: None,
            });
        }
    }

    if let Some(obj) = c.get_game_object_mut("momentum_counter") {
        obj.visible = false;
    }

    if let Some(obj) = c.get_game_object_mut("gravity_indicator") {
        obj.visible = false;
    }

    // Y meter
    if let Some(obj) = c.get_game_object_mut("y_meter") {
        obj.position = (30.0, 344.0);
        if dirty_py {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (420.0, 86.0), 0.0),
                image: y_counter_img(display_py).into(),
                color: None,
            });
        }
    }

    // X meter
    if let Some(obj) = c.get_game_object_mut("x_meter") {
        obj.position = (30.0, 442.0);
        if dirty_px {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (420.0, 86.0), 0.0),
                image: x_counter_img(px).into(),
                color: None,
            });
        }
    }

    if let Some(obj) = c.get_game_object_mut("flip_timer") {
        obj.visible = false;
    }

    if let Some(obj) = c.get_game_object_mut("zero_g_timer") {
        obj.visible = false;
    }

    // Hide combo flash periodically
    if ticks % 40 == 0 {
        c.run(Action::Hide { target: Target::name("combo_flash") });
    }
}
