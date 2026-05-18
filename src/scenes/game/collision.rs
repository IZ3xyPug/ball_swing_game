use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::zone_index_for_distance;
use crate::objects::*;
use crate::state::*;
use super::helpers::*;

pub fn tick_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    tick_spinner_collision(c, st);
    tick_gate_collision(c, st);
    tick_pad_bounce(c, st);
    tick_rocket_pad_collision(c, st);
    tick_asteroid_collision(c, st);
    tick_asteroid_asteroid_collision(c, st);
    tick_asteroid_pad_bounce(c, st);
    tick_asteroid_spinner_collision(c, st);
    tick_hook_player_impact(c, st);
    tick_freeze_hooks(c, st);
    tick_comet_warnings(c, st);
    tick_move_comets(c, st);
    tick_comet_player_collision(c, st);
}

/// Zero out momentum on all live hooks every tick so they are completely immovable.
fn tick_freeze_hooks(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let asteroid_mode = matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
    if !asteroid_mode { return; }
    let live_hooks = st.lock().unwrap().live_hooks.clone();
    for name in &live_hooks {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.momentum = (0.0, 0.0);
            obj.rotation_momentum = 0.0;
        }
    }
}

/// Sets state fields for unhook + queues canvas ops (rope hide, gravity restore).
/// Caller must drop the lock before processing the returned commands.
struct UnhookOps {
    prev_hook: String,
    zone_idx: usize,
    gravity_val: f32,
}

fn begin_unhook(s: &mut State) -> Option<UnhookOps> {
    if !s.hooked { return None; }
    let prev = s.active_hook.clone();
    let zone_idx = zone_index_for_distance(s.distance);
    let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
    let gdir = s.gravity_dir;
    s.hooked = false;
    s.active_hook = String::new();
    Some(UnhookOps { prev_hook: prev, zone_idx, gravity_val: GRAVITY * gravity_scale * gdir })
}

fn apply_unhook(c: &mut Canvas, ops: &UnhookOps) {
    c.run(Action::Hide { target: Target::name("rope") });
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.gravity = ops.gravity_val;
    }
    // Restore hook to base colour (or asteroid skin if mode is on).
    if !ops.prev_hook.is_empty() {
        let asteroid_mode = matches!(c.get_var("asteroid_hooks_on"), Some(Value::Bool(true)));
        if let Some(hobj) = c.get_game_object_mut(&ops.prev_hook) {
            if asteroid_mode {
                if let Some(sprite) = &mut hobj.animated_sprite { sprite.reset(); sprite.set_fps(0.001); }
            } else {
                let (r, g, b) = hook_base_for_obj(hobj, ops.zone_idx);
                hobj.set_image(hook_img(r, g, b));
            }
            hobj.clear_glow();
        }
    }
}

// ── Spinning obstacle collision ──────────────────────────────────────────────

fn tick_spinner_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !s.spinners_enabled { return; }

    let live = s.spinner_live.clone();
    for name in live {
        let hit_info = {
            if let Some(obj) = c.get_game_object(&name) {
                circle_hits_obb(
                    (s.px, s.py), PLAYER_R + 4.0,
                    obj.position, obj.size, obj.rotation,
                )
            } else { None }
        };

        if let Some((push_x, push_y)) = hit_info {
            s.px += push_x;
            s.py += push_y;

            let push_len = (push_x * push_x + push_y * push_y).sqrt().max(0.001);
            let nx = push_x / push_len;
            let ny = push_y / push_len;

            let inward = -(s.vx * nx + s.vy * ny);
            if inward > 0.0 {
                s.vx += nx * inward;
                s.vy += ny * inward;
            }

            if s.spinner_hit_cooldown == 0 {
                let push_mag = (SPINNER_HIT_PUSH_X * SPINNER_HIT_PUSH_X
                    + SPINNER_HIT_PUSH_Y * SPINNER_HIT_PUSH_Y).sqrt();
                s.vx += nx * push_mag;
                s.vy += ny * push_mag;
                s.spinner_hit_cooldown = 6;
                s.glow_flashes.push((name.clone(), 10));

                let unhook_ops = begin_unhook(&mut s);
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&name) {
                    obj.set_glow(GlowConfig { color: Color(255, 100, 80, 220), width: 8.0 });
                }
                if let Some(ref ops) = unhook_ops {
                    apply_unhook(c, ops);
                }

                s = st.lock().unwrap();
            }
        }
    }
}

