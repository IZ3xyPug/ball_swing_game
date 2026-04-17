use quartz::*;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;

use crate::constants::*;
use crate::gameplay::*;
use crate::state::*;
use super::helpers::*;
use quartz::GrappleConstraint;

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

        s.hooked = false;
        s.active_hook = String::new();

        // Write the impulse result to the engine object.
        // Gravity stays as-is (no need to re-enable — it was never zeroed).
        let (nvx, nvy) = (s.vx, s.vy);
        drop(s);

        // Release the grapple constraint — crystalline stops applying rope forces.
        c.release_grapple("player");

        if let Some(obj) = c.get_game_object_mut("player") {
            obj.momentum = (nvx, nvy);
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
            c.objects_in_radius(player_obj, ROPE_LEN_MAX)
                .into_iter()
                .filter(|o| o.tags.iter().any(|t| t == "hook"))
                .map(|o| {
                    let hcx = o.position.0 + HOOK_R;
                    let hcy = o.position.1 + HOOK_R;
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
                    (o.id.clone(), hcx, hcy, player_d2, cursor_d2)
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

        if let Some((hook_id, hx, hy, player_d2, _cursor_d2)) = nearest {
            let rope_len = player_d2.sqrt().clamp(ROPE_LEN_MIN, ROPE_LEN_MAX);

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

            // Write grab impulse to engine.
            // Gravity stays active — the GrappleConstraint + gravity creates
            // natural pendulum swing. No need to zero gravity.
            let (nvx, nvy) = (s.vx, s.vy);
            drop(s);

            if let Some(obj) = c.get_game_object_mut("player") {
                obj.momentum = (nvx, nvy);
            }

            // Attach a grapple constraint — crystalline handles the rope physics.
            // stiffness=1.0 → fully rigid rope (hard position projection).
            // damping=0.001 → very slight tangential energy loss per frame,
            // matching the feel of the original SWING_DRAG=0.999.
            let grapple = GrappleConstraint::at_point((hx, hy), rope_len)
                .with_stiffness(1.0)
                .with_damping(0.001);
            c.attach_grapple("player", grapple);

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
