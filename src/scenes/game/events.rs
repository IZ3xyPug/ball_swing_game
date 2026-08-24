use quartz::*;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;

use crate::constants::*;
use crate::gameplay::*;
use crate::state::*;
use super::helpers::*;

/// Register do_release and do_grab custom events.
pub fn register_events(canvas: &mut Canvas, state: &Arc<Mutex<State>>) {
    // ── Release ──────────────────────────────────────────────────────────
    let st = state.clone();
    canvas.register_custom_event("do_release".into(), move |c| {
        let mut s = st.lock().unwrap();
        if s.dead || !s.hooked { return; }

        apply_release_impulse(&mut s);

        let prev = s.active_hook.clone();
        let zone_idx = zone_index_for_distance(s.distance);
        let gravity_scale = if s.zero_g_timer > 0 { ZERO_G_GRAVITY_SCALE } else { 1.0 };

        s.hooked = false;
        s.active_hook = String::new();

        // Write the impulse result to the engine object and re-enable gravity.
        let (nvx, nvy) = (s.vx, s.vy);
        let gdir = s.gravity_dir;
        drop(s);

        if let Some(obj) = c.get_game_object_mut("player") {
            obj.momentum = (nvx, nvy);
            obj.gravity = GRAVITY * gravity_scale * gdir;
        }

        c.run(Action::Hide { target: Target::name("rope") });

        if !prev.is_empty() {
            let asteroid_mode = c.get_bool("asteroid_hooks_on");
            if let Some(obj) = c.get_game_object_mut(&prev) {
                if is_special_hook_obj(obj) {
                    // Pause the green artifact gif at frame 0.
                    if let Some(sprite) = &mut obj.animated_sprite {
                        sprite.reset();
                        sprite.set_fps(0.001);
                    }
                } else if !asteroid_mode {
                    let (r, g, b) = hook_base_for_obj(obj, zone_idx);
                    obj.set_image(hook_img(r, g, b));
                }
                obj.clear_glow();
            }
        }
    });

    // ── Grab ─────────────────────────────────────────────────────────────
    let st = state.clone();
    canvas.register_custom_event("do_grab".into(), move |c| {
        let mut s = st.lock().unwrap();
        if s.dead || s.hooked || s.cannon_fast_travel_grace > 0 { return; }

        let mouse_target = if matches!(c.get_var("grab_from_mouse"), Some(Value::Bool(true))) {
            Some((c.get_f32("mouse_grab_x"), c.get_f32("mouse_grab_y")))
        } else {
            None
        };

        // Sync State position from engine before computing grab.
        if let Some(obj) = c.get_game_object("player") {
            s.px = obj.position.0 + PLAYER_R;
            s.py = obj.position.1 + PLAYER_R;
            s.vx = obj.momentum.0;
            s.vy = obj.momentum.1;
        }

        let nearest = if let Some(player_obj) = c.get_game_object("player") {
            let reach_mult = if s.boss_active { 1.45 } else { 1.0 };
            let normal_reach  = ROPE_LEN_MAX * reach_mult;
            let extended_reach = normal_reach * EXTENDED_HOOK_REACH_MULT;
            c.objects_in_radius(player_obj, extended_reach)
                .into_iter()
                .filter(|o| o.tags.iter().any(|t| t == "hook"))
                .map(|o| {
                    let hcx = o.position.0 + o.size.0 * 0.5;
                    let hcy = o.position.1 + o.size.1 * 0.5;
                    let pdx = hcx - s.px;
                    let pdy = hcy - s.py;
                    let player_d2 = pdx * pdx + pdy * pdy;
                    let cursor_d2 = if let Some((mx, my)) = mouse_target {
                        let cdx = hcx - mx;
                        let cdy = hcy - my;
                        cdx * cdx + cdy * cdy
                    } else {
                        player_d2
                    };
                    let is_special  = o.tags.iter().any(|t| t == SPECIAL_HOOK_TAG);
                    let is_extended = o.tags.iter().any(|t| t == EXTENDED_HOOK_TAG);
                    (o.id.clone(), hcx, hcy, player_d2, cursor_d2, is_special, is_extended)
                })
                // Filter: non-extended hooks are only grabbable within normal reach.
                .filter(|(_, _, _, player_d2, _, _, is_extended)| {
                    *is_extended || *player_d2 <= normal_reach * normal_reach
                })
                .min_by(|a, b| {
                    if mouse_target.is_some() {
                        a.4
                            .partial_cmp(&b.4)
                            .unwrap_or(Ordering::Equal)
                            .then(a.3.partial_cmp(&b.3).unwrap_or(Ordering::Equal))
                    } else {
                        a.3.partial_cmp(&b.3).unwrap_or(Ordering::Equal)
                    }
                })
        } else {
            None
        };

        if let Some((hook_id, hx, hy, player_d2, _cursor_d2, is_special_hook, _is_extended_hook)) = nearest {
            let rope_len = player_d2.sqrt().clamp(ROPE_LEN_MIN, ROPE_LEN_MAX);

            // Capture incoming velocity before it's redirected by the grab impulse.
            let (pvx, pvy) = (s.vx, s.vy);
            apply_grab_impulse(&mut s, hx, hy);
            if is_special_hook {
                apply_special_hook_boost(&mut s, hx, hy);
            }

            s.hooked = true;
            s.hook_x = hx;
            s.hook_y = hy;
            s.rope_len = rope_len;
            s.active_hook = hook_id.clone();

            let zone_idx = zone_index_for_distance(s.distance);

            // Write grab impulse to engine; disable gravity (rope handles it).
            let (nvx, nvy) = (s.vx, s.vy);
            drop(s);

            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum = (nvx, nvy);
                obj.gravity = 0.0;
            }

            let asteroid_mode = c.get_bool("asteroid_hooks_on");
            let mut artifact_grab_info: Option<(i32, String)> = None;
            // If a countdown for a different hook is still running, freeze it now
            // before starting a new one, to prevent it looping indefinitely.
            if asteroid_mode {
                let orphan_id = match c.get_var("hook_artifact_anim_id") {
                    Some(Value::Str(s)) if !s.is_empty() && s != hook_id => Some(s),
                    _ => None,
                };
                if let Some(old_id) = orphan_id {
                    if let Some(old_obj) = c.get_game_object_mut(&old_id) {
                        if let Some(sprite) = &mut old_obj.animated_sprite {
                            sprite.reset();
                            sprite.set_fps(0.001);
                        }
                    }
                    c.set_var("hook_artifact_play_ticks", 0i32);
                }
            }
            if let Some(obj) = c.get_game_object_mut(&hook_id) {
                if is_special_hook_obj(obj) {
                    // Resume the green artifact gif from frame 0 at full fps, no glow.
                    if let Some(sprite) = &mut obj.animated_sprite {
                        sprite.reset();
                        sprite.set_fps(HOOK_ARTIFACT_FPS);
                    }
                    obj.clear_glow();
                } else if asteroid_mode {
                    // Proximity intro is done (hook frozen at frame 4); resume at full speed.
                    if let Some(sprite) = &mut obj.animated_sprite {
                        sprite.set_fps(HOOK_ARTIFACT_FPS);
                        let remaining = sprite.frame_count() - sprite.current_frame_index();
                        let ticks = (remaining as f32 * (60.0 / HOOK_ARTIFACT_FPS)).round() as i32;
                        artifact_grab_info = Some((ticks.max(1), hook_id.clone()));
                    }
                    obj.clear_glow();
                    // Transfer player momentum to the grabbed asteroid hook.
                    // Size-normalise so smaller hooks react more visibly.
                    let size_norm = SPACE_ASTEROID_SIZE_MIN / obj.size.0.max(SPACE_ASTEROID_SIZE_MIN);
                    let factor = ASTEROID_HOOK_IMPULSE_FACTOR * size_norm;
                    obj.momentum.0 += pvx * factor;
                    obj.momentum.1 += pvy * factor;
                } else {
                    let (r, g, b) = hook_on_for_obj(obj, zone_idx);
                    obj.set_image(hook_img(r, g, b));
                    obj.set_glow(GlowConfig { color: Color(255, 215, 100, 255), width: 24.0 });
                }
            }

            if is_special_hook {
                c.set_var("special_hook_boost_ticks", SPECIAL_HOOK_CAP_WINDOW_TICKS);
            }
            // Buff tether node: grant a timed damage/momentum buff.
            if let Some(obj) = c.get_game_object(&hook_id) {
                if obj.tags.iter().any(|t| t == BUFF_HOOK_TAG) {
                    let mut s = st.lock().unwrap();
                    s.player_buff = 1;
                    s.buff_timer = BUFF_DURATION_TICKS;
                    s.buff_hit_flash = 0;
                    drop(s);
                    if let Some(p) = c.get_game_object_mut("player") {
                        p.set_glow(GlowConfig { color: Color(110, 230, 255, 255), width: 22.0 });
                    }
                }
            }
            if let Some((ticks, anim_id)) = artifact_grab_info {
                c.set_var("hook_artifact_play_ticks", ticks);
                c.set_var("hook_artifact_anim_id", anim_id);
            }
            if asteroid_mode && !is_special_hook {
                c.set_var("hook_prox_id", String::new());
            }

            c.run(Action::Show { target: Target::name("rope") });
            c.play_sound_with(ASSET_CARTOON_CAT, SoundOptions::new().volume(sfx_vol(c, 0.6)));
        }
    });

    // ── Mouse ────────────────────────────────────────────────────────────
    // Callbacks only latch a flag; the on_update tick polls it with
    // edge-detection so mouse and spacebar trigger at exactly the same
    // point in the frame, avoiding inter-tick timing differences.
    let mouse_registered = matches!(canvas.get_var("game_mouse_registered"), Some(Value::Bool(true)));
    if !mouse_registered {
        canvas.on_mouse_press(move |c, btn, _pos| {
            if btn != MouseButton::Left { return; }
            c.set_var("mouse_left_held", true);
        });
        canvas.on_mouse_release(move |c, btn, _pos| {
            if btn != MouseButton::Left { return; }
            c.set_var("mouse_left_held", false);
        });
        canvas.set_var("mouse_left_held", false);
        canvas.set_var("game_mouse_registered", true);
    }
}
