//! Every boss's sounds but the Colossus's (which has its own, finer-grained
//! `colossus_sfx`) and the Conductor's (whose fight is its music). Generated
//! by `tools/make_boss_sfx.py`.
//!
//! Driven from the fight's STATE, not hooked into each fight: once a frame
//! this compares what it saw last frame — each part's stage, hp and life, the
//! Serpent's act, the Sun Devourer's live shots and generators — and plays a
//! sound on the frame something begins. One tracker for five fights means no
//! fight's attack code had to learn about audio.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::{BossKind, PartState, SerpentAct};
use crate::state::State;

macro_rules! wav { ($n:literal) => { include_bytes!(concat!("../../../../assets/", $n, ".wav")) as &'static [u8] }; }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cue { Intro, Windup, Attack, Hit, Break, Shot, Act(SerpentAct) }

fn sound(kind: BossKind, cue: Cue) -> Option<(&'static [u8], f32)> {
    use BossKind::*;
    Some(match (kind, cue) {
        (GravityWeaver, Cue::Intro) => (wav!("weaver_intro"), 0.6),
        (GravityWeaver, Cue::Windup) => (wav!("weaver_windup"), 0.45),
        (GravityWeaver, Cue::Attack) => (wav!("weaver_attack"), 0.6),
        (GravityWeaver, Cue::Hit) => (wav!("weaver_hit"), 0.55),
        (GravityWeaver, Cue::Break) => (wav!("weaver_break"), 0.85),
        (FlareTitan, Cue::Intro) => (wav!("titan_intro"), 0.6),
        (FlareTitan, Cue::Windup) => (wav!("titan_windup"), 0.45),
        (FlareTitan, Cue::Attack) => (wav!("titan_attack"), 0.65),
        (FlareTitan, Cue::Hit) => (wav!("titan_hit"), 0.55),
        (FlareTitan, Cue::Break) => (wav!("titan_break"), 0.85),
        (Magnetar, Cue::Intro) => (wav!("magnetar_intro"), 0.55),
        (Magnetar, Cue::Windup) => (wav!("magnetar_windup"), 0.45),
        (Magnetar, Cue::Attack) => (wav!("magnetar_attack"), 0.6),
        (Magnetar, Cue::Hit) => (wav!("magnetar_hit"), 0.55),
        (Magnetar, Cue::Break) => (wav!("magnetar_break"), 0.85),
        (Serpent, Cue::Intro) => (wav!("serpent_intro"), 0.6),
        (Serpent, Cue::Hit) => (wav!("serpent_hit"), 0.55),
        (Serpent, Cue::Break) => (wav!("serpent_break"), 0.85),
        (Serpent, Cue::Act(a)) => match a {
            SerpentAct::Prowl => return None,
            SerpentAct::Coil => (wav!("serpent_coil"), 0.5),
            SerpentAct::TailSweep => (wav!("serpent_sweep"), 0.6),
            SerpentAct::RiftStrikes => (wav!("serpent_rift"), 0.6),
            SerpentAct::TailLaunch => (wav!("serpent_launch"), 0.6),
            SerpentAct::SpineLash => (wav!("serpent_lash"), 0.6),
            SerpentAct::WormholeGambit => (wav!("serpent_wormhole"), 0.6),
        },
        (SunDevourer, Cue::Intro) => (wav!("devourer_intro"), 0.65),
        (SunDevourer, Cue::Shot) => (wav!("devourer_shot"), 0.3),
        (SunDevourer, Cue::Hit) => (wav!("devourer_hit"), 0.6),
        (SunDevourer, Cue::Break) => (wav!("devourer_break"), 0.85),
        (SunDevourer, Cue::Windup) => (wav!("devourer_windup"), 0.5),
        _ => return None,
    })
}

/// What last frame looked like.
#[derive(Default)]
struct Seen {
    active: bool,
    kind: Option<BossKind>,
    parts: Vec<(PartState, i32, bool)>,
    act: Option<SerpentAct>,
    bolts: usize,
    hp: i32,
    gens: Vec<i32>,
    shot_cool: u32,
    /// The Magnetar's clock, its beams' cycle, its pulses so far.
    mag: (u8, u8, u32),
}

static SEEN: Mutex<Option<Seen>> = Mutex::new(None);

fn play(c: &mut Canvas, kind: BossKind, cue: Cue) {
    let Some((bytes, vol)) = sound(kind, cue) else { return };
    // Headless check: `BOSS_SFX_LOG=1` prints each sound as it plays.
    if std::env::var("BOSS_SFX_LOG").is_ok() {
        eprintln!("BOSS_SFX {kind:?} {cue:?}");
    }
    let v = crate::scenes::game::helpers::sfx_vol(c, vol);
    c.play_sound_bytes_with(bytes, SoundOptions::new().volume(v));
}

pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let now = {
        let s = st.lock().unwrap();
        Seen {
            active: s.boss_active,
            kind: Some(s.boss_kind),
            parts: s.boss_parts.iter().map(|p| (p.state, p.hp, p.alive)).collect(),
            act: Some(s.serpent_act),
            bolts: s.boss_bolt_live.len(),
            hp: s.boss_hp,
            gens: s.boss_generator_hp.clone(),
            shot_cool: 0,
            mag: (s.magnetar_clock, s.magnetar_beam, s.magnetar_pulses),
        }
    };
    let mut guard = SEEN.lock().unwrap();
    let last = guard.take().unwrap_or_default();
    let kind = now.kind.unwrap();
    let skip = matches!(kind, BossKind::Colossus | BossKind::Conductor);
    // A new fight (or a different boss) starts the comparison afresh, so the
    // first frame never reads as every part changing at once.
    let fresh = !last.active || last.kind != now.kind || last.parts.len() != now.parts.len();
    let mut shot_cool = last.shot_cool.saturating_sub(1);
    if now.active && !skip {
        let mut cues: Vec<Cue> = Vec::new();
        if fresh {
            cues.push(Cue::Intro);
        } else {
            let (mut wind, mut atk, mut hit, mut brk) = (false, false, false, false);
            for (a, b) in last.parts.iter().zip(&now.parts) {
                if a.2 && !b.2 { brk = true; continue; }
                if !b.2 { continue; }
                if b.1 < a.1 { hit = true; }
                if b.0 != a.0 {
                    match b.0 {
                        PartState::Telegraph => wind = true,
                        PartState::Attack => atk = true,
                        _ => {}
                    }
                }
            }
            if kind == BossKind::Serpent && now.act != last.act {
                cues.push(Cue::Act(now.act.unwrap()));
            }
            // The Magnetar keeps its own clock rather than part stages: beams
            // charging (1) and live (2), the pulse count-in, each pulse, the
            // starquake (clock 3).
            if kind == BossKind::Magnetar {
                let (c0, b0, p0) = last.mag;
                let (c1, b1, p1) = now.mag;
                if (b1 == 1 && b0 != 1) || (c1 == 1 && c0 != 1) { wind = true; }
                if (b1 == 2 && b0 != 2) || p1 > p0 || (c1 == 3 && c0 != 3) { atk = true; }
            }
            // The Sun Devourer: a body, not parts.
            if kind == BossKind::SunDevourer {
                if now.hp < last.hp { hit = true; }
                if now.gens.len() == last.gens.len()
                    && last.gens.iter().zip(&now.gens).any(|(a, b)| *a > 0 && *b <= 0) {
                    brk = true;
                } else if now.gens.len() == last.gens.len()
                    && last.gens.iter().zip(&now.gens).any(|(a, b)| b < a) {
                    hit = true;
                }
                // Shots come in volleys: one sound per volley, not per bolt.
                if now.bolts > last.bolts && shot_cool == 0 {
                    cues.push(Cue::Shot);
                    shot_cool = 8;
                }
            }
            // One of each per frame, the biggest first.
            if brk { cues.push(Cue::Break) } else if hit { cues.push(Cue::Hit) }
            if atk { cues.push(Cue::Attack) } else if wind { cues.push(Cue::Windup) }
        }
        for cue in cues { play(c, kind, cue); }
    }
    *guard = Some(Seen { shot_cool, ..now });
}
