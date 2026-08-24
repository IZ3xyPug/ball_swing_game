// ── scenes/game/upgrades.rs — roguelike upgrade nodes (spend coins) ───────────
// Nodes spawn in the normal zone as glowing purple rings. Touching one with
// enough coins buys a boost:
//   - extra heart  (permanent, cost grows exponentially per owned extra heart)
//   - controlled breathing (slower oxygen drain, run-long)
//   - momentum cap boost (run-long)
// Purchases are applied immediately and (for hearts) persist via META_EXTRA_HEARTS_VAR.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

pub fn tick_upgrades(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (in_space, boss) = {
        let s = st.lock().unwrap();
        (s.in_space_mode, s.boss_active)
    };
    if in_space || boss {
        return;
    }
    spawn_upgrade_nodes(c, st);
    purchase_upgrades(c, st);
}

fn spawn_upgrade_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let mut spawned = 0usize;
    while spawned < UPGRADE_SPAWN_BUDGET_PER_TICK
        && !s.upgrade_free.is_empty()
        && s.upgrade_rightmost < s.px + GEN_AHEAD
    {
        let gap = lcg_range(&mut s.seed, UPGRADE_GAP_MIN, UPGRADE_GAP_MAX);
        let x = s.upgrade_rightmost + gap;
        let y = lcg_range(&mut s.seed, HOOK_Y_MIN, HOOK_Y_MAX);
        let Some(id) = s.upgrade_free.pop() else { break; };
        let roll = lcg(&mut s.seed);
        let ty = if roll < 0.4 { "upgrade_heart" } else if roll < 0.7 { "upgrade_breath" } else { "upgrade_momentum" };
        s.upgrade_live.push(id.clone());
        if x > s.upgrade_rightmost {
            s.upgrade_rightmost = x;
        }
        spawned += 1;
        drop(s);
        if let Some(obj) = c.get_game_object_mut(&id) {
            let r = UPGRADE_R;
            obj.position = (x - r, y - r);
            obj.size = (r * 2.0, r * 2.0);
            obj.visible = true;
            obj.gravity = 0.0;
            obj.momentum = (0.0, 0.0);
            obj.rotation_momentum = 0.0;
            obj.tags.retain(|t| t != "upgrade_heart" && t != "upgrade_breath" && t != "upgrade_momentum");
            obj.tags.push(ty.into());
        }
        s = st.lock().unwrap();
    }
}

fn purchase_upgrades(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (px, py, coins) = {
        let s = st.lock().unwrap();
        (s.px, s.py, s.coin_count)
    };
    let live: Vec<String> = st.lock().unwrap().upgrade_live.clone();
    if live.is_empty() {
        return;
    }
    let mut recycle: Vec<String> = Vec::new();
    for id in &live {
        let Some(obj) = c.get_game_object(id) else { continue; };
        if !obj.visible {
            recycle.push(id.clone());
            continue;
        }
        let cx = obj.position.0 + obj.size.0 * 0.5;
        let cy = obj.position.1 + obj.size.1 * 0.5;
        // Cull far-behind nodes.
        let cutoff = px - VW * 3.0;
        if obj.position.0 + obj.size.0 < cutoff {
            recycle.push(id.clone());
            continue;
        }
        let rr = PLAYER_R + UPGRADE_R;
        if (cx - px) * (cx - px) + (cy - py) * (cy - py) >= rr * rr {
            continue;
        }
        let is_heart = obj.tags.iter().any(|t| t == "upgrade_heart");
        let is_breath = obj.tags.iter().any(|t| t == "upgrade_breath");
        let is_momentum = obj.tags.iter().any(|t| t == "upgrade_momentum");
        let extra = match c.get_var(META_EXTRA_HEARTS_VAR) {
            Some(Value::I32(v)) => v.max(0),
            _ => 0,
        };
        let cost = if is_heart {
            (UPGRADE_HEART_BASE_COST as f32 * UPGRADE_HEART_GROWTH.powi(extra)).round() as u32
        } else if is_breath {
            UPGRADE_BREATH_COST
        } else {
            UPGRADE_MOMENTUM_COST
        };
        if coins < cost {
            continue;
        }

        {
            let mut s = st.lock().unwrap();
            s.coin_count -= cost;
            if is_heart {
                s.max_hearts += 1;
                s.hearts += 1;
                c.set_var(META_EXTRA_HEARTS_VAR, Value::I32(extra + 1));
            } else if is_breath {
                s.oxygen_drain_scale = UPGRADE_BREATH_DRAIN_SCALE.min(s.oxygen_drain_scale);
            } else if is_momentum {
                s.upgrade_momentum_bonus = true;
            }
            s.upgrade_live.retain(|n| n != id);
            s.upgrade_free.push(id.clone());
            drop(s);
            if let Some(o) = c.get_game_object_mut(id) {
                o.visible = false;
            }
        }
    }

    if !recycle.is_empty() {
        let mut s = st.lock().unwrap();
        for id in recycle {
            s.upgrade_live.retain(|n| n != &id);
            s.upgrade_free.push(id.clone());
            if let Some(o) = c.get_game_object_mut(&id) {
                o.visible = false;
            }
        }
    }
}
