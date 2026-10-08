//! titan_fx.rs — the Flare Titan's attacks, solar plasma.
//!
//! A star that throws pieces of itself. Where the Colossus's fire is rock and
//! magma (dull reds, smoke, debris), the Titan's is the surface of a sun —
//! white-gold, liquid, and always moving:
//!
//! * a thrown prominence is a burning plasma head shedding droplets along its
//!   flight;
//! * where it lands it SPLASHES — droplets thrown up and raining back, flame
//!   tongues standing up, a ring of light;
//! * the arc then burns where it lies, flames licking up along its length;
//! * kindling, the star draws its corona in (motes falling to it); flaring,
//!   the solar wind blows out across the arena in streaks.
//!
//! Visual only. The fight calls `arc` and `landed` with what it is drawing.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;
use super::fx_kit::{self as k, P, Pool};
use super::flare_titan::{CLOCK_FLARE, CLOCK_KINDLE, TITAN_CORE_IDX};

const HEADS: usize = 8;

struct Fx {
    rng: u64,
    t: u32,
    flames: Pool,
    drops: Pool,
    wind: Pool,
    /// Which arcs have a head in flight this frame (`arc`), and where.
    heads: [Option<(f32, f32)>; HEADS],
    made: bool,
}

fn fx() -> &'static Mutex<Fx> {
    static F: OnceLock<Mutex<Fx>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(Fx {
        rng: 0x7174_4E5,
        t: 0,
        flames: Pool::new("tfx_flame", 90, 19),
        drops: Pool::new("tfx_drop", 90, 19),
        wind: Pool::new("tfx_wind", 70, 27),
        heads: [None; HEADS],
        made: false,
    }))
}

/// The fight's prominence `i` this frame: `stage` and `progress` as it
/// draws them.
pub(crate) fn arc(i: usize, path: &ArcPath, stage: ArcStage, progress: f32) {
    let mut f = fx().lock().unwrap();
    let rng = &mut { f.rng };
    match stage {
        ArcStage::Flight => {
            let at = path.point(progress);
            let back = path.point((progress - 0.03).max(0.0));
            if i < HEADS { f.heads[i] = Some(at); }
            // Droplets shed behind the head, sagging as they cool.
            let (dx, dy) = (at.0 - back.0, at.1 - back.1);
            for _ in 0..3 {
                let s = 30.0 + 40.0 * k::rnd(rng);
                f.drops.emit(P::at(at.0 + k::rnds(rng) * 60.0, at.1 + k::rnds(rng) * 60.0)
                    .vel(-dx * 0.08 + k::rnds(rng) * 3.0, -dy * 0.08 + k::rnds(rng) * 3.0)
                    .life(24.0 + 18.0 * k::rnd(rng)).size(s, s).grav(0.25).drag(0.97).shape(k::DOT)
                    .rgb([255, 235, 140], [245, 70, 0]).fade_in(0.01));
            }
        }
        ArcStage::Burn => {
            // Flames licking up along the length, dying with the burn.
            for _ in 0..2 {
                let at = path.point(k::rnd(rng));
                let h = (180.0 + 200.0 * k::rnd(rng)) * (0.4 + 0.6 * progress);
                f.flames.emit(P::at(at.0 + k::rnds(rng) * 30.0, at.1).vel(k::rnds(rng) * 1.5, -4.0 - 4.0 * k::rnd(rng))
                    .life(22.0 + 14.0 * k::rnd(rng)).size(h * 0.5, h).grow(-0.4).drag(0.98).shape(k::FLAME)
                    .rgb([255, 215, 90], [235, 50, 0]).alpha(0.95).fade_in(0.15));
            }
            if k::rnd(rng) < 0.5 {
                let at = path.point(k::rnd(rng));
                f.drops.emit(P::at(at.0, at.1).vel(k::rnds(rng) * 3.0, -6.0 - 6.0 * k::rnd(rng)).life(30.0)
                    .size(22.0, 22.0).grav(0.15).shape(k::DOT).rgb([255, 220, 110], [245, 60, 0]));
            }
        }
        _ => {}
    }
    f.rng = *rng;
}

/// A prominence landing at `at`: the splash.
pub(crate) fn landed(at: (f32, f32)) {
    k::trace("titan splash");
    let mut f = fx().lock().unwrap();
    let rng = &mut { f.rng };
    let r = TITAN_ARC_SPLASH_R;
    // Droplets thrown up and raining back down.
    for _ in 0..30 {
        let a = -std::f32::consts::FRAC_PI_2 + k::rnds(rng) * 1.4;
        let sp = 14.0 + 24.0 * k::rnd(rng);
        let s = 26.0 + 40.0 * k::rnd(rng);
        f.drops.emit(P::at(at.0, at.1).vel(a.cos() * sp, a.sin() * sp).life(40.0 + 25.0 * k::rnd(rng))
            .size(s, s).grav(0.8).drag(0.985).shape(k::DOT).rgb([255, 235, 140], [245, 60, 0]).fade_in(0.01));
    }
    // Flame tongues standing up out of it.
    for _ in 0..9 {
        let h = 260.0 + 280.0 * k::rnd(rng);
        f.flames.emit(P::at(at.0 + k::rnds(rng) * r * 1.2, at.1 - h * 0.3).vel(k::rnds(rng) * 2.0, -5.0 - 5.0 * k::rnd(rng))
            .life(26.0 + 16.0 * k::rnd(rng)).size(h * 0.45, h).grow(-0.3).drag(0.97).shape(k::FLAME)
            .rgb([255, 220, 100], [235, 50, 0]).fade_in(0.1));
    }
    // A ring of light, a glint, a bloom.
    f.drops.emit(P::at(at.0, at.1).life(20.0).size(r * 2.5, r * 2.5).grow(3.0).shape(k::RING)
        .rgb([255, 250, 220], [255, 140, 40]).fade_in(0.01));
    f.drops.emit(P::at(at.0, at.1).life(14.0).size(r * 5.0, r * 5.0).shape(k::STAR)
        .rgb([255, 255, 240], [255, 190, 90]).fade_in(0.01));
    f.drops.emit(P::at(at.0, at.1).life(18.0).size(r * 6.0, r * 6.0).shape(k::HALO)
        .rgb([255, 220, 140], [255, 120, 40]).alpha(0.8).fade_in(0.01));
    f.rng = *rng;
}

