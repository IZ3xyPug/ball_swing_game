            // ── Score x2 pickup (5s timer refresh) ─────────────────────────
            let player_left   = s.px - PLAYER_R;
            let player_right  = s.px + PLAYER_R;
            let player_top    = s.py - PLAYER_R;
            let player_bottom = s.py + PLAYER_R;
            let mut hit_score_x2: Option<String> = None;
            for name in &s.score_x2_live {
                if let Some(obj) = c.get_game_object(name) {
                    let xl = obj.position.0;
                    let xr = obj.position.0 + SCORE_X2_W;
                    let xt = obj.position.1;
                    let xb = obj.position.1 + SCORE_X2_H;
                    if player_right > xl && player_left < xr && player_bottom > xt && player_top < xb {
                        hit_score_x2 = Some(name.clone());
                        break;
                    }
                }
            }
            if let Some(name) = hit_score_x2 {
                s.score_x2_timer = SCORE_X2_DURATION;
                s.score_x2_live.retain(|n| n != &name);
                s.score_x2_free.push(name.clone());
                s.glow_flashes.push(("player".to_string(), 12));
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&name) {
                    obj.visible = false;
                    obj.position = (-3850.0, -3850.0);
                }
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.set_glow(GlowConfig { color: Color(255, 220, 120, 220), width: 14.0 });
                }

                s = st.lock().unwrap();
            }

            // Score x2 countdown
            if s.score_x2_timer > 0 {
                s.score_x2_timer -= 1;
            }

            // ── Coin magnet pull (radius + latch) ───────────────────────────
            // Runs AFTER physics so we use the current-frame player position.
            {
                let coin_live_now = s.coin_live.clone();
                let (px_m, py_m) = (s.px, s.py);

                // Latch coins when they first enter radius.
                for name in &coin_live_now {
                    if s.coin_magnet_locked.iter().any(|n| n == name) {
                        continue;
                    }
                    if let Some(obj) = c.get_game_object(name) {
                        let ccx = obj.position.0 + COIN_R;
                        let ccy = obj.position.1 + COIN_R;
                        let dx = px_m - ccx;
                        let dy = py_m - ccy;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist <= COIN_MAGNET_RADIUS {
                            s.coin_magnet_locked.push(name.clone());
                        }
                    }
                }

                let locked_now = s.coin_magnet_locked.clone();
                let magnet_debug_now = s.magnet_debug;
                drop(s);

                // Pull latched coins toward the player using proportional approach
                // so they always converge regardless of player speed.
                for name in &locked_now {
                    if let Some(obj) = c.get_game_object_mut(name) {
                        let ccx = obj.position.0 + COIN_R;
                        let ccy = obj.position.1 + COIN_R;
                        let dx = px_m - ccx;
                        let dy = py_m - ccy;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist > 0.5 {
                            // Move coin COIN_MAGNET_PULL fraction of the remaining distance
                            obj.position.0 += dx * COIN_MAGNET_PULL;
                            obj.position.1 += dy * COIN_MAGNET_PULL;
                        } else {
                            // Snap when very close
                            obj.position.0 = px_m - COIN_R;
                            obj.position.1 = py_m - COIN_R;
                        }
                    }
                }

                if let Some(obj) = c.get_game_object_mut("coin_magnet_radius") {
                    obj.position = (px_m - COIN_MAGNET_RADIUS, py_m - COIN_MAGNET_RADIUS);
                    obj.visible = magnet_debug_now;
                }

                s = st.lock().unwrap();
            }

            // ── Coin pickup (sparse, pink) ───────────────────────────────────
            let player_left   = s.px - PLAYER_R;
            let player_right  = s.px + PLAYER_R;
            let player_top    = s.py - PLAYER_R;
            let player_bottom = s.py + PLAYER_R;
            let mut hit_coin: Option<String> = None;
            for name in &s.coin_live {
                if let Some(obj) = c.get_game_object(name) {
                    let cl = obj.position.0;
                    let cr = obj.position.0 + COIN_R * 2.0;
                    let ct = obj.position.1;
                    let cb = obj.position.1 + COIN_R * 2.0;
                    if player_right > cl && player_left < cr && player_bottom > ct && player_top < cb {
                        hit_coin = Some(name.clone());
                        break;
                    }
                }
            }
            if let Some(name) = hit_coin {
                let score_mult = if s.score_x2_timer > 0 { 2 } else { 1 };
                s.score = s.score.saturating_add(COIN_SCORE.saturating_mul(score_mult));
                s.coin_count = s.coin_count.saturating_add(1);
                s.coin_live.retain(|n| n != &name);
                s.coin_magnet_locked.retain(|n| n != &name);
                s.coin_free.push(name.clone());
                s.glow_flashes.push(("player".to_string(), 8));
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&name) {
                    obj.visible = false;
                    obj.position = (-3700.0, -3700.0);
                }
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.set_glow(GlowConfig { color: Color(255, 95, 210, 200), width: 10.0 });
                }

                s = st.lock().unwrap();
            }

            // Track distance
            let travelled = (s.px - SPAWN_X).max(0.0);
            if travelled > s.distance {
                s.distance = travelled;
            }

            let zone_idx = zone_index_for_distance(s.distance);
            let dark_now = s.dark_mode;
            let floor_y = if s.gravity_dir < 0.0 { 0.0 } else { VH - 28.0 };

            // ── Sync player object position ───────────────────────────────────
            // NOT IN API: no Action::SetPosition. We set obj.position directly
            // and zero engine momentum to prevent double-integration.
            // SUGGESTED API ADDITION: Action::SetPosition { target, x, y }
            let (px, py) = (s.px, s.py);
            drop(s);

            if let Some(obj) = c.get_game_object_mut("player") {
                obj.position = (px - PLAYER_R, py - PLAYER_R);
                obj.momentum = (0.0, 0.0);
            }

            // Pin background and floor to the camera position each tick
            // so they always fill the screen without being world-sized textures.
            let cam_x = c.camera().map(|cam| cam.position.0).unwrap_or(0.0);
            if let Some(obj) = c.get_game_object_mut("bg") {
                obj.position = (cam_x, 0.0);
                let bg_theme = (dark_now, zone_idx);
                if prev_bg_theme != Some(bg_theme) {
                    let image_data = if dark_now {
                        solid(4, 4, 8, 255)
                    } else {
                        match zone_idx {
                            0 => bg_zone_start_img.clone(),
                            1 => bg_zone_purple_img.clone(),
                            _ => bg_zone_black_img.clone(),
                        }
                    };
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
                        image: image_data.into(),
                        color: None,
                    });
                    prev_bg_theme = Some(bg_theme);
                }
            }
            if let Some(obj) = c.get_game_object_mut("danger_floor") {
                obj.position = (cam_x, floor_y);
            }

            // ── Dark mode: set/clear glows only on transition ──────────────
            if dark_now && !dark_mode_prev {
                // Entering dark mode — apply ambient glows
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.set_glow(GlowConfig {
                        color: Color(80, 255, 180, 220),
                        width: 28.0,
                    });
                }
                if let Some(obj) = c.get_game_object_mut("danger_floor") {
                    obj.set_glow(GlowConfig { color: Color(200, 50, 50, 180), width: 14.0 });
                }
                // Apply glows to all live spinners/pads
                {
                    let s = st.lock().unwrap();
                    let spinners = s.spinner_live.clone();
                    let pads = s.pad_live.clone();
                    drop(s);
                    for name in &spinners {
                        if let Some(obj) = c.get_game_object_mut(name) {
                            obj.set_glow(GlowConfig { color: Color(255, 60, 50, 160), width: 10.0 });
                        }
                    }
                    for name in &pads {
                        if let Some(obj) = c.get_game_object_mut(name) {
                            obj.set_glow(GlowConfig { color: Color(60, 180, 255, 160), width: 10.0 });
                        }
                    }
                }
            } else if !dark_now && dark_mode_prev {
                // Exiting dark mode — clear all glows
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.clear_highlight();
                }
                if let Some(obj) = c.get_game_object_mut("danger_floor") {
                    obj.clear_highlight();
                }
                if let Some(obj) = c.get_game_object_mut("rope") {
                    obj.clear_highlight();
                }
                {
                    let s = st.lock().unwrap();
                    let spinners = s.spinner_live.clone();
                    let pads = s.pad_live.clone();
                    drop(s);
                    for name in &spinners {
                        if let Some(obj) = c.get_game_object_mut(name) {
                            obj.clear_highlight();
                        }
                    }
                    for name in &pads {
                        if let Some(obj) = c.get_game_object_mut(name) {
                            obj.clear_highlight();
                        }
                    }
                }
            }
            // Rope glow — set each frame in dark mode when visible (rope visibility changes)
            if dark_now {
                if let Some(obj) = c.get_game_object_mut("rope") {
                    if obj.visible {
                        if obj.highlight.is_none() {
                            obj.set_glow(GlowConfig { color: Color(220, 220, 255, 180), width: 8.0 });
                        }
                    }
                }
            }
            dark_mode_prev = dark_now;

            // ── Highlight nearest grabbable hook ──────────────────────────────
            {
                let s = st.lock().unwrap();
                let cur_nearest = if !s.hooked {
                    if let Some(player_obj) = c.get_game_object("player") {
                        c.objects_in_radius(player_obj, ROPE_LEN_MAX)
                            .into_iter()
                            .filter(|o| o.tags.iter().any(|t| t == "hook"))
                            .map(|o| {
                                let hcx = o.position.0 + HOOK_R;
                                let hcy = o.position.1 + HOOK_R;
                                let dx = hcx - s.px;
                                let dy = hcy - s.py;
                                (o.id.clone(), (dx*dx + dy*dy).sqrt())
                            })
                            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                            .map(|(id, _)| id)
                            .unwrap_or_default()
                    } else { String::new() }
                } else { String::new() };
                let active = s.active_hook.clone();
                drop(s);

                if cur_nearest != prev_nearest_hook {
                    // Reset old highlight (unless it's the actively hooked one)
                    if !prev_nearest_hook.is_empty() && prev_nearest_hook != active {
                        if let Some(obj) = c.get_game_object_mut(&prev_nearest_hook) {
                            obj.set_image(Image {
                                shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                                image: circle_img(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2).into(),
                                color: None,
                            });
                            obj.clear_glow();
                        }
                    }
                    // Set new highlight
                    if !cur_nearest.is_empty() && cur_nearest != active {
                        if let Some(obj) = c.get_game_object_mut(&cur_nearest) {
                            obj.set_image(Image {
                                shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                                image: circle_img(HOOK_R as u32, C_HOOK_NEAR.0, C_HOOK_NEAR.1, C_HOOK_NEAR.2).into(),
                                color: None,
                            });
                            obj.set_glow(GlowConfig { color: Color(255, 170, 80, 220), width: 10.0 });
                        }
                    }
                    prev_nearest_hook = cur_nearest;
                }
            }

