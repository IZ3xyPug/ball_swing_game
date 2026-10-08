//! phase_cine.rs — a boss's phase change, played as a moment.
//!
//! When a fight turns (a shield drops, a core is exposed, the tempo doubles),
//! the player is held where they are, the camera pulls back to frame the boss,
//! and after a beat hands back — the player carrying on with the velocity they
//! had. Every boss plays it its own way (`CineStyle`): the Weaver's slow pull
//! under a rumble, the Colossus's stomps, the Conductor stepping out on the
//! beat, and so on. A fight's later phase changes hit harder (`step`).
//!
//! The player cannot be hurt while it plays: they are held still on camera,
//! and an attack already in flight landing on them then would be a cheap hit.
//!
//! The fight stands still too: `tick_boss` skips the boss's own tick, and
//! everything the engine moves by itself (bolts, meteors — anything with
//! momentum or gravity) is held here and handed its motion back on release.
//! That still frame is where each boss's own phase-change animation will go.
//!
//! The styles differ in more than timing — each has its own camera move and
//! its own staging, built from a small kit: letterbox bars, a full-screen
//! tint, a gravity lens over the boss, and shockwave rings/bursts (the shared
//! impact pool). See `tick_phase_cine` for each one.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

/// One style's shape: how long it holds, the zoom it reaches, and how many
/// ticks the camera takes to get there.
struct Shape {
    ticks: u32,
    zoom: f32,
    ease: u32,
}

fn shape(style: CineStyle, step: u32) -> Shape {
    let step = step.max(1) as f32;
    match style {
        CineStyle::Loom => Shape { ticks: WEAVER_REVEAL_TICKS, zoom: WEAVER_REVEAL_ZOOM, ease: WEAVER_REVEAL_EASE_TICKS },
        // Each phase of the Colossus is bigger: longer, wider, more stomps.
        CineStyle::Quake => Shape { ticks: 90 + 30 * step as u32, zoom: (0.66 - 0.06 * step).max(0.5), ease: 12 },
        CineStyle::Crescendo => Shape { ticks: CRESCENDO_BEAT * 4 + 20, zoom: 0.55, ease: CRESCENDO_BEAT },
        CineStyle::Flare => Shape { ticks: 150, zoom: 0.55, ease: 60 },
        CineStyle::Magnet => Shape { ticks: 130, zoom: 0.58, ease: 30 },
        // The serpent's head coming out is the bigger one.
        CineStyle::Coil => Shape { ticks: 120 + 30 * step as u32, zoom: (0.56 - 0.05 * step).max(0.46), ease: 28 },
        CineStyle::Eclipse => Shape { ticks: 170, zoom: 0.45, ease: 50 },
    }
}

/// The Conductor's beat for its crescendo, ticks (100 BPM at 60 fps).
const CRESCENDO_BEAT: u32 = 36;

/// Start a phase change. Does nothing if one is already playing.
pub(crate) fn start_phase_cine(s: &mut State, style: CineStyle, step: u32) {
    if s.phase_cine.ticks > 0 { return; }
    let sh = shape(style, step);
    s.phase_cine = PhaseCine {
        style, ticks: sh.ticks, total: sh.ticks, step: step.max(1),
        pos: (s.px, s.py), vel: (s.vx, s.vy),
    };
}

/// Whether one is playing (the player is held and cannot be hurt).
pub(crate) fn phase_cine_active(s: &State) -> bool { s.phase_cine.ticks > 0 }

/// The style each boss uses.
pub(crate) fn cine_style_for(kind: BossKind) -> CineStyle {
    match kind {
        BossKind::GravityWeaver => CineStyle::Loom,
        BossKind::Colossus => CineStyle::Quake,
        BossKind::Conductor => CineStyle::Crescendo,
        BossKind::FlareTitan => CineStyle::Flare,
        BossKind::Magnetar => CineStyle::Magnet,
        BossKind::Serpent => CineStyle::Coil,
        BossKind::SunDevourer => CineStyle::Eclipse,
    }
}

/// Dev key: play the current boss's phase change now (step 1).
pub(crate) fn debug_phase_cine(st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.boss_active && s.boss_spawned {
        let style = cine_style_for(s.boss_kind);
        s.phase_cine.ticks = 0;
        start_phase_cine(&mut s, style, 1);
        log::info!("debug: {style:?} phase change");
    } else {
        log::info!("debug: a phase change needs a live boss fight");
    }
}

