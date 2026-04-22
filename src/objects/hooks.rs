// ── objects/hooks.rs ──────────────────────────────────────────────────────────
use quartz::*;
use crate::constants::*;
use crate::images::*;

pub fn make_hook(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
            image: circle_cached(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2),
            color: None,
        }),
        (HOOK_R*2.0, HOOK_R*2.0),
        (x - HOOK_R, y - HOOK_R),
        vec!["hook".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    )
}
