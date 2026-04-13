        let st = state.clone();
        canvas.register_custom_event("do_release".into(), move |c| {
            let mut s = st.lock().unwrap();
            if s.dead || !s.hooked { return; }

            apply_release_impulse(&mut s);

            let prev = s.active_hook.clone();
            s.hooked = false;
            s.active_hook = String::new();
            drop(s);

            c.run(Action::Hide { target: Target::name("rope") });

            // Restore hook colour to default
            if !prev.is_empty() {
                if let Some(obj) = c.get_game_object_mut(&prev) {
                    obj.set_image(Image {
                        shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                        image: circle_img(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2).into(),
                        color: None,
                    });
                }
            }
        });

        // ── Grab logic ────────────────────────────────────────────────────────
        let st = state.clone();
        canvas.register_custom_event("do_grab".into(), move |c| {
            let mut s = st.lock().unwrap();
            if s.dead || s.hooked { return; }

            // ── Find nearest hook via objects_in_radius ────────────
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
                let speed    = (s.vx*s.vx + s.vy*s.vy).sqrt();

                apply_grab_impulse(&mut s, hx, hy);

                    s.hooked     = true;
                    s.hook_x     = hx;
                    s.hook_y     = hy;
                    s.rope_len   = rope_len;
                    s.active_hook = hook_id.clone();
                    s.pad_bounce_count = 0;
                    let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
                    s.score      += ((speed * 2.0) as u32).saturating_mul(score_mult);
                    let do_combo  = speed > 16.0;
                    drop(s);

                    // Swing sound
                    c.play_sound_with(ASSET_SWOOSH_SFX, SoundOptions::new().volume(3.0));

                    // Highlight active hook
                    if let Some(obj) = c.get_game_object_mut(&hook_id) {
                        obj.set_image(Image {
                            shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                            image: circle_img(HOOK_R as u32, C_HOOK_ON.0, C_HOOK_ON.1, C_HOOK_ON.2).into(),
                            color: None,
                        });
                        obj.set_glow(GlowConfig { color: Color(220, 80, 30, 200), width: 8.0 });
                    }
                    // Track glow flash for hook
                    {
                        let mut s2 = st.lock().unwrap();
                        s2.glow_flashes.push((hook_id.clone(), 15));
                    }

                    c.run(Action::Show { target: Target::name("rope") });
                    if do_combo {
                        c.run(Action::Show { target: Target::name("combo_flash") });
                    }
                }
        });

        // ── Main tick ─────────────────────────────────────────────────────────
