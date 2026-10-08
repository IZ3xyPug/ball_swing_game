//! weaver_fx.rs — the Gravity Weaver's attacks, starlight thread.
//!
//! The Weaver spins space itself, so its attacks are woven out of light — a
//! loom of stars, gold-white and glittering, where the Serpent's are void:
//!
//! * a live thread races with sparkles running out along it to a needle-bright
//!   tip that sheds glints; winding back in, the sparkles drift home calmly;
//! * the count-in to a flip draws stardust spiralling into the loom;
//! * the flip itself sweeps a sheet of starlight across the arena in the
//!   new direction of fall, and rings out from the loom.
//!
//! Visual only. The fight calls `thread` with each thread it draws.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;
use super::fx_kit::{self as k, P, Pool};

const GOLD: [u8; 3] = [255, 230, 160];
const LILAC: [u8; 3] = [200, 150, 255];
const WHITE: [u8; 3] = [255, 252, 240];
const SHEATHS: usize = 4;

struct Fx {
    rng: u64,
    t: u32,
    glints: Pool,
    dust: Pool,
    /// This frame's threads: (spindle, direction, length, 0 lane / 1 live / 2 reeling).
    threads: Vec<((f32, f32), (f32, f32), f32, u8)>,
    last_flips: u32,
    made: bool,
}

fn fx() -> &'static Mutex<Fx> {
    static F: OnceLock<Mutex<Fx>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(Fx {
        rng: 0x3EA7_E12,
        t: 0,
        glints: Pool::new("wfx_glint", 110, 19),
        dust: Pool::new("wfx_dust", 90, 19),
        threads: Vec::new(),
        last_flips: u32::MAX,
        made: false,
    }))
}

/// A thread the fight is drawing this frame.
pub(crate) fn thread(from: (f32, f32), dir: (f32, f32), len: f32, kind: u8) {
    fx().lock().unwrap().threads.push((from, dir, len, kind));
}

