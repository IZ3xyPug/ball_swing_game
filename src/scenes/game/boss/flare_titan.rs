use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
#[allow(unused_imports)]
use super::*;

// ── The Flare Titan ─────────────────────────────────────────────────────────
//
// A caged star and four furnace vents orbiting it. Sixth fight of the run,
// and the one the solar flare has been building toward since minute 44: here
// the flare is the boss's own clock.
//
//   CALM    the vents throw PROMINENCES: plasma along a curved path, shown
//           first and cleared just before it flies. Where it lies the arc
//           burns for a moment, and the vent that threw it stays open while
//           it cools.
//   KINDLE  the flare's count-in. The shelter domes brighten, a solar wind
//           blows out from the core, and the player has three seconds to be
//           tethered to a shielded node.
//   FLARE   unsheltered burns, a heart at a time; sheltered CHARGES — Solar
//           Charge is the buff every hit needs. Survive it well and you come
//           out of it armed.
//   VENT    the star cools: every live part is open and holds still.
//
// Kill the vents (two hits each: crack, then kill) and the core is exposed:
// it throws its own prominences and flares sooner. With half the vents gone
// the Titan HUNTS during the flare — arcs land on the shelter you are using,
// so sheltering becomes a route between nodes rather than a place to wait.
// 4 x 2 + 12 = 20 hits.
//
// The flare's screen wash, its banner and the shelter domes belong to the
// solar system (`solar.rs`). Inside this arena it stops running a flare of
// its own and draws from the Titan's clock instead (`titan_owns_flares`),
// so there is one flare language in the game, not two.

/// Index of the core in `boss_parts`; the vents are 0..TITAN_VENTS.
pub(crate) const TITAN_CORE_IDX: usize = TITAN_VENTS;

pub(crate) const CLOCK_CALM: u8 = 0;
pub(crate) const CLOCK_KINDLE: u8 = 1;
pub(crate) const CLOCK_FLARE: u8 = 2;
pub(crate) const CLOCK_VENT: u8 = 3;

fn titan_part_size(i: usize) -> f32 {
    if i == TITAN_CORE_IDX { TITAN_CORE_SIZE } else { TITAN_VENT_SIZE }
}

fn titan_part_hit_r(i: usize) -> f32 {
    if i == TITAN_CORE_IDX { TITAN_CORE_SIZE * 0.40 } else { TITAN_VENT_SIZE * 0.38 }
}

/// Ticks a part rests between prominences, staggered per vent.
fn titan_idle_len(i: usize) -> u32 {
    if i == TITAN_CORE_IDX {
        return TITAN_CORE_IDLE_TICKS;
    }
    TITAN_VENT_IDLE_TICKS + (i as u32 * 37) % 71
}

/// Where vent `i` sits relative to the core.
pub(crate) fn titan_vent_offset(i: usize, orbit: f32) -> (f32, f32) {
    let a = orbit + i as f32 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4;
    (a.cos() * TITAN_ORBIT_RX, a.sin() * TITAN_ORBIT_RY)
}

/// The arc a part's prominence follows, from what the wind-up locked in:
/// `attack_start` (the mouth), `target` (the landing) and `beam_curve` (the
/// rise, negative to bulge the other way).
fn titan_arc(p: &BossPart) -> ArcPath {
    ArcPath {
        start: p.attack_start,
        end: p.target,
        rise: p.beam_curve.abs(),
        flip: p.beam_curve < 0.0,
    }
}

/// The rise and bulge side for a throw from `from` to `to` around a core at
/// `core`: a prominence loops OUT, away from the star. Pure, for the test.
pub(crate) fn titan_arc_rise(from: (f32, f32), to: (f32, f32), core: (f32, f32)) -> f32 {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let chord = (dx * dx + dy * dy).sqrt().max(1.0);
    let rise = (chord * TITAN_ARC_RISE).clamp(TITAN_ARC_RISE_MIN, TITAN_ARC_RISE_MAX);
    // Left of the throw in a y-down world (what `ArcPath` bulges toward
    // unflipped), against the way out from the core at the chord's middle.
    let left = (dy / chord, -dx / chord);
    let mid = ((from.0 + to.0) * 0.5, (from.1 + to.1) * 0.5);
    let out = (mid.0 - core.0, mid.1 - core.1);
    if left.0 * out.0 + left.1 * out.1 >= 0.0 { rise } else { -rise }
}

/// Advance the clock one phase. Pure: (next phase, its length, a flare
/// began, the vent window began).
pub(crate) fn titan_next_phase(clock: u8, core_phase: bool) -> (u8, u32) {
    match clock {
        CLOCK_CALM => (CLOCK_KINDLE, TITAN_KINDLE_TICKS),
        CLOCK_KINDLE => (CLOCK_FLARE, TITAN_FLARE_TICKS),
        CLOCK_FLARE => (CLOCK_VENT, TITAN_VENT_TICKS),
        _ => (CLOCK_CALM, if core_phase { TITAN_CALM_TICKS_CORE } else { TITAN_CALM_TICKS }),
    }
}

/// While the Titan is being fought, the flare is its clock: the solar system
/// neither runs nor resets one of its own, it draws the Titan's.
pub(crate) fn titan_owns_flares(s: &State) -> bool {
    s.boss_active && s.boss_kind == BossKind::FlareTitan
}

