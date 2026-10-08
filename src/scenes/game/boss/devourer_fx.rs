//! devourer_fx.rs — the Sun Devourer's attacks, a black sun.
//!
//! The Devourer has eaten the light (its whole fight is dark), so its look is
//! an eclipse: black at the heart, a burning red-gold rim, smoke and embers.
//!
//! * its body wears a corona — flame tongues standing off its rim all the
//!   time, so it reads as a sun gone dark rather than a sprite in the dark;
//! * its bolts are black fire: a red-gold halo round each, dragging smoke and
//!   embers behind it;
//! * its lunge leaves a wake of dark corona and a ring where it sets off;
//! * the generators' tethers crackle with dark-gold lightning.
//!
//! Visual only: it reads where the fight puts things and never changes them.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;
use super::fx_kit::{self as k, Bolts, P, Pool};

const EMBER: [u8; 3] = [255, 170, 60];
const RIM: [u8; 3] = [255, 80, 30];
const SMOKE_C: [u8; 3] = [70, 25, 40];

struct Fx {
    rng: u64,
    t: u32,
    corona: Pool,
    smoke: Pool,
    embers: Pool,
    bolts: Bolts,
    was_lunging: bool,
    made: bool,
}

fn fx() -> &'static Mutex<Fx> {
    static F: OnceLock<Mutex<Fx>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(Fx {
        rng: 0xDE_70_0E2,
        t: 0,
        corona: Pool::new("dfx_corona", 90, 20),
        smoke: Pool::new("dfx_smoke", 90, 29),
        embers: Pool::new("dfx_ember", 100, 31),
        bolts: Bolts::new("dfx_bolt", 60, 19),
        was_lunging: false,
        made: false,
    }))
}