/// Every boss frame (after the fight's own tick, so `arc` has run).
pub(crate) fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (on, frozen, clock, clock_left) = {
        let s = st.lock().unwrap();
        (s.boss_active && s.boss_kind == BossKind::FlareTitan, super::phase_cine_active(&s),
         s.titan_clock, s.titan_clock_ticks)
    };
    let mut guard = fx().lock().unwrap();
    let f = &mut *guard;
    if !on {
        if f.made {
            f.made = false;
            f.flames.clear(c); f.drops.clear(c); f.wind.clear(c);
            for i in 0..HEADS { k::hide(c, &format!("tfx_head_{i}")); k::hide(c, &format!("tfx_core_glow")); }
        }
        return;
    }
    if !f.made {
        f.made = true;
        for i in 0..HEADS { k::ensure(c, &format!("tfx_head_{i}"), 19); }
        k::ensure(c, "tfx_core_glow", 19);
    }
    if !frozen { f.t = f.t.wrapping_add(1); }
    let t = f.t as f32;
    let rng = &mut { f.rng };
    let tau = std::f32::consts::TAU;

    // ── Heads in flight: a burning plasma ball ──
    for i in 0..HEADS {
        match f.heads[i].take() {
            Some(at) => {
                let s = TITAN_ARC_HEAD_R * 3.2 * (1.0 + 0.08 * (t * 0.9 + i as f32).sin());
                k::put(c, &format!("tfx_head_{i}"), &k::img(k::HALO), at, (s, s), 0.0, [255, 210, 120, 230]);
            }
            None => k::hide(c, &format!("tfx_head_{i}")),
        }
    }

    // ── The corona: drawn in while kindling, blown out while flaring ──
    let core = c.get_game_object(&format!("titan_part_{TITAN_CORE_IDX}")).filter(|o| o.visible)
        .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5));
    let cr = TITAN_CORE_SIZE * 0.5;
    match (core, clock) {
        (Some((x, y)), CLOCK_KINDLE) => {
            let kk = 1.0 - clock_left as f32 / TITAN_KINDLE_TICKS.max(1) as f32;
            let s = TITAN_CORE_SIZE * (1.6 + 0.6 * kk);
            k::put(c, "tfx_core_glow", &k::img(k::HALO), (x, y), (s, s), 0.0, [255, 200, 110, (120.0 + 120.0 * kk) as u8]);
            if !frozen {
                for _ in 0..(1 + (kk * 3.0) as usize) {
                    let a = k::rnd(rng) * tau;
                    let rr = cr * (2.6 + 1.6 * k::rnd(rng));
                    let sp = 30.0 + 30.0 * kk;
                    f.wind.emit(P::at(x + a.cos() * rr, y + a.sin() * rr).vel(-a.cos() * sp, -a.sin() * sp)
                        .life((rr - cr) / sp).size(220.0, 18.0).shape(k::SPARK).align()
                        .rgb([255, 200, 120], [255, 250, 220]).alpha(0.9));
                }
            }
        }
        (Some((x, y)), CLOCK_FLARE) => {
            let pulse = 0.85 + 0.15 * (t * 0.5).sin();
            let s = TITAN_CORE_SIZE * 2.8 * pulse;
            k::put(c, "tfx_core_glow", &k::img(k::HALO), (x, y), (s, s), 0.0, [255, 230, 160, 255]);
            if !frozen {
                if f.t % 20 == 0 { k::trace("titan flare wind"); }
                for _ in 0..4 {
                    let a = k::rnd(rng) * tau;
                    let sp = 70.0 + 50.0 * k::rnd(rng);
                    f.wind.emit(P::at(x + a.cos() * cr, y + a.sin() * cr).vel(a.cos() * sp, a.sin() * sp)
                        .life(28.0 + 20.0 * k::rnd(rng)).size(600.0 + 400.0 * k::rnd(rng), 26.0).shape(k::SPARK).align()
                        .rgb([255, 250, 220], [255, 120, 30]).alpha(0.8));
                }
            }
        }
        _ => k::hide(c, "tfx_core_glow"),
    }

    if !frozen { f.flames.step(); f.drops.step(); f.wind.step(); }
    f.rng = *rng;
    f.wind.draw(c); f.flames.draw(c); f.drops.draw(c);
}
