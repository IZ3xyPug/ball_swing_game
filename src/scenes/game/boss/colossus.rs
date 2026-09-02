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

/// Boss-like layout offset for a part by index (Colossus), forming a distinct
/// body pose: the two hands hang at the sides, the torso is the centre, the head
/// sits above. Parts are near-still at idle (a slow, subtle, per-part breathing
/// bob) rather than orbiting, so the body reads as a creature with a lull — and
/// the parts never move in lockstep because each bob has its own phase. When
/// `bob` is false (the body is frozen during an attack) the part sits exactly at
/// its base pose.
pub(crate) fn boss_part_offset(i: u32, phase: f32, bob: bool) -> (f32, f32) {
    let base = match i {
        0 => (-1120.0,  280.0), // hand_l (hangs at the left)
        1 => ( 1120.0,  280.0), // hand_r (hangs at the right)
        2 => (   0.0,    0.0),  // torso (centre)
        _ => (   0.0, -1120.0), // head (above)
    };
    if !bob { return base; }
    // Slow, independent breathing: each part bobs on its own phase, so no two
    // parts move in unison, and the amplitude is small (near-still).
    let bob = (phase * 0.55 + i as f32 * 1.7).sin() * 24.0
            + (phase * 0.30 + i as f32 * 0.9).cos() * 14.0;
    (base.0 + bob * 0.5, base.1 + bob)
}

/// Ticks (at the player's momentum cap) it takes a part to lunge from `a` to
/// `b`, so the FSM knows when it has physically arrived at its target.
pub(crate) fn colossus_arrival(a: (f32, f32), b: (f32, f32)) -> u32 {
    let dist = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    (dist / MOMENTUM_CAP).ceil() as u32
}

/// Danger-zone radius for a Colossus part, matching the sized telegraph disc.
pub(crate) fn colossus_zone_r(id: &str) -> f32 {
    match id {
        "hand_l" | "hand_r" => COLOSSUS_HAND_ZONE_R,
        "torso"             => COLOSSUS_TORSO_ZONE_R,
        _                   => COLOSSUS_HEAD_ZONE_R,
    }
}

/// Idle length per part: a base plus a small per-part jitter so parts never
/// attack in lockstep (the pattern director additionally spaces them out).
pub(crate) fn colossus_idle_len(i: usize) -> u32 {
    // The head is timed against its own vulnerability window rather than the
    // shared idle: its next gravity well opens ~1 s after the window closes, so
    // the counter-attack ends cleanly instead of the next attack starting on
    // top of it. Index 3 is the head (see `boss_part_offset`).
    if i == 3 {
        return COLOSSUS_HEAD_VULN_AFTER + COLOSSUS_HEAD_REARM_GAP;
    }
    COLOSSUS_IDLE_TICKS + (i as u32 % 2) * 26 + (i as u32 * 7) % 17
}

/// Build the launch schedule for one storm.
///
/// Two properties do the work, and both were missing from the burst this
/// replaces:
///
///  * SEQUENTIAL. Meteors are 0.5-1.0 s apart, so the storm is a series of
///    dodges the player moves through rather than one instant that they either
///    happened to be clear of or did not. Three simultaneous rocks is a coin
///    flip; five spaced rocks is a skill.
///
///  * ALTERNATING SIDES. The side flips every meteor, so consecutive rocks come
///    from opposite halves of the sky and the player is pushed back and forth
///    instead of settling into one safe corner. Picking each angle at random
///    independently would cluster them — three from the left in a row is a
///    likely draw, and it reads as the boss doing the same thing three times.
///
/// Angles stay in `COLOSSUS_METEOR_ANGLE_MIN..MAX` (above and to the sides),
/// never from below.
pub(crate) fn meteor_storm_schedule(seed: &mut u64) -> Vec<(u32, f32)> {
    let mut out = Vec::with_capacity(COLOSSUS_METEOR_COUNT as usize);
    let mid = (COLOSSUS_METEOR_ANGLE_MIN + COLOSSUS_METEOR_ANGLE_MAX) * 0.5;
    // A short lead-in so the storm does not fire on the same frame the pose
    // lands — the player needs to see the torso commit before the first rock.
    let mut delay = COLOSSUS_METEOR_GAP_MIN;
    for n in 0..COLOSSUS_METEOR_COUNT {
        // Alternate halves, then jitter inside the half.
        let (lo, hi) = if n % 2 == 0 {
            (mid, COLOSSUS_METEOR_ANGLE_MAX)
        } else {
            (COLOSSUS_METEOR_ANGLE_MIN, mid)
        };
        let angle = lcg_range(seed, lo, hi);
        out.push((delay, angle));
        let gap = lcg_range(
            seed,
            COLOSSUS_METEOR_GAP_MIN as f32,
            COLOSSUS_METEOR_GAP_MAX as f32,
        );
        delay += gap
            .round()
            .clamp(COLOSSUS_METEOR_GAP_MIN as f32, COLOSSUS_METEOR_GAP_MAX as f32)
            as u32;
    }
    out
}

/// Launch any meteor whose delay has elapsed.
///
/// Runs unconditionally rather than only while the torso is attacking: a storm
/// already committed should finish even if the torso is destroyed mid-way. The
/// alternative — cancelling the queue on death — makes a kill silently delete
/// hazards already telegraphed on screen, which reads as the warnings lying.
pub(crate) fn tick_colossus_meteors(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let due: Vec<f32> = {
        let mut s = st.lock().unwrap();
        if s.boss_meteor_queue.is_empty() { return; }
        if s.dead || !s.boss_active {
            s.boss_meteor_queue.clear();
            return;
        }
        let mut due = Vec::new();
        for entry in s.boss_meteor_queue.iter_mut() {
            entry.0 = entry.0.saturating_sub(1);
        }
        s.boss_meteor_queue.retain(|(ticks, angle)| {
            if *ticks == 0 { due.push(*angle); false } else { true }
        });
        due
    };
    for angle in due {
        crate::scenes::game::spawning::spawn_comet_from_angle(c, st, angle);
    }
}

