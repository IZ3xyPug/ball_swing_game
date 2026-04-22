// ── objects/turrets.rs ────────────────────────────────────────────────────────
use quartz::*;
use crate::constants::*;
use crate::images::*;

pub fn make_turret(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    let s = TURRET_FULL_SIZE;
    let img = turret_img(
        TURRET_R as u32,
        TURRET_BARREL_LEN as u32,
        TURRET_BARREL_W as u32,
        C_TURRET_BODY,
        C_TURRET_BARREL,
    );
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (s, s), 0.0),
            image: img.into(),
            color: None,
        }),
        (s, s),
        (x - s * 0.5, y - s * 0.5),
        vec!["turret".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    )
}

pub fn make_turret_bullet(ctx: &mut Context, id: &str) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (BULLET_W, BULLET_H), 0.0),
            image: solid(C_TURRET_BULLET.0, C_TURRET_BULLET.1, C_TURRET_BULLET.2, 255).into(),
            color: None,
        }),
        (BULLET_W, BULLET_H),
        (-5000.0, -5000.0),
        vec!["bullet".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    )
}
