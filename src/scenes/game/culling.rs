use quartz::*;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use crate::gameplay::zone_index_for_distance;

pub fn tick_culling(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    cull_hooks(c, st);
    cull_pads(c, st);
    cull_spinners(c, st);
    cull_coins(c, st);
    cull_flips(c, st);
    cull_score_x2(c, st);
    cull_zero_g(c, st);
    cull_gates(c, st);
    cull_gravity_wells(c, st);
    cull_turrets(c, st);
    cull_bullets(c, st);
}

fn cull_hooks(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.live_hooks.iter()
        .filter(|name| c.get_game_object(name).map(|o| o.position.0 + HOOK_R*2.0 < cutoff).unwrap_or(true))
        .cloned().collect();

    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-2000.0, -2000.0);
        }
    }
    let rm_set: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    let active_hook_removed = s.hooked && rm_set.contains(s.active_hook.as_str());
    s.live_hooks.retain(|n| !rm_set.contains(n.as_str()));
    for name in to_remove { s.pool_free.push(name); }

    if active_hook_removed {
        let _zone_idx = zone_index_for_distance(s.distance);
        let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
        let gdir = s.gravity_dir;
        s.hooked = false;
        s.active_hook = String::new();
        drop(s);
        c.run(Action::Hide { target: Target::name("rope") });
        c.release_grapple("player");
        // Re-enable gravity when unhooked.
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * gravity_scale * gdir;
        }
    }
}

fn cull_pads(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.pad_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + PAD_W < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; obj.position = (-3000.0, -3000.0); }
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.pad_live.retain(|n| !rm.contains(n.as_str()));
    for name in &to_remove { s.pad_origins.retain(|(n, _, _, _, _)| n != name); }
    for name in to_remove { s.pad_free.push(name); }
}

fn cull_spinners(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.spinner_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + SPINNER_W < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; obj.position = (-3500.0, -3500.0); obj.rotation_momentum = 0.0; }
        c.remove_light(&format!("spinner_light_{}", name));
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.spinner_live.retain(|n| !rm.contains(n.as_str()));
    s.spinner_origins.retain(|(id, _, _, _, _)| !rm.contains(id.as_str()));
    for name in to_remove { s.spinner_free.push(name); }
}

fn cull_coins(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.coin_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + COIN_R * 2.0 < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; obj.position = (-3700.0, -3700.0); }
        c.remove_light(&format!("coin_light_{}", name));
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.coin_live.retain(|n| !rm.contains(n.as_str()));
    s.coin_magnet_locked.retain(|n| !rm.contains(n.as_str()));
    for name in to_remove { s.coin_free.push(name); }
}

fn cull_flips(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.flip_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + FLIP_W < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; obj.position = (-3800.0, -3800.0); }
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.flip_live.retain(|n| !rm.contains(n.as_str()));
    for name in to_remove { s.flip_free.push(name); }
}

fn cull_score_x2(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.score_x2_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + SCORE_X2_W < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; obj.position = (-3850.0, -3850.0); }
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.score_x2_live.retain(|n| !rm.contains(n.as_str()));
    for name in to_remove { s.score_x2_free.push(name); }
}

fn cull_zero_g(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.zero_g_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + ZERO_G_W < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) { obj.visible = false; obj.position = (-3875.0, -3875.0); }
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.zero_g_live.retain(|n| !rm.contains(n.as_str()));
    for name in to_remove { s.zero_g_free.push(name); }
}

fn cull_gates(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.gate_live.iter()
        .filter(|n| {
            let top_id = format!("{n}_top");
            c.get_game_object(&top_id).map(|o| o.position.0 + GATE_W < cutoff).unwrap_or(true)
        })
        .cloned().collect();
    for name in &to_remove {
        let top_id = format!("{name}_top");
        let bot_id = format!("{name}_bot");
        if let Some(obj) = c.get_game_object_mut(&top_id) { obj.visible = false; obj.position = (-3900.0, -3900.0); }
        if let Some(obj) = c.get_game_object_mut(&bot_id) { obj.visible = false; obj.position = (-3900.0, -3900.0); }
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.gate_live.retain(|n| !rm.contains(n.as_str()));
    for name in to_remove { s.gate_free.push(name); }
}

fn cull_gravity_wells(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.gwell_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + o.size.0 < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-4000.0, -4000.0);
            obj.planet_radius = None; // disable gravity source
        }
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.gwell_live.retain(|n| !rm.contains(n.as_str()));
    s.gwell_timers.retain(|(n, _, _)| !rm.contains(n.as_str()));
    for name in to_remove { s.gwell_free.push(name); }
}

fn cull_turrets(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff = s.px - VW * 1.5;
    let to_remove: Vec<String> = s.turret_live.iter()
        .filter(|n| c.get_game_object(n).map(|o| o.position.0 + TURRET_FULL_SIZE < cutoff).unwrap_or(true))
        .cloned().collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-4500.0, -4500.0);
        }
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.turret_live.retain(|n| !rm.contains(n.as_str()));
    s.turret_timers.retain(|(n, _)| !rm.contains(n.as_str()));
    for name in to_remove { s.turret_free.push(name); }
}

fn cull_bullets(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let cutoff_x_lo = s.px - VW * 2.0;
    let cutoff_x_hi = s.px + VW * 2.0;
    let to_remove: Vec<String> = s.bullet_live.iter()
        .filter(|(n, _, _, ticks)| {
            *ticks == 0 || c.get_game_object(n).map(|o| {
                o.position.0 < cutoff_x_lo || o.position.0 > cutoff_x_hi
            }).unwrap_or(true)
        })
        .map(|(n, _, _, _)| n.clone())
        .collect();
    for name in &to_remove {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
            obj.position = (-5000.0, -5000.0);
        }
    }
    let rm: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
    s.bullet_live.retain(|(n, _, _, _)| !rm.contains(n.as_str()));
    for name in to_remove { s.bullet_free.push(name); }
}
