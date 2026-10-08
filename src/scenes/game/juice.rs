//! Juice: small feedback that makes the swing feel physical. None of it
//! changes the rules.
//!
//!   * HITSTOP: the world freezes for a few frames when a hit lands — a boss
//!     part struck, a heart lost — so the impact has weight.
//!   * GRAB KICK: a camera kick on every grab, scaled by how fast you were
//!     going, so speed is felt as well as seen.
//!   * RELEASE WHOOSH: a whoosh off a fast release, louder the faster.
//!   * COMBO PITCH: grabs chained quickly play a tick that climbs in pitch,
//!     so a long chain sounds like it is building; coins picked up in a
//!     quick run climb the same way.
//!   * NEAR MISSES: a whoosh when you skim past a spinner's bar or a comet
//!     without being hit.
//!
//! Every effect respects the sound volume settings and the graphics quality
//! (`graphics.rs`: shakes are lighter on LOW).

use quartz::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;
use super::helpers::sfx_vol;

/// Frames since the run started (counted by `tick`).
static FRAME: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct Juice {
    chain: u32,
    best_chain: u32,
    last_release: u64,
    coin_run: u32,
    last_coin: u64,
    /// The spinner or comet last skimmed, and when, so one pass whooshes once.
    near_id: String,
    near_at: u64,
}

fn juice() -> &'static Mutex<Juice> {
    static J: std::sync::OnceLock<Mutex<Juice>> = std::sync::OnceLock::new();
    J.get_or_init(|| Mutex::new(Juice::default()))
}

fn now() -> u64 { FRAME.load(Ordering::Relaxed) }

/// A run is starting.
pub fn reset() {
    *juice().lock().unwrap() = Juice::default();
}

// ── Hitstop ──────────────────────────────────────────────────────────────────

const STOP_VAR: &str = "juice_hitstop";

/// Freeze the world for `frames`. Never over a pause menu, a transition or
/// another freeze already running (they do not stack).
pub fn hitstop(c: &mut Canvas, frames: i32) {
    register(c);
    // `NO_HITSTOP=1`: off (headless A/B runs).
    if std::env::var("NO_HITSTOP").is_ok() || !crate::graphics::fx_on(crate::graphics::FX_HITSTOP) { return; }
    if c.is_paused() || c.get_bool("game_paused") || crate::transition::busy(c) { return; }
    if c.get_i32(STOP_VAR) > 0 { return; }
    c.set_var(STOP_VAR, frames);
    c.pause();
}

fn register(c: &mut Canvas) {
    if c.get_bool("juice_registered") { return; }
    c.set_var("juice_registered", true);
    // Runs while paused: counts the freeze down and lets the world go.
    c.on_update_always(|c| {
        let n = c.get_i32(STOP_VAR);
        if n <= 0 { return; }
        c.set_var(STOP_VAR, n - 1);
        if n - 1 == 0 && !c.get_bool("game_paused") && !crate::transition::busy(c) && c.is_scene("game") {
            c.resume();
        }
    });
}

// ── Grab, release, coins ─────────────────────────────────────────────────────

fn shake(c: &mut Canvas, intensity: f32, secs: f32) {
    let k = crate::graphics::shake_scale(c);
    if k <= 0.0 { return; }
    if let Some(cam) = c.camera_mut() { cam.shake(intensity * k, secs); }
}

/// The player grabbed a node at `speed` (px/tick).
pub fn on_grab(c: &mut Canvas, speed: f32) {
    // A kick scaled by speed: nothing at a crawl, a real jolt at the cap.
    let k = ((speed - 18.0) / (MOMENTUM_CAP - 18.0)).clamp(0.0, 1.0);
    if k > 0.0 { shake(c, 3.0 + 9.0 * k, 0.10 + 0.06 * k); }
    // Chained grabs (within ~1.2 s of the last release) climb in pitch.
    let chain = {
        let mut j = juice().lock().unwrap();
        j.chain = if now().saturating_sub(j.last_release) <= 72 { j.chain + 1 } else { 0 };
        j.best_chain = j.best_chain.max(j.chain + 1);
        j.chain
    };
    if chain >= 2 {
        let pitch = 1.0 + 0.06 * (chain.min(14) as f32 - 2.0);
        c.play_sound_bytes_with(ASSET_COIN_SFX_3, SoundOptions::new()
            .volume(sfx_vol(c, 0.10 + 0.01 * chain.min(10) as f32)).pitch(pitch));
    }
}

