// ── objects/spinners.rs ───────────────────────────────────────────────────────
use quartz::*;
use crate::constants::*;
use crate::images::*;

pub fn make_spinner(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::build(id)
        .size(SPINNER_W, SPINNER_H)
        .position(x, y)
        .image(Image {
            shape: ShapeType::Rectangle(0.0, (SPINNER_W, SPINNER_H), 0.0),
            image: spinner_image_cached(),
            color: None,
        })
        .tag("spinner")
        .tag("obstacle")
        .rotation_resistance(1.0)
        .build(ctx)
}
