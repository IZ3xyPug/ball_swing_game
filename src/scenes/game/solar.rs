// ── scenes/game/solar.rs — solar flares and shielded nodes ────────────────────
//
// A flare runs Idle → Warning (3 s) → Active (5 s) → Cooldown, and costs a
// heart every 2 s spent inside the active window without shelter.
//
// Shelter means TETHERED to a `shield_node`, not merely near one. That is the
// whole point of the mechanic: the counter-play is a swing decision made three
// seconds early, not a position you happen to be standing in. The previous
// implementation checked `s.live_hooks` — every hook in the world — so the tag
// conferred nothing and the mechanic did not exist at runtime.
//
// A flare will not begin its telegraph unless a live shielded node is within
// reach (see `shelter_in_range`). That guarantee is structural rather than a
// race: shielded nodes are placed on a fixed node-count cadence by the spawner,
// and the flare waits for one rather than firing into a stretch with no answer.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::difficulty::ramp;
use crate::state::*;
use super::fx;

// ── Phase helpers ────────────────────────────────────────────────────────────

/// What the flare system is doing right now, derived from `State`. Kept as a
/// view rather than a stored field so there is no second source of truth to
/// drift from the timers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlarePhase {
    Idle,
    Warning,
    Active,
}

pub fn flare_phase(s: &State) -> FlarePhase {
    if s.flare_warn > 0 {
        FlarePhase::Warning
    } else if s.flare_active {
        FlarePhase::Active
    } else {
        FlarePhase::Idle
    }
}

/// True when the player is currently sheltered: tethered to a shielded node.
pub fn player_is_sheltered(c: &Canvas, s: &State) -> bool {
    if !s.hooked || s.active_hook.is_empty() {
        return false;
    }
    c.get_game_object(&s.active_hook)
        .map(|o| o.tags.iter().any(|t| t == SHIELD_HOOK_TAG))
        .unwrap_or(false)
}

/// Whether any live shielded node sits close enough that the player could
/// realistically reach it inside the telegraph window.
fn shelter_in_range(c: &Canvas, s: &State) -> bool {
    s.live_hooks.iter().any(|id| {
        c.get_game_object(id)
            .map(|o| {
                if !o.tags.iter().any(|t| t == SHIELD_HOOK_TAG) {
                    return false;
                }
                let hx = o.position.0 + o.size.0 * 0.5;
                let dx = hx - s.px;
                dx <= FLARE_SHELTER_SEARCH_AHEAD && dx >= -FLARE_SHELTER_SEARCH_BEHIND
            })
            .unwrap_or(false)
    })
}

/// Flare interval in ticks at the player's current point on the difficulty
/// curve. Only the gap between flares scales — never the telegraph.
///
/// `debug_flare_interval` overrides it so the headless harness can exercise
/// many flares inside a short episode; at the shipped 90 s easy interval a test
/// run would see at most one.
fn flare_interval(c: &Canvas, distance: f32) -> u32 {
    if let Some(Value::I32(t)) = c.get_var("debug_flare_interval") {
        if t > 0 {
            return t as u32;
        }
    }
    ramp(distance, FLARE_INTERVAL_EASY, FLARE_INTERVAL_HARD).round().max(60.0) as u32
}

// ── Tick ─────────────────────────────────────────────────────────────────────

pub fn tick_solar(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Published every frame so the HUD (and the headless harness) can tell
    // sheltered from exposed without re-deriving the rule.
    let sheltered = {
        let s = st.lock().unwrap();
        player_is_sheltered(c, &s)
    };
    c.set_var("player_sheltered", sheltered);

    tick_flare_state(c, st);
    draw_shield_domes(c, st);
    draw_flare_overlay(c, st);
}

