use quartz::*;

use crate::constants::*;
use crate::hud::*;
use crate::images::*;
use crate::objects::*;

/// All pools and starter hook names created during scene construction.
pub struct PoolSets {
    pub starter_names: Vec<String>,
    pub pool_free:     Vec<String>,
    pub pad_free:      Vec<String>,
    pub spinner_free:  Vec<String>,
    pub coin_free:     Vec<String>,
    pub flip_free:     Vec<String>,
    pub score_x2_free: Vec<String>,
    pub zero_g_free:   Vec<String>,
    pub gate_free:     Vec<String>,
    pub gwell_free:    Vec<String>,
    pub turret_free:   Vec<String>,
    pub bullet_free:   Vec<String>,
    pub coin_static_sprite:  Image,
    pub coin_anim_template:  Option<AnimatedSprite>,
    #[allow(dead_code)]
    pub score_x2_anim_template: Option<AnimatedSprite>,
}
pub fn build_scene_objects(ctx: &mut Context) -> (Scene, PoolSets) {
    // ── Background images ────────────────────────────────────────────────
    let bg_zone_start = gradient_rect(4, VH as u32, C_SKY_TOP, C_SKY_BOT);

    let mut bg = GameObject::new_rect(
        ctx, "bg".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
            image: bg_zone_start.into(),
            color: None,
        }),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    bg.ignore_zoom = true;

    let mut bg_space = GameObject::new_rect(
        ctx, "bg_space".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (VW, VH), 0.0),
            image: gradient_rect(4, VH as u32, C_SKY_TOP, C_SKY_BOT).into(),
            color: None,
        }),
        (VW, VH), (0.0, 0.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    bg_space.ignore_zoom = true;
    bg_space.visible = false;
    bg_space.set_tint(Color(255, 255, 255, 0));

    // ── Asteroid (top-right of space background) ─────────────────────────
    const ASTEROID_W: f32 = 480.0;
    const ASTEROID_H: f32 = 480.0;
    let asteroid_decoded = image::open(ASSET_ASTEROID)
        .or_else(|_| image::open(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/asteroid.png")));
    let asteroid_img = if let Ok(decoded) = asteroid_decoded {
        Image {
            shape: ShapeType::Rectangle(0.0, (ASTEROID_W, ASTEROID_H), 0.0),
            image: decoded.into_rgba8().into(),
            color: None,
        }
    } else {
        // Fallback keeps the game bootable if the external asset is malformed.
        Image {
            shape: ShapeType::Rectangle(0.0, (ASTEROID_W, ASTEROID_H), 0.0),
            image: solid(120, 120, 132, 255).into(),
            color: None,
        }
    };
    let mut asteroid = GameObject::new_rect(
        ctx, "asteroid".into(),
        Some(asteroid_img),
        (ASTEROID_W, ASTEROID_H),
        (VW - ASTEROID_W - 80.0, 80.0),
        vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    asteroid.ignore_zoom = true;

    // ── Player — engine-native gravity ───────────────────────────────────
    let mut player = GameObject::new_rect(
        ctx, "player".into(),
        Some(Image {
            shape: ShapeType::Ellipse(0.0, (PLAYER_R*2.0, PLAYER_R*2.0), 0.0),
            image: circle_cached(PLAYER_R as u32, C_PLAYER.0, C_PLAYER.1, C_PLAYER.2),
            color: None,
        }),
        (PLAYER_R*2.0, PLAYER_R*2.0),
        (SPAWN_X - PLAYER_R, SPAWN_Y - PLAYER_R),
        vec!["player".into()],
        (18.0, 0.0),   // initial rightward push
        (1.0, 1.0),   // no engine resistance
        0.0,           // gravity set to 0 initially (hooked at start)
    );
    // Opt into gravity well forces.
    player.gravity_all_sources = true;
    player.gravity_falloff = GravityFalloff::InverseSquare;

    // Rope
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

    // Danger floor
    let mut floor = GameObject::new_rect(
        ctx, "danger_floor".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (VW, 28.0), 0.0),
            image: solid(C_DANGER.0, C_DANGER.1, C_DANGER.2, 200).into(),
            color: None,
        }),
        (VW, 28.0), (0.0, VH - 28.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    floor.ignore_zoom = true;

    // ── HUD elements ─────────────────────────────────────────────────────
    let mut dist_bar = GameObject::new_rect(
        ctx, "dist_bar".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (920.0, 48.0), 0.0),
            image: bar_img(920, 48, 0.0, 80, 220, 160).into(),
            color: None,
        }),
        (920.0, 48.0), (VW * 0.5 - 460.0, 30.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    dist_bar.ignore_zoom = true;

    let mut coin_counter = GameObject::new_rect(
        ctx, "coin_counter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (420.0, 98.0), 0.0),
            image: coin_counter_img(0).into(),
            color: None,
        }),
        (420.0, 98.0), (30.0, 40.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    coin_counter.ignore_zoom = true;

    let mut score_counter = GameObject::new_rect(
        ctx, "score_counter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (420.0, 98.0), 0.0),
            image: score_counter_img(0).into(),
            color: None,
        }),
        (420.0, 98.0), (VW - 450.0, 40.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    score_counter.ignore_zoom = true;

    let mut momentum_counter = GameObject::new_rect(
        ctx, "momentum_counter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (420.0, 86.0), 0.0),
            image: momentum_counter_img(0.0).into(),
            color: None,
        }),
        (420.0, 86.0), (30.0, 150.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    momentum_counter.ignore_zoom = true;

    let mut gravity_indicator = GameObject::new_rect(
        ctx, "gravity_indicator".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (308.0, 84.0), 0.0),
            image: gravity_indicator_img(false, true).into(),
            color: None,
        }),
        (308.0, 84.0), (30.0, 248.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    gravity_indicator.ignore_zoom = true;

    let mut y_meter = GameObject::new_rect(
        ctx, "y_meter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (420.0, 86.0), 0.0),
            image: y_counter_img(SPAWN_Y).into(),
            color: None,
        }),
        (420.0, 86.0), (30.0, 344.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    y_meter.ignore_zoom = true;

    let mut x_meter = GameObject::new_rect(
        ctx, "x_meter".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (420.0, 86.0), 0.0),
            image: x_counter_img(SPAWN_X).into(),
            color: None,
        }),
        (420.0, 86.0), (30.0, 442.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    x_meter.ignore_zoom = true;

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
    combo_flash.ignore_zoom = true;

    let mut pause_overlay = {
        const PO_OVERSCAN: f32 = 400.0;
        let po_w = VW + PO_OVERSCAN * 2.0;
        let mut obj = GameObject::new_rect(
            ctx, "pause_overlay".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (po_w, VH), 0.0),
                image: pause_overlay_img().into(),
                color: None,
            }),
            (po_w, VH), (-PO_OVERSCAN, 0.0),
            vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        );
        obj
    };
    pause_overlay.visible = false;
    pause_overlay.layer = 10_000;
    pause_overlay.ignore_zoom = true;

    let mut flip_timer_hud = GameObject::new_rect(
        ctx, "flip_timer".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (504.0, 118.0), 0.0),
            image: flip_timer_img(FLIP_DURATION, FLIP_DURATION).into(),
            color: None,
        }),
        (504.0, 118.0), (VW * 0.5 - 252.0, 560.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    flip_timer_hud.visible = false;
    flip_timer_hud.ignore_zoom = true;

    let mut zero_g_timer_hud = GameObject::new_rect(
        ctx, "zero_g_timer".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (504.0, 118.0), 0.0),
            image: flip_timer_img(ZERO_G_DURATION, ZERO_G_DURATION).into(),
            color: None,
        }),
        (504.0, 118.0), (VW * 0.5 - 252.0, 690.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    zero_g_timer_hud.visible = false;
    zero_g_timer_hud.ignore_zoom = true;

    let mut coin_magnet_radius = {
        let d = (COIN_MAGNET_RADIUS * 2.0).round().max(2.0) as u32;
        let mut img = image::RgbaImage::new(d, d);
        let r = COIN_MAGNET_RADIUS;
        let ctr = r;
        for py in 0..d {
            for px in 0..d {
                let dx = px as f32 + 0.5 - ctr;
                let dy = py as f32 + 0.5 - ctr;
                let dist = (dx * dx + dy * dy).sqrt();
                if (dist - r).abs() <= 2.0 {
                    img.put_pixel(px, py, image::Rgba([255, 245, 140, 200]));
                } else {
                    img.put_pixel(px, py, image::Rgba([0, 0, 0, 0]));
                }
            }
        }
        GameObject::new_rect(
            ctx, "coin_magnet_radius".into(),
            Some(Image {
                shape: ShapeType::Rectangle(0.0, (d as f32, d as f32), 0.0),
                image: img.into(),
                color: None,
            }),
            (d as f32, d as f32),
            (SPAWN_X - COIN_MAGNET_RADIUS, SPAWN_Y - COIN_MAGNET_RADIUS),
            vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
        )
    };
    coin_magnet_radius.visible = false;

    // ── Starter hooks ────────────────────────────────────────────────────
    let starter_hooks: &[(f32, f32)] = &[
        (START_HOOK_X,       START_HOOK_Y),
        (SPAWN_X + 1060.0,   VH * 0.30),
        (SPAWN_X + 1860.0,  VH * 0.46),
        (SPAWN_X + 2760.0,  VH * 0.34),
        (SPAWN_X + 3720.0,  VH * 0.52),
    ];

    let mut scene = Scene::new("game")
        .with_object("bg",           bg)
        .with_object("bg_space",     bg_space)
        .with_object("asteroid",     asteroid)
        .with_object("danger_floor", floor)
        .with_object("rope",         rope)
        .with_object("player",       player)
        .with_object("dist_bar",     dist_bar)
        .with_object("coin_counter", coin_counter)
        .with_object("score_counter", score_counter)
        .with_object("momentum_counter", momentum_counter)
        .with_object("gravity_indicator", gravity_indicator)
        .with_object("y_meter", y_meter)
        .with_object("x_meter", x_meter)
        .with_object("combo_flash",  combo_flash)
        .with_object("flip_timer", flip_timer_hud)
        .with_object("zero_g_timer", zero_g_timer_hud)
        .with_object("coin_magnet_radius", coin_magnet_radius);

    // ── Hook pool ────────────────────────────────────────────────────────
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

    // ── Pad pool ─────────────────────────────────────────────────────────
    let mut pad_free: Vec<String> = Vec::new();
    for i in 0..PAD_POOL_SIZE {
        let id = format!("pad_{i}");
        let mut obj = make_pad(ctx, &id, -3000.0, -3000.0);
        obj.visible = false;
        pad_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Spinner pool ─────────────────────────────────────────────────────
    let mut spinner_free: Vec<String> = Vec::new();
    for i in 0..SPINNER_POOL_SIZE {
        let id = format!("spinner_{i}");
        let mut obj = make_spinner(ctx, &id, -3500.0, -3500.0);
        obj.visible = false;
        spinner_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Coin pool ────────────────────────────────────────────────────────
    let coin_static_sprite = load_image_sized(ASSET_COIN_GIF, COIN_R * 2.0, COIN_R * 2.0);
    let coin_anim_template = AnimatedSprite::new(
        include_bytes!("../../../assets/coin.gif"),
        (COIN_R * 2.0, COIN_R * 2.0),
        12.0,
    ).ok();
    let score_x2_anim_template = AnimatedSprite::new(
        include_bytes!("../../../assets/2x.gif"),
        (SCORE_X2_W, SCORE_X2_H),
        12.0,
    ).ok();

    let mut coin_free: Vec<String> = Vec::new();
    for i in 0..COIN_POOL_SIZE {
        let id = format!("coin_{i}");
        let mut obj = make_coin(ctx, &id, -3700.0, -3700.0);
        obj.set_image(coin_static_sprite.clone());
        obj.visible = false;
        coin_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Flip pool ────────────────────────────────────────────────────────
    let mut flip_free: Vec<String> = Vec::new();
    for i in 0..FLIP_POOL_SIZE {
        let id = format!("flip_{i}");
        let mut obj = make_flip(ctx, &id, -3800.0, -3800.0);
        obj.visible = false;
        flip_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Score x2 pool ────────────────────────────────────────────────────
    let score_x2_sprite = load_image_sized(ASSET_SCORE_X2_GIF, SCORE_X2_W, SCORE_X2_H);
    let mut score_x2_free: Vec<String> = Vec::new();
    for i in 0..SCORE_X2_POOL_SIZE {
        let id = format!("score_x2_{i}");
        let mut obj = make_score_x2(ctx, &id, -3850.0, -3850.0);
        obj.set_image(score_x2_sprite.clone());
        if let Some(anim) = &score_x2_anim_template {
            obj.set_animation(anim.clone());
        }
        obj.visible = false;
        score_x2_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Zero-g pool ──────────────────────────────────────────────────────
    let mut zero_g_free: Vec<String> = Vec::new();
    for i in 0..ZERO_G_POOL_SIZE {
        let id = format!("zero_g_{i}");
        let mut obj = make_zero_g(ctx, &id, -3875.0, -3875.0);
        obj.visible = false;
        zero_g_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Gate pool ────────────────────────────────────────────────────────
    let mut gate_free: Vec<String> = Vec::new();
    for i in 0..GATE_POOL_SIZE {
        let gid = format!("gate_{i}");
        let top_id = format!("{gid}_top");
        let bot_id = format!("{gid}_bot");

        let mut top_obj = make_gate_segment(ctx, &top_id, -3900.0, -3900.0, GATE_TOP_SEG_H, gate_top_image_cached());
        top_obj.visible = false;
        scene = scene.with_object(top_id, top_obj);

        let mut bot_obj = make_gate_segment(ctx, &bot_id, -3900.0, -3900.0, GATE_BOT_SEG_H, gate_bot_image_cached());
        bot_obj.visible = false;
        scene = scene.with_object(bot_id, bot_obj);

        gate_free.push(gid);
    }

    // ── Gravity well pool ────────────────────────────────────────────────
    let mut gwell_free: Vec<String> = Vec::new();
    for i in 0..GWELL_POOL_SIZE {
        let id = format!("gwell_{i}");
        let default_visual_r = PLAYER_R * GWELL_VISUAL_SCALE_MIN;
        let mut obj = make_gravity_well(ctx, &id, -4000.0, -4000.0, GWELL_RADIUS_MIN, GWELL_STRENGTH_MIN, default_visual_r);
        obj.visible = false;
        gwell_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Turret pool ──────────────────────────────────────────────────────
    let mut turret_free: Vec<String> = Vec::new();
    for i in 0..TURRET_POOL_SIZE {
        let id = format!("turret_{i}");
        let mut obj = make_turret(ctx, &id, -4500.0, -4500.0);
        obj.visible = false;
        turret_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // ── Bullet pool ──────────────────────────────────────────────────────
    let mut bullet_free: Vec<String> = Vec::new();
    for i in 0..BULLET_POOL_SIZE {
        let id = format!("bullet_{i}");
        let mut obj = make_turret_bullet(ctx, &id);
        obj.visible = false;
        bullet_free.push(id.clone());
        scene = scene.with_object(id, obj);
    }

    // Pause overlay last so it renders above everything.
    scene = scene.with_object("pause_overlay", pause_overlay);

    // ── Pause menu buttons (above overlay) ───────────────────────────────
    let pause_btn_w: f32 = 700.0;
    let pause_btn_h: f32 = 170.0;
    let pause_btn_x: f32 = (VW - pause_btn_w) / 2.0;
    let pause_title_w: f32 = 650.0;
    let pause_title_h: f32 = 100.0;

    let mut pause_title = GameObject::new_rect(
        ctx, "pause_title".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (pause_title_w, pause_title_h), 0.0),
            image: pause_title_img().into(),
            color: None,
        }),
        (pause_title_w, pause_title_h), ((VW - pause_title_w) / 2.0, VH * 0.20),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    pause_title.visible = false;
    pause_title.layer = 10_001;
    pause_title.ignore_zoom = true;

    let make_pause_btn = |ctx: &mut Context, name: &str, r: u8, g: u8, b: u8, label: &str, y: f32| {
        let img = pause_btn_img(pause_btn_w as u32, pause_btn_h as u32, r, g, b, label);
        let corner_r = (pause_btn_h * 0.48 * 1.33).clamp(1.0, pause_btn_h * 0.5 - 1.0);
        let mut obj = GameObject::new_rect(
            ctx, name.to_string().into(),
            Some(Image {
                shape: ShapeType::RoundedRectangle(0.0, (pause_btn_w, pause_btn_h), 0.0, corner_r),
                image: img.into(),
                color: None,
            }),
            (pause_btn_w, pause_btn_h), (pause_btn_x, y),
            vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
        );
        obj.visible = false;
        obj.layer = 10_001;
        obj.ignore_zoom = true;
        obj
    };

    let pause_resume_btn = make_pause_btn(ctx, "pause_resume_btn", 50, 160, 90, "RESUME", 780.0);
    let pause_restart_btn = make_pause_btn(ctx, "pause_restart_btn", 60, 120, 200, "RESTART", 1000.0);
    let pause_settings_btn = make_pause_btn(ctx, "pause_settings_btn", 80, 80, 100, "SETTINGS", 1220.0);
    let pause_menu_btn = make_pause_btn(ctx, "pause_menu_btn", 170, 65, 65, "MENU", 1440.0);

    let mut start_prompt_text = GameObject::build("start_prompt_text")
        .size(1300.0, 120.0)
        .position((VW - 1300.0) * 0.5, VH * 0.50)
        .tag("hud")
        .build(ctx);
    start_prompt_text.visible = false;
    start_prompt_text.layer = 10_002;
    start_prompt_text.ignore_zoom = true;

    let mut settings_text = GameObject::build("settings_text")
        .size(1400.0, 800.0)
        .position((VW - 1400.0) * 0.5, VH * 0.15)
        .tag("hud")
        .build(ctx);
    settings_text.visible = false;
    settings_text.layer = 10_002;
    settings_text.ignore_zoom = true;

    let settings_back_btn = make_pause_btn(ctx, "settings_back_btn", 80, 80, 100, "BACK", 1660.0);

    scene = scene
        .with_object("pause_title", pause_title)
        .with_object("pause_resume_btn", pause_resume_btn)
        .with_object("pause_restart_btn", pause_restart_btn)
        .with_object("pause_settings_btn", pause_settings_btn)
        .with_object("pause_menu_btn", pause_menu_btn)
        .with_object("start_prompt_text", start_prompt_text)
        .with_object("settings_text", settings_text)
        .with_object("settings_back_btn", settings_back_btn);

    let pools = PoolSets {
        starter_names,
        pool_free,
        pad_free,
        spinner_free,
        coin_free,
        flip_free,
        score_x2_free,
        zero_g_free,
        gate_free,
        gwell_free,
        turret_free,
        bullet_free,
        coin_static_sprite,
        coin_anim_template,
        score_x2_anim_template,
    };

    (scene, pools)
}