// ── Gate collision ──────────────────────────────────────────────────────────

fn tick_gate_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if !GATES_ENABLED { return; }

    let live = s.gate_live.clone();
    for gate_id in live {
        let top_id = format!("{gate_id}_top");
        let bot_id = format!("{gate_id}_bot");
        for seg_id in [top_id, bot_id] {
            let hit_info = {
                if let Some(obj) = c.get_game_object(&seg_id) {
                    circle_hits_aabb(
                        (s.px, s.py), PLAYER_R + 2.0,
                        obj.position, obj.size,
                    )
                } else { None }
            };

            if let Some((push_x, push_y)) = hit_info {
                s.px += push_x;
                s.py += push_y;

                let push_len = (push_x * push_x + push_y * push_y).sqrt().max(0.001);
                let nx = push_x / push_len;
                let ny = push_y / push_len;
                let inward = -(s.vx * nx + s.vy * ny);
                if inward > 0.0 {
                    s.vx += nx * inward;
                    s.vy += ny * inward;
                }

                s.vx += nx * 4.0;
                s.vy += ny * 4.0;

                let unhook_ops = begin_unhook(&mut s);
                drop(s);
                if let Some(ref ops) = unhook_ops {
                    apply_unhook(c, ops);
                }
                s = st.lock().unwrap();
            }
        }
    }
}

// ── Pad bounce ──────────────────────────────────────────────────────────────

fn tick_pad_bounce(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let falling_down = s.gravity_dir > 0.0 && s.vy > 0.0;
    let falling_up   = s.gravity_dir < 0.0 && s.vy < 0.0;
    if !(falling_down || falling_up) { return; }

    let player_bottom = s.py + PLAYER_R;
    let player_top    = s.py - PLAYER_R;
    let mut bounced_pad: Option<(String, f32, f32)> = None;

    for name in &s.pad_live {
        if let Some(obj) = c.get_game_object(name) {
            if !obj.visible { continue; }
            let pad_left   = pad_collision_left(obj.position.0);
            let pad_w      = pad_collision_w();
            let pad_top    = obj.position.1;
            let pad_bottom = pad_top + PAD_H;
            let rounded_hit = circle_overlaps_rounded_rect(
                s.px,
                s.py,
                PLAYER_R,
                pad_left,
                pad_top,
                pad_w,
                PAD_H,
                pad_corner_radius(),
            );
            let hit = if falling_down {
                rounded_hit
                    && player_bottom >= pad_top
                    && player_bottom <= pad_top + PLAYER_R * 2.0 + s.vy.abs()
            } else {
                rounded_hit
                    && player_top <= pad_bottom
                    && player_top >= pad_bottom - PLAYER_R * 2.0 - s.vy.abs()
            };
            if hit { bounced_pad = Some((name.clone(), pad_top, pad_bottom)); break; }
        }
    }

    if let Some((pad_name, pad_top, pad_bottom)) = bounced_pad {
        s.vy = PAD_BOUNCE_VY * s.gravity_dir;
        if falling_down {
            s.py = pad_top - PLAYER_R;
        } else {
            s.py = pad_bottom + PLAYER_R;
        }

        let (new_px, new_py, new_vx, new_vy) = (s.px, s.py, s.vx, s.vy);
        let unhook_ops = begin_unhook(&mut s);
        // Restart one-shot tech_bounce playback for this pad.
        s.pad_bounce_anim.retain(|(id, _, _)| id != &pad_name);
        s.pad_bounce_anim.push((pad_name.clone(), 1, 0));
        drop(s);

        // Signal cap_momentum_and_write_back to skip capping this frame so the
        // full PAD_BOUNCE_VY is preserved regardless of current speed.
        c.set_var("post_bounce", true);

        if let Some(ref ops) = unhook_ops {
            apply_unhook(c, ops);
        }
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.position = (new_px - PLAYER_R, new_py - PLAYER_R);
            obj.momentum = (new_vx, new_vy);
        }
    }
}

