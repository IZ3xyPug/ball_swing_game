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
/// * `gravity_r`  — radius of the gravitational sphere of influence
/// * `color_idx`  — index into `C_SPACE_PLANET` palette
///
/// The returned `GameObject` is sized to the gravity field (`gravity_r * 2` square)
/// and carries both the planet image and the `gravity_well` physics attribute.
pub fn make_planet(
    ctx: &mut Context,
    id: &str,
    x: f32,
    y: f32,
    visual_r: f32,
    gravity_r: f32,
    color_idx: usize,
) -> GameObject {
    let (pr, pg, pb) = C_SPACE_PLANET[color_idx % C_SPACE_PLANET.len()];
    let img = planet_img_cached(visual_r, gravity_r, pr, pg, pb);
    let d = gravity_r * 2.0;

    let mut obj = GameObject::build(id)
        .size(d, d)
        .position(x - gravity_r, y - gravity_r)
        .image(Image {
            shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
            image: img,
            color: None,
        })
        .tag("space_planet")
        // planet_radius = visual_r so the engine handles landing/collision on the
        // visible surface. gravity_influence_mult (default 3×) extends the field.
        .gravity_well(visual_r, SPACE_PLANET_GRAV_STRENGTH)
        .build(ctx);

    // Subtle glow matching planet color
    obj.set_glow(GlowConfig {
        color: Color(pr, pg, pb, 60),
        width: 18.0,
    });
    obj
}
