// ── objects/gravity_wells.rs ──────────────────────────────────────────────────
use quartz::*;
use crate::constants::*;
use crate::images::*;

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
    let ring_img = gwell_ring_cached(
        visual_r,
        C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2,
        GWELL_RING_COUNT,
        200.0,
    );
    GameObject::build(id)
        .size(d, d)
        .position(x - visual_r, y - visual_r)
        .image(Image {
            shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
            image: ring_img,
            color: None,
        })
        .tag("gwell")
        .gravity_well(radius, strength)
        .build(ctx)
}