// ── Rocket pad launch ─────────────────────────────────────────────────────────
// Circle-AABB hit test: player ball landing on top of a rocket pad launches
// the player upward with a big velocity boost and unhooks them.

pub fn tick_rocket_pad_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    // Rocket pads only work in normal mode (not in space already)
    if s.in_space_mode { return; }

    let player_bottom = s.py + PLAYER_R;
    let player_left   = s.px - PLAYER_R;
    let player_right  = s.px + PLAYER_R;
    let approaching   = s.gravity_dir > 0.0 && s.vy > 0.0;

    if !approaching { return; }

    let live = s.rocket_pad_live.clone();
    let mut hit_pad: Option<String> = None;
    for name in &live {
        if let Some(obj) = c.get_game_object(name) {
            let pad_top   = obj.position.1;
            let pad_left  = obj.position.0;
            let pad_right = obj.position.0 + ROCKET_PAD_W;
            let overlap_x = player_right > pad_left && player_left < pad_right;
            if overlap_x && player_bottom >= pad_top && player_bottom <= pad_top + ROCKET_PAD_H + s.vy.abs() {
                hit_pad = Some(name.clone());
                break;
            }
        }
    }

    if let Some(ref pad_name) = hit_pad {
        // Apply launch velocity — strong enough to clear the entire normal zone
        s.vy = ROCKET_PAD_LAUNCH_VY;  // large negative = upward
        s.vx += ROCKET_PAD_LAUNCH_VX;
        s.py = s.py - PLAYER_R * 0.5; // nudge up to avoid re-trigger
        // This is the ONLY place that unlocks the space zone entry check.
        s.space_launch_active = true;

        let unhook_ops = begin_unhook(&mut s);
        drop(s);

        if let Some(ref ops) = unhook_ops {
            apply_unhook(c, ops);
        }

        // Camera shake + flash
        if let Some(cam) = c.camera_mut() {
            cam.shake(55.0, 0.5);
            cam.flash_with(Color(C_ROCKET_PAD_GLOW.0, C_ROCKET_PAD_GLOW.1, C_ROCKET_PAD_GLOW.2, 160),
                0.5, FlashMode::Pulse, FlashEase::Sharp, 0.85, 0.0);
        }

        // Glowing highlight on the pad
        if let Some(obj) = c.get_game_object_mut(pad_name) {
            obj.set_glow(GlowConfig {
                color: Color(C_ROCKET_PAD.0, C_ROCKET_PAD.1, C_ROCKET_PAD.2, 240),
                width: 18.0,
            });
        }
    }
}

// ── Floating asteroid collision ──────────────────────────────────────────────

