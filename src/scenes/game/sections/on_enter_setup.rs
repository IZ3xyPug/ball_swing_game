    scene.on_enter(move |canvas| {
        // Quartz particles run through the crystalline step path.
        // Enable once so player emitter can render.
        let crystalline_ready = matches!(canvas.get_var("crystalline_ready"), Some(Value::Bool(true)));
        if !crystalline_ready {
            canvas.enable_crystalline();
            canvas.set_var("crystalline_ready", true);
        }

        // Player particle trail (Quartz particles branch).
        canvas.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
        let player_trail = EmitterBuilder::new(PLAYER_TRAIL_EMITTER_NAME)
            .rate(72.0)
            .lifetime(0.68)
            .velocity(-2.0, 8.0)
            .spread(6.0, 6.0)
            .size(9.0)
            .color(C_PLAYER.0, C_PLAYER.1, C_PLAYER.2, 255)
            .gravity_scale(0.0)
            .collision(CollisionResponse::None)
            .build();
        canvas.add_emitter(player_trail);
        canvas.attach_emitter_to(PLAYER_TRAIL_EMITTER_NAME, "player");

        // ── Background music (looped) ────────────────────────────────────
        // Usually started from menu on first boot; keep this as a fallback
        // in case game scene is entered directly.
        let bgm_started = matches!(canvas.get_var("bgm_started"), Some(Value::Bool(true)));
        if !bgm_started {
            canvas.play_sound_with(
                ASSET_BGM_TRACK,
                SoundOptions::new().volume(0.5).looping(true),
            );
            canvas.set_var("bgm_started", true);
        }

        // Camera follows the player horizontally across the huge world
        // Camera world width is large but not texture-backed — just a scroll bound.
        // No texture is created at this size; it's just a coordinate clamp.
        let mut cam = Camera::new((VW*80.0, VH), (VW, VH));
        cam.follow(Some(Target::name("player")));
        cam.lerp_speed = 0.10;
        canvas.set_camera(cam);

        // ── Pause toggle (P key) ─────────────────────────────────────────
        // Register once globally; callbacks persist across scene reloads.
        let pause_key_registered = matches!(canvas.get_var("pause_key_registered"), Some(Value::Bool(true)));
        if !pause_key_registered {
            canvas.on_key_press(|c, key| {
                let is_pause_key = *key == Key::Character("p".into());
                if !is_pause_key || !c.is_scene("game") { return; }

                if c.is_paused() {
                    c.resume();
                    c.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
                    c.clear_particles();
                    let player_trail = EmitterBuilder::new(PLAYER_TRAIL_EMITTER_NAME)
                        .rate(72.0)
                        .lifetime(0.68)
                        .velocity(-2.0, 8.0)
                        .spread(6.0, 6.0)
                        .size(9.0)
                        .color(C_PLAYER.0, C_PLAYER.1, C_PLAYER.2, 255)
                        .gravity_scale(0.0)
                        .collision(CollisionResponse::None)
                        .build();
                    c.add_emitter(player_trail);
                    c.attach_emitter_to(PLAYER_TRAIL_EMITTER_NAME, "player");
                    c.set_var("pause_animating", false);
                    c.set_var("pause_anim_frames", 0);
                    if let Some(obj) = c.get_game_object_mut("pause_overlay") {
                        obj.visible = false;
                    }
                } else {
                    let animating = matches!(c.get_var("pause_animating"), Some(Value::Bool(true)));
                    if animating { return; }

                    // Keep particles from rendering over the pause panel.
                    c.remove_emitter(PLAYER_TRAIL_EMITTER_NAME);
                    c.clear_particles();

                    // Start overlay above the screen and slide it in.
                    // ignore_zoom objects use screen-space coords: (0,0) = top-left.
                    if let Some(obj) = c.get_game_object_mut("pause_overlay") {
                        obj.position = (0.0, -VH);
                        obj.visible = true;
                    }
                    c.set_var("pause_anim_total", PAUSE_MENU_ANIM_FRAMES);
                    c.set_var("pause_anim_frames", PAUSE_MENU_ANIM_FRAMES);
                    c.set_var("pause_animating", true);
                }
            });
            canvas.set_var("pause_key_registered", true);
        }

        canvas.set_var("pause_anim_frames", 0);
        canvas.set_var("pause_anim_total", PAUSE_MENU_ANIM_FRAMES);
        canvas.set_var("pause_animating", false);

        let mut seed: u64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xDEAD_BEEF);
        // Keep opening hooks fixed so the starting route is always reliable.
        // Generate procedural hooks after the fixed starter section.
        // Pending hooks are consumed in insertion order (near to far).
        let mut gen_y = starter_hooks.last().map(|(_, y)| *y).unwrap_or(SPAWN_Y);
        let first_from = starter_hooks
            .last()
            .map(|(x, _)| *x + 620.0)
            .unwrap_or(SPAWN_X + 2000.0);
        let first_batch = gen_hook_batch(&mut seed, first_from, &mut gen_y, 0.0);
        // rightmost_x tracks the last *spawned* hook, not the last *pending* one.
        // Initialize to the last starter hook so the spawn loop places pending hooks.
        let rightmost_x = starter_hooks.last().map(|(x, _)| *x).unwrap_or(SPAWN_X);

        let start_hook = starter_hooks.first().copied().unwrap_or((START_HOOK_X, START_HOOK_Y));
        let start_rope_len = ((SPAWN_X - start_hook.0).powi(2) + (SPAWN_Y - start_hook.1).powi(2)).sqrt();

        let coin_spawn_anim = coin_anim_template.clone();
        let coin_spawn_image = coin_static_sprite.clone();

        let fresh_state = State {
            px: SPAWN_X, py: SPAWN_Y,
            vx: 18.0,    vy: 0.0,
            hooked: true,
            hook_x: start_hook.0, hook_y: start_hook.1,
            rope_len: start_rope_len,
            active_hook: "hook_0".into(),
            distance:   0.0,
            score:      0,
            coin_count: 0,
            gravity_dir: 1.0,
            seed,
            pending:     first_batch,
            live_hooks:  starter_names.clone(),
            pool_free:   pool_free.clone(),
            gen_y,
            rightmost_x,
            dead:  false,
            ticks: 0,
            pad_live:      Vec::new(),
            pad_free:      pad_free.clone(),
            pad_rightmost: SPAWN_X,
            pad_origins:   Vec::new(),
            pad_bounce_count: 0,
            spinner_live:      Vec::new(),
            spinner_free:      spinner_free.clone(),
            spinner_rightmost: SPAWN_X + VW * 0.65,
            spinner_origins:   Vec::new(),
            spinners_enabled: true,
            spinner_spin_enabled: true,
            spinner_hit_cooldown: 0,
            coin_free:      coin_free.clone(),
            coin_rightmost: SPAWN_X,
            coin_magnet_locked: Vec::new(),
            magnet_debug: false,
            flip_live:      Vec::new(),
            flip_free:      flip_free.clone(),
            flip_rightmost: SPAWN_X + VW * 1.1,
            flip_timer:     0,
            score_x2_live:      Vec::new(),
            score_x2_free:      score_x2_free.clone(),
            score_x2_rightmost: SPAWN_X + VW * 1.35,
            score_x2_timer:     0,
            gate_live:      Vec::new(),
            gate_free:      gate_free.clone(),
            gate_rightmost: SPAWN_X + VW * 1.0,
            bounce_enabled: true,
            dark_mode: false,
            glow_flashes: Vec::new(),
        };

        // Reuse persistent Arc across respawns so on_update keeps working.
        {
            let mut slot = persistent_state.lock().unwrap();
            if let Some(existing) = slot.as_ref() {
                *existing.lock().unwrap() = fresh_state;
            } else {
                *slot = Some(Arc::new(Mutex::new(fresh_state)));
            }
        }
        let state = persistent_state.lock().unwrap().as_ref().unwrap().clone();

        // Start already attached and moving forward.
        if let Some(obj) = canvas.get_game_object_mut("hook_0") {
            obj.set_image(Image {
                shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                image: circle_img(HOOK_R as u32, C_HOOK_ON.0, C_HOOK_ON.1, C_HOOK_ON.2).into(),
                color: None,
            });
        }
        canvas.run(Action::Show { target: Target::name("rope") });

        // ── Grapple on mouse press, release on mouse release ────────────────
        // Register only once so callbacks don't stack across respawns.
        // They fire custom events which are replaced each on_enter, so the
        // latest State arc is always used.
        let mouse_registered = matches!(canvas.get_var("game_mouse_registered"), Some(Value::Bool(true)));
        if !mouse_registered {
            canvas.on_mouse_press(move |c, btn, _pos| {
                if btn != MouseButton::Left || !c.is_scene("game") { return; }
                c.run(Action::Custom { name: "do_grab".into() });
            });
            canvas.on_mouse_release(move |c, btn, _pos| {
                if btn != MouseButton::Left || !c.is_scene("game") { return; }
                c.run(Action::Custom { name: "do_release".into() });
            });
            canvas.set_var("game_mouse_registered", true);
        }

        // ── Release logic ─────────────────────────────────────────────────────
