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

/// The Flare Titan: a single-body boss whose fight is the flare loop. Telegraph
/// → flare (tether a shielded node or lose a heart; tethering grants Solar
/// Charge) → weakpoint window (core vents; only a buffed hit hurts it, for 4s)
/// → repeat. Touching the body costs a heart.
pub(crate) fn tick_flare_titan(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }
    if s.dead { return; }
    if s.boss_stasis_active { return; }

    // Appearance (once).
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
                    name, &font, 42.0 * sc, Color(255, 170, 60, 255), 1000.0 * sc,
                )));
            }
        }
        ensure_arena_shelter_nodes(c, st);
        s = st.lock().unwrap();
    }

    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };

    // ── Flare cycle ──
    // idle → warning (flare_warn) → active (flare_active) → weakpoint window
    // (boss_flare_window_ticks).
    let mut unhook_flare = false;
    {
        let mut s = st.lock().unwrap();
        if s.boss_flare_window_ticks > 0 {
            s.boss_flare_window_ticks -= 1;
        } else if s.flare_active {
            s.flare_active_ticks = s.flare_active_ticks.saturating_sub(1);
            s.flare_damage_timer = s.flare_damage_timer.saturating_sub(1);
            let sheltered = crate::scenes::game::solar::player_is_sheltered(c, &s);
            // Tethering a shielded node grants / refreshes Solar Charge.
            if sheltered {
                s.player_buff = 1;
                s.buff_timer = 600;
                s.buff_absorbs = 3;
            }
            if s.flare_damage_timer == 0 {
                s.flare_damage_timer = FLARE_TITAN_DAMAGE_INTERVAL;
                if !sheltered {
                    unhook_flare = s.hooked;
                }
            }
            if s.flare_active_ticks == 0 {
                s.flare_active = false;
                s.boss_flare_window_ticks = FLARE_TITAN_WINDOW_TICKS;
            }
        } else if s.flare_warn > 0 {
            s.flare_warn -= 1;
            c.set_var("flare_warning", s.flare_warn > 0);
            if s.flare_warn == 0 {
                s.flare_active = true;
                s.flare_active_ticks = FLARE_TITAN_ACTIVE_TICKS;
                s.flare_damage_timer = FLARE_TITAN_DAMAGE_INTERVAL;
                c.set_var("flare_active", true);
                c.set_var("flare_warning", false);
            }
        } else if s.flare_cooldown == 0 {
            s.flare_cooldown = FLARE_TITAN_INTERVAL;
            s.flare_warn = FLARE_TITAN_TELEGRAPH_TICKS;
        } else {
            s.flare_cooldown -= 1;
        }
    }

    if unhook_flare {
        let mut s = st.lock().unwrap();
        s.hooked = false;
        s.active_hook = String::new();
        drop(s);
        c.run(Action::Hide { target: Target::name("rope") });
        crate::scenes::game::hearts::lose_heart(c, st);
    }

    // ── Weakpoint window: a buffed hit on the core damages the boss ──
    let window_open = { let s = st.lock().unwrap(); s.boss_flare_window_ticks > 0 };
    if window_open && buffed && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + 320.0).powi(2) {
        let mut s = st.lock().unwrap();
        s.boss_hp = (s.boss_hp - 4).max(0);
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