fn tick_asteroid_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    let live = s.space_asteroid_live.clone();

    for name in live {
        // Circle-circle overlap: treat each asteroid as a circle of radius = half its size.
        let hit_info: Option<(f32, f32)> = {
            if let Some(obj) = c.get_game_object(&name) {
                let ax = obj.position.0 + obj.size.0 * 0.5;
                let ay = obj.position.1 + obj.size.1 * 0.5;
                let asteroid_r = obj.size.0 * 0.30;
                let min_dist = PLAYER_R + 6.0 + asteroid_r;
                let dx = s.px - ax;
                let dy = s.py - ay;
                let dist2 = dx * dx + dy * dy;
                if dist2 < min_dist * min_dist {
                    let dist = dist2.sqrt().max(0.001);
                    let push = min_dist - dist;
                    Some((dx / dist * push, dy / dist * push))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((push_x, push_y)) = hit_info {
            // Push player out of overlap.
            s.px += push_x;
            s.py += push_y;

            let push_len = (push_x * push_x + push_y * push_y).sqrt().max(0.001);
            let nx = push_x / push_len;
            let ny = push_y / push_len;

            // Strip inward velocity component (elastic deflect).
            let inward = -(s.vx * nx + s.vy * ny);
            if inward > 0.0 {
                s.vx += nx * inward;
                s.vy += ny * inward;
            }

            // Tangential component of player velocity drives asteroid spin.
            let tx = -ny;
            let ty = nx;
            let tang = s.vx * tx + s.vy * ty;
            let spin_impulse = tang * 0.006;

            drop(s);

            if let Some(obj) = c.get_game_object_mut(&name) {
                obj.rotation_momentum += spin_impulse;
                // Nudge the asteroid away from the player on impact.
                obj.momentum.0 += -nx * 1.5;
                obj.momentum.1 += -ny * 1.5;
            }

            s = st.lock().unwrap();
        }
    }
}

// ── Asteroid–Asteroid Collision ─────────────────────────────────────────────
// Manual circle-circle check run each game tick.  Crystalline's dynamic-dynamic
// path is correct but asteroids spawn 1300-2800 px apart and close too slowly
// to collide during normal play.  This check works directly on live object state
// so collisions are detected and resolved the moment any two circles overlap.

fn tick_asteroid_asteroid_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let ast_live = {
        let s = st.lock().unwrap();
        s.space_asteroid_live.clone()
    };
    let live = ast_live;
    if live.len() < 2 { return; }

    // Snapshot: all values needed for contact math, read before any mutation.
    struct Snap { name: String, cx: f32, cy: f32, r: f32, mass: f32, mx: f32, my: f32 }
    let snaps: Vec<Snap> = live.iter().filter_map(|name| {
        let obj = c.get_game_object(name)?;
        if !obj.visible { return None; }
        Some(Snap {
            name: name.clone(),
            cx:   obj.position.0 + obj.size.0 * 0.5,
            cy:   obj.position.1 + obj.size.1 * 0.5,
            r:    obj.size.0 * 0.38,
            mass: (obj.material.density * obj.size.0 * obj.size.1).max(0.001),
            mx:   obj.momentum.0,
            my:   obj.momentum.1,
        })
    }).collect();

    let n = snaps.len();
    if n < 2 { return; }

    // Accumulated corrections per asteroid: (pos_dx, pos_dy, mom_dx, mom_dy)
    let mut corr: Vec<(f32, f32, f32, f32)> = vec![(0.0, 0.0, 0.0, 0.0); n];

    for i in 0..n {
        for j in (i + 1)..n {
            let a = &snaps[i];
            let b = &snaps[j];
            let dx = a.cx - b.cx;
            let dy = a.cy - b.cy;
            let dist2 = dx * dx + dy * dy;
            let min_dist = a.r + b.r;
            if dist2 >= min_dist * min_dist { continue; }

            let dist = dist2.sqrt().max(0.001);
            let pen  = min_dist - dist;
            // nx,ny: unit normal pointing from B's centre toward A's centre.
            let nx = dx / dist;
            let ny = dy / dist;

            // Position correction split by opposite-mass ratio so heavy bodies
            // barely move and light bodies are pushed away.
            let total = a.mass + b.mass;
            let ra = b.mass / total; // fraction A moves
            let rb = a.mass / total; // fraction B moves
            corr[i].0 += nx * pen * ra;
            corr[i].1 += ny * pen * ra;
            corr[j].0 -= nx * pen * rb;
            corr[j].1 -= ny * pen * rb;

            // Velocity impulse — only when approaching (rel velocity toward each other).
            // approach = (va - vb) · (direction from A to B) = dot with (-nx,-ny).
            let rel_vn = (a.mx - b.mx) * (-nx) + (a.my - b.my) * (-ny);
            if rel_vn > 0.0 {
                let inv_a = 1.0 / a.mass;
                let inv_b = 1.0 / b.mass;
                // e = 0.5 (partially elastic rock-on-rock).
                let j_imp = rel_vn * 1.5 / (inv_a + inv_b);
                // Push A away from B (+nx direction) and B away from A (-nx direction).
                corr[i].2 += nx * j_imp * inv_a;
                corr[i].3 += ny * j_imp * inv_a;
                corr[j].2 -= nx * j_imp * inv_b;
                corr[j].3 -= ny * j_imp * inv_b;
            }
        }
    }

    // Write accumulated corrections back to each asteroid object.
    for (snap, (pdx, pdy, mdx, mdy)) in snaps.iter().zip(corr.iter()) {
        if pdx.abs() < 0.001 && pdy.abs() < 0.001
            && mdx.abs() < 0.001 && mdy.abs() < 0.001
        {
            continue;
        }
        if let Some(obj) = c.get_game_object_mut(&snap.name) {
            obj.position.0 += pdx;
            obj.position.1 += pdy;
            obj.momentum.0 += mdx;
            obj.momentum.1 += mdy;
        }
    }
}

// ── Asteroid–Pad collision ───────────────────────────────────────────────────
// Each live space asteroid is treated as a circle; bounce it off any pad top/bottom.

pub fn tick_asteroid_pad_bounce(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (pads, ast_live) = {
        let s = st.lock().unwrap();
        (s.pad_live.clone(), s.space_asteroid_live.clone())
    };
    let asteroids = ast_live;

    for ast_name in &asteroids {
        let snap = {
            if let Some(obj) = c.get_game_object(ast_name) {
                if !obj.visible { continue; }
                let cx = obj.position.0 + obj.size.0 * 0.5;
                let cy = obj.position.1 + obj.size.1 * 0.5;
                let r  = obj.size.0 * 0.38;
                let mx = obj.momentum.0;
                let my = obj.momentum.1;
                (cx, cy, r, mx, my)
            } else { continue; }
        };
        let (cx, cy, r, mx, my) = snap;

        for pad_name in &pads {
            let hit = {
                if let Some(obj) = c.get_game_object(pad_name) {
                    if !obj.visible { continue; }
                    let pad_x = pad_collision_left(obj.position.0);
                    let pad_w = pad_collision_w();
                    let pad_y = obj.position.1;
                    circle_overlaps_rounded_rect(cx, cy, r, pad_x, pad_y, pad_w, PAD_H, pad_corner_radius())
                } else { false }
            };
            if !hit { continue; }

            // Read pad_top before the mutable borrow on the asteroid.
            let pad_top = if let Some(p) = c.get_game_object(pad_name) { p.position.1 } else { continue; };

            // Reflect the y component of momentum (simple top/bottom bounce).
            let new_my = -my * PAD_ASTEROID_RESTITUTION;
            if let Some(obj) = c.get_game_object_mut(ast_name) {
                obj.momentum = (mx, new_my);
                // Push asteroid out upward or downward based on momentum direction.
                let shift = if my > 0.0 { -r - 2.0 } else { r + 2.0 };
                obj.position.1 = pad_top + shift;
            }
            break; // one pad collision per asteroid per tick is sufficient
        }
    }
}

// ── Asteroid–Spinner collision ───────────────────────────────────────────────
// Each live asteroid treated as a circle; deflect off rotating spinner OBBs.

pub fn tick_asteroid_spinner_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let (spinners_enabled, spinners, ast_live) = {
        let s = st.lock().unwrap();
        (s.spinners_enabled, s.spinner_live.clone(), s.space_asteroid_live.clone())
    };
    if !spinners_enabled { return; }
    let asteroids = ast_live;

    for ast_name in &asteroids {
        let snap = {
            if let Some(obj) = c.get_game_object(ast_name) {
                if !obj.visible { continue; }
                let cx = obj.position.0 + obj.size.0 * 0.5;
                let cy = obj.position.1 + obj.size.1 * 0.5;
                let r  = obj.size.0 * 0.38;
                let mx = obj.momentum.0;
                let my = obj.momentum.1;
                (cx, cy, r, mx, my)
            } else { continue; }
        };
        let (cx, cy, r, mx, my) = snap;

        for sp_name in &spinners {
            let hit_info = {
                if let Some(obj) = c.get_game_object(sp_name) {
                    circle_hits_obb(
                        (cx, cy), r + 2.0,
                        obj.position, obj.size, obj.rotation,
                    )
                } else { None }
            };
            if let Some((push_x, push_y)) = hit_info {
                let len = (push_x * push_x + push_y * push_y).sqrt().max(0.001);
                let nx = push_x / len;
                let ny = push_y / len;
                let inward = -(mx * nx + my * ny);
                let reflect_x = mx + nx * inward.max(0.0) * 2.0;
                let reflect_y = my + ny * inward.max(0.0) * 2.0;
                if let Some(obj) = c.get_game_object_mut(ast_name) {
                    obj.position.0 += push_x;
                    obj.position.1 += push_y;
                    obj.momentum = (reflect_x, reflect_y);
                }
                break; // one spinner collision per asteroid per tick
            }
        }
    }
}

