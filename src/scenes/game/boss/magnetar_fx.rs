//! magnetar_fx.rs — the Magnetar's attacks, lightning and magnetism.
//!
//! A dead star with a field strong enough to tear the rope off. Its look is
//! electric — cold blue-white, jagged, never still:
//!
//! * charging, iron-filing streaks gather along each beam's axis and run in
//!   to the poles; live, lightning crackles along the beam and spits sparks
//!   off its root;
//! * the field's pulse is SEEN: a pull drags streaks in from the whole
//!   arena, a push blasts them out behind a shock ring;
//! * a quake throws bolts of lightning out from the star.
//!
//! Visual only: it reads where the fight puts things and never changes them.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;
use super::fx_kit::{self as k, Bolts, P, Pool};
use super::magnetar::{BEAM_CHARGING, BEAM_LIVE, MAGNETAR_BEAMS, MAGNETAR_CORE_IDX, MCLOCK_BEAMS,
    MCLOCK_COUNT_IN, MCLOCK_PULSE, MCLOCK_QUAKE};

struct Fx {
    rng: u64,
    t: u32,
    sparks: Pool,
    field: Pool,
    bolts: Bolts,
    /// The beams' lightning, re-rolled every few ticks (it crackles).
    paths: Vec<(Vec<(f32, f32)>, f32, [u8; 4])>,
    /// Quake bolts: (end point, ticks left).
    quake: Vec<((f32, f32), u32)>,
    last_clock: u8,
    made: bool,
}

fn fx() -> &'static Mutex<Fx> {
    static F: OnceLock<Mutex<Fx>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(Fx {
        rng: 0x4A6_E7A2,
        t: 0,
        sparks: Pool::new("mfx_spark", 90, 19),
        field: Pool::new("mfx_field", 90, 19),
        bolts: Bolts::new("mfx_bolt", 120, 19),
        paths: Vec::new(),
        quake: Vec::new(),
        last_clock: 255,
        made: false,
    }))
}

const COLD: [u8; 3] = [150, 220, 255];
const WHITE: [u8; 3] = [235, 250, 255];

