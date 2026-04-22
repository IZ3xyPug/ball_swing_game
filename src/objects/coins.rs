// ── objects/coins.rs ──────────────────────────────────────────────────────────
use quartz::*;
use crate::constants::*;
use crate::images::*;

pub fn make_coin(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (COIN_R * 2.0, COIN_R * 2.0), 0.0),
            image: circle_cached(COIN_R as u32, C_COIN.0, C_COIN.1, C_COIN.2),
            color: None,
        }),
        (COIN_R * 2.0, COIN_R * 2.0),
        (x - COIN_R, y - COIN_R),
        vec!["coin".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    )
}

pub fn make_score_x2(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (SCORE_X2_W, SCORE_X2_H), 0.0),
            image: circle_cached((SCORE_X2_W * 0.5) as u32, 255, 220, 90),
            color: None,
        }),
        (SCORE_X2_W, SCORE_X2_H),
        (x, y),
        vec!["score_x2".into()],
        (0.0, 0.0),
        (1.0, 1.0),
        0.0,
    )
}