// ── Asteroid-hook / player collision ─────────────────────────────────────────
// In asteroid-hook mode the hooks are solid obstacles.  When the player body
// overlaps a hook circle we push the player out and nudge the hook away —
// identical treatment to the floating asteroid gifs.
// Hooks the player is currently grappled to are skipped so rope physics isn't
// disturbed while swinging.

pub fn tick_hook_player_impact(_c: &mut Canvas, _st: &Arc<Mutex<State>>) {
    // Grab hooks are non-collidable — player and all objects phase through them.
}

// ── Comet warning tick ──────────────────────────────────────────────────
// Advances warning animations; when a warning expires it activates its comet.

fn tick_comet_warnings(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.comet_warn_live.is_empty() { return; }

    let player_px = s.px;
    let player_py = s.py;

    // Advance timers.
    for w in &mut s.comet_warn_live {
        w.timer += 1;
    }

    // Separate finished warnings from in-progress.
    let mut to_spawn: Vec<CometWarn> = Vec::new();
    for w in &s.comet_warn_live {
        if w.timer >= COMET_WARN_TOTAL {
            to_spawn.push(w.clone());
        }
    }
    s.comet_warn_live.retain(|w| w.timer < COMET_WARN_TOTAL);

    // Snapshot still-active warnings for visual updates.
    let active: Vec<CometWarn> = s.comet_warn_live.clone();
    drop(s);

    // Camera bounds in world space — used to keep warnings visible on screen.
    let (cam_left, cam_top, cam_zoom) = if let Some(cam) = c.camera() {
        (cam.position.0, cam.position.1, cam.zoom.max(0.1))
    } else {
        (0.0, 0.0, 1.0)
    };
    let view_w = VW / cam_zoom;
    let view_h = VH / cam_zoom;
    // Margin from the camera edge where the warning can be placed (world units).
    let margin = 80.0;

    // Update in-progress warnings: follow player, scale during phase-2 intro.
    // Position is clamped to the visible viewport so the indicator is always visible.
    // The indicator is placed near the screen edge in the direction the comet comes from.
    for w in &active {
        let img = warn_image_for_timer(w.timer);
        let scale = warn_size_scale(w.timer);
        let w_scaled = COMET_WARN_W * scale;
        let h_scaled = COMET_WARN_H * scale;

        // Direction from player toward comet spawn (comet comes FROM that direction).
        // h_offset is lateral, v_offset is how far above the player.
        let dir_dx = w.h_offset;
        let dir_dy = -w.v_offset; // negative = upward in world space
        let dir_len = (dir_dx * dir_dx + dir_dy * dir_dy).sqrt().max(1.0);
        let ndx = dir_dx / dir_len;
        let ndy = dir_dy / dir_len;

        // Desired center: project from player along direction, clamped to viewport.
        let cam_right  = cam_left + view_w;
        let cam_bottom = cam_top  + view_h;

        // The farthest we can move inside the viewport from its center toward the comet.
        // We want the indicator near the edge in that direction.
        let max_x = if ndx > 0.0 { cam_right  - margin - w_scaled * 0.5 }
                    else          { cam_left   + margin + w_scaled * 0.5 };
        let max_y = if ndy > 0.0 { cam_bottom - margin - h_scaled * 0.5 }
                    else          { cam_top    + margin + h_scaled * 0.5 };

        // Center of the warning indicator in world space, clamped to viewport.
        let cx = (player_px + ndx * view_w * 0.45)
            .clamp(cam_left   + margin + w_scaled * 0.5, cam_right  - margin - w_scaled * 0.5);
        let cy = (player_py + ndy * view_h * 0.45)
            .clamp(cam_top    + margin + h_scaled * 0.5, cam_bottom - margin - h_scaled * 0.5);
        let _ = (max_x, max_y); // suppress unused warning

        let x = cx - w_scaled * 0.5;
        let y = cy - h_scaled * 0.5;
        if let Some(obj) = c.get_game_object_mut(&w.warn_obj_id) {
            obj.set_image(img);
            obj.size = (w_scaled, h_scaled);
            obj.position = (x, y);
        }
    }

    // Spawn comets whose warning has finished.
    // Comet spawn position is recalculated from the player's CURRENT position + stored offsets.
    for w in &to_spawn {
        // Hide and recycle the warning object.
        if let Some(obj) = c.get_game_object_mut(&w.warn_obj_id) {
            obj.visible = false;
            obj.position = (-9500.0, -9500.0);
        }
        // Calculate final spawn from current player position.
        let spawn_x = player_px + w.h_offset;
        let spawn_y = player_py - w.v_offset;
        let dx = player_px - spawn_x;
        let dy = player_py - spawn_y;
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let vx = dx / len * COMET_SPEED;
        let vy = dy / len * COMET_SPEED;
        let rotation = vy.atan2(vx).to_degrees() + 180.0;

        // Activate the comet.
        if let Some(obj) = c.get_game_object_mut(&w.comet_id) {
            obj.set_animation(super::spawning::comet_template());
            obj.position = (spawn_x - COMET_SIZE * 0.5, spawn_y - COMET_SIZE * 0.5);
            obj.size = (COMET_SIZE, COMET_SIZE);
            obj.rotation = rotation;
            obj.gravity = 0.0;
            obj.momentum = (0.0, 0.0);
            obj.rotation_momentum = 0.0;
            obj.collision_mode = CollisionMode::NonPlatform;
            obj.visible = true;
        }
        let mut s = st.lock().unwrap();
        s.warn_free.push(w.warn_obj_id.clone());
        s.comet_live.push((w.comet_id.clone(), vx, vy, COMET_LIFETIME));
    }
}

