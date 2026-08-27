// ── objects/gravity_wells.rs ──────────────────────────────────────────────────
use quartz::*;

pub fn make_gravity_well(
    ctx: &mut Context,
    id: &str,
    x: f32,
    y: f32,
    radius: f32,
    strength: f32,
    visual_r: f32,
) -> GameObject {
    let d = visual_r * 2.0;
    // No static base image: the well must show ONLY its animated gwellon/gwelloff
    // sprite so it never lingers as a plain translucent purple circle.
    GameObject::build(id)
        .size(d, d)
        .position(x - visual_r, y - visual_r)
        .tag("gwell")
        .gravity_well(radius, strength)
        .build(ctx)
}
