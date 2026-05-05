            // ── Physics integration ───────────────────────────────────────────
            if s.hooked {
                // Constrain to a circular path and evolve tangential speed only.
                let dx   = s.px - s.hook_x;
                let dy   = s.py - s.hook_y;
                let dist = (dx*dx + dy*dy).sqrt().max(1.0);
                let nx = dx / dist;
                let ny = dy / dist;
                let tx = -ny;
                let ty = nx;

                let radial_v = s.vx * nx + s.vy * ny;
                let mut tangent_v = s.vx * tx + s.vy * ty;

                // Keep the rope taut and remove radial velocity so momentum stays on-arc.
                s.px = s.hook_x + nx * s.rope_len;
                s.py = s.hook_y + ny * s.rope_len;
                s.vx -= radial_v * nx * SWING_TENSION;
                s.vy -= radial_v * ny * SWING_TENSION;

                // Apply only tangential gravity while hooked; allows full loops if fast enough.
                tangent_v += GRAVITY * s.gravity_dir * ty;
                if !s.in_space_mode { tangent_v *= SWING_DRAG; }
                s.vx = tx * tangent_v;
                s.vy = ty * tangent_v;

                // Update rope transform each frame; avoids expensive image rebuilds.
                let (rdx, rdy, hx, hy) = (s.px - s.hook_x, s.py - s.hook_y, s.hook_x, s.hook_y);
                let rope_len = (rdx * rdx + rdy * rdy).sqrt().max(1.0);
                let rope_ang = rdy.atan2(rdx).to_degrees();
                let rope_mid_x = hx + rdx * 0.5;
                let rope_mid_y = hy + rdy * 0.5;
                drop(s);

                if let Some(rope_obj) = c.get_game_object_mut("rope") {
                    rope_obj.size = (rope_len, ROPE_THICKNESS);
                    rope_obj.position = (rope_mid_x - rope_len * 0.5, rope_mid_y - ROPE_THICKNESS * 0.5);
                    rope_obj.rotation = rope_ang;
                }

                s = st.lock().unwrap();
            } else {
                // Free-fall gravity while not attached.
                s.vy += GRAVITY * s.gravity_dir;
            }

            // Clamp max speed
            let speed = (s.vx*s.vx + s.vy*s.vy).sqrt();
            if speed > MOMENTUM_CAP {
                s.vx = s.vx / speed * MOMENTUM_CAP;
                s.vy = s.vy / speed * MOMENTUM_CAP;
            }

            // Integrate position
            s.px += s.vx;
            s.py += s.vy;

            // ── Spinning obstacle collision ──────────────────────────────────
            // Always depenetrate to prevent phasing. Only apply bounce impulse
            // when cooldown is 0 to avoid jitter.
            if s.spinners_enabled {
                for name in s.spinner_live.clone() {
                    if let Some(obj) = c.get_game_object(&name) {
                        if let Some((push_x, push_y)) = circle_hits_obb(
                            (s.px, s.py),
                            PLAYER_R + 4.0,
                            obj.position,
                            obj.size,
                            obj.rotation,
                        ) {
                            // Always depenetrate
                            s.px += push_x;
                            s.py += push_y;

                            let push_len = (push_x * push_x + push_y * push_y).sqrt().max(0.001);
                            let nx = push_x / push_len;
                            let ny = push_y / push_len;

                            // Cancel inward velocity
                            let inward = -(s.vx * nx + s.vy * ny);
                            if inward > 0.0 {
                                s.vx += nx * inward;
                                s.vy += ny * inward;
                            }

                            // Bounce impulse + effects only on fresh hit
                            if s.spinner_hit_cooldown == 0 {
                                let push_mag = (SPINNER_HIT_PUSH_X * SPINNER_HIT_PUSH_X
                                    + SPINNER_HIT_PUSH_Y * SPINNER_HIT_PUSH_Y).sqrt();
                                s.vx += nx * push_mag;
                                s.vy += ny * push_mag;
                                s.spinner_hit_cooldown = 6;
                                s.glow_flashes.push((name.clone(), 10));
                                drop(s);
                                if let Some(obj) = c.get_game_object_mut(&name) {
                                    obj.set_glow(GlowConfig { color: Color(255, 100, 80, 220), width: 8.0 });
                                }
                                s = st.lock().unwrap();

                                if s.hooked {
                                    let prev = s.active_hook.clone();
                                    s.hooked = false;
                                    s.active_hook = String::new();
                                    drop(s);
                                    c.run(Action::Hide { target: Target::name("rope") });
                                    if !prev.is_empty() {
                                        if let Some(obj) = c.get_game_object_mut(&prev) {
                                            obj.set_image(Image {
                                                shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                                                image: circle_img(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2).into(),
                                                color: None,
                                            });
                                        }
                                    }
                                    s = st.lock().unwrap();
                                }
                            }
                        }
                    }
                }
            }

            // ── Gate obstacle collision (flappy-style top/bottom blockers) ──
            if GATES_ENABLED {
                for gate_id in s.gate_live.clone() {
                    let top_id = format!("{gate_id}_top");
                    let bot_id = format!("{gate_id}_bot");
                    for seg_id in [top_id, bot_id] {
                        if let Some(obj) = c.get_game_object(&seg_id) {
                            if let Some((push_x, push_y)) = circle_hits_aabb(
                                (s.px, s.py),
                                PLAYER_R + 2.0,
                                obj.position,
                                obj.size,
                            ) {
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

                                // Gate hit gives a small shove away and breaks rope.
                                s.vx += nx * 4.0;
                                s.vy += ny * 4.0;
                                if s.hooked {
                                    let prev = s.active_hook.clone();
                                    s.hooked = false;
                                    s.active_hook = String::new();
                                    drop(s);
                                    c.run(Action::Hide { target: Target::name("rope") });
                                    if !prev.is_empty() {
                                        if let Some(hook_obj) = c.get_game_object_mut(&prev) {
                                            hook_obj.set_image(Image {
                                                shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                                                image: circle_img(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2).into(),
                                                color: None,
                                            });
                                        }
                                    }
                                    s = st.lock().unwrap();
                                }
                            }
                        }
                    }
                }
            }

            // ── Bounce pad collision ──────────────────────────────────────────
            // Player is moving toward a pad and overlaps it → bounce away.
            let falling_down = s.gravity_dir > 0.0 && s.vy > 0.0;
            let falling_up   = s.gravity_dir < 0.0 && s.vy < 0.0;
            if s.bounce_enabled && (falling_down || falling_up) {
                let player_bottom = s.py + PLAYER_R;
                let player_top    = s.py - PLAYER_R;
                let player_left   = s.px - PLAYER_R;
                let player_right  = s.px + PLAYER_R;
                let mut bounced_pad: Option<String> = None;
                for name in &s.pad_live {
                    if let Some(obj) = c.get_game_object(name) {
                        let pad_top   = obj.position.1;
                        let pad_bottom = obj.position.1 + PAD_H;
                        let pad_left  = obj.position.0;
                        let pad_right = obj.position.0 + PAD_W;
                        let overlap_x = player_right > pad_left && player_left < pad_right;
                        let hit = if falling_down {
                            overlap_x && player_bottom >= pad_top && player_bottom <= pad_top + PAD_H + s.vy.abs()
                        } else {
                            overlap_x && player_top <= pad_bottom && player_top >= pad_bottom - PAD_H - s.vy.abs()
                        };
                        if hit {
                            bounced_pad = Some(name.clone());
                            break;
                        }
                    }
                }
                if let Some(pad_name) = bounced_pad {
                    let bounce_factor = (1.0 - s.pad_bounce_count as f32 * PAD_BOUNCE_DECAY)
                        .max(PAD_BOUNCE_MIN_FACTOR);
                    s.vy = PAD_BOUNCE_VY_START * bounce_factor * s.gravity_dir;
                    s.pad_bounce_count = s.pad_bounce_count.saturating_add(1);
                    s.player_ball_hit_rewind = true;
                    s.player_ball_frame_timer = 0;
                    // Place player outside pad
                    if let Some(pad_obj) = c.get_game_object(&pad_name) {
                        if falling_down {
                            s.py = pad_obj.position.1 - PLAYER_R;
                        } else {
                            s.py = pad_obj.position.1 + PAD_H + PLAYER_R;
                        }
                    }
                    // If hooked, release
                    if s.hooked {
                        let prev = s.active_hook.clone();
                        s.hooked = false;
                        s.active_hook = String::new();
                        drop(s);
                        c.run(Action::Hide { target: Target::name("rope") });
                        if !prev.is_empty() {
                            if let Some(obj) = c.get_game_object_mut(&prev) {
                                obj.set_image(Image {
                                    shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                                    image: circle_img(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2).into(),
                                    color: None,
                                });
                            }
                        }
                        // Flash the pad
                        if let Some(obj) = c.get_game_object_mut(&pad_name) {
                            obj.set_image(Image {
                                shape: ShapeType::Rectangle(0.0, (PAD_W, PAD_H), 0.0),
                                image: pad_img(PAD_W as u32, PAD_H as u32, C_PAD_HIT.0, C_PAD_HIT.1, C_PAD_HIT.2).into(),
                                color: None,
                            });
                            obj.set_glow(GlowConfig { color: Color(60, 200, 255, 220), width: 10.0 });
                        }
                        s = st.lock().unwrap();
                        s.glow_flashes.push((pad_name.clone(), 12));
                    } else {
                        drop(s);
                        // Flash the pad
                        if let Some(obj) = c.get_game_object_mut(&pad_name) {
                            obj.set_image(Image {
                                shape: ShapeType::Rectangle(0.0, (PAD_W, PAD_H), 0.0),
                                image: pad_img(PAD_W as u32, PAD_H as u32, C_PAD_HIT.0, C_PAD_HIT.1, C_PAD_HIT.2).into(),
                                color: None,
                            });
                            obj.set_glow(GlowConfig { color: Color(60, 200, 255, 220), width: 10.0 });
                        }
                        s = st.lock().unwrap();
                        s.glow_flashes.push((pad_name.clone(), 12));
                    }
                }
            }

            // ── Gravity flip pickup ──────────────────────────────────────────
            let player_left   = s.px - PLAYER_R;
            let player_right  = s.px + PLAYER_R;
            let player_top    = s.py - PLAYER_R;
            let player_bottom = s.py + PLAYER_R;
            let mut hit_flip: Option<String> = None;
            {
                for name in &s.flip_live {
                    if let Some(obj) = c.get_game_object(name) {
                        let fl = obj.position.0;
                        let fr = obj.position.0 + FLIP_W;
                        let ft = obj.position.1;
                        let fb = obj.position.1 + FLIP_H;
                        if player_right > fl && player_left < fr && player_bottom > ft && player_top < fb {
                            hit_flip = Some(name.clone());
                            break;
                        }
                    }
                }
            }
            if let Some(name) = hit_flip {
                // If already flipped, just refresh the timer
                if s.flip_timer > 0 {
                    s.flip_timer = FLIP_DURATION;
                    s.flip_live.retain(|n| n != &name);
                    s.flip_free.push(name.clone());
                    s.glow_flashes.push(("player".to_string(), 20));
                    drop(s);
                    if let Some(obj) = c.get_game_object_mut(&name) {
                        obj.visible = false;
                        obj.position = (-3800.0, -3800.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("player") {
                        obj.set_glow(GlowConfig { color: Color(255, 245, 120, 255), width: 18.0 });
                    }
                    s = st.lock().unwrap();
                } else {
                    // First flip: toggle gravity and start timer
                    s.gravity_dir *= -1.0;
                    s.flip_timer = FLIP_DURATION;
                    if s.hooked {
                        s.vy = -s.vy;
                    } else {
                        s.vy = -s.vy * 0.55;
                    }
                    s.py = VH - s.py;
                    s.hook_y = VH - s.hook_y;
                    s.flip_live.retain(|n| n != &name);
                    s.flip_free.push(name.clone());
                    s.glow_flashes.push(("player".to_string(), 20));

                    let all_objs: Vec<(String, f32)> =
                        s.live_hooks.iter().map(|n| (n.clone(), HOOK_R * 2.0))
                        .chain(s.pad_live.iter().map(|n| (n.clone(), PAD_H)))
                        .chain(s.spinner_live.iter().map(|n| (n.clone(), SPINNER_H)))
                        .chain(s.coin_live.iter().map(|n| (n.clone(), COIN_R * 2.0)))
                        .chain(s.flip_live.iter().map(|n| (n.clone(), FLIP_H)))
                        .chain(s.score_x2_live.iter().map(|n| (n.clone(), SCORE_X2_H)))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_top"), GATE_TOP_SEG_H)))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_bot"), GATE_BOT_SEG_H)))
                        .collect();
                    drop(s);

                    for (obj_name, obj_h) in &all_objs {
                        if let Some(obj) = c.get_game_object_mut(obj_name) {
                            obj.position.1 = VH - obj.position.1 - obj_h;
                        }
                    }

                    if let Some(obj) = c.get_game_object_mut(&name) {
                        obj.visible = false;
                        obj.position = (-3800.0, -3800.0);
                    }
                    if let Some(obj) = c.get_game_object_mut("player") {
                        obj.set_glow(GlowConfig { color: Color(255, 245, 120, 255), width: 18.0 });
                    }

                    s = st.lock().unwrap();
                }
            }

            // ── Flip timer countdown — revert gravity when time runs out ─────
            if s.flip_timer > 0 {
                s.flip_timer -= 1;
                if s.flip_timer == 0 {
                    // Timer expired: flip gravity back to normal
                    s.gravity_dir *= -1.0;
                    if s.hooked {
                        s.vy = -s.vy;
                    } else {
                        s.vy = -s.vy * 0.55;
                    }
                    s.py = VH - s.py;
                    s.hook_y = VH - s.hook_y;
                    s.glow_flashes.push(("player".to_string(), 20));

                    let all_objs: Vec<(String, f32)> =
                        s.live_hooks.iter().map(|n| (n.clone(), HOOK_R * 2.0))
                        .chain(s.pad_live.iter().map(|n| (n.clone(), PAD_H)))
                        .chain(s.spinner_live.iter().map(|n| (n.clone(), SPINNER_H)))
                        .chain(s.coin_live.iter().map(|n| (n.clone(), COIN_R * 2.0)))
                        .chain(s.flip_live.iter().map(|n| (n.clone(), FLIP_H)))
                        .chain(s.score_x2_live.iter().map(|n| (n.clone(), SCORE_X2_H)))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_top"), GATE_TOP_SEG_H)))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_bot"), GATE_BOT_SEG_H)))
                        .collect();
                    drop(s);

                    for (obj_name, obj_h) in &all_objs {
                        if let Some(obj) = c.get_game_object_mut(obj_name) {
                            obj.position.1 = VH - obj.position.1 - obj_h;
                        }
                    }
                    if let Some(obj) = c.get_game_object_mut("player") {
                        obj.set_glow(GlowConfig { color: Color(255, 245, 120, 255), width: 18.0 });
                    }
                    // Hide the timer HUD
                    if let Some(obj) = c.get_game_object_mut("flip_timer") {
                        obj.visible = false;
                    }

                    s = st.lock().unwrap();
                }
            }

