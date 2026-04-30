// ── objects/planets.rs ────────────────────────────────────────────────────────
// Space zone planets: always-on gravity wells with faint field-boundary rings.
use quartz::*;
use crate::constants::*;
use crate::images::*;

/// Build a planet game object.
///
/// * `id`         — registration key for the physics/gravity object
/// * `x`, `y`     — world center position (negative y for space zone)
/// * `visual_r`   — radius of the rendered planet body in pixels
/// * `gravity_r`  — kept for call-site compat; no longer used for sizing
/// * `color_idx`  — index into `C_SPACE_PLANET` palette
///
/// The returned `GameObject` is sized to the visible body (`visual_r * 2` square).
/// `.planet(visual_r)` sets solid-circle collision AND makes this object a gravity
/// source via the engine's built-in planet system (no separate gravity_well needed).
/// Objects with `.gravity_target("space_planet")` are pulled toward it; the field
/// is bounded to `planet_radius × gravity_influence_mult` (default 3×) by the engine.
pub fn make_planet(
    ctx: &mut Context,
    id: &str,
    x: f32,
    y: f32,
    visual_r: f32,
    gravity_r: f32,
    color_idx: usize,
) -> GameObject {
    let _ = gravity_r; // sizing now driven by visual_r only
    let (pr, pg, pb) = C_SPACE_PLANET[color_idx % C_SPACE_PLANET.len()];
    // Body-only image: pass visual_r for both params so no extra ring padding
    let img = planet_img_cached(visual_r, visual_r, pr, pg, pb);
    let d = visual_r * 2.0;

    let mut obj = GameObject::build(id)
        .size(d, d)
        .position(x - visual_r, y - visual_r)
        .image(Image {
            shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
            image: img,
            color: None,
        })
        .tag("space_planet")
        // .planet() sets solid_circle collision + registers as a gravity source.
        // Receivers with gravity_target("space_planet") are attracted to this object.
        // The engine bounds the field to planet_radius × gravity_influence_mult (3× default).
        .planet(visual_r)
        .gravity_strength(SPACE_PLANET_GRAV_STRENGTH)
        .build(ctx);

    // Subtle glow matching planet color
    obj.set_glow(GlowConfig {
        color: Color(pr, pg, pb, 60),
        width: 18.0,
    });
    obj
}
