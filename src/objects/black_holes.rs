// ── objects/black_holes.rs ────────────────────────────────────────────────────
// Space zone black holes: always-on high-gravity wells with subtle dark aesthetic.
// Unlike pulsing gravity wells, black holes are always active.
// The effective gravity field extends exactly to the image boundary (radius).
use quartz::*;
use crate::constants::*;
use crate::images::*;

/// Build a black hole game object.
///
/// * `id`        — registration key
/// * `x`, `y`   — world center position (negative y for space zone)
/// * `radius`   — both the visual image radius AND the gravity field radius
pub fn make_black_hole(
    ctx: &mut Context,
    id: &str,
    x: f32,
    y: f32,
    radius: f32,
) -> GameObject {
    let img = black_hole_img_cached(radius);
    let d = radius * 2.0;

    let mut obj = GameObject::build(id)
        .size(d, d)
        .position(x - radius, y - radius)
        .image(Image {
            shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
            image: img,
            color: None,
        })
        .tag("space_blackhole")
        .gravity_well(radius, SPACE_BLACKHOLE_GRAV_STRENGTH)
        .build(ctx);

    // Barely-visible dark purple glow to hint at presence
    obj.set_glow(GlowConfig {
        color: Color(40, 10, 60, 45),
        width: 20.0,
    });
    obj
}
