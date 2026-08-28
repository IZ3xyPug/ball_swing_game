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
    pulse_upgrade_nodes(c, st);
    tick_upgrade_interaction(c, st);
}

/// Pulse the upgrade-node ring so it reads as a live pickup rather than a dead
/// purple circle.
fn pulse_upgrade_nodes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let live: Vec<String> = st.lock().unwrap().upgrade_live.clone();
    if live.is_empty() {
        return;
    }
    let t = st.lock().unwrap().ticks as f32 * 0.08;
    let pulse = (t.sin() + 1.0) * 0.5;
    let w = 12.0 + 12.0 * pulse;
    for id in &live {
        if let Some(obj) = c.get_game_object_mut(id) {
            if !obj.visible {
                continue;
            }
            obj.set_glow(GlowConfig { color: Color(C_UPGRADE.0, C_UPGRADE.1, C_UPGRADE.2, 175), width: w });
        }
    }
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
        // Guarantee a hook node within tether range of the upgrade node so the
        // player can always leave the upgrade dialogue (and post-dialogue
        // stasis) by grabbing a node.
        if let Some(hid) = { let mut s2 = st.lock().unwrap(); s2.pool_free.pop() } {
            let hx = x + UPGRADE_R + HOOK_R + 60.0;
            let hy = y;
            let asteroid_mode = c.get_bool("asteroid_hooks_on");
            if let Some(obj) = c.get_game_object_mut(&hid) {
                obj.visible = true;
                obj.position = (hx - HOOK_R, hy - HOOK_R);
                obj.size = (HOOK_R * 2.0, HOOK_R * 2.0);
                obj.gravity = 0.0;
                obj.momentum = (0.0, 0.0);
                obj.rotation_momentum = 0.0;
                obj.collision_mode = CollisionMode::NonPlatform;
                obj.tags.retain(|t| t != BUFF_HOOK_TAG && t != SHIELD_HOOK_TAG && t != SPECIAL_HOOK_TAG && t != EXTENDED_HOOK_TAG);
                obj.tags.push("hook".into());
                if asteroid_mode {
                    obj.set_animation(super::helpers::hook_artifact_anim());
                    obj.size = (HOOK_ARTIFACT_R * 2.0, HOOK_ARTIFACT_R * 2.0);
                } else {
                    obj.set_image(super::helpers::hook_img(C_HOOK.0, C_HOOK.1, C_HOOK.2));
                }
            }
            let mut s2 = st.lock().unwrap();
            s2.live_hooks.push(hid);
            // NOTE: deliberately does NOT touch `rightmost_x`.
            //
            // `rightmost_x` means "how far the grab-node CHAIN extends", and it
            // gates the chain spawner (`rightmost_x < px + GEN_AHEAD`). This
            // companion node is placed beside an upgrade node up to 55 000 px
            // ahead of the player, so advancing the frontier to it told the
            // chain spawner it had already generated that far. Measured: on the
            // first second of a run the frontier jumped to 49 603 px with 37
            // free pool slots and 16 hooks still queued, and stayed blocked for
            // eight seconds — the whole stretch covered only by the
            // `ensure_player_hooks` failsafe.
        }
        s = st.lock().unwrap();
    }
}

// Run-persisting upgrade costs (cheap first buy, escalating per purchase this run).
fn run_heart_cost(s: &State) -> u32 {
    (UPGRADE_RUN_HEART_BASE as f32 * UPGRADE_RUN_HEART_GROWTH.powi(s.run_heart_buys as i32)).round() as u32
}
fn run_breath_cost(s: &State) -> u32 {
    (UPGRADE_BREATH_BASE as f32 * UPGRADE_BREATH_GROWTH.powi(s.run_breath_buys as i32)).round() as u32
}
fn run_momentum_cost(s: &State) -> u32 {
    (UPGRADE_MOMENTUM_BASE as f32 * UPGRADE_MOMENTUM_GROWTH.powi(s.run_momentum_buys as i32)).round() as u32
}

