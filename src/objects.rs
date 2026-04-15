use quartz::*;
use std::sync::Arc;
use crate::constants::*;
use crate::images::*;

pub fn make_hook(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
            image: circle_img(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2).into(),
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

pub fn make_pad(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (PAD_W, PAD_H), 0.0),
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

pub fn make_coin(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (COIN_R * 2.0, COIN_R * 2.0), 0.0),
            image: circle_img(COIN_R as u32, C_COIN.0, C_COIN.1, C_COIN.2).into(),
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

pub fn make_score_x2(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (SCORE_X2_W, SCORE_X2_H), 0.0),
            image: circle_img((SCORE_X2_W * 0.5) as u32, 255, 220, 90).into(),
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

pub fn make_zero_g(ctx: &mut Context, id: &str, x: f32, y: f32) -> GameObject {
    GameObject::new_rect(
        ctx,
        id.into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (ZERO_G_W, ZERO_G_H), 0.0),
            image: circle_img((ZERO_G_W * 0.5) as u32, 135, 220, 255).into(),
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

pub fn make_gravity_well(ctx: &mut Context, id: &str, x: f32, y: f32, radius: f32, strength: f32, visual_r: f32) -> GameObject {
    let d = visual_r * 2.0;
    let ring_img = gwell_ring_img(visual_r, C_GWELL_ACTIVE.0, C_GWELL_ACTIVE.1, C_GWELL_ACTIVE.2, GWELL_RING_COUNT, 200.0);
    GameObject::build(id)
        .size(d, d)
        .position(x - visual_r, y - visual_r)
        .image(Image {
            shape: ShapeType::Ellipse(0.0, (d, d), 0.0),
            image: ring_img.into(),
            color: None,
        })
        .tag("gwell")
        .gravity_well(radius, strength)
        .build(ctx)
}

pub fn ui_text_spec(text: &str, font: &Font, font_size: f32, color: Color, width: f32) -> Text {
    Text::new(
        vec![Span::new(
            text.to_string(),
            font_size,
            Some(font_size * 1.25),
            Arc::new(font.clone()),
            color,
            0.0,
        )],
        Some(width),
        Align::Center,
        None,
    )
}

pub fn circle_hits_obb(
    circle_center: (f32, f32),
    circle_radius: f32,
    rect_pos: (f32, f32),
    rect_size: (f32, f32),
    rect_rotation_deg: f32,
) -> Option<(f32, f32)> {
    let (cx, cy) = circle_center;
    let (rx, ry) = rect_pos;
    let (rw, rh) = rect_size;

    let half_w = rw * 0.5;
    let half_h = rh * 0.5;
    let rcx = rx + half_w;
    let rcy = ry + half_h;

    let theta = rect_rotation_deg.to_radians();
    let cos_t = theta.cos();
    let sin_t = theta.sin();
    let dx = cx - rcx;
    let dy = cy - rcy;

    let local_x = dx * cos_t + dy * sin_t;
    let local_y = -dx * sin_t + dy * cos_t;

    let closest_x = local_x.clamp(-half_w, half_w);
    let closest_y = local_y.clamp(-half_h, half_h);
    let diff_x = local_x - closest_x;
    let diff_y = local_y - closest_y;

    let dist_sq = diff_x * diff_x + diff_y * diff_y;
    if dist_sq > circle_radius * circle_radius {
        return None;
    }

    let dist = dist_sq.sqrt();
    let (local_nx, local_ny, penetration) = if dist > 0.001 {
        let pen = circle_radius - dist;
        (diff_x / dist, diff_y / dist, pen)
    } else {
        let push_pos_x = half_w - local_x;
        let push_neg_x = half_w + local_x;
        let push_pos_y = half_h - local_y;
        let push_neg_y = half_h + local_y;

        let min_push = push_pos_x.min(push_neg_x).min(push_pos_y).min(push_neg_y);
        if min_push == push_pos_x {
            (1.0, 0.0, push_pos_x + circle_radius)
        } else if min_push == push_neg_x {
            (-1.0, 0.0, push_neg_x + circle_radius)
        } else if min_push == push_pos_y {
            (0.0, 1.0, push_pos_y + circle_radius)
        } else {
            (0.0, -1.0, push_neg_y + circle_radius)
        }
    };

    let world_nx = local_nx * cos_t - local_ny * sin_t;
    let world_ny = local_nx * sin_t + local_ny * cos_t;
    Some((world_nx * penetration, world_ny * penetration))
}

pub fn circle_hits_aabb(
    circle_center: (f32, f32),
    circle_radius: f32,
    rect_pos: (f32, f32),
    rect_size: (f32, f32),
) -> Option<(f32, f32)> {
    let (cx, cy) = circle_center;
    let (rx, ry) = rect_pos;
    let (rw, rh) = rect_size;

    let closest_x = cx.clamp(rx, rx + rw);
    let closest_y = cy.clamp(ry, ry + rh);
    let dx = cx - closest_x;
    let dy = cy - closest_y;
    let dist_sq = dx * dx + dy * dy;
    if dist_sq > circle_radius * circle_radius {
        return None;
    }

    let dist = dist_sq.sqrt();
    if dist > 0.001 {
        let pen = circle_radius - dist;
        return Some((dx / dist * pen, dy / dist * pen));
    }

    let left = cx - rx;
    let right = (rx + rw) - cx;
    let up = cy - ry;
    let down = (ry + rh) - cy;
    let min_axis = left.min(right).min(up).min(down);
    if min_axis == left {
        Some((-(left + circle_radius), 0.0))
    } else if min_axis == right {
        Some((right + circle_radius, 0.0))
    } else if min_axis == up {
        Some((0.0, -(up + circle_radius)))
    } else {
        Some((0.0, down + circle_radius))
    }
}
