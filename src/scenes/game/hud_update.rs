use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::zone_index_for_distance;
use crate::hud::*;
use crate::images::*;
use crate::state::*;

pub fn tick_hud(c: &mut Canvas, st: &Arc<Mutex<State>>, cam_x: f32) {
    let s = st.lock().unwrap();
    let distance = s.distance;
    let zone_idx = zone_index_for_distance(distance);
    let zone_start = zone_idx as f32 * ZONE_DISTANCE_STEP;
    let dist_fill = ((distance - zone_start) / ZONE_DISTANCE_STEP).clamp(0.0, 1.0);
    let coins = s.coin_count;
    let momentum = (s.vx * s.vx + s.vy * s.vy).sqrt();
    let gravity_flipped = s.gravity_dir < 0.0;
    let py = s.py;
    let px = s.px;
    let flip_timer_val = s.flip_timer;
    let zero_g_timer_val = s.zero_g_timer;
    let ticks = s.ticks;
    drop(s);

    // Distance progress bar
    if let Some(obj) = c.get_game_object_mut("dist_bar") {
        obj.position = (cam_x + VW * 0.5 - 460.0, 30.0);
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (920.0, 48.0), 0.0),
            image: bar_img(920, 48, dist_fill, 80, 220, 160).into(),
            color: None,
        });
    }

    // Coin counter
    if let Some(obj) = c.get_game_object_mut("coin_counter") {
        obj.position = (cam_x + 30.0, 40.0);
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (300.0, 70.0), 0.0),
            image: coin_counter_img(coins).into(),
            color: None,
        });
    }

    // Momentum counter
    if let Some(obj) = c.get_game_object_mut("momentum_counter") {
        obj.position = (cam_x + 30.0, 128.0);
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
            image: momentum_counter_img(momentum).into(),
            color: None,
        });
    }

    // Gravity indicator
    if let Some(obj) = c.get_game_object_mut("gravity_indicator") {
        obj.position = (cam_x + 30.0, 200.0);
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (220.0, 60.0), 0.0),
            image: gravity_indicator_img(gravity_flipped, true).into(),
            color: None,
        });
    }

    // Y meter
    if let Some(obj) = c.get_game_object_mut("y_meter") {
        obj.position = (cam_x + 30.0, 272.0);
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
            image: y_counter_img(py).into(),
            color: None,
        });
    }

    // X meter
    if let Some(obj) = c.get_game_object_mut("x_meter") {
        obj.position = (cam_x + 30.0, 344.0);
        obj.set_image(Image {
            shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
            image: x_counter_img(px).into(),
            color: None,
        });
    }

    // Flip timer HUD
    if let Some(obj) = c.get_game_object_mut("flip_timer") {
        if flip_timer_val > 0 {
            obj.position = (cam_x + VW * 0.5 - 180.0, 460.0);
            obj.visible = true;
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (360.0, 84.0), 0.0),
                image: flip_timer_img(flip_timer_val, FLIP_DURATION).into(),
                color: None,
            });
        } else {
            obj.visible = false;
        }
    }

    // Zero-g timer HUD
    if let Some(obj) = c.get_game_object_mut("zero_g_timer") {
        if zero_g_timer_val > 0 {
            obj.position = (cam_x + VW * 0.5 - 180.0, 556.0);
            obj.visible = true;
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (360.0, 84.0), 0.0),
                image: flip_timer_img(zero_g_timer_val, ZERO_G_DURATION).into(),
                color: None,
            });
        } else {
            obj.visible = false;
        }
    }

    // Hide combo flash periodically
    if ticks % 40 == 0 {
        c.run(Action::Hide { target: Target::name("combo_flash") });
    }
}