fn tick_flare_state(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    // Flares are a normal-zone hazard. Space has its own sun, and a boss arena
    // has no shielded nodes to reach.
    let (suspended, distance) = {
        let s = st.lock().unwrap();
        // Flares are the LAST hazard introduced (minute 44), so for most of a
        // run the whole system is dormant. Shielded nodes still spawn from the
        // start — the player should have seen them, and wondered about them,
        // long before the first flare explains what they are for.
        // The debug interval override also lifts the introduction gate, so the
        // harness can exercise flares without simulating 44 minutes of play.
        let forced = matches!(c.get_var("debug_flare_interval"), Some(Value::I32(t)) if t > 0);
        let too_early = !forced
            && !crate::hazards::hazard_active(s.distance, crate::hazards::Hazard::SolarFlare);
        (too_early || s.in_space_mode || s.space_launch_active || s.boss_active
            || s.god_mode || s.dead,
         s.distance)
    };
    if suspended {
        let mut s = st.lock().unwrap();
        s.flare_warn = 0;
        s.flare_active = false;
        s.flare_active_ticks = 0;
        s.flare_damage_timer = 0;
        drop(s);
        c.set_var("flare_warning", false);
        c.set_var("flare_active", false);
        return;
    }

    let phase = flare_phase(&st.lock().unwrap());

    match phase {
        FlarePhase::Warning => {
            let erupt = {
                let mut s = st.lock().unwrap();
                s.flare_warn = s.flare_warn.saturating_sub(1);
                s.flare_warn == 0
            };
            if erupt {
                let mut s = st.lock().unwrap();
                s.flare_active = true;
                s.flare_active_ticks = FLARE_ACTIVE_TICKS;
                // First damage tick lands one second in, not on the eruption
                // frame, so a correct late reaction still saves the heart.
                s.flare_damage_timer = FLARE_DAMAGE_GRACE;
                s.flare_wards_left = s.perm_flare_wards;
                drop(s);
                c.set_var("flare_active", true);
                c.set_var("flare_warning", false);
            } else {
                c.set_var("flare_warning", true);
            }
        }

        FlarePhase::Active => {
            // Damage is applied on a cadence across the window, so shelter
            // reached mid-flare genuinely saves the remaining ticks.
            let fire_damage = {
                let mut s = st.lock().unwrap();
                s.flare_active_ticks = s.flare_active_ticks.saturating_sub(1);
                s.flare_damage_timer = s.flare_damage_timer.saturating_sub(1);
                if s.flare_damage_timer == 0 {
                    s.flare_damage_timer = FLARE_DAMAGE_INTERVAL;
                    true
                } else {
                    false
                }
            };

            if fire_damage {
                let sheltered = {
                    let s = st.lock().unwrap();
                    player_is_sheltered(c, &s)
                };
                if sheltered {
                    let mut s = st.lock().unwrap();
                    s.flare_ticks_sheltered = s.flare_ticks_sheltered.saturating_add(1);
                    let n = s.flare_ticks_sheltered as i32;
                    drop(s);
                    c.set_var("flare_saves", Value::I32(n));
                } else {
                    // SUNPROOFING absorbs damage ticks before hearts are
                    // touched. Wards refill at the start of each flare, so the
                    // upgrade buys slack inside a flare rather than immunity
                    // across a run.
                    let warded = {
                        let mut s = st.lock().unwrap();
                        if s.flare_wards_left > 0 {
                            s.flare_wards_left -= 1;
                            true
                        } else {
                            false
                        }
                    };
                    if warded {
                        let n = { st.lock().unwrap().flare_wards_left as i32 };
                        c.set_var("flare_wards_left", Value::I32(n));
                    } else {
                        {
                            let mut s = st.lock().unwrap();
                            s.flare_hearts_lost = s.flare_hearts_lost.saturating_add(1);
                            let n = s.flare_hearts_lost as i32;
                            drop(s);
                            c.set_var("flare_hearts_lost", Value::I32(n));
                        }
                        super::hearts::lose_heart(c, st);
                    }
                }
            }

            let ended = {
                let s = st.lock().unwrap();
                s.flare_active_ticks == 0
            };
            if ended {
                let mut s = st.lock().unwrap();
                s.flare_active = false;
                s.flare_damage_timer = 0;
                s.flare_cooldown = flare_interval(c, distance);
                drop(s);
                c.set_var("flare_active", false);
            }
        }

        FlarePhase::Idle => {
            let interval = flare_interval(c, distance);
            let ready = {
                let mut s = st.lock().unwrap();
                // Clamp to the CURRENT interval before counting down. A cooldown
                // started under an easier interval should not outlive the point
                // where the curve has shortened it — and it lets the debug
                // override take effect on the first flare rather than the second.
                s.flare_cooldown = s.flare_cooldown.min(interval).saturating_sub(1);
                s.flare_cooldown == 0
            };
            if ready {
                let has_shelter = {
                    let s = st.lock().unwrap();
                    shelter_in_range(c, &s)
                };
                let mut s = st.lock().unwrap();
                if has_shelter {
                    s.flare_warn = FLARE_WARN_TICKS;
                    s.flares_fired = s.flares_fired.saturating_add(1);
                    let n = s.flares_fired as i32;
                    drop(s);
                    c.set_var("flare_warning", true);
                    c.set_var("flares_fired", Value::I32(n));
                } else {
                    // No answer available yet. Retry shortly rather than firing
                    // a flare the player provably cannot survive.
                    s.flare_cooldown = FLARE_NO_SHELTER_RETRY;
                }
            }
            c.set_var("flare_active", false);
        }
    }
}