/// Current permanent-extra-heart price in meta currency.
fn perm_heart_cost() -> u64 {
    let g = crate::profile::profile();
    let owned = g.lock().unwrap().permanent_extra_hearts;
    crate::profile::permanent_heart_cost(owned)
}

// ── Dialogue interaction ─────────────────────────────────────────────────────

/// Open the dialogue when the player touches a node (unless already in it).
fn tick_upgrade_interaction(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    if st.lock().unwrap().upgrade_dialogue_active {
        return;
    }
    let (px, py) = {
        let s = st.lock().unwrap();
        (s.px, s.py)
    };
    let live: Vec<String> = st.lock().unwrap().upgrade_live.clone();
    for id in &live {
        let Some(obj) = c.get_game_object(id) else { continue; };
        if !obj.visible {
            continue;
        }
        let cx = obj.position.0 + obj.size.0 * 0.5;
        let cy = obj.position.1 + obj.size.1 * 0.5;
        let rr = PLAYER_R + UPGRADE_R;
        if (cx - px) * (cx - px) + (cy - py) * (cy - py) < rr * rr {
            open_dialogue(c, st, id.clone());
            return;
        }
    }
}

fn open_dialogue(c: &mut Canvas, st: &Arc<Mutex<State>>, node_id: String) {
    // Record where to hold the player (the node is consumed straight away so it
    // can never retrigger the dialogue while the player is still nearby).
    let (nx, ny) = c.get_game_object(&node_id)
        .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5))
        .unwrap_or((0.0, 0.0));
    {
        let mut s = st.lock().unwrap();
        s.upgrade_dialogue_active = true;
        s.upgrade_dialogue_node = node_id.clone();
        s.upgrade_hold_x = nx;
        s.upgrade_hold_y = ny;
        s.upgrade_hold_until_tether = false;
        s.hooked = false;
        s.active_hook = String::new();
        s.vx = 0.0;
        s.vy = 0.0;
        // Consume the node: remove from live and return to the free pool so it
        // can't be re-triggered.
        s.upgrade_live.retain(|n| n != &node_id);
        s.upgrade_free.push(node_id.clone());
    }
    if let Some(obj) = c.get_game_object_mut(&node_id) {
        obj.visible = false;
    }
    // Soft-pause the world (like the start stasis) so hazards/comets can't kill
    // the player while they're choosing. The pause menu is NOT opened.
    c.set_var("game_paused", true);
    c.run(Action::Hide { target: Target::name("rope") });
    c.set_var("rope_visible_at_pause", false);
    update_dialogue_text(c, st);
}

/// Close the dialogue; the player is held in stasis (world still paused) until
/// they tether.
///
/// Resuming REQUIRES a successful grab, so the exit is only reachable if a node
/// is actually within rope range of where the player is being held. The
/// companion node placed beside the upgrade node is normally that node — but it
/// is placed 30 000-55 000 px ahead of the player, sits in the shared hook pool
/// for that whole stretch, and is recycled wholesale by `clear_world_for_respawn`
/// on any heart loss. `upgrade_rightmost` is not rewound on respawn either, so
/// the upgrade node survives while its companion does not, and the player who
/// later reaches it closes the dialogue into a world with nothing to grab and
/// is stuck for good.
///
/// So the guarantee is re-established here, at the moment it is needed, rather
/// than assumed to have survived from tens of thousands of pixels earlier.
fn close_dialogue(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let mut s = st.lock().unwrap();
        s.upgrade_dialogue_active = false;
        s.upgrade_dialogue_node = String::new();
        s.upgrade_hold_until_tether = true;
        s.upgrade_hold_ticks = 0;
        s.vx = 0.0;
        s.vy = 0.0;
    }
    ensure_exit_node(c, st);
    hide_dialogue(c);
}

