// ── scenes/game/fx.rs — shader-effect policy ──────────────────────────────────
// The effects themselves are quartz's typed `Effect` enum: `c.attach_effect(id,
// Effect::ThrustPlume { thrust }, colour, size)`, `c.clear_effect(id)`, and
// `c.push_effect(..)` for an overlay that belongs to no object. The engine owns
// the packing — this file used to hand-pack `[u32; 4]` bitmasks and keep its
// own copy of the shader's bit numbers, which is how a mis-packed word drew
// the wrong effect with no error.
//
// What is left here is GAME policy: how big a marker draws around a part, and
// which colour space the game's colour constants are in.

use quartz::*;
use crate::constants::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// The game's effect colour constants are LINEAR `(r, g, b)` — they were tuned
/// by eye straight into the shader, which multiplies in linear space. This
/// says so at every call site; new colours picked in a colour picker should use
/// `EffectColor::srgb` / `srgb8` instead, which decode.
pub fn lin(rgb: (f32, f32, f32)) -> EffectColor {
    EffectColor::linear(rgb.0, rgb.1, rgb.2)
}

// ── Object-attached effects ──────────────────────────────────────────────────
//
// A pushed effect (`push_effect`) queues into a renderer pass that runs after colour, image
// and material — so a pushed effect is in front of the ENTIRE scene and there
// is no z that puts it behind anything. That is right for an overlay on the
// player and wrong for an aura that belongs to a world object: a buff node's
// electricity drew over the asteroids in front of it, which reads as the
// effect floating in a different plane from the thing it is attached to.
//
// `attach_effect` hangs the sprite off the object instead. The item
// is then emitted from that object's own `draw`, so it inherits the object's
// layer, its culling, and its depth in the same list as every other sprite —
// after culling, with no index to guess. Anything on a higher layer occludes
// it, exactly as it occludes the object itself.
//
// Both halves are needed and the first attempt shipped only one: the renderer
// has to take the sprite's depth FROM the item's place in the draw list, and
// pushed effects have to be emitted after every child so they still land on
// top. Without the first, the item's position is decorative and every effect
// draws over the scene regardless of where it was emitted.
//
// Effects attached this way persist until `clear_effect`, unlike pushed ones
// (drained every frame), so a system that attaches must also detach — on every
// path, early returns included.

// ── State timing and hit flashes ─────────────────────────────────────────────
//
// A state marker knows WHAT a part is (open, shielded, winding up) but not
// WHEN it became so, and the "now" of a weak point — the white flash as it
// opens, Metroid Dread's counter tell — is all about when. Rather than every
// boss tracking the frame each of its states began, this does it once, keyed
// by object: a marker called with the same mode on consecutive frames is
// the same state getting older; a different mode, or a gap of a frame, is a
// new one. `tick_fx` advances the frame; the boss tick calls it first.

struct Seen {
    mode: MarkerMode,
    age: u32,
    frame: u64,
}

#[derive(Default)]
struct FxState {
    frame: u64,
    markers: HashMap<String, Seen>,
    /// Objects mid hit-flash, with frames left.
    flashes: HashMap<String, u32>,
}

fn fx_state() -> &'static Mutex<FxState> {
    static S: OnceLock<Mutex<FxState>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(FxState::default()))
}

/// Advance effect time by one frame. Call once per tick, before any marker
/// or flash: it ages every state and ends hit flashes whose time is up.
pub fn tick_fx(c: &mut Canvas) {
    let ended: Vec<String> = {
        let mut s = fx_state().lock().unwrap_or_else(|e| e.into_inner());
        s.frame += 1;
        let frame = s.frame;
        // States not seen for a while are gone; keep the map small.
        s.markers.retain(|_, v| v.frame + 120 > frame);
        let mut ended = Vec::new();
        s.flashes.retain(|id, left| {
            *left = left.saturating_sub(1);
            if *left == 0 {
                ended.push(id.clone());
                false
            } else {
                true
            }
        });
        ended
    };
    for id in ended {
        if let Some(obj) = c.get_game_object_mut(&id) {
            obj.clear_tint();
        }
    }
}

/// How long `object_id` has been in `mode`, advancing it by this frame.
fn state_age(object_id: &str, mode: MarkerMode) -> u32 {
    let mut s = fx_state().lock().unwrap_or_else(|e| e.into_inner());
    let frame = s.frame;
    let entry = s.markers.entry(object_id.to_owned()).or_insert(Seen { mode, age: 0, frame });
    // Same state, seen last frame (or already this frame): older. Anything
    // else — a new mode, or a frame without it — starts over, so a part that
    // closes and reopens flashes again.
    if entry.mode == mode && entry.frame + 1 >= frame {
        if entry.frame != frame {
            entry.age = entry.age.saturating_add(1);
        }
    } else {
        entry.mode = mode;
        entry.age = 0;
    }
    entry.frame = frame;
    entry.age
}

