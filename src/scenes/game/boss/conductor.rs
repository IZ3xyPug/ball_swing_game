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

/// The Conductor: a single-body boss fought to a beat. Releasing your tether
/// within a few frames of a beat (the release window) earns one stack of
/// Resonance; at three stacks the weakpoint arms and a buffed hit damages the
/// boss. Missing a beat costs a stack. Touching the body costs a heart.
pub(crate) fn tick_conductor(c: &mut Canvas, st: &Arc<Mutex<State>>) {
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
                    name, &font, 42.0 * sc, Color(90, 220, 200, 255), 1000.0 * sc,
                )));
            }
        }
        s = st.lock().unwrap();
        s.boss_beat_ticks = s.boss_beat_interval;
    }

    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };

    // Detect a release edge (hooked last tick, free now) and open the window.
    let mut beat_hit = false;
    {
        let mut s = st.lock().unwrap();
        let hooked = s.hooked;
        if s.boss_was_hooked && !hooked {
            s.boss_release_window = CONDUCTOR_RELEASE_WINDOW;
        }
        s.boss_was_hooked = hooked;
        if s.boss_release_window > 0 {
            s.boss_release_window -= 1;
        }

        // Advance the beat.
        if s.boss_beat_ticks > 0 {
            s.boss_beat_ticks -= 1;
        }
        if s.boss_beat_ticks == 0 {
            // Beat landed. On-beat release → Resonance stack; otherwise lose one.
            if s.boss_release_window > 0 {
                s.boss_resonance = (s.boss_resonance + 1).min(CONDUCTOR_RESONANCE_REQUIRED);
            } else if s.boss_resonance > 0 {
                s.boss_resonance -= 1;
            }
            // Phase two: faster bar once below half HP.
            let speedup = s.boss_hp <= BOSS_MAX_HP / 2;
            s.boss_beat_interval = if speedup { 30 } else { 36 };
            s.boss_beat_ticks = s.boss_beat_interval;
            // At full Resonance, arm the weakpoint window.
            if s.boss_resonance >= CONDUCTOR_RESONANCE_REQUIRED {
                s.boss_flare_window_ticks = 180;
            }
            beat_hit = true;
        }
    }

    // ── Weakpoint window (armed) → buffed hit damages the boss ──
    let window_open = { let s = st.lock().unwrap(); s.boss_flare_window_ticks > 0 && s.boss_resonance >= CONDUCTOR_RESONANCE_REQUIRED };
    if window_open && buffed && (px - bcx).powi(2) + (py - bcy).powi(2) < (PLAYER_R + 320.0).powi(2) {
        let mut s = st.lock().unwrap();
        s.boss_hp = (s.boss_hp - 2).max(0);
        s.buff_hit_flash = 20;
        s.boss_resonance = 0; // spent: rebuild the stacks.
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
