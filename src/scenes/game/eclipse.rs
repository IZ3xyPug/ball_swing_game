// ── scenes/game/eclipse.rs — the solar eclipse that precedes a boss ──────────
//
// The last stretch before a boss teleporter goes dark. A warning names the
// event, the ambient light falls away over the approach, and the player becomes
// the main light source in the world: a wide lamp that throws real shadows off
// pads and spinners, with faint marker lights on the grab nodes so the route
// stays readable without giving the hazards away.
//
// It exists to solve a pacing problem as much as a visual one. The teleport used
// to arrive with no build-up at all, so the fight began before the player knew
// one was coming. The eclipse is a minute of unmistakable "something is about to
// happen" that also changes how the level plays while it lasts.
//
// Ends the moment the fight starts (`boss_active`), and restores every light,
// the ambient, and the shadow-caster flags it touched.
//
// ── How quartz lighting actually works ──────────────────────────────────────
// Traced through `quartz/src/lighting` -> `canvas/core.rs` -> `wgpu_canvas` ->
// `lit_rectangle.wgsl`. Read this before changing any value here; three
// rewrites of this file were spent guessing at it.
//
//     accum = ambient_rgb * ambient_strength
//           + SUM over lights in range( color * ndl * intensity * atten * shadow )
//     lit   = clamp(base_color * accum, 0, 1)
//
//   * LIGHT IS MULTIPLICATIVE. It scales a sprite's own colour, so it can only
//     restore art toward the brightness it was drawn at — it can never add
//     light to a dark surface. Empty black sky stays black under any lamp. An
//     eclipse here is therefore "ambient down, lamp restores what it touches",
//     and the visible effect is objects entering and leaving normal brightness.
//   * `ndl` is a CONSTANT 0.4472 in 2D: the default normal map is flat
//     (128,128,255) = (0,0,1) and `ldir = normalize(vec3(dir_2d, 0.5))`, so the
//     dot product has no directional term. Any intensity derivation must divide
//     by it.
//   * `atten = 1 - smoothstep(0, radius, dist)` with a HARD cutoff at radius,
//     and the accumulator clamps — so a high intensity gives a fully-restored
//     pool out to where `ndl * intensity * atten` reaches 1, then a fade.
//   * Every `Item::Image` becomes a lit sprite while lighting is on, so this
//     applies to the background too — but only to the extent its art is bright.
//   * Engine presets run intensity 0.3-1.2. Those light an already-lit scene;
//     restoring a near-black one needs far more, and that is legitimate.

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

/// Light ids, so teardown can be exhaustive.
const PLAYER_LIGHT: &str = "eclipse_player_light";

fn gwell_light_id(i: usize) -> String {
    format!("eclipse_gwell_light_{i}")
}
const NODE_LIGHT_COUNT: usize = 16;

fn node_light_id(i: usize) -> String {
    format!("eclipse_node_light_{i}")
}

/// How dark it is `gap` px before the boss teleporter.
///
/// Two curves, not one:
///   `rise`  — how far the darkening has progressed, 0 at the far edge of the
///             approach and 1 by the release point. Drives the banner.
///   `dark`  — `rise` multiplied by a release that lifts the dark again over
///             the last `BOSS_ECLIPSE_RELEASE` px, so the player reaches the
///             black hole in daylight.
///
/// Running the darkness right into the teleport made the eclipse and the
/// teleport read as one event. Separating them lets the eclipse be its own beat
/// that visibly passes, and leaves the black hole lit when it matters.
pub fn eclipse_curve(gap: f32) -> (f32, f32) {
    let ramp = (BOSS_ECLIPSE_RANGE - BOSS_ECLIPSE_RELEASE).max(1.0);
    let rise = ((BOSS_ECLIPSE_RANGE - gap) / ramp).clamp(0.0, 1.0);
    let release = (gap / BOSS_ECLIPSE_RELEASE).clamp(0.0, 1.0);
    (rise, rise * release)
}

// ── Tick ─────────────────────────────────────────────────────────────────────

