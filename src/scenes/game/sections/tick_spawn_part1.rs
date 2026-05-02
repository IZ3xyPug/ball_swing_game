        // Register on_update only once. The persistent_state Arc ensures that
        // the single callback always sees the latest State (replaced in-place
        // on each respawn), so no generation counter or state shuttle needed.
        let tick_registered = matches!(canvas.get_var("game_tick_registered"), Some(Value::Bool(true)));
        if !tick_registered {
        let st = state.clone();
        let mut space_was_down = false;
        let mut prev_nearest_hook: String = String::new();
        let mut dark_mode_prev = false;
        let mut prev_bg_theme: Option<(bool, usize)> = None;
        let bg_zone_start_img = bg_zone_start.clone();
        let bg_zone_purple_img = bg_zone_purple.clone();
        let bg_zone_black_img = bg_zone_black.clone();
        canvas.on_update(move |c| {
            // ── Early-exit for stale callbacks from previous game sessions ───
            {
                let s = st.lock().unwrap();
                if s.dead { return; }
            }

            // ── Pause menu entrance animation (slide from top) ─────────────
            if matches!(c.get_var("pause_animating"), Some(Value::Bool(true))) {
                let mut remaining = c.get_i32("pause_anim_frames").max(0);
                let total = c.get_i32("pause_anim_total").max(1);

                if remaining > 0 {
                    remaining -= 1;
                    let t = 1.0 - (remaining as f32 / total as f32);
                    let ease = 1.0 - (1.0 - t).powi(3);
                    let y = -VH + VH * ease;

                    if let Some(obj) = c.get_game_object_mut("pause_overlay") {
                        obj.position = (0.0, y);
                        obj.visible = true;
                    }

                    c.set_var("pause_anim_frames", remaining);
                    if remaining == 0 {
                        if let Some(obj) = c.get_game_object_mut("pause_overlay") {
                            obj.position = (0.0, 0.0);
                        }
                        c.set_var("pause_animating", false);
                        c.pause();
                    }
                    return;
                }

                c.set_var("pause_animating", false);
            }

            // Space: press to grab, release to ungrab
            let space_now = c.key("space");
            if space_now && !space_was_down {
                c.run(Action::Custom { name: "do_grab".into() });
            } else if !space_now && space_was_down {
                c.run(Action::Custom { name: "do_release".into() });
            }
            space_was_down = space_now;

            // Speed-reactive trail: faster movement produces a denser,
            // longer and slightly wider connected ribbon.
            {
                let s = st.lock().unwrap();
                let speed = (s.vx * s.vx + s.vy * s.vy).sqrt();
                let rate = (62.0 + speed * 1.6).clamp(62.0, 150.0);
                let life = (0.62 + speed * 0.010).clamp(0.62, 0.95);
                let size = (8.0 + speed * 0.06).clamp(8.0, 12.0);
                let spread = (5.0 + speed * 0.05).clamp(5.0, 9.5);
                let evx = (-s.vx * 0.35).clamp(-26.0, 26.0);
                let evy = (-s.vy * 0.35 + 6.0).clamp(-26.0, 26.0);
                drop(s);

                c.run(Action::set_emitter_rate(PLAYER_TRAIL_EMITTER_NAME, rate));
                c.run(Action::set_emitter_lifetime(PLAYER_TRAIL_EMITTER_NAME, life));
                c.run(Action::set_emitter_size(PLAYER_TRAIL_EMITTER_NAME, size));
                c.run(Action::set_emitter_spread(PLAYER_TRAIL_EMITTER_NAME, spread, spread));
                c.run(Action::set_emitter_velocity(PLAYER_TRAIL_EMITTER_NAME, evx, evy));
                c.run(Action::set_emitter_color(
                    PLAYER_TRAIL_EMITTER_NAME,
                    C_PLAYER.0,
                    C_PLAYER.1,
                    C_PLAYER.2,
                    255,
                ));
            }

            let mut s = st.lock().unwrap();
            s.ticks += 1;
            if s.spinner_hit_cooldown > 0 {
                s.spinner_hit_cooldown -= 1;
            }

            // Tick down glow flash timers
            let mut expired_glows: Vec<String> = Vec::new();
            s.glow_flashes.retain_mut(|(id, frames)| {
                *frames = frames.saturating_sub(1);
                if *frames == 0 {
                    expired_glows.push(id.clone());
                    false
                } else {
                    true
                }
            });
            let dark = s.dark_mode;
            drop(s);
            for id in &expired_glows {
                if let Some(obj) = c.get_game_object_mut(id) {
                    if dark && id == "player" {
                        // Restore ambient player glow in dark mode
                        obj.set_glow(GlowConfig { color: Color(80, 255, 180, 220), width: 28.0 });
                    } else {
                        obj.clear_glow();
                    }
                }
            }
            s = st.lock().unwrap();

            let spinner_enabled = s.spinners_enabled;
            let spinner_spin_enabled = s.spinner_spin_enabled;
            let bounce_enabled = s.bounce_enabled;
            let dark_mode = s.dark_mode;
            let zone_idx = zone_index_for_distance(s.distance);
            let zone_spinner_speed = spinner_speed_for_zone(zone_idx);
            let spinner_live = s.spinner_live.clone();
            let spinner_origins = s.spinner_origins.clone();
            let pad_live = s.pad_live.clone();
            let spinner_vertical_active = zone_idx >= 2;
            let spinner_t = s.ticks as f32 / 60.0;
            drop(s);
            for name in &spinner_live {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = spinner_enabled;
                    if spinner_spin_enabled {
                        let dir = if obj.rotation_momentum.abs() < 0.01 {
                            1.0
                        } else {
                            obj.rotation_momentum.signum()
                        };
                        obj.rotation_momentum = dir * zone_spinner_speed;
                    } else {
                        obj.rotation_momentum = 0.0;
                    }
                    if obj.rotation > 360.0 || obj.rotation < -360.0 {
                        obj.rotation = obj.rotation.rem_euclid(360.0);
                    }
                    if let Some((_, base_y, amp, speed, phase)) =
                        spinner_origins.iter().find(|(id, _, _, _, _)| id == name)
                    {
                        obj.position.1 = if spinner_vertical_active {
                            *base_y + (spinner_t * *speed + *phase).sin() * *amp
                        } else {
                            *base_y
                        };
                    }
                    // Dark mode: apply glow to newly-visible spinners
                    if dark_mode && obj.highlight.is_none() {
                        obj.set_glow(GlowConfig { color: Color(255, 60, 50, 160), width: 10.0 });
                    }
                }
            }
            for name in &pad_live {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = bounce_enabled;
                    // Dark mode: apply glow to newly-visible pads
                    if dark_mode && obj.highlight.is_none() {
                        obj.set_glow(GlowConfig { color: Color(60, 180, 255, 160), width: 10.0 });
                    }
                }
            }

            s = st.lock().unwrap();

            // ── Spawn pending hooks ahead of the player ───────────────────────
            let mut hooks_spawned_this_tick = 0usize;
            while hooks_spawned_this_tick < HOOKS_SPAWN_BUDGET_PER_TICK
                && s.rightmost_x < s.px + GEN_AHEAD
                && !s.pool_free.is_empty()
            {
                if let Some(spec) = s.pending.pop_front() {
                    let Some(id) = s.pool_free.pop() else { break; };
                    let mut hx = spec.x;
                    let hook_spinner_min_x_gap = HOOK_SPINNER_MIN_X_GAP;
                    for spinner_name in &s.spinner_live {
                        if let Some(spinner_obj) = c.get_game_object(spinner_name) {
                            let spinner_center_x = spinner_obj.position.0 + SPINNER_W * 0.5;
                            let dx = hx - spinner_center_x;
                            if dx.abs() < hook_spinner_min_x_gap {
                                let dir = if dx >= 0.0 { 1.0 } else { -1.0 };
                                hx += dir * HOOK_SPINNER_PUSH_X;
                            }
                        }
                    }
                    s.rightmost_x = hx;
                    let hy = if s.gravity_dir < 0.0 { VH - spec.y } else { spec.y };
                    s.live_hooks.push(id.clone());
                    hooks_spawned_this_tick += 1;
                    drop(s);

                    if let Some(obj) = c.get_game_object_mut(&id) {
                        obj.position = (hx - HOOK_R, hy - HOOK_R);
                        obj.visible = true;
                        obj.set_image(Image {
                            shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                            image: circle_img(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2).into(),
                            color: None,
                        });
                    }

                    s = st.lock().unwrap();
                } else {
                    // Pending empty — generate another batch (don't advance rightmost_x;
                    // it gets updated per-hook as they are actually spawned above)
                    let from = s.rightmost_x;
                    let diff = (s.distance / 18000.0).min(1.0);
                    let mut next_seed = s.seed;
                    let mut next_gen_y = s.gen_y;
                    let batch = gen_hook_batch(&mut next_seed, from, &mut next_gen_y, diff);
                    s.seed = next_seed;
                    s.gen_y = next_gen_y;
                    s.pending = batch;
                }
            }

            // ── Spawn bounce pads ahead of the player ────────────────────────
            let mut pads_spawned_this_tick = 0usize;
            while pads_spawned_this_tick < PADS_SPAWN_BUDGET_PER_TICK
                && s.bounce_enabled
                && s.pad_rightmost < s.px + GEN_AHEAD
                && !s.pad_free.is_empty()
            {
                let gap = lcg_range(&mut s.seed, PAD_GAP_MIN, PAD_GAP_MAX);
                let pad_x = s.pad_rightmost + gap;
                let pad_y = if s.gravity_dir < 0.0 {
                    28.0
                } else {
                    VH - 28.0 - PAD_H
                };
                s.pad_rightmost = pad_x;
                // Static-heavy mix; movers have unique phase/speed/amp so they don't sync.
                let moves = lcg(&mut s.seed) < 0.35;
                let Some(id) = s.pad_free.pop() else { break; };
                s.pad_live.push(id.clone());
                pads_spawned_this_tick += 1;
                if moves {
                    let amp = lcg_range(&mut s.seed, PAD_MOVE_RANGE * 0.45, PAD_MOVE_RANGE * 1.10);
                    let speed = lcg_range(&mut s.seed, PAD_MOVE_SPEED * 0.65, PAD_MOVE_SPEED * 1.45);
                    let phase = lcg_range(&mut s.seed, 0.0, core::f32::consts::TAU);
                    s.pad_origins.push((id.clone(), pad_x, amp, speed, phase));
                }
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&id) {
                    obj.position = (pad_x, pad_y);
                    obj.visible = true;
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (PAD_W, PAD_H), 0.0),
                        image: std::sync::Arc::new(crate::images::solid(
                            crate::constants::C_PAD.0,
                            crate::constants::C_PAD.1,
                            crate::constants::C_PAD.2,
                            255,
                        )),
                        color: None,
                    });
                }

                s = st.lock().unwrap();
            }

            // ── Spawn spinning obstacles ahead of the player ─────────────────
            let mut spinners_spawned_this_tick = 0usize;
            while spinners_spawned_this_tick < SPINNERS_SPAWN_BUDGET_PER_TICK
                && s.spinners_enabled
                && s.spinner_rightmost < s.px + GEN_AHEAD
                && !s.spinner_free.is_empty()
            {
                let gap = lcg_range(&mut s.seed, SPINNER_GAP_MIN, SPINNER_GAP_MAX);
                let mut spin_x = s.spinner_rightmost + gap;
                for gate_name in &s.gate_live {
                    let gate_top = format!("{gate_name}_top");
                    if let Some(gobj) = c.get_game_object(&gate_top) {
                        let overlaps_gate = spin_x + SPINNER_W > gobj.position.0 - 80.0
                            && spin_x < gobj.position.0 + GATE_W + 80.0;
                        if overlaps_gate {
                            spin_x = gobj.position.0 + GATE_W + 220.0;
                        }
                    }
                }

                // Spinner Y: prefer above a nearby hook; otherwise use legacy low lanes.
                let mut hook_anchor_y: Option<f32> = None;
                let mut best_dx = f32::MAX;
                let target_x = spin_x + SPINNER_W * 0.5;
                for hook_name in &s.live_hooks {
                    if let Some(hook_obj) = c.get_game_object(hook_name) {
                        let hx = hook_obj.position.0 + HOOK_R;
                        let hy = hook_obj.position.1 + HOOK_R;
                        let dx = (hx - target_x).abs();
                        if dx < best_dx {
                            best_dx = dx;
                            hook_anchor_y = Some(hy);
                        }
                    }
                }

                let raw_spin_y = if let Some(hy) = hook_anchor_y {
                    // Two hook-anchored variants:
                    // Spinner A around Y=+1350, Spinner B around Y=-0500.
                    if lcg(&mut s.seed) < 0.5 {
                        lcg_range(&mut s.seed, 1240.0, 1460.0)
                    } else {
                        hy - 800.0
                    }
                } else {
                    let spin_lanes = [VH * 0.62, VH * 0.70, VH * 0.76, VH * 0.82];
                    let lane_i = ((lcg(&mut s.seed) * spin_lanes.len() as f32) as usize).min(spin_lanes.len() - 1);
                    (spin_lanes[lane_i] + lcg_range(&mut s.seed, -22.0, 22.0)).clamp(VH * 0.58, VH * 0.86)
                };
                let spin_y = if s.gravity_dir < 0.0 {
                    VH - raw_spin_y - SPINNER_H
                } else {
                    raw_spin_y
                };
                s.spinner_rightmost = spin_x;
                let zone_idx = zone_index_for_distance(s.distance);
                let zone_spinner_speed = spinner_speed_for_zone(zone_idx);
                let spin_dir = if lcg(&mut s.seed) < 0.5 { -zone_spinner_speed } else { zone_spinner_speed };
                let spin_enabled_now = s.spinner_spin_enabled;
                let Some(id) = s.spinner_free.pop() else { break; };
                s.spinner_live.push(id.clone());
                let move_amp = lcg_range(&mut s.seed, SPINNER_BLACK_MOVE_AMP_MIN, SPINNER_BLACK_MOVE_AMP_MAX);
                let move_speed = lcg_range(&mut s.seed, SPINNER_BLACK_MOVE_SPEED_MIN, SPINNER_BLACK_MOVE_SPEED_MAX);
                let move_phase = lcg_range(&mut s.seed, 0.0, core::f32::consts::TAU);
                s.spinner_origins.push((id.clone(), spin_y, move_amp, move_speed, move_phase));
                spinners_spawned_this_tick += 1;
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&id) {
                    obj.position = (spin_x, spin_y);
                    obj.size = (SPINNER_W, SPINNER_H);
                    obj.visible = true;
                    obj.rotation = 0.0;
                    obj.rotation_momentum = if spin_enabled_now { spin_dir } else { 0.0 };
                    obj.rotation_resistance = 1.0;
                    obj.is_platform = false;
                    obj.collision_mode = CollisionMode::NonPlatform;
                    obj.surface_velocity = None;
                }

                s = st.lock().unwrap();
            }

            // ── Spawn gravity flip pickups ahead of the player ──────────────
            let mut flips_spawned_this_tick = 0usize;
            while flips_spawned_this_tick < FLIPS_SPAWN_BUDGET_PER_TICK
                && s.flip_rightmost < s.px + GEN_AHEAD
                && !s.flip_free.is_empty()
            {
                let gap = lcg_range(&mut s.seed, FLIP_GAP_MIN, FLIP_GAP_MAX);
                let flip_x = s.flip_rightmost + gap;
                let raw_flip_y = lcg_range(&mut s.seed, VH * 0.28, VH * 0.66);
                let flip_y = if s.gravity_dir < 0.0 {
                    VH - raw_flip_y - FLIP_H
                } else {
                    raw_flip_y
                };
                s.flip_rightmost = flip_x;
                let Some(id) = s.flip_free.pop() else { break; };
                s.flip_live.push(id.clone());
                flips_spawned_this_tick += 1;
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&id) {
                    obj.position = (flip_x, flip_y);
                    obj.visible = true;
                }

                s = st.lock().unwrap();
            }