/// Draw and resolve the rotating plasma spokes.
///
/// Kept out of the shared part loop: the vent is not a lunge with a landing
/// circle, it is a rotating field around a stationary part, and threading that
/// through the lunge machinery would have meant a special case in every branch
/// of it. It reads the torso's FSM state and owns everything else.
///
/// The spokes damage on CONTACT with a cooldown rather than once per attack: a
/// rotating beam you are standing in would otherwise take every heart in the
/// second it takes to get out.
pub(crate) fn tick_core_vent(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (venting, ticks, buffed, px, py) = {
        let mut s = st.lock().unwrap();
        if s.boss_vent_hit_cooldown > 0 { s.boss_vent_hit_cooldown -= 1; }
        let venting = s.boss_active
            && !s.dead
            && !s.boss_stasis_active
            && torso_attack_for(s.boss_torso_attack) == TorsoAttack::CoreVent
            && s.boss_parts.iter().any(|p| {
                p.id == "torso" && p.alive && !p.shielded && p.state == PartState::Attack
            });
        let ticks = s.boss_parts.iter()
            .find(|p| p.id == "torso")
            .map(|p| p.state_ticks)
            .unwrap_or(0);
        (venting, ticks, s.player_buff > 0, s.px, s.py)
    };

    if !venting {
        for i in 0..COLOSSUS_VENT_SPOKES {
            if let Some(obj) = c.get_game_object_mut(&format!("colossus_vent_{i}")) {
                obj.visible = false;
            }
        }
        return;
    }

    // Where the torso is. The vent radiates from the chest, so the spokes are
    // anchored to the part rather than to the body anchor.
    let Some((tx, ty)) = c.get_game_object("colossus_part_2").map(|o| {
        (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5)
    }) else { return; };

    let spin = ticks as f32 * COLOSSUS_VENT_SPIN;
    let step = 360.0 / COLOSSUS_VENT_SPOKES as f32;
    let half = COLOSSUS_VENT_LENGTH * 0.5;
    let mut hit = false;

    for i in 0..COLOSSUS_VENT_SPOKES {
        let deg = spin + step * i as f32;
        let rad = deg.to_radians();
        let (dx, dy) = (rad.cos(), rad.sin());
        let tip = (tx + dx * COLOSSUS_VENT_LENGTH, ty + dy * COLOSSUS_VENT_LENGTH);
        let mid = (tx + dx * half, ty + dy * half);

        if let Some(obj) = c.get_game_object_mut(&format!("colossus_vent_{i}")) {
            obj.size = (COLOSSUS_VENT_LENGTH, COLOSSUS_VENT_THICKNESS);
            obj.rotation = deg;
            obj.position = (mid.0 - COLOSSUS_VENT_LENGTH * 0.5, mid.1 - COLOSSUS_VENT_THICKNESS * 0.5);
            obj.visible = true;
        }

        // Half the drawn thickness plus the player, as the gaze beam uses — the
        // damaging area is what is on screen.
        if point_segment_dist((px, py), (tx, ty), tip)
            < COLOSSUS_VENT_THICKNESS * 0.5 + PLAYER_R
        {
            hit = true;
        }
    }

    if !hit { return; }
    let on_cooldown = { st.lock().unwrap().boss_vent_hit_cooldown > 0 };
    if on_cooldown { return; }
    { st.lock().unwrap().boss_vent_hit_cooldown = COLOSSUS_VENT_HIT_COOLDOWN; }

    // Thrown clear of the torso, so a hit also solves the problem of being
    // inside the spokes — being hit twice in a row by the same rotation would
    // be the attack punishing the player for its own knockback.
    let dx = px - tx;
    let dy = py - ty;
    let d = (dx * dx + dy * dy).sqrt().max(1.0);
    let push = (dx / d * 62.0, dy / d * 62.0);
    {
        let mut s = st.lock().unwrap();
        s.vx = push.0;
        s.vy = push.1;
        s.hooked = false;
        s.active_hook = String::new();
    }
    c.run(Action::Hide { target: Target::name("rope") });
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.momentum = push;
    }
    c.set_var("boss_knockback_ticks", Value::I32(18));

    if buffed {
        // The buff shields the heart and spends an absorption, as it does for
        // every other Colossus attack.
        let mut s = st.lock().unwrap();
        if s.player_buff > 0 {
            s.buff_absorbs = s.buff_absorbs.saturating_sub(1);
            if s.buff_absorbs == 0 {
                s.player_buff = 0;
                s.buff_timer = 0;
            }
        }
    } else {
        let dead = { st.lock().unwrap().dead };
        if !dead { crate::scenes::game::hearts::lose_heart(c, st); }
    }
}

/// Expand and fade the clap's force-wave ring.
pub(crate) fn tick_clap_wave(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (ticks, at) = {
        let mut s = st.lock().unwrap();
        if s.boss_clap_wave == 0 {
            if let Some(obj) = c.get_game_object_mut("colossus_clap_wave") {
                obj.visible = false;
            }
            return;
        }
        s.boss_clap_wave -= 1;
        (s.boss_clap_wave, s.boss_clap_at)
    };
    let t = 1.0 - ticks as f32 / COLOSSUS_CLAP_WAVE_TICKS as f32;
    let r = COLOSSUS_CLAP_WAVE_R * t;
    if let Some(obj) = c.get_game_object_mut("colossus_clap_wave") {
        obj.size = (r * 2.0, r * 2.0);
        obj.position = (at.0 - r, at.1 - r);
        obj.visible = true;
    }
}

