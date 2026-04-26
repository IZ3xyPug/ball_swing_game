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
    let ticks = s.ticks;

    // Quantize for dirty checks
    let q_dist_fill = (dist_fill * 1000.0) as u32;
    let q_py        = py as i32;
    let q_px        = px as i32;

    let dirty_dist     = q_dist_fill     != s.hud_last_dist_fill;
    let dirty_coins    = coins           != s.hud_last_coins;
    let dirty_py       = q_py            != s.hud_last_py;
    let dirty_px       = q_px            != s.hud_last_px;
    let dirty_score    = score           != s.hud_last_score;

    let previous_coins = s.hud_last_coins;
    let coin_gained = previous_coins != u32::MAX && coins > previous_coins;
    if coin_gained {
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

    // Update tracking
    s.hud_last_dist_fill    = q_dist_fill;
    s.hud_last_coins        = coins;
    s.hud_last_py           = q_py;
    s.hud_last_px           = q_px;
    s.hud_last_score        = score;
    let in_space = s.in_space_mode;
    drop(s);

    // Distance progress bar (hidden while in space; oxygen bar takes its slot)
    if !in_space {
        if let Some(obj) = c.get_game_object_mut("dist_bar") {
            obj.position = (VW * 0.5 - 460.0, 30.0);
            obj.visible = true;
            if dirty_dist {
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (920.0, 48.0), 0.0),
                    image: bar_img(920, 48, dist_fill, 80, 220, 160).into(),
                    color: None,
                });
            }
        }
    }

    // Coin counter
    if let Some(obj) = c.get_game_object_mut("coin_counter") {
        obj.position = (30.0, 40.0);
        obj.visible = true;
        if dirty_coins {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (420.0, 98.0), 0.0),
                image: coin_counter_img(coins).into(),
                color: None,
            });
        }
        obj.set_tint(Color(255, 255, 255, coin_alpha));
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
                image: y_counter_img(py).into(),
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
