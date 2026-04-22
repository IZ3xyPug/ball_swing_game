// ── objects/pads.rs ───────────────────────────────────────────────────────────
use quartz::*;
use crate::constants::*;
use crate::images::*;

pub fn make_pad(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    let corner_r = pad_corner_radius();
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::RoundedRectangle(0.0, (PAD_W, PAD_H), 0.0, corner_r),
            image: pad_image_cached(),
            color: None,
        }),
        (PAD_W, PAD_H),
        (x, y),
        vec!["pad".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    )
}
