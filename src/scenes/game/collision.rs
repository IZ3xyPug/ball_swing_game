use quartz::*;
use quartz::Timer;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::zone_index_for_distance;
use crate::images::*;
use crate::objects::*;
use crate::state::*;
use super::helpers::*;

pub fn tick_collision(c: &mut Canvas, st: &Arc<Mutex<State>>) {
    tick_spinner_collision(c, st);
    tick_gate_collision(c, st);
    tick_pad_bounce(c, st);
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
    c.release_grapple("player");
    if let Some(obj) = c.get_game_object_mut("player") {
        obj.gravity = ops.gravity_val;
    }
    // Restore hook to base colour.
    if !ops.prev_hook.is_empty() {
        if let Some(hobj) = c.get_game_object_mut(&ops.prev_hook) {
            let (r, g, b) = hook_base_for_zone(ops.zone_idx);
            hobj.set_image(hook_img(r, g, b));
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

                // Capture burst info before dropping the lock.
                let hit_pos = (s.px, s.py);
                let burst_id = format!("burst_{}", s.burst_counter);
                s.burst_counter = s.burst_counter.wrapping_add(1);
                s.burst_emitters.push((burst_id.clone(), 4));
                // Trigger mega shader explosive-sparks overlay.
                s.spinner_hit_vfx_timer = 24;
                s.spinner_hit_pos = hit_pos;

                let unhook_ops = begin_unhook(&mut s);
                let can_shake = s.shake_cooldown.is_finished();
                if can_shake {
                    s.shake_cooldown = Timer::new(0.5);
                }
                drop(s);

                // Kill air-barrier immediately so it's already off before the
                // camera starts moving, preventing UV-position drift.
                if can_shake {
                    c.disable_air_barrier();
                }

                if let Some(obj) = c.get_game_object_mut(&name) {
                    obj.set_glow(GlowConfig { color: Color(255, 100, 80, 220), width: 8.0 });
                }

                // Spawn a lit particle burst at the impact point.
                let burst = EmitterBuilder::new(&burst_id)
                    .origin(hit_pos.0, hit_pos.1)
                    .rate(400.0)
                    .lifetime(0.35)
                    .spread(140.0, 140.0)
                    .size(5.0)
                    .color(255, 130, 60, 240)
                    .gravity_scale(0.3)
                    .render_layer(3)
                    .build();
                c.add_emitter(burst);

                // Brief flash light at impact.
                if c.has_lighting() {
                    let flash_light = LightSource::new(
                        format!("flash_{}", burst_id),
                        hit_pos,
                        Color(255, 140, 60, 255),
                        520.0,
                        4.5,
                    ).with_shadows(false).with_effect(LightEffect::FadeOut {
                        duration: 0.25,
                    });
                    c.add_light(flash_light);
                }
                if let Some(ref ops) = unhook_ops {
                    apply_unhook(c, ops);
                }

                // Camera shake on spinner hit (cooldown prevents spam).
                if can_shake {
                    if let Some(cam) = c.camera_mut() {
                        cam.shake(60.0, 0.25);
                    }
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
                let can_flash = s.flash_cooldown.is_finished();
                if can_flash {
                    s.flash_cooldown = Timer::new(0.4);
                }
                drop(s);
                if let Some(ref ops) = unhook_ops {
                    apply_unhook(c, ops);
                }

                // Kill air-barrier immediately before camera flash activates.
                if can_flash {
                    c.disable_air_barrier();
                }

                // Red flash on gate hit (cooldown prevents spam).
                if can_flash {
                    if let Some(cam) = c.camera_mut() {
                        cam.flash(Color(255, 60, 40, 180), 0.2);
                    }
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
    if !s.bounce_enabled || !(falling_down || falling_up) { return; }

    let player_bottom = s.py + PLAYER_R;
    let player_top    = s.py - PLAYER_R;
    let player_left   = s.px - PLAYER_R;
    let player_right  = s.px + PLAYER_R;
    let mut bounced_pad: Option<(String, f32, f32)> = None; // (name, pad_top, pad_bot)

    for name in &s.pad_live {
        if let Some(obj) = c.get_game_object(name) {
            let pad_top    = obj.position.1;
            let pad_bottom = obj.position.1 + PAD_H;
            let pad_left   = obj.position.0;
            let pad_right  = obj.position.0 + PAD_W;
            let overlap_x = player_right > pad_left && player_left < pad_right;
            let hit = if falling_down {
                overlap_x && player_bottom >= pad_top && player_bottom <= pad_top + PAD_H + s.vy.abs()
            } else {
                overlap_x && player_top <= pad_bottom && player_top >= pad_bottom - PAD_H - s.vy.abs()
            };
            if hit { bounced_pad = Some((name.clone(), pad_top, pad_bottom)); break; }
        }
    }

    if let Some((pad_name, pad_top, pad_bottom)) = bounced_pad {
        let bounce_factor = (1.0 - s.pad_bounce_count as f32 * PAD_BOUNCE_DECAY).max(PAD_BOUNCE_MIN_FACTOR);
        s.vy = PAD_BOUNCE_VY_START * bounce_factor * s.gravity_dir;
        s.pad_bounce_count = s.pad_bounce_count.saturating_add(1);

        if falling_down {
            s.py = pad_top - PLAYER_R;
        } else {
            s.py = pad_bottom + PLAYER_R;
        }

        let unhook_ops = begin_unhook(&mut s);
        let zone_idx = zone_index_for_distance(s.distance);
        s.glow_flashes.push((pad_name.clone(), 12));

        // Capture burst info before dropping the lock.
        let hit_pos = (s.px, s.py);
        let burst_id = format!("burst_{}", s.burst_counter);
        s.burst_counter = s.burst_counter.wrapping_add(1);
        s.burst_emitters.push((burst_id.clone(), 4));
        // Trigger mega shader shockwave overlay.
        s.pad_hit_vfx_timer = 24;
        s.pad_hit_pos = hit_pos;
        drop(s);

        if let Some(ref ops) = unhook_ops {
            apply_unhook(c, ops);
        }

        if let Some(obj) = c.get_game_object_mut(&pad_name) {
            let (pr, pg, pb) = pad_hit_for_zone(zone_idx);
            obj.set_image(Image {
                shape: ShapeType::Rectangle(0.0, (PAD_W, PAD_H), 0.0),
                image: pad_img(PAD_W as u32, PAD_H as u32, pr, pg, pb).into(),
                color: None,
            });
            obj.set_glow(GlowConfig { color: Color(60, 200, 255, 220), width: 10.0 });
        }

        // Spawn a lit particle burst at the bounce point.
        let burst = EmitterBuilder::new(&burst_id)
            .origin(hit_pos.0, hit_pos.1)
            .rate(300.0)
            .lifetime(0.3)
            .spread(100.0, 80.0)
            .velocity(0.0, -40.0)
            .size(4.0)
            .color(80, 200, 255, 230)
            .gravity_scale(0.2)
            .render_layer(3)
            .build();
        c.add_emitter(burst);

        // Brief flash light at impact.
        if c.has_lighting() {
            let flash_light = LightSource::new(
                format!("flash_{}", burst_id),
                hit_pos,
                Color(60, 200, 255, 255),
                460.0,
                3.8,
            ).with_shadows(false).with_effect(LightEffect::FadeOut {
                duration: 0.2,
            });
            c.add_light(flash_light);
        }

        // Zoom punch on pad bounce — bouncy visual feedback.
        if let Some(cam) = c.camera_mut() {
            cam.zoom_punch(0.12, 0.2);
        }
    }
}