/// Guarantee a grabbable node within rope reach of the hold position.
fn ensure_exit_node(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (hx, hy, hooks) = {
        let s = st.lock().unwrap();
        (s.upgrade_hold_x, s.upgrade_hold_y, s.live_hooks.clone())
    };
    let reach = ROPE_LEN_MAX * 0.85; // comfortably inside the grab radius
    let have = hooks.iter().any(|id| {
        c.get_game_object(id)
            .map(|o| {
                if !o.visible {
                    return false;
                }
                let cx = o.position.0 + o.size.0 * 0.5;
                let cy = o.position.1 + o.size.1 * 0.5;
                (cx - hx) * (cx - hx) + (cy - hy) * (cy - hy) <= reach * reach
            })
            .unwrap_or(false)
    });
    if have {
        return;
    }

    let Some(id) = ({ let mut s = st.lock().unwrap(); s.pool_free.pop() }) else { return };
    let nx = hx + UPGRADE_R + HOOK_R + 60.0;
    let ny = hy;
    let asteroid_mode = c.get_bool("asteroid_hooks_on");
    if let Some(obj) = c.get_game_object_mut(&id) {
        obj.visible = true;
        obj.position = (nx - HOOK_R, ny - HOOK_R);
        obj.size = (HOOK_R * 2.0, HOOK_R * 2.0);
        obj.gravity = 0.0;
        obj.momentum = (0.0, 0.0);
        obj.rotation_momentum = 0.0;
        obj.collision_mode = CollisionMode::NonPlatform;
        obj.tags.retain(|t| {
            t != BUFF_HOOK_TAG && t != SHIELD_HOOK_TAG
                && t != SPECIAL_HOOK_TAG && t != EXTENDED_HOOK_TAG
        });
        if !obj.tags.iter().any(|t| t == "hook") {
            obj.tags.push("hook".into());
        }
        if asteroid_mode {
            obj.set_animation(super::helpers::hook_artifact_anim());
            obj.size = (HOOK_ARTIFACT_R * 2.0, HOOK_ARTIFACT_R * 2.0);
        } else {
            obj.animated_sprite = None;
            obj.set_image(super::helpers::hook_img(C_HOOK.0, C_HOOK.1, C_HOOK.2));
        }
    }
    // Auxiliary node: joins `live_hooks` but must NOT advance `rightmost_x`
    // (see the chain-frontier rule in `spawning.rs`).
    st.lock().unwrap().live_hooks.push(id);
}

/// Handle a dialogue key press. Returns true if the key was consumed.
pub fn upgrade_dialogue_key(c: &mut Canvas, st: &Arc<Mutex<State>>, key: &Key) -> bool {
    if !st.lock().unwrap().upgrade_dialogue_active {
        return false;
    }
    let opt: u8 = match key {
        Key::Character(ch) if ch == "1" => 1,
        Key::Character(ch) if ch == "2" => 2,
        Key::Character(ch) if ch == "3" => 3,
        Key::Character(ch) if ch == "4" => 4,
        Key::Character(ch) if ch == "5" => 5,
        Key::Named(NamedKey::Escape) => 5,
        _ => return false,
    };
    if opt == 5 {
        close_dialogue(c, st);
        return true;
    }
    buy_option(c, st, opt);
    update_dialogue_text(c, st);
    true
}

fn buy_option(c: &mut Canvas, st: &Arc<Mutex<State>>, opt: u8) {
    match opt {
        1 => {
            let (cost, coins) = {
                let s = st.lock().unwrap();
                (run_heart_cost(&s), s.coin_count)
            };
            if coins < cost {
                return;
            }
            let mut s = st.lock().unwrap();
            s.coin_count -= cost;
            s.max_hearts += 1;
            s.hearts += 1;
            s.run_heart_buys += 1;
        }
        2 => {
            if !crate::profile::buy_permanent_heart() {
                return;
            }
            let mut s = st.lock().unwrap();
            s.max_hearts += 1;
            s.hearts += 1;
        }
        3 => {
            let (cost, coins) = {
                let s = st.lock().unwrap();
                (run_breath_cost(&s), s.coin_count)
            };
            if coins < cost {
                return;
            }
            let mut s = st.lock().unwrap();
            s.coin_count -= cost;
            s.oxygen_drain_scale = UPGRADE_BREATH_DRAIN_SCALE.min(s.oxygen_drain_scale);
            s.run_breath_buys += 1;
        }
        4 => {
            let (cost, coins) = {
                let s = st.lock().unwrap();
                (run_momentum_cost(&s), s.coin_count)
            };
            if coins < cost {
                return;
            }
            let mut s = st.lock().unwrap();
            s.coin_count -= cost;
            s.upgrade_momentum_bonus = true;
            s.run_momentum_buys += 1;
        }
        _ => {}
    }
}

