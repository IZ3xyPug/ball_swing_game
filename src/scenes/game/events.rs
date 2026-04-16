use quartz::*;
use std::sync::{Arc, Mutex};

use crate::constants::*;
use crate::gameplay::*;
use crate::images::*;
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
            if let Some(obj) = c.get_game_object_mut(&prev) {
                let (r, g, b) = hook_base_for_zone(zone_idx);
                obj.set_image(hook_img(r, g, b));
                obj.clear_glow();
            }
        }
    });

    // ── Grab ─────────────────────────────────────────────────────────────
    let st = state.clone();
    canvas.register_custom_event("do_grab".into(), move |c| {
        let mut s = st.lock().unwrap();
        if s.dead || s.hooked { return; }

        // Sync State position from engine before computing grab.
        if let Some(obj) = c.get_game_object("player") {
            s.px = obj.position.0 + PLAYER_R;
            s.py = obj.position.1 + PLAYER_R;
            s.vx = obj.momentum.0;
            s.vy = obj.momentum.1;
        }

        let nearest = if let Some(player_obj) = c.get_game_object("player") {
            c.objects_in_radius(player_obj, ROPE_LEN_MAX)
                .into_iter()
                .filter(|o| o.tags.iter().any(|t| t == "hook"))
                .map(|o| {
                    let hcx = o.position.0 + HOOK_R;
                    let hcy = o.position.1 + HOOK_R;
                    let dx = hcx - s.px;
                    let dy = hcy - s.py;
                    (o.id.clone(), hcx, hcy, (dx*dx + dy*dy).sqrt())
                })
                .min_by(|a, b| a.3.partial_cmp(&b.3).unwrap())
        } else {
            None
        };

        if let Some((hook_id, hx, hy, dist)) = nearest {
            let rope_len = dist.clamp(ROPE_LEN_MIN, ROPE_LEN_MAX);

            apply_grab_impulse(&mut s, hx, hy);

            s.hooked = true;
            s.hook_x = hx;
            s.hook_y = hy;
            s.rope_len = rope_len;
            s.active_hook = hook_id.clone();
            s.pad_bounce_count = 0;

            let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
            s.score = s.score.saturating_add(100u32.saturating_mul(score_mult));

            let zone_idx = zone_index_for_distance(s.distance);

            // Write grab impulse to engine; disable gravity (rope handles it).
            let (nvx, nvy) = (s.vx, s.vy);
            drop(s);

            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum = (nvx, nvy);
                obj.gravity = 0.0;
            }

            if let Some(obj) = c.get_game_object_mut(&hook_id) {
                let (r, g, b) = hook_on_for_zone(zone_idx);
                obj.set_image(hook_img(r, g, b));
                obj.set_glow(GlowConfig { color: Color(255, 200, 80, 255), width: 18.0 });
            }

            c.run(Action::Show { target: Target::name("rope") });
            c.play_sound_with(ASSET_SWOOSH_SFX, SoundOptions::new().volume(0.6));
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
