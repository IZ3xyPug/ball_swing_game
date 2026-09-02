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

/// The Magnetar: a single-body boss that pulses a strong gravity attraction,
/// dragging the player's rope toward it. The player resists by grabbing a
/// shielded node or letting go and swinging against the pull. While over-charged
/// (venting) the core is the weakpoint; a buffed hit damages it. Touching the
/// body costs a heart.
pub(crate) fn tick_magnetar(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
                    name, &font, 42.0 * sc, Color(200, 120, 255, 255), 1000.0 * sc,
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

    // ── Charge cycle ──
    // idle (flare_cooldown) → pull window (boss_pull_ticks) → weakpoint window.
    let mut pulling = false;
    {
        let mut s = st.lock().unwrap();
        if s.boss_flare_window_ticks > 0 {
            s.boss_flare_window_ticks -= 1;
        } else if s.boss_pull_ticks > 0 {
            s.boss_pull_ticks -= 1;
            pulling = true;
            if s.boss_pull_ticks == 0 {
                s.boss_flare_window_ticks = MAGNETAR_WINDOW_TICKS;
            }
        } else if s.flare_cooldown == 0 {
            s.flare_cooldown = MAGNETAR_PULL_INTERVAL;
            s.boss_pull_ticks = MAGNETAR_PULL_TICKS;
        } else {
            s.flare_cooldown -= 1;
        }
    }

    // The pull: drag the player toward the boss's core.
    if pulling {
        let dx = px - bcx;
        let dy = py - bcy;
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let strength = 0.9 * (1.0 - (d / 2600.0).clamp(0.0, 1.0));
        if strength > 0.01 {
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum.0 -= dx / d * strength;
                obj.momentum.1 -= dy / d * strength;
            }
        }
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
