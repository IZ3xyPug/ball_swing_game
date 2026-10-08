//! conductor_fx.rs — the Conductor's attacks, neon sound.
//!
//! A fight played to a bar, so its attacks look like music made visible:
//! synthwave neon, cyan and magenta, everything on the beat.
//!
//! * a bar-line wave is a wall of sound: a glowing waveform aura round the
//!   band, notes and equaliser blips shed in its wake;
//! * the equaliser guard's arcs throw notes and sparks out of the quadrant
//!   they fire in, the louder the further;
//! * the downbeat rings out twice more, cyan then magenta.
//!
//! Visual only: it reads where the fight puts things and never changes them.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;
use super::fx_kit::{self as k, P, Pool};

const CYAN: [u8; 3] = [90, 240, 255];
const MAGENTA: [u8; 3] = [255, 80, 210];
const WHITE: [u8; 3] = [250, 250, 255];
const SLOTS: [&str; 4] = ["conductor_bar_0_top", "conductor_bar_0_bot", "conductor_bar_1_top", "conductor_bar_1_bot"];

struct Fx {
    rng: u64,
    t: u32,
    notes: Pool,
    sparks: Pool,
    /// Each slot's x last frame, to know which way it is going.
    prev_x: [Option<f32>; 4],
    last_eq: [f32; 4],
    ring_was: bool,
    made: bool,
}

fn fx() -> &'static Mutex<Fx> {
    static F: OnceLock<Mutex<Fx>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(Fx {
        rng: 0xC0_4D_0C7,
        t: 0,
        notes: Pool::new("nfx_note", 70, 31),
        sparks: Pool::new("nfx_spark", 110, 30),
        prev_x: [None; 4],
        last_eq: [0.0; 4],
        ring_was: false,
        made: false,
    }))
}

fn neon(rng: &mut u64) -> ([u8; 3], [u8; 3]) {
    if k::rnd(rng) < 0.5 { (CYAN, MAGENTA) } else { (MAGENTA, CYAN) }
}