// ── Visuals ──────────────────────────────────────────────────────────────────

/// Draw the protective dome on every live shielded node, and on the player
/// while sheltered.
///
/// The dome is always visible, not only during a flare — the player has to be
/// able to plan a route toward shelter two nodes ahead, which they cannot do if
/// the shelter only announces itself once the telegraph starts.
fn draw_shield_domes(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (phase, hooks, px, py, sheltered, ticks) = {
        let s = st.lock().unwrap();
        if s.in_space_mode || s.boss_active || s.dead {
            return;
        }
        (
            flare_phase(&s),
            s.live_hooks.clone(),
            s.px,
            s.py,
            player_is_sheltered(c, &s),
            s.ticks,
        )
    };

    // Domes brighten as the threat rises so the same object carries "shelter is
    // here" and "shelter is needed now" without a second visual language.
    let (base_a, pulse_a) = match phase {
        FlarePhase::Idle => (0.16, 0.05),
        FlarePhase::Warning => (0.42, 0.22),
        FlarePhase::Active => (0.70, 0.18),
    };
    let pulse = ((ticks as f32 * 0.11).sin() + 1.0) * 0.5;
    let alpha = base_a + pulse_a * pulse;

    let cam_x = px;
    for id in &hooks {
        let Some(o) = c.get_game_object(id) else { continue; };
        if !o.visible || !o.tags.iter().any(|t| t == SHIELD_HOOK_TAG) {
            continue;
        }
        let hx = o.position.0 + o.size.0 * 0.5;
        let hy = o.position.1 + o.size.1 * 0.5;
        // Cheap horizontal cull — a sprite far off-screen costs a draw for
        // nothing, and shielded nodes persist for a long stretch of world.
        if (hx - cam_x).abs() > VW {
            continue;
        }
        let d = FLARE_SHIELD_RADIUS * 2.0;
        fx::push_mega_fx(
            c,
            crate::images::shield_dome_img(),
            (hx, hy),
            (d, d),
            (1.0, 0.86, 0.42, alpha),
            0,
        );
    }

    // The player's own shield while tethered to shelter: a full bubble, drawn
    // by the dedicated energy-dome effect rather than the forward-facing
    // air-shield arc (which is a different shape and reads as speed, not
    // protection).
    if sheltered {
        let d = PLAYER_R * 2.0 * 2.6;
        fx::push_energy_dome_fx(
            c,
            (px, py),
            (d, d),
            (0.55, 0.85, 1.0, 0.85),
        );
    }
}