// ── Dialogue text (HUD) ─────────────────────────────────────────────────────

fn hide_dialogue(c: &mut Canvas) {
    for name in [
        "upgrade_dialogue_panel", "upgrade_dialogue_title", "upgrade_dialogue_meta",
        "upgrade_opt_0", "upgrade_opt_1", "upgrade_opt_2", "upgrade_opt_3", "upgrade_opt_4",
    ] {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.visible = false;
        }
    }
}

fn set_text(c: &mut Canvas, name: &str, text: &str, rgba: (u8, u8, u8, u8)) {
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
        let s = c.virtual_scale();
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                text, &font, 30.0 * s, Color(rgba.0, rgba.1, rgba.2, rgba.3), 900.0 * s,
            )));
            obj.visible = true;
        }
    }
}

fn update_dialogue_text(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (coins, r_h_b, r_b_b, r_m_b) = {
        let s = st.lock().unwrap();
        (s.coin_count, s.run_heart_buys, s.run_breath_buys, s.run_momentum_buys)
    };
    let rhc = (UPGRADE_RUN_HEART_BASE as f32 * UPGRADE_RUN_HEART_GROWTH.powi(r_h_b as i32)).round() as u32;
    let rbc = (UPGRADE_BREATH_BASE as f32 * UPGRADE_BREATH_GROWTH.powi(r_b_b as i32)).round() as u32;
    let rmc = (UPGRADE_MOMENTUM_BASE as f32 * UPGRADE_MOMENTUM_GROWTH.powi(r_m_b as i32)).round() as u32;
    let phc = perm_heart_cost();
    let (meta, perm_owned) = {
        let g = crate::profile::profile();
        let p = g.lock().unwrap();
        (p.meta_currency, p.permanent_extra_hearts)
    };

    if let Some(obj) = c.get_game_object_mut("upgrade_dialogue_panel") {
        obj.visible = true;
    }
    set_text(c, "upgrade_dialogue_title", "UPGRADE NODE", (255, 255, 255, 255));
    set_text(
        c, "upgrade_dialogue_meta",
        &format!("Coins: {coins}   \u{2022}   Meta: {meta}   \u{2022}   Perm. Hearts: {perm_owned}"),
        (200, 220, 255, 230),
    );

    let line = |afford: bool, body: String| {
        if afford {
            (body, (215u8, 255u8, 225u8, 255u8))
        } else {
            (format!("{body}   [not enough]"), (150u8, 160u8, 180u8, 200u8))
        }
    };

    let (t0, c0) = line(coins >= rhc, format!("1)  Extra Heart (this run)  \u{2014}  {rhc} coins"));
    set_text(c, "upgrade_opt_0", &t0, c0);
    let (t1, c1) = line(meta >= phc, format!("2)  +1 Permanent Heart  \u{2014}  {phc} meta"));
    set_text(c, "upgrade_opt_1", &t1, c1);
    let (t2, c2) = line(coins >= rbc, format!("3)  Controlled Breathing (run)  \u{2014}  {rbc} coins"));
    set_text(c, "upgrade_opt_2", &t2, c2);
    let (t3, c3) = line(coins >= rmc, format!("4)  Momentum (run)  \u{2014}  {rmc} coins"));
    set_text(c, "upgrade_opt_3", &t3, c3);
    set_text(c, "upgrade_opt_4", "5)  Close  (Esc)", (215, 230, 255, 255));
}