/// Returns the correct warning image based on elapsed ticks.
fn warn_image_for_timer(timer: u32) -> Image {
    if timer < COMET_WARN_P1_END {
        // Phase 1: fast alternation between dark and light.
        if (timer / COMET_WARN_ALT) % 2 == 0 { warn_img_dark() } else { warn_img_light() }
    } else {
        // Phase 2: light_explode (scaled) → dark_explode → light_explode.
        let sub = timer - COMET_WARN_P1_END;
        if sub < COMET_WARN_P2_A {
            warn_img_light_explode()
        } else if sub < COMET_WARN_P2_B {
            warn_img_dark_explode()
        } else {
            warn_img_light_explode()
        }
    }
}

/// Returns size scale for the warning indicator.
/// Only phase-2 intro (first third of P2_A) gets scaled up.
fn warn_size_scale(timer: u32) -> f32 {
    if timer < COMET_WARN_P1_END { return 1.0; }
    let sub = timer - COMET_WARN_P1_END;
    if sub < COMET_WARN_P2_A {
        let third = COMET_WARN_P2_A / 3; // ~6 ticks each step
        if sub < third { 2.0 }
        else if sub < third * 2 { 1.5 }
        else { 1.0 }
    } else {
        1.0
    }
}

// ── Comets movement ───────────────────────────────────────────────────────────