/// Every boss frame (after the fight's own tick).
pub(crate) fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (on, frozen, lunging, alive) = {
        let s = st.lock().unwrap();
        (s.boss_active && s.boss_kind == BossKind::SunDevourer && s.boss_spawned,
         super::phase_cine_active(&s), s.boss_lunge_ticks > 0, s.boss_hp > 0)
    };
    let mut guard = fx().lock().unwrap();
    let f = &mut *guard;
    if !on {
        if f.made {
            f.made = false;
            f.corona.clear(c); f.smoke.clear(c); f.embers.clear(c); f.bolts.draw(c, &[]);
            k::hide(c, "dfx_rim");
            for i in 0..BOSS_BOLT_POOL_SIZE { k::hide(c, &format!("dfx_bolt_glow_{i}")); }
        }
        return;
    }
    if !f.made {
        f.made = true;
        k::ensure(c, "dfx_rim", 20);
        for i in 0..BOSS_BOLT_POOL_SIZE { k::ensure(c, &format!("dfx_bolt_glow_{i}"), 30); }
    }
    if !frozen { f.t = f.t.wrapping_add(1); }
    let t = f.t as f32;
    let rng = &mut { f.rng };
    let tau = std::f32::consts::TAU;

    // ── The body's corona ──
    let body = c.get_game_object("boss").filter(|o| o.visible)
        .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5));
    if let (Some((x, y)), true) = (body, alive) {
        let r = DEVOURER_BODY_SIZE * 0.5;
        let s = DEVOURER_BODY_SIZE * (2.4 + 0.1 * (t * 0.07).sin());
        k::put(c, "dfx_rim", &k::img(k::HALO), (x, y), (s, s), 0.0, [255, 130, 50, 255]);
        if !frozen {
            for _ in 0..4 {
                let a = k::rnd(rng) * tau;
                let h = r * (0.7 + 0.6 * k::rnd(rng));
                let (px, py) = (x + a.cos() * r * 0.92, y + a.sin() * r * 0.92);
                f.corona.emit(P::at(px + a.cos() * h * 0.4, py + a.sin() * h * 0.4).vel(a.cos() * 3.0, a.sin() * 3.0)
                    .life(26.0 + 16.0 * k::rnd(rng)).size(h * 0.45, h).grow(-0.3)
                    // The flame shape stands tip-up; turn it to point out of the rim.
                    .rot(a.to_degrees() + 90.0, 0.0).shape(k::FLAME).rgb([255, 225, 130], [230, 40, 20]).fade_in(0.15));
            }
            if f.t % 3 == 0 {
                let a = k::rnd(rng) * tau;
                f.embers.emit(P::at(x + a.cos() * r, y + a.sin() * r).vel(a.cos() * 6.0, a.sin() * 6.0 - 1.0)
                    .life(50.0).size(30.0, 30.0).drag(0.98).shape(k::DOT).rgb(EMBER, RIM));
            }
        }
        // ── The lunge: a wake of dark corona, a ring where it set off ──
        if lunging && !frozen {
            if !f.was_lunging {
                k::trace("devourer lunge");
                f.embers.emit(P::at(x, y).life(24.0).size(DEVOURER_BODY_SIZE * 1.2, DEVOURER_BODY_SIZE * 1.2).grow(2.4)
                    .shape(k::RING).rgb([255, 200, 120], RIM).fade_in(0.01));
            }
            for _ in 0..3 {
                let s = DEVOURER_BODY_SIZE * (0.5 + 0.3 * k::rnd(rng));
                f.smoke.emit(P::at(x + k::rnds(rng) * r * 0.5, y + k::rnds(rng) * r * 0.5).life(36.0).size(s, s).grow(0.8)
                    .rot(k::rnd(rng) * 360.0, k::rnds(rng)).shape(k::SMOKE).rgb(SMOKE_C, [20, 5, 15]).alpha(0.7));
            }
            for _ in 0..2 {
                let a = k::rnd(rng) * tau;
                f.embers.emit(P::at(x + a.cos() * r, y + a.sin() * r).vel(a.cos() * 10.0, a.sin() * 10.0).life(30.0)
                    .size(170.0, 16.0).drag(0.95).shape(k::SPARK).align().rgb([255, 220, 140], RIM));
            }
        }
    } else {
        k::hide(c, "dfx_rim");
    }
    f.was_lunging = lunging;

    // ── Bolts: black fire ──
    for i in 0..BOSS_BOLT_POOL_SIZE {
        let bolt = c.get_game_object(&format!("boss_bolt_{i}")).filter(|o| o.visible)
            .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5, o.rotation));
        let glow = format!("dfx_bolt_glow_{i}");
        let Some((x, y, rot)) = bolt else { k::hide(c, &glow); continue };
        let s = BOSS_BOLT_W * (3.2 + 0.3 * (t * 0.8 + i as f32).sin());
        k::put(c, &glow, &k::img(k::HALO), (x, y), (s, s), 0.0, [255, 120, 50, 230]);
        if !frozen {
            if f.t % 30 == 0 { k::trace("devourer bolt"); }
            let back = (-(rot.to_radians().cos()), -(rot.to_radians().sin()));
            let sm = BOSS_BOLT_W * (1.0 + 0.8 * k::rnd(rng));
            f.smoke.emit(P::at(x + back.0 * BOSS_BOLT_W * 0.5, y + back.1 * BOSS_BOLT_W * 0.5)
                .vel(back.0 * 2.0 + k::rnds(rng), back.1 * 2.0 + k::rnds(rng)).life(30.0).size(sm, sm).grow(1.2)
                .rot(k::rnd(rng) * 360.0, k::rnds(rng) * 2.0).shape(k::SMOKE).rgb([90, 30, 40], [25, 8, 15]).alpha(0.75));
            f.embers.emit(P::at(x + k::rnds(rng) * 30.0, y + k::rnds(rng) * 30.0)
                .vel(back.0 * 5.0 + k::rnds(rng) * 3.0, back.1 * 5.0 + k::rnds(rng) * 3.0).life(22.0 + 10.0 * k::rnd(rng))
                .size(26.0, 26.0).drag(0.96).shape(k::DOT).rgb(EMBER, RIM));
        }
    }

    // ── The generators' tethers: dark-gold lightning ──
    let mut lines = Vec::new();
    if f.t % 2 == 0 || frozen {
        for i in 0..BOSS_GENERATOR_COUNT {
            let Some((pos, size, rot, vis)) = c.get_game_object(&format!("boss_gen_tether_{i}"))
                .map(|o| (o.position, o.size, o.rotation, o.visible)) else { continue };
            if !vis || size.0 < 20.0 { continue; }
            let d = (rot.to_radians().cos(), rot.to_radians().sin());
            let mid = (pos.0 + size.0 * 0.5, pos.1 + size.1 * 0.5);
            let a = (mid.0 - d.0 * size.0 * 0.5, mid.1 - d.1 * size.0 * 0.5);
            let b = (mid.0 + d.0 * size.0 * 0.5, mid.1 + d.1 * size.0 * 0.5);
            let segs = ((size.0 / 220.0) as usize).clamp(4, 10);
            lines.push((k::bolt_path(a, b, segs, 70.0, rng), 16.0, [255, 200, 110, 230]));
        }
        f.bolts.draw(c, &lines);
    }

    if !frozen { f.corona.step(); f.smoke.step(); f.embers.step(); }
    f.rng = *rng;
    f.corona.draw(c); f.smoke.draw(c); f.embers.draw(c);
}