/// Every boss frame (after the fight's own tick).
pub(crate) fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (on, frozen, eq) = {
        let s = st.lock().unwrap();
        (s.boss_active && s.boss_kind == BossKind::Conductor, super::phase_cine_active(&s), s.conductor_eq)
    };
    let mut guard = fx().lock().unwrap();
    let f = &mut *guard;
    if !on {
        if f.made {
            f.made = false;
            f.notes.clear(c); f.sparks.clear(c);
            for i in 0..4 { k::hide(c, &format!("nfx_aura_{i}")); }
        }
        f.prev_x = [None; 4];
        return;
    }
    if !f.made {
        f.made = true;
        for i in 0..4 { k::ensure(c, &format!("nfx_aura_{i}"), 28); }
    }
    if !frozen { f.t = f.t.wrapping_add(1); }
    let t = f.t as f32;
    let rng = &mut { f.rng };

    // ── Bar-line waves: walls of sound ──
    for (i, slot) in SLOTS.iter().enumerate() {
        let bar = c.get_game_object(slot).filter(|o| o.visible).map(|o| (o.position, o.size));
        let Some((pos, size)) = bar else {
            k::hide(c, &format!("nfx_aura_{i}"));
            f.prev_x[i] = None;
            continue;
        };
        let x = pos.0 + size.0 * 0.5;
        let (top, bot) = (pos.1, pos.1 + size.1);
        let dir = match f.prev_x[i] { Some(px) if (x - px).abs() > 0.1 => (x - px).signum(), _ => 0.0 };
        f.prev_x[i] = Some(x);
        // The aura: a soft neon glow round the band, throbbing.
        let throb = 0.75 + 0.25 * (t * 0.45 + i as f32).sin();
        let col = if i % 2 == 0 { CYAN } else { MAGENTA };
        k::strip(c, &format!("nfx_aura_{i}"), &k::glow_band(), (x, top), (x, bot), size.0 * 9.0,
                 [col[0], col[1], col[2], (200.0 * throb) as u8]);
        if !frozen {
            if f.t % 20 == 0 { k::trace("conductor wave"); }
            // Notes shed in its wake, drifting back and floating up.
            if k::rnd(rng) < 0.6 {
                let y = top + k::rnd(rng) * (bot - top);
                let s = 180.0 + 140.0 * k::rnd(rng);
                let (a, b) = neon(rng);
                f.notes.emit(P::at(x - dir * size.0 * 2.0, y).vel(-dir * (6.0 + 6.0 * k::rnd(rng)), -2.0 - 3.0 * k::rnd(rng))
                    .life(40.0 + 20.0 * k::rnd(rng)).size(s * 0.75, s).rot(k::rnds(rng) * 15.0, k::rnds(rng) * 1.0)
                    .drag(0.97).shape(k::NOTE).rgb(a, b).fade_in(0.1));
            }
            // Equaliser blips: short upright bars flicking off the band.
            for _ in 0..3 {
                let y = top + k::rnd(rng) * (bot - top);
                let h = 120.0 + 260.0 * k::rnd(rng);
                let (a, b) = neon(rng);
                f.sparks.emit(P::at(x - dir * (size.0 * 1.5 + 200.0 * k::rnd(rng)), y).vel(-dir * 10.0, 0.0)
                    .life(10.0 + 8.0 * k::rnd(rng)).size(h, 26.0).rot(90.0, 0.0).shape(k::SPARK).rgb(a, b).fade_in(0.01));
            }
        }
    }

    // ── The equaliser guard: notes thrown from the quadrant that fires ──
    let core = c.get_game_object("conductor_core").filter(|o| o.visible)
        .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5));
    if let Some((cx, cy)) = core {
        if !frozen {
            for q in 0..4 {
                // A fresh push: this quadrant's level jumped this frame.
                if eq[q] > f.last_eq[q] + 0.2 {
                    k::trace("conductor eq");
                    let mid = q as f32 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4 - std::f32::consts::PI;
                    let n = (10.0 + 14.0 * eq[q]) as usize;
                    for _ in 0..n {
                        let a = mid + k::rnds(rng) * 0.7;
                        let r0 = CONDUCTOR_CORE_SIZE * 0.6;
                        let sp = 26.0 + 30.0 * k::rnd(rng) * eq[q];
                        let (ca, cb) = neon(rng);
                        if k::rnd(rng) < 0.4 {
                            let s = 200.0 + 120.0 * k::rnd(rng);
                            f.notes.emit(P::at(cx + a.cos() * r0, cy + a.sin() * r0).vel(a.cos() * sp, a.sin() * sp)
                                .life(34.0).size(s * 0.75, s).rot(k::rnds(rng) * 20.0, k::rnds(rng) * 2.0).drag(0.95)
                                .shape(k::NOTE).rgb(WHITE, ca).fade_in(0.05));
                        } else {
                            f.sparks.emit(P::at(cx + a.cos() * r0, cy + a.sin() * r0).vel(a.cos() * sp * 1.4, a.sin() * sp * 1.4)
                                .life(18.0 + 10.0 * k::rnd(rng)).size(240.0, 18.0).drag(0.94).shape(k::SPARK).align()
                                .rgb(ca, cb));
                        }
                    }
                }
            }
        }
        // ── The downbeat: two more rings, cyan then magenta ──
        let ring = c.get_game_object("conductor_beat_ring").map_or(false, |o| o.visible);
        if ring && !f.ring_was && !frozen {
            let d = CONDUCTOR_CORE_SIZE * 1.4;
            f.sparks.emit(P::at(cx, cy).life(30.0).size(d, d).grow(3.5).shape(k::RING).rgb(CYAN, MAGENTA).fade_in(0.01));
            f.sparks.emit(P::at(cx, cy).life(40.0).size(d * 0.8, d * 0.8).grow(4.5).shape(k::RING).rgb(MAGENTA, CYAN)
                .alpha(0.7).fade_in(0.2));
        }
        f.ring_was = ring;
    }
    f.last_eq = eq;

    if !frozen { f.notes.step(); f.sparks.step(); }
    f.rng = *rng;
    f.sparks.draw(c); f.notes.draw(c);
}