/// What the engine was moving when the phase change began: (object,
/// momentum, gravity), zeroed while it plays and handed back after.
static HELD: Mutex<Vec<(String, (f32, f32), f32)>> = Mutex::new(Vec::new());

/// Stop everything the engine moves on its own (not the player, who is held
/// separately). Re-zeroes the held ones every frame, in case anything else
/// pushes them.
fn hold_world(c: &mut Canvas, first: bool) {
    let mut held = HELD.lock().unwrap();
    if first {
        held.clear();
        for name in c.object_names().to_vec() {
            if name == "player" { continue; }
            if let Some(o) = c.get_game_object(&name) {
                if o.momentum != (0.0, 0.0) || o.gravity != 0.0 {
                    held.push((name.clone(), o.momentum, o.gravity));
                }
            }
        }
    }
    for (name, _, _) in held.iter() {
        if let Some(o) = c.get_game_object_mut(name) {
            o.momentum = (0.0, 0.0);
            o.gravity = 0.0;
        }
    }
}

/// Hand back what `hold_world` took.
fn release_world(c: &mut Canvas) {
    let held: Vec<_> = std::mem::take(&mut *HELD.lock().unwrap());
    for (name, m, g) in held {
        if let Some(o) = c.get_game_object_mut(&name) {
            o.momentum = m;
            o.gravity = g;
        }
    }
}

