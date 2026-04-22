// ── objects/pickups.rs ────────────────────────────────────────────────────────
// Gravity-flip token and zero-gravity token.
use quartz::*;
use crate::constants::*;
use crate::images::*;

pub fn make_flip(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (FLIP_W, FLIP_H), 0.0),
            image: flip_image_cached(),
            color: None,
        }),
        (FLIP_W, FLIP_H),
        (x, y),
        vec!["flip".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    )
}

pub fn make_zero_g(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (ZERO_G_W, ZERO_G_H), 0.0),
            image: circle_cached((ZERO_G_W * 0.5) as u32, 135, 220, 255),
            color: None,
        }),
        (ZERO_G_W, ZERO_G_H),
        (x, y),
        vec!["zero_g".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    )
}
