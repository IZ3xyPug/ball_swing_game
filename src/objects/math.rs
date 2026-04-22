// ── objects/math.rs ───────────────────────────────────────────────────────────
// Collision geometry helpers used by the game's collision system.
// Pure Rust — no engine or image dependencies.

/// Circle vs oriented bounding box (OBB) — returns penetration vector or None.
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

/// Circle vs axis-aligned bounding box (AABB) — returns penetration vector or None.
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

    let left  = cx - rx;
    let right = (rx + rw) - cx;
    let up    = cy - ry;
    let down  = (ry + rh) - cy;
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
