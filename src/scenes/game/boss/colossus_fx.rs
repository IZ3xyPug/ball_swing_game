//! colossus_fx.rs — the Colossus's attacks, molten rock.
//!
//! The head's last stand has its own module (`colossus_finale`); this dresses
//! the rest of the fight in the same family — a titan of stone and magma:
//!
//! * a fist landing SHATTERS the ground: rock shards thrown up and falling
//!   back, a dust wall rolling out, magma spatter and a hot shock ring;
//! * the clap's force wave is a ring of superheated air dragging a wall of
//!   dust and sparks with it, not a cold blue circle;
//! * the core vent's spokes are plasma jets — a white-hot core in an orange
//!   sheath — shedding embers along their length and sparks off the tips.
//!
//! Visual only: it reads where the fight puts things and never changes them.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;
use super::fx_kit::{self as k, Img, P, Pool};

struct Fx {
    rng: u64,
    t: u32,
    /// Rock, dust and spatter from slams and the clap (over the parts).
    rock: Pool,
    dust: Pool,
    hot: Pool,
    /// The vent's embers and tip sparks.
    vent: Pool,
    last_clap: u32,
}

fn fx() -> &'static Mutex<Fx> {
    static F: OnceLock<Mutex<Fx>> = OnceLock::new();
    F.get_or_init(|| Mutex::new(Fx {
        rng: 0xC0105_5A1,
        t: 0,
        rock: Pool::new("cfx_rock", 60, 31),
        dust: Pool::new("cfx_dust", 40, 30),
        hot: Pool::new("cfx_hot", 70, 32),
        vent: Pool::new("cfx_vent", 80, 30),
        last_clap: 0,
    }))
}

/// The vent spoke across its width: white core, gold, orange, a red edge.
fn jet_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    k::cached(&I, || k::band(&[
        (0.0, [255.0, 255.0, 255.0, 255.0]), (0.2, [255.0, 248.0, 210.0, 255.0]),
        (0.45, [255.0, 190.0, 80.0, 235.0]), (0.75, [255.0, 100.0, 30.0, 150.0]), (1.0, [180.0, 30.0, 10.0, 0.0]),
    ]))
}

/// The clap's wave: a hot leading edge with a dusty, smoky wake inside it.
fn wave_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    k::cached(&I, || k::radial(256, &[
        (0.0, [120.0, 90.0, 70.0, 0.0]), (0.55, [140.0, 105.0, 80.0, 30.0]),
        (0.82, [200.0, 140.0, 90.0, 110.0]), (0.93, [255.0, 220.0, 150.0, 255.0]),
        (0.97, [255.0, 150.0, 60.0, 170.0]), (1.0, [255.0, 90.0, 30.0, 0.0]),
    ]))
}

