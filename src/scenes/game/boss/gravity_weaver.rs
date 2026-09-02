use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use crate::images::circle_cached;
use crate::scenes::game::bootstrap::hook_asteroid_anim_for_spawn;
use crate::scenes::game::helpers::center_warp_on_player;
use crate::scenes::game::space_zone::wormhole2_template;
#[allow(unused_imports)]
use super::*;

/// The Gravity Weaver: a single-body boss that periodically INVERTS the world
/// (flips `gravity_dir`, so the arena's ceiling becomes the floor). Tether nodes
/// exist on both sides, so the player keeps swinging as the world turns over.
/// The boss's core opens for a short window right after each flip; a buffed hit
/// in that window damages it. Touching the body costs a heart.
pub(crate) fn tick_gravity_weaver(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }
    if s.dead { return; }
    if s.boss_stasis_active { return; }

    if !s.boss_spawned {
        s.boss_spawned = true;
        let name = s.boss_kind.name();
        drop(s);
        let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
        let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (spawn_x, spawn_y);
            obj.visible = true;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(120, 210, 255, 255), 1000.0 * sc,
                )));
            }
        }
        s = st.lock().unwrap();
    }

    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };

    // ── Flip cycle ──
    // weakpoint window open (after a flip) → countdown → flip + open window.
    let mut did_flip = false;
    {
        let mut s = st.lock().unwrap();
        if s.boss_flare_window_ticks > 0 {
            s.boss_flare_window_ticks -= 1;
        } else if s.boss_gravity_flip_ticks > 0 {
            s.boss_gravity_flip_ticks -= 1;
        } else {
            // Invert the world (like a space_rift).
            s.gravity_dir = -s.gravity_dir;
            s.boss_flare_window_ticks = GRAVITY_WEAVER_WINDOW_TICKS;
            s.boss_gravity_flip_ticks = GRAVITY_WEAVER_FLIP_INTERVAL;
            did_flip = true;
            // Keep the arena nodes / player oriented: flip the player's gravity.
            let gdir = s.gravity_dir;
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.gravity = GRAVITY * gdir;
            }
            drop(s);
            if let Some(cam) = c.camera_mut() {
                cam.flash_with(Color(120, 200, 255, 120), 0.35, FlashMode::Pulse, FlashEase::Sharp, 0.8, 0.0);
            }
            s = st.lock().unwrap();
        }
        let _ = did_flip;
    }

    // ── Weakpoint window: buffed hit on the core damages the boss ──
    let window_open = { let s = st.lock().unwrap(); s.boss_flare_window_ticks > 0 };
    if window_open && buffed && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + 320.0).powi(2) {
        let mut s = st.lock().unwrap();
        s.boss_hp = (s.boss_hp - 2).max(0);
        s.buff_hit_flash = 20;
        let hp = s.boss_hp;
        drop(s);
        if hp <= 0 {
            if let Some(obj) = c.get_game_object_mut("boss") {
                obj.visible = false;
                obj.position = (-6000.0, -6000.0);
            }
            finish_boss(c, st);
        }
    }

    // ── Contact-rule inversion ──
    {
        let s = st.lock().unwrap();
        let touching = !s.dead
            && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + BOSS_SIZE * 0.5).powi(2);
        drop(s);
        if touching {
            crate::scenes::game::hearts::lose_heart(c, st);
        }
    }
}
