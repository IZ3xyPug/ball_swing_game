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
        for i in 0..NODE_LIGHT_COUNT {
            c.set_light_enabled(&node_light_id(i), false);
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

    // The player's lamp widens as the dark deepens, so visibility falls less
    // sharply than the ambient does — the world gets darker, the player's reach
    // into it gets longer.
    if let Some(light) = c.get_light_mut(PLAYER_LIGHT) {
        // Grows modestly as the dark deepens, so visibility falls less sharply
        // than the ambient does — but stays a POOL with a visible edge, which
        // is the whole reason the eclipse reads as one.
        light.radius = ECLIPSE_PLAYER_LIGHT_R * (0.85 + 0.30 * fall);
        light.intensity = ECLIPSE_PLAYER_LIGHT_INTENSITY * (0.45 + 0.55 * fall);
    }

    // Marker lights on the nearest live grab nodes. Nodes only: pads, spinners
    // and wells stay unlit so the player is finding their ROUTE by light and
    // still finding the hazards by the player lamp.
    let mut nodes: Vec<(f32, f32, f32)> = {
        let s = st.lock().unwrap();
        s.live_hooks
            .iter()
            .filter_map(|id| c.get_game_object(id))
            .filter(|o| o.visible)
            .map(|o| {
                let hx = o.position.0 + o.size.0 * 0.5;
                let hy = o.position.1 + o.size.1 * 0.5;
                let d = (hx - px) * (hx - px) + (hy - py) * (hy - py);
                (d, hx, hy)
            })
            .collect()
    };
    nodes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    for i in 0..NODE_LIGHT_COUNT {
        let id = node_light_id(i);
        match nodes.get(i) {
            Some(&(_, hx, hy)) => {
                c.set_light_position(&id, hx, hy);
                if let Some(light) = c.get_light_mut(&id) {
                    light.intensity = ECLIPSE_NODE_LIGHT_INTENSITY * fall;
                }
                c.set_light_enabled(&id, true);
            }
            None => c.set_light_enabled(&id, false),
        }
    }

    set_shadow_casters(c, st, fall > 0.05);
}

/// Turn nearby pads and spinners into shadow occluders while the eclipse runs.
///
/// Tracked in `State` so teardown is exhaustive: an object left flagged after
/// the eclipse would keep casting shadows for the rest of the run, and the flag
/// is invisible in the editor.
fn set_shadow_casters(c: &mut Canvas, st: &Arc<Mutex<State>>, on: bool) {
    let (pads, spinners, already) = {
        let s = st.lock().unwrap();
        (s.pad_live.clone(), s.spinner_live.clone(), s.eclipse_shadow_ids.clone())
    };
    if !on {
        if !already.is_empty() {
            clear_shadow_casters(c, st);
        }
        return;
    }

    let mut flagged: Vec<String> = Vec::new();
    for id in pads.iter().chain(spinners.iter()) {
        if let Some(obj) = c.get_game_object_mut(id) {
            if obj.visible {
                obj.shadow_caster = true;
                flagged.push(id.clone());
            }
        }
    }
    // Anything that was flagged and is no longer live must be cleared, or a
    // recycled pool object carries the flag into its next life.
    for id in &already {
        if !flagged.contains(id) {
            if let Some(obj) = c.get_game_object_mut(id) {
                obj.shadow_caster = false;
            }
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