/// A fist landing at `at`, its strike `r` across. In open space, so there is
/// no ground to break: the blow bursts outward — rock cracking off the fist,
/// magma spattering, a ring of hot air, a little rock dust.
pub(crate) fn slam(at: (f32, f32), r: f32) {
    k::trace("colossus slam");
    let mut f = fx().lock().unwrap();
    let rng = &mut { f.rng };
    let tau = std::f32::consts::TAU;
    // Rock cracking off, tumbling out.
    for _ in 0..22 {
        let a = k::rnd(rng) * tau;
        let sp = 10.0 + 22.0 * k::rnd(rng);
        let s = 30.0 + 50.0 * k::rnd(rng);
        let grey = 95.0 + 45.0 * k::rnd(rng);
        f.rock.emit(P::at(at.0 + a.cos() * r * 0.3, at.1 + a.sin() * r * 0.3)
            .vel(a.cos() * sp, a.sin() * sp).life(45.0 + 30.0 * k::rnd(rng)).size(s, s * (0.6 + 0.5 * k::rnd(rng)))
            .rot(k::rnd(rng) * 360.0, k::rnds(rng) * 14.0).grav(0.25).drag(0.975).shape(k::SHARD)
            .rgb([grey as u8 + 40, (grey * 0.8) as u8 + 25, (grey * 0.62) as u8 + 10], [90, 60, 45]).fade_in(0.01));
    }
    // A faint puff of rock dust, spreading.
    for _ in 0..5 {
        let a = k::rnd(rng) * tau;
        let sp = 4.0 + 7.0 * k::rnd(rng);
        let s = r * (0.25 + 0.15 * k::rnd(rng));
        f.dust.emit(P::at(at.0 + a.cos() * r * 0.2, at.1 + a.sin() * r * 0.2)
            .vel(a.cos() * sp, a.sin() * sp).life(55.0 + 30.0 * k::rnd(rng)).size(s, s).grow(1.4)
            .rot(k::rnd(rng) * 360.0, k::rnds(rng) * 0.6).drag(0.95).shape(k::SMOKE)
            .rgb([215, 185, 160], [130, 110, 105]).alpha(0.32).fade_in(0.05));
    }
    // Magma spatter: fast streaks, cooling from gold to red.
    for _ in 0..24 {
        let a = k::rnd(rng) * tau;
        let sp = 22.0 + 30.0 * k::rnd(rng);
        f.hot.emit(P::at(at.0, at.1).vel(a.cos() * sp, a.sin() * sp).life(24.0 + 22.0 * k::rnd(rng))
            .size(100.0 + 90.0 * k::rnd(rng), 14.0).grav(0.2).drag(0.965).shape(k::SPARK).align()
            .rgb([255, 240, 170], [255, 70, 20]).fade_in(0.01));
    }
    // Molten droplets that hang a moment longer.
    for _ in 0..10 {
        let a = k::rnd(rng) * tau;
        let sp = 8.0 + 12.0 * k::rnd(rng);
        let s = 26.0 + 26.0 * k::rnd(rng);
        f.hot.emit(P::at(at.0, at.1).vel(a.cos() * sp, a.sin() * sp).life(50.0 + 20.0 * k::rnd(rng))
            .size(s, s).drag(0.97).shape(k::DOT).rgb([255, 220, 140], [200, 40, 10]).fade_in(0.01));
    }
    // The shock: two thin rings of hot air and a short flash.
    f.hot.emit(P::at(at.0, at.1).life(22.0).size(r * 1.0, r * 1.0).grow(2.6).shape(k::RING)
        .rgb([255, 235, 180], [255, 120, 40]).alpha(0.95).fade_in(0.01));
    f.hot.emit(P::at(at.0, at.1).life(30.0).size(r * 0.7, r * 0.7).grow(2.0).shape(k::RING)
        .rgb([255, 170, 90], [200, 60, 20]).alpha(0.6).fade_in(0.15));
    f.hot.emit(P::at(at.0, at.1).life(10.0).size(r * 1.5, r * 1.5).grow(0.3).shape(k::HALO)
        .rgb([255, 225, 160], [255, 120, 50]).alpha(0.7).fade_in(0.01));
    f.rng = *rng;
}

