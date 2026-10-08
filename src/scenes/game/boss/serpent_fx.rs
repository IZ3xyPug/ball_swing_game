//! serpent_fx.rs — the Serpent's attacks, void energy.
//!
//! The Serpent swims through rifts, so its attacks are made of the void it
//! swims in — dark at the heart, burning violet at the rim:
//!
//! * the spine it lashes and sweeps with is a ribbon of void (a black core in
//!   a violet sheath) that leaves fading afterimages along its swing and
//!   crackles with glints;
//! * its rifts drag motes spiralling in under a turning violet ring, and an
//!   eruption bursts out of one in a spray of void shards;
//! * the launched tail streaks a trail of violet plasma.
//!
//! Visual only: it reads where the fight puts things and never changes them.

use quartz::*;
use std::sync::{Arc, Mutex, OnceLock};

use crate::constants::*;
use crate::state::*;
use super::fx_kit::{self as k, Img, P, Pool};

const RIFTS: usize = 2;

struct Fx {
    rng: u64,
    t: u32,
    /// Afterimages of the spine (its own ribbon art).
    ghosts: Pool,
    /// Glints, sparks, motes, shards.
    sparks: Pool,
    last_phase: Option<RiftPhase>,
    made: bool,
}

fn fx() -> &'static Mutex<Fx> {
    static F: OnceLock<Mutex<Fx>> = OnceLock::new();
    F.get_or_init(|| {
        let mut ghosts = Pool::new("sfx_ghost", 24, 28);
        ghosts.extra = vec![ribbon_img()];
        Mutex::new(Fx {
            rng: 0x5E4_9E17,
            t: 0,
            ghosts,
            sparks: Pool::new("sfx_spark", 110, 31),
            last_phase: None,
            made: false,
        })
    })
}

/// The spine across its width: a black-violet core, a burning violet rim.
fn ribbon_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    k::cached(&I, || k::band(&[
        (0.0, [20.0, 0.0, 40.0, 255.0]), (0.28, [45.0, 5.0, 90.0, 255.0]),
        (0.55, [210.0, 140.0, 255.0, 255.0]), (0.66, [250.0, 225.0, 255.0, 255.0]),
        (0.8, [150.0, 70.0, 255.0, 140.0]), (1.0, [90.0, 30.0, 200.0, 0.0]),
    ]))
}

/// A rift's turning rim: a broken violet ring (three arcs), so its spin shows.
fn rim_img() -> Img {
    static I: OnceLock<Img> = OnceLock::new();
    k::cached(&I, || {
        let n = 256u32;
        image::RgbaImage::from_fn(n, n, |x, y| {
            let (fx, fy) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
            let d = (fx * fx + fy * fy).sqrt();
            let ang = fy.atan2(fx);
            let arcs = (0.5 + 0.5 * (ang * 3.0).sin()).powf(2.0);
            let ring = (1.0 - ((d - 0.86) / 0.1).abs()).clamp(0.0, 1.0);
            let a = ring * (0.25 + 0.75 * arcs);
            let hot = ring.powf(3.0);
            image::Rgba([(170.0 + 85.0 * hot) as u8, (90.0 + 150.0 * hot) as u8, 255, (a * 255.0) as u8])
        })
    })
}

fn ensure(c: &mut Canvas) {
    for i in 0..RIFTS {
        for j in 0..2 {
            let n = format!("sfx_rim_{i}_{j}");
            if c.get_game_object(&n).is_none() {
                c.add_game_object(n.clone(), k::quad(&n, 31));
            }
        }
    }
}