/// Flash a part white for a few frames: the hit landed, on THIS part. The
/// standard 2D confirmation (Hollow Knight, Dead Cells) — cheap, because
/// the tint is the sprite's own alpha mask, cached per image.
pub fn flash_hit(c: &mut Canvas, object_id: &str) {
    if let Some(obj) = c.get_game_object_mut(object_id) {
        obj.set_tint(Color(255, 255, 255, HIT_FLASH_ALPHA));
    } else {
        return;
    }
    let mut s = fx_state().lock().unwrap_or_else(|e| e.into_inner());
    s.flashes.insert(object_id.to_owned(), HIT_FLASH_TICKS);
}

/// Attach a boss state marker to `object_id`.
///
/// One call for every "this part is vulnerable / shielded / winding up" cue in
/// the game. Drawn larger than the part (`STATE_MARKER_SCALE`) so the reticle
/// sits around the silhouette instead of over it; the shield hugs it
/// (`SHIELD_MARKER_SCALE`).
pub fn attach_state_marker(
    c: &mut Canvas, object_id: &str, part_size: (f32, f32),
    rgb: (f32, f32, f32), intensity: f32, mode: MarkerMode,
) {
    attach_state_marker_timed(c, object_id, part_size, rgb, intensity, mode, None);
}

/// `attach_state_marker`, knowing how close the state is to ending:
/// `closing` 0 with time to spare, 1 about to end. A vulnerable window then
/// speeds its pulse into a flicker as it closes — the player sees the time
/// left instead of guessing it. `None` when the boss does not know.
pub fn attach_state_marker_timed(
    c: &mut Canvas, object_id: &str, part_size: (f32, f32),
    rgb: (f32, f32, f32), intensity: f32, mode: MarkerMode, closing: Option<f32>,
) {
    let age = state_age(object_id, mode);
    // Vulnerable and shielded follow the sprite's OWN OUTLINE where it has
    // one — a shell wrapped on the fist, a hot rim on the spindle — and only
    // fall back to the circle for shapes and fills. Winding-up stays a ring:
    // it is about the space around the part, not the part.
    if mode != MarkerMode::WindingUp {
        let timing = StateTiming {
            opened: (age as f32 / STATE_OPEN_TICKS as f32).min(1.0),
            closing: closing.unwrap_or(0.0).clamp(0.0, 1.0),
        };
        if c.attach_outline_state(object_id, mode, intensity, lin(rgb), timing) {
            return;
        }
    }
    attach_circle_marker(c, object_id, part_size, rgb, intensity, mode);
}

/// Wind-up rings — "something is coming" — on the OVERLAY slot, so they sit
/// on top of whatever state the part shows: a spindle winding up is still
/// protected, and says both at once.
pub fn attach_windup(
    c: &mut Canvas, object_id: &str, part_size: (f32, f32),
    rgb: (f32, f32, f32), intensity: f32,
) {
    c.attach_overlay_effect(
        object_id,
        Effect::StateMarker { mode: MarkerMode::WindingUp, intensity },
        lin(rgb),
        (part_size.0 * STATE_MARKER_SCALE, part_size.1 * STATE_MARKER_SCALE),
    );
}

/// Drop the wind-up rings from `object_id`.
pub fn clear_windup(c: &mut Canvas, object_id: &str) {
    c.clear_overlay_effect(object_id);
}

/// The circle state marker, never the outline: for ZONES — a weakpoint ring,
/// a strike area — whose image is an abstract ring rather than a part. An
/// outline wrapped on a thin ring draws a doubled rim around nothing.
pub fn attach_circle_marker(
    c: &mut Canvas, object_id: &str, part_size: (f32, f32),
    rgb: (f32, f32, f32), intensity: f32, mode: MarkerMode,
) {
    let k = if mode == MarkerMode::Shielded { SHIELD_MARKER_SCALE } else { STATE_MARKER_SCALE };
    c.attach_effect(
        object_id,
        Effect::StateMarker { mode, intensity },
        lin(rgb),
        (part_size.0 * k, part_size.1 * k),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next_frame() {
        fx_state().lock().unwrap().frame += 1;
    }

    #[test]
    fn a_state_ages_while_it_holds_and_restarts_when_it_changes_or_lapses() {
        // Unique ids: the tracker is process-wide and tests run in parallel.
        let id = "fx_test_part_age";
        next_frame();
        assert_eq!(state_age(id, MarkerMode::Vulnerable), 0, "a new state starts at 0");
        assert_eq!(state_age(id, MarkerMode::Vulnerable), 0, "twice in one frame is one frame");
        next_frame();
        assert_eq!(state_age(id, MarkerMode::Vulnerable), 1);
        next_frame();
        assert_eq!(state_age(id, MarkerMode::Vulnerable), 2);
        next_frame();
        assert_eq!(state_age(id, MarkerMode::Shielded), 0, "a different state is a new one");
        // Skip a frame: the part was not shielded then, so this is a NEW
        // shield — it must power on again, not carry on from before.
        next_frame();
        next_frame();
        assert_eq!(state_age(id, MarkerMode::Shielded), 0, "a lapsed state restarts");
    }
}