pub fn tick_eclipse(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mode = crate::mode::current_mode(c);

    let (px, py, boss_active, in_space, dead, boss_index, was_active) = {
        let s = st.lock().unwrap();
        (s.px, s.py, s.boss_active, s.in_space_mode || s.space_launch_active,
         s.dead, s.boss_index, s.eclipse_active)
    };

    // How far to the next fight. `None` means this mode/run has none left, so
    // there is nothing to build up to.
    let to_threshold = crate::mode::boss_trigger_distance(mode, boss_index)
        .map(|d| (SPAWN_X + d) - px);

    let want = match to_threshold {
        Some(gap) if !boss_active && !in_space && !dead => {
            gap <= BOSS_ECLIPSE_RANGE && gap > 0.0
        }
        _ => false,
    };

    if !want {
        if was_active {
            end_eclipse(c, st);
        }
        return;
    }

    let gap = to_threshold.unwrap_or(BOSS_ECLIPSE_RANGE);
    let (rise, dark) = eclipse_curve(gap);

    if !was_active {
        begin_eclipse(c, st);
    }
    {
        let mut s = st.lock().unwrap();
        s.eclipse_active = true;
        s.eclipse_t = dark;
    }
    c.set_var("eclipse_active", true);
    c.set_var("eclipse_t", Value::F32(dark));

    drive_darkness(c, dark);
    drive_lights(c, st, px, py, dark);
    drive_banner(c, rise);
}

// ── Begin / end ──────────────────────────────────────────────────────────────

fn begin_eclipse(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    if !ECLIPSE_USE_POINT_LIGHTS {
        return;
    }
    if !c.has_lighting() {
        // Lighting is enabled at scene entry, but never assume — without it the
        // eclipse is a banner and nothing else, which is still better than a
        // teleport out of nowhere.
        return;
    }

    // The player's own lamp. Wide, warm, and the only thing in the world that
    // throws shadows, so the light reads as coming FROM the player.
    if c.get_light(PLAYER_LIGHT).is_none() {
        let mut ls = LightSource::new(
            PLAYER_LIGHT,
            (0.0, 0.0),
            Color(255, 238, 205, 255),
            ECLIPSE_PLAYER_LIGHT_R,
            ECLIPSE_PLAYER_LIGHT_INTENSITY,
        );
        ls.casts_shadows = true;
        c.add_light(ls);
        c.attach_light(PLAYER_LIGHT, "player", (0.0, 0.0));
    }
    c.set_light_enabled(PLAYER_LIGHT, true);


    // ONE LIGHT PER HOOK POOL SLOT, attached to that slot's object.
    //
    // Not a shared pool repositioned onto the nearest few: that has to re-rank
    // as the player moves, and every re-rank switches lights on and off, which
    // is what made nodes appear to light up and go dark as you passed them.
    // Attached lights follow their object for free and never change identity,
    // so all a frame has to do is match `enabled` to `visible`.
    for i in 0..HOOK_POOL_SIZE {
        let id = node_light_id(i);
        if c.get_light(&id).is_none() {
            let mut ls = LightSource::new(
                id.clone(),
                (0.0, 0.0),
                Color(120, 200, 255, 255),
                ECLIPSE_NODE_LIGHT_R,
                0.0,
            );
            ls.casts_shadows = false;
            c.add_light(ls);
            c.attach_light(&id, &format!("hook_{i}"), (0.0, 0.0));
        }
    }

    // Gravity wells light THEMSELVES. They are a hazard the player has to see
    // coming even when the lamp is nowhere near them, and a well-shaped hole in
    // the dark reads as geometry rather than as danger. They are excluded from
    // the shadow casters for the same reason.
    for i in 0..ECLIPSE_GWELL_LIGHT_COUNT {
        let id = gwell_light_id(i);
        if c.get_light(&id).is_none() {
            let mut ls = LightSource::new(
                id.clone(),
                (0.0, 0.0),
                Color(190, 120, 255, 255),
                ECLIPSE_GWELL_LIGHT_R,
                0.0,
            );
            ls.casts_shadows = false;
            c.add_light(ls);
            c.attach_light(&id, &format!("gwell_{i}"), (0.0, 0.0));
        }
    }

    let _ = st;
}

fn end_eclipse(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    {
        let mut s = st.lock().unwrap();
        s.eclipse_active = false;
        s.eclipse_t = 0.0;
    }
    c.set_var("eclipse_active", false);
    c.set_var("eclipse_t", Value::F32(0.0));

    if ECLIPSE_USE_POINT_LIGHTS && c.has_lighting() {
        c.set_light_enabled(PLAYER_LIGHT, false);
        for i in 0..HOOK_POOL_SIZE {
            c.set_light_enabled(&node_light_id(i), false);
        }
        for i in 0..ECLIPSE_GWELL_LIGHT_COUNT {
            c.set_light_enabled(&gwell_light_id(i), false);
        }
        // Full daylight again. The boss fight sets its own ambient on entry, so
        // this only matters when the eclipse ends by backtracking.
        c.set_ambient(Color(255, 255, 255, 255), 1.0);
    }

    clear_shadow_casters(c, st);

    if let Some(obj) = c.get_game_object_mut("eclipse_banner") {
        obj.visible = false;
    }
    c.set_var("eclipse_banner_text", Value::Str(String::new()));
}