/// Every boss frame (after the fight's own tick, so `thread` has run).
pub(crate) fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (on, frozen, flips, flip_ticks, inverted, gauntlet) = {
        let s = st.lock().unwrap();
        (s.boss_active && s.boss_kind == BossKind::GravityWeaver, super::phase_cine_active(&s),
         s.weaver_flips, s.weaver_flip_ticks, s.weaver_inverted, s.weaver_gauntlet)
    };
    let mut guard = fx().lock().unwrap();
    let f = &mut *guard;
    let threads = std::mem::take(&mut f.threads);
    if !on {
        if f.made {
            f.made = false;
            f.glints.clear(c); f.dust.clear(c);
            k::hide(c, "wfx_loom_glow");
            for i in 0..SHEATHS { k::hide(c, &format!("wfx_sheath_{i}")); }
        }
        f.last_flips = u32::MAX;
        return;
    }
    if !f.made {
        f.made = true;
        k::ensure(c, "wfx_loom_glow", 19);
        for i in 0..SHEATHS { k::ensure(c, &format!("wfx_sheath_{i}"), 19); }
    }
    if !frozen { f.t = f.t.wrapping_add(1); }
    let t = f.t as f32;
    let rng = &mut { f.rng };
    let tau = std::f32::consts::TAU;

    // ── Threads: a soft gold sheath on each live one ──
    let mut sheaths = 0;
    for &(from, d, len, kind) in &threads {
        if kind != 1 || sheaths >= SHEATHS { continue; }
        let tip = (from.0 + d.0 * len, from.1 + d.1 * len);
        let a = (170.0 + 50.0 * (t * 0.6).sin()) as u8;
        k::strip(c, &format!("wfx_sheath_{sheaths}"), &k::glow_band(), from, tip, WEAVER_THREAD_W * 3.2, [255, 235, 180, a]);
        sheaths += 1;
    }
    for i in sheaths..SHEATHS { k::hide(c, &format!("wfx_sheath_{i}")); }
    if !frozen {
        for &(from, d, len, kind) in &threads {
            let n = (-d.1, d.0);
            match kind {
                1 => {
                    // Sparkles racing out along it.
                    for _ in 0..5 {
                        let u = k::rnd(rng) * 0.9;
                        let (x, y) = (from.0 + d.0 * len * u, from.1 + d.1 * len * u);
                        let off = k::rnds(rng) * WEAVER_THREAD_W * 0.6;
                        f.glints.emit(P::at(x + n.0 * off, y + n.1 * off).vel(d.0 * 40.0, d.1 * 40.0)
                            .life(10.0 + 8.0 * k::rnd(rng)).size(380.0, 26.0).shape(k::SPARK).align()
                            .rgb(WHITE, GOLD).fade_in(0.01));
                    }
                    for _ in 0..4 {
                        let u = k::rnd(rng);
                        let (x, y) = (from.0 + d.0 * len * u, from.1 + d.1 * len * u);
                        let s = 140.0 + 180.0 * k::rnd(rng);
                        f.glints.emit(P::at(x, y).life(12.0 + 10.0 * k::rnd(rng)).size(s, s).rot(k::rnd(rng) * 90.0, 2.0)
                            .shape(k::STAR).rgb(WHITE, LILAC).fade_in(0.2));
                    }
                    // The needle: a bright glint at the tip, shedding dust.
                    let tip = (from.0 + d.0 * len, from.1 + d.1 * len);
                    let s = 520.0 + 120.0 * (t * 0.8).sin();
                    f.glints.emit(P::at(tip.0, tip.1).life(2.0).size(s, s).rot(t * 4.0, 0.0).shape(k::STAR)
                        .rgb(WHITE, WHITE).fade_in(0.01));
                    let a = k::rnd(rng) * tau;
                    f.dust.emit(P::at(tip.0, tip.1).vel(a.cos() * 6.0, a.sin() * 6.0).life(40.0).size(54.0, 54.0)
                        .drag(0.96).shape(k::DOT).rgb(GOLD, LILAC));
                }
                2 => {
                    // Reeling in: a few motes drifting home, calm.
                    if k::rnd(rng) < 0.5 {
                        let u = k::rnd(rng);
                        let (x, y) = (from.0 + d.0 * len * u, from.1 + d.1 * len * u);
                        f.dust.emit(P::at(x, y).vel(-d.0 * 12.0, -d.1 * 12.0).life(30.0).size(56.0, 56.0)
                            .shape(k::DOT).rgb(GOLD, LILAC).alpha(0.7));
                    }
                }
                _ => {}
            }
        }
    }

    // ── The loom: stardust drawn in through the count-in ──
    let loom = c.get_game_object("boss").map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5));
    let counting = gauntlet == 0 && flip_ticks < WEAVER_FLIP_TELEGRAPH;
    if let Some((x, y)) = loom {
        if counting {
            let kk = 1.0 - flip_ticks as f32 / WEAVER_FLIP_TELEGRAPH as f32;
            let s = WEAVER_LOOM_SIZE * (1.4 + 0.8 * kk);
            k::put(c, "wfx_loom_glow", &k::img(k::HALO), (x, y), (s, s), 0.0, [220, 180, 255, (90.0 + 140.0 * kk) as u8]);
            if !frozen {
                for _ in 0..(1 + (kk * 4.0) as usize) {
                    let a = k::rnd(rng) * tau;
                    let rr = WEAVER_LOOM_SIZE * (1.6 + 1.4 * k::rnd(rng));
                    // Inward and round, a spiral's step.
                    let (vx, vy) = (-a.cos() * 26.0 - a.sin() * 30.0, -a.sin() * 26.0 + a.cos() * 30.0);
                    let s = 40.0 + 70.0 * k::rnd(rng);
                    f.dust.emit(P::at(x + a.cos() * rr, y + a.sin() * rr).vel(vx, vy).life(rr / 42.0)
                        .size(s, s).rot(k::rnd(rng) * 90.0, 3.0).shape(k::STAR).rgb(LILAC, WHITE).alpha(0.9));
                }
            }
        } else {
            k::hide(c, "wfx_loom_glow");
        }

        // ── The flip: a sheet of starlight falling the new way ──
        if f.last_flips != u32::MAX && flips != f.last_flips && !frozen {
            k::trace("weaver flip");
            let down = if inverted { -1.0 } else { 1.0 };
            for _ in 0..50 {
                let px = x + k::rnds(rng) * 5200.0;
                let py = y + k::rnds(rng) * 2600.0;
                f.dust.emit(P::at(px, py).vel(0.0, down * (40.0 + 40.0 * k::rnd(rng))).life(20.0 + 14.0 * k::rnd(rng))
                    .size(300.0 + 300.0 * k::rnd(rng), 14.0).shape(k::SPARK).align().rgb(WHITE, LILAC).alpha(0.8));
            }
            for (sz, life) in [(WEAVER_LOOM_SIZE * 1.4, 26.0), (WEAVER_LOOM_SIZE * 0.9, 34.0)] {
                f.glints.emit(P::at(x, y).life(life).size(sz, sz).grow(3.2).shape(k::RING)
                    .rgb(WHITE, LILAC).fade_in(0.01));
            }
        }
    }
    f.last_flips = flips;

    if !frozen { f.glints.step(); f.dust.step(); }
    f.rng = *rng;
    f.dust.draw(c); f.glints.draw(c);
}