/// Void shards bursting out of a rift.
fn erupt(f: &mut Fx, at: (f32, f32), rng: &mut u64) {
    k::trace("serpent eruption");
    let tau = std::f32::consts::TAU;
    for _ in 0..30 {
        let a = k::rnd(rng) * tau;
        let sp = 24.0 + 34.0 * k::rnd(rng);
        let s = 110.0 + 150.0 * k::rnd(rng);
        f.sparks.emit(P::at(at.0, at.1).vel(a.cos() * sp, a.sin() * sp).life(30.0 + 20.0 * k::rnd(rng))
            .size(s * 0.5, s).rot(a.to_degrees() + 90.0, k::rnds(rng) * 10.0).drag(0.95).shape(k::SHARD)
            .rgb([225, 190, 255], [70, 20, 140]).fade_in(0.01));
    }
    f.sparks.emit(P::at(at.0, at.1).life(22.0).size(SERPENT_RIFT_R * 2.2, SERPENT_RIFT_R * 2.2).grow(2.4)
        .shape(k::RING).rgb([235, 200, 255], [120, 50, 230]).fade_in(0.01));
    f.sparks.emit(P::at(at.0, at.1).life(14.0).size(SERPENT_RIFT_R * 2.6, SERPENT_RIFT_R * 2.6)
        .shape(k::HALO).rgb([200, 150, 255], [90, 30, 200]).alpha(0.8).fade_in(0.01));
}

