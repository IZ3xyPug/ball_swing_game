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

use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::state::*;

/// Light ids, so teardown can be exhaustive.
const PLAYER_LIGHT: &str = "eclipse_player_light";
const FILL_LIGHT: &str = "eclipse_fill_light";

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

    // Wide dim fill, no shadows. Flattens the point light's falloff so the
    // player is not sitting in a hotspot with their own trail fading out behind
    // them; leaving shadows to the single main lamp is what keeps them defined.
    if c.get_light(FILL_LIGHT).is_none() {
        let mut ls = LightSource::new(
            FILL_LIGHT,
            (0.0, 0.0),
            Color(215, 228, 255, 255),
            ECLIPSE_FILL_LIGHT_R,
            ECLIPSE_FILL_LIGHT_INTENSITY,
        );
        ls.casts_shadows = false;
        c.add_light(ls);
        c.attach_light(FILL_LIGHT, "player", (0.0, 0.0));
    }
    c.set_light_enabled(FILL_LIGHT, true);

    // A bounded pool of marker lights. Bounded because the lighting config caps
    // at 64 and the boss fight wants a chunk of those for its bolts — one light
    // per live node would blow the budget the moment the node count rises.
    for i in 0..NODE_LIGHT_COUNT {
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
        }
        c.set_light_enabled(&id, true);
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
        }
        c.set_light_enabled(&id, true);
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

    if c.has_lighting() {
        c.set_light_enabled(PLAYER_LIGHT, false);
        c.set_light_enabled(FILL_LIGHT, false);
        for i in 0..NODE_LIGHT_COUNT {
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
    if let Some(light) = c.get_light_mut(FILL_LIGHT) {
        light.radius = ECLIPSE_FILL_LIGHT_R;
        light.intensity = ECLIPSE_FILL_LIGHT_INTENSITY * (0.70 + 0.30 * fall);
    }

    // Marker lights on the nearest live grab nodes. Nodes only: pads, spinners
    // and wells stay unlit so the player is finding their ROUTE by light and
    // still finding the hazards by the player lamp.
    //
    // Refreshed on a cadence, not every frame. Collecting every live node,
    // measuring it and sorting the result is O(n log n) with a fresh allocation,
    // and at 60 Hz alongside the shadow pass it was the bulk of the eclipse's
    // frame cost. Nodes drift slowly relative to the player, so a few frames of
    // staleness is invisible.
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
        // Reuse the buffer rather than allocating one per refresh.
        let mut nodes = std::mem::take(&mut st.lock().unwrap().eclipse_node_buf);
        nodes.clear();
        {
            let s = st.lock().unwrap();
            for id in &s.live_hooks {
                let Some(o) = c.get_game_object(id) else { continue };
                if !o.visible {
                    continue;
                }
                let hx = o.position.0 + o.size.0 * 0.5;
                let hy = o.position.1 + o.size.1 * 0.5;
                // Only nodes that could plausibly be lit are worth ranking.
                if (hx - px).abs() > ECLIPSE_FILL_LIGHT_R || (hy - py).abs() > ECLIPSE_FILL_LIGHT_R {
                    continue;
                }
                nodes.push(((hx - px) * (hx - px) + (hy - py) * (hy - py), hx, hy));
            }
        }
        nodes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        for i in 0..NODE_LIGHT_COUNT {
            let id = node_light_id(i);
            match nodes.get(i) {
                Some(&(_, hx, hy)) => {
                    c.set_light_position(&id, hx, hy);
                    c.set_light_enabled(&id, true);
                }
                None => c.set_light_enabled(&id, false),
            }
        }
        st.lock().unwrap().eclipse_node_buf = nodes;

        // Nearest wells get a self-light so they stay readable outside the lamp.
        let wells: Vec<String> = st.lock().unwrap().gwell_live.clone();
        let mut ranked: Vec<(f32, f32, f32)> = wells
            .iter()
            .filter_map(|id| c.get_game_object(id))
            .filter(|o| o.visible)
            .map(|o| {
                let cx = o.position.0 + o.size.0 * 0.5;
                let cy = o.position.1 + o.size.1 * 0.5;
                ((cx - px) * (cx - px) + (cy - py) * (cy - py), cx, cy)
            })
            .collect();
        ranked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        for i in 0..ECLIPSE_GWELL_LIGHT_COUNT {
            let id = gwell_light_id(i);
            match ranked.get(i) {
                Some(&(_, cx, cy)) => {
                    c.set_light_position(&id, cx, cy);
                    c.set_light_enabled(&id, true);
                }
                None => c.set_light_enabled(&id, false),
            }
        }

        set_shadow_casters(c, st, fall > 0.05);
    }

    // Intensity is cheap and wants to be smooth, so it still updates every frame.
    for i in 0..NODE_LIGHT_COUNT {
        if let Some(light) = c.get_light_mut(&node_light_id(i)) {
            light.intensity = ECLIPSE_NODE_LIGHT_INTENSITY * fall;
        }
    }
    for i in 0..ECLIPSE_GWELL_LIGHT_COUNT {
        if let Some(light) = c.get_light_mut(&gwell_light_id(i)) {
            light.intensity = ECLIPSE_GWELL_LIGHT_INTENSITY * fall;
        }
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
        if d > ECLIPSE_FILL_LIGHT_R * ECLIPSE_FILL_LIGHT_R {
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
