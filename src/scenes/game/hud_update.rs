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
    let momentum = (s.vx * s.vx + s.vy * s.vy).sqrt();
    let score = s.score;
    let gravity_flipped = s.gravity_dir < 0.0;
    let py = s.py;
    let px = s.px;
    let flip_timer_val = s.flip_timer;
    let zero_g_timer_val = s.zero_g_timer;
    let ticks = s.ticks;

    // Quantize for dirty checks
    let q_dist_fill = (dist_fill * 1000.0) as u32;
    let q_momentum  = (momentum * 10.0) as u32;
    let q_py        = py as i32;
    let q_px        = px as i32;

    let dirty_dist     = q_dist_fill     != s.hud_last_dist_fill;
    let dirty_coins    = coins           != s.hud_last_coins;
    let dirty_momentum = q_momentum      != s.hud_last_momentum;
    let dirty_gravity  = gravity_flipped != s.hud_last_gravity_flip;
    let dirty_py       = q_py            != s.hud_last_py;
    let dirty_px       = q_px            != s.hud_last_px;
    let dirty_flip     = flip_timer_val  != s.hud_last_flip_timer;
    let dirty_zero_g   = zero_g_timer_val != s.hud_last_zero_g_timer;
    let dirty_score    = score           != s.hud_last_score;

    // Update tracking
    s.hud_last_dist_fill    = q_dist_fill;
    s.hud_last_coins        = coins;
    s.hud_last_momentum     = q_momentum;
    s.hud_last_gravity_flip = gravity_flipped;
    s.hud_last_py           = q_py;
    s.hud_last_px           = q_px;
    s.hud_last_flip_timer   = flip_timer_val;
    s.hud_last_zero_g_timer = zero_g_timer_val;
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
        if dirty_coins {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (420.0, 98.0), 0.0),
                image: coin_counter_img(coins).into(),
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

    // Momentum counter
    if let Some(obj) = c.get_game_object_mut("momentum_counter") {
        obj.position = (30.0, 150.0);
        if dirty_momentum {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (420.0, 86.0), 0.0),
                image: momentum_counter_img(momentum).into(),
                color: None,
            });
        }
    }

    // Gravity indicator
    if let Some(obj) = c.get_game_object_mut("gravity_indicator") {
        obj.position = (30.0, 248.0);
        if dirty_gravity {
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (308.0, 84.0), 0.0),
                image: gravity_indicator_img(gravity_flipped, true).into(),
                color: None,
            });
        }
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

    // Flip timer HUD
    if let Some(obj) = c.get_game_object_mut("flip_timer") {
        if flip_timer_val > 0 {
            obj.position = (VW * 0.5 - 252.0, 96.0);
            obj.visible = true;
            if dirty_flip {
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (504.0, 118.0), 0.0),
                    image: flip_timer_img(flip_timer_val, FLIP_DURATION).into(),
                    color: None,
                });
            }
        } else {
            obj.visible = false;
        }
    }

    // Zero-g timer HUD
    if let Some(obj) = c.get_game_object_mut("zero_g_timer") {
        if zero_g_timer_val > 0 {
            obj.position = (VW * 0.5 - 252.0, 226.0);
            obj.visible = true;
            if dirty_zero_g {
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (504.0, 118.0), 0.0),
                    image: flip_timer_img(zero_g_timer_val, ZERO_G_DURATION).into(),
                    color: None,
                });
            }
        } else {
            obj.visible = false;
        }
    }

    // Hide combo flash periodically
    if ticks % 40 == 0 {
        c.run(Action::Hide { target: Target::name("combo_flash") });
    }
}