fn smoothstep(t: f32) -> f32 { let t = t.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

fn flash(c: &mut Canvas, rgb: (u8, u8, u8), alpha: u8, secs: f32, intensity: f32) {
    if let Some(cam) = c.camera_mut() {
        cam.flash_with(Color(rgb.0, rgb.1, rgb.2, alpha), secs, FlashMode::Pulse, FlashEase::Sharp, intensity, 0.0);
    }
}

fn shake(c: &mut Canvas, amount: f32, tail: f32) {
    if let Some(cam) = c.camera_mut() { cam.shake(amount, tail); }
}

fn punch(c: &mut Canvas, amount: f32, secs: f32) {
    if let Some(cam) = c.camera_mut() { cam.zoom_punch(amount, secs); }
}

// ── Staging: bars, tint, lens ────────────────────────────────────────────────

const BAR_TOP: &str = "cine_bar_top";
const BAR_BOT: &str = "cine_bar_bot";
const TINT: &str = "cine_tint";
const LENS: &str = "cine_lens";
/// How far the letterbox bars come in, at full.
const BAR_H: f32 = VH * 0.11;

/// Create the staging objects the first time they are needed (they live in
/// the game scene from then on, hidden between phase changes).
fn ensure_stage(c: &mut Canvas) {
    if c.get_game_object(TINT).is_some() { return; }
    // Screen-space and overdrawn half a screen past every edge, so a window
    // of another aspect or a heavy shake never shows an uncovered strip.
    let screen = |name: &str, layer: i32| {
        let mut o = GameObject::build(name)
            .position(-VW * 0.5, -VH * 0.5)
            .size(VW * 2.0, VH * 2.0)
            .image(tint_overlay(VW * 2.0, VH * 2.0, Color(0, 0, 0, 0)))
            .layer(layer)
            .ignore_zoom()
            .finish();
        o.visible = false;
        o.unlit = true;
        o
    };
    // Under the engine's flash (i32::MAX), over the HUD.
    c.add_game_object(TINT.into(), screen(TINT, i32::MAX - 2));
    c.add_game_object(BAR_TOP.into(), screen(BAR_TOP, i32::MAX - 1));
    c.add_game_object(BAR_BOT.into(), screen(BAR_BOT, i32::MAX - 1));
    // Built like the Weaver's own lens: an effect needs a (clear) quad to
    // draw on, and it sits just under the nodes.
    let mut lens = GameObject::build(LENS)
        .position(-9000.0, -9000.0)
        .size(8.0, 8.0)
        .image(tint_overlay(8.0, 8.0, Color(0, 0, 0, 0)))
        .layer(19) // bootstrap.rs: LAYER_SPACE_HOOK - 2, the Weaver lens's layer
        .finish();
    lens.visible = false;
    c.add_game_object(LENS.into(), lens);
}

/// Letterbox bars in by `amt` (0 = gone).
fn set_bars(c: &mut Canvas, amt: f32) {
    let h = BAR_H * amt.clamp(0.0, 1.0);
    let on = h > 0.5;
    if let Some(o) = c.get_game_object_mut(BAR_TOP) {
        o.visible = on;
        o.position = (-VW * 0.5, -VH);
        o.size = (VW * 2.0, VH + h);
        o.set_image(tint_overlay(VW * 2.0, VH + h, Color(0, 0, 0, 255)));
    }
    if let Some(o) = c.get_game_object_mut(BAR_BOT) {
        o.visible = on;
        o.position = (-VW * 0.5, VH - h);
        o.size = (VW * 2.0, VH + h);
        o.set_image(tint_overlay(VW * 2.0, VH + h, Color(0, 0, 0, 255)));
    }
}

/// The whole screen washed with `col` (alpha 0 hides it).
fn set_tint(c: &mut Canvas, col: Color) {
    if let Some(o) = c.get_game_object_mut(TINT) {
        o.visible = col.3 > 0;
        o.set_image(tint_overlay(VW * 2.0, VH * 2.0, col));
    }
}

/// A gravity lens `d` across over `at`, of `strength` (0 hides it).
fn set_lens(c: &mut Canvas, at: (f32, f32), d: f32, strength: f32, rgb: (u8, u8, u8)) {
    if strength <= 0.0 {
        c.clear_effect(LENS);
        if let Some(o) = c.get_game_object_mut(LENS) { o.visible = false; }
        return;
    }
    if let Some(o) = c.get_game_object_mut(LENS) {
        o.visible = true;
        if o.size != (d, d) { o.set_image(tint_overlay(d, d, Color(0, 0, 0, 0))); }
        o.size = (d, d);
        o.position = (at.0 - d * 0.5, at.1 - d * 0.5);
    }
    c.attach_effect(LENS, Effect::GravityLens { strength }, EffectColor::srgb8(rgb.0, rgb.1, rgb.2), (d, d));
}

/// Take all the staging down.
fn end_stage(c: &mut Canvas) {
    if c.get_game_object(TINT).is_none() { return; }
    set_bars(c, 0.0);
    set_tint(c, Color(0, 0, 0, 0));
    set_lens(c, (0.0, 0.0), 8.0, 0.0, (0, 0, 0));
}

fn ring(c: &mut Canvas, st: &Arc<Mutex<State>>, at: (f32, f32), d: f32, rgb: (f32, f32, f32), pulse: bool) {
    super::common::spawn_fx_ring(c, st, at, d, rgb, pulse);
}

/// Centre of a game object, if it exists.
fn centre(c: &Canvas, name: &str) -> Option<(f32, f32)> {
    c.get_game_object(name).map(|o| (o.position.0 + o.size.0 * 0.5, o.position.1 + o.size.1 * 0.5))
}

/// Play the phase change, if one is on: hold the player, drive the camera's
/// boss focus (`boss_cam_focus*`, read by `tick_zoom`) and the style's own
/// staging. Every boss frame, after the boss's own tick.
pub(crate) fn tick_phase_cine(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (cine, alive) = {
        let mut s = st.lock().unwrap();
        // A fall, a restart or the fight ending cancels it.
        if s.dead || !s.boss_active || !s.boss_spawned || s.boss_stasis_active {
            s.phase_cine.ticks = 0;
        }
        (s.phase_cine, s.boss_hp > 0)
    };
    if cine.ticks == 0 || !alive {
        if cine.ticks > 0 { st.lock().unwrap().phase_cine.ticks = 0; }
        // Cancelled or finished: whatever was held moves again, the staging
        // comes down.
        release_world(c);
        end_stage(c);
        c.set_var("boss_cam_focus", 0.0f32);
        return;
    }
    ensure_stage(c);
    let sh = shape(cine.style, cine.step);
    let left = cine.ticks - 1;
    let el = cine.total - left; // ticks elapsed, 1..=total
    let released = left == 0;
    if released { release_world(c); } else { hold_world(c, el == 1); }
    {
        let mut s = st.lock().unwrap();
        s.phase_cine.ticks = left;
        let (hx, hy) = cine.pos;
        s.px = hx;
        s.py = hy;
        // Held still, not stopped: the last frame hands the velocity back.
        (s.vx, s.vy) = if released { cine.vel } else { (0.0, 0.0) };
    }
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.position = (cine.pos.0 - PLAYER_R, cine.pos.1 - PLAYER_R);
        obj.momentum = if released { cine.vel } else { (0.0, 0.0) };
    }

    let focus_obj = if cine.style == CineStyle::Crescendo { "conductor_core" } else { "boss" };
    let boss = centre(c, focus_obj).unwrap_or((cine.pos.0, BOSS_Y_CENTER));
    let (mut fx, mut fy) = boss;
    let mut zoom = sh.zoom;
    let mut amt = smoothstep(el as f32 / sh.ease.max(1) as f32);
    let t = el as f32;
    let total = cine.total as f32;
    let p = t / total; // progress 0..1
    let k = cine.step.max(1) as f32;
    // Bars ease in over the first 20 ticks and out over the last 20.
    let bars_full = smoothstep(t / 20.0) * smoothstep(left as f32 / 20.0);
    let mut bars = 0.0;

    match cine.style {
        // GRAVITY WEAVER — space bends. Letterboxed; the view rises slowly
        // as a lens over the loom deepens and the rumble builds, then at
        // three quarters the world DROPS (a punch, a spike, a violet flash)
        // — the inversion, felt before it is seen.
        CineStyle::Loom => {
            bars = bars_full;
            let built = (t / WEAVER_REVEAL_SHAKE_RAMP as f32).min(1.0);
            shake(c, WEAVER_REVEAL_SHAKE * built, WEAVER_REVEAL_SHAKE_TAIL);
            let drop_at = (total * 0.72) as u32;
            if el < drop_at {
                fy -= 320.0 * smoothstep(p / 0.72);
            } else {
                // After the drop the view hangs a little BELOW, inverted.
                fy += 180.0 * smoothstep((t - drop_at as f32) / 10.0);
            }
            if el == drop_at {
                punch(c, 0.10, 0.25);
                shake(c, 160.0, 0.4);
                flash(c, WEAVER_SRGB, 170, 0.35, 1.0);
                ring(c, st, boss, 3200.0, (0.65, 0.35, 1.0), false);
            }
            if el % 30 == 1 && el < drop_at { ring(c, st, boss, 1600.0 + 40.0 * t, (0.6, 0.35, 1.0), true); }
            let lens = if released { 0.0 } else if el < drop_at { 0.2 + 0.8 * (p / 0.72) } else { 1.0 - (t - drop_at as f32) / (total - drop_at as f32) };
            set_lens(c, boss, 2800.0, lens, WEAVER_LENS_SRGB);
        }

        // COLOSSUS — weight. A hard snap out, then stomps: each one lurches
        // the camera down and rings out a burst, bigger every phase, and
        // the last is the biggest.
        CineStyle::Quake => {
            let first = 14u32;
            let gap = 28u32;
            let last = first + gap * ((cine.total - first - 24) / gap);
            let since = if el >= first { (el - first) % gap } else { gap };
            let lurch = if el >= first { (1.0 - since as f32 / 14.0).max(0.0) } else { 0.0 };
            fy += 170.0 * lurch * lurch;
            if el >= first && since == 0 && el <= last {
                let big = el == last;
                shake(c, if big { 170.0 } else { 70.0 + 30.0 * k }, 0.35);
                punch(c, if big { 0.12 } else { 0.05 + 0.02 * k }, 0.2);
                flash(c, if big { (255, 235, 210) } else { (255, 150, 70) }, if big { 160 } else { 70 }, 0.25, 0.6);
                ring(c, st, (boss.0, boss.1 + 400.0), if big { 3600.0 } else { 1500.0 + 350.0 * k }, (1.0, 0.55, 0.2), false);
            }
        }

        // CONDUCTOR — tempo. Four beats; on each the camera steps further
        // out and swings to the other side, a sonic ring wider each time;
        // the fourth lands with the letterbox snapping shut and a white hit.
        CineStyle::Crescendo => {
            let beat = (el / CRESCENDO_BEAT).min(4);
            let within = (el % CRESCENDO_BEAT) as f32 / 8.0;
            amt = ((beat as f32 + smoothstep(within).min(1.0)) / 4.0).min(1.0);
            let side = if beat % 2 == 0 { -1.0 } else { 1.0 };
            fx += side * 300.0 * smoothstep(within) * if beat >= 4 { 0.0 } else { 1.0 };
            if el % CRESCENDO_BEAT == 1 && beat < 4 {
                let last = beat == 3;
                flash(c, if last { (255, 255, 255) } else { (180, 140, 255) }, if last { 190 } else { 90 }, 0.3, if last { 1.0 } else { 0.5 });
                shake(c, if last { 90.0 } else { 30.0 }, 0.2);
                punch(c, 0.04 + 0.02 * beat as f32, 0.15);
                ring(c, st, boss, 1000.0 + 700.0 * beat as f32, (0.35, 0.9, 1.0), true);
                if last { ring(c, st, boss, 3400.0, (0.9, 0.85, 1.0), false); }
            }
            if beat >= 3 { bars = smoothstep((el - 3 * CRESCENDO_BEAT) as f32 / 6.0) * smoothstep(left as f32 / 20.0); }
        }

        // FLARE TITAN — heat. Letterboxed; the screen warms toward orange,
        // a shimmer builds, flares burst from the core faster and wider,
        // and it whites out on the release.
        CineStyle::Flare => {
            bars = bars_full;
            shake(c, 14.0 + 34.0 * p, 0.1);
            let heat = (p * 1.15).min(1.0);
            set_tint(c, if released { Color(0, 0, 0, 0) } else { Color(255, 140, 40, (100.0 * heat) as u8) });
            let period = (36.0 - 28.0 * p).max(8.0) as u32;
            if el % period == 1 {
                ring(c, st, boss, 1200.0 + 2200.0 * p, (1.0, 0.7, 0.25), false);
            }
            zoom += 0.04 * (t * 0.5).sin() * p; // the shimmer, in the lens
            if released {
                flash(c, (255, 250, 235), 240, 0.7, 1.0);
                ring(c, st, boss, 4200.0, (1.0, 0.95, 0.8), false);
            }
        }

        // MAGNETAR — the field. The view is dragged round the core while
        // the zoom breathes, a lens over it pulsing, a faint blue cast;
        // on the release the lens snaps shut.
        CineStyle::Magnet => {
            let a = t * 0.07;
            let r = 280.0 * amt;
            fx += a.cos() * r;
            fy += a.sin() * r * 0.7;
            zoom += 0.06 * (t * 0.12).sin();
            shake(c, 22.0, 0.1);
            set_tint(c, if released { Color(0, 0, 0, 0) } else { Color(60, 120, 255, (45.0 * amt) as u8) });
            let lens = if released { 0.0 } else { (0.35 + 0.65 * (t * 0.15).sin().abs()) * amt };
            set_lens(c, boss, 2600.0, lens, (110, 180, 255));
            if el % 32 == 1 { ring(c, st, boss, 2200.0, (0.4, 0.7, 1.0), true); }
            if released { flash(c, (170, 210, 255), 150, 0.35, 0.8); }
        }

        // SERPENT — length. A figure-eight sweep along the coils while
        // pulses run down the body, head to tail, over and over; the head's
        // phase change is letterboxed.
        CineStyle::Coil => {
            if cine.step >= 2 { bars = bars_full; }
            fx += (t * 0.035).sin() * 900.0 * amt;
            fy += (t * 0.07).sin() * 260.0 * amt;
            shake(c, 18.0 + 14.0 * k, 0.15);
            set_tint(c, if released { Color(0, 0, 0, 0) } else { Color(60, 255, 140, (28.0 * amt) as u8) });
            if el % 7 == 1 {
                let segs = SERPENT_SEGMENTS + 2;
                let i = (el / 7) as usize % segs;
                // Head first: the last part index is the head.
                let name = format!("serpent_part_{}", segs - 1 - i);
                if let Some(at) = centre(c, &name) {
                    ring(c, st, at, 900.0, (0.4, 1.0, 0.6), true);
                }
            }
            if el == 1 { flash(c, (120, 230, 160), 90, 0.4, 0.6); }
        }

        // SUN DEVOURER — the eclipse. The widest pull; the screen goes dark
        // as red rings pulse wider and a quake grows; then the camera
        // rushes back IN and it breaks in a white burst.
        CineStyle::Eclipse => {
            bars = bars_full;
            let rush = 26.0;
            let rushing = (left as f32) < rush;
            shake(c, 120.0 * p, 0.3);
            let dark = smoothstep(p / 0.75) * if rushing { left as f32 / rush } else { 1.0 };
            set_tint(c, Color(8, 0, 4, (175.0 * dark) as u8));
            if el % 40 == 1 && !rushing {
                ring(c, st, boss, 1400.0 + 2600.0 * p, (1.0, 0.25, 0.15), true);
            }
            if rushing { zoom += 0.35 * smoothstep(1.0 - left as f32 / rush); }
            if released {
                flash(c, (255, 245, 230), 230, 0.7, 1.0);
                ring(c, st, boss, 4600.0, (1.0, 0.85, 0.6), false);
            }
        }
    }

    set_bars(c, bars);
    c.set_var("boss_cam_focus", if released { 0.0 } else { amt });
    c.set_var("boss_cam_focus_x", fx);
    c.set_var("boss_cam_focus_y", fy);
    c.set_var("boss_cam_focus_zoom", zoom);
    if released { end_stage(c); }
}
