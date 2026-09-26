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
    let k = if mode == MarkerMode::Shielded { SHIELD_MARKER_SCALE } else { STATE_MARKER_SCALE };
    c.attach_effect(
        object_id,
        Effect::StateMarker { mode, intensity },
        lin(rgb),
        (part_size.0 * k, part_size.1 * k),
    );
}