/// Put the flare the Titan was driving out: the wash, the banner, the
/// dome brightness all read these. Every exit from the fight comes here.
pub(crate) fn titan_end_flare(s: &mut State) {
    s.flare_warn = 0;
    s.flare_active = false;
    s.flare_active_ticks = 0;
    s.flare_damage_timer = 0;
}

/// While the flare counts in or burns and the player is NOT sheltered, the
/// nearest shelter wears the house "this one" reticle, so finding cover is
/// a glance instead of a search. Moves to whichever shelter is nearest, and
/// comes off the moment the player is sheltered or the flare is over.
fn titan_guide_to_shelter(c: &mut Canvas, st: &Arc<Mutex<State>>, show: bool) {
    let (prev, want) = {
        let s = st.lock().unwrap();
        let want = if show {
            s.live_hooks
                .iter()
                .filter_map(|id| {
                    let o = c.get_game_object(id)?;
                    if !o.visible || !o.tags.iter().any(|t| t == SHIELD_HOOK_TAG) {
                        return None;
                    }
                    let (hx, hy) = (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5);
                    Some((id.clone(), (hx - s.px).powi(2) + (hy - s.py).powi(2)))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, _)| id)
        } else {
            None
        };
        (s.titan_guide_node.clone(), want)
    };
    let want_id = want.clone().unwrap_or_default();
    if prev != want_id {
        if !prev.is_empty() {
            c.clear_overlay_effect(&prev);
        }
        st.lock().unwrap().titan_guide_node = want_id;
    }
    if let Some(id) = want {
        let d = HOOK_R * 2.0 * 3.4;
        c.attach_overlay_effect(
            &id,
            Effect::StateMarker { mode: MarkerMode::Vulnerable, intensity: 1.0 },
            EffectColor::linear(1.0, 0.86, 0.35),
            (d, d),
        );
    }
}

/// Take the shelter reticle off, wherever it is. Every exit from the fight.
pub(crate) fn titan_clear_guide(c: &mut Canvas, s: &mut State) {
    let prev = std::mem::take(&mut s.titan_guide_node);
    if !prev.is_empty() {
        c.clear_overlay_effect(&prev);
    }
}

/// Hide every Titan piece. Called on victory (the fight stops being ticked
/// the moment it dies) and on every fight start.
pub(crate) fn hide_titan(c: &mut Canvas) {
    for i in 0..=TITAN_CORE_IDX {
        for name in [format!("titan_part_{i}"), format!("titan_arc_{i}"), format!("titan_zone_{i}")] {
            c.clear_effect(&name);
            c.clear_overlay_effect(&name);
            if let Some(o) = c.get_game_object_mut(&name) {
                o.visible = false;
                o.position = (-9000.0, -9000.0);
                o.rotation = 0.0;
            }
        }
    }
    c.clear_effect("titan_corona");
    if let Some(o) = c.get_game_object_mut("titan_corona") {
        o.visible = false;
        o.position = (-9000.0, -9000.0);
    }
    c.set_var("flare_warning", false);
    c.set_var("flare_active", false);
}