/// Every boss frame (after the fight's own tick).
pub(crate) fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (on, frozen, clock, clock_left, beam, pull) = {
        let s = st.lock().unwrap();
        (s.boss_active && s.boss_kind == BossKind::Magnetar, super::phase_cine_active(&s),
         s.magnetar_clock, s.magnetar_clock_ticks, s.magnetar_beam, s.magnetar_pull_next)
    };
    let mut guard = fx().lock().unwrap();
    let f = &mut *guard;
    if !on {
        if f.made {
            f.made = false;
            f.sparks.clear(c); f.field.clear(c); f.bolts.draw(c, &[]);
            f.paths.clear(); f.quake.clear();
        }
        f.last_clock = 255;
        return;
    }
    f.made = true;
    if !frozen { f.t = f.t.wrapping_add(1); }
    let rng = &mut { f.rng };
    let tau = std::f32::consts::TAU;
    let core = c.get_game_object(&format!("magnetar_part_{MAGNETAR_CORE_IDX}")).filter(|o| o.visible)
        .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5))
        .or_else(|| c.get_game_object("boss").map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5)));

    // ── The beams ──
    let reroll = !frozen && f.t % 3 == 0;
    if reroll { f.paths.clear(); }
    let mut any_live = false;
    for kk in 0..MAGNETAR_BEAMS {
        let Some((pos, size, rot, vis)) = c.get_game_object(&format!("magnetar_beam_{kk}"))
            .map(|o| (o.position, o.size, o.rotation, o.visible)) else { continue };
        if !vis { continue; }
        let d = (rot.to_radians().cos(), rot.to_radians().sin());
        let mid = (pos.0 + size.0 * 0.5, pos.1 + size.1 * 0.5);
        let start = (mid.0 - d.0 * size.0 * 0.5, mid.1 - d.1 * size.0 * 0.5);
        let end = (mid.0 + d.0 * size.0 * 0.5, mid.1 + d.1 * size.0 * 0.5);
        let n = (-d.1, d.0);
        if clock == MCLOCK_BEAMS && beam == BEAM_LIVE {
            any_live = true;
            if reroll {
                for (w, jag, a) in [(40.0, MAGNETAR_BEAM_HALF * 2.2, 255u8), (22.0, MAGNETAR_BEAM_HALF * 3.6, 200)] {
                    let segs = ((size.0 / 260.0) as usize).clamp(6, 22);
                    let p = k::bolt_path(start, end, segs, jag, rng);
                    f.paths.push((p, w, [WHITE[0], WHITE[1], WHITE[2], a]));
                }
            }
            if !frozen {
                // Sparks spat off the root, and off the line here and there.
                for _ in 0..2 {
                    let a = rot.to_radians() + k::rnds(rng) * 0.9;
                    let sp = 24.0 + 30.0 * k::rnd(rng);
                    f.sparks.emit(P::at(start.0, start.1).vel(a.cos() * sp, a.sin() * sp).life(14.0 + 10.0 * k::rnd(rng))
                        .size(150.0, 12.0).drag(0.92).shape(k::SPARK).align().rgb(WHITE, COLD));
                }
                let u = k::rnd(rng);
                let (x, y) = (start.0 + d.0 * size.0 * u, start.1 + d.1 * size.0 * u);
                let side = k::rnds(rng);
                f.sparks.emit(P::at(x, y).vel(n.0 * side * 26.0, n.1 * side * 26.0).life(10.0).size(110.0, 10.0)
                    .drag(0.9).shape(k::SPARK).align().rgb(WHITE, COLD));
            }
        } else if clock == MCLOCK_BEAMS && beam == BEAM_CHARGING && !frozen {
            // Filings gathering along the axis, running in to the pole.
            for _ in 0..2 {
                let u = 0.15 + 0.85 * k::rnd(rng);
                let off = k::rnds(rng) * MAGNETAR_BEAM_HALF * 3.0;
                let (x, y) = (start.0 + d.0 * size.0 * u + n.0 * off, start.1 + d.1 * size.0 * u + n.1 * off);
                f.field.emit(P::at(x, y).vel(-d.0 * 34.0, -d.1 * 34.0).life((size.0 * u / 34.0).min(40.0))
                    .size(120.0, 8.0).shape(k::SPARK).align().rgb(COLD, WHITE).alpha(0.8));
            }
        }
    }
    if !any_live && reroll { f.paths.retain(|_| false); }
    if !any_live { f.paths.clear(); }

    // ── The field's pulse: seen across the arena ──
    if let Some((x, y)) = core {
        let began = clock != f.last_clock;
        if !frozen {
            match clock {
                MCLOCK_COUNT_IN | MCLOCK_PULSE => {
                    let strong = clock == MCLOCK_PULSE;
                    let n = if strong { 4 } else { 1 + (f.t % 2) as usize };
                    for _ in 0..n {
                        let a = k::rnd(rng) * tau;
                        let (sp, r0) = if strong { (60.0, 0.0) } else { (30.0, 0.0) };
                        if pull {
                            let rr = MAGNETAR_FIELD_VISUAL * (0.45 + 0.45 * k::rnd(rng)) + r0;
                            f.field.emit(P::at(x + a.cos() * rr, y + a.sin() * rr).vel(-a.cos() * sp, -a.sin() * sp)
                                .life((rr - MAGNETAR_CORE_SIZE * 0.5) / sp).size(if strong { 380.0 } else { 220.0 }, 14.0)
                                .shape(k::SPARK).align().rgb(COLD, WHITE).alpha(0.85));
                        } else {
                            let rr = MAGNETAR_CORE_SIZE * 0.55;
                            f.field.emit(P::at(x + a.cos() * rr, y + a.sin() * rr).vel(a.cos() * sp, a.sin() * sp)
                                .life(40.0).size(if strong { 380.0 } else { 220.0 }, 14.0).drag(0.99)
                                .shape(k::SPARK).align().rgb(WHITE, COLD).alpha(0.85));
                        }
                    }
                    if strong && began {
                        k::trace("magnetar pulse");
                        f.sparks.emit(P::at(x, y).life(30.0).size(MAGNETAR_CORE_SIZE * 1.2, MAGNETAR_CORE_SIZE * 1.2)
                            .grow(if pull { -0.6 } else { 3.0 }).shape(k::RING).rgb(WHITE, COLD).fade_in(0.01));
                    }
                }
                MCLOCK_QUAKE if began => {
                    k::trace("magnetar quake");
                    for _ in 0..9 {
                        let a = k::rnd(rng) * tau;
                        let rr = MAGNETAR_CORE_SIZE * (1.4 + 1.4 * k::rnd(rng));
                        f.quake.push(((x + a.cos() * rr, y + a.sin() * rr), 16 + (k::rnd(rng) * 10.0) as u32));
                    }
                    f.sparks.emit(P::at(x, y).life(18.0).size(MAGNETAR_CORE_SIZE * 2.4, MAGNETAR_CORE_SIZE * 2.4)
                        .shape(k::HALO).rgb(WHITE, COLD).fade_in(0.01));
                }
                _ => {}
            }
        }
        let _ = clock_left;
        // Quake bolts: star to their ends, re-rolled as they flicker out.
        if !f.quake.is_empty() {
            let mut lines = std::mem::take(&mut f.paths);
            for (end, left) in f.quake.iter() {
                let a = ((*left as f32 / 20.0).min(1.0) * 255.0) as u8;
                let p = k::bolt_path((x, y), *end, 10, 160.0, rng);
                lines.push((p, 22.0, [WHITE[0], WHITE[1], WHITE[2], a]));
            }
            f.paths = lines;
            if !frozen {
                for q in f.quake.iter_mut() { q.1 = q.1.saturating_sub(1); }
                f.quake.retain(|q| q.1 > 0);
            }
        }
    }
    f.last_clock = clock;

    if !frozen { f.sparks.step(); f.field.step(); }
    f.rng = *rng;
    f.field.draw(c);
    let paths = std::mem::take(&mut f.paths);
    f.bolts.draw(c, &paths);
    // Quake bolts are rebuilt every frame; beam bolts keep until the re-roll.
    f.paths = if any_live { paths.into_iter().filter(|p| p.1 != 22.0).collect() } else { Vec::new() };
    f.sparks.draw(c);
}