fn tick_move_comets(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let move_list: Vec<(String, f32, f32)>;
    let recycle: Vec<String>;
    {
        let mut s = st.lock().unwrap();
        if s.comet_live.is_empty() { return; }

        let mut mv: Vec<(String, f32, f32)> = Vec::new();
        let mut rc: Vec<String> = Vec::new();
        for (name, vx, vy, ttl) in &mut s.comet_live {
            mv.push((name.clone(), *vx, *vy));
            if *ttl > 0 { *ttl -= 1; }
            if *ttl == 0 { rc.push(name.clone()); }
        }
        for id in &rc {
            s.comet_live.retain(|(n, _, _, _)| n != id);
            s.comet_free.push(id.clone());
        }
        move_list = mv;
        recycle = rc;
    }

    for (name, vx, vy) in &move_list {
        if let Some(obj) = c.get_game_object_mut(name) {
            obj.position.0 += vx;
            obj.position.1 += vy;
        }
    }

    for id in &recycle {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-9000.0, -9000.0);
        }
    }
}

// ── Comet → player collision ──────────────────────────────────────────────────

fn tick_comet_player_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    let mut s = st.lock().unwrap();
    if s.comet_live.is_empty() || s.dead { return; }

    let px = s.px;
    let py = s.py;
    let hit_r = PLAYER_R + COMET_HIT_RADIUS;

    let live_snapshot: Vec<(String, f32, f32)> = s.comet_live
        .iter()
        .map(|(n, vx, vy, _)| (n.clone(), *vx, *vy))
        .collect();

    let mut hit_ids: Vec<(String, f32, f32)> = Vec::new(); // (id, vx, vy)
    for (name, vx, vy) in &live_snapshot {
        if let Some(obj) = c.get_game_object(name) {
            let cx = obj.position.0 + COMET_SIZE * 0.5;
            let cy = obj.position.1 + COMET_SIZE * 0.5;
            let dx = px - cx;
            let dy = py - cy;
            if dx * dx + dy * dy < hit_r * hit_r {
                hit_ids.push((name.clone(), *vx, *vy));
            }
        }
    }

    if hit_ids.is_empty() { return; }

    // Recycle hit comets.
    for (id, _, _) in &hit_ids {
        s.comet_live.retain(|(n, _, _, _)| n != id);
        s.comet_free.push(id.clone());
    }

    // Use the first hit comet's velocity direction for knockback.
    let (_, kvx, kvy) = &hit_ids[0];
    let klen = (kvx * kvx + kvy * kvy).sqrt().max(1.0);
    let nx = kvx / klen;
    let ny = kvy / klen;
    let kbx = nx * COMET_KNOCKBACK;
    let kby = ny * COMET_KNOCKBACK;

    s.vx = kbx;
    s.vy = kby;

    let was_hooked = s.hooked;
    let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };
    let gdir = s.gravity_dir;
    if was_hooked {
        s.hooked = false;
        s.active_hook = String::new();
    }
    drop(s);

    if was_hooked {
        c.run(Action::Hide { target: Target::name("rope") });
        if let Some(obj) = c.get_game_object_mut("player") {
            obj.gravity = GRAVITY * gravity_scale * gdir;
        }
    }
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.momentum = (kbx, kby);
    }

    // Hide recycled comets.
    for (id, _, _) in &hit_ids {
        if let Some(obj) = c.get_game_object_mut(id) {
            obj.visible = false;
            obj.position = (-9000.0, -9000.0);
        }
    }
}
