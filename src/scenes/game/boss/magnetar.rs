use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
#[allow(unused_imports)]
use super::*;

// ── The Magnetar ────────────────────────────────────────────────────────────
//
// The finale: a neutron star, two magnetic poles on an axis through it, and
// a field that is the fight. Everything the run has taught comes back —
// lanes that clear before the strike, the shelter of a rope, the window
// after the attack — at the speed of a last boss.
//
//   BEAMS   the axis turns like a lighthouse. Each beam cycle: dark, then
//           CHARGING (a thin line where the beam will be, chevrons down its
//           leading side pointing the way it will sweep — clearing just
//           before it fires), then LIVE, sweeping slowly. Get off the line
//           and out of its way.
//   PULSE   the field fires. Count-in first (space bending in for a pull,
//           rings pressing out for a push), then the player is dragged in
//           toward the star or thrown out from it. Pull and push alternate.
//           A rope holds against either; being loose is the danger.
//   QUAKE   a starquake: the beams stall, the axis stops, the crust cracks
//           and the poles are open. The window.
//
// Kill both poles (three hits each) and the beams die with them — the bare
// core goes wild instead, four short beams spinning out of it, and opens in
// each starquake. 2 x 3 + 12 = 18 hits.

/// Index of the core in `boss_parts`; the poles are 0..MAGNETAR_POLES.
pub(crate) const MAGNETAR_CORE_IDX: usize = MAGNETAR_POLES;

pub(crate) const MCLOCK_BEAMS: u8 = 0;
pub(crate) const MCLOCK_COUNT_IN: u8 = 1;
pub(crate) const MCLOCK_PULSE: u8 = 2;
pub(crate) const MCLOCK_QUAKE: u8 = 3;

pub(crate) const BEAM_DARK: u8 = 0;
pub(crate) const BEAM_CHARGING: u8 = 1;
pub(crate) const BEAM_LIVE: u8 = 2;

/// Beams drawn at most at once: one per pole, or four from the bare core.
pub(crate) const MAGNETAR_BEAMS: usize = 4;

fn magnetar_part_size(i: usize) -> f32 {
    if i == MAGNETAR_CORE_IDX { MAGNETAR_CORE_SIZE } else { MAGNETAR_POLE_SIZE }
}

fn magnetar_part_hit_r(i: usize) -> f32 {
    if i == MAGNETAR_CORE_IDX { MAGNETAR_CORE_SIZE * 0.40 } else { MAGNETAR_POLE_SIZE * 0.30 }
}

/// The unit axis of pole `i` (0 or 1: opposite ends) at `angle`.
pub(crate) fn magnetar_axis(i: usize, angle: f32) -> (f32, f32) {
    let a = angle + i as f32 * std::f32::consts::PI;
    (a.cos(), a.sin())
}

/// Where a beam starts, which way it runs, and how long it is: from each
/// live pole's tip while the poles live, else four from the core's surface.
pub(crate) fn magnetar_beams(
    core: (f32, f32), angle: f32, poles_alive: [bool; MAGNETAR_POLES],
) -> Vec<(usize, (f32, f32), (f32, f32), f32)> {
    let mut out = Vec::new();
    if poles_alive.iter().any(|&a| a) {
        for (i, &alive) in poles_alive.iter().enumerate() {
            if !alive { continue; }
            let d = magnetar_axis(i, angle);
            let tip = MAGNETAR_POLE_DIST + MAGNETAR_POLE_SIZE * 0.42;
            out.push((i, (core.0 + d.0 * tip, core.1 + d.1 * tip), d, MAGNETAR_BEAM_LEN));
        }
    } else {
        for k in 0..MAGNETAR_BEAMS {
            let a = angle + k as f32 * std::f32::consts::FRAC_PI_2;
            let d = (a.cos(), a.sin());
            let r = magnetar_part_hit_r(MAGNETAR_CORE_IDX);
            out.push((k, (core.0 + d.0 * r, core.1 + d.1 * r), d, MAGNETAR_CORE_BEAM_LEN));
        }
    }
    out
}

