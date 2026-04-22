// ── objects/gates.rs ──────────────────────────────────────────────────────────
use quartz::*;
use std::sync::Arc;
use crate::constants::*;

pub fn make_gate_segment(
    ctx: &mut Context,
    id: &str,
    x: f32,
    y: f32,
    h: f32,
    image: Arc<image::RgbaImage>,
) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (GATE_W, h), 0.0),
            image,
            color: None,
        }),
        (GATE_W, h),
        (x, y),
        vec!["gate".into(), "obstacle".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    )
}