pub(crate) fn tick_flare_titan(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let s = st.lock().unwrap();
        if !s.boss_active || s.dead || s.boss_stasis_active { return; }
    }
    // TITAN_CHECK: park a buffed player on the first open part, so every
    // hit, art swap, gating and clock path runs in a real fight loop.
    // TITAN_REACH_CHECK: fly a buffed stand-in at a player's speed to the
    // nearest open part and log how far into each window its first hit
    // lands — or that the window closed first.
    let check = std::env::var("TITAN_CHECK").is_ok();
    let reach = std::env::var("TITAN_REACH_CHECK").is_ok();

    // ── Appearance (once) ──────────────────────────────────────────────────
    let spawned = { st.lock().unwrap().boss_spawned };
    if !spawned {
        let name = {
            let mut s = st.lock().unwrap();
            s.boss_spawned = true;
            // A fresh clock every time the fight (re)starts: after a fall the
            // star must not flare the frame the player lands.
            s.titan_clock = CLOCK_CALM;
            s.titan_clock_ticks = TITAN_CALM_TICKS;
            s.titan_orbit = 0.0;
            s.titan_charged = false;
            s.titan_burn_timer = 0;
            titan_end_flare(&mut s);
            s.boss_pattern_cooldown = TITAN_PATTERN_COOLDOWN;
            s.boss_entry_ticks = 0;
            for p in s.boss_parts.iter_mut() {
                p.state = PartState::Idle;
                p.state_ticks = 0;
                p.post_attack = false;
                p.beam_hit_done = false;
            }
            s.boss_kind.name()
        };
        hide_titan(c);
        ensure_arena_shelter_nodes(c, st, true);
        if check {
            // Coverage: how many shelters, and the worst distance from any
            // node to its nearest one.
            let hooks = st.lock().unwrap().live_hooks.clone();
            let pts: Vec<((f32, f32), bool)> = hooks
                .iter()
                .filter_map(|id| c.get_game_object(id).map(|o| (
                    (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5),
                    o.tags.iter().any(|t| t == SHIELD_HOOK_TAG),
                )))
                .collect();
            let shelters: Vec<(f32, f32)> = pts.iter().filter(|p| p.1).map(|p| p.0).collect();
            let worst = pts
                .iter()
                .map(|&((x, y), _)| {
                    shelters.iter().map(|&(sx, sy)| ((sx - x).powi(2) + (sy - y).powi(2)).sqrt())
                        .fold(f32::MAX, f32::min)
                })
                .fold(0.0f32, f32::max);
            eprintln!("titan-check: {} of {} nodes are shelter; farthest node is {:.0} px from one",
                      shelters.len(), pts.len(), worst);
        }
        let cx = arena_center_x(c);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (cx - BOSS_SIZE * 0.5, BOSS_Y_CENTER - BOSS_SIZE * 0.5);
            obj.visible = false;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc, Color(TITAN_SRGB.0, TITAN_SRGB.1, TITAN_SRGB.2, 255), 1000.0 * sc,
                )));
            }
        }
    }

    // ── Clocks ─────────────────────────────────────────────────────────────
    let (px, py, pvx, pvy, buffed) = {
        let mut s = st.lock().unwrap();
        s.boss_entry_ticks = s.boss_entry_ticks.saturating_add(1);
        if s.boss_pattern_cooldown > 0 { s.boss_pattern_cooldown -= 1; }
        if s.boss_part_invuln_ticks > 0 { s.boss_part_invuln_ticks -= 1; }
        if s.boss_contact_cooldown > 0 { s.boss_contact_cooldown -= 1; }
        // The star holds still while it cools: an open part that drifts off
        // is a window that cannot be reached.
        let frozen = s.titan_clock == CLOCK_VENT;
        if !frozen {
            s.titan_orbit += TITAN_ORBIT_RATE;
            s.boss_phase += 0.005;
        }
        if check || reach {
            // The checks park the player inside the fight; they prove the
            // boss's paths, not the player's survival.
            s.hearts = s.hearts.max(3);
        }
        (s.px, s.py, s.vx, s.vy, s.buff_active())
    };
    if check || reach {
        c.set_var("debug_force_buff", true);
    }

    // The core drifts slowly; the vents ride its orbit.
    let (bcx, bcy) = {
        let phase = st.lock().unwrap().boss_phase;
        let cx = arena_center_x(c) + (phase * 0.7).sin() * 1400.0;
        let cy = BOSS_Y_CENTER + (phase * 0.5 + 0.5).sin() * 600.0;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (cx - BOSS_SIZE * 0.5, cy - BOSS_SIZE * 0.5);
        }
        (cx, cy)
    };
    let (zx1, zx2) = arena_bounds(c);

    // ── Phase gating ───────────────────────────────────────────────────────
    // Every vent is live from the start; the core is shielded until they are
    // all dead. Half of them dead and the Titan starts HUNTING shelter.
    let (core_phase, escalated) = {
        let mut s = st.lock().unwrap();
        let live = (0..TITAN_VENTS).filter(|&i| s.boss_parts[i].alive).count();
        let core = &mut s.boss_parts[TITAN_CORE_IDX];
        if core.alive && core.shielded && live == 0 {
            core.shielded = false;
            if check { eprintln!("titan-check: core exposed"); }
        }
        (live == 0, live <= TITAN_VENTS / 2)
    };

    // ── The clock ──────────────────────────────────────────────────────────
    let (clock, clock_left) = {
        let mut s = st.lock().unwrap();
        if s.titan_clock_ticks > 0 {
            s.titan_clock_ticks -= 1;
        }
        if s.titan_clock_ticks == 0 {
            let (next, len) = titan_next_phase(s.titan_clock, core_phase);
            s.titan_clock = next;
            s.titan_clock_ticks = len;
            if next == CLOCK_KINDLE {
                s.titan_reshelter = true;
            }
            if next == CLOCK_FLARE {
                s.titan_flares += 1;
                s.titan_charged = false;
                s.titan_burn_timer = TITAN_BURN_GRACE;
                // SUNPROOFING works here as it does against the run's own
                // flares: its wards refill each flare and are spent before
                // hearts. A bought upgrade silently not applying in the one
                // fight built on flares would be a lie.
                s.flare_wards_left = s.perm_flare_wards;
            }
            if check {
                let label = ["calm", "kindle", "flare", "vent"][next as usize];
                eprintln!("titan-check: {label} (flare {})", s.titan_flares);
            }
        }
        // Mirror into the solar system's fields: its wash, banner and domes
        // read them.
        match s.titan_clock {
            CLOCK_KINDLE => {
                s.flare_warn = s.titan_clock_ticks.max(1);
                s.flare_active = false;
                s.flare_active_ticks = 0;
            }
            CLOCK_FLARE => {
                s.flare_warn = 0;
                s.flare_active = true;
                s.flare_active_ticks = s.titan_clock_ticks.max(1);
            }
            _ => titan_end_flare(&mut s),
        }
        (s.titan_clock, s.titan_clock_ticks)
    };
    c.set_var("flare_warning", clock == CLOCK_KINDLE);
    c.set_var("flare_active", clock == CLOCK_FLARE);
    // Top the cover up as the arena fills in, and before every flare, so
    // nodes that arrived after the fight began are covered too.
    let top_up = {
        let mut s = st.lock().unwrap();
        std::mem::take(&mut s.titan_reshelter) || s.ticks % 20 == 0
    };
    if top_up {
        ensure_arena_shelter_nodes(c, st, false);
    }

    // ── The flare: it burns the exposed and charges the sheltered ─────────
    let sheltered = {
        let s = st.lock().unwrap();
        crate::scenes::game::solar::player_is_sheltered(c, &s)
    };
    titan_guide_to_shelter(c, st, matches!(clock, CLOCK_KINDLE | CLOCK_FLARE) && !sheltered);
    if clock == CLOCK_FLARE {
        let burn = {
            let mut s = st.lock().unwrap();
            if sheltered {
                if !s.titan_charged && check {
                    eprintln!("titan-check: charged by flare {}", s.titan_flares);
                }
                s.player_buff = 1;
                s.buff_timer = s.buff_timer.max(TITAN_CHARGE_TICKS);
                s.buff_absorbs = s.buff_absorbs.max(BUFF_ABSORB_MAX);
                s.titan_charged = true;
            }
            if s.titan_burn_timer > 0 {
                s.titan_burn_timer -= 1;
            }
            if s.titan_burn_timer == 0 {
                s.titan_burn_timer = TITAN_BURN_INTERVAL;
                !sheltered && !s.dead
            } else {
                false
            }
        };
        if burn {
            if let Some(cam) = c.camera_mut() {
                cam.flash_with(Color(255, 190, 90, 170), 0.3, FlashMode::Pulse, FlashEase::Sharp, 0.8, 0.0);
            }
            let warded = {
                let mut s = st.lock().unwrap();
                if s.flare_wards_left > 0 {
                    s.flare_wards_left -= 1;
                    true
                } else {
                    false
                }
            };
            if warded {
                let n = { st.lock().unwrap().flare_wards_left as i32 };
                c.set_var("flare_wards_left", Value::I32(n));
            } else {
                crate::scenes::game::hearts::lose_heart(c, st);
            }
        }
    }

    // ── The solar wind: through the kindle, out from the core ─────────────
    if clock == CLOCK_KINDLE {
        let k = 1.0 - clock_left as f32 / TITAN_KINDLE_TICKS as f32;
        let (dx, dy) = (px - bcx, py - bcy);
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let mut s = st.lock().unwrap();
        s.vx += dx / d * TITAN_WIND * k;
        s.vy += dy / d * TITAN_WIND * k;
    }

    // ── Per-part FSM ───────────────────────────────────────────────────────
    #[derive(Clone, Copy)]
    struct Frame {
        alive: bool,
        shielded: bool,
        weak_open: bool,
        pos: (f32, f32),
        /// Which way the part faces, world degrees (the vent's mouth).
        aim_deg: f32,
        arc: Option<(ArcPath, ArcStage, f32)>,
        /// The landing's countdown while the path shows.
        zone: Option<((f32, f32), f32)>,
        windup: bool,
        damaged: bool,
        /// Thrown this way off a strike.
        strike: Option<(f32, f32)>,
        landed: Option<(f32, f32)>,
        closing: Option<f32>,
    }
    let frames: Vec<Frame> = {
        let mut s = st.lock().unwrap();
        let orbit = s.titan_orbit;
        let cooldown_ok = s.boss_pattern_cooldown == 0;
        let calm = clock == CLOCK_CALM;
        let hunting = clock == CLOCK_FLARE && escalated;
        let vent_open = clock == CLOCK_VENT;
        let vent_closing = 1.0 - clock_left as f32 / TITAN_VENT_TICKS as f32;
        let tel_shown = TITAN_ARC_TELEGRAPH_TICKS - TITAN_TELEGRAPH_CLEAR;
        let mut began = false;
        let mut frames = Vec::with_capacity(s.boss_parts.len());
        for i in 0..s.boss_parts.len() {
            let ring = if i == TITAN_CORE_IDX {
                (bcx, bcy)
            } else {
                let o = titan_vent_offset(i, orbit);
                (bcx + o.0, bcy + o.1)
            };
            let pos = {
                let q = &s.boss_parts[i];
                let holding = matches!(q.state, PartState::Telegraph | PartState::Attack | PartState::Recover);
                if i == TITAN_CORE_IDX {
                    (bcx, bcy)
                } else if q.alive && holding {
                    q.path_start
                } else if q.alive && q.state == PartState::Idle && q.post_attack
                    && q.state_ticks < TITAN_REJOIN_TICKS
                {
                    let t = q.state_ticks as f32 / TITAN_REJOIN_TICKS as f32;
                    lerp2(q.path_start, ring, t * t * (3.0 - 2.0 * t))
                } else {
                    ring
                }
            };
            let out_deg = if i == TITAN_CORE_IDX {
                0.0
            } else {
                (pos.1 - bcy).atan2(pos.0 - bcx).to_degrees()
            };
            let p = &mut s.boss_parts[i];
            let damaged = p.hp * 2 <= p.max_hp;
            if !p.alive {
                frames.push(Frame { alive: false, shielded: false, weak_open: false, pos, aim_deg: out_deg,
                    arc: None, zone: None, windup: false, damaged, strike: None, landed: None, closing: None });
                continue;
            }
            if p.shielded {
                p.state = PartState::Idle;
                p.state_ticks = 0;
                p.post_attack = false;
                frames.push(Frame { alive: true, shielded: true, weak_open: false, pos, aim_deg: out_deg,
                    arc: None, zone: None, windup: false, damaged, strike: None, landed: None, closing: None });
                continue;
            }
            let mut strike = None;
            let mut landed = None;
            match p.state {
                PartState::Idle => {
                    p.state_ticks += 1;
                    // Calm: on each part's own rest. Hunting: through the
                    // flare, at the shelter the player is holding.
                    let may = cooldown_ok && !began
                        && ((calm && p.state_ticks >= titan_idle_len(i))
                            || (hunting && p.state_ticks >= 30));
                    if may {
                        let lead = if hunting { 0.0 } else { TITAN_AIM_LEAD };
                        let aim = (
                            (px + pvx * lead).clamp(zx1 + 400.0, zx2 - 400.0),
                            (py + pvy * lead).clamp(BOSS_ARENA_NODE_Y_BOT - 500.0, BOSS_ARENA_NODE_Y_TOP + 300.0),
                        );
                        let (dx, dy) = (aim.0 - pos.0, aim.1 - pos.1);
                        let d = (dx * dx + dy * dy).sqrt().max(1.0);
                        let r = titan_part_hit_r(i);
                        let mouth = (pos.0 + dx / d * r, pos.1 + dy / d * r);
                        p.attack_start = mouth;
                        p.target = aim;
                        p.beam_curve = titan_arc_rise(mouth, aim, (bcx, bcy));
                        // It throws from HERE, and holds here until it has
                        // cooled: the path is a promise.
                        p.path_start = pos;
                        p.state = PartState::Telegraph;
                        p.state_ticks = 0;
                        p.post_attack = false;
                        p.beam_hit_done = false;
                        began = true;
                        if check && hunting { eprintln!("titan-check: part {i} hunts the shelter"); }
                    }
                }
                PartState::Telegraph => {
                    p.state_ticks += 1;
                    if p.state_ticks >= TITAN_ARC_TELEGRAPH_TICKS {
                        p.state = PartState::Attack;
                        p.state_ticks = 0;
                    }
                }
                PartState::Attack => {
                    p.state_ticks += 1;
                    let arc = titan_arc(p);
                    let t = (p.state_ticks as f32 / TITAN_ARC_FLIGHT_TICKS as f32).min(1.0);
                    let head = arc.point(t);
                    let away = |from: (f32, f32)| {
                        let (dx, dy) = (px - from.0, py - from.1);
                        let d = (dx * dx + dy * dy).sqrt();
                        if d > 0.001 { (dx / d, dy / d) } else { (0.0, -1.0) }
                    };
                    let hd = ((px - head.0).powi(2) + (py - head.1).powi(2)).sqrt();
                    if !p.beam_hit_done && hd < TITAN_ARC_HEAD_R + PLAYER_R {
                        p.beam_hit_done = true;
                        strike = Some(away(head));
                    }
                    if p.state_ticks >= TITAN_ARC_FLIGHT_TICKS {
                        let ld = ((px - p.target.0).powi(2) + (py - p.target.1).powi(2)).sqrt();
                        if !p.beam_hit_done && ld < TITAN_ARC_SPLASH_R + PLAYER_R {
                            p.beam_hit_done = true;
                            strike = Some(away(p.target));
                        }
                        landed = Some(p.target);
                        p.state = PartState::Recover;
                        p.state_ticks = 0;
                        p.post_attack = true;
                    }
                }
                PartState::Recover => {
                    p.state_ticks += 1;
                    if p.state_ticks < TITAN_ARC_BURN_TICKS && !p.beam_hit_done {
                        // The burn thins as it dies, and so does what it hurts.
                        let life = 1.0 - p.state_ticks as f32 / TITAN_ARC_BURN_TICKS as f32;
                        let arc = titan_arc(p);
                        let (d, along) = arc.distance((px, py));
                        if d < TITAN_ARC_HALF_W * (0.35 + 0.65 * life) + PLAYER_R {
                            p.beam_hit_done = true;
                            let near = arc.point(along);
                            let (dx, dy) = (px - near.0, py - near.1);
                            let n = (dx * dx + dy * dy).sqrt();
                            strike = Some(if n > 0.001 { (dx / n, dy / n) } else { (0.0, -1.0) });
                        }
                    }
                    if p.state_ticks >= TITAN_RECOVER_TICKS {
                        p.state = PartState::Idle;
                        p.state_ticks = 0;
                    }
                }
            }
            // Open while it cools after a throw, and — every live part at
            // once — through the vent window.
            let weak_open = p.state == PartState::Recover || (vent_open && p.state == PartState::Idle);
            // Window bookkeeping for the reach check: `weakpoint_window`
            // counts ticks open, `attack_timer` hits landed in this window.
            if p.weakpoint_open && !weak_open {
                if reach && p.attack_timer == 0 {
                    eprintln!("reach: part {i} window CLOSED unhit after {} ticks", p.weakpoint_window);
                }
                p.weakpoint_window = 0;
                p.attack_timer = 0;
            }
            if weak_open {
                p.weakpoint_window += 1;
            }
            p.weakpoint_open = weak_open;
            // Whichever window ends LATER is the one the marker counts down.
            let closing = if !weak_open {
                None
            } else {
                let rec_left = if p.state == PartState::Recover {
                    TITAN_RECOVER_TICKS.saturating_sub(p.state_ticks)
                } else {
                    0
                };
                let vent_left = if vent_open { clock_left } else { 0 };
                if rec_left >= vent_left {
                    Some(p.state_ticks as f32 / TITAN_RECOVER_TICKS as f32)
                } else {
                    Some(vent_closing)
                }
            };
            let path_up = p.state == PartState::Telegraph && p.state_ticks < tel_shown;
            let arc = match p.state {
                _ if path_up => Some((titan_arc(p), ArcStage::Path, p.state_ticks as f32 / tel_shown as f32)),
                PartState::Attack => Some((
                    titan_arc(p), ArcStage::Flight,
                    (p.state_ticks as f32 / TITAN_ARC_FLIGHT_TICKS as f32).min(1.0),
                )),
                PartState::Recover if p.state_ticks < TITAN_ARC_BURN_TICKS => Some((
                    titan_arc(p), ArcStage::Burn,
                    1.0 - p.state_ticks as f32 / TITAN_ARC_BURN_TICKS as f32,
                )),
                _ => None,
            };
            let zone = path_up.then(|| (p.target, p.state_ticks as f32 / tel_shown as f32));
            // Facing: the mouth follows the throw from wind-up to landing,
            // and faces out from the star the rest of the time.
            let aim_deg = if matches!(p.state, PartState::Telegraph | PartState::Attack) {
                let arc = titan_arc(p);
                let a = arc.point(0.0);
                let b = arc.point(0.06);
                (b.1 - a.1).atan2(b.0 - a.0).to_degrees()
            } else {
                out_deg
            };
            frames.push(Frame {
                alive: true, shielded: false, weak_open, pos, aim_deg, arc, zone,
                windup: path_up, damaged, strike, landed, closing,
            });
        }
        if began {
            s.boss_pattern_cooldown = if hunting { TITAN_HUNT_COOLDOWN } else { TITAN_PATTERN_COOLDOWN };
        }
        frames
    };

    if reach {
        let target = frames
            .iter()
            .filter(|f| f.alive && f.weak_open)
            .map(|f| f.pos)
            .min_by(|a, b| {
                let da = (a.0 - px).powi(2) + (a.1 - py).powi(2);
                let db = (b.0 - px).powi(2) + (b.1 - py).powi(2);
                da.total_cmp(&db)
            })
            .unwrap_or((bcx, bcy + 1800.0));
        let (dx, dy) = (target.0 - px, target.1 - py);
        let d = (dx * dx + dy * dy).sqrt();
        if d > 1.0 {
            let step = d.min(WEAVER_REACH_SPEED);
            let np = (px + dx / d * step, py + dy / d * step);
            {
                let mut s = st.lock().unwrap();
                s.px = np.0;
                s.py = np.1;
                s.vx = 0.0;
                s.vy = 0.0;
                s.hooked = false;
            }
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.position = (np.0 - PLAYER_R, np.1 - PLAYER_R);
                obj.momentum = (0.0, 0.0);
            }
        }
    }

    // ── The corona: the solar wind blowing out through the kindle ─────────
    {
        let d = TITAN_CORE_SIZE * TITAN_CORONA_SCALE;
        let strength = match clock {
            CLOCK_KINDLE => Some(0.3 + 0.7 * (1.0 - clock_left as f32 / TITAN_KINDLE_TICKS as f32)),
            CLOCK_FLARE => Some(1.0),
            _ => None,
        };
        match strength {
            Some(k) => {
                if let Some(o) = c.get_game_object_mut("titan_corona") {
                    o.size = (d, d);
                    o.position = (bcx - d * 0.5, bcy - d * 0.5);
                    o.visible = true;
                }
                let (r, g, b) = TITAN_SRGB;
                c.attach_effect("titan_corona", Effect::SonicRing { intensity: k },
                                EffectColor::srgb8(r, g, b), (d, d));
            }
            None => {
                c.clear_effect("titan_corona");
                if let Some(o) = c.get_game_object_mut("titan_corona") { o.visible = false; }
            }
        }
    }

    // ── Visuals, arcs, hits ────────────────────────────────────────────────
    let mut any_alive = false;
    let kindle_beats = if clock == CLOCK_KINDLE {
        let k = 1.0 - clock_left as f32 / TITAN_KINDLE_TICKS as f32;
        Some(0.45 + 0.4 * (k * 3.0 * std::f32::consts::PI).sin().abs())
    } else {
        None
    };
    for (i, f) in frames.iter().enumerate() {
        let part = format!("titan_part_{i}");
        let arc_id = format!("titan_arc_{i}");
        let zone_id = format!("titan_zone_{i}");
        let size = titan_part_size(i);
        if !f.alive {
            c.clear_effect(&part);
            crate::scenes::game::fx::clear_windup(c, &part);
            for id in [&part, &arc_id, &zone_id] {
                c.clear_effect(id);
                if let Some(o) = c.get_game_object_mut(id) { o.visible = false; }
            }
            continue;
        }
        any_alive = true;

        if let Some(o) = c.get_game_object_mut(&part) {
            o.size = (size, size);
            o.position = (f.pos.0 - size * 0.5, f.pos.1 - size * 0.5);
            o.visible = true;
            if i != TITAN_CORE_IDX {
                let want = f.aim_deg - TITAN_VENT_ART_DEG;
                let mut delta = (want - o.rotation).rem_euclid(360.0);
                if delta > 180.0 { delta -= 360.0; }
                o.rotation += delta * 0.2;
            }
            // Cracked at half HP: a state the art carries itself.
            if f.damaged && o.animated_sprite.is_some() {
                let art = if i == TITAN_CORE_IDX { ASSET_PL_TITAN_CORE_DAMAGED } else { ASSET_PL_TITAN_VENT_DAMAGED };
                if let Some(img) = crate::scenes::game::helpers::pl_image_cached(art, size) {
                    o.animated_sprite = None;
                    o.set_image(Image { shape: ShapeType::Rectangle(0.0, (size, size), 0.0), image: img, color: None });
                    if check { eprintln!("titan-check: part {i} shows its damaged art"); }
                }
            }
        }

        // State, ALWAYS shown: the phase shield, the open burn, or a lighter
        // shell when merely closed. The telegraph rides OVER it.
        {
            let (rgb, k, mode) = if f.shielded {
                (TITAN_MARKER_SHIELDED_RGB, 0.85, MarkerMode::Shielded)
            } else if f.weak_open {
                (TITAN_MARKER_VULNERABLE_RGB, 1.0, MarkerMode::Vulnerable)
            } else {
                (TITAN_MARKER_SHIELDED_RGB, TITAN_CLOSED_SHELL, MarkerMode::Shielded)
            };
            crate::scenes::game::fx::attach_state_marker_timed(
                c, &part, (size, size), rgb, k, mode,
                if mode == MarkerMode::Vulnerable { f.closing } else { None });
            let windup = if f.windup { Some(0.95) } else { kindle_beats };
            match windup {
                Some(k) => crate::scenes::game::fx::attach_windup(c, &part, (size, size), TITAN_MARKER_WINDUP_RGB, k),
                None => crate::scenes::game::fx::clear_windup(c, &part),
            }
        }

        // The prominence: its path while it winds up, the plasma in flight,
        // then the arc burning where it lies.
        match f.arc {
            Some((path, stage, progress)) => {
                // The head in flight is as big as what it hits; the path and
                // the burn are the stroke that hurts.
                let half_w = if stage == ArcStage::Flight { TITAN_ARC_HEAD_R / 0.8 } else { TITAN_ARC_HALF_W };
                let l = path.layout(half_w);
                if let Some(o) = c.get_game_object_mut(&arc_id) {
                    o.size = l.size;
                    o.rotation = l.rotation_deg;
                    o.position = (l.center.0 - l.size.0 * 0.5, l.center.1 - l.size.1 * 0.5);
                    o.visible = true;
                }
                let (color, intensity) = if stage == ArcStage::Path {
                    (crate::scenes::game::fx::lin(TITAN_MARKER_WINDUP_RGB), 1.0)
                } else {
                    (crate::scenes::game::fx::lin(TITAN_PLASMA_RGB), 1.0)
                };
                c.attach_effect(&arc_id, Effect::ProminenceArc {
                    stage, intensity, progress,
                    bulge: l.bulge, thickness: l.thickness, aspect: l.aspect, flip: path.flip,
                }, color, l.size);
            }
            None => {
                c.clear_effect(&arc_id);
                if let Some(o) = c.get_game_object_mut(&arc_id) { o.visible = false; }
            }
        }
        match f.zone {
            Some((at, progress)) => {
                let d = 2.0 * TITAN_ARC_SPLASH_R / 0.62;
                if let Some(o) = c.get_game_object_mut(&zone_id) {
                    o.size = (d, d);
                    o.position = (at.0 - d * 0.5, at.1 - d * 0.5);
                    o.visible = true;
                }
                c.attach_effect(&zone_id, Effect::DangerZone { intensity: 1.0, progress },
                                crate::scenes::game::fx::lin(TITAN_MARKER_WINDUP_RGB), (d, d));
            }
            None => {
                c.clear_effect(&zone_id);
                if let Some(o) = c.get_game_object_mut(&zone_id) { o.visible = false; }
            }
        }
        if let Some(at) = f.landed {
            crate::scenes::game::boss::common::spawn_impact(c, st, at, TITAN_ARC_SPLASH_R * 2.2, TITAN_PLASMA_RGB, false);
            if let Some(cam) = c.camera_mut() {
                cam.shake(HIT_SHAKE_INTENSITY * 0.6, HIT_SHAKE_SECS);
            }
        }

        // A strike: thrown off, off the rope, and a heart — or, charged, one
        // absorption spent.
        if let Some(n) = f.strike {
            let kick = (n.0 * TITAN_ARC_KICK, n.1 * TITAN_ARC_KICK);
            {
                let mut s = st.lock().unwrap();
                s.vx = kick.0;
                s.vy = kick.1;
                s.hooked = false;
                s.active_hook = String::new();
            }
            c.run(Action::Hide { target: Target::name("rope") });
            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum = kick;
            }
            c.set_var("boss_knockback_ticks", Value::I32(18));
            if buffed {
                let mut s = st.lock().unwrap();
                s.buff_absorbs = s.buff_absorbs.saturating_sub(1);
                if s.buff_absorbs == 0 { s.retire_buff(); }
            } else {
                let dead = { st.lock().unwrap().dead };
                if !dead { crate::scenes::game::hearts::lose_heart(c, st); }
            }
        }

        // SOLID: bounce off every part. Open and charged, a hit; closed, the
        // contact penalty (charged, just the bounce).
        if check && f.weak_open && buffed {
            let mut s = st.lock().unwrap();
            if s.hitstop_ticks == 0 {
                s.px = f.pos.0;
                s.py = f.pos.1;
            }
        }
        if let Some(normal) = crate::scenes::game::boss::common::bounce_off_part(
            c, st, f.pos, titan_part_hit_r(i))
        {
            if f.weak_open && buffed {
                let mut landed = false;
                {
                    let mut s = st.lock().unwrap();
                    if s.boss_part_invuln_ticks == 0 {
                        let p = &mut s.boss_parts[i];
                        if p.alive {
                            p.attack_timer += 1;
                            if reach && p.attack_timer == 1 {
                                eprintln!("reach: part {i} first hit {} ticks into its window", p.weakpoint_window);
                            }
                            p.hp -= 1;
                            if check { eprintln!("titan-check: hit part {i} (hp {} -> {})", p.hp + 1, p.hp); }
                            if p.hp <= 0 {
                                p.alive = false;
                                if check { eprintln!("titan-check: part {i} destroyed"); }
                            }
                            landed = true;
                        }
                        if landed {
                            s.boss_part_invuln_ticks = TITAN_PART_INVULN_TICKS;
                            s.buff_hit_flash = 20;
                        }
                    }
                }
                if landed {
                    let r = titan_part_hit_r(i);
                    let at = (f.pos.0 + normal.0 * r, f.pos.1 + normal.1 * r);
                    crate::scenes::game::boss::common::land_hit(
                        c, st, &part, at, size * 0.6, TITAN_MARKER_VULNERABLE_RGB, normal);
                }
            } else if !f.weak_open {
                let penalise = {
                    let mut s = st.lock().unwrap();
                    let fire = s.boss_contact_cooldown == 0 && !s.dead;
                    if fire {
                        s.boss_contact_cooldown = 45;
                    }
                    fire && !buffed
                };
                if penalise {
                    crate::scenes::game::hearts::lose_heart(c, st);
                }
            }
        }
    }

    // HUD total.
    {
        let mut s = st.lock().unwrap();
        s.boss_hp = boss_total_hp(&s);
        s.boss_hp_max = s.boss_hp.max(1);
    }

    // ── Win ────────────────────────────────────────────────────────────────
    if !any_alive {
        if check { eprintln!("titan-check: core destroyed, fight over"); }
        {
            let mut s = st.lock().unwrap();
            titan_end_flare(&mut s);
            titan_clear_guide(c, &mut s);
        }
        hide_titan(c);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.visible = false;
            obj.position = (-6000.0, -6000.0);
        }
        finish_boss(c, st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_solar_wind_outweighs_arena_gravity_at_its_peak() {
        // At 0.03 the wind was weaker than the gravity a roped swing hangs
        // from, and in play nobody noticed it.
        assert!(TITAN_WIND > 2.0 * GRAVITY * BOSS_GRAVITY_SCALE);
    }

    #[test]
    fn the_clock_runs_calm_kindle_flare_vent_and_round_again() {
        let (k, n) = titan_next_phase(CLOCK_CALM, false);
        assert_eq!((k, n), (CLOCK_KINDLE, TITAN_KINDLE_TICKS));
        let (f, n) = titan_next_phase(k, false);
        assert_eq!((f, n), (CLOCK_FLARE, TITAN_FLARE_TICKS));
        let (v, n) = titan_next_phase(f, false);
        assert_eq!((v, n), (CLOCK_VENT, TITAN_VENT_TICKS));
        assert_eq!(titan_next_phase(v, false), (CLOCK_CALM, TITAN_CALM_TICKS));
        assert_eq!(titan_next_phase(v, true), (CLOCK_CALM, TITAN_CALM_TICKS_CORE), "the exposed core flares sooner");
        // The solar system's wash and banner are normalised to ITS lengths.
        assert_eq!(TITAN_KINDLE_TICKS, FLARE_WARN_TICKS);
        assert_eq!(TITAN_FLARE_TICKS, FLARE_ACTIVE_TICKS);
    }

    #[test]
    fn a_prominence_loops_out_away_from_the_star() {
        let core = (0.0, 0.0);
        // A throw across the top of the star, left to right: out is UP.
        let rise = titan_arc_rise((-2000.0, -1500.0), (2000.0, -1500.0), core);
        let arc = ArcPath { start: (-2000.0, -1500.0), end: (2000.0, -1500.0), rise: rise.abs(), flip: rise < 0.0 };
        assert!(arc.point(0.5).1 < -1500.0, "it bulged toward the star: {:?}", arc.point(0.5));
        // The same throw right to left still loops out.
        let rise = titan_arc_rise((2000.0, -1500.0), (-2000.0, -1500.0), core);
        let arc = ArcPath { start: (2000.0, -1500.0), end: (-2000.0, -1500.0), rise: rise.abs(), flip: rise < 0.0 };
        assert!(arc.point(0.5).1 < -1500.0, "{:?}", arc.point(0.5));
        // And the rise is clamped.
        assert_eq!(titan_arc_rise((0.0, 0.0), (100.0, 0.0), (0.0, 500.0)).abs(), TITAN_ARC_RISE_MIN);
    }

    #[test]
    fn the_vents_ring_the_star_a_quarter_turn_apart() {
        let a = titan_vent_offset(0, 0.0);
        let b = titan_vent_offset(2, 0.0);
        assert!((a.0 + b.0).abs() < 1e-2 && (a.1 + b.1).abs() < 1e-2, "opposite: {a:?} {b:?}");
    }
}
