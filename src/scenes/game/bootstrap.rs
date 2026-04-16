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

    // ── Player — engine-native gravity ───────────────────────────────────
    let mut player = GameObject::new_rect(
        ctx, "player".into(),
        Some(solid_ellipse(PLAYER_R*2.0, PLAYER_R*2.0, Color(C_PLAYER.0, C_PLAYER.1, C_PLAYER.2, 255))),
        (PLAYER_R*2.0, PLAYER_R*2.0),
        (SPAWN_X - PLAYER_R, SPAWN_Y - PLAYER_R),
        vec!["player".into()],
        (8.0, 0.0),   // initial rightward push
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
    let floor = GameObject::new_rect(
        ctx, "danger_floor".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (VW, 28.0), 0.0),
            image: solid(C_DANGER.0, C_DANGER.1, C_DANGER.2, 200).into(),
            color: None,
        }),
        (VW, 28.0), (0.0, VH - 28.0), vec![], (0.0, 0.0), (1.0, 1.0), 0.0,
    );

    // ── HUD elements ─────────────────────────────────────────────────────
    let dist_bar = GameObject::new_rect(
        ctx, "dist_bar".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (920.0, 48.0), 0.0),
            image: bar_img(920, 48, 0.0, 80, 220, 160).into(),
            color: None,
        }),
        (920.0, 48.0), (VW * 0.5 - 460.0, 30.0),
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
    pause_overlay.layer = 10_000;
    pause_overlay.ignore_zoom = true;

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

    let mut zero_g_timer_hud = GameObject::new_rect(
        ctx, "zero_g_timer".into(),
        Some(Image {
            shape: ShapeType::Rectangle(0.0, (360.0, 84.0), 0.0),
            image: flip_timer_img(ZERO_G_DURATION, ZERO_G_DURATION).into(),
            color: None,
        }),
        (360.0, 84.0), (VW * 0.5 - 180.0, 556.0),
        vec!["hud".into()], (0.0, 0.0), (1.0, 1.0), 0.0,
    );
    zero_g_timer_hud.visible = false;

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
        .with_object("danger_floor", floor)
        .with_object("rope",         rope)
        .with_object("player",       player)
        .with_object("dist_bar",     dist_bar)
        .with_object("coin_counter", coin_counter)
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

    // Pause overlay last so it renders above everything.
    scene = scene.with_object("pause_overlay", pause_overlay);

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
        coin_static_sprite,
        coin_anim_template,
        score_x2_anim_template,
    };

    (scene, pools)
}
