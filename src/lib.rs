use quartz::*;
use ramp::prism;
use std::collections::HashSet;

mod constants;
mod images;
mod hud;
mod state;
mod objects;
mod menu;

#[cfg(test)]
mod sim_tests;

use constants::*;
use images::*;
use hud::*;
use state::*;
use objects::*;
use menu::*;

// ─────────────────────────────────────────────────────────────────────────────
// Game scene
// ─────────────────────────────────────────────────────────────────────────────
fn build_game_scene(ctx: &mut Context) -> Scene {
    // Background — screen-sized gradient, repositioned each tick to follow the camera.
    // Texture must be ≤8192px on any axis (GPU limit), so we never make it world-sized.
    let bg = GameObject::new_rect(
        ctx, "bg".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
            image: gradient_rect(4, VH as u32, C_SKY_TOP, C_SKY_BOT).into(),
            color: None,
        }),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    // Player — a circle; gravity=0 and resistance=1 so we manage physics manually
    let player = GameObject::new_rect(
        ctx, "player".into(),
        Some(Image {
            shape: ShapeType::Ellipse(0.0, (PLAYER_R*2.0, PLAYER_R*2.0), 0.0),
            image: circle_img(PLAYER_R as u32, C_PLAYER.0, C_PLAYER.1, C_PLAYER.2).into(),
            color: None,
        }),
        (PLAYER_R*2.0, PLAYER_R*2.0),
        (SPAWN_X - PLAYER_R, SPAWN_Y - PLAYER_R),
        vec!["player".into()],
        (8.0, 0.0),  // initial rightward push
        (1.0, 1.0),  // no engine resistance
        0.0,         // no engine gravity — we apply it ourselves
    );

    // Rope — image rebuilt every frame while hooked
    let mut rope = GameObject::new_rect(
        ctx, "rope".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (4.0, 4.0), 0.0),
            image: solid(C_ROPE.0, C_ROPE.1, C_ROPE.2, 255).into(),
            color: None,
        }),
        (4.0, 4.0), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    rope.visible = false;

    // Red danger strip — screen-wide, repositioned each tick to follow camera.
    // Death is detected by py position check, not collision, so width doesn't matter.
    let floor = GameObject::new_rect(
        ctx, "danger_floor".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (VW, 28.0), 0.0),
            image: solid(C_DANGER.0, C_DANGER.1, C_DANGER.2, 200).into(),
            color: None,
        }),
        (VW, 28.0), (0.0, VH - 28.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    // HUD: distance bar
    let dist_bar = GameObject::new_rect(
        ctx, "dist_bar".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (500.0, 40.0), 0.0),
            image: bar_img(500, 40, 0.0, 80, 220, 160).into(),
            color: None,
        }),
        (500.0, 40.0), (VW - 580.0, 50.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let coin_counter = GameObject::new_rect(
        ctx, "coin_counter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (300.0, 70.0), 0.0),
            image: coin_counter_img(0).into(),
            color: None,
        }),
        (300.0, 70.0), (30.0, 40.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let boost_meter = GameObject::new_rect(
        ctx, "boost_meter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (320.0, 34.0), 0.0),
            image: bar_img(320, 34, 0.0, 120, 255, 140).into(),
            color: None,
        }),
        (320.0, 34.0), (30.0, 128.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let momentum_counter = GameObject::new_rect(
        ctx, "momentum_counter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
            image: momentum_counter_img(0.0).into(),
            color: None,
        }),
        (300.0, 62.0), (30.0, 176.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let gravity_indicator = GameObject::new_rect(
        ctx, "gravity_indicator".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (220.0, 60.0), 0.0),
            image: gravity_indicator_img(false, true).into(),
            color: None,
        }),
        (220.0, 60.0), (30.0, 248.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let y_meter = GameObject::new_rect(
        ctx, "y_meter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
            image: y_counter_img(SPAWN_Y).into(),
            color: None,
        }),
        (300.0, 62.0), (30.0, 320.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    let x_meter = GameObject::new_rect(
        ctx, "x_meter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
            image: x_counter_img(SPAWN_X).into(),
            color: None,
        }),
        (300.0, 62.0), (30.0, 392.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    // Combo flash — shown briefly when grabbing a hook at high speed
    let mut combo_flash = {
        let (w, h) = (420u32, 80u32);
        let mut img = image::RgbaImage::new(w, h);
        for py in 0..h { for px in 0..w {
            img.put_pixel(px, py, image::Rgba([255, 200, 60, 230]));
        }}
        GameObject::new_rect(
            ctx, "combo_flash".into(),
            Some(Image { shape: ShapeType::Rectangle(0.0, (w as f32, h as f32), 0.0), image: img.into(), color: None }),
            (w as f32, h as f32), (VW/2.0 - w as f32/2.0, VH*0.08),
            vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };
    combo_flash.visible = false;

    let mut pause_overlay = GameObject::new_rect(
        ctx, "pause_overlay".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
            image: pause_overlay_img().into(),
            color: None,
        }),
        (VW, VH), (0.0, 0.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    pause_overlay.visible = false;

    let mut flip_timer_hud = GameObject::new_rect(
        ctx, "flip_timer".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (360.0, 84.0), 0.0),
            image: flip_timer_img(FLIP_DURATION, FLIP_DURATION).into(),
            color: None,
        }),
        (360.0, 84.0), (VW * 0.5 - 180.0, 460.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    flip_timer_hud.visible = false;

    let mut coin_magnet_radius = {
        let d = (COIN_MAGNET_RADIUS * 2.0).round().max(2.0) as u32;
        let mut img = image::RgbaImage::new(d, d);
        let r = COIN_MAGNET_RADIUS;
        let c = r;
        for py in 0..d {
            for px in 0..d {
                let dx = px as f32 + 0.5 - c;
                let dy = py as f32 + 0.5 - c;
                let dist = (dx * dx + dy * dy).sqrt();
                if (dist - r).abs() <= 2.0 {
                    img.put_pixel(px, py, image::Rgba([255, 245, 140, 200]));
                } else {
                    img.put_pixel(px, py, image::Rgba([0, 0, 0, 0]));
                }
            }
        }
        GameObject::new_rect(
            ctx,
            "coin_magnet_radius".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (d as f32, d as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (d as f32, d as f32),
            (SPAWN_X - COIN_MAGNET_RADIUS, SPAWN_Y - COIN_MAGNET_RADIUS),
            vec![],
            (0.0, 0.0),
            (1.0, 1.0),
            0.0,
        )
    };
    coin_magnet_radius.visible = false;

    // Pre-place a handful of close hooks so there's something to grab immediately.
    // The full hook pool is preallocated here (with ctx available), then recycled.
    let starter_hooks: &[(f32, f32)] = &[
        (START_HOOK_X,     START_HOOK_Y),
        (SPAWN_X + 1060.0,  VH * 0.30),
        (SPAWN_X + 1860.0, VH * 0.46),
        (SPAWN_X + 2760.0, VH * 0.34),
        (SPAWN_X + 3720.0, VH * 0.52),
    ];

    let mut scene = Scene::new("game")
        .with_object("bg",           bg)
        .with_object("danger_floor", floor)
        .with_object("rope",         rope)
        .with_object("player",       player)
        .with_object("dist_bar",     dist_bar)
        .with_object("coin_counter", coin_counter)
        .with_object("boost_meter",  boost_meter)
        .with_object("momentum_counter", momentum_counter)
        .with_object("gravity_indicator", gravity_indicator)
        .with_object("y_meter", y_meter)
        .with_object("x_meter", x_meter)
        .with_object("combo_flash",  combo_flash)
        .with_object("pause_overlay", pause_overlay)
        .with_object("flip_timer", flip_timer_hud)
        .with_object("coin_magnet_radius", coin_magnet_radius);

    let mut starter_names: Vec<String> = Vec::new();
    let mut pool_free: Vec<String> = Vec::new();
    for i in 0..HOOK_POOL_SIZE {
        let id = format!("hook_{i}");
        let mut obj = if i < starter_hooks.len() {
            let (hx, hy) = starter_hooks[i];
            make_hook(ctx, &id, hx, hy)
        } else {
            make_hook(ctx, &id, -2000.0, -2000.0)
        };

        if i < starter_hooks.len() {
            starter_names.push(id.clone());
        } else {
            obj.visible = false;
            pool_free.push(id.clone());
        }

        scene = scene.with_object(id, obj);
    }

    // Preallocate bounce pad pool (same pattern as hooks)
    let mut pad_free: Vec<String> = Vec::new();
    for i in 0..PAD_POOL_SIZE {
        let id = format!("pad_{i}");
        let mut obj = make_pad(ctx, &id, -3000.0, -3000.0);
        obj.visible = false;
        pad_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // Preallocate spinning obstacle pool
    let mut spinner_free: Vec<String> = Vec::new();
    for i in 0..SPINNER_POOL_SIZE {
        let id = format!("spinner_{i}");
        let mut obj = make_spinner(ctx, &id, -3500.0, -3500.0);
        obj.visible = false;
        spinner_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // Preallocate phasing boost pool
    let mut boost_free: Vec<String> = Vec::new();
    for i in 0..BOOST_POOL_SIZE {
        let id = format!("boost_{i}");
        let mut obj = make_boost(ctx, &id, -3600.0, -3600.0);
        obj.visible = false;
        boost_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // Decode the coin GIF once. We keep pooled coins static until first use,
    // then enable animation lazily to avoid startup lag spikes.
    let coin_static_sprite = load_image_sized("assets/coin.gif", COIN_R * 2.0, COIN_R * 2.0);
    let coin_anim_template = AnimatedSprite::new(
        include_bytes!("../assets/coin.gif"),
        (COIN_R * 2.0, COIN_R * 2.0),
        12.0,
    ).ok();

    // Preallocate sparse coin pool
    let mut coin_free: Vec<String> = Vec::new();
    for i in 0..COIN_POOL_SIZE {
        let id = format!("coin_{i}");
        let mut obj = make_coin(ctx, &id, -3700.0, -3700.0);
        obj.set_image(coin_static_sprite.clone());
        obj.visible = false;
        coin_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // Preallocate gravity flip pickup pool
    let mut flip_free: Vec<String> = Vec::new();
    for i in 0..FLIP_POOL_SIZE {
        let id = format!("flip_{i}");
        let mut obj = make_flip(ctx, &id, -3800.0, -3800.0);
        obj.visible = false;
        flip_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // Preallocate flappy-style gate pool (each id owns top+bottom objects)
    let mut gate_free: Vec<String> = Vec::new();
    for i in 0..GATE_POOL_SIZE {
        let gid = format!("gate_{i}");
        let top_id = format!("{gid}_top");
        let bot_id = format!("{gid}_bot");

        let mut top_obj = make_gate_segment(
            ctx,
            &top_id,
            -3900.0,
            -3900.0,
            GATE_TOP_SEG_H,
            gate_top_image_cached(),
        );
        top_obj.visible = false;
        scene = scene.with_object(top_id, top_obj);

        let mut bot_obj = make_gate_segment(
            ctx,
            &bot_id,
            -3900.0,
            -3900.0,
            GATE_BOT_SEG_H,
            gate_bot_image_cached(),
        );
        bot_obj.visible = false;
        scene = scene.with_object(bot_id, bot_obj);

        gate_free.push(gid);
    }

    let scene = scene;

    scene.on_enter(move |canvas| {
        use std::sync::{Arc, Mutex};

        // ── Background music (looped) ────────────────────────────────────
        let bgm_started = matches!(canvas.get_var("bgm_started"), Some(Value::Bool(true)));
        if !bgm_started {
            canvas.play_sound_with(
                "assets/synful_reach.mp3",
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
                    c.resume_audio();
                    if let Some(obj) = c.get_game_object_mut("pause_overlay") {
                        obj.visible = false;
                    }
                } else {
                    // Position overlay to cover the current camera view
                    let cam_x = c.camera().map(|cam| cam.position.0).unwrap_or(0.0);
                    if let Some(obj) = c.get_game_object_mut("pause_overlay") {
                        obj.position = (cam_x, 0.0);
                        obj.visible = true;
                    }
                    c.pause_audio();
                    c.pause();
                }
            });
            canvas.set_var("pause_key_registered", true);
        }

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

        let state = Arc::new(Mutex::new(State {
            px: SPAWN_X, py: SPAWN_Y,
            vx: 13.0,    vy: 0.0,
            hooked: true,
            hook_x: start_hook.0, hook_y: start_hook.1,
            rope_len: start_rope_len,
            active_hook: "hook_0".into(),
            distance:   0.0,
            score:      0,
            coin_count: 0,
            boost_charge: 0.0,
            difficulty: 0.0,
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
            spinners_enabled: true,
            spinner_spin_enabled: true,
            spinner_hit_cooldown: 0,
            boost_live:      Vec::new(),
            boost_free:      boost_free.clone(),
            boost_rightmost: SPAWN_X,
            coin_live:      Vec::new(),
            coin_free:      coin_free.clone(),
            coin_rightmost: SPAWN_X,
            coin_magnet_locked: Vec::new(),
            magnet_debug: false,
            flip_live:      Vec::new(),
            flip_free:      flip_free.clone(),
            flip_rightmost: SPAWN_X + VW * 1.1,
            flip_timer:     0,
            gate_live:      Vec::new(),
            gate_free:      gate_free.clone(),
            gate_rightmost: SPAWN_X + VW * 1.0,
            bounce_enabled: true,
            dark_mode: false,
            glow_flashes: Vec::new(),
            zoom: 1.0,
            zoom_cx: SPAWN_X,
            zoom_anchor_y: VH,
        }));

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
        canvas.on_mouse_press(move |c, btn, _pos| {
            if btn != MouseButton::Left { return; }
            c.run(Action::Custom { name: "do_grab".into() });
        });
        canvas.on_mouse_release(move |c, btn, _pos| {
            if btn != MouseButton::Left { return; }
            c.run(Action::Custom { name: "do_release".into() });
        });

        // ── Release logic ─────────────────────────────────────────────────────
        let st = state.clone();
        canvas.register_custom_event("do_release".into(), move |c| {
            let mut s = st.lock().unwrap();
            if s.dead || !s.hooked { return; }

            // Add release impulse from swing speed, scaled so weak swings get tiny/no bonus.
            let dx = s.px - s.hook_x;
            let dy = s.py - s.hook_y;
            let dist = (dx*dx + dy*dy).sqrt().max(1.0);
            let nx = dx / dist;
            let ny = dy / dist;
            let tx = -ny;
            let ty = nx;
            let tangent_v = s.vx * tx + s.vy * ty;
            let swing_speed = tangent_v.abs();
            let surge = ((swing_speed - RELEASE_MIN_SWING_SPEED).max(0.0) * RELEASE_SURGE_SCALE)
                .clamp(0.0, RELEASE_SURGE_MAX);
            if surge > 0.0 {
                let dir = if tangent_v.abs() > 0.01 { tangent_v.signum() } else { 1.0 };
                s.vx += tx * surge * dir;
                s.vy += ty * surge * dir;
            }

            // Double momentum on release for bigger launches
            s.vx *= 2.0;
            s.vy *= 2.0;

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

                // Slightly boost tangential speed on attach for snappier re-grabs.
                let dx = s.px - hx;
                let dy = s.py - hy;
                let inv_dist = 1.0 / (dx*dx + dy*dy).sqrt().max(1.0);
                let nx = dx * inv_dist;
                let ny = dy * inv_dist;
                let tx = -ny;
                let ty = nx;
                let tangent_v = s.vx * tx + s.vy * ty;
                    let dir = if tangent_v.abs() > 0.05 {
                        tangent_v.signum()
                    } else if s.vx.abs() > 0.05 {
                        s.vx.signum()
                    } else if s.px >= hx {
                        1.0
                    } else {
                        -1.0
                    };
                    s.vx += tx * GRAB_SURGE * dir;
                    s.vy += ty * GRAB_SURGE * dir;

                    s.hooked     = true;
                    s.hook_x     = hx;
                    s.hook_y     = hy;
                    s.rope_len   = rope_len;
                    s.active_hook = hook_id.clone();
                    s.pad_bounce_count = 0;
                    s.score      += (speed * 2.0) as u32;
                    let do_combo  = speed > 16.0;
                    drop(s);

                    // Swing sound
                    c.play_sound_with("assets/swoosh.mp3", SoundOptions::new().volume(3.0));

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
        let st = state.clone();
        let mut space_was_down = false;
        let mut w_was_down = false;
        let mut one_was_down = false;
        let mut two_was_down = false;
        let mut three_was_down = false;
        let mut four_was_down = false;
        let mut five_was_down = false;
        let mut six_was_down = false;
        let mut seven_was_down = false;
        let mut prev_nearest_hook: String = String::new();
        let mut dark_mode_prev = false;
        canvas.on_update(move |c| {
            // ── Early-exit for stale callbacks from previous game sessions ───
            {
                let s = st.lock().unwrap();
                if s.dead { return; }
            }

            // ── Un-zoom from previous frame ───────────────────────────────────
            // Reverses the zoom applied at the end of the previous tick so all
            // game logic runs in real (un-zoomed) world coordinates.
            {
                let s = st.lock().unwrap();
                let z = s.zoom;
                if z > 1.001 {
                    let zcx = s.zoom_cx;
                    let zay = s.zoom_anchor_y;
                    let world_objs: Vec<(String, (f32, f32))> =
                        s.live_hooks.iter().map(|n| (n.clone(), (HOOK_R*2.0, HOOK_R*2.0)))
                        .chain(s.pad_live.iter().map(|n| (n.clone(), (PAD_W, PAD_H))))
                        .chain(s.spinner_live.iter().map(|n| (n.clone(), (SPINNER_W, SPINNER_H))))
                        .chain(s.boost_live.iter().map(|n| (n.clone(), (BOOST_W, BOOST_H))))
                        .chain(s.coin_live.iter().map(|n| (n.clone(), (COIN_R*2.0, COIN_R*2.0))))
                        .chain(std::iter::once((
                            "coin_magnet_radius".to_string(),
                            (COIN_MAGNET_RADIUS * 2.0, COIN_MAGNET_RADIUS * 2.0),
                        )))
                        .chain(s.flip_live.iter().map(|n| (n.clone(), (FLIP_W, FLIP_H))))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_top"), (GATE_W, GATE_TOP_SEG_H))))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_bot"), (GATE_W, GATE_BOT_SEG_H))))
                        .collect();
                    drop(s);
                    for (name, base_size) in &world_objs {
                        if let Some(obj) = c.get_game_object_mut(name) {
                            obj.position.0 = zcx + (obj.position.0 - zcx) * z;
                            obj.position.1 = zay + (obj.position.1 - zay) * z;
                            obj.size = *base_size;
                        }
                    }
                } else {
                    drop(s);
                }
            }
            // Space: press to grab, release to ungrab
            let space_now = c.key("space");
            if space_now && !space_was_down {
                c.run(Action::Custom { name: "do_grab".into() });
            } else if !space_now && space_was_down {
                c.run(Action::Custom { name: "do_release".into() });
            }
            space_was_down = space_now;

            let w_now = c.key("w");
            if w_now && !w_was_down {
                let mut s = st.lock().unwrap();
                if s.boost_charge >= BOOST_USE_MIN {
                    let use_amt = s.boost_charge.min(0.35);
                    s.boost_charge -= use_amt;
                    // Zero out unusable residual so the bar doesn't show a misleading sliver
                    if s.boost_charge < BOOST_USE_MIN { s.boost_charge = 0.0; }
                    let speed = (s.vx * s.vx + s.vy * s.vy).sqrt().max(1.0);
                    let dir_x = s.vx / speed;
                    let dir_y = s.vy / speed;
                    let power = 8.0 + 12.0 * use_amt;
                    s.vx += dir_x * power;
                    s.vy += dir_y * power;
                }
            }
            w_was_down = w_now;

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

            // Runtime feature toggles.
            let k1 = c.key("1");
            if k1 && !one_was_down {
                s.spinners_enabled = !s.spinners_enabled;
            }
            one_was_down = k1;

            let k2 = c.key("2");
            if k2 && !two_was_down {
                s.bounce_enabled = !s.bounce_enabled;
            }
            two_was_down = k2;

            let k3 = c.key("3");
            if k3 && !three_was_down {
                // Free flip: toggle gravity direction (same as hitting a flip block)
                if s.flip_timer > 0 {
                    // Already flipped — refresh timer
                    s.flip_timer = FLIP_DURATION;
                } else {
                    // Not flipped — do the full flip + start timer
                    s.gravity_dir *= -1.0;
                    s.flip_timer = FLIP_DURATION;
                    if s.hooked {
                        s.vy = -s.vy;
                    } else {
                        s.vy = -s.vy * 0.55;
                    }
                    s.py = VH - s.py;
                    s.hook_y = VH - s.hook_y;

                    let all_objs: Vec<(String, f32)> =
                        s.live_hooks.iter().map(|n| (n.clone(), HOOK_R * 2.0))
                        .chain(s.pad_live.iter().map(|n| (n.clone(), PAD_H)))
                        .chain(s.spinner_live.iter().map(|n| (n.clone(), SPINNER_H)))
                        .chain(s.boost_live.iter().map(|n| (n.clone(), BOOST_H)))
                        .chain(s.coin_live.iter().map(|n| (n.clone(), COIN_R * 2.0)))
                        .chain(s.flip_live.iter().map(|n| (n.clone(), FLIP_H)))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_top"), GATE_TOP_SEG_H)))
                        .chain(s.gate_live.iter().map(|n| (format!("{n}_bot"), GATE_BOT_SEG_H)))
                        .collect();
                    drop(s);
                    for (name, obj_h) in &all_objs {
                        if let Some(obj) = c.get_game_object_mut(name) {
                            obj.position.1 = VH - obj.position.1 - obj_h;
                        }
                    }
                    s = st.lock().unwrap();
                }
            }
            three_was_down = k3;

            let k4 = c.key("4");
            if k4 && !four_was_down {
                let all_disabled = !s.spinners_enabled && !s.bounce_enabled;
                if all_disabled {
                    s.spinners_enabled = true;
                    s.bounce_enabled = true;
                } else {
                    s.spinners_enabled = false;
                    s.bounce_enabled = false;
                }
            }
            four_was_down = k4;

            let k5 = c.key("5");
            if k5 && !five_was_down {
                s.spinner_spin_enabled = !s.spinner_spin_enabled;
            }
            five_was_down = k5;

            let k6 = c.key("6");
            if k6 && !six_was_down {
                s.dark_mode = !s.dark_mode;
            }
            six_was_down = k6;

            let k7 = c.key("7");
            if k7 && !seven_was_down {
                s.magnet_debug = !s.magnet_debug;
            }
            seven_was_down = k7;

            let spinner_enabled = s.spinners_enabled;
            let spinner_spin_enabled = s.spinner_spin_enabled;
            let bounce_enabled = s.bounce_enabled;
            let dark_mode = s.dark_mode;
            let spinner_live = s.spinner_live.clone();
            let pad_live = s.pad_live.clone();
            drop(s);
            for name in &spinner_live {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = spinner_enabled;
                    if spinner_spin_enabled {
                        if obj.rotation_momentum.abs() < 0.01 {
                            obj.rotation_momentum = SPINNER_ROT_SPEED;
                        }
                    } else {
                        obj.rotation_momentum = 0.0;
                    }
                    if obj.rotation > 360.0 || obj.rotation < -360.0 {
                        obj.rotation = obj.rotation.rem_euclid(360.0);
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
            while s.rightmost_x < s.px + GEN_AHEAD && !s.pool_free.is_empty() {
                if let Some(spec) = s.pending.pop_front() {
                    let Some(id) = s.pool_free.pop() else { break; };
                    s.rightmost_x = spec.x;
                    let hx = spec.x;
                    let hy = if s.gravity_dir < 0.0 { VH - spec.y } else { spec.y };
                    s.live_hooks.push(id.clone());
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
                    let diff = s.difficulty;
                    let mut next_seed = s.seed;
                    let mut next_gen_y = s.gen_y;
                    let batch = gen_hook_batch(&mut next_seed, from, &mut next_gen_y, diff);
                    s.seed = next_seed;
                    s.gen_y = next_gen_y;
                    s.pending = batch;
                }
            }

            // ── Spawn bounce pads ahead of the player ────────────────────────
            while s.bounce_enabled && s.pad_rightmost < s.px + GEN_AHEAD && !s.pad_free.is_empty() {
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
                        image: pad_image_cached(),
                        color: None,
                    });
                }

                s = st.lock().unwrap();
            }

            // ── Spawn spinning obstacles ahead of the player ─────────────────
            while s.spinners_enabled && s.spinner_rightmost < s.px + GEN_AHEAD && !s.spinner_free.is_empty() {
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
                let spin_dir = if lcg(&mut s.seed) < 0.5 { -SPINNER_ROT_SPEED } else { SPINNER_ROT_SPEED };
                let spin_enabled_now = s.spinner_spin_enabled;
                let Some(id) = s.spinner_free.pop() else { break; };
                s.spinner_live.push(id.clone());
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

            // ── Spawn phasing boosts ahead of the player ─────────────────────
            while s.boost_rightmost < s.px + GEN_AHEAD && !s.boost_free.is_empty() {
                let gap = lcg_range(&mut s.seed, BOOST_GAP_MIN, BOOST_GAP_MAX);
                let boost_x = s.boost_rightmost + gap;
                let boost_lanes = [VH * 0.40, VH * 0.48, VH * 0.56];
                let lane_i = ((lcg(&mut s.seed) * boost_lanes.len() as f32) as usize).min(boost_lanes.len() - 1);
                let raw_boost_y = (boost_lanes[lane_i] + lcg_range(&mut s.seed, -26.0, 26.0)).clamp(VH * 0.30, VH * 0.62);
                let boost_y = if s.gravity_dir < 0.0 {
                    VH - raw_boost_y - BOOST_H
                } else {
                    raw_boost_y
                };
                s.boost_rightmost = boost_x;
                let Some(id) = s.boost_free.pop() else { break; };
                s.boost_live.push(id.clone());
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&id) {
                    obj.position = (boost_x, boost_y);
                    obj.visible = true;
                }

                s = st.lock().unwrap();
            }

            // ── Spawn gravity flip pickups ahead of the player ──────────────
            while s.flip_rightmost < s.px + GEN_AHEAD && !s.flip_free.is_empty() {
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
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&id) {
                    obj.position = (flip_x, flip_y);
                    obj.visible = true;
                }

                s = st.lock().unwrap();
            }

            // ── Spawn flappy-style gate obstacles ahead of the player ───────
            // Gap obstacles disabled: procedural gate clusters removed from gameplay loop.
            while GATES_ENABLED && s.gate_rightmost < s.px + GEN_AHEAD && !s.gate_free.is_empty() {
                let gap = lcg_range(&mut s.seed, GATE_GAP_MIN, GATE_GAP_MAX);
                let base_x = s.gate_rightmost + gap.max(GATE_MIN_CLUSTER_SEPARATION);
                let gaps_in_cluster = 2 + ((lcg(&mut s.seed) * 3.0) as usize);
                let cluster_spacing = GATE_MIN_CLUSTER_SEPARATION;
                let mut spawn_batch: Vec<(String, String, f32, Option<(String, f32, f32)>)> = Vec::new();

                for i in 0..gaps_in_cluster {
                    let Some(gid) = s.gate_free.pop() else { break; };
                    let gate_x = base_x + i as f32 * cluster_spacing;
                    s.gate_live.push(gid.clone());
                    let top_id = format!("{gid}_top");
                    let bot_id = format!("{gid}_bot");

                    // Spawn a hook near each gate gap when possible.
                    let hook_spawn = if let Some(hook_id) = s.pool_free.pop() {
                        let hx = gate_x - 450.0;
                        let hy = 650.0;
                        s.live_hooks.push(hook_id.clone());
                        Some((hook_id, hx, hy))
                    } else {
                        None
                    };

                    spawn_batch.push((top_id, bot_id, gate_x, hook_spawn));
                }

                if spawn_batch.is_empty() {
                    break;
                }

                let last_gate_x = spawn_batch.last().map(|(_, _, x, _)| *x).unwrap_or(base_x);
                s.gate_rightmost = last_gate_x;
                let spinner_ids = s.spinner_live.clone();
                drop(s);

                for (top_id, bot_id, gate_x, hook_spawn) in &spawn_batch {
                    if let Some(obj) = c.get_game_object_mut(top_id) {
                        obj.position = (*gate_x, -GATE_VERTICAL_OVERFLOW);
                        obj.size = (GATE_W, GATE_TOP_SEG_H);
                        obj.visible = true;
                    }
                    if let Some(obj) = c.get_game_object_mut(bot_id) {
                        obj.position = (*gate_x, GATE_TOP_BASE_H + GATE_GAP_H);
                        obj.size = (GATE_W, GATE_BOT_SEG_H);
                        obj.visible = true;
                    }

                    // Keep gate opening clear: push overlapping spinners away.
                    for sid in &spinner_ids {
                        if let Some(sp) = c.get_game_object_mut(sid) {
                            let overlaps = sp.position.0 + SPINNER_W > *gate_x - 80.0
                                && sp.position.0 < *gate_x + GATE_W + 80.0;
                            if overlaps {
                                sp.position.0 = *gate_x + GATE_W + 240.0;
                            }
                        }
                    }

                    if let Some((hook_id, hx, hy)) = hook_spawn {
                        if let Some(obj) = c.get_game_object_mut(hook_id) {
                            obj.position = (*hx - HOOK_R, *hy - HOOK_R);
                            obj.visible = true;
                            obj.set_image(Image {
                                shape: ShapeType::Ellipse(0.0, (HOOK_R*2.0, HOOK_R*2.0), 0.0),
                                image: circle_img(HOOK_R as u32, C_HOOK.0, C_HOOK.1, C_HOOK.2).into(),
                                color: None,
                            });
                        }
                    }
                }

                s = st.lock().unwrap();
            }

            // ── Spawn sparse coins ahead of the player ───────────────────────
            while s.coin_rightmost < s.px + GEN_AHEAD && !s.coin_free.is_empty() {
                let gap = lcg_range(&mut s.seed, COIN_GAP_MIN, COIN_GAP_MAX);
                let start_x = s.coin_rightmost + gap;
                let spawn_array = s.coin_free.len() >= COIN_ARRAY_COUNT && lcg(&mut s.seed) < COIN_ARRAY_CHANCE;
                let mut spawn_batch: Vec<(String, f32, f32)> = Vec::new();

                if spawn_array {
                    // Sample center from a safe range so the arc always keeps its full shape
                    // inside the target band without clamp flattening.
                    let center_min = (COIN_ARRAY_Y_MIN + COIN_CURVE_RISE).min(COIN_ARRAY_Y_MAX);
                    let center_raw_y = lcg_range(&mut s.seed, center_min, COIN_ARRAY_Y_MAX);
                    let half = (COIN_ARRAY_COUNT as f32 - 1.0) * 0.5;

                    // Curved 5-coin formation constrained to the upper band.
                    for i in 0..COIN_ARRAY_COUNT {
                        let x = start_x + i as f32 * COIN_ARRAY_SPACING;
                        let t = i as f32 - half;
                        let norm = if half > 0.0 { (t.abs() / half).clamp(0.0, 1.0) } else { 0.0 };
                        let arch = 1.0 - norm * norm;
                        let raw_y = center_raw_y - arch * COIN_CURVE_RISE;
                        let y = if s.gravity_dir < 0.0 { VH - raw_y } else { raw_y };

                        let Some(id) = s.coin_free.pop() else { break; };
                        s.coin_live.push(id.clone());
                        spawn_batch.push((id, x, y));
                    }
                } else {
                    // Single coins can spawn anywhere below the array band.
                    let raw_y = lcg_range(&mut s.seed, COIN_SINGLE_Y_MIN, COIN_SINGLE_Y_MAX);
                    let y = if s.gravity_dir < 0.0 { VH - raw_y } else { raw_y };
                    if let Some(id) = s.coin_free.pop() {
                        s.coin_live.push(id.clone());
                        spawn_batch.push((id, start_x, y));
                    }
                }

                if spawn_batch.is_empty() {
                    break;
                }

                s.coin_rightmost = if spawn_array {
                    start_x + (COIN_ARRAY_COUNT as f32 - 1.0) * COIN_ARRAY_SPACING
                } else {
                    start_x
                };
                drop(s);

                for (id, coin_x, coin_y) in &spawn_batch {
                    if let Some(obj) = c.get_game_object_mut(id) {
                        obj.position = (*coin_x - COIN_R, *coin_y - COIN_R);
                        obj.visible = true;
                        obj.set_image(coin_spawn_image.clone());
                        if obj.animated_sprite.is_none() {
                            if let Some(anim) = &coin_spawn_anim {
                                obj.set_animation(anim.clone());
                            }
                        }
                    }
                }

                s = st.lock().unwrap();
            }

            // ── Cull pads behind the player ───────────────────────────────────
            let pad_cutoff = s.px - VW * 1.5;
            let pads_remove: Vec<String> = s.pad_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + PAD_W < pad_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &pads_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3000.0, -3000.0);
                }
            }
            let pads_rm_set: HashSet<&str> = pads_remove.iter().map(|n| n.as_str()).collect();
            s.pad_live.retain(|n| !pads_rm_set.contains(n.as_str()));
            for name in &pads_remove {
                s.pad_origins.retain(|(n, _, _, _, _)| n != name);
            }
            for name in pads_remove {
                s.pad_free.push(name);
            }

            // ── Cull spinning obstacles behind the player ────────────────────
            let spin_cutoff = s.px - VW * 1.5;
            let spins_remove: Vec<String> = s.spinner_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + SPINNER_W < spin_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &spins_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3500.0, -3500.0);
                    obj.rotation_momentum = 0.0;
                }
            }
            let spins_rm_set: HashSet<&str> = spins_remove.iter().map(|n| n.as_str()).collect();
            s.spinner_live.retain(|n| !spins_rm_set.contains(n.as_str()));
            for name in spins_remove {
                s.spinner_free.push(name);
            }

            // ── Cull boosts behind the player ─────────────────────────────────
            let boost_cutoff = s.px - VW * 1.5;
            let boosts_remove: Vec<String> = s.boost_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + BOOST_W < boost_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &boosts_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3600.0, -3600.0);
                }
            }
            let boosts_rm_set: HashSet<&str> = boosts_remove.iter().map(|n| n.as_str()).collect();
            s.boost_live.retain(|n| !boosts_rm_set.contains(n.as_str()));
            for name in boosts_remove {
                s.boost_free.push(name);
            }

            // ── Cull coins behind the player ──────────────────────────────────
            let coin_cutoff = s.px - VW * 1.5;
            let coins_remove: Vec<String> = s.coin_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + COIN_R * 2.0 < coin_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &coins_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3700.0, -3700.0);
                }
            }
            let coins_rm_set: HashSet<&str> = coins_remove.iter().map(|n| n.as_str()).collect();
            s.coin_live.retain(|n| !coins_rm_set.contains(n.as_str()));
            s.coin_magnet_locked.retain(|n| !coins_rm_set.contains(n.as_str()));
            for name in coins_remove {
                s.coin_free.push(name);
            }

            // ── Cull gravity flips behind the player ─────────────────────────
            let flip_cutoff = s.px - VW * 1.5;
            let flips_remove: Vec<String> = s.flip_live.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + FLIP_W < flip_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &flips_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-3800.0, -3800.0);
                }
            }
            let flips_rm_set: HashSet<&str> = flips_remove.iter().map(|n| n.as_str()).collect();
            s.flip_live.retain(|n| !flips_rm_set.contains(n.as_str()));
            for name in flips_remove {
                s.flip_free.push(name);
            }

            // ── Cull gates behind the player ─────────────────────────────────
            let gate_cutoff = s.px - VW * 1.5;
            let gates_remove: Vec<String> = s.gate_live.iter()
                .filter(|name| {
                    let top_id = format!("{name}_top");
                    c.get_game_object(&top_id)
                        .map(|o| o.position.0 + GATE_W < gate_cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();
            for name in &gates_remove {
                let top_id = format!("{name}_top");
                let bot_id = format!("{name}_bot");
                if let Some(obj) = c.get_game_object_mut(&top_id) {
                    obj.visible = false;
                    obj.position = (-3900.0, -3900.0);
                }
                if let Some(obj) = c.get_game_object_mut(&bot_id) {
                    obj.visible = false;
                    obj.position = (-3900.0, -3900.0);
                }
            }
            let gates_rm_set: HashSet<&str> = gates_remove.iter().map(|n| n.as_str()).collect();
            s.gate_live.retain(|n| !gates_rm_set.contains(n.as_str()));
            for name in gates_remove {
                s.gate_free.push(name);
            }

            // ── Animate moving pads ───────────────────────────────────────────
            let ticks = s.ticks;
            let pad_origins_snap: Vec<(String, f32, f32, f32, f32)> = s.pad_origins.clone();
            drop(s);
            for (id, origin_x, amp, speed, phase) in &pad_origins_snap {
                let offset = (ticks as f32 * speed * 0.02 + phase).sin() * amp;
                let new_x = origin_x + offset;
                if let Some(obj) = c.get_game_object_mut(id) {
                    obj.position.0 = new_x;
                }
            }
            s = st.lock().unwrap();

            // ── Cull hooks that have scrolled far behind the player ───────────
            // We track live_hooks ourselves — see NOT IN API note on State struct.
            let cutoff = s.px - VW * 1.5;
            let to_remove: Vec<String> = s.live_hooks.iter()
                .filter(|name| {
                    c.get_game_object(name)
                        .map(|o| o.position.0 + HOOK_R*2.0 < cutoff)
                        .unwrap_or(true)
                })
                .cloned()
                .collect();

            for name in &to_remove {
                if let Some(obj) = c.get_game_object_mut(name) {
                    obj.visible = false;
                    obj.position = (-2000.0, -2000.0);
                }
            }
            let to_remove_set: HashSet<&str> = to_remove.iter().map(|n| n.as_str()).collect();
            let active_hook_removed = s.hooked && to_remove_set.contains(s.active_hook.as_str());
            s.live_hooks.retain(|n| !to_remove_set.contains(n.as_str()));
            for name in to_remove {
                s.pool_free.push(name);
            }

            // Unhook if the active hook was culled
            if active_hook_removed {
                s.hooked = false;
                s.active_hook = String::new();
                drop(s);
                c.run(Action::Hide { target: Target::name("rope") });
                s = st.lock().unwrap();
            }

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
                tangent_v *= SWING_DRAG;
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

            // ── Speed boost collection (phase through) ───────────────────────
            let player_left   = s.px - PLAYER_R;
            let player_right  = s.px + PLAYER_R;
            let player_top    = s.py - PLAYER_R;
            let player_bottom = s.py + PLAYER_R;
            let mut hit_boost: Option<String> = None;
            for name in &s.boost_live {
                if let Some(obj) = c.get_game_object(name) {
                    let bl = obj.position.0;
                    let br = obj.position.0 + BOOST_W;
                    let bt = obj.position.1;
                    let bb = obj.position.1 + BOOST_H;
                    if player_right > bl && player_left < br && player_bottom > bt && player_top < bb {
                        hit_boost = Some(name.clone());
                        break;
                    }
                }
            }
            if let Some(name) = hit_boost {
                s.vx += BOOST_VX;
                s.vy += BOOST_VY;
                s.boost_charge = (s.boost_charge + BOOST_CHARGE_PER_PICKUP).min(1.0);
                s.boost_live.retain(|n| n != &name);
                s.boost_free.push(name.clone());
                s.glow_flashes.push(("player".to_string(), 10));
                drop(s);

                if let Some(obj) = c.get_game_object_mut(&name) {
                    obj.visible = false;
                    obj.position = (-3600.0, -3600.0);
                }
                if let Some(obj) = c.get_game_object_mut("player") {
                    obj.set_glow(GlowConfig { color: Color(120, 255, 140, 220), width: 14.0 });
                }

                s = st.lock().unwrap();
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
                        .chain(s.boost_live.iter().map(|n| (n.clone(), BOOST_H)))
                        .chain(s.coin_live.iter().map(|n| (n.clone(), COIN_R * 2.0)))
                        .chain(s.flip_live.iter().map(|n| (n.clone(), FLIP_H)))
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
                        .chain(s.boost_live.iter().map(|n| (n.clone(), BOOST_H)))
                        .chain(s.coin_live.iter().map(|n| (n.clone(), COIN_R * 2.0)))
                        .chain(s.flip_live.iter().map(|n| (n.clone(), FLIP_H)))
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
                s.score = s.score.saturating_add(COIN_SCORE);
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
                s.distance   = travelled;
                s.difficulty = (s.distance / 18000.0).min(1.0);
            }

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
            let dark_now = st.lock().unwrap().dark_mode;
            if let Some(obj) = c.get_game_object_mut("bg") {
                obj.position = (cam_x, 0.0);
                if dark_now {
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
                        image: solid(4, 4, 8, 255).into(),
                        color: None,
                    });
                } else {
                    obj.set_image(Image {
                        shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
                        image: gradient_rect(4, VH as u32, C_SKY_TOP, C_SKY_BOT).into(),
                        color: None,
                    });
                }
            }
            {
                let s = st.lock().unwrap();
                let floor_y = if s.gravity_dir < 0.0 { 0.0 } else { VH - 28.0 };
                drop(s);
                if let Some(obj) = c.get_game_object_mut("danger_floor") {
                    obj.position = (cam_x, floor_y);
                }
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
                        }
                    }
                    prev_nearest_hook = cur_nearest;
                }
            }

            // ── Update HUD ────────────────────────────────────────────────────
            let dist_fill = { st.lock().unwrap().distance / 40000.0 }.min(1.0);
            if let Some(obj) = c.get_game_object_mut("dist_bar") {
                obj.position = (cam_x + VW - 580.0, 50.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (500.0, 40.0), 0.0),
                    image: bar_img(500, 40, dist_fill, 80, 220, 160).into(),
                    color: None,
                });
            }
            let (coins, boost_fill, momentum_now, gravity_flipped, y_now, x_now, flip_timer_val) = {
                let ss = st.lock().unwrap();
                (
                    ss.coin_count,
                    ss.boost_charge,
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
            if let Some(obj) = c.get_game_object_mut("boost_meter") {
                obj.position = (cam_x + 30.0, 128.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (320.0, 34.0), 0.0),
                    image: bar_img(320, 34, boost_fill, 120, 255, 140).into(),
                    color: None,
                });
            }
            if let Some(obj) = c.get_game_object_mut("momentum_counter") {
                obj.position = (cam_x + 30.0, 176.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
                    image: momentum_counter_img(momentum_now).into(),
                    color: None,
                });
            }
            if let Some(obj) = c.get_game_object_mut("gravity_indicator") {
                obj.position = (cam_x + 30.0, 248.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (220.0, 60.0), 0.0),
                    image: gravity_indicator_img(gravity_flipped, true).into(),
                    color: None,
                });
            }
            if let Some(obj) = c.get_game_object_mut("y_meter") {
                obj.position = (cam_x + 30.0, 320.0);
                obj.set_image(Image {
                    shape: ShapeType::Rectangle(0.0, (300.0, 62.0), 0.0),
                    image: y_counter_img(y_now).into(),
                    color: None,
                });
            }
            if let Some(obj) = c.get_game_object_mut("x_meter") {
                obj.position = (cam_x + 30.0, 392.0);
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
                        .chain(s.boost_live.iter().map(|n| (n.clone(), (BOOST_W, BOOST_H))))
                        .chain(s.coin_live.iter().map(|n| (n.clone(), (COIN_R*2.0, COIN_R*2.0))))
                        .chain(std::iter::once((
                            "coin_magnet_radius".to_string(),
                            (COIN_MAGNET_RADIUS * 2.0, COIN_MAGNET_RADIUS * 2.0),
                        )))
                        .chain(s.flip_live.iter().map(|n| (n.clone(), (FLIP_W, FLIP_H))))
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
    })
}

// ─────────────────────────────────────────────────────────────────────────────
pub struct App;

impl App {
    fn new(ctx: &mut Context, _assets: Assets) -> impl Drawable {
        let mut canvas = Canvas::new(ctx, CanvasMode::Landscape);
        canvas.add_scene(build_menu_scene(ctx));
        canvas.add_scene(build_game_scene(ctx));
        canvas.add_scene(build_gameover_scene(ctx));
        canvas.load_scene("menu");
        canvas
    }
}

ramp::run! { |ctx: &mut Context, assets: Assets| { App::new(ctx, assets) } }