/// The field's pull (positive) or push (negative) on a point `d` from the
/// core, as an acceleration toward the core: strongest at the star, nothing
/// at `MAGNETAR_FIELD_REACH`. Pure, for the test.
pub(crate) fn magnetar_field_accel(d: f32, pull: bool) -> f32 {
    let k = (1.0 - d / MAGNETAR_FIELD_REACH).clamp(0.0, 1.0);
    if pull { MAGNETAR_PULL_ACCEL * k } else { -MAGNETAR_PUSH_ACCEL * k }
}

/// Hide every Magnetar piece. Called on victory, on a fall, and on every
/// fight start.
pub(crate) fn hide_magnetar(c: &mut Canvas) {
    for i in 0..=MAGNETAR_CORE_IDX {
        let name = format!("magnetar_part_{i}");
        c.clear_effect(&name);
        c.clear_overlay_effect(&name);
        if let Some(o) = c.get_game_object_mut(&name) {
            o.visible = false;
            o.position = (-9000.0, -9000.0);
            o.rotation = 0.0;
        }
    }
    for k in 0..MAGNETAR_BEAMS {
        let name = format!("magnetar_beam_{k}");
        c.clear_effect(&name);
        if let Some(o) = c.get_game_object_mut(&name) {
            o.visible = false;
            o.position = (-9000.0, -9000.0);
        }
    }
    c.clear_effect("magnetar_field");
    if let Some(o) = c.get_game_object_mut("magnetar_field") {
        o.visible = false;
        o.position = (-9000.0, -9000.0);
    }
}