/// Every boss frame (after the fight's own tick).
pub(crate) fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (on, frozen, clap, clap_at) = {
        let s = st.lock().unwrap();
        (s.boss_active && s.boss_kind == BossKind::Colossus, super::phase_cine_active(&s),
         s.boss_clap_wave, s.boss_clap_at)
    };
    let mut guard = fx().lock().unwrap();
    let f = &mut *guard;
    if !on {
        if !f.rock.is_empty() || !f.dust.is_empty() || !f.hot.is_empty() || !f.vent.is_empty() {
            f.rock.clear(c); f.dust.clear(c); f.hot.clear(c); f.vent.clear(c);
        }
        return;
    }
    f.t = f.t.wrapping_add(1);
    let rng = &mut { f.rng };

    // ── The clap's wave ──
    if clap > 0 {
        let t = 1.0 - clap as f32 / COLOSSUS_CLAP_WAVE_TICKS as f32;
        let r = COLOSSUS_CLAP_WAVE_R * t;
        if let Some(o) = c.get_game_object_mut("colossus_clap_wave") {
            o.set_image(Image { shape: ShapeType::Ellipse(0.0, o.size, 0.0), image: wave_img(),
                color: Some(Color(255, 255, 255, (255.0 * (1.0 - t * t)) as u8)) });
        }
        if clap == COLOSSUS_CLAP_WAVE_TICKS || f.last_clap == 0 {
            // The clap itself: a flash between the palms.
            k::trace("colossus clap");
            f.hot.emit(P::at(clap_at.0, clap_at.1).life(14.0).size(1000.0, 1000.0).grow(0.6).shape(k::HALO)
                .rgb([255, 240, 200], [255, 140, 60]).alpha(0.75).fade_in(0.01));
            f.hot.emit(P::at(clap_at.0, clap_at.1).life(14.0).size(900.0, 900.0).grow(1.0).shape(k::STAR)
                .rgb([255, 255, 240], [255, 190, 110]).fade_in(0.01));
        }
        if !frozen {
            // Dust and sparks dragged along the front.
            for _ in 0..3 {
                let a = k::rnd(rng) * std::f32::consts::TAU;
                let (x, y) = (clap_at.0 + a.cos() * r, clap_at.1 + a.sin() * r);
                let s = 200.0 + 200.0 * k::rnd(rng);
                f.dust.emit(P::at(x, y).vel(a.cos() * 8.0, a.sin() * 8.0).life(40.0).size(s, s).grow(1.2)
                    .rot(k::rnd(rng) * 360.0, 0.4).drag(0.94).shape(k::SMOKE)
                    .rgb([225, 195, 165], [140, 115, 105]).alpha(0.3));
            }
            for _ in 0..4 {
                let a = k::rnd(rng) * std::f32::consts::TAU;
                let (x, y) = (clap_at.0 + a.cos() * r, clap_at.1 + a.sin() * r);
                let sp = 26.0 + 20.0 * k::rnd(rng);
                f.hot.emit(P::at(x, y).vel(a.cos() * sp, a.sin() * sp).life(22.0).size(150.0, 16.0)
                    .drag(0.95).shape(k::SPARK).align().rgb([255, 240, 190], [255, 110, 40]));
            }
        }
    }
    f.last_clap = clap;

    // ── The core vent: plasma jets ──
    let torso = c.get_game_object("colossus_part_2").filter(|o| o.visible)
        .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5));
    let mut spokes = Vec::new();
    for i in 0..COLOSSUS_VENT_SPOKES {
        let name = format!("colossus_vent_{i}");
        let Some((pos, size, rot, vis)) = c.get_game_object(&name).map(|o| (o.position, o.size, o.rotation, o.visible)) else { continue };
        if !vis { continue; }
        let flick = 0.92 + 0.08 * ((f.t as f32 * 0.7 + i as f32 * 1.7).sin());
        if let Some(o) = c.get_game_object_mut(&name) {
            o.clear_glow();
            // A touch wider than the hit band: the soft sheath sits outside it.
            o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: jet_img(),
                color: Some(Color(255, 255, 255, (255.0 * flick) as u8)) });
        }
        let c0 = (pos.0 + size.0 * 0.5, pos.1 + size.1 * 0.5);
        let d = (rot.to_radians().cos(), rot.to_radians().sin());
        spokes.push((c0, d, size.0));
    }
    if !frozen {
        if let Some(core) = torso {
            for (_, d, len) in &spokes {
                let n = (-d.1, d.0);
                // Embers peeling off the length, drifting up.
                for _ in 0..2 {
                    let t = 0.1 + 0.9 * k::rnd(rng);
                    let (x, y) = (core.0 + d.0 * len * t, core.1 + d.1 * len * t);
                    let side = k::rnds(rng);
                    f.vent.emit(P::at(x + n.0 * side * 60.0, y + n.1 * side * 60.0)
                        .vel(n.0 * side * 5.0 + k::rnds(rng), n.1 * side * 5.0 - 2.0).life(40.0 + 30.0 * k::rnd(rng))
                        .size(26.0 + 30.0 * k::rnd(rng), 26.0).grav(-0.06).drag(0.97).shape(k::DOT)
                        .rgb([255, 230, 160], [255, 70, 20]));
                }
                // Sparks off the tip, thrown on along the jet.
                let tip = (core.0 + d.0 * len, core.1 + d.1 * len);
                let sp = 30.0 + 20.0 * k::rnd(rng);
                let a = d.1.atan2(d.0) + k::rnds(rng) * 0.5;
                f.vent.emit(P::at(tip.0, tip.1).vel(a.cos() * sp, a.sin() * sp).life(20.0 + 10.0 * k::rnd(rng))
                    .size(170.0, 18.0).drag(0.93).shape(k::SPARK).align().rgb([255, 245, 210], [255, 90, 30]));
            }
            // The chest itself burns while it vents.
            if !spokes.is_empty() && f.t % 60 == 0 { k::trace("colossus vent"); }
            if !spokes.is_empty() && f.t % 3 == 0 {
                f.hot.emit(P::at(core.0, core.1).life(10.0).size(1300.0, 1300.0).shape(k::HALO)
                    .rgb([255, 190, 110], [255, 120, 50]).alpha(0.55));
            }
        }
    }

    if !frozen {
        f.rock.step(); f.dust.step(); f.hot.step(); f.vent.step();
    }
    f.rng = *rng;
    f.rock.draw(c); f.dust.draw(c); f.hot.draw(c); f.vent.draw(c);
}