// ── Darkness ─────────────────────────────────────────────────────────────────

fn drive_darkness(c: &mut Canvas, dark: f32) {
    if !c.has_lighting() {
        return;
    }
    // Hold full light briefly so the warning lands before the world reacts.
    // Never reaches true black: the player must still be able to read the
    // horizon and the danger floor.
    let fall = ((dark - ECLIPSE_WARN_FRACTION) / (1.0 - ECLIPSE_WARN_FRACTION)).clamp(0.0, 1.0);
    // Ease OUT, not smoothstep. Smoothstep is symmetric, so half the approach
    // was spent barely changing; the dark needs to arrive early and then settle,
    // which is what an eclipse actually looks like.
    let inv = 1.0 - fall;
    let eased = 1.0 - inv * inv;
    let strength = 1.0 + (ECLIPSE_MIN_AMBIENT - 1.0) * eased;
    // Cool the ambient as it dims — a dimmed white reads as fog, a dimmed blue
    // reads as an eclipse.
    let r = (255.0 - 175.0 * eased) as u8;
    let g = (255.0 - 170.0 * eased) as u8;
    let b = (255.0 - 105.0 * eased) as u8;
    c.set_ambient(Color(r, g, b, 255), strength);
}

// ── Lights ───────────────────────────────────────────────────────────────────

fn drive_lights(c: &mut Canvas, st: &Arc<Mutex<State>>, px: f32, py: f32, dark: f32) {
    if !ECLIPSE_USE_POINT_LIGHTS {
        return;
    }
    if !c.has_lighting() {
        return;
    }
    let fall = ((dark - ECLIPSE_WARN_FRACTION) / (1.0 - ECLIPSE_WARN_FRACTION)).clamp(0.0, 1.0);
    let fall = (fall / ECLIPSE_FULL_DARK_AT).clamp(0.0, 1.0);

    // The player's lamp widens as the dark deepens, so visibility falls less
    // sharply than the ambient does — the world gets darker, the player's reach
    // into it gets longer.
    // The lamp holds its size — it is sized to contain the trail, and a lamp
    // that grows into that size spends the first half of the eclipse showing a
    // half-lit trail. Only the intensity ramps, and from a high floor: "on but
    // dim" still has to read as a working light, not a fault.
    if let Some(light) = c.get_light_mut(PLAYER_LIGHT) {
        light.radius = ECLIPSE_PLAYER_LIGHT_R;
        light.intensity = ECLIPSE_PLAYER_LIGHT_INTENSITY * (0.70 + 0.30 * fall);
    }

    // Markers: every pool slot has its own attached light, so a frame only has
    // to match `enabled` to `visible` and set the brightness. No ranking, no
    // sort, no allocation — and nothing pops as the player moves.
    let node_i = ECLIPSE_NODE_LIGHT_INTENSITY * fall;
    for i in 0..HOOK_POOL_SIZE {
        let vis = c
            .get_game_object(&format!("hook_{i}"))
            .map(|o| o.visible)
            .unwrap_or(false);
        let id = node_light_id(i);
        c.set_light_enabled(&id, vis);
        if vis {
            if let Some(light) = c.get_light_mut(&id) {
                light.intensity = node_i;
            }
        }
    }
    let well_i = ECLIPSE_GWELL_LIGHT_INTENSITY * fall;
    for i in 0..ECLIPSE_GWELL_LIGHT_COUNT {
        let vis = c
            .get_game_object(&format!("gwell_{i}"))
            .map(|o| o.visible)
            .unwrap_or(false);
        let id = gwell_light_id(i);
        c.set_light_enabled(&id, vis);
        if vis {
            if let Some(light) = c.get_light_mut(&id) {
                light.intensity = well_i;
            }
        }
    }

    // Shadow casters still refresh on a cadence: that scan DOES rank by
    // distance, because the renderer can only upload 32 occluders.
    let due = {
        let mut s = st.lock().unwrap();
        s.eclipse_light_timer = s.eclipse_light_timer.saturating_sub(1);
        if s.eclipse_light_timer == 0 {
            s.eclipse_light_timer = ECLIPSE_LIGHT_REFRESH_TICKS;
            true
        } else {
            false
        }
    };
    if due {
        set_shadow_casters(c, st, fall > 0.05);
    }
}