/// Every boss frame (after the fight's own tick).
pub(crate) fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (on, frozen, phase, hole, next, tail_out, tail_pos) = {
        let s = st.lock().unwrap();
        (s.boss_active && s.boss_kind == BossKind::Serpent, super::phase_cine_active(&s),
         s.serpent_rift_phase, s.serpent_hole, s.serpent_next_hole, s.serpent_tail_out, s.serpent_tail_pos)
    };
    let mut guard = fx().lock().unwrap();
    let f = &mut *guard;
    if !on {
        if f.made {
            f.made = false;
            f.ghosts.clear(c); f.sparks.clear(c);
            for i in 0..RIFTS { for j in 0..2 { k::hide(c, &format!("sfx_rim_{i}_{j}")); } }
        }
        f.last_phase = None;
        return;
    }
    if !f.made { f.made = true; ensure(c); }
    if !frozen { f.t = f.t.wrapping_add(1); }
    let t = f.t as f32;
    let rng = &mut { f.rng };
    let tau = std::f32::consts::TAU;

    // ── The spine: a void ribbon, afterimages, glints ──
    if let Some((pos, size, rot, vis)) = c.get_game_object("serpent_spine").map(|o| (o.position, o.size, o.rotation, o.visible)) {
        if vis {
            // Thin while it winds up (the warning), full while it hurts.
            let live = size.1 >= SERPENT_LASH_THICKNESS * 0.8;
            if let Some(o) = c.get_game_object_mut("serpent_spine") {
                o.clear_glow();
                o.set_image(Image { shape: ShapeType::Rectangle(0.0, size, 0.0), image: ribbon_img(),
                    color: Some(Color(255, 255, 255, if live { 255 } else { 150 })) });
            }
            let mid = (pos.0 + size.0 * 0.5, pos.1 + size.1 * 0.5);
            let d = (rot.to_radians().cos(), rot.to_radians().sin());
            if live && !frozen {
                if f.t % 30 == 0 { k::trace("serpent spine live"); }
                if f.t % 2 == 0 {
                    f.ghosts.emit(P::at(mid.0, mid.1).life(16.0).size(size.0, size.1).rot(rot, 0.0)
                        .shape(k::EXTRA).rgb([200, 150, 255], [60, 20, 140]).alpha(0.45).fade_in(0.01));
                }
                // Glints crackling along it, sparks shed off the far end.
                for _ in 0..2 {
                    let u = k::rnds(rng) * 0.5;
                    let (x, y) = (mid.0 + d.0 * size.0 * u, mid.1 + d.1 * size.0 * u);
                    let s = 70.0 + 90.0 * k::rnd(rng);
                    f.sparks.emit(P::at(x, y).life(10.0 + 8.0 * k::rnd(rng)).size(s, s).rot(k::rnd(rng) * 90.0, 0.0)
                        .shape(k::STAR).rgb([245, 225, 255], [150, 80, 255]).fade_in(0.01));
                }
                let n = (-d.1, d.0);
                for end in [-0.5f32, 0.5] {
                    let (x, y) = (mid.0 + d.0 * size.0 * end, mid.1 + d.1 * size.0 * end);
                    let side = if k::rnd(rng) < 0.5 { 1.0 } else { -1.0 };
                    let sp = 14.0 + 16.0 * k::rnd(rng);
                    let (vx, vy) = (n.0 * side * sp + k::rnds(rng) * 6.0, n.1 * side * sp + k::rnds(rng) * 6.0);
                    f.sparks.emit(P::at(x, y).vel(vx, vy).life(16.0 + 10.0 * k::rnd(rng)).size(140.0, 14.0)
                        .drag(0.92).shape(k::SPARK).align().rgb([235, 210, 255], [110, 40, 230]));
                }
            }
        }
    }

    // ── The rifts: a turning rim, motes spiralling in ──
    for i in 0..RIFTS {
        let rift = c.get_game_object(&format!("serpent_rift_{i}")).filter(|o| o.visible)
            .map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5, o.size.0 * 0.5));
        let Some((x, y, r)) = rift else {
            for j in 0..2 { k::hide(c, &format!("sfx_rim_{i}_{j}")); }
            continue;
        };
        if f.t % 60 == 0 { k::trace("serpent rift"); }
        let spin = t * 3.0 * if i == 0 { 1.0 } else { -1.0 };
        k::put(c, &format!("sfx_rim_{i}_0"), &rim_img(), (x, y), (r * 2.5, r * 2.5), spin, [255, 255, 255, 230]);
        k::put(c, &format!("sfx_rim_{i}_1"), &rim_img(), (x, y), (r * 3.2, r * 3.2), -spin * 0.6 + 40.0, [190, 150, 255, 140]);
        if !frozen && f.t % 2 == 0 {
            let a = k::rnd(rng) * tau;
            let rr = r * (1.8 + 0.8 * k::rnd(rng));
            let (px, py) = (x + a.cos() * rr, y + a.sin() * rr);
            // Inward and round: a short streak tangent to the swirl.
            let sw = if i == 0 { 1.0 } else { -1.0 };
            let (vx, vy) = (-a.cos() * 22.0 - a.sin() * 22.0 * sw, -a.sin() * 22.0 + a.cos() * 22.0 * sw);
            f.sparks.emit(P::at(px, py).vel(vx, vy).life(rr / 32.0).size(150.0, 12.0).shape(k::SPARK).align()
                .rgb([200, 180, 255], [255, 255, 255]).alpha(0.9));
        }
    }

    // ── An eruption: the head bursting out of the rift ahead ──
    if !frozen {
        if phase == RiftPhase::Emerge && f.last_phase != Some(RiftPhase::Emerge) {
            if let Some(at) = next.or(hole) { erupt(f, at, rng); }
        }
        f.last_phase = Some(phase);
    }

    // ── The launched tail: a violet plasma trail ──
    if tail_out && !frozen {
        if f.t % 20 == 0 { k::trace("serpent tail out"); }
        for _ in 0..2 {
            let s = 60.0 + 70.0 * k::rnd(rng);
            f.sparks.emit(P::at(tail_pos.0 + k::rnds(rng) * 40.0, tail_pos.1 + k::rnds(rng) * 40.0)
                .vel(k::rnds(rng) * 2.0, k::rnds(rng) * 2.0).life(26.0 + 10.0 * k::rnd(rng)).size(s, s).grow(-0.5)
                .shape(k::DOT).rgb([220, 190, 255], [80, 20, 180]).fade_in(0.01));
        }
    }

    if !frozen { f.ghosts.step(); f.sparks.step(); }
    f.rng = *rng;
    f.ghosts.draw(c); f.sparks.draw(c);
}