/// Full-screen wash for the telegraph and the flare itself.
fn draw_flare_overlay(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (phase, warn_left, active_left, ticks, sheltered) = {
        let s = st.lock().unwrap();
        (flare_phase(&s), s.flare_warn, s.flare_active_ticks, s.ticks,
         player_is_sheltered(c, &s))
    };

    let (tint, alpha) = match phase {
        FlarePhase::Idle => {
            set_overlay(c, 0.0, (0, 0, 0));
            set_flare_banner(c, None);
            return;
        }
        FlarePhase::Warning => {
            // Ramp in across the telegraph, so "it is coming" is legible from
            // the first frame and unmistakable by the last.
            let t = 1.0 - (warn_left as f32 / FLARE_WARN_TICKS as f32);
            let (r, g, b, a) = C_FLARE_WARN;
            ((r, g, b), (a as f32 / 255.0) * t)
        }
        FlarePhase::Active => {
            let (r, g, b, a) = C_FLARE_ACTIVE;
            // Sheltered players get a much dimmer wash: the screen effect is
            // the danger, and being safe should look and feel different.
            let shelter_mult = if sheltered { 0.28 } else { 1.0 };
            let flicker = 0.82 + 0.18 * ((ticks as f32 * 0.9).sin() * 0.5 + 0.5);
            ((r, g, b), (a as f32 / 255.0) * flicker * shelter_mult)
        }
    };

    set_overlay(c, alpha, tint);

    let banner = match phase {
        FlarePhase::Warning => Some("SOLAR FLARE INCOMING — REACH A SHIELDED NODE"),
        FlarePhase::Active if sheltered => Some("SHELTERED"),
        FlarePhase::Active => Some("SOLAR FLARE — TETHER TO A SHIELDED NODE"),
        FlarePhase::Idle => None,
    };
    set_flare_banner(c, banner);

    // A wavefront sweeping across the screen during the active window, so the
    // flare reads as something passing over rather than a static colour cast.
    if phase == FlarePhase::Active {
        let t = 1.0 - (active_left as f32 / FLARE_ACTIVE_TICKS as f32);
        let px = st.lock().unwrap().px;
        let sweep_x = px - VW * 0.6 + VW * 1.2 * t;
        let py = st.lock().unwrap().py;
        fx::push_mega_fx(
            c,
            crate::images::flare_front_img(),
            (sweep_x, py),
            (VW * 0.18, VH * 2.4),
            (1.0, 0.92, 0.62, 0.5),
            0,
        );
    }
}

fn set_overlay(c: &mut Canvas, alpha: f32, tint: (u8, u8, u8)) {
    let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
    if let Some(obj) = c.get_game_object_mut("flare_overlay") {
        if a == 0 {
            obj.visible = false;
        } else {
            obj.visible = true;
            obj.set_image(crate::images::flare_overlay_img(tint.0, tint.1, tint.2, a));
        }
    }
}

fn set_flare_banner(c: &mut Canvas, text: Option<&str>) {
    let Some(text) = text else {
        if let Some(obj) = c.get_game_object_mut("flare_banner") {
            obj.visible = false;
        }
        return;
    };
    let last = c.get_var("flare_banner_text");
    let unchanged = matches!(&last, Some(Value::Str(s)) if s == text);
    let scale = c.virtual_scale();
    if let Some(obj) = c.get_game_object_mut("flare_banner") {
        obj.visible = true;
        if unchanged {
            return;
        }
    } else {
        return;
    }
    // Text layout is cached by content hash, so only rebuild the drawable when
    // the string actually changes — the banner is live for hundreds of frames.
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
        let spec = crate::objects::ui_text_spec(
            text, &font, 46.0 * scale, Color(255, 236, 190, 255), 1800.0 * scale,
        );
        if let Some(obj) = c.get_game_object_mut("flare_banner") {
            obj.set_drawable(Box::new(spec));
        }
    }
    c.set_var("flare_banner_text", Value::Str(text.to_string()));
}