/// Turn nearby pads and spinners into shadow occluders while the eclipse runs.
///
/// Tracked in `State` so teardown is exhaustive: an object left flagged after
/// the eclipse would keep casting shadows for the rest of the run, and the flag
/// is invisible in the editor.
fn set_shadow_casters(c: &mut Canvas, st: &Arc<Mutex<State>>, on: bool) {
    // Everything solid enough to read as an object casts. Grab nodes do NOT —
    // they are the route, they carry their own marker lights, and a node that
    // throws a shadow across the line you are swinging into is noise.
    // Candidates, and how many of the leading ones are rectangular. Gravity
    // wells are deliberately absent — they light themselves instead.
    let (rect, round, already, px, py) = {
        let s = st.lock().unwrap();
        let mut rect: Vec<String> = Vec::new();
        rect.extend(s.pad_live.iter().cloned());
        rect.extend(s.spinner_live.iter().cloned());
        rect.extend(s.turret_live.iter().cloned());
        let round: Vec<String> = s.space_asteroid_live.clone();
        (rect, round, s.eclipse_shadow_ids.clone(), s.px, s.py)
    };
    if !on {
        if !already.is_empty() {
            clear_shadow_casters(c, st);
        }
        return;
    }

    // Clear the previous set FIRST, then flag the current one. The old version
    // built the new list and then diffed it against the old with `contains`,
    // which is O(n*m) over string comparisons every frame — and it left any
    // object that had scrolled out of `*_live` still flagged, so pooled objects
    // carried `shadow_caster` into their next life and threw shadows from
    // nowhere.
    for id in &already {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.shadow_caster = false;
        }
    }

    // Rank by distance and keep only what the renderer can actually upload.
    // Anything past the cap would be dropped silently, and anything outside the
    // lamp casts a shadow nobody can see.
    let mut ranked: Vec<(f32, &String, bool)> = Vec::new();
    for (id, is_round) in rect.iter().map(|i| (i, false)).chain(round.iter().map(|i| (i, true))) {
        let Some(obj) = c.get_game_object(id) else { continue };
        if !obj.visible {
            continue;
        }
        let cx = obj.position.0 + obj.size.0 * 0.5;
        let cy = obj.position.1 + obj.size.1 * 0.5;
        let d = (cx - px) * (cx - px) + (cy - py) * (cy - py);
        if d > ECLIPSE_PLAYER_LIGHT_R * ECLIPSE_PLAYER_LIGHT_R {
            continue;
        }
        ranked.push((d, id, is_round));
    }
    ranked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(ECLIPSE_MAX_SHADOW_CASTERS);

    let mut flagged = std::mem::take(&mut st.lock().unwrap().eclipse_shadow_ids);
    flagged.clear();
    for (_, id, is_round) in &ranked {
        if let Some(obj) = c.get_game_object_mut(id) {
            // Asteroids are discs; without this they cast the shadow of their
            // bounding box, which reads as a floating slab.
            obj.shadow_circle = *is_round;
            obj.shadow_caster = true;
            flagged.push((*id).clone());
        }
    }
    st.lock().unwrap().eclipse_shadow_ids = flagged;
}

fn clear_shadow_casters(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let ids = std::mem::take(&mut st.lock().unwrap().eclipse_shadow_ids);
    for id in ids {
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.shadow_caster = false;
        }
    }
}

// ── Banner ───────────────────────────────────────────────────────────────────

fn drive_banner(c: &mut Canvas, rise: f32) {
    // The warning names the event early, then hands over to a countdown of sorts
    // — "the dark is deepening" is carried by the world, not by text.
    let text = if rise < ECLIPSE_WARN_FRACTION {
        Some("\u{26A0}  SOLAR ECLIPSE EVENT DETECTED")
    } else if rise < ECLIPSE_WARN_FRACTION * 2.2 {
        Some("LIGHT FAILING \u{2014} SOMETHING IS AHEAD")
    } else {
        None
    };

    let Some(text) = text else {
        if let Some(obj) = c.get_game_object_mut("eclipse_banner") {
            obj.visible = false;
        }
        return;
    };

    let unchanged = matches!(c.get_var("eclipse_banner_text"), Some(Value::Str(ref s)) if s == text);
    let scale = c.virtual_scale();
    if let Some(obj) = c.get_game_object_mut("eclipse_banner") {
        obj.visible = true;
    } else {
        return;
    }
    if unchanged {
        return;
    }
    // Text layout is cached by content hash, so only rebuild when the string
    // actually changes — this banner is live for thousands of frames.
    if let Ok(font) = Font::from_bytes(include_bytes!("../../../assets/font.ttf")) {
        let spec = crate::objects::ui_text_spec(
            text, &font, 52.0 * scale, Color(200, 220, 255, 255), 1900.0 * scale,
        );
        if let Some(obj) = c.get_game_object_mut("eclipse_banner") {
            obj.set_drawable(Box::new(spec));
        }
    }
    c.set_var("eclipse_banner_text", Value::Str(text.to_string()));
}
