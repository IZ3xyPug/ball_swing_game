// ── objects/rocket_pads.rs ────────────────────────────────────────────────────
// Rocket pad: rare special platform that launches the player into the space zone.
use quartz::*;
use crate::constants::*;
use crate::images::*;

pub fn make_rocket_pad(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    let corner_r = (ROCKET_PAD_H * 0.38).clamp(1.0, ROCKET_PAD_H * 0.5 - 1.0);
    let mut obj = GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::RoundedRectangle(0.0, (ROCKET_PAD_W, ROCKET_PAD_H), 0.0, corner_r),
            image: rocket_pad_image_cached(),
            color: None,
        }),
        (ROCKET_PAD_W, ROCKET_PAD_H),
        (x, y),
        vec!["rocket_pad".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    );
    obj.set_glow(GlowConfig {
        color: Color(C_ROCKET_PAD_GLOW.0, C_ROCKET_PAD_GLOW.1, C_ROCKET_PAD_GLOW.2, 160),
        width: 12.0,
    });
    obj
}
