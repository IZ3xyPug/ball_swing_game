            // ── Update HUD ────────────────────────────────────────────────────
            let dist_fill = {
                let distance_now = st.lock().unwrap().distance;
                let zone_idx_now = zone_index_for_distance(distance_now);
                let zone_start = zone_idx_now as f32 * ZONE_DISTANCE_STEP;
                ((distance_now - zone_start) / ZONE_DISTANCE_STEP).clamp(0.0, 1.0)
            };
            if let Some(obj) = c.get_game_object_mut("dist_bar") {
                obj.position = (cam_x + VW * 0.5 - 460.0, 30.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (920.0, 48.0), 0.0),
                    image: bar_img(920, 48, dist_fill, 80, 220, 160).into(),
                    color: None,
                });
            }
            let (coins, momentum_now, gravity_flipped, y_now, x_now, flip_timer_val) = {
                let ss = st.lock().unwrap();
                (
                    ss.coin_count,
                    (ss.vx*ss.vx + ss.vy*ss.vy).sqrt(),
                    ss.gravity_dir < 0.0,
                    ss.py,
                    ss.px,
                    ss.flip_timer,
                )
            };
            if let Some(obj) = c.get_game_object_mut("coin_counter") {
                obj.position = (cam_x + 30.0, 40.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (300.0, 70.0), 0.0),
                    image: coin_counter_img(coins).into(),
                    color: None,
                });
            }
            if let Some(obj) = c.get_game_object_mut("momentum_counter") {
                obj.position = (cam_x + 30.0, 128.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
                    image: momentum_counter_img(momentum_now).into(),
                    color: None,
                });
            }
            if let Some(obj) = c.get_game_object_mut("gravity_indicator") {
                obj.position = (cam_x + 30.0, 200.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (220.0, 60.0), 0.0),
                    image: gravity_indicator_img(gravity_flipped, true).into(),
                    color: None,
                });
            }
            if let Some(obj) = c.get_game_object_mut("y_meter") {
                obj.position = (cam_x + 30.0, 272.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
                    image: y_counter_img(y_now).into(),
                    color: None,
                });
            }
            if let Some(obj) = c.get_game_object_mut("x_meter") {
                obj.position = (cam_x + 30.0, 344.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
                    image: x_counter_img(x_now).into(),
                    color: None,
                });
            }

            // Flip timer HUD — top-center of screen, visible only while active
            if let Some(obj) = c.get_game_object_mut("flip_timer") {
                if flip_timer_val > 0 {
                    obj.position = (cam_x + VW * 0.5 - 180.0, 460.0);
                    obj.visible = true;
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (360.0, 84.0), 0.0),
                        image: flip_timer_img(flip_timer_val, FLIP_DURATION).into(),
                        color: None,
                    });
                } else {
                    obj.visible = false;
                }
            }

            // Hide combo flash after 40 ticks
            if st.lock().unwrap().ticks % 40 == 0 {
                c.run(Action::Hide { target: Target::name("combo_flash") });
            }

            // ── Apply zoom (Dune-style: zoom out when player goes high) ──────
            // Gravity-aware: normal gravity anchors at VH (bottom),
            // flipped gravity anchors at 0 (top).
            {
                let mut s = st.lock().unwrap();
                let flipped = s.gravity_dir < 0.0;
                let anchor_y = if flipped { 0.0 } else { VH };

                let target_zoom = if flipped {
                    // Flipped: player falls up. Zoom when y increases (away from ceiling).
                    let effective_y = s.py + s.vy.max(0.0) * ZOOM_LOOKAHEAD_T;
                    (effective_y / (VH - ZOOM_TOP_MARGIN)).clamp(1.0, ZOOM_MAX)
                } else {
                    // Normal: player falls down. Zoom when y decreases (away from floor).
                    let effective_y = s.py + s.vy.min(0.0) * ZOOM_LOOKAHEAD_T;
                    ((VH - effective_y) / (VH - ZOOM_TOP_MARGIN)).clamp(1.0, ZOOM_MAX)
                };

                let lerp = if target_zoom > s.zoom { ZOOM_OUT_LERP } else { ZOOM_IN_LERP };
                s.zoom += (target_zoom - s.zoom) * lerp;
                if (s.zoom - 1.0).abs() < 0.003 && (target_zoom - 1.0).abs() < 0.001 {
                    s.zoom = 1.0;
                }

                let z = s.zoom;
                if z > 1.001 {
                    let zcx = s.px;
                    s.zoom_cx = zcx;
                    s.zoom_anchor_y = anchor_y;

                    let world_objs: Vec<(String, (f32, f32))> =
                        s.live_hooks.iter().map(|n| (n.clone(), (HOOK_R*2.0, HOOK_R*2.0)))
                        .chain(s.pad_live.iter().map(|n| (n.clone(), (PAD_W, PAD_H))))
                        .chain(s.spinner_live.iter().map(|n| (n.clone(), (SPINNER_W, SPINNER_H))))
                        .chain(s.coin_live.iter().map(|n| (n.clone(), (COIN_R*2.0, COIN_R*2.0))))
                        .chain(std::iter::once((
                            "coin_magnet_radius".to_string(),
                            (COIN_MAGNET_RADIUS * 2.0, COIN_MAGNET_RADIUS * 2.0),
                        )))
                        .chain(s.flip_live.iter().map(|n| (n.clone(), (FLIP_W, FLIP_H))))
                        .chain(s.score_x2_live.iter().map(|n| (n.clone(), (SCORE_X2_W, SCORE_X2_H))))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_top"), (GATE_W, GATE_TOP_SEG_H))))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_bot"), (GATE_W, GATE_BOT_SEG_H))))
                        .collect();
                    drop(s);

                    // Zoom world objects: anchor at ground, shrink toward it
                    for (name, base_size) in &world_objs {
                        if let Some(obj) = c.get_game_object_mut(name) {
                            obj.position.0 = zcx + (obj.position.0 - zcx) / z;
                            obj.position.1 = anchor_y + (obj.position.1 - anchor_y) / z;
                            obj.size = (base_size.0 / z, base_size.1 / z);
                        }
                    }
                    // Zoom player
                    if let Some(obj) = c.get_game_object_mut("player") {
                        obj.position.1 = anchor_y + (obj.position.1 - anchor_y) / z;
                        obj.size = (PLAYER_R * 2.0 / z, PLAYER_R * 2.0 / z);
                    }
                    // Zoom rope
                    if let Some(obj) = c.get_game_object_mut("rope") {
                        if obj.visible {
                            obj.position.0 = zcx + (obj.position.0 - zcx) / z;
                            obj.position.1 = anchor_y + (obj.position.1 - anchor_y) / z;
                            obj.size = (obj.size.0 / z, obj.size.1 / z);
                        }
                    }
                } else {
                    drop(s);
                }
            }

            // ── Death: off-screen in current gravity direction ───────────────
            let mut s = st.lock().unwrap();
            let dead_now = (s.gravity_dir > 0.0 && s.py > VH + 150.0)
                || (s.gravity_dir < 0.0 && s.py < -150.0);
            if dead_now {
                c.set_var("last_distance", s.distance);
                c.set_var("last_coins", s.coin_count as i32);
                s.dead = true;
                s.zoom = 1.0; // reset zoom before scene change
                drop(s);
                c.load_scene("gameover");
            }
        });
        canvas.set_var("game_tick_registered", true);
        } // end if !tick_registered
    })
