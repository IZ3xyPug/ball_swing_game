// ── boss/ — one module per boss, plus the shared arena and helpers ──────────
//
// Split out of a single 4200-line boss.rs. The per-boss files hold ONLY that
// boss's fight; everything a fight needs but does not own — the arena, entry,
// warp, stasis, tether nodes, HUD, and the finish sequence — stays in `arena`,
// because duplicating that per boss is how seven copies drift apart.
//
// `common` is the geometry every boss reaches for (capped motion, segment
// distance, the beam polyline). It is shared rather than per-boss so a fix to,
// say, the arrival-edge test lands everywhere at once.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

pub(crate) mod common;
pub(crate) mod arena;
pub(crate) mod colossus;
pub(crate) mod serpent;
pub(crate) mod conductor;
pub(crate) mod flare_titan;
pub(crate) mod gravity_weaver;
pub(crate) mod magnetar;
pub(crate) mod sun_devourer;

pub(crate) use common::*;
pub(crate) use arena::*;
pub(crate) use colossus::*;
pub(crate) use serpent::*;
pub(crate) use conductor::*;
pub(crate) use flare_titan::*;
pub(crate) use gravity_weaver::*;
pub(crate) use magnetar::*;
pub(crate) use sun_devourer::*;

pub fn tick_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Published every frame: several systems (distance tracking, the headless
    // harness, the HUD) need to know an arena is active, and deriving it from
    // boss HP is wrong during entry and victory stasis.
    {
        let active = st.lock().unwrap().boss_active;
        c.set_var("boss_active", active);
    }
    tick_boss_zone_entry(c, st);
    tick_boss_stasis(c, st);
    tick_warp_flash(c, st);

    // Multi-part bosses (Colossus, Serpent) run their own part-driven loop; the
    // single-body bosses dispatch by kind below.
    let is_multi = { let s = st.lock().unwrap(); !s.boss_parts.is_empty() };
    if is_multi {
        // The Serpent is a distinct multi-part fight (head-HP win, tetherable
        // chain); the Colossus uses the shared part loop.
        let kind = { let s = st.lock().unwrap(); s.boss_kind };
        if kind == crate::constants::BossKind::Serpent {
            tick_serpent(c, st);
        } else {
            tick_multi_part_boss(c, st);
        }
    } else {
        let kind = { let s = st.lock().unwrap(); s.boss_kind };
        match kind {
            crate::constants::BossKind::FlareTitan => tick_flare_titan(c, st),
            crate::constants::BossKind::GravityWeaver => tick_gravity_weaver(c, st),
            crate::constants::BossKind::Magnetar => tick_magnetar(c, st),
            crate::constants::BossKind::Conductor => tick_conductor(c, st),
            _ => {
                tick_boss_appearance(c, st);
                tick_boss_movement(c, st);
                tick_boss_asteroid_drift(c, st);
                tick_boss_shooting(c, st);
                tick_boss_darkness(c, st);
                tick_boss_weakpoints(c, st);
                tick_boss_forcefield(c, st);
                tick_generators(c, st);
                tick_barrier(c, st);
                tick_desperation(c, st);
                tick_boss_bolts(c, st);
                tick_boss_bolt_player_collision(c, st);
                tick_boss_player_hits_boss(c, st);
            }
        }
    }

    tick_colossus_meteors(c, st);
    tick_core_vent(c, st);
    tick_clap_wave(c, st);
    tick_boss_hud(c, st);
    tick_boss_indicators(c, st);
    tick_boss_lights(c, st);
    tick_buff_node_elec(c, st);
    tick_arena_walls(c, st);
}