pub(crate) fn tick_magnetar(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let s = st.lock().unwrap();
        if !s.boss_active || s.dead || s.boss_stasis_active { return; }
    }
    // MAGNETAR_CHECK: park a buffed player on the first open part, so every
    // hit, art swap, gating and clock path runs in a real fight loop.
    // MAGNETAR_REACH_CHECK: fly a buffed stand-in at a player's speed to the
    // nearest open part and log how far into each window its first hit
    // lands, or that the window closed first.
    let check = std::env::var("MAGNETAR_CHECK").is_ok();
    let reach = std::env::var("MAGNETAR_REACH_CHECK").is_ok();

    // ── Appearance (once) ──────────────────────────────────────────────────
    let spawned = { st.lock().unwrap().boss_spawned };
    if !spawned {
        let name = {
            let mut s = st.lock().unwrap();
            s.boss_spawned = true;
            // A fresh clock every time the fight (re)starts: after a fall the
            // beams must not be live the frame the player lands.
            s.magnetar_clock = MCLOCK_BEAMS;
            s.magnetar_clock_ticks = 0;
            s.magnetar_beam = BEAM_DARK;
            s.magnetar_beam_ticks = MAGNETAR_BEAM_OFF_TICKS * 2;
            s.magnetar_beam_cycles = 0;
            s.magnetar_beam_hit = 0;
            s.magnetar_angle = 0.0;
            s.magnetar_spin = 1.0;
            s.magnetar_pull_next = true;
            s.boss_entry_ticks = 0;
            for p in s.boss_parts.iter_mut() {
                p.state = PartState::Idle;
                p.state_ticks = 0;
            }
            s.boss_kind.name()
        };
        hide_magnetar(c);
        let cx = arena_center_x(c);
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (cx - BOSS_SIZE * 0.5, BOSS_Y_CENTER - BOSS_SIZE * 0.5);
            obj.visible = false;
        }
        if let Ok(font) = Font::from_bytes(include_bytes!("../../../../assets/font.ttf")) {
            let sc = c.virtual_scale();
            if let Some(obj) = c.get_game_object_mut("boss_name_text") {
                obj.set_drawable(Box::new(crate::objects::ui_text_spec(
                    name, &font, 42.0 * sc,
                    Color(MAGNETAR_SRGB.0, MAGNETAR_SRGB.1, MAGNETAR_SRGB.2, 255), 1000.0 * sc,
                )));
            }
        }
    }

    // ── Clocks ─────────────────────────────────────────────────────────────
    let (px, py, buffed) = {
        let mut s = st.lock().unwrap();
        s.boss_entry_ticks = s.boss_entry_ticks.saturating_add(1);
        if s.boss_part_invuln_ticks > 0 { s.boss_part_invuln_ticks -= 1; }
        if s.boss_contact_cooldown > 0 { s.boss_contact_cooldown -= 1; }
        // The star holds still through a quake, like its poles.
        if s.magnetar_clock != MCLOCK_QUAKE {
            s.boss_phase += 0.004;
        }
        if check || reach {
            s.hearts = s.hearts.max(3);
        }
        (s.px, s.py, s.buff_active())
    };
    if check || reach {
        c.set_var("debug_force_buff", true);
    }

    let (bcx, bcy) = {
        let phase = st.lock().unwrap().boss_phase;
        let cx = arena_center_x(c) + (phase * 0.6).sin() * 1300.0;
        let cy = BOSS_Y_CENTER + (phase * 0.45 + 0.9).sin() * 550.0;
        if let Some(obj) = c.get_game_object_mut("boss") {
            obj.position = (cx - BOSS_SIZE * 0.5, cy - BOSS_SIZE * 0.5);
        }
        (cx, cy)
    };

    // ── Phase gating ───────────────────────────────────────────────────────
    let (poles_alive, lone) = {
        let mut s = st.lock().unwrap();
        let alive = [s.boss_parts[0].alive, s.boss_parts[1].alive];
        let live = alive.iter().filter(|&&a| a).count();
        let core = &mut s.boss_parts[MAGNETAR_CORE_IDX];
        if core.alive && core.shielded && live == 0 {
            core.shielded = false;
            if check { eprintln!("magnetar-check: core exposed"); }
        }
        (alive, live == 1)
    };

    // ── The clock ──────────────────────────────────────────────────────────
    // BEAMS runs its beam cycles; a pulse (count-in, then the field) and a
    // starquake follow; round again. The beam cycle only turns inside BEAMS.
    let (clock, clock_left, beam, beam_left, pull, quake_began) = {
        let mut s = st.lock().unwrap();
        let mut quake_began = false;
        match s.magnetar_clock {
            MCLOCK_BEAMS => {
                if s.magnetar_beam_ticks > 0 {
                    s.magnetar_beam_ticks -= 1;
                }
                if s.magnetar_beam_ticks == 0 {
                    match s.magnetar_beam {
                        BEAM_DARK => {
                            s.magnetar_beam = BEAM_CHARGING;
                            s.magnetar_beam_ticks = MAGNETAR_BEAM_CHARGE_TICKS;
                        }
                        BEAM_CHARGING => {
                            s.magnetar_beam = BEAM_LIVE;
                            s.magnetar_beam_ticks = MAGNETAR_BEAM_LIVE_TICKS;
                            s.magnetar_beam_hit = 0;
                        }
                        _ => {
                            s.magnetar_beam_cycles += 1;
                            if s.magnetar_beam_cycles >= MAGNETAR_BEAM_CYCLES {
                                s.magnetar_beam = BEAM_DARK;
                                s.magnetar_beam_cycles = 0;
                                s.magnetar_clock = MCLOCK_COUNT_IN;
                                s.magnetar_clock_ticks = MAGNETAR_PULSE_TELEGRAPH;
                                if check {
                                    let kind = if s.magnetar_pull_next { "pull" } else { "push" };
                                    eprintln!("magnetar-check: {kind} counting in");
                                }
                            } else {
                                s.magnetar_beam = BEAM_DARK;
                                s.magnetar_beam_ticks = if lone { MAGNETAR_BEAM_OFF_TICKS_LONE } else { MAGNETAR_BEAM_OFF_TICKS };
                            }
                        }
                    }
                }
            }
            _ => {
                if s.magnetar_clock_ticks > 0 {
                    s.magnetar_clock_ticks -= 1;
                }
                if s.magnetar_clock_ticks == 0 {
                    match s.magnetar_clock {
                        MCLOCK_COUNT_IN => {
                            s.magnetar_clock = MCLOCK_PULSE;
                            s.magnetar_clock_ticks = MAGNETAR_PULSE_TICKS;
                            s.magnetar_pulses += 1;
                        }
                        MCLOCK_PULSE => {
                            s.magnetar_clock = MCLOCK_QUAKE;
                            s.magnetar_clock_ticks = MAGNETAR_QUAKE_TICKS;
                            s.magnetar_pull_next = !s.magnetar_pull_next;
                            quake_began = true;
                            if check { eprintln!("magnetar-check: starquake (pulse {})", s.magnetar_pulses); }
                        }
                        _ => {
                            // Out of the quake the axis turns the other way.
                            s.magnetar_clock = MCLOCK_BEAMS;
                            s.magnetar_spin = -s.magnetar_spin;
                            s.magnetar_beam = BEAM_DARK;
                            s.magnetar_beam_ticks = if lone { MAGNETAR_BEAM_OFF_TICKS_LONE } else { MAGNETAR_BEAM_OFF_TICKS };
                            if check { eprintln!("magnetar-check: beams resume"); }
                        }
                    }
                }
            }
        }
        // The axis: fast while dark, slowing to charge, slow while live,
        // idling through a pulse, stopped by a quake.
        let spin = match (s.magnetar_clock, s.magnetar_beam) {
            (MCLOCK_BEAMS, BEAM_DARK) => MAGNETAR_SPIN_OFF,
            (MCLOCK_BEAMS, BEAM_CHARGING) => MAGNETAR_SPIN_CHARGE,
            (MCLOCK_BEAMS, _) => if lone { MAGNETAR_SPIN_LIVE_LONE } else { MAGNETAR_SPIN_LIVE },
            (MCLOCK_QUAKE, _) => 0.0,
            _ => MAGNETAR_SPIN_PULSE,
        };
        s.magnetar_angle += spin * s.magnetar_spin;
        // What the pulse does is fixed at its count-in; `pull_next` flips at
        // the quake, after it.
        (s.magnetar_clock, s.magnetar_clock_ticks, s.magnetar_beam, s.magnetar_beam_ticks,
         s.magnetar_pull_next, quake_began)
    };
    if quake_began {
        if let Some(cam) = c.camera_mut() {
            cam.shake(HIT_SHAKE_INTENSITY * 1.5, 0.45);
            cam.flash_with(Color(MAGNETAR_SRGB.0, MAGNETAR_SRGB.1, MAGNETAR_SRGB.2, 120),
                           0.3, FlashMode::Pulse, FlashEase::Sharp, 0.8, 0.0);
        }
    }

    // ── The field ──────────────────────────────────────────────────────────
    if clock == MCLOCK_PULSE {
        let (dx, dy) = (bcx - px, bcy - py);
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let a = magnetar_field_accel(d, pull);
        let mut s = st.lock().unwrap();
        s.vx += dx / d * a;
        s.vy += dy / d * a;
    }
    {
        let dsz = MAGNETAR_FIELD_VISUAL;
        let (effect, show) = match clock {
            MCLOCK_COUNT_IN => {
                let k = 0.25 + 0.75 * (1.0 - clock_left as f32 / MAGNETAR_PULSE_TELEGRAPH as f32);
                if pull {
                    (Effect::GravityLens { strength: k }, true)
                } else {
                    (Effect::SonicRing { intensity: k }, true)
                }
            }
            MCLOCK_PULSE => {
                if pull {
                    (Effect::GravityLens { strength: 1.0 }, true)
                } else {
                    (Effect::SonicRing { intensity: 1.0 }, true)
                }
            }
            // Faintly always: the star is the thing bending space.
            _ => (Effect::GravityLens { strength: 0.1 }, true),
        };
        if show {
            if let Some(o) = c.get_game_object_mut("magnetar_field") {
                o.size = (dsz, dsz);
                o.position = (bcx - dsz * 0.5, bcy - dsz * 0.5);
                o.visible = true;
            }
            let (r, g, b) = MAGNETAR_SRGB;
            c.attach_effect("magnetar_field", effect, EffectColor::srgb8(r, g, b), (dsz, dsz));
        }
    }

    // ── The beams ──────────────────────────────────────────────────────────
    let angle = st.lock().unwrap().magnetar_angle;
    let beams = magnetar_beams((bcx, bcy), angle, poles_alive);
    let charge_shown = MAGNETAR_BEAM_CHARGE_TICKS - MAGNETAR_TELEGRAPH_CLEAR;
    let in_beams = clock == MCLOCK_BEAMS;
    let charging_up = in_beams && beam == BEAM_CHARGING
        && MAGNETAR_BEAM_CHARGE_TICKS - beam_left < charge_shown;
    let live = in_beams && beam == BEAM_LIVE;
    let spin_sign = st.lock().unwrap().magnetar_spin;
    let mut strike: Option<(f32, f32)> = None;
    for k in 0..MAGNETAR_BEAMS {
        let id = format!("magnetar_beam_{k}");
        let this = beams.iter().find(|b| b.0 == k);
        let Some(&(_, start, dir, len)) = this.filter(|_| charging_up || live) else {
            c.clear_effect(&id);
            if let Some(o) = c.get_game_object_mut(&id) { o.visible = false; }
            continue;
        };
        let h = MAGNETAR_BEAM_HALF / 0.15;
        let mid = (start.0 + dir.0 * len * 0.5, start.1 + dir.1 * len * 0.5);
        if let Some(o) = c.get_game_object_mut(&id) {
            o.size = (len, h);
            o.rotation = dir.1.atan2(dir.0).to_degrees();
            o.position = (mid.0 - len * 0.5, mid.1 - h * 0.5);
            o.visible = true;
        }
        // The axis turns toward local +y when the spin is positive (y-down
        // angles grow clockwise), which is the side the chevrons stand on.
        if charging_up {
            let progress = (MAGNETAR_BEAM_CHARGE_TICKS - beam_left) as f32 / charge_shown as f32;
            c.attach_effect(&id, Effect::PulsarBeam {
                stage: BeamStage::Charging, intensity: 1.0, progress, sweep: Some(spin_sign > 0.0),
            }, crate::scenes::game::fx::lin(MAGNETAR_MARKER_WINDUP_RGB), (len, h));
        } else {
            let life = beam_left as f32 / MAGNETAR_BEAM_LIVE_TICKS as f32;
            c.attach_effect(&id, Effect::PulsarBeam {
                stage: BeamStage::Live, intensity: 1.0, progress: life, sweep: None,
            }, crate::scenes::game::fx::lin(MAGNETAR_BEAM_RGB), (len, h));
            // What it burns is what is drawn: the core 30% of the sprite.
            let hit_bit = 1u8 << k;
            let already = st.lock().unwrap().magnetar_beam_hit & hit_bit != 0;
            let end = (start.0 + dir.0 * len, start.1 + dir.1 * len);
            if !already && point_segment_dist((px, py), start, end) < MAGNETAR_BEAM_HALF + PLAYER_R {
                st.lock().unwrap().magnetar_beam_hit |= hit_bit;
                // Thrown the way the beam is sweeping.
                let n = (-dir.1 * spin_sign, dir.0 * spin_sign);
                strike = Some(n);
            }
        }
    }

    // ── Parts ──────────────────────────────────────────────────────────────
    #[derive(Clone, Copy)]
    struct Frame {
        alive: bool,
        shielded: bool,
        weak_open: bool,
        pos: (f32, f32),
        aim_deg: f32,
        damaged: bool,
        closing: Option<f32>,
    }
    let quake = clock == MCLOCK_QUAKE;
    let quake_closing = 1.0 - clock_left as f32 / MAGNETAR_QUAKE_TICKS as f32;
    let frames: Vec<Frame> = {
        let mut s = st.lock().unwrap();
        let mut frames = Vec::with_capacity(s.boss_parts.len());
        for i in 0..s.boss_parts.len() {
            let (pos, aim_deg) = if i == MAGNETAR_CORE_IDX {
                ((bcx, bcy), 0.0)
            } else {
                let d = magnetar_axis(i, angle);
                ((bcx + d.0 * MAGNETAR_POLE_DIST, bcy + d.1 * MAGNETAR_POLE_DIST), d.1.atan2(d.0).to_degrees())
            };
            let p = &mut s.boss_parts[i];
            let damaged = p.hp * 2 <= p.max_hp;
            let weak_open = p.alive && !p.shielded && quake;
            if p.weakpoint_open && !weak_open {
                if reach && p.attack_timer == 0 && p.alive {
                    eprintln!("reach: part {i} window CLOSED unhit after {} ticks", p.weakpoint_window);
                }
                p.weakpoint_window = 0;
                p.attack_timer = 0;
            }
            if weak_open {
                p.weakpoint_window += 1;
            }
            p.weakpoint_open = weak_open;
            frames.push(Frame {
                alive: p.alive, shielded: p.shielded, weak_open, pos, aim_deg, damaged,
                closing: weak_open.then_some(quake_closing),
            });
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
            .unwrap_or((bcx, bcy + 2200.0));
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

    // A beam strike: thrown along the sweep, off the rope, and a heart — or,
    // buffed, one absorption spent.
    if let Some(n) = strike {
        let kick = (n.0 * MAGNETAR_BEAM_KICK, n.1 * MAGNETAR_BEAM_KICK);
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
        if check { eprintln!("magnetar-check: beam struck the player"); }
        if buffed {
            let mut s = st.lock().unwrap();
            s.buff_absorbs = s.buff_absorbs.saturating_sub(1);
            if s.buff_absorbs == 0 { s.retire_buff(); }
        } else {
            let dead = { st.lock().unwrap().dead };
            if !dead { crate::scenes::game::hearts::lose_heart(c, st); }
        }
    }

    // ── Visuals and hits ───────────────────────────────────────────────────
    let mut any_alive = false;
    let pulse_count_in = if clock == MCLOCK_COUNT_IN {
        let k = 1.0 - clock_left as f32 / MAGNETAR_PULSE_TELEGRAPH as f32;
        Some(0.45 + 0.4 * (k * 3.0 * std::f32::consts::PI).sin().abs())
    } else {
        None
    };
    for (i, f) in frames.iter().enumerate() {
        let part = format!("magnetar_part_{i}");
        let size = magnetar_part_size(i);
        if !f.alive {
            c.clear_effect(&part);
            crate::scenes::game::fx::clear_windup(c, &part);
            if let Some(o) = c.get_game_object_mut(&part) { o.visible = false; }
            continue;
        }
        any_alive = true;
        if let Some(o) = c.get_game_object_mut(&part) {
            o.size = (size, size);
            o.position = (f.pos.0 - size * 0.5, f.pos.1 - size * 0.5);
            o.visible = true;
            if i != MAGNETAR_CORE_IDX {
                // Locked to the axis: the beam leaves the crystal's tip.
                o.rotation = f.aim_deg - MAGNETAR_POLE_ART_DEG;
            }
            if f.damaged && o.animated_sprite.is_some() {
                let art = if i == MAGNETAR_CORE_IDX { ASSET_PL_MAGNETAR_CORE_DAMAGED } else { ASSET_PL_MAGNETAR_POLE_DAMAGED };
                if let Some(img) = crate::scenes::game::helpers::pl_image_cached(art, size) {
                    o.animated_sprite = None;
                    o.set_image(Image { shape: ShapeType::Rectangle(0.0, (size, size), 0.0), image: img, color: None });
                    if check { eprintln!("magnetar-check: part {i} shows its damaged art"); }
                }
            }
        }
        {
            let (rgb, k, mode) = if f.shielded {
                (MAGNETAR_MARKER_SHIELDED_RGB, 0.85, MarkerMode::Shielded)
            } else if f.weak_open {
                (MAGNETAR_MARKER_VULNERABLE_RGB, 1.0, MarkerMode::Vulnerable)
            } else {
                (MAGNETAR_MARKER_SHIELDED_RGB, MAGNETAR_CLOSED_SHELL, MarkerMode::Shielded)
            };
            crate::scenes::game::fx::attach_state_marker_timed(
                c, &part, (size, size), rgb, k, mode,
                if mode == MarkerMode::Vulnerable { f.closing } else { None });
            // Wind-up over the state: a pole charging its beam; the core
            // through the pulse's count-in.
            let windup = if i != MAGNETAR_CORE_IDX && charging_up {
                Some(0.95)
            } else if i == MAGNETAR_CORE_IDX {
                pulse_count_in
            } else {
                None
            };
            match windup {
                Some(k) => crate::scenes::game::fx::attach_windup(c, &part, (size, size), MAGNETAR_MARKER_WINDUP_RGB, k),
                None => crate::scenes::game::fx::clear_windup(c, &part),
            }
        }

        if check && f.weak_open && buffed {
            let mut s = st.lock().unwrap();
            if s.hitstop_ticks == 0 {
                s.px = f.pos.0;
                s.py = f.pos.1;
            }
        }
        if let Some(normal) = crate::scenes::game::boss::common::bounce_off_part(
            c, st, f.pos, magnetar_part_hit_r(i))
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
                            if check { eprintln!("magnetar-check: hit part {i} (hp {} -> {})", p.hp + 1, p.hp); }
                            if p.hp <= 0 {
                                p.alive = false;
                                if check { eprintln!("magnetar-check: part {i} destroyed"); }
                            }
                            landed = true;
                        }
                        if landed {
                            s.boss_part_invuln_ticks = MAGNETAR_PART_INVULN_TICKS;
                            s.buff_hit_flash = 20;
                        }
                    }
                }
                if landed {
                    let r = magnetar_part_hit_r(i);
                    let at = (f.pos.0 + normal.0 * r, f.pos.1 + normal.1 * r);
                    crate::scenes::game::boss::common::land_hit(
                        c, st, &part, at, size * 0.6, MAGNETAR_MARKER_VULNERABLE_RGB, normal);
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

    {
        let mut s = st.lock().unwrap();
        s.boss_hp = boss_total_hp(&s);
        s.boss_hp_max = s.boss_hp.max(1);
    }

    if !any_alive {
        if check { eprintln!("magnetar-check: core destroyed, fight over"); }
        hide_magnetar(c);
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
    fn the_poles_sit_at_opposite_ends_of_the_axis() {
        let a = magnetar_axis(0, 0.3);
        let b = magnetar_axis(1, 0.3);
        assert!((a.0 + b.0).abs() < 1e-5 && (a.1 + b.1).abs() < 1e-5, "{a:?} {b:?}");
    }

    #[test]
    fn a_beam_leaves_each_live_pole_and_the_bare_core_throws_four() {
        let both = magnetar_beams((0.0, 0.0), 0.0, [true, true]);
        assert_eq!(both.len(), 2);
        // From the pole's TIP, outward along its axis.
        assert!(both[0].1 .0 > MAGNETAR_POLE_DIST && both[0].2 .0 > 0.99, "{:?}", both[0]);
        assert!(both[1].1 .0 < -MAGNETAR_POLE_DIST && both[1].2 .0 < -0.99, "{:?}", both[1]);
        let one = magnetar_beams((0.0, 0.0), 0.0, [false, true]);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].0, 1, "the surviving pole keeps its own beam");
        let bare = magnetar_beams((0.0, 0.0), 0.0, [false, false]);
        assert_eq!(bare.len(), MAGNETAR_BEAMS);
        assert!(bare.iter().all(|b| b.3 == MAGNETAR_CORE_BEAM_LEN));
    }

    #[test]
    fn the_field_pulls_in_pushes_out_and_fades_with_distance() {
        assert!(magnetar_field_accel(0.0, true) > 0.0);
        assert!(magnetar_field_accel(0.0, false) < 0.0);
        assert!(magnetar_field_accel(1000.0, true) > magnetar_field_accel(3000.0, true));
        assert_eq!(magnetar_field_accel(MAGNETAR_FIELD_REACH + 1.0, true), 0.0);
    }
}
