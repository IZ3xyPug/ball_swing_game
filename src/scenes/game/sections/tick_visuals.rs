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
            // Uses engine smooth_zoom + zoom_anchor instead of manual per-object transform.
            // Gravity-aware: normal gravity anchors at VH (bottom),
            // flipped gravity anchors at 0 (top).
            {
                let s = st.lock().unwrap();
                let flipped = s.gravity_dir < 0.0;
                let anchor_y = if flipped { 0.0 } else { VH };

                let target_zoom = if flipped {
                    let effective_y = s.py + s.vy.max(0.0) * ZOOM_LOOKAHEAD_T;
                    (effective_y / (VH - ZOOM_TOP_MARGIN)).clamp(1.0, ZOOM_MAX)
                } else {
                    let effective_y = s.py + s.vy.min(0.0) * ZOOM_LOOKAHEAD_T;
                    ((VH - effective_y) / (VH - ZOOM_TOP_MARGIN)).clamp(1.0, ZOOM_MAX)
                };

                let px = s.px;
                drop(s);

                if let Some(cam) = c.camera_mut() {
                    // Asymmetric lerp: fast zoom-out, slow zoom-in
                    cam.zoom_lerp_speed = if target_zoom > cam.zoom { ZOOM_OUT_LERP } else { ZOOM_IN_LERP };
                    cam.zoom_anchor = Some((px, anchor_y));
                    cam.smooth_zoom(target_zoom);
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
                drop(s);
                if let Some(cam) = c.camera_mut() {
                    cam.snap_zoom(1.0);
                }
                c.load_scene("gameover");
            }
        });
        canvas.set_var("game_tick_registered", true);
        } // end if !tick_registered
    })