/// The longest chain of quick grabs this run (for the run summary).
pub fn best_chain() -> u32 { juice().lock().unwrap().best_chain }

/// The player let go at `speed` (px/tick, after the release impulse).
pub fn on_release(c: &mut Canvas, speed: f32) {
    juice().lock().unwrap().last_release = now();
    let k = ((speed - 30.0) / (MOMENTUM_CAP - 30.0)).clamp(0.0, 1.0);
    if k > 0.0 {
        c.play_sound_bytes_with(ASSET_SWOOSH_SFX, SoundOptions::new()
            .volume(sfx_vol(c, 0.10 + 0.25 * k)).pitch(0.9 + 0.3 * k));
    }
}

/// Pitch for a coin pickup: coins in quick succession climb a scale.
pub fn coin_pitch() -> f32 {
    let mut j = juice().lock().unwrap();
    j.coin_run = if now().saturating_sub(j.last_coin) <= 30 { (j.coin_run + 1).min(12) } else { 0 };
    j.last_coin = now();
    // A major-ish scale, so the run sounds like notes rather than a slide.
    const STEPS: [f32; 13] = [1.0, 1.12, 1.26, 1.33, 1.5, 1.68, 1.89, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0];
    STEPS[j.coin_run as usize]
}

// ── Impacts ──────────────────────────────────────────────────────────────────

/// A hit landed: `taken` by the player, or dealt to a boss.
pub fn on_impact(c: &mut Canvas, taken: bool) {
    if taken {
        hitstop(c, 6);
        shake(c, 16.0, 0.22);
    } else {
        hitstop(c, 3);
        shake(c, 7.0, 0.12);
    }
}

// ── Near misses ──────────────────────────────────────────────────────────────

fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / (abx * abx + aby * aby).max(1e-3)).clamp(0.0, 1.0);
    let (cx, cy) = (a.0 + abx * t, a.1 + aby * t);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// Every game frame (alive, not paused).
pub fn tick(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    register(c);
    let f = FRAME.fetch_add(1, Ordering::Relaxed) + 1;
    let (px, py, spinners, dead) = {
        let s = st.lock().unwrap();
        (s.px, s.py, s.spinner_live.clone(), s.dead)
    };
    if dead { return; }
    let mut skim: Option<String> = None;
    // A spinner's bar: a rotating segment.
    for id in &spinners {
        let Some(o) = c.get_game_object(id) else { continue };
        if !o.visible { continue; }
        let (cx, cy) = (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5);
        if (cx - px).abs() > 900.0 || (cy - py).abs() > 900.0 { continue; }
        let r = o.rotation.to_radians();
        let half = o.size.0.max(o.size.1) * 0.5;
        let (dx, dy) = (r.cos() * half, r.sin() * half);
        let d = seg_dist((px, py), (cx - dx, cy - dy), (cx + dx, cy + dy)) - SPINNER_H * 0.5;
        if d > PLAYER_R + 8.0 && d < PLAYER_R + 95.0 { skim = Some(id.clone()); break; }
    }
    // A comet passing close.
    if skim.is_none() {
        if let Some(p) = c.get_game_object("player") {
            for o in c.objects_in_radius(p, 520.0) {
                if !o.visible || !o.tags.iter().any(|t| t == "comet") { continue; }
                let (cx, cy) = (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5);
                let d = ((cx - px).powi(2) + (cy - py).powi(2)).sqrt() - o.size.0.min(o.size.1) * 0.4;
                if d > PLAYER_R + 8.0 && d < PLAYER_R + 140.0 { skim = Some(o.id.clone()); break; }
            }
        }
    }
    if let Some(id) = skim {
        let fire = {
            let mut j = juice().lock().unwrap();
            let fresh = j.near_id != id || f.saturating_sub(j.near_at) > 90;
            if fresh { j.near_id = id; j.near_at = f; }
            fresh
        };
        if fire {
            c.play_sound_bytes_with(ASSET_SWOOSH_SFX, SoundOptions::new().volume(sfx_vol(c, 0.32)).pitch(1.35));
        }
    }
}