/// Run a multi-part boss fight. Parts live in `s.boss_parts`; the `boss` object
/// is the visual/body anchor. The spec's core loop applies to every multi-part
/// boss:
///  * Parts are shielded until their dependency is destroyed (phase gating).
///  * A buffed player hit near an unshielded part's weakpoint damages that part.
///  * Unprotected body contact costs the player a heart (contact-rule inversion).
///  * When the last part dies the fight is won.
///
/// The distinct attack behaviours per boss (hands, pulses, beams, segments) are
/// layered on top of this loop; this is the shared skeleton they run on.
pub(crate) fn tick_multi_part_boss(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.boss_active { return; }
    if s.dead { return; }
    // Entry/victory stasis: nothing to run yet.
    if s.boss_stasis_active { return; }

    // Appearance (once): reveal the boss body and set the banner name.
    if !s.boss_spawned {
        s.boss_spawned = true;
        let name = s.boss_kind.name();
        drop(s);
        let spawn_x = arena_center_x(c) - BOSS_SIZE * 0.5;
        let spawn_y = BOSS_Y_CENTER - BOSS_SIZE * 0.5;
        // The `boss` object is an anchor (for the off-screen arrow); the part
        // circles are the visible multi-part body, so don't show the square.
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (spawn_x, spawn_y);
            obj.visible = false;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(200, 60, 220, 255), 1000.0 * sc,
                )));
            }
        }
        s = st.lock().unwrap();
    }

    // Move the boss on a slow lissajous so its parts visibly travel with it.
    // The whole body FREEZES while any part is mid-attack (so the only thing
    // that moves is the attacking part), and stays frozen after the torso's
    // slam until the summoned meteors have fired and cleared.
    let body_still = {
        let any_attacking = s.boss_parts.iter().any(|p| {
            p.alive && !p.shielded && p.state != PartState::Idle
        });
        any_attacking || s.boss_meteor_lock_ticks > 0
    };
    if !body_still {
        s.boss_phase += 0.010;
    }
    {
        let phase = s.boss_phase;
        let x_liss = (phase * 2.1).sin();
        let y_liss = (phase * 1.5 + 0.5).sin();
        let nx = arena_center_x(c) + x_liss * 2600.0;
        let ny = BOSS_Y_CENTER + y_liss * 1360.0;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (nx - BOSS_SIZE * 0.5, ny - BOSS_SIZE * 0.5);
        }
    }

    let phase = s.boss_phase;
    let px = s.px;
    let py = s.py;
    let boss_pos = c.get_game_object("boss").map(|o| o.position).unwrap_or((-6000.0, -6000.0));
    let bcx = boss_pos.0 + BOSS_SIZE * 0.5;
    let bcy = boss_pos.1 + BOSS_SIZE * 0.5;
    drop(s);

    let buffed = { let g = st.lock().unwrap(); g.player_buff > 0 };
    let mut any_alive = false;

    // ── Colossus: per-part FSM (Idle → Telegraph → Attack → Recover) ──────
    // Each part is an independent body with its own state machine and a leash:
    // it never strays far from its home orbit unless it is attacking. The
    // pattern director (`boss_pattern_cooldown`) stops both hands attacking at
    // once. A part's danger zone is drawn while it Telegraphs so the player can
    // see exactly where it will strike ~1s before it lands.
    #[derive(Clone)]
    struct PartFrame {
        id: &'static str,
        alive: bool,
        shielded: bool,
        weak_open: bool,
        offset: (f32, f32),
        state_ticks: u32,
        zone_visible: bool,
        zone_solid: bool,
        zone_pos: (f32, f32),
        zone_r: f32,
        path_visible: bool,
        path_start: (f32, f32),
        strike_unhook: bool,
        strike_kick: (f32, f32),
        strike_heart: bool,
        strike_consume_absorb: bool,
        strike_big_throw: bool,
        /// This part is the torso, mid meteor storm.
        storm: bool,
        /// This part is the torso, mid core vent.
        vent: bool,
        /// hand_l only: the pair completed a clap this tick.
        clap_wave: bool,
        /// Head only: how far the current beam has swept, 0..1. `None` when no
        /// beam is firing this frame (winding up, or in the gap between shots).
        beam_t: Option<f32>,
        /// Head only: lateral bow of the current beam.
        beam_curve: f32,
    }

    let frames: Vec<PartFrame> = {
        let mut s = st.lock().unwrap();
        if s.boss_pattern_cooldown > 0 { s.boss_pattern_cooldown -= 1; }
        if s.boss_part_invuln_ticks > 0 { s.boss_part_invuln_ticks -= 1; }
        if s.boss_meteor_lock_ticks > 0 { s.boss_meteor_lock_ticks -= 1; }
        let hooked = s.hooked;
        // While the torso is mid-slam, the head rides along with the body, so it
        // moves together with the torso instead of sitting still.
        let mut torso_disp = (0.0_f32, 0.0_f32);
        let mut frames = Vec::with_capacity(s.boss_parts.len());
        for i in 0..s.boss_parts.len() {
            let home = boss_part_offset(i as u32, phase, !body_still);
            s.boss_parts[i].home_offset = home;
            let pid = s.boss_parts[i].id;
            let zone_r = colossus_zone_r(pid);
            // The torso performs BOTH of its attacks from where it stands — a
            // meteor storm calls rocks down, a core vent radiates from the chest
            // — so it never displaces, and the head has nothing to ride. Kept as
            // an explicit zero rather than deleted: the head's offset still adds
            // it, and a silent removal would be a puzzle the next time the torso
            // gains a moving attack.
            if pid == "torso" && s.boss_parts[i].alive {
                torso_disp = (0.0, 0.0);
            }
            if !s.boss_parts[i].alive {
                frames.push(PartFrame {
                    id: pid, alive: false, shielded: s.boss_parts[i].shielded,
                    weak_open: false, offset: home, state_ticks: 0, zone_visible: false, zone_solid: false,
                    zone_pos: (bcx + home.0, bcy + home.1), zone_r,
                    path_visible: false, path_start: (bcx + home.0, bcy + home.1),
                    strike_unhook: false, strike_kick: (0.0, 0.0), strike_heart: false,
                    strike_consume_absorb: false, strike_big_throw: false, storm: false, vent: false, clap_wave: false, beam_t: None, beam_curve: 0.0,
                });
                continue;
            }
            any_alive = true;

            // Shielded parts sit idle and are visible, but never attack nor open.
            if s.boss_parts[i].shielded {
                s.boss_parts[i].weakpoint_open = false;
                s.boss_parts[i].state = PartState::Idle;
                s.boss_parts[i].state_ticks = 0;
                // Cleared here as well as on the next telegraph: the frame a
                // shield drops is exactly the frame this must not be set.
                s.boss_parts[i].post_attack = false;
                frames.push(PartFrame {
                    id: pid, alive: true, shielded: true, weak_open: false,
                    offset: home, state_ticks: 0, zone_visible: false, zone_solid: false,
                    zone_pos: (bcx + home.0, bcy + home.1), zone_r,
                    path_visible: false, path_start: (bcx + home.0, bcy + home.1),
                    strike_unhook: false, strike_kick: (0.0, 0.0), strike_heart: false,
                    strike_consume_absorb: false, strike_big_throw: false, storm: false, vent: false, clap_wave: false, beam_t: None, beam_curve: 0.0,
                });
                continue;
            }

            // ── FSM advance ──
            let cooldown_ok = s.boss_pattern_cooldown == 0;
            let vx = s.vx;
            let vy = s.vy;
            let mut began_attack = false;
            // Which attack the torso is on. Read BEFORE the FSM may bump it, so
            // the whole frame agrees; `next_torso_attack` applies the bump after.
            let mut torso_storm = torso_attack_for(s.boss_torso_attack) == TorsoAttack::MeteorStorm;
            // Both hands read ONE counter, so a clap is a decision the pair
            // makes rather than two hands happening to agree.
            //
            // And it takes TWO hands. Gated on both being in the fight rather
            // than on the counter alone, which means a hand destroyed mid-clap
            // drops the survivor straight back to lunge rules on the next frame
            // — including its vulnerability window. Left to the counter, a lone
            // hand would keep performing a clap it cannot complete AND keep the
            // clap's "only hittable once home" rule, so killing one hand would
            // have made the other one harder to kill.
            let both_hands_ready = s
                .boss_parts
                .iter()
                .filter(|q| (q.id == "hand_l" || q.id == "hand_r") && q.alive && !q.shielded)
                .count()
                == 2;
            let mut hand_clap =
                both_hands_ready && hand_attack_for(s.boss_hand_attack) == HandAttack::Clap;
            // Both hands lunge at the same speed cap from different distances,
            // so they do not arrive together. The clap is the moment the LATER
            // one lands — the nearer hand waits at the point and the second
            // slams into it, which is what the impact should look like.
            let clap_tick = {
                let arr = |id: &str| {
                    s.boss_parts
                        .iter()
                        .find(|q| q.id == id && q.alive && !q.shielded)
                        .map(|q| colossus_arrival(q.attack_start, q.target))
                };
                match (arr("hand_l"), arr("hand_r")) {
                    (Some(a), Some(b)) => a.max(b),
                    (Some(a), None) | (None, Some(a)) => a,
                    (None, None) => 0,
                }
            };
            let mut next_torso_attack = false;
            let mut next_hand_attack = false;
            let mut begin_clap = false;
            let mut start_storm = false;
            let mut roll_beam_shot = false;
            let mut roll_burst = false;
            {
                let p = &mut s.boss_parts[i];
                match p.state {
                    PartState::Idle => {
                        p.state_ticks += 1;
                        if cooldown_ok && p.state_ticks >= colossus_idle_len(i) {
                            p.state = PartState::Telegraph;
                            p.state_ticks = 0;
                            p.post_attack = false;
                            // Commit the torso to its next attack here, not at
                            // the strike: the wind-up, the pose, the hit and the
                            // vulnerability window all have to agree about which
                            // attack this is, and the telegraph is the first of
                            // them to be drawn.
                            if p.id == "torso" { next_torso_attack = true; }
                            // Same for the hands, except the decision is made
                            // once for the pair — hand_l speaks for both, and a
                            // clap drags hand_r into the same telegraph on the
                            // same tick.
                            if p.id == "hand_l" { next_hand_attack = true; }
                            // The hands aim at (a slight prediction ahead of) the
                            // player's position RIGHT NOW, then the path is locked
                            // — no homing. The lead makes it feel intelligent and,
                            // because the path is telegraphed, readable.
                            //
                            // The torso slams at the player (radial AoE), and the
                            // head aims its gaze beam at the player — both also
                            // lead slightly, and both paths are locked here.
                            let aim = (px + vx * COLOSSUS_ATTACK_LEAD, py + vy * COLOSSUS_ATTACK_LEAD);
                            let home_world = (bcx + home.0, bcy + home.1);
                            let world_target = if p.id == "head" {
                                // The head fires a RAY of fixed length through
                                // the aim point rather than stopping at it, so
                                // the beam sweeps past the player and keeps
                                // going. Stopping at the player made standing
                                // beyond the aim point unconditionally safe.
                                beam_end(home_world, aim)
                            } else {
                                leash_clamp(home_world, aim, COLOSSUS_LEASH)
                            };
                            p.target = (world_target.0 - bcx, world_target.1 - bcy);
                            p.path_start = home_world;
                            began_attack = true;
                        }
                    }
                    PartState::Telegraph => {
                        p.state_ticks += 1;
                        if p.state_ticks >= COLOSSUS_TELEGRAPH_TICKS {
                            p.state = PartState::Attack;
                            p.state_ticks = 0;
                            // Record where the part launches from (its wind-up
                            // position), so the lunge travels at the cap rather
                            // than teleporting to the target.
                            let home = p.home_offset;
                            let tgt = p.target;
                            let dx = home.0 - tgt.0;
                            let dy = home.1 - tgt.1;
                            let d = (dx * dx + dy * dy).sqrt().max(0.001);
                            p.attack_start = (
                                home.0 + dx / d * COLOSSUS_TELEGRAPH_PULL,
                                home.1 + dy / d * COLOSSUS_TELEGRAPH_PULL,
                            );
                            // Re-arm the head's gaze beam so a new sweep can hit.
                            p.beam_hit_done = false;
                            if p.id == "head" { roll_beam_shot = true; roll_burst = true; }
                            if p.id == "torso" && torso_storm { start_storm = true; }
                        }
                    }
                    PartState::Attack => {
                        p.state_ticks += 1;
                        // The head holds its beak while firing the gaze; the hands
                        // and torso hold at the lunge target after arriving (the
                        // torso for longer, so the meteors can clear).
                        // Each beam in the burst re-aims at wherever the player
                        // is NOW, so dodging the first one is the start of the
                        // attack rather than the end of it.
                        if p.id == "head"
                            && p.state_ticks > 0
                            && p.state_ticks % beam_shot_len() == 0
                            && p.state_ticks / beam_shot_len() < p.beam_shots
                        {
                            roll_beam_shot = true;
                        }
                        let duration = if p.id == "head" {
                            p.beam_shots * beam_shot_len()
                        } else if p.id == "torso" {
                            if torso_storm { COLOSSUS_STORM_TICKS } else { COLOSSUS_VENT_TICKS }
                        } else {
                            let arrival = colossus_arrival(p.attack_start, p.target);
                            // Only the hands reach this branch — the head and
                            // the torso both attack from where they stand.
                            arrival + COLOSSUS_HOLD_TICKS
                        };
                        if p.state_ticks >= duration {
                            p.state = PartState::Recover;
                            p.state_ticks = 0;
                            // The window that follows belongs to THIS attack.
                            p.post_attack = true;
                        }
                    }
                    PartState::Recover => {
                        p.state_ticks += 1;
                        if p.state_ticks >= COLOSSUS_RECOVER_TICKS {
                            p.state = PartState::Idle;
                            p.state_ticks = 0;
                        }
                    }
                }
            }
            if roll_burst {
                let n = lcg_range(
                    &mut s.seed,
                    COLOSSUS_BEAM_SHOTS_MIN as f32,
                    COLOSSUS_BEAM_SHOTS_MAX as f32 + 0.999,
                ) as u32;
                s.boss_parts[i].beam_shots = n.clamp(COLOSSUS_BEAM_SHOTS_MIN, COLOSSUS_BEAM_SHOTS_MAX);
            }
            if roll_beam_shot {
                // Re-aim at the player's CURRENT position, and roll a fresh
                // curve, so no two beams in a burst are the same problem.
                let straight = lcg_range(&mut s.seed, 0.0, 1.0) > COLOSSUS_BEAM_CURVE_CHANCE;
                let curve = if straight {
                    0.0
                } else {
                    lcg_range(&mut s.seed, -COLOSSUS_BEAM_CURVE_MAX, COLOSSUS_BEAM_CURVE_MAX)
                };
                let start = (bcx + s.boss_parts[i].home_offset.0, bcy + s.boss_parts[i].home_offset.1);
                let end = beam_end(start, (px, py));
                let p = &mut s.boss_parts[i];
                p.beam_curve = curve;
                p.path_start = start;
                p.target = (end.0 - bcx, end.1 - bcy);
                p.beam_hit_done = false;
            }
            if next_torso_attack {
                s.boss_torso_attack = s.boss_torso_attack.wrapping_add(1);
                // Refresh the frame's copy too. Without this the FIRST frame of
                // a telegraph still describes the PREVIOUS attack — the glow,
                // the landing circle and the vulnerability all disagree with the
                // pose for one frame, which is exactly long enough to flicker.
                torso_storm = torso_attack_for(s.boss_torso_attack) == TorsoAttack::MeteorStorm;
            }
            if next_hand_attack {
                s.boss_hand_attack = s.boss_hand_attack.wrapping_add(1);
                hand_clap = both_hands_ready
                    && hand_attack_for(s.boss_hand_attack) == HandAttack::Clap;
                begin_clap = hand_clap;
            }
            if begin_clap {
                // A clap is the one moment the fight suspends its own
                // one-part-at-a-time rule: hand_r is dragged into the same
                // telegraph on the same tick, with the same target, so the two
                // hands wind up on opposite sides of it and arrive together.
                let (target, path_start) = {
                    let l = &s.boss_parts[i];
                    (l.target, l.path_start)
                };
                if let Some(j) = s.boss_parts.iter().position(|q| q.id == "hand_r" && q.alive && !q.shielded) {
                    let r = &mut s.boss_parts[j];
                    r.state = PartState::Telegraph;
                    r.state_ticks = 0;
                    r.target = target;
                    // hand_r starts its wind-up from its OWN home, so the pull
                    // back is mirrored rather than duplicated.
                    r.path_start = (bcx + r.home_offset.0, bcy + r.home_offset.1);
                    let _ = path_start;
                }
            }
            if began_attack {
                s.boss_pattern_cooldown = COLOSSUS_PATTERN_COOLDOWN;
            }
            if start_storm {
                s.boss_meteor_queue = meteor_storm_schedule(&mut s.seed);
                // Hold the body still for the whole storm, so the only things
                // moving on screen are the meteors the player has to read.
                s.boss_meteor_lock_ticks = COLOSSUS_STORM_TICKS + COMET_WARN_TOTAL;
            }

            let (off, weak_open, strike) = {
                let p = &s.boss_parts[i];
                let target = p.target;
                // Parts that attack from where they stand rather than lunging:
                // the head (gravity well + gaze beam from its perch) and the
                // torso while it is calling down a meteor storm. Everything
                // else travels to a telegraphed point.
                let head_stays = p.id == "head" || p.id == "torso";
                let off = match p.state {
                    PartState::Idle => home,
                    PartState::Telegraph => {
                        if head_stays { home } else {
                            // Pull back from the target to visibly wind up.
                            let dx = home.0 - target.0;
                            let dy = home.1 - target.1;
                            let d = (dx * dx + dy * dy).sqrt().max(0.001);
                            (home.0 + dx / d * COLOSSUS_TELEGRAPH_PULL,
                             home.1 + dy / d * COLOSSUS_TELEGRAPH_PULL)
                        }
                    }
                    // The attack lunge and the return both move at the player's
                    // momentum cap, so the boss never out-runs the player.
                    PartState::Attack if head_stays => home,
                    PartState::Attack => capped_toward(p.attack_start, target, p.state_ticks, MOMENTUM_CAP),
                    PartState::Recover if head_stays => home,
                    PartState::Recover => capped_toward(target, home, p.state_ticks, MOMENTUM_CAP),
                };
                // The head rides along with the torso's slam so the whole body
                // moves together when the torso lunges (and sits still otherwise).
                let off = if p.id == "head" {
                    (off.0 + torso_disp.0, off.1 + torso_disp.1)
                } else { off };
                // Weakpoint timing:
                //  * Hands/torso: opens 0.5s AFTER the part arrives at the
                //    telegraphed target (not on arrival), stays open through the
                //    hold, the slow retract, and 1s after getting home.
                //  * Head: NOT vulnerable while it fires the gaze / gravity well;
                //    it opens after (Recover) and for a long post-well window.
                let arrival = if p.id == "head" { 0 } else { colossus_arrival(p.attack_start, target) };
                let vuln_after = if p.id == "head" { COLOSSUS_HEAD_VULN_AFTER } else { COLOSSUS_VULN_AFTER_TICKS };
                // A meteor storm is a pure dodge phase: the torso is not
                // hittable during it, through its recovery, or in the lull
                // after. The window to damage the torso belongs to the SLAM, so
                // the two beats stay distinct — survive one, punish the other.
                let weak_open = if p.id == "torso" {
                    if torso_storm {
                        // A meteor storm is a pure dodge phase: not hittable
                        // during it, through its recovery, or in the lull after.
                        // The window belongs to the vent, so the two beats stay
                        // distinct — survive one, punish the other.
                        false
                    } else {
                        // The core vent is dangerous AND vulnerable at once.
                        // Opens half a second into the rotation (so the wind-up
                        // is not free) and stays open well past the end of the
                        // vent.
                        //
                        // The after-window is measured in TICKS SINCE THE VENT
                        // ENDED, not per FSM state, because it now outlasts the
                        // recovery and runs on into the idle. Expressed per
                        // state it would have been capped at the recovery's
                        // length without anything saying so — the window would
                        // have silently stopped growing when the constant went
                        // past 70.
                        match p.state {
                            PartState::Attack => p.state_ticks >= COLOSSUS_VENT_VULN_DELAY,
                            PartState::Recover => p.state_ticks < COLOSSUS_VENT_VULN_AFTER,
                            PartState::Idle => {
                                p.post_attack
                                    && COLOSSUS_RECOVER_TICKS + p.state_ticks
                                        < COLOSSUS_VENT_VULN_AFTER
                            }
                            _ => false,
                        }
                    }
                } else if p.id == "hand_l" || p.id == "hand_r" {
                    if hand_clap {
                        // After a clap the hands are only hittable once they are
                        // HOME. Not while they are jammed together mid-arena:
                        // the reward for reading a clap should be a clean window
                        // at a known place, not a scramble to the middle of the
                        // arena with everything else still live.
                        p.post_attack
                            && p.state == PartState::Idle
                            && p.state_ticks < COLOSSUS_CLAP_VULN_AFTER
                    } else {
                        match p.state {
                            PartState::Attack => p.state_ticks >= arrival + COLOSSUS_ATTACK_VULN_DELAY,
                            PartState::Recover => true,
                            PartState::Idle => p.post_attack && p.state_ticks < vuln_after,
                            _ => false,
                        }
                    }
                } else {
                    match p.state {
                        PartState::Attack => {
                            p.id != "head"
                                && p.state_ticks >= arrival + COLOSSUS_ATTACK_VULN_DELAY
                        }
                        PartState::Recover => true,
                        // `post_attack` is what stops a part being wide open the
                        // instant its shield drops. The head unshields when the
                        // torso dies and lands in Idle at tick 0, which matched
                        // this window exactly — so it could be killed on the
                        // spot without ever attacking.
                        PartState::Idle => p.post_attack && p.state_ticks < vuln_after,
                        _ => false,
                    }
                };
                // Strike:
                //  * Hands + torso: the moment the lunge physically reaches the
                //    telegraphed zone (the hit and the motion line up).
                //  * Head: the gaze beam is a travelling sweep handled by the
                //    application loop, so the FSM does not strike for it here.
                let strike = if p.state == PartState::Attack {
                    // During a clap both hands land on the SAME point, so
                    // letting each resolve its own hit would cost two hearts
                    // for one attack. hand_l resolves it for the pair.
                    if head_stays || (hand_clap && p.id == "hand_r") {
                        false
                    } else {
                        let dist = ((target.0 - p.attack_start.0).powi(2)
                                  + (target.1 - p.attack_start.1).powi(2)).sqrt();
                        let now = MOMENTUM_CAP * p.state_ticks as f32 >= dist;
                        let prev = MOMENTUM_CAP * p.state_ticks.saturating_sub(1) as f32 >= dist;
                        // `now && (state_ticks == 1 || !prev)` catches the arrival
                        // even when the target is right next to the launch point
                        // (dist ~ 0, where `!prev` is false from tick 1).
                        now && (p.state_ticks == 1 || !prev)
                    }
                } else { false };
                (off, weak_open, strike)
            };

            {
                let p = &mut s.boss_parts[i];
                p.weakpoint_open = weak_open;
            }

            let zone_pos = (bcx + s.boss_parts[i].target.0, bcy + s.boss_parts[i].target.1);
            // "Storm, and currently performing it". `torso_storm` alone is
            // also true through the lull AFTER a storm (the counter is only
            // bumped on the next telegraph), which is what keeps the weakpoint
            // shut in that lull — but it must not keep the summoning glow lit.
            let performing = matches!(s.boss_parts[i].state, PartState::Telegraph | PartState::Attack);
            let storm_frame = pid == "torso" && torso_storm && performing;
            let vent_frame = pid == "torso" && !torso_storm && performing;
            // Neither torso attack travels, so neither gets a landing circle or
            // a trajectory strip — both would promise an impact that never comes.
            let torso_frame = storm_frame || vent_frame;
            let (zone_visible, zone_solid) = {
                let p = &s.boss_parts[i];
                // A meteor storm has no landing circle: nothing lunges, so a
                // disc on the ground would promise an impact that never comes.
                // Its telegraph is the torso's own summoning glow plus each
                // meteor's two-second warning marker.
                if torso_frame {
                    (false, false)
                } else if p.id == "head" {
                    // The well stays open for the whole attack; the beam is
                    // only "solid" while a shot is actually sweeping, so the
                    // gaps between beams show the charge orb recharging.
                    let firing = p.state == PartState::Attack
                        && (p.state_ticks % beam_shot_len()) < COLOSSUS_BEAM_TICKS;
                    (p.state == PartState::Telegraph || p.state == PartState::Attack, firing)
                } else {
                    (p.state == PartState::Telegraph || p.state == PartState::Attack,
                     p.state == PartState::Attack)
                }
            };
            let (beam_t, beam_curve) = {
                let p = &s.boss_parts[i];
                if p.id == "head" && p.state == PartState::Attack {
                    let shot_t = p.state_ticks % beam_shot_len();
                    if shot_t < COLOSSUS_BEAM_TICKS {
                        (Some(shot_t as f32 / COLOSSUS_BEAM_TICKS as f32), p.beam_curve)
                    } else {
                        (None, p.beam_curve)
                    }
                } else {
                    (None, p.beam_curve)
                }
            };
            // The path telegraph (trajectory) is shown while the part winds up,
            // from where it started the telegraph to where it will strike.
            let (path_visible, path_start) = {
                let p = &s.boss_parts[i];
                // The head shows its path through the wind-up and while a beam
                // is actually sweeping — but NOT in the gap between beams of a
                // burst. During the gap the path still describes the PREVIOUS
                // shot (the next one is re-aimed when it starts), so leaving it
                // up both lies about where the next beam goes and keeps a
                // full-length translucent quad on screen for half a second per
                // shot. The charge orb covers the "recharging" read.
                let head_showing = p.id == "head"
                    && (p.state == PartState::Telegraph || beam_t.is_some());
                (!torso_frame
                 && (head_showing
                     || (p.id != "head" && p.state == PartState::Telegraph)),
                 p.path_start)
            };
            let mut strike_unhook = false;
            let mut strike_kick = (0.0f32, 0.0f32);
            let mut strike_heart = false;
            let mut strike_consume_absorb = false;
            let mut strike_big_throw = false;
            if strike {
                // Hit detection:
                //  * Hands/torso: the player is in the radial danger zone.
                //  * Head: the player is on the gaze-beam line (path_start → target).
                let hit = if pid == "head" {
                    let (ax, ay) = (s.boss_parts[i].path_start.0, s.boss_parts[i].path_start.1);
                    point_segment_dist((px, py), (ax, ay), zone_pos) < COLOSSUS_PATH_THICKNESS + PLAYER_R
                } else {
                    (px - zone_pos.0).powi(2) + (py - zone_pos.1).powi(2)
                        < (PLAYER_R + zone_r).powi(2)
                };
                if hit {
                    let d = ((px - zone_pos.0).powi(2) + (py - zone_pos.1).powi(2)).sqrt().max(1.0);
                    // A direct hit throws the player HARD (much further than a
                    // regular body-contact push), so being caught in an attack
                    // is dangerous even when the buff protects your hearts.
                    let power = 78.0;
                    strike_kick = ((px - zone_pos.0) / d * power, (py - zone_pos.1) / d * power);
                    strike_big_throw = true;
                    if buffed {
                        // The buff shields the heart, but you still get flung and
                        // spend one absorption.
                        strike_unhook = hooked;
                        strike_consume_absorb = true;
                    } else {
                        strike_heart = true;
                    }
                }
                // Meteors are the torso's OTHER attack now, queued when the
                // storm begins — not a rider on every slam. See
                // `queue_meteor_storm`.
            }

            frames.push(PartFrame {
                id: pid, alive: true, shielded: false, weak_open, offset: off,
                state_ticks: s.boss_parts[i].state_ticks, zone_visible, zone_solid, zone_pos, zone_r,
                path_visible, path_start,
                strike_unhook, strike_kick, strike_heart, strike_consume_absorb,
                strike_big_throw, storm: storm_frame, vent: vent_frame,
                clap_wave: hand_clap
                    && pid == "hand_l"
                    && s.boss_parts[i].state == PartState::Attack
                    && s.boss_parts[i].state_ticks == clap_tick,
                beam_t, beam_curve,
            });
        }
        frames
    };

    // ── Apply per-part visuals + resolve strikes ──
    for (idx, f) in frames.iter().enumerate() {
        let part_size = colossus_part_size(idx as u32);

        // The head opens a large gravity well during its wind-up and beam, so it
        // drags the player toward the head — and FORCES an untether so the
        // player is dragged in freely instead of just swinging on their rope.
        if f.id == "head" {
            let gx = bcx + f.offset.0;
            let gy = bcy + f.offset.1;
            if f.alive && f.zone_visible {
                // The pull is STRONGEST far from the head (it drags you in from
                // across the arena) and weakens toward the core, so you aren't
                // flung once you're close — you just get dragged in. It is
                // applied to the STATE velocity (not the object's momentum) so
                // the momentum write-back can't wipe it, and it's strong enough
                // at the outer edge to haul you in after you're untethered.
                let dx = px - gx;
                let dy = py - gy;
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                // Steeper ramp than linear: the pull stays strong across most of
                // the well and only falls off in a small region at the core, so
                // it reliably drags you in from the outer edge.
                let strength = COLOSSUS_GRAVITY_STRENGTH * (d / COLOSSUS_GRAVITY_RANGE).clamp(0.0, 1.0).powf(0.35);
                let mut unhooked = false;
                {
                    let mut s = st.lock().unwrap();
                    if strength > 0.01 {
                        s.vx -= dx / d * strength;
                        s.vy -= dy / d * strength;
                    }
                    // Force the untether only for the first ~0.5s of the well, so
                    // the player can re-tether and save themselves from the beam
                    // rather than being sucked to a guaranteed hit.
                    if !f.zone_solid && f.state_ticks < 30 {
                        if s.hooked {
                            s.hooked = false;
                            s.active_hook = String::new();
                            unhooked = true;
                        }
                    }
                }
                if unhooked {
                    c.run(Action::Hide { target: Target::name("rope") });
                }
                // While the well is active, flatten the player's own gravity so
                // the pull wins and they can't free-fall to their doom.
                if let Some(obj) = c.get_game_object_mut("player") {
                    let sign = if obj.gravity < 0.0 { -1.0 } else { 1.0 };
                    obj.gravity = GRAVITY * 0.02 * sign;
                }
                // Gravity well visual.
                if let Some(obj) = c.get_game_object_mut("colossus_well") {
                    obj.position = (gx - COLOSSUS_GRAVITY_RANGE, gy - COLOSSUS_GRAVITY_RANGE);
                    obj.visible = true;
                }
            } else {
                if let Some(obj) = c.get_game_object_mut("colossus_well") {
                    obj.visible = false;
                }
                // The well is over: restore the arena's (reduced) gravity so the
                // player plays normally again.
                if let Some(obj) = c.get_game_object_mut("player") {
                    let sign = if obj.gravity < 0.0 { -1.0 } else { 1.0 };
                    obj.gravity = GRAVITY * BOSS_GRAVITY_SCALE * sign;
                }
            }

            // Charge-up orb: grows at the head while it winds up, so the gaze
            // clearly reads as "about to fire".
            if f.alive && f.zone_visible && !f.zone_solid {
                let charge = (f.state_ticks as f32 / COLOSSUS_TELEGRAPH_TICKS as f32).clamp(0.0, 1.0);
                let r = 70.0 + charge * 120.0;
                if let Some(obj) = c.get_game_object_mut("colossus_charge") {
                    obj.size = (r * 2.0, r * 2.0);
                    obj.position = (gx - r, gy - r);
                    obj.visible = true;
                }
            } else if let Some(obj) = c.get_game_object_mut("colossus_charge") {
                obj.visible = false;
            }

            // Bright beam core: sweeps from the head out along the path as it
            // fires. Drawn as a polyline so a curved beam is drawn by the same
            // code as a straight one.
            if f.alive && f.zone_solid {
                let t = f.beam_t.unwrap_or(0.0);
                let pts = beam_polyline(f.path_start, f.zone_pos, f.beam_curve, t.max(0.001));
                draw_beam_strip(c, "colossus_beam_core", &pts, COLOSSUS_BEAM_THICKNESS * 0.46);
                let front = *pts.last().unwrap_or(&f.path_start);

                // Little contact explosions: as the beam sweeps the path, pops
                // appear at the beam front and quickly grow a few sizes, so it
                // reads as the beam detonating along the telegraphed line.
                {
                    let mut updates: Vec<(String, f32, f32, f32, bool)> = Vec::new();
                    let mut new_pop: Option<(String, f32, f32, u32)> = None;
                    {
                        let mut s = st.lock().unwrap();
                        let mut i = 0;
                        while i < s.beam_explode_live.len() {
                            let (id, x, y, ttl) = s.beam_explode_live[i].clone();
                            let nttl = ttl.saturating_sub(1);
                            if nttl == 0 {
                                s.beam_explode_live.remove(i);
                                updates.push((id, x, y, 0.0, false));
                                continue;
                            }
                            s.beam_explode_live[i].3 = nttl;
                            let growth = (COLOSSUS_BEAM_EXPLODE_TTL as f32 - nttl as f32)
                                / COLOSSUS_BEAM_EXPLODE_TTL as f32;
                            let r = COLOSSUS_BEAM_EXPLODE_R0
                                + growth * (COLOSSUS_BEAM_EXPLODE_R1 - COLOSSUS_BEAM_EXPLODE_R0);
                            updates.push((id, x, y, r, true));
                            i += 1;
                        }
                        // Spawn a new pop at the beam front every few ticks.
                        // Every 3 kept up to 8 large glowing circles alive at
                        // once over the beam, which is a lot of overdraw on top
                        // of an already-large beam.
                        if f.state_ticks % 6 == 0 {
                            let live_ids: Vec<&String> =
                                s.beam_explode_live.iter().map(|(id, _, _, _)| id).collect();
                            let free = (0..COLOSSUS_BEAM_EXPLODE_MAX_LIVE)
                                .map(|i| format!("colossus_beam_explode_{i}"))
                                .find(|id| !live_ids.contains(&id));
                            if let Some(id) = free {
                                s.beam_explode_live.push((id.clone(), front.0, front.1, COLOSSUS_BEAM_EXPLODE_TTL));
                                new_pop = Some((id, front.0, front.1, COLOSSUS_BEAM_EXPLODE_TTL));
                            }
                        }
                    }
                    for (id, x, y, r, vis) in updates {
                        if let Some(obj) = c.get_game_object_mut(&id) {
                            obj.size = (r * 2.0, r * 2.0);
                            obj.position = (x - r, y - r);
                            obj.visible = vis;
                        }
                    }
                    if let Some((id, x, y, _ttl)) = new_pop {
                        if let Some(obj) = c.get_game_object_mut(&id) {
                            let r = COLOSSUS_BEAM_EXPLODE_R0;
                            obj.size = (r * 2.0, r * 2.0);
                            obj.position = (x - r, y - r);
                            obj.visible = true;
                        }
                    }
                }
            } else {
                hide_beam_strip(c, "colossus_beam_core");
            }
            // When the beam is not firing, also stop the explosion trail.
            if !(f.alive && f.zone_visible) {
                let mut s = st.lock().unwrap();
                for (id, _, _, _) in s.beam_explode_live.drain(..) {
                    if let Some(obj) = c.get_game_object_mut(&id) {
                        obj.visible = false;
                    }
                }
            }
        }

        // Buffed hit on an exposed (unshielded, weakpoint-open) part damages it.
        // The boss has a short invulnerability window after a part is destroyed,
        // so two parts can't be killed back-to-back within the same second.
        if f.alive && !f.shielded && f.weak_open && buffed {
            let sx = bcx + f.offset.0;
            let sy = bcy + f.offset.1;
            let hit_r = colossus_part_hit_r(idx as u32);
            if (px - sx).powi(2) + (py - sy).powi(2) < (PLAYER_R + hit_r).powi(2) {
                let mut s = st.lock().unwrap();
                if s.boss_part_invuln_ticks == 0 {
                    if let Some(p) = s.boss_parts.iter_mut().find(|p| p.id == f.id && p.alive) {
                        p.hp -= 1;
                        if p.hp <= 0 {
                            p.alive = false;
                            s.boss_part_invuln_ticks = COLOSSUS_PART_INVULN_TICKS;
                        }
                        s.buff_hit_flash = 20;
                    }
                }
            }
        }

        // Part body (composite silhouette, sized to the part).
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_part_{idx}")) {            if f.alive {
                let (sx, sy) = (bcx + f.offset.0, bcy + f.offset.1);
                let half = part_size * 0.5;
                obj.size = (part_size, part_size);
                obj.position = (sx - half, sy - half);
                obj.visible = true;
                if f.shielded {
                    obj.set_glow(GlowConfig { color: Color(120, 220, 255, 90), width: 22.0 });
                } else if f.weak_open {
                    // VULNERABLE: a bright, pulsing gold glow — the "hit me now"
                    // cue. Takes priority so the strike window is unmistakable.
                    let pulse = 170 + (((f.zone_pos.0 as i32 / 4) + (f.zone_pos.1 as i32 / 4)).rem_euclid(6) as u8) * 14;
                    obj.set_glow(GlowConfig { color: Color(255, 224, 70, pulse), width: 42.0 });
                } else if f.vent {
                    // Hot orange while the chest is open: the torso is
                    // dangerous here, but it is ALSO the only moment it can be
                    // hurt, so the cue has to say "come here" and "carefully"
                    // at once — which is why it is neither the storm's cold
                    // violet nor the plain strike red.
                    obj.set_glow(GlowConfig { color: Color(255, 170, 80, 220), width: 52.0 });
                } else if f.storm {
                    // Summoning glow: cold violet-white, deliberately NOT the
                    // red-orange of an incoming strike. The two torso attacks
                    // have to be distinguishable during the wind-up, because
                    // one is a beat to dodge and the other is the only beat
                    // where the torso can be hurt — reading it late costs the
                    // player the window entirely.
                    obj.set_glow(GlowConfig { color: Color(190, 150, 255, 200), width: 46.0 });
                } else if f.zone_visible {
                    // Wind-up glow: pulsing red-orange while it commits to the strike.
                    let wide = if f.strike_unhook || f.strike_heart || f.strike_kick.0 != 0.0 || f.strike_kick.1 != 0.0 { 190 } else { 110 };
                    obj.set_glow(GlowConfig { color: Color(255, 80, 30, wide), width: 30.0 });
                } else {
                    obj.clear_glow();
                }
            } else {
                obj.visible = false;
            }
        }

        // Attack-path telegraph: a translucent red strip from where the part
        // started winding up to where it will strike. For the head's gaze beam
        // this strip grows from the head toward a travelling front during the
        // attack, so the player sees the sweep coming and can be off the line.
        if f.id == "head" {
            // The head's path is a curve, so it gets the polyline pool rather
            // than the single rotated strip the lunging parts use. Shown for
            // the whole wind-up and while the beam sweeps, so the player can
            // read the full arc before it is dangerous.
            if f.alive && f.path_visible {
                // Only the stretch still AHEAD of the sweep. Behind the front
                // the bright core already covers the same ground, so drawing
                // the full ray underneath was a second full-length translucent
                // quad buying nothing.
                let ahead_from = f.beam_t.unwrap_or(0.0);
                let pts = beam_polyline_range(
                    f.path_start, f.zone_pos, f.beam_curve, ahead_from, 1.0,
                );
                if pts.len() >= 2 {
                    draw_beam_strip(c, "colossus_beam_tel", &pts, COLOSSUS_BEAM_THICKNESS);
                } else {
                    hide_beam_strip(c, "colossus_beam_tel");
                }
            } else {
                hide_beam_strip(c, "colossus_beam_tel");
            }
        }
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_path_{idx}")) {
            if f.alive && f.path_visible && f.id != "head" {
                let (ax, ay) = f.path_start;
                let (bx, by) = f.zone_pos;
                let dx = bx - ax;
                let dy = by - ay;
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let deg = dy.atan2(dx).to_degrees();
                let th = COLOSSUS_PATH_THICKNESS;
                let mid = ((ax + bx) * 0.5, (ay + by) * 0.5);
                // `rotation_adjusted_offset` keeps the rendered centre locked at
                // `position + size/2`, so positioning by the strip's centre is
                // enough — the engine handles the rotated AABB compensation.
                obj.size = (len, th);
                obj.rotation = deg;
                obj.position = (mid.0 - len * 0.5, mid.1 - th * 0.5);
                obj.visible = true;
            } else {
                obj.visible = false;
            }
        }

        // Traveling gaze beam hit: the beam front sweeps the path and, when it
        // reaches the player, costs a heart ALWAYS (the buff does not shield the
        // gaze) and throws them hard.
        if f.id == "head" && f.alive && f.zone_solid {
            let t = f.beam_t.unwrap_or(0.0);
            let pts = beam_polyline(f.path_start, f.zone_pos, f.beam_curve, t.max(0.001));
            let front = *pts.last().unwrap_or(&f.path_start);
            let hit_done = {
                let s = st.lock().unwrap();
                s.boss_parts.iter().find(|p| p.id == "head").map(|p| p.beam_hit_done).unwrap_or(true)
            };
            // Against the swept polyline, at HALF the drawn thickness plus the
            // player's radius — the damaging area is what is on screen. The old
            // test used the full path thickness as a radius, so the beam hurt
            // twice as far as it looked.
            if !hit_done && beam_dist((px, py), &pts) < beam_hit_radius() {
                {
                    let mut s = st.lock().unwrap();
                    if let Some(p) = s.boss_parts.iter_mut().find(|p| p.id == "head" && p.alive) {
                        p.beam_hit_done = true;
                    }
                }
                // The gaze always costs a heart.
                let dead = { let s = st.lock().unwrap(); s.dead };
                if !dead { crate::scenes::game::hearts::lose_heart(c, st); }
                // And throws the player away from the beam front.
                let dx = px - front.0;
                let dy = py - front.1;
                let d = (dx * dx + dy * dy).sqrt().max(1.0);
                let power = 78.0;
                let push = (dx / d * power, dy / d * power);
                let mut s = st.lock().unwrap();
                s.vx = push.0;
                s.vy = push.1;
                drop(s);
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.momentum = push;
                }
                c.set_var("boss_knockback_ticks", Value::I32(20));
            }
        }

        // Vulnerability ring: pulsing gold outline around a hittable part.
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_vuln_{idx}")) {
            if f.alive && !f.shielded && f.weak_open {
                let r = part_size * 0.55;
                let (sx, sy) = (bcx + f.offset.0, bcy + f.offset.1);
                obj.position = (sx - r, sy - r);
                // Pulse the ring's visibility/scale so it throbs.
                let on = ((f.zone_pos.0 as i32 / 3) + (f.zone_pos.1 as i32 / 3)).rem_euclid(5) < 3;
                obj.visible = on;
            } else {
                obj.visible = false;
            }
        }

        // Danger-zone telegraph disc (only while a part telegraphs / strikes).
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_zone_{idx}")) {
            if f.id == "head" {
                // Head: a small targeting reticle that runs the course of the
                // path line just ahead of the beam. During the telegraph the
                // line IS the telegraph, so the reticle only appears as it
                // fires (unlike the hands/torso landing circles).
                if f.alive && f.zone_solid {
                    let t = f.beam_t.unwrap_or(0.0);
                    let front = beam_point(f.path_start, f.zone_pos, f.beam_curve, t);
                    let r = COLOSSUS_BEAM_THICKNESS * 0.34;
                    obj.size = (r * 2.0, r * 2.0);
                    obj.position = (front.0 - r, front.1 - r);
                    obj.visible = true;
                } else {
                    obj.visible = false;
                }
            } else if f.alive && f.zone_visible {
                obj.position = (f.zone_pos.0 - f.zone_r, f.zone_pos.1 - f.zone_r);
                // Zone flickers during the telegraph, then goes solid for the strike.
                let on = f.zone_solid
                    || ((f.zone_pos.0 as i32 / 5) + (f.zone_pos.1 as i32 / 5)).rem_euclid(6) < 4;
                obj.visible = on;
            } else {
                obj.visible = false;
            }
        }

        // Strike effects (unhook / kick / heart) — resolved once per attack.
        if f.strike_unhook {
            let mut s = st.lock().unwrap();
            s.hooked = false;
            s.active_hook = String::new();
            drop(s);
            c.run(Action::Hide { target: Target::name("rope") });
        }
        if f.strike_kick.0 != 0.0 || f.strike_kick.1 != 0.0 {
            let mut s = st.lock().unwrap();
            s.vx = f.strike_kick.0;
            s.vy = f.strike_kick.1;
            drop(s);
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum = f.strike_kick;
            }
            // A direct hit throws the player hard: briefly bypass the momentum
            // cap so the throw actually flies rather than being clamped away.
            if f.strike_big_throw {
                c.set_var("boss_knockback_ticks", Value::I32(20));
            }
        }
        if f.strike_heart {
            let dead = { let s = st.lock().unwrap(); s.dead };
            if !dead { crate::scenes::game::hearts::lose_heart(c, st); }
        }
        if f.clap_wave {
            // The wave leaves the impact point whether or not the hands caught
            // anyone. Being outside the kill zone is not the same as being
            // unaffected — the throw IS the attack, and it is what turns a
            // dodged clap into a repositioning problem instead of a non-event.
            let (cx, cy) = f.zone_pos;
            let dx = px - cx;
            let dy = py - cy;
            let d = (dx * dx + dy * dy).sqrt().max(1.0);
            {
                let mut s = st.lock().unwrap();
                s.boss_clap_wave = COLOSSUS_CLAP_WAVE_TICKS;
                s.boss_clap_at = (cx, cy);
            }
            if d < COLOSSUS_CLAP_WAVE_R {
                // Falls off to nothing at the edge, so the wave has a readable
                // reach rather than a hard boundary you cannot see.
                let fall = 1.0 - (d / COLOSSUS_CLAP_WAVE_R).clamp(0.0, 1.0);
                let power = COLOSSUS_CLAP_WAVE_POWER * fall;
                let push = (dx / d * power, dy / d * power);
                {
                    let mut s = st.lock().unwrap();
                    s.vx = push.0;
                    s.vy = push.1;
                    s.hooked = false;
                    s.active_hook = String::new();
                }
                c.run(Action::Hide { target: Target::name("rope") });
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.momentum = push;
                }
                // Briefly bypass the momentum cap so the throw actually flies
                // rather than being clamped away on the next frame.
                c.set_var("boss_knockback_ticks", Value::I32(22));
            }
        }
        if f.strike_consume_absorb {
            // The buff ate the hit: spend one absorption. When it runs out the
            // buff ends, so the shield is a limited resource.
            let mut s = st.lock().unwrap();
            if s.player_buff > 0 {
                s.buff_absorbs = s.buff_absorbs.saturating_sub(1);
                if s.buff_absorbs == 0 {
                    s.player_buff = 0;
                    s.buff_timer = 0;
                }
            }
        }
    }

    // Hide the visuals of any destroyed/leashed parts.
    for idx in 0..frames.len() {
        if let Some(obj) = c.get_game_object_mut(&format!("colossus_part_{idx}")) {
            if !frames[idx].alive {
                obj.visible = false;
            }
        }
    }

    // Phase gating: once a dependency is dead, the next part loses its shield.
    // (Colossus: torso unshields when both hands die; head unshields when the
    // torso dies. Serpent segments uncover as the one before dies.)
    {
        let mut s = st.lock().unwrap();
        for i in 0..s.boss_parts.len() {
            let prev_dead = i > 0 && s.boss_parts[..i].iter().all(|p| !p.alive);
            if prev_dead {
                let part = &mut s.boss_parts[i];
                if part.alive && part.shielded {
                    part.shielded = false;
                }
            }
        }
        s.boss_hp = boss_total_hp(&s);
    }

    // Simple shield dome glow while any part is still shielded (the full
    // BIT_ENERGY_DOME shader is layered on later).
    let any_shielded = { let s = st.lock().unwrap(); s.boss_parts.iter().any(|p| p.alive && p.shielded) };
    if let Some(obj) = c.get_game_object_mut("boss") {
        if any_shielded {
            obj.set_glow(GlowConfig { color: Color(120, 220, 255, 90), width: 22.0 });
        } else {
            obj.clear_glow();
        }
    }

    // Contact-rule inversion: touching a part you are NOT currently able to hit
    // (it's shielded or idle/winding up) costs one heart — but not your whole
    // life (the cooldown stops repeated contact from draining every heart in a
    // couple of frames). With the buff it costs no heart, just tears you off.
    // This is a light contact push; the attack STRIKE is the one that throws you
    // hard.
    {
        let mut s = st.lock().unwrap();
        if s.boss_contact_cooldown > 0 { s.boss_contact_cooldown -= 1; }
        let touching = if s.boss_contact_cooldown == 0 && !s.dead {
            frames.iter().enumerate().any(|(i, f)| {
                f.alive && !f.weak_open && {
                    let sx = bcx + f.offset.0;
                    let sy = bcy + f.offset.1;
                    let cr = colossus_part_hit_r(i as u32) + PLAYER_R;
                    (px - sx).powi(2) + (py - sy).powi(2) < cr * cr
                }
            })
        } else { false };
        if touching { s.boss_contact_cooldown = 45; }
        let dead = s.dead;
        drop(s);
        if touching && !dead {
            let d = ((px - bcx).powi(2) + (py - bcy).powi(2)).sqrt().max(1.0);
            let push = ((px - bcx) / d * 34.0, (py - bcy) / d * 34.0);
            let mut s = st.lock().unwrap();
            s.vx = push.0;
            s.vy = push.1;
            if !buffed {
                // No buff: contact costs a heart (see the cooldown — one, not all).
                drop(s);
                crate::scenes::game::hearts::lose_heart(c, st);
            } else {
                // Buff shields the heart, but contact still tears you off.
                s.hooked = false;
                s.active_hook = String::new();
                drop(s);
                c.run(Action::Hide { target: Target::name("rope") });
            }
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum = push;
            }
        }
    }

    // Win when no parts are alive.
    if !any_alive {
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
        }
        finish_boss(c, st);
    }
}
